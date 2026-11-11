//! Panic containment at the X-Plane ABI boundary.
//!
//! X-Plane calls plugin code through `extern "C"` functions, and a panic must
//! never unwind out of one. [`guard`] runs a callback body under
//! `catch_unwind`. The first panic logs the callback name and message through
//! `XPLMDebugString`, latches the plugin as faulted, and runs the fault hook a
//! provider registered with [`set_fault_hook`] exactly once. From then on every
//! guarded callback returns its default without running plugin code, so state
//! a panic left half-updated is never observed again. Teardown entry points use
//! [`guard_teardown`], which still runs after a fault so registrations can be
//! released before X-Plane unloads the plugin.
//!
//! The guard only takes effect in unwinding builds. With `panic = "abort"` a
//! panic still aborts before the guard sees it, exactly as without the guard.

use std::{
    any::Any,
    cell::RefCell,
    panic::{self, AssertUnwindSafe},
    sync::{
        Mutex, OnceLock, PoisonError,
        atomic::{AtomicBool, Ordering},
    },
};

/// The first panic caught at the plugin boundary.
#[derive(Clone, Copy, Debug)]
pub struct Fault<'a> {
    /// The guarded entry point or callback that caught the panic.
    pub callback: &'static str,
    /// The panic message, with its source location when one was recorded.
    pub message: &'a str,
}

/// Runs once, after the first panic, to put the plugin's resources into a
/// safe state. It must not assume any borrow is available: the panic may have
/// happened while an outer callback held one. A panic inside the hook is
/// contained and logged; the hook never runs a second time.
pub type FaultHook = fn(&Fault<'_>);

/// A panic boundary with its own fault latch and hook.
///
/// Plugins use the process-wide [`PLUGIN`] boundary through the free
/// functions in this module. Separate instances exist for tests.
pub struct Boundary {
    faulted: AtomicBool,
    hook_started: AtomicBool,
    hook: Mutex<Option<FaultHook>>,
    log: Mutex<Option<fn(&str)>>,
    label: OnceLock<String>,
}

impl Default for Boundary {
    fn default() -> Self {
        Self::new()
    }
}

impl Boundary {
    /// Creates an unfaulted boundary. Diagnostics go to standard error until
    /// a logger is installed with [`Boundary::set_logger`].
    pub const fn new() -> Self {
        Self {
            faulted: AtomicBool::new(false),
            hook_started: AtomicBool::new(false),
            hook: Mutex::new(None),
            log: Mutex::new(None),
            label: OnceLock::new(),
        }
    }

    /// Names the plugin in diagnostics. Only the first label is kept.
    pub fn set_label(&self, label: &str) {
        let _ = self.label.set(label.to_owned());
    }

    /// Routes diagnostics through `log`, for example [`xplane_log`].
    pub fn set_logger(&self, log: fn(&str)) {
        *self.log.lock().unwrap_or_else(PoisonError::into_inner) = Some(log);
    }

    fn log(&self, text: &str) {
        let log = *self.log.lock().unwrap_or_else(PoisonError::into_inner);
        match log {
            Some(log) => log(text),
            None => {
                use std::io::Write;
                let _ = std::io::stderr().write_all(text.as_bytes());
            }
        }
    }

    /// Installs the hook run after the first panic and returns the previous one.
    pub fn set_fault_hook(&self, hook: Option<FaultHook>) -> Option<FaultHook> {
        let mut slot = self.hook.lock().unwrap_or_else(PoisonError::into_inner);
        std::mem::replace(&mut *slot, hook)
    }

    /// Whether a panic has been caught at this boundary.
    pub fn faulted(&self) -> bool {
        self.faulted.load(Ordering::SeqCst)
    }

    /// Runs one callback body. After a fault, returns `default` without
    /// running `body`. A panic in `body` is reported and yields `default`.
    pub fn call<R>(&self, callback: &'static str, default: R, body: impl FnOnce() -> R) -> R {
        if self.faulted() {
            return default;
        }
        self.contain(callback, default, body)
    }

    /// Runs a teardown body even after a fault, containing any panic.
    ///
    /// Plugin stop and disable must still release SDK registrations, or
    /// X-Plane would call back into a plugin that has been unloaded.
    pub fn call_teardown<R>(
        &self,
        callback: &'static str,
        default: R,
        body: impl FnOnce() -> R,
    ) -> R {
        self.contain(callback, default, body)
    }

    fn contain<R>(&self, callback: &'static str, default: R, body: impl FnOnce() -> R) -> R {
        match panic::catch_unwind(AssertUnwindSafe(body)) {
            Ok(value) => value,
            Err(payload) => {
                self.report_panic(callback, payload);
                default
            }
        }
    }

    /// Reports a panic that a caller caught itself, with the same latch, log
    /// and hook as a guarded callback. Use it where a callback must recover
    /// locally before returning to the SDK.
    pub fn report_panic(&self, callback: &'static str, payload: Box<dyn Any + Send>) {
        let message = describe(payload.as_ref());
        // A payload whose destructor panics would abort while unwinding has
        // already finished; leaking one small allocation is the safer choice.
        std::mem::forget(payload);
        self.fault(callback, &message);
    }

    /// Latches a fault detected without a panic, for example a cleanup that
    /// could not release its registration, and runs the hook once.
    pub fn report_fault(&self, callback: &'static str, message: &str) {
        self.fault(callback, message);
    }

    fn fault(&self, callback: &'static str, message: &str) {
        let first = !self.faulted.swap(true, Ordering::SeqCst);
        let label = self.label.get().map_or("plugin", String::as_str);
        let state = if first {
            "; callbacks are now disabled"
        } else {
            ""
        };
        self.log(&format!("{label}: fault in {callback}: {message}{state}\n"));
        if self.hook_started.swap(true, Ordering::SeqCst) {
            return;
        }
        let hook = *self.hook.lock().unwrap_or_else(PoisonError::into_inner);
        let Some(hook) = hook else {
            return;
        };
        let fault = Fault { callback, message };
        if let Err(payload) = panic::catch_unwind(AssertUnwindSafe(|| hook(&fault))) {
            let message = describe(payload.as_ref());
            std::mem::forget(payload);
            self.log(&format!("{label}: fault hook failed: {message}\n"));
        }
    }
}

thread_local! {
    static LOCATION: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Formats a panic payload, appending the location recorded by
/// [`install_panic_hook`] on this thread.
fn describe(payload: &(dyn Any + Send)) -> String {
    let text = if let Some(text) = payload.downcast_ref::<&'static str>() {
        (*text).to_owned()
    } else if let Some(text) = payload.downcast_ref::<String>() {
        text.clone()
    } else {
        "non-string panic payload".to_owned()
    };
    let location = LOCATION
        .try_with(|slot| slot.try_borrow_mut().ok().and_then(|mut slot| slot.take()))
        .ok()
        .flatten();
    match location {
        Some(location) => format!("{text} at {location}"),
        None => text,
    }
}

/// Records each panic's source location for the boundary's diagnostic, then
/// runs the previously installed hook. Idempotent.
pub fn install_panic_hook() {
    static INSTALLED: AtomicBool = AtomicBool::new(false);
    if INSTALLED.swap(true, Ordering::SeqCst) {
        return;
    }
    let previous = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        if let Some(location) = info.location() {
            let text = format!("{}:{}", location.file(), location.line());
            let _ = LOCATION.try_with(|slot| {
                if let Ok(mut slot) = slot.try_borrow_mut() {
                    *slot = Some(text);
                }
            });
        }
        previous(info);
    }));
}

