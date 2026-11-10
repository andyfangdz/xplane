//! Synthetic provider for exercising the shared aircraft harness through xphost.
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    ffi::c_void,
    ptr,
    rc::Rc,
};
use xplane_plugin::{
    Command, DebugLogger, FlightLoopFault, FlightLoopFrame, FlightLoopHandler, ManagedFlightLoop,
    OwnedDataRef, PhaseFlightLoop, PluginMetadata,
    lifecycle::{Host, Lifecycle},
};
use xplane_sdk_sys::{XPLMCommandPhase, XPLMCommandRef, xplm_CommandBegin};

const LOGGER: DebugLogger = DebugLogger::new("aircraft-provider fixture");
const RETIRE_MESSAGE: i32 = 0x4669_7874;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Instance(u32);

thread_local! {
    static LIFECYCLE: Lifecycle<Instance> = Lifecycle::default();
    static NEXT_INSTANCE: Cell<u32> = const { Cell::new(0) };
    static INSTANCES: RefCell<BTreeMap<Instance, Rc<ManagedFlightLoop<Frame>>>> = const { RefCell::new(BTreeMap::new()) };
    static DIAGNOSTICS: RefCell<Option<Diagnostics>> = const { RefCell::new(None) };
    static RETIRE_REQUESTED: Cell<bool> = const { Cell::new(false) };
    static CLEANUP: RefCell<Option<PhaseFlightLoop>> = const { RefCell::new(None) };
}

struct Diagnostics {
    frames: OwnedDataRef,
    messages: OwnedDataRef,
    created: OwnedDataRef,
    suspended: OwnedDataRef,
    destroyed: OwnedDataRef,
    reenabled: OwnedDataRef,
    failures: OwnedDataRef,
    _commands: [Command; 2],
}

fn diagnostics<T>(f: impl FnOnce(&Diagnostics) -> T) -> Option<T> {
    DIAGNOSTICS.with(|diagnostics| diagnostics.borrow().as_ref().map(f))
}

fn increment(reference: &OwnedDataRef) {
    reference.set_i32(reference.get_i32() + 1);
}

fn report_failure(error: &str) {
    diagnostics(|d| increment(&d.failures));
    LOGGER.log(&format!("failure: {error}"));
}

fn instance(id: Instance) -> Result<Rc<ManagedFlightLoop<Frame>>, String> {
    INSTANCES.with(|instances| {
        instances
            .borrow()
            .get(&id)
            .cloned()
            .ok_or_else(|| format!("unknown instance {}", id.0))
    })
}

struct Frame(Instance);

impl FlightLoopHandler for Frame {
    type Error = String;

    fn frame(&self, _: FlightLoopFrame) -> Result<f32, Self::Error> {
        LIFECYCLE.with(|lifecycle| {
            if lifecycle.current() != Some(self.0) {
                return Err("frame ran outside its active instance".into());
            }
            diagnostics(|d| increment(&d.frames));
            if RETIRE_REQUESTED.replace(false) {
                // Reenter disable through the message dispatch while this SDK
                // callback is running. The frame's registration must survive
                // until the cleanup loop drains the pending instance.
                lifecycle.receive(&Provider, 0, RETIRE_MESSAGE, ptr::null_mut());
                if lifecycle.current().is_some()
                    || !lifecycle.has_pending()
                    || !instance(self.0)?.executing()
                {
                    return Err("retirement escaped the executing frame".into());
                }
            }
            Ok(-1.0)
        })
    }

    fn fault(&self, error: FlightLoopFault<Self::Error>) {
        report_failure(&format!("frame fault: {error:?}"));
    }
}

struct Provider;

impl Host for Provider {
    type Instance = Instance;
    type Error = String;

    fn start(&self) -> Result<bool, String> {
        let counter =
            |name| OwnedDataRef::integer(&format!("fixture/aircraft/{name}"), 0, false, None);
        DIAGNOSTICS.set(Some(Diagnostics {
            frames: counter("frames")?,
            messages: counter("messages")?,
            created: counter("created")?,
            suspended: counter("suspended")?,
            destroyed: counter("destroyed")?,
            reenabled: counter("reenabled")?,
            failures: counter("failures")?,
            _commands: [
                Command::create(
                    "fixture/aircraft/retire",
                    "Retire on the next frame",
                    Some(command),
                    true,
                    0,
                )?,
                Command::create(
                    "fixture/aircraft/resume",
                    "Create another instance",
                    Some(command),
                    true,
                    1,
                )?,
            ],
        }));
        Ok(true)
    }

