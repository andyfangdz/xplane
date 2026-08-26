#![allow(non_snake_case)]

mod pad;
mod runtime;
mod xplm;

use std::ffi::{c_char, c_int, c_void};

use xplm::XPLMPluginID;

#[no_mangle]
/// X-Plane plugin entry point.
///
/// # Safety
/// X-Plane must pass writable SDK-sized output buffers and load the plugin on
/// its normal plugin-management thread.
pub unsafe extern "C" fn XPluginStart(
    out_name: *mut c_char,
    out_signature: *mut c_char,
    out_description: *mut c_char,
) -> c_int {
    runtime::start(out_name, out_signature, out_description)
}

#[no_mangle]
/// X-Plane plugin shutdown entry point.
///
/// # Safety
/// X-Plane must invoke this only after a successful `XPluginStart` and after
/// it has stopped dispatching callbacks to this plugin.
pub unsafe extern "C" fn XPluginStop() {
    runtime::stop();
}

#[no_mangle]
/// X-Plane plugin enable entry point.
///
/// # Safety
/// This must be called by X-Plane's plugin manager.
pub unsafe extern "C" fn XPluginEnable() -> c_int {
    1
}

#[no_mangle]
/// X-Plane plugin disable entry point.
///
/// # Safety
/// This must be called by X-Plane's plugin manager.
pub unsafe extern "C" fn XPluginDisable() {}

#[no_mangle]
/// Receives X-Plane broadcast messages.
///
/// # Safety
/// The message and sender values must follow the XPLM ABI and the callback
/// must be made on X-Plane's plugin thread.
pub unsafe extern "C" fn XPluginReceiveMessage(
    from: XPLMPluginID,
    message: c_int,
    _parameter: *mut c_void,
) {
    runtime::receive_message(from, message);
}
