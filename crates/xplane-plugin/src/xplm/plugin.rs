//! `XPLMPlugin.h`: this plugin's id and other plugins' state.
//!
//! Plugin ids are integers X-Plane looks up in its own plugin table (module
//! note), passed through unchanged: an id may legitimately come from calls
//! this module does not wrap (`XPLMGetNthPlugin`, `XPLMFindPluginByPath`, the
//! sender of a plugin message) or be a constant such as `XPLM_PLUGIN_XPLANE`.

use xplane_sdk_sys as xp;

value_calls! {
    /// `XPLMGetMyID()`.
    pub fn my_id() -> i32 = XPLMGetMyID;
    /// `XPLMIsPluginEnabled(id)`.
    pub fn is_plugin_enabled(id: i32) -> i32 = XPLMIsPluginEnabled;
}

/// The outputs of [`plugin_info`]; `None` passes a null pointer, which
/// `XPLMPlugin.h` allows for each.
#[derive(Default)]
pub struct PluginInfo<'a> {
    /// At least 256 bytes.
    pub name: Option<&'a mut [u8]>,
    /// At least 256 bytes.
    pub file_path: Option<&'a mut [u8]>,
    /// At least 256 bytes.
    pub signature: Option<&'a mut [u8]>,
    /// At least 256 bytes.
    pub description: Option<&'a mut [u8]>,
}

/// `XPLMGetPluginInfo(id, name, file_path, signature, description)`.
///
/// # Panics
/// If a buffer is shorter than the 256 bytes `XPLMPlugin.h` asks for.
pub fn plugin_info(id: i32, info: PluginInfo<'_>) {
    let out = |buffer: Option<&mut [u8]>| {
        buffer.map_or(std::ptr::null_mut(), |buffer| {
            assert!(buffer.len() >= 256, "XPLM plugin info buffer too small");
            buffer.as_mut_ptr().cast()
        })
    };
    let (name, file_path) = (out(info.name), out(info.file_path));
    let (signature, description) = (out(info.signature), out(info.description));
    // SAFETY: `id` is an integer X-Plane looks up itself (module note); each output is null
    // (skipped by XPLM) or an exclusively borrowed buffer of at least 256 bytes, the size
    // XPLMPlugin.h asks for. The call takes no lengths, so that documented minimum is the bound
    // relied on.
    unsafe { xp::XPLMGetPluginInfo(id, name, file_path, signature, description) }
}

/// `XPLMGetPluginInfo(id, null, path, null, null)`: the plugin's file path.
///
/// # Panics
/// If `path` is shorter than 512 bytes.
pub fn plugin_file_path(id: i32, path: &mut [u8]) {
    assert!(path.len() >= 512, "XPLM plugin path buffer too small");
    plugin_info(
        id,
        PluginInfo {
            file_path: Some(path),
            ..PluginInfo::default()
        },
    );
}

str_calls! {
    /// `XPLMFindPluginBySignature(signature)`: the plugin's id, or
    /// `XPLM_NO_PLUGIN_ID`.
    pub fn find_plugin_by_signature(signature: &CStr) -> i32 = XPLMFindPluginBySignature;
    /// `XPLMEnableFeature(feature, enable)`.
    pub fn enable_feature(feature: &CStr, enable: i32) = XPLMEnableFeature;
}
