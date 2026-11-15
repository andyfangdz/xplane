use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    ffi::{CString, c_void},
    fmt, ptr,
    rc::Rc,
    sync::{LazyLock, Mutex, MutexGuard, PoisonError},
    thread::{self, ThreadId},
};

use xplane_sdk_sys as xp;
use xplane_sdk_sys::{
    XPLMFlightLoop_f, XPLMRegisterFlightLoopCallback, XPLMUnregisterFlightLoopCallback,
};

use crate::callbacks::{Refcon, lease};
use crate::xplm::DataRef;

/// An active null-refcon XPLM flight-loop registration.
///
/// Dropping the value unregisters the exact callback that was registered.
#[must_use = "dropping a flight-loop registration immediately unregisters it"]
pub struct FlightLoop {
    callback: XPLMFlightLoop_f,
}

impl FlightLoop {
    pub fn register(callback: XPLMFlightLoop_f, interval: f32) -> Result<Self, String> {
        if callback.is_none() {
            return Err("flight-loop callback cannot be null".to_owned());
        }
        // SAFETY: the callback has the XPLM ABI, has static code lifetime, and
        // receives the same null refcon retained by this registration.
        unsafe { XPLMRegisterFlightLoopCallback(callback, interval, ptr::null_mut()) };
        Ok(Self { callback })
    }
}

impl Drop for FlightLoop {
    fn drop(&mut self) {
        // SAFETY: this exactly matches the callback/refcon pair registered by
        // `FlightLoop::register`, and the value can only be dropped once.
        unsafe { XPLMUnregisterFlightLoopCallback(self.callback, ptr::null_mut()) };
    }
}

/// Owns a modern flight loop with explicit before/after physics timing.
pub struct PhaseFlightLoop(Cell<xp::XPLMFlightLoopID>);
impl PhaseFlightLoop {
    pub fn after_physics(callback: XPLMFlightLoop_f) -> Result<Self, String> {
        Self::register(
            callback,
            xplane_sdk_sys::xplm_FlightLoop_Phase_AfterFlightModel,
        )
    }
    pub fn before_physics(callback: XPLMFlightLoop_f) -> Result<Self, String> {
        Self::register(
            callback,
            xplane_sdk_sys::xplm_FlightLoop_Phase_BeforeFlightModel,
        )
    }
    fn register(
        callback: XPLMFlightLoop_f,
        phase: xp::XPLMFlightLoopPhaseType,
    ) -> Result<Self, String> {
        // SAFETY: NOT enforced here. The public safe `after_physics` and
        // `before_physics` take any `XPLMFlightLoop_f` and rely on the caller's
        // callback accepting a null refcon on the plugin thread, as
        // `unscheduled_before_physics` states; their signatures are kept for
        // source compatibility.
        let handle = unsafe { create_phase_loop(phase, callback, ptr::null_mut()) }?;
        let registration = Self(Cell::new(handle));
        registration.schedule(-1., true);
        Ok(registration)
    }

    /// Creates a before-physics loop which first runs after `schedule` is called.
    ///
    /// # Safety
    /// The callback must accept the SDK's flight-loop arguments and a null
    /// refcon. Destroy the registration on the plugin thread and outside its
    /// callback.
    pub unsafe fn unscheduled_before_physics(callback: XPLMFlightLoop_f) -> Result<Self, String> {
        // SAFETY: the caller guarantees the callback accepts the null refcon.
        let handle = unsafe {
            create_phase_loop(
                xp::xplm_FlightLoop_Phase_BeforeFlightModel,
                callback,
                ptr::null_mut(),
            )
        }?;
        Ok(Self(Cell::new(handle)))
    }

    /// Creates a dormant loop for `phase` whose callback is a safe function
    /// (so it accepts any refcon), passing it `refcon`.
    pub(crate) fn unscheduled(
        phase: xp::XPLMFlightLoopPhaseType,
        callback: SafeFlightLoop,
        refcon: *mut c_void,
    ) -> Result<Self, String> {
        // SAFETY: a safe function has no precondition on its arguments, and
        // X-Plane only hands `refcon` back to it.
        let handle = unsafe { create_phase_loop(phase, Some(callback), refcon) }?;
        Ok(Self(Cell::new(handle)))
    }

