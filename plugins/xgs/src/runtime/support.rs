use std::ffi::{c_char, CStr, CString};
use std::path::PathBuf;
use std::ptr;

use xplane_sdk_sys::{
    XPLMDebugString, XPLMGetMyID, XPLMGetPluginInfo, XPLMGetPrefsPath, XPLMGetSystemPath,
};

pub(super) fn c_string(value: &str) -> CString {
    CString::new(value.replace('\0', " ")).expect("sanitized string contains no NUL")
}

pub(in crate::runtime) fn log(message: &str) {
    let message = c_string(&format!("xgs-rs: {message}\n"));
    // SAFETY: `message` is a live NUL-terminated string for this call.
    unsafe { XPLMDebugString(message.as_ptr()) }
}

pub(super) unsafe fn write_plugin_string(destination: *mut c_char, value: &str) {
    if destination.is_null() {
        return;
    }
    let bytes = value.as_bytes();
    // SAFETY: the plugin ABI guarantees a writable SDK metadata buffer.
    unsafe {
        ptr::copy_nonoverlapping(bytes.as_ptr(), destination.cast::<u8>(), bytes.len());
        *destination.add(bytes.len()) = 0;
    }
}

fn path_from_xplm(fill: impl FnOnce(*mut c_char)) -> PathBuf {
    let mut buffer = [0_i8; 2048];
    fill(buffer.as_mut_ptr());
    // SAFETY: XPLM path APIs return a NUL-terminated string in the supplied buffer.
    let value = unsafe { CStr::from_ptr(buffer.as_ptr()) }
        .to_string_lossy()
        .into_owned();
    PathBuf::from(value)
}

pub(super) fn system_path() -> PathBuf {
    path_from_xplm(|buffer| {
        // SAFETY: `buffer` points to a writable 2048-byte array.
        unsafe { XPLMGetSystemPath(buffer) }
    })
}

pub(super) fn preferences_directory() -> PathBuf {
    let path = path_from_xplm(|buffer| {
        // SAFETY: `buffer` points to a writable 2048-byte array.
        unsafe { XPLMGetPrefsPath(buffer) }
    });
    path.parent().map(PathBuf::from).unwrap_or(path)
}

pub(super) fn plugin_directory() -> PathBuf {
    let mut name = [0_i8; 512];
    let mut path = [0_i8; 2048];
    let mut signature = [0_i8; 512];
    let mut description = [0_i8; 512];
    // SAFETY: all output buffers are live and sufficiently large for XPLM plugin metadata.
    unsafe {
        XPLMGetPluginInfo(
            XPLMGetMyID(),
            name.as_mut_ptr(),
            path.as_mut_ptr(),
            signature.as_mut_ptr(),
            description.as_mut_ptr(),
        );
    }
    // SAFETY: XPLM writes a NUL-terminated path into `path`.
    let binary = PathBuf::from(
        unsafe { CStr::from_ptr(path.as_ptr()) }
            .to_string_lossy()
            .as_ref(),
    );
    binary
        .parent()
        .and_then(|directory| directory.parent())
        .map(PathBuf::from)
        .unwrap_or_else(|| system_path().join("Resources/plugins/XgsRust"))
}

pub(super) fn angular_delta(from: f64, to: f64) -> f64 {
    (to - from + 180.0).rem_euclid(360.0) - 180.0
}
