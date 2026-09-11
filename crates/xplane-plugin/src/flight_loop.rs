use std::ptr;

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

/// A scheduled modern flight loop with explicit before/after physics timing.
pub struct PhaseFlightLoop(xplane_sdk_sys::XPLMFlightLoopID);
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
        phase: xplane_sdk_sys::XPLMFlightLoopPhaseType,
    ) -> Result<Self, String> {
        if callback.is_none() {
            return Err("flight-loop callback cannot be null".into());
        }
        let mut options = xplane_sdk_sys::XPLMCreateFlightLoop_t {
            structSize: std::mem::size_of::<xplane_sdk_sys::XPLMCreateFlightLoop_t>() as i32,
            phase,
            callbackFunc: callback,
            refcon: ptr::null_mut(),
        };
        // SAFETY: options live through registration; callback has static ABI
        // lifetime and no refcon. The owned registration is destroyed on drop.
        let handle = unsafe { xplane_sdk_sys::XPLMCreateFlightLoop(&mut options) };
        if handle.is_null() {
            return Err("flight-loop creation failed".into());
        }
        // SAFETY: handle was just created. -1 requests every simulator frame.
        unsafe { xplane_sdk_sys::XPLMScheduleFlightLoop(handle, -1.0, 1) };
        Ok(Self(handle))
    }
}
impl Drop for PhaseFlightLoop {
    fn drop(&mut self) {
        // SAFETY: this value uniquely owns the live registration.
        unsafe { xplane_sdk_sys::XPLMDestroyFlightLoop(self.0) };
    }
}
