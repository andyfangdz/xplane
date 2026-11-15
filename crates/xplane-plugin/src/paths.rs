use std::path::PathBuf;

use crate::xplm::{self, PluginInfo};

const PATH_BUFFER_SIZE: usize = 2048;

pub fn system_path() -> PathBuf {
    let mut buffer = [0_u8; PATH_BUFFER_SIZE];
    xplm::system_path(&mut buffer);
    path_from_bytes(&buffer)
}

pub fn preferences_directory() -> PathBuf {
    let mut buffer = [0_u8; PATH_BUFFER_SIZE];
    xplm::prefs_path(&mut buffer);
    let path = path_from_bytes(&buffer);
    path.parent().map(PathBuf::from).unwrap_or(path)
}

pub fn plugin_directory() -> Option<PathBuf> {
    let mut name = [0_u8; 512];
    let mut path = [0_u8; PATH_BUFFER_SIZE];
    let mut signature = [0_u8; 512];
    let mut description = [0_u8; 512];
    let id = xplm::my_id();
    xplm::plugin_info(
        id,
        PluginInfo {
            name: Some(&mut name),
            file_path: Some(&mut path),
            signature: Some(&mut signature),
            description: Some(&mut description),
        },
    );
    path_from_bytes(&path)
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

/// A path from an SDK byte buffer: up to the first NUL, or all of it without
/// one, decoded lossily.
fn path_from_bytes(buffer: &[u8]) -> PathBuf {
    let end = buffer
        .iter()
        .position(|&byte| byte == 0)
        .unwrap_or(buffer.len());
    PathBuf::from(String::from_utf8_lossy(&buffer[..end]).into_owned())
}
