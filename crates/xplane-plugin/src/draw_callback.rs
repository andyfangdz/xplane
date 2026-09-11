use std::{ffi::c_void, marker::PhantomData, rc::Rc};
use xplane_sdk_sys::{
    XPLMDrawCallback_f, XPLMDrawingPhase, XPLMRegisterDrawCallback, XPLMUnregisterDrawCallback,
};

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
    pub fn register(
        callback: XPLMDrawCallback_f,
        phase: XPLMDrawingPhase,
        before: bool,
        identifier: usize,
    ) -> Result<Self, String> {
        if callback.is_none() {
            return Err("draw callback cannot be null".to_owned());
        }
        let before = i32::from(before);
        // SAFETY: callback code has static lifetime; the opaque integer token is
        // not a borrowed pointer. The registration tuple is saved for Drop.
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