    fn create(&self) -> Result<Instance, String> {
        let id = NEXT_INSTANCE.with(|next| {
            let id = next.get() + 1;
            next.set(id);
            Instance(id)
        });
        let flight_loop =
            ManagedFlightLoop::before_physics(Frame(id)).map_err(|e| e.to_string())?;
        flight_loop.start().map_err(|e| e.to_string())?;
        INSTANCES.with(|instances| instances.borrow_mut().insert(id, Rc::new(flight_loop)));
        diagnostics(|d| increment(&d.created));
        Ok(id)
    }

    fn destroy(&self, id: Instance) {
        let flight_loop = INSTANCES.with(|instances| instances.borrow_mut().remove(&id));
        let Some(flight_loop) = flight_loop else {
            report_failure("destroy received an unknown instance");
            return;
        };
        if let Err(error) = flight_loop.close() {
            report_failure(&error.to_string());
            return;
        }
        diagnostics(|d| increment(&d.destroyed));
        LOGGER.log(&format!("destroyed instance {}", id.0));
    }

    fn reenabled(&self, id: Instance) -> Result<(), String> {
        if !instance(id)?.active() {
            return Err("reenable received an inactive instance".into());
        }
        diagnostics(|d| increment(&d.reenabled));
        Ok(())
    }

    fn stop(&self) -> Result<(), String> {
        let clean = INSTANCES.with(|instances| instances.borrow().is_empty())
            && diagnostics(|d| {
                d.created.get_i32() == d.destroyed.get_i32() && d.failures.get_i32() == 0
            })
            .unwrap_or(false);
        CLEANUP.set(None);
        DIAGNOSTICS.set(None);
        if clean {
            LOGGER.log("stopped cleanly");
            Ok(())
        } else {
            Err("stop found a resource leak or callback failure".into())
        }
    }

    fn message(&self, _: Instance, _: i32, message: i32, _: *mut c_void) -> Result<(), String> {
        if message == RETIRE_MESSAGE {
            disable();
        } else {
            diagnostics(|d| increment(&d.messages));
        }
        Ok(())
    }

    fn busy(&self, id: Instance) -> bool {
        instance(id).is_ok_and(|flight_loop| flight_loop.executing())
    }

    fn suspend(&self, id: Instance) {
        match instance(id).and_then(|flight_loop| flight_loop.stop().map_err(|e| e.to_string())) {
            Ok(()) => {
                diagnostics(|d| increment(&d.suspended));
            }
            Err(error) => report_failure(&error),
        }
    }

    fn defer(&self) {
        let result = CLEANUP.with(|cleanup_loop| {
            let mut slot = cleanup_loop.borrow_mut();
            if slot.is_none() {
                // SAFETY: cleanup has static code lifetime, uses only thread-local
                // state, and accepts the null refcon used by PhaseFlightLoop.
                *slot =
                    Some(unsafe { PhaseFlightLoop::unscheduled_before_physics(Some(cleanup)) }?);
            }
            if let Some(loop_) = slot.as_ref() {
                loop_.schedule(-1.0, true);
            }
            Ok::<(), String>(())
        });
        if let Err(error) = result {
            report_failure(&error);
        }
    }

    fn report(&self, operation: &str, error: &String) {
        report_failure(&format!("{operation}: {error}"));
    }
}

unsafe extern "C" fn cleanup(_: f32, _: f32, _: i32, _: *mut c_void) -> f32 {
    LIFECYCLE.with(|lifecycle| {
        lifecycle.drain(&Provider);
        if lifecycle.has_pending() { -1.0 } else { 0.0 }
    })
}

unsafe extern "C" fn command(
    _: XPLMCommandRef,
    phase: XPLMCommandPhase,
    refcon: *mut c_void,
) -> i32 {
    if phase == xplm_CommandBegin {
        match Command::identifier_from_refcon(refcon) {
            0 => RETIRE_REQUESTED.set(true),
            1 => {
                if !enable() {
                    report_failure("could not resume the fixture");
                }
            }
            _ => report_failure("unknown fixture command"),
        }
    }
    1
}

fn start() -> bool {
    LIFECYCLE.with(|lifecycle| lifecycle.start(&Provider))
}

fn enable() -> bool {
    LIFECYCLE.with(|lifecycle| lifecycle.enable(&Provider))
}

fn disable() {
    LIFECYCLE.with(|lifecycle| lifecycle.disable(&Provider));
}

fn stop() {
    LIFECYCLE.with(|lifecycle| lifecycle.stop(&Provider));
}

fn receive_message(from: i32, message: i32, parameter: *mut c_void) {
    LIFECYCLE.with(|lifecycle| lifecycle.receive(&Provider, from, message, parameter));
}

xplane_plugin::export_plugin! {
    metadata: PluginMetadata {
        name: "Synthetic Aircraft Provider",
        signature: "org.xphost.fixture.aircraft-provider",
        description: "Exercises the public aircraft lifecycle and flight-loop ownership.",
    },
    start: start,
    stop: stop,
    enable: enable,
    disable: disable,
    receive_message: receive_message,
}