    /// The SDK handle; null once closed.
    pub(crate) fn handle(&self) -> xp::XPLMFlightLoopID {
        self.0.get()
    }

    /// Schedules the loop with the SDK interval convention. A negative interval
    /// counts frames, a positive interval counts seconds, and zero suspends it.
    pub fn schedule(&self, interval: f32, relative_to_now: bool) {
        // SAFETY: the registration uniquely owns the handle. Closed handles are
        // checked by the internal callers before scheduling.
        unsafe {
            xp::XPLMScheduleFlightLoop(self.0.get(), interval, i32::from(relative_to_now));
        }
    }

    fn close(&self) {
        let handle = self.0.get();
        if !handle.is_null() {
            // SAFETY: the owner closes the live registration outside its callback.
            unsafe { xp::XPLMDestroyFlightLoop(handle) };
            self.0.set(ptr::null_mut());
        }
    }
}
impl Drop for PhaseFlightLoop {
    fn drop(&mut self) {
        self.close();
    }
}

/// A flight-loop callback that is a safe function, so X-Plane may call it
/// with any arguments.
pub(crate) type SafeFlightLoop = extern "C" fn(f32, f32, i32, *mut c_void) -> f32;

/// `XPLMCreateFlightLoop` for `phase` with `callback` and `refcon`.
///
/// # Safety
/// Until the loop is destroyed, X-Plane may call `callback` with `refcon` on
/// the plugin thread; every such call must be sound.
unsafe fn create_phase_loop(
    phase: xp::XPLMFlightLoopPhaseType,
    callback: XPLMFlightLoop_f,
    refcon: *mut c_void,
) -> Result<xp::XPLMFlightLoopID, String> {
    if callback.is_none() {
        return Err("flight-loop callback cannot be null".into());
    }
    let mut options = xp::XPLMCreateFlightLoop_t {
        structSize: std::mem::size_of::<xp::XPLMCreateFlightLoop_t>() as i32,
        phase,
        callbackFunc: callback,
        refcon,
    };
    // SAFETY: the options live through the call; the caller guarantees the
    // callback is sound with this refcon.
    let handle = unsafe { xp::XPLMCreateFlightLoop(&mut options) };
    if handle.is_null() {
        return Err("flight-loop creation failed".into());
    }
    Ok(handle)
}

/// Timing values supplied by the SDK for one invocation of a flight loop.
#[derive(Clone, Copy, Debug)]
pub struct FlightLoopFrame {
    pub elapsed_since_last_call: f32,
    pub elapsed_since_last_loop: f32,
    pub counter: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlightLoopError {
    CreationFailed,
    WrongThread,
    CannotStart,
    Executing,
}

impl fmt::Display for FlightLoopError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::CreationFailed => "XPLMCreateFlightLoop failed",
            Self::WrongThread => "flight loop requires owner thread",
            Self::CannotStart => "closed, executing, or faulted flight loop",
            Self::Executing => "cannot destroy executing flight loop",
        })
    }
}
impl std::error::Error for FlightLoopError {}

#[derive(Clone, Debug)]
pub enum FlightLoopFault<E> {
    Control(FlightLoopError),
    Callback(E),
}

/// Supplies one frame of work and receives the original failure, if any.
pub trait FlightLoopHandler: 'static {
    type Error: Clone;

    /// Returns the SDK interval for the next invocation. A stop requested from
    /// inside the callback takes precedence over the returned interval. A zero
    /// result leaves the loop dormant until the owner calls `start` again.
    fn frame(&self, frame: FlightLoopFrame) -> Result<f32, Self::Error>;

    fn fault(&self, error: FlightLoopFault<Self::Error>);
}

/// Owns a callback and its SDK registration on the plugin thread.
/// Construction leaves the loop dormant until `start`. If a handler needs to
/// retire its own loop, it can stop during a callback and close after it returns.
///
/// # Wrong-thread calls
///
/// The loop's state is not `Send` and its refcon registry is per thread, so a
/// callback X-Plane makes on a thread other than the owner's cannot reach the
/// state there: it runs no handler code and returns 0. It is recorded in a
/// process-wide table of managed loops instead, and the owner thread turns it
/// into the wrong-thread fault on its next contact with the loop (a callback,
/// `start`, `stop`, `failed` or `failure`): the loop goes inactive,
/// `failure()` is `Control(WrongThread)` and the handler's `fault` runs, on
/// the owner thread. Before this registry the fault was recorded and the
/// handler called on the wrong thread itself, during the callback.
#[must_use = "dropping a managed flight loop destroys its registration"]
pub struct ManagedFlightLoop<H: FlightLoopHandler> {
    state: Rc<ManagedState<H>>,
}

