use std::ffi::c_void;
use std::ptr::NonNull;

use xplane_sdk_sys::{
    XPLMCommandCallback_f, XPLMCommandRef, XPLMCreateCommand, XPLMRegisterCommandHandler,
    XPLMUnregisterCommandHandler,
};

use crate::c_string;

/// A command handle paired with this plugin's active callback registration.
///
/// X-Plane owns the command handle; dropping this value unregisters only the
/// callback owned by the plugin.
#[must_use = "dropping a command immediately unregisters its handler"]
pub struct Command {
    handle: NonNull<c_void>,
    callback: XPLMCommandCallback_f,
    before: i32,
    identifier: usize,
}

impl Command {
    pub fn create(
        name: &str,
        description: &str,
        callback: XPLMCommandCallback_f,
        before: bool,
        identifier: usize,
    ) -> Result<Self, String> {
        if callback.is_none() {
            return Err("command callback cannot be null".to_owned());
        }
        let name = c_string(name);
        let description = c_string(description);
        // SAFETY: both strings are NUL-terminated and remain live for the call.
        let handle =
            NonNull::new(unsafe { XPLMCreateCommand(name.as_ptr(), description.as_ptr()) })
                .ok_or_else(|| format!("unable to create command {name:?}"))?;
        let before = i32::from(before);
        // SAFETY: the command is live, the callback has the required ABI, and
        // the integer token is only round-tripped as an opaque pointer.
        unsafe {
            XPLMRegisterCommandHandler(
                handle.as_ptr(),
                callback,
                before,
                identifier as *mut c_void,
            );
        }
        Ok(Self {
            handle,
            callback,
            before,
            identifier,
        })
    }

    pub(crate) fn handle(&self) -> XPLMCommandRef {
        self.handle.as_ptr()
    }

    /// Decodes an integer token previously registered through `Command`.
    pub fn identifier_from_refcon(refcon: *mut c_void) -> usize {
        refcon as usize
    }
}

impl Drop for Command {
    fn drop(&mut self) {
        // SAFETY: this tuple exactly matches the one registered in `create`.
        unsafe {
            XPLMUnregisterCommandHandler(
                self.handle.as_ptr(),
                self.callback,
                self.before,
                self.identifier as *mut c_void,
            );
        }
    }
}
