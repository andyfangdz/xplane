use std::ffi::c_void;

use xplane_sdk_sys::{
    XPLMCommandCallback_f, XPLMRegisterCommandHandler, XPLMUnregisterCommandHandler,
};

use crate::c_string;
use crate::registrations::CommandHandle;

/// A command handle paired with this plugin's active callback registration.
///
/// X-Plane owns the command handle; dropping this value unregisters only the
/// callback owned by the plugin.
#[must_use = "dropping a command immediately unregisters its handler"]
pub struct Command {
    handle: CommandHandle,
    callback: XPLMCommandCallback_f,
    before: i32,
    identifier: usize,
}

impl Command {
    /// Intercepts an existing command without creating or owning its handle.
    pub fn intercept(
        name: &str,
        callback: XPLMCommandCallback_f,
        before: bool,
        identifier: usize,
    ) -> Result<Self, String> {
        if callback.is_none() {
            return Err("command callback cannot be null".to_owned());
        }
        let name = c_string(name);
        let handle =
            CommandHandle::find(&name).ok_or_else(|| format!("missing command {name:?}"))?;
        Ok(Self::register(handle, callback, before, identifier))
    }

    /// `XPLMRegisterCommandHandler(handle, callback, before, identifier)`,
    /// owned by the returned value, which unregisters the same tuple on drop.
    fn register(
        handle: CommandHandle,
        callback: XPLMCommandCallback_f,
        before: bool,
        identifier: usize,
    ) -> Self {
        let before = i32::from(before);
        // SAFETY: the command is live (a `CommandHandle`); the callback has static code lifetime
        // and the required ABI (residue, as before: these safe constructors take the raw callback
        // their callers wrote for this command). The integer refcon is an opaque token, never
        // dereferenced, retained for exact cleanup.
        unsafe {
            XPLMRegisterCommandHandler(handle.as_raw(), callback, before, identifier as *mut c_void)
        };
        Self {
            handle,
            callback,
            before,
            identifier,
        }
    }

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
        let handle = CommandHandle::create(&name, &description)
            .ok_or_else(|| format!("unable to create command {name:?}"))?;
        Ok(Self::register(handle, callback, before, identifier))
    }

    pub(crate) fn command_handle(&self) -> CommandHandle {
        self.handle
    }

    /// Decodes an integer token previously registered through `Command`.
    pub fn identifier_from_refcon(refcon: *mut c_void) -> usize {
        refcon as usize
    }
}

/// Executes an existing X-Plane command on the plugin thread.
pub fn command_once(name: &str) -> bool {
    let name = c_string(name);
    let Some(command) = CommandHandle::find(&name) else {
        return false;
    };
    // XPLM owns the handle. Commands may call back synchronously, so clients
    // should use the reentrancy-safe PluginStateSlot for their state.
    crate::xplm::once_handle(command);
    true
}

impl Drop for Command {
    fn drop(&mut self) {
        // SAFETY: this tuple exactly matches the one registered in `create`.
        unsafe {
            XPLMUnregisterCommandHandler(
                self.handle.as_raw(),
                self.callback,
                self.before,
                self.identifier as *mut c_void,
            );
        }
    }
}