struct ManagedState<H: FlightLoopHandler> {
    owner: ThreadId,
    handler: H,
    registration: PhaseFlightLoop,
    /// The registry entry the loop's refcon names; it lives as long as the
    /// state, after the registration field has destroyed the loop.
    refcon: Refcon,
    track_outer_callbacks: bool,
    active: Cell<bool>,
    executing: Cell<bool>,
    failure: RefCell<Option<FlightLoopFault<H::Error>>>,
}

impl<H: FlightLoopHandler> ManagedFlightLoop<H> {
    pub fn before_physics(handler: H) -> Result<Self, FlightLoopError> {
        Self::new(xp::xplm_FlightLoop_Phase_BeforeFlightModel, handler, true)
    }

    pub fn after_physics(handler: H) -> Result<Self, FlightLoopError> {
        Self::new(xp::xplm_FlightLoop_Phase_AfterFlightModel, handler, true)
    }

    /// Creates a dormant loop whose innermost callback return clears the
    /// execution flag. An outer callback can then schedule or close the loop.
    /// Each callback retains its handler until it returns.
    ///
    /// # Safety
    /// If the SDK can recursively invoke the same loop, it must support
    /// scheduling and destroying that loop from the outer callback after
    /// the nested invocation returns.
    pub unsafe fn before_physics_with_nested_control(handler: H) -> Result<Self, FlightLoopError> {
        Self::new(xp::xplm_FlightLoop_Phase_BeforeFlightModel, handler, false)
    }

    fn new(
        phase: xp::XPLMFlightLoopPhaseType,
        handler: H,
        track_outer_callbacks: bool,
    ) -> Result<Self, FlightLoopError> {
        let state = Rc::new_cyclic(|context| ManagedState {
            owner: thread::current().id(),
            handler,
            registration: PhaseFlightLoop(Cell::new(ptr::null_mut())),
            refcon: Refcon::new(context),
            track_outer_callbacks,
            active: Cell::new(false),
            executing: Cell::new(false),
            failure: RefCell::new(None),
        });
        state.enter_managed();
        let registration =
            PhaseFlightLoop::unscheduled(phase, managed_callback::<H>, state.refcon.as_ptr())
                .map_err(|_| FlightLoopError::CreationFailed)?;
        // Move the handle into the state's own (empty) registration.
        state
            .registration
            .0
            .set(registration.0.replace(ptr::null_mut()));
        Ok(Self { state })
    }

    pub(crate) fn handle(&self) -> xp::XPLMFlightLoopID {
        self.state.registration.0.get()
    }

    pub fn start(&self) -> Result<(), FlightLoopError> {
        self.state.check_thread()?;
        self.state.absorb_foreign_calls();
        if self.handle().is_null() || self.executing() || self.failed() {
            return Err(FlightLoopError::CannotStart);
        }
        if !self.active() {
            self.state.registration.schedule(-1., true);
            self.state.active.set(true);
        }
        Ok(())
    }

    pub fn stop(&self) -> Result<(), FlightLoopError> {
        self.state.check_thread()?;
        self.state.absorb_foreign_calls();
        self.state.active.set(false);
        if !self.handle().is_null() && !self.executing() {
            self.state.registration.schedule(0., true);
        }
        Ok(())
    }

    pub fn close(&self) -> Result<(), FlightLoopError> {
        self.state.check_thread()?;
        if self.executing() {
            return Err(FlightLoopError::Executing);
        }
        self.state.active.set(false);
        self.state.registration.close();
        Ok(())
    }

    pub fn active(&self) -> bool {
        self.state.active.get()
    }

    pub fn executing(&self) -> bool {
        self.state.executing.get()
    }

    pub fn failed(&self) -> bool {
        self.state.absorb_foreign_calls();
        self.state.failure.borrow().is_some()
    }