/// Writes a diagnostic line to X-Plane's `Log.txt`. [`crate::export_plugin`]
/// installs it on [`PLUGIN`] at startup. Keeping the SDK call out of the
/// default path lets unit tests link the guard without the X-Plane library.
pub fn xplane_log(text: &str) {
    let text = crate::c_string(text);
    // SAFETY: `text` is NUL-terminated and outlives the call.
    unsafe { xplane_sdk_sys::XPLMDebugString(text.as_ptr()) };
}

/// The boundary shared by this plugin's entry points and harness callbacks.
pub static PLUGIN: Boundary = Boundary::new();

/// Runs a callback body under [`PLUGIN`]; see [`Boundary::call`].
pub fn guard<R>(callback: &'static str, default: R, body: impl FnOnce() -> R) -> R {
    PLUGIN.call(callback, default, body)
}

/// Runs a teardown body under [`PLUGIN`]; see [`Boundary::call_teardown`].
pub fn guard_teardown<R>(callback: &'static str, default: R, body: impl FnOnce() -> R) -> R {
    PLUGIN.call_teardown(callback, default, body)
}

/// Installs the plugin's fault hook; see [`Boundary::set_fault_hook`].
pub fn set_fault_hook(hook: FaultHook) -> Option<FaultHook> {
    PLUGIN.set_fault_hook(Some(hook))
}

/// Whether the plugin has latched a fault.
pub fn faulted() -> bool {
    PLUGIN.faulted()
}

/// Reports a panic the caller caught; see [`Boundary::report_panic`].
pub fn report_panic(callback: &'static str, payload: Box<dyn Any + Send>) {
    PLUGIN.report_panic(callback, payload);
}

