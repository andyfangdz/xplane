use std::{
    ffi::{c_int, c_void},
    marker::PhantomData,
    rc::Rc,
};
use xplane_sdk_sys::{
    XPLMDrawCallback_f, XPLMDrawingPhase, XPLMRegisterDrawCallback, XPLMUnregisterDrawCallback,
};

/// A draw callback (`XPLMDrawCallback_f`) that is a safe function:
/// `(phase, is_before, refcon) -> 1 to let X-Plane draw, 0 to suppress it`
/// (the result matters only before a phase). Being safe, it must be sound for
/// any argument values; X-Plane calls it on the plugin thread.
pub type DrawCallbackFn = extern "C" fn(XPLMDrawingPhase, c_int, *mut c_void) -> c_int;

/// Owns one exact phase/callback/refcon registration on X-Plane's plugin thread.
#[must_use = "dropping a draw callback unregisters it"]
pub struct DrawCallback {
    callback: XPLMDrawCallback_f,
    phase: XPLMDrawingPhase,
    before: i32,
    identifier: usize,
    _thread: PhantomData<Rc<()>>,
}
impl DrawCallback {
    /// `XPLMRegisterDrawCallback(callback, phase, before, identifier)`. X-Plane
    /// passes the callback `identifier` as its refcon. `None` is an error,
    /// with no call.
    pub fn register(
        callback: Option<DrawCallbackFn>,
        phase: XPLMDrawingPhase,
        before: bool,
        identifier: usize,
    ) -> Result<Self, String> {
        let callback: XPLMDrawCallback_f = callback.map(|callback| callback as _);
        if callback.is_none() {
            return Err("draw callback cannot be null".to_owned());
        }
        let before = i32::from(before);
        // SAFETY: the callback is a safe function, sound for any arguments, with static code
        // lifetime; the opaque integer token is not a borrowed pointer. The registration tuple
        // is saved for Drop.
        if unsafe { XPLMRegisterDrawCallback(callback, phase, before, identifier as *mut c_void) }
            == 0
        {
            return Err(format!("draw phase {phase} registration failed"));
        }
        Ok(Self {
            callback,
            phase,
            before,
            identifier,
            _thread: PhantomData,
        })
    }
}
impl Drop for DrawCallback {
    fn drop(&mut self) {
        // SAFETY: the exact registration is still owned by this non-Send value.
        unsafe {
            XPLMUnregisterDrawCallback(
                self.callback,
                self.phase,
                self.before,
                self.identifier as *mut c_void,
            )
        };
    }
}