    pub fn failure(&self) -> Option<FlightLoopFault<H::Error>> {
        self.state.absorb_foreign_calls();
        self.state.failure.borrow().clone()
    }
}

/// Every live managed loop's refcon address, with its owner thread and
/// whether a callback for it arrived on another thread since the owner last
/// looked (type note on [`ManagedFlightLoop`]). Plain data only: nothing of
/// a loop's non-`Send` state is reachable from it.
static MANAGED: LazyLock<Mutex<HashMap<usize, (ThreadId, bool)>>> = LazyLock::new(Mutex::default);

/// The table, locked for one short step. A poisoned lock still holds a
/// consistent map: every step is a single insert, update or remove.
fn managed() -> MutexGuard<'static, HashMap<usize, (ThreadId, bool)>> {
    MANAGED.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Records a callback that `lease` could not resolve: when `refcon` is a
/// live managed loop owned by another thread, marks it for its owner.
fn note_foreign_call(refcon: *mut c_void) {
    let current = thread::current().id();
    if let Some((owner, foreign)) = managed().get_mut(&refcon.addr())
        && *owner != current
    {
        *foreign = true;
    }
}

impl<H: FlightLoopHandler> Drop for ManagedFlightLoop<H> {
    fn drop(&mut self) {
        if self.handle().is_null() || self.close().is_ok() {
            return;
        }
        // The loop is being dropped from inside its own callback, which still
        // borrows the context. Deactivate it so it returns 0 and is not
        // rescheduled, then leak the state: the registration and its refcon
        // stay valid for the rest of the process.
        self.state.active.set(false);
        crate::guard::report_fault(
            "flight loop cleanup",
            "loop dropped while executing; registration leaked",
        );
        std::mem::forget(self.state.clone());
    }
}

impl<H: FlightLoopHandler> ManagedState<H> {
    fn check_thread(&self) -> Result<(), FlightLoopError> {
        if thread::current().id() != self.owner {
            return Err(FlightLoopError::WrongThread);
        }
        Ok(())
    }

    /// Enters this state's refcon in the process-wide table of managed loops.
    fn enter_managed(&self) {
        managed().insert(self.refcon.as_ptr().addr(), (self.owner, false));
    }

    /// Whether a callback for this loop arrived on another thread since the
    /// last look; clears the mark.
    fn take_foreign_call(&self) -> bool {
        managed()
            .get_mut(&self.refcon.as_ptr().addr())
            .is_some_and(|(_, foreign)| std::mem::take(foreign))
    }

    /// On the owner thread outside a callback: turns a pending wrong-thread
    /// callback into the wrong-thread fault (type note on
    /// [`ManagedFlightLoop`]).
    fn absorb_foreign_calls(&self) {
        if self.take_foreign_call() {
            self.record_fault(FlightLoopFault::Control(FlightLoopError::WrongThread));
        }
    }

    fn record_fault(&self, error: FlightLoopFault<H::Error>) {
        self.active.set(false);
        if let Ok(mut failure) = self.failure.try_borrow_mut() {
            *failure = Some(error.clone());
        }
        self.handler.fault(error);
    }

    fn frame(&self, frame: FlightLoopFrame) -> f32 {
        if !self.active.get() {
            return 0.;
        }
        let already_executing = self.executing.replace(true);
        // Restores the execution flag even if the handler unwinds to the
        // callback's panic boundary, so the owner can still close the loop.
        let executing = Executing {
            flag: &self.executing,
            restore: self.track_outer_callbacks && already_executing,
        };
        let result = self
            .check_thread()
            .and_then(|()| {
                if self.take_foreign_call() {
                    Err(FlightLoopError::WrongThread)
                } else {
                    Ok(())
                }
            })
            .map_err(FlightLoopFault::Control)
            .and_then(|()| self.handler.frame(frame).map_err(FlightLoopFault::Callback));
        let interval = match result {
            Ok(interval) => interval,
            Err(error) => {
                self.record_fault(error);
                0.
            }
        };
        drop(executing);
        if !self.active.get() {
            return 0.;
        }
        // Recognize either signed zero without a floating-point comparison
        // that can modify the caller's exception flags.
        if interval.to_bits() & 0x7fff_ffff == 0 {
            self.active.set(false);
        }
        interval
    }
}

