use std::ffi::{c_char, CString};
use std::ptr;

use xplane_sdk_sys::{XPLMDebugString, XPLMEnableFeature};

#[derive(Copy, Clone)]
pub struct PluginMetadata<'a> {
    pub name: &'a str,
    pub signature: &'a str,
    pub description: &'a str,
}

pub fn c_string(value: &str) -> CString {
    CString::new(value.replace('\0', " ")).expect("sanitized string contains no NUL")
}

/// Writes the three metadata strings supplied to `XPluginStart`.
///
/// # Safety
/// Each destination must be the writable SDK-sized buffer supplied by X-Plane.
pub unsafe fn write_plugin_metadata(
    out_name: *mut c_char,
    out_signature: *mut c_char,
    out_description: *mut c_char,
    metadata: PluginMetadata<'_>,
) {
    // SAFETY: the function contract is forwarded to each individual buffer.
    unsafe {
        write_plugin_string(out_name, metadata.name);
        write_plugin_string(out_signature, metadata.signature);
        write_plugin_string(out_description, metadata.description);
    }
}

pub struct DebugLogger {
    prefix: &'static str,
}

impl DebugLogger {
    pub const fn new(prefix: &'static str) -> Self {
        Self { prefix }
    }

    pub fn log(&self, message: &str) {
        let message = c_string(&format!("{}: {message}\n", self.prefix));
        // SAFETY: `message` is NUL-terminated and live for the call.
        unsafe { XPLMDebugString(message.as_ptr()) }
    }
}

pub fn enable_feature(name: &str) {
    let name = c_string(name);
    // SAFETY: `name` is NUL-terminated and live for the call.
    unsafe { XPLMEnableFeature(name.as_ptr(), 1) };
}

unsafe fn write_plugin_string(destination: *mut c_char, value: &str) {
    if destination.is_null() {
        return;
    }
    let bytes = value.as_bytes();
    // SAFETY: the caller guarantees an SDK-sized writable output buffer. The
    // metadata values passed by these plugins are much shorter than that buffer.
    unsafe {
        ptr::copy_nonoverlapping(bytes.as_ptr(), destination.cast::<u8>(), bytes.len());
        *destination.add(bytes.len()) = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::c_string;

    #[test]
    fn c_string_replaces_interior_nuls() {
        let value = c_string("one\0two");
        assert_eq!(value.to_bytes(), b"one two");
    }
}
