use std::ffi::{c_char, CStr, CString};
use std::path::PathBuf;
use std::ptr;

use xplane_sdk_sys::{XPLMDebugString, XPLMGetSystemPath};

pub(super) fn c_string(value: &str) -> CString {
    CString::new(value.replace('\0', " ")).unwrap()
}

pub(in crate::runtime) fn log(message: &str) {
    let message = c_string(&format!("PositionAircraftNative: {message}\n"));
    // SAFETY: `message` is a live NUL-terminated string for the duration of the call.
    unsafe { XPLMDebugString(message.as_ptr()) }
}

pub(super) unsafe fn write_plugin_string(destination: *mut c_char, value: &str) {
    if destination.is_null() {
        return;
    }
    let bytes = value.as_bytes();
    // SAFETY: the caller guarantees an SDK-sized writable output buffer. All
    // values passed here are short plugin metadata constants.
    unsafe {
        ptr::copy_nonoverlapping(bytes.as_ptr(), destination.cast::<u8>(), bytes.len());
        *destination.add(bytes.len()) = 0;
    }
}

pub(super) fn system_path() -> PathBuf {
    let mut buffer = [0_i8; 1024];
    // SAFETY: the SDK accepts this fixed writable buffer and guarantees a
    // NUL-terminated path on return.
    let path = unsafe {
        XPLMGetSystemPath(buffer.as_mut_ptr());
        CStr::from_ptr(buffer.as_ptr())
            .to_string_lossy()
            .into_owned()
    };
    PathBuf::from(path)
}