/// Latches a fault without a panic; see [`Boundary::report_fault`].
pub fn report_fault(callback: &'static str, message: &str) {
    PLUGIN.report_fault(callback, message);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Mutex as StdMutex,
        atomic::{AtomicUsize, Ordering},
    };

    // Each test owns a boundary, log and hook counter; statics are separate
    // per test because hooks are plain function pointers.
    macro_rules! fixture {
        ($boundary:ident, $log:ident, $hooks:ident, $last:ident) => {
            static $log: StdMutex<Vec<String>> = StdMutex::new(Vec::new());
            static $hooks: AtomicUsize = AtomicUsize::new(0);
            static $last: StdMutex<Option<(String, String)>> = StdMutex::new(None);
            fn record(text: &str) {
                $log.lock().unwrap().push(text.to_owned());
            }
            fn hook(fault: &Fault<'_>) {
                $hooks.fetch_add(1, Ordering::SeqCst);
                *$last.lock().unwrap() =
                    Some((fault.callback.to_owned(), fault.message.to_owned()));
            }
            let $boundary = Boundary::new();
            $boundary.set_logger(record);
            $boundary.set_label("test plugin");
            $boundary.set_fault_hook(Some(hook));
        };
    }

    #[test]
    fn values_pass_through_until_a_panic() {
        fixture!(boundary, LOG, HOOKS, LAST);
        assert_eq!(boundary.call("frame", 0, || 7), 7);
        assert!(!boundary.faulted());
        assert!(LOG.lock().unwrap().is_empty());
        assert_eq!(HOOKS.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn a_panic_returns_the_default_latches_and_runs_the_hook_once() {
        fixture!(boundary, LOG, HOOKS, LAST);
        let value = boundary.call("draw", -1.5f32, || -> f32 { panic!("broken invariant") });
        assert_eq!(value, -1.5);
        assert!(boundary.faulted());
        assert_eq!(HOOKS.load(Ordering::SeqCst), 1);
        let (callback, message) = LAST.lock().unwrap().clone().unwrap();
        assert_eq!(callback, "draw");
        assert!(message.starts_with("broken invariant"), "{message}");
        let log = LOG.lock().unwrap().clone();
        assert_eq!(log.len(), 1);
        assert!(log[0].starts_with("test plugin: fault in draw: broken invariant"));
        assert!(
            log[0].ends_with("callbacks are now disabled\n"),
            "{}",
            log[0]
        );

        // Later callbacks short-circuit; a teardown still runs and a second
        // panic is logged without running the hook again.
        let ran = AtomicBool::new(false);
        assert_eq!(
            boundary.call("frame", 3, || {
                ran.store(true, Ordering::SeqCst);
                4
            }),
            3
        );
        assert!(!ran.load(Ordering::SeqCst));
        assert_eq!(boundary.call_teardown("disable", 0, || 9), 9);
        let message = String::from("teardown failure");
        boundary.call_teardown("stop", (), || panic!("{message}"));
        assert_eq!(HOOKS.load(Ordering::SeqCst), 1);
        let log = LOG.lock().unwrap().clone();
        assert_eq!(log.len(), 2);
        assert_eq!(log[1], "test plugin: fault in stop: teardown failure\n");
    }

    #[test]
    fn a_panicking_hook_is_contained_and_not_rerun() {
        static LOG: StdMutex<Vec<String>> = StdMutex::new(Vec::new());
        static HOOKS: AtomicUsize = AtomicUsize::new(0);
        fn record(text: &str) {
            LOG.lock().unwrap().push(text.to_owned());
        }
        fn hook(_: &Fault<'_>) {
            HOOKS.fetch_add(1, Ordering::SeqCst);
            panic!("hook failure");
        }
        let boundary = Boundary::new();
        boundary.set_logger(record);
        boundary.set_fault_hook(Some(hook));
        boundary.call("command", (), || panic!("first"));
        boundary.call_teardown("disable", (), || panic!("second"));
        assert_eq!(HOOKS.load(Ordering::SeqCst), 1);
        let log = LOG.lock().unwrap().clone();
        assert_eq!(log.len(), 3, "{log:?}");
        assert!(log[0].starts_with("plugin: fault in command: first"));
        assert!(log[1].starts_with("plugin: fault hook failed: hook failure"));
        assert!(log[2].starts_with("plugin: fault in disable: second"));
    }

    #[test]
    fn caller_caught_panics_and_plain_faults_share_the_latch() {
        fixture!(boundary, LOG, HOOKS, LAST);
        let payload = panic::catch_unwind(|| panic!("local recovery")).unwrap_err();
        boundary.report_panic("avionics draw", payload);
        boundary.report_fault("cleanup", "registration still executing");
        assert!(boundary.faulted());
        assert_eq!(HOOKS.load(Ordering::SeqCst), 1);
        assert_eq!(LAST.lock().unwrap().clone().unwrap().0, "avionics draw");
        assert_eq!(LOG.lock().unwrap().len(), 2);
    }

    #[test]
    fn the_hook_sees_the_fault_latched_so_nested_callbacks_short_circuit() {
        static BOUNDARY: Boundary = Boundary::new();
        static NESTED: AtomicUsize = AtomicUsize::new(0);
        fn hook(_: &Fault<'_>) {
            // A hook that reenters the SDK can trigger another guarded
            // callback; it must not run plugin code.
            BOUNDARY.call("nested", (), || {
                NESTED.fetch_add(1, Ordering::SeqCst);
            });
        }
        BOUNDARY.set_fault_hook(Some(hook));
        BOUNDARY.call("outer", (), || panic!("outer"));
        assert!(BOUNDARY.faulted());
        assert_eq!(NESTED.load(Ordering::SeqCst), 0);
    }
}
