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
