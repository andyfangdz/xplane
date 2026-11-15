use std::ffi::{c_int, c_void};

use xplane_sdk_sys::{
    XPLMCommandCallback_f, XPLMCommandPhase, XPLMCommandRef, XPLMRegisterCommandHandler,
    XPLMUnregisterCommandHandler,
};

use crate::c_string;
use crate::registrations::CommandHandle;

/// A command handler (`XPLMCommandCallback_f`) that is a safe function:
/// `(command, phase, refcon) -> 1 to pass the command on, 0 to consume it`.
/// Being safe, it must be sound for any argument values; X-Plane calls it on
/// the plugin thread.
pub type CommandCallbackFn = extern "C" fn(XPLMCommandRef, XPLMCommandPhase, *mut c_void) -> c_int;

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
    /// Intercepts an existing command without creating or owning its handle:
    /// `XPLMRegisterCommandHandler(command, callback, before, identifier)`.
    /// X-Plane passes the callback `identifier` as its refcon. `None` is an
    /// error, with no call.
    pub fn intercept(
        name: &str,
        callback: Option<CommandCallbackFn>,
        before: bool,
        identifier: usize,
    ) -> Result<Self, String> {
        let callback: XPLMCommandCallback_f = callback.map(|callback| callback as _);
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
        // SAFETY: the command is live (a `CommandHandle`); the callback is a safe function (the
        // public constructors take only `CommandCallbackFn`), sound for any arguments, with
        // static code lifetime. The integer refcon is an opaque token, never dereferenced here,
        // retained for exact cleanup.
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

    /// Creates (or finds) a command and registers `callback` for it, as
    /// [`Command::intercept`]: X-Plane passes the callback `identifier` as its
    /// refcon. `None` is an error, with no call.
    pub fn create(
        name: &str,
        description: &str,
        callback: Option<CommandCallbackFn>,
        before: bool,
        identifier: usize,
    ) -> Result<Self, String> {
        let callback: XPLMCommandCallback_f = callback.map(|callback| callback as _);
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
