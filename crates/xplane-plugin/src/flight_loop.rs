use std::{
    cell::{Cell, RefCell},
    ffi::{CString, c_void},
    fmt, ptr,
    rc::{Rc, Weak},
    thread::{self, ThreadId},
};

use xplane_sdk_sys as xp;
use xplane_sdk_sys::{
    XPLMFlightLoop_f, XPLMRegisterFlightLoopCallback, XPLMUnregisterFlightLoopCallback,
};

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
        let handle = create_phase_loop(phase, callback, ptr::null_mut())?;
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
        let handle = create_phase_loop(
            xp::xplm_FlightLoop_Phase_BeforeFlightModel,
            callback,
            ptr::null_mut(),
        )?;
        Ok(Self(Cell::new(handle)))
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

fn create_phase_loop(
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
    // SAFETY: options live through creation. Callers retain any refcon for
    // the lifetime of the owned registration.
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
#[must_use = "dropping a managed flight loop destroys its registration"]
pub struct ManagedFlightLoop<H: FlightLoopHandler> {
    state: Rc<ManagedState<H>>,
}

struct ManagedState<H: FlightLoopHandler> {
    owner: ThreadId,
    handler: H,
    registration: PhaseFlightLoop,
    context: Weak<Self>,
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
            context: context.clone(),
            track_outer_callbacks,
            active: Cell::new(false),
            executing: Cell::new(false),
            failure: RefCell::new(None),
        });
        let refcon = ptr::from_ref(&state.context).cast_mut().cast();
        let handle = create_phase_loop(phase, Some(managed_callback::<H>), refcon)
            .map_err(|_| FlightLoopError::CreationFailed)?;
        state.registration.0.set(handle);
        Ok(Self { state })
    }

    pub(crate) fn handle(&self) -> xp::XPLMFlightLoopID {
        self.state.registration.0.get()
    }

    pub fn start(&self) -> Result<(), FlightLoopError> {
        self.state.check_thread()?;
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
        self.state.failure.borrow().is_some()
    }

    pub fn failure(&self) -> Option<FlightLoopFault<H::Error>> {
        self.state.failure.borrow().clone()
    }
}

impl<H: FlightLoopHandler> Drop for ManagedFlightLoop<H> {
    fn drop(&mut self) {
        if !self.handle().is_null() && self.close().is_err() {
            // The SDK callback may still be borrowing the context.
            std::process::abort();
        }
    }
}

impl<H: FlightLoopHandler> ManagedState<H> {
    fn check_thread(&self) -> Result<(), FlightLoopError> {
        if thread::current().id() != self.owner {
            return Err(FlightLoopError::WrongThread);
        }
        Ok(())
    }

    fn frame(&self, frame: FlightLoopFrame) -> f32 {
        if !self.active.get() {
            return 0.;
        }
        let already_executing = self.executing.replace(true);
        let result = self
            .check_thread()
            .map_err(FlightLoopFault::Control)
            .and_then(|()| self.handler.frame(frame).map_err(FlightLoopFault::Callback));
        let interval = match result {
            Ok(interval) => interval,
            Err(error) => {
                self.active.set(false);
                *self.failure.borrow_mut() = Some(error.clone());
                self.handler.fault(error);
                0.
            }
        };
        self.executing
            .set(self.track_outer_callbacks && already_executing);
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

unsafe extern "C" fn managed_callback<H: FlightLoopHandler>(
    elapsed_since_last_call: f32,
    elapsed_since_last_loop: f32,
    counter: i32,
    refcon: *mut c_void,
) -> f32 {
    // SAFETY: the refcon points to the weak reference stored in the registered
    // state. Upgrade it before dispatch so a reentrant close can retire the
    // registration without releasing the running handler.
    let context = unsafe { &*refcon.cast::<Weak<ManagedState<H>>>() };
    let Some(state) = context.upgrade() else {
        return 0.;
    };
    state.frame(FlightLoopFrame {
        elapsed_since_last_call,
        elapsed_since_last_loop,
        counter,
    })
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
    refs: [xp::XPLMDataRef; 6],
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
        let mut refs = [ptr::null_mut(); 6];
        for (slot, (name, kind)) in refs.iter_mut().zip(names) {
            let name_c = CString::new(name).unwrap();
            // SAFETY: the name is null terminated and lives through the call.
            let handle = unsafe { xp::XPLMFindDataRef(name_c.as_ptr()) };
            // SAFETY: the type check runs only on a valid dataref handle.
            if handle.is_null() || (unsafe { xp::XPLMGetDataRefTypes(handle) } & kind) == 0 {
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
        // SAFETY: all handles were checked for their read type at construction.
        unsafe {
            Ok(SimulationTimingSample {
                simulation_period: xp::XPLMGetDataf(r[0]),
                actual_speed: xp::XPLMGetDataf(r[1]),
                actual_speed_ogl: xp::XPLMGetDataf(r[2]),
                requested_speed: xp::XPLMGetDatai(r[3]),
                paused: xp::XPLMGetDatai(r[4]) != 0,
                replay: xp::XPLMGetDatai(r[5]) != 0,
                callback_wall_seconds,
                callback_counter,
            })
        }
    }
}
