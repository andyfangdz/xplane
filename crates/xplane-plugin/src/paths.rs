use std::ffi::c_char;
use std::path::PathBuf;

use xplane_sdk_sys::{XPLMGetPluginInfo, XPLMGetPrefsPath};

const PATH_BUFFER_SIZE: usize = 2048;

pub fn system_path() -> PathBuf {
    let mut buffer = [0_u8; PATH_BUFFER_SIZE];
    crate::xplm::system_path(&mut buffer);
    path_from_bytes(&buffer)
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
    let id = crate::xplm::my_id();
    // SAFETY: all output buffers are writable and live for the call.
    unsafe {
        XPLMGetPluginInfo(
            id,
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
    let mut file_name = [0_u8; 256];
    let mut path = [0_u8; PATH_BUFFER_SIZE];
    crate::xplm::nth_aircraft_model(0, &mut file_name, &mut path);
    let path = path_from_bytes(&path);
    (!path.as_os_str().is_empty()).then_some(path)
}

fn path_from_xplm(fill: impl FnOnce(*mut c_char)) -> PathBuf {
    let mut buffer = [0_i8; PATH_BUFFER_SIZE];
    fill(buffer.as_mut_ptr());
    path_from_buffer(&buffer)
}

fn path_from_buffer(buffer: &[c_char]) -> PathBuf {
    PathBuf::from(crate::plugin::read_c_buffer(buffer))
}

/// As [`path_from_buffer`], for a byte buffer: up to the first NUL, or all of
/// it without one, decoded lossily.
fn path_from_bytes(buffer: &[u8]) -> PathBuf {
    let end = buffer
        .iter()
        .position(|&byte| byte == 0)
        .unwrap_or(buffer.len());
    PathBuf::from(String::from_utf8_lossy(&buffer[..end]).into_owned())
}
