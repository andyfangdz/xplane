use std::ffi::{c_char, CStr};
use std::path::PathBuf;

use xplane_sdk_sys::{
    XPLMGetMyID, XPLMGetNthAircraftModel, XPLMGetPluginInfo, XPLMGetPrefsPath, XPLMGetSystemPath,
};

const PATH_BUFFER_SIZE: usize = 2048;

pub fn system_path() -> PathBuf {
    path_from_xplm(|buffer| {
        // SAFETY: `buffer` points to a writable `PATH_BUFFER_SIZE` array.
        unsafe { XPLMGetSystemPath(buffer) }
    })
}

pub fn preferences_directory() -> PathBuf {
    let path = path_from_xplm(|buffer| {
        // SAFETY: `buffer` points to a writable `PATH_BUFFER_SIZE` array.
        unsafe { XPLMGetPrefsPath(buffer) }
    });
    path.parent().map(PathBuf::from).unwrap_or(path)
}

pub fn plugin_directory() -> Option<PathBuf> {
    let mut name = [0_i8; 512];
    let mut path = [0_i8; PATH_BUFFER_SIZE];
    let mut signature = [0_i8; 512];
    let mut description = [0_i8; 512];
    // SAFETY: all output buffers are writable and live for the call.
    unsafe {
        XPLMGetPluginInfo(
            XPLMGetMyID(),
            name.as_mut_ptr(),
            path.as_mut_ptr(),
            signature.as_mut_ptr(),
            description.as_mut_ptr(),
        );
    }
    path_from_buffer(&path)
        .parent()
        .and_then(|directory| directory.parent())
        .map(PathBuf::from)
}

pub fn current_aircraft_path() -> Option<PathBuf> {
    let mut file_name = [0_i8; 256];
    let mut path = [0_i8; PATH_BUFFER_SIZE];
    // SAFETY: both output buffers are writable and exceed the SDK minimum.
    unsafe { XPLMGetNthAircraftModel(0, file_name.as_mut_ptr(), path.as_mut_ptr()) };
    let path = path_from_buffer(&path);
    (!path.as_os_str().is_empty()).then_some(path)
}

fn path_from_xplm(fill: impl FnOnce(*mut c_char)) -> PathBuf {
    let mut buffer = [0_i8; PATH_BUFFER_SIZE];
    fill(buffer.as_mut_ptr());
    path_from_buffer(&buffer)
}

fn path_from_buffer(buffer: &[c_char]) -> PathBuf {
    // SAFETY: buffers are zero-initialized before XPLM writes into them, so a
    // terminating NUL remains even if an SDK call produces an empty value.
    let value = unsafe { CStr::from_ptr(buffer.as_ptr()) }.to_string_lossy();
    PathBuf::from(value.as_ref())
}
