#![allow(non_snake_case)]
#![deny(unsafe_op_in_unsafe_fn)]

mod runtime;

use std::ffi::{c_char, c_int, c_void};

use xplane_sdk_sys::XPLMPluginID;

#[no_mangle]
/// X-Plane plugin startup entry point.
///
/// # Safety
/// X-Plane must provide writable SDK-sized metadata buffers.
pub unsafe extern "C" fn XPluginStart(
    out_name: *mut c_char,
    out_signature: *mut c_char,
    out_description: *mut c_char,
) -> c_int {
    // SAFETY: the caller supplies the writable plugin metadata buffers.
    unsafe { runtime::start(out_name, out_signature, out_description) }
}

#[no_mangle]
/// X-Plane plugin shutdown entry point.
///
/// # Safety
/// This must be called by X-Plane's plugin manager.
pub unsafe extern "C" fn XPluginStop() {
    runtime::stop();
}

#[no_mangle]
/// X-Plane plugin enable entry point.
///
/// # Safety
/// This must be called by X-Plane's plugin manager.
pub unsafe extern "C" fn XPluginEnable() -> c_int {
    i32::from(runtime::enable())
}

#[no_mangle]
/// X-Plane plugin disable entry point.
///
/// # Safety
/// This must be called by X-Plane's plugin manager.
pub unsafe extern "C" fn XPluginDisable() {
    runtime::disable();
}

#[no_mangle]
/// Receives X-Plane broadcast messages.
///
/// # Safety
/// Message values and callback threading must follow the XPLM ABI.
pub unsafe extern "C" fn XPluginReceiveMessage(
    from: XPLMPluginID,
    message: c_int,
    parameter: *mut c_void,
) {
    runtime::receive_message(from, message, parameter);
}