impl<H: FlightLoopHandler> Drop for ManagedState<H> {
    fn drop(&mut self) {
        // Before the refcon field releases the address for reuse.
        managed().remove(&self.refcon.as_ptr().addr());
    }
}

struct Executing<'a> {
    flag: &'a Cell<bool>,
    restore: bool,
}

impl Drop for Executing<'_> {
    fn drop(&mut self) {
        self.flag.set(self.restore);
    }
}

extern "C" fn managed_callback<H: FlightLoopHandler>(
    elapsed_since_last_call: f32,
    elapsed_since_last_loop: f32,
    counter: i32,
    refcon: *mut c_void,
) -> f32 {
    // A strong reference for the call, so a reentrant close can retire the
    // registration without releasing the running handler. The state's
    // registry entry lives as long as the state (Drop leaks it if the loop
    // cannot be destroyed).
    let Some(state) = lease::<ManagedState<H>>(refcon) else {
        // On another thread the per-thread registry cannot resolve the refcon:
        // mark the loop for its owner (type note on `ManagedFlightLoop`).
        note_foreign_call(refcon);
        return 0.;
    };
    crate::guard::guard("flight loop", 0., || {
        state.frame(FlightLoopFrame {
            elapsed_since_last_call,
            elapsed_since_last_loop,
            counter,
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Recorder {
        frames: Cell<u32>,
        faults: RefCell<Vec<FlightLoopError>>,
    }

    impl FlightLoopHandler for Recorder {
        type Error = ();

        fn frame(&self, _: FlightLoopFrame) -> Result<f32, ()> {
            self.frames.set(self.frames.get() + 1);
            Ok(-1.)
        }

        fn fault(&self, error: FlightLoopFault<()>) {
            if let FlightLoopFault::Control(error) = error {
                self.faults.borrow_mut().push(error);
            }
        }
    }

    /// An active loop state with no SDK registration. It is never dropped
    /// (`ManuallyDrop`): its drop glue would reference `XPLMDestroyFlightLoop`,
    /// and a test binary that links an XPLM import fails to load without
    /// X-Plane's library. For the same reason the tests call no owner method
    /// that schedules.
    /// Not in an `Rc` either: `Rc::new_cyclic` references the drop glue too.
    /// The refcon names a leaked anchor allocation instead of the state, which
    /// the wrong-thread table does not care about: it is keyed by the refcon.
    fn active_state() -> std::mem::ManuallyDrop<ManagedState<Recorder>> {
        let anchor = Rc::new(());
        let refcon = Refcon::new(&Rc::downgrade(&anchor));
        // Keeps the refcon's address reserved for the rest of the test process.
        std::mem::forget(anchor);
        // The registration is built last, so no unwind can drop it either.
        let state = std::mem::ManuallyDrop::new(ManagedState {
            owner: thread::current().id(),
            handler: Recorder {
                frames: Cell::new(0),
                faults: RefCell::new(Vec::new()),
            },
            refcon,
            track_outer_callbacks: true,
            active: Cell::new(true),
            executing: Cell::new(false),
            failure: RefCell::new(None),
            registration: PhaseFlightLoop(Cell::new(ptr::null_mut())),
        });
        state.enter_managed();
        state
    }

    /// One owner-thread frame, as `managed_callback` runs it after its lease.
    /// (`managed_callback` itself is not called: dropping its lease references
    /// the state's drop glue.)
    fn frame(state: &ManagedState<Recorder>) -> f32 {
        state.frame(FlightLoopFrame {
            elapsed_since_last_call: 0.,
            elapsed_since_last_loop: 0.,
            counter: 1,
        })
    }

    /// What `managed_callback` does on a thread whose registry cannot resolve
    /// the refcon.
    fn call_elsewhere(refcon: usize) {
        std::thread::spawn(move || note_foreign_call(ptr::without_provenance_mut(refcon)))
            .join()
            .unwrap();
    }

    fn wrong_thread(state: &ManagedState<Recorder>) -> bool {
        matches!(
            *state.failure.borrow(),
            Some(FlightLoopFault::Control(FlightLoopError::WrongThread))
        )
    }

    #[test]
    fn a_callback_on_another_thread_faults_the_loop_on_its_owner() {
        let state = active_state();
        let refcon = state.refcon.as_ptr().addr();
        assert_eq!(frame(&state), -1.);
        call_elsewhere(refcon);
        assert!(state.failure.borrow().is_none());
        // The owner's next callback records the fault and runs no handler frame.
        assert_eq!(frame(&state), 0.);
        assert_eq!(state.handler.frames.get(), 1);
        assert_eq!(
            *state.handler.faults.borrow(),
            [FlightLoopError::WrongThread]
        );
        assert!(!state.active.get());
        assert!(wrong_thread(&state));
    }

    #[test]
    fn a_wrong_thread_call_is_seen_by_the_owner_outside_a_callback() {
        let state = active_state();
        let refcon = state.refcon.as_ptr().addr();
        call_elsewhere(refcon);
        // What `failed`, `failure`, `start` and `stop` do first.
        state.absorb_foreign_calls();
        assert!(wrong_thread(&state));
        assert!(!state.active.get());
        state.absorb_foreign_calls();
        assert_eq!(state.handler.faults.borrow().len(), 1);
    }

    #[test]
    fn unknown_refcons_mark_nothing() {
        let state = active_state();
        let refcon = state.refcon.as_ptr().addr();
        // An address that is no managed loop, from another thread: nothing to mark.
        call_elsewhere(0x10);
        assert!(!managed().contains_key(&0x10));
        assert!(!state.take_foreign_call());
        // The owner thread's own call is never marked.
        note_foreign_call(ptr::without_provenance_mut(refcon));
        assert!(!state.take_foreign_call());
        assert_eq!(frame(&state), -1.);
    }
}

/// Standard simulation timing observed during a flight-loop callback.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct SimulationTimingSample {
    pub simulation_period: f32,
    pub actual_speed: f32,
    pub actual_speed_ogl: f32,
    pub requested_speed: i32,
    pub paused: bool,
    pub replay: bool,
    pub callback_wall_seconds: f32,
    pub callback_counter: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SimulationTimingError {
    WrongThread,
    MissingOrWrongType(&'static str),
}

impl fmt::Display for SimulationTimingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongThread => f.write_str("timing sample requires owner thread"),
            Self::MissingOrWrongType(name) => {
                write!(f, "missing or wrong type timing dataref: {name}")
            }
        }
    }
}
impl std::error::Error for SimulationTimingError {}

/// Resolves the SDK datarefs once and reads each sample on the plugin thread.
pub struct SimulationTimingSampler {
    owner: ThreadId,
    refs: [DataRef; 6],
}

impl SimulationTimingSampler {
    pub fn new() -> Result<Self, SimulationTimingError> {
        let names = [
            ("sim/operation/misc/frame_rate_period", xp::xplmType_Float),
            ("sim/time/sim_speed_actual", xp::xplmType_Float),
            ("sim/time/sim_speed_actual_ogl", xp::xplmType_Float),
            ("sim/time/sim_speed", xp::xplmType_Int),
            ("sim/time/paused", xp::xplmType_Int),
            ("sim/time/is_in_replay", xp::xplmType_Int),
        ];
        let mut refs = [DataRef::NULL; 6];
        for (slot, (name, kind)) in refs.iter_mut().zip(names) {
            let name_c = CString::new(name).unwrap();
            let handle = DataRef::find(&name_c);
            // The type check runs only on a found dataref.
            if handle.is_null() || (handle.types() & kind) == 0 {
                return Err(SimulationTimingError::MissingOrWrongType(name));
            }
            *slot = handle;
        }
        Ok(Self {
            owner: thread::current().id(),
            refs,
        })
    }

    pub fn sample(
        &self,
        callback_wall_seconds: f32,
        callback_counter: i32,
    ) -> Result<SimulationTimingSample, SimulationTimingError> {
        if self.owner != thread::current().id() {
            return Err(SimulationTimingError::WrongThread);
        }
        let r = &self.refs;
        Ok(SimulationTimingSample {
            simulation_period: r[0].get_f32(),
            actual_speed: r[1].get_f32(),
            actual_speed_ogl: r[2].get_f32(),
            requested_speed: r[3].get_i32(),
            paused: r[4].get_i32() != 0,
            replay: r[5].get_i32() != 0,
            callback_wall_seconds,
            callback_counter,
        })
    }
}
