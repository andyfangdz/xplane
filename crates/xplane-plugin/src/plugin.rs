use std::ffi::{c_char, CString};
use std::ptr;

use xplane_sdk_sys::{XPLMDebugString, XPLMEnableFeature};

const PLUGIN_STRING_CAPACITY: usize = 256;

#[derive(Copy, Clone)]
pub struct PluginMetadata<'a> {
    pub name: &'a str,
    pub signature: &'a str,
    pub description: &'a str,
}

/// Defines X-Plane's five required plugin entry points around safe lifecycle
/// functions. The only raw-buffer operation remains inside the shared crate.
#[macro_export]
macro_rules! export_plugin {
    (
        metadata: $metadata:expr,
        start: $start:path,
        stop: $stop:path,
        enable: $enable:path,
        disable: $disable:path,
        receive_message: $receive_message:path $(,)?
    ) => {
        #[no_mangle]
        /// X-Plane plugin startup entry point.
        ///
        /// # Safety
        /// X-Plane must provide writable SDK-sized metadata buffers and invoke
        /// this function on its plugin-management thread.
        pub unsafe extern "C" fn XPluginStart(
            out_name: *mut ::std::ffi::c_char,
            out_signature: *mut ::std::ffi::c_char,
            out_description: *mut ::std::ffi::c_char,
        ) -> ::std::ffi::c_int {
            // SAFETY: this entry point's contract is precisely the contract of
            // the shared metadata writer.
            unsafe {
                $crate::write_plugin_metadata(out_name, out_signature, out_description, $metadata);
            }
            ::std::ffi::c_int::from($start())
        }

        #[no_mangle]
        pub extern "C" fn XPluginStop() {
            $stop();
        }

        #[no_mangle]
        pub extern "C" fn XPluginEnable() -> ::std::ffi::c_int {
            ::std::ffi::c_int::from($enable())
        }

        #[no_mangle]
        pub extern "C" fn XPluginDisable() {
            $disable();
        }

        #[no_mangle]
        pub extern "C" fn XPluginReceiveMessage(
            from: xplane_sdk_sys::XPLMPluginID,
            message: ::std::ffi::c_int,
            parameter: *mut ::std::ffi::c_void,
        ) {
            $receive_message(from, message, parameter);
        }
    };
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
    let length = bytes.len().min(PLUGIN_STRING_CAPACITY - 1);
    // SAFETY: the caller guarantees an SDK-sized writable output buffer, and
    // `length` is capped so the terminator stays within its 256 bytes.
    unsafe {
        ptr::copy_nonoverlapping(bytes.as_ptr(), destination.cast::<u8>(), length);
        *destination.add(length) = 0;
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::c_char;

    use super::{c_string, write_plugin_string, PLUGIN_STRING_CAPACITY};

    #[test]
    fn c_string_replaces_interior_nuls() {
        let value = c_string("one\0two");
        assert_eq!(value.to_bytes(), b"one two");
    }

    #[test]
    fn plugin_strings_are_truncated_to_the_sdk_buffer() {
        let mut buffer = [b'!' as c_char; PLUGIN_STRING_CAPACITY + 1];
        // SAFETY: `buffer` provides the SDK-sized writable region required by
        // `write_plugin_string`, plus a sentinel byte used by this test.
        unsafe { write_plugin_string(buffer.as_mut_ptr(), &"x".repeat(400)) };
        assert_eq!(buffer[PLUGIN_STRING_CAPACITY - 1], 0);
        assert_eq!(buffer[PLUGIN_STRING_CAPACITY], b'!' as c_char);
    }
}
