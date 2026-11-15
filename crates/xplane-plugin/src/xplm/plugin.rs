//! `XPLMPlugin.h`: this plugin's id and other plugins' state.

use std::ffi::CStr;

use xplane_sdk_sys as xp;

/// `XPLMGetMyID()`.
pub fn my_id() -> i32 {
    // SAFETY: no arguments; returns this plugin's id.
    unsafe { xp::XPLMGetMyID() }
}

/// `XPLMGetPluginInfo(id, null, path, null, null)`: the plugin's file path.
///
/// # Panics
/// If `path` is shorter than 512 bytes.
pub fn plugin_file_path(id: i32, path: &mut [u8]) {
    assert!(path.len() >= 512, "XPLM plugin path buffer too small");
    let null = std::ptr::null_mut();
    // SAFETY: null outputs are skipped by XPLM; `path` is exclusively borrowed and at least 512
    // bytes, twice the 256 XPLMPlugin.h asks for. The call takes no length, so that documented
    // minimum is the bound relied on.
    unsafe { xp::XPLMGetPluginInfo(id, null, path.as_mut_ptr().cast(), null, null) }
}

/// `XPLMFindPluginBySignature(signature)`: the plugin's id, or
/// `XPLM_NO_PLUGIN_ID`.
pub fn find_plugin_by_signature(signature: &CStr) -> i32 {
    // SAFETY: `signature` is NUL-terminated and outlives the call; XPLM only reads it.
    unsafe { xp::XPLMFindPluginBySignature(signature.as_ptr()) }
}

/// `XPLMIsPluginEnabled(id)`.
pub fn is_plugin_enabled(id: i32) -> i32 {
    // SAFETY: a plain plugin id, which XPLM looks up itself (an unknown id is not enabled).
    unsafe { xp::XPLMIsPluginEnabled(id) }
}
