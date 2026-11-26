use std::cell::RefCell;
use std::ffi::{CString, c_char, c_void};
use std::mem::MaybeUninit;

use crate::{CommandHandler, registrations::CommandHandle};
use xplane_sdk_sys as sdk;

const PLUGIN_STRING_CAPACITY: usize = 256;

#[derive(Copy, Clone)]
pub struct PluginMetadata<'a> {
    pub name: &'a str,
    pub signature: &'a str,
    pub description: &'a str,
}

/// Defines X-Plane's five required plugin entry points around safe lifecycle
/// functions. The only raw-buffer operation remains inside the shared crate.
///
/// Every entry point runs under the plugin's panic boundary
/// ([`crate::guard`]). After a fault, start, enable and message delivery
/// return without running plugin code; stop and disable still run so the
/// plugin can release its SDK registrations before X-Plane unloads it.
#[macro_export]
macro_rules! export_plugin {
    (
        metadata: $metadata:expr_2021,
        start: $start:path,
        stop: $stop:path,
        enable: $enable:path,
        disable: $disable:path,
        receive_message: $receive_message:path $(,)?
    ) => {
        #[unsafe(no_mangle)]
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
            let metadata: $crate::PluginMetadata<'_> = $metadata;
            $crate::guard::install_panic_hook();
            $crate::guard::PLUGIN.set_logger($crate::guard::xplane_log);
            $crate::guard::PLUGIN.set_label(metadata.name);
            // SAFETY: this entry point's contract is precisely the contract of
            // the shared metadata writer.
            unsafe {
                $crate::write_plugin_metadata(out_name, out_signature, out_description, metadata);
            }
            $crate::guard::guard("XPluginStart", 0, || ::std::ffi::c_int::from($start()))
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn XPluginStop() {
            $crate::guard::guard_teardown("XPluginStop", (), || {
                $crate::remove_development_reload();
                $stop()
            });
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn XPluginEnable() -> ::std::ffi::c_int {
            $crate::guard::guard("XPluginEnable", 0, || {
                let enabled = ::std::ffi::c_int::from($enable());
                if enabled != 0
                    && ::std::env::var_os("XPLANE_DEV_RELOAD").as_deref()
                        == Some(::std::ffi::OsStr::new("1"))
                {
                    $crate::install_development_reload($metadata);
                }
                enabled
            })
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn XPluginDisable() {
            $crate::guard::guard_teardown("XPluginDisable", (), || {
                $crate::remove_development_reload();
                $disable()
            });
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn XPluginReceiveMessage(
            from: xplane_sdk_sys::XPLMPluginID,
            message: ::std::ffi::c_int,
            parameter: *mut ::std::ffi::c_void,
        ) {
            $crate::guard::guard("XPluginReceiveMessage", (), || {
                $receive_message(from, message, parameter)
            });
        }
    };
}

thread_local! {
    static RELOAD_COMMAND: RefCell<Option<CommandHandler>> = const { RefCell::new(None) };
}

/// Called on the plugin thread by `export_plugin!` in an opted-in development session.
pub fn install_development_reload(metadata: PluginMetadata<'_>) {
    if RELOAD_COMMAND.with_borrow(Option::is_some) {
        return;
    }
    let (mut sim_version, mut sdk_version, mut host) = (0, 0, 0);
    // SAFETY: the lifecycle callback is on X-Plane's thread and all three outputs are live.
    unsafe { sdk::XPLMGetVersions(&mut sim_version, &mut sdk_version, &mut host) };
    if sdk_version < 440 {
        return;
    }
    // Look up the optional SDK function at runtime so older plugins using the
    // shared entry points retain their original minimum simulator version.
    let address = unsafe { sdk::XPLMFindSymbol(c"XPLMReloadThisPlugin".as_ptr()) };
    if address.is_null() {
        return;
    }
    // SAFETY: the SDK name and signature match XPLMPlugin.h. The simulator
    // owns the function for the lifetime of the command registration.
    let reload = unsafe { std::mem::transmute::<*mut c_void, unsafe extern "C" fn(i32)>(address) };
    let name = c_string(&format!(
        "xplane/dev/{}/reload_for_replacement",
        metadata.signature
    ));
    let description = c_string(&format!(
        "Unload {} and pause to replace its plugin binary",
        metadata.name
    ));
    if let Some(command) = CommandHandle::create(&name, &description) {
        let handler =
            CommandHandler::register(command, true, "development reload", move |_, phase| {
                if phase == sdk::xplm_CommandBegin {
                    // SAFETY: a main-thread command callback; X-Plane defers unloading
                    // this plugin until the callback has returned.
                    unsafe { reload(1) };
                }
                0
            });
        RELOAD_COMMAND.set(Some(handler));
    }
}

pub fn remove_development_reload() {
    drop(RELOAD_COMMAND.take());
}

/// Decodes only the supplied SDK buffer, even if it has no NUL terminator.
pub(crate) fn read_c_buffer(buffer: &[c_char]) -> String {
    let bytes: Vec<u8> = buffer
        .iter()
        .copied()
        .take_while(|&c| c != 0)
        .map(|c| c as u8)
        .collect();
    String::from_utf8_lossy(&bytes).into_owned()
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
    for (destination, value) in [
        (out_name, metadata.name),
        (out_signature, metadata.signature),
        (out_description, metadata.description),
    ] {
        if destination.is_null() {
            continue;
        }
        // SAFETY: the caller guarantees each destination is X-Plane's writable SDK-sized buffer
        // (256 bytes), not otherwise in use during this call; it may be uninitialized, so it is
        // viewed as `MaybeUninit` bytes and only written.
        let buffer = unsafe {
            std::slice::from_raw_parts_mut(
                destination.cast::<MaybeUninit<u8>>(),
                PLUGIN_STRING_CAPACITY,
            )
        };
        write_plugin_string(buffer, value);
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
        crate::xplm::debug_string(&message);
    }
}

pub fn enable_feature(name: &str) {
    crate::xplm::enable_feature(&c_string(name), 1);
}

/// Writes `value`, cut to leave room for the terminator, and a NUL into the
/// SDK-sized `destination`.
fn write_plugin_string(destination: &mut [MaybeUninit<u8>], value: &str) {
    let bytes = value.as_bytes();
    let length = bytes.len().min(PLUGIN_STRING_CAPACITY - 1);
    for (slot, &byte) in destination.iter_mut().zip(&bytes[..length]) {
        slot.write(byte);
    }
    destination[length].write(0);
}

#[cfg(test)]
mod tests {
    use std::ffi::c_char;
    use std::mem::MaybeUninit;

    use super::{PLUGIN_STRING_CAPACITY, c_string, read_c_buffer, write_plugin_string};

    #[test]
    fn sdk_strings_are_bounded_with_or_without_a_terminator() {
        let read =
            |bytes: &[u8]| read_c_buffer(&bytes.iter().map(|&b| b as c_char).collect::<Vec<_>>());
        assert_eq!(read(b""), "");
        assert_eq!(read(b"\0trailing"), "");
        assert_eq!(read(b"runway\0trailing"), "runway");
        assert_eq!(read(b"full buffer"), "full buffer");
        assert_eq!(read("é".as_bytes()), "é");
        assert_eq!(read(&[0xff, 0]), "\u{fffd}");
    }

    #[test]
    fn c_string_replaces_interior_nuls() {
        let value = c_string("one\0two");
        assert_eq!(value.to_bytes(), b"one two");
    }

    #[test]
    fn plugin_strings_are_truncated_to_the_sdk_buffer() {
        let mut buffer = [MaybeUninit::new(b'!'); PLUGIN_STRING_CAPACITY + 1];
        write_plugin_string(&mut buffer[..PLUGIN_STRING_CAPACITY], &"x".repeat(400));
        let short = &mut [MaybeUninit::new(b'!'); 8];
        write_plugin_string(short, "abc");
        // SAFETY: every byte of both buffers was initialized when they were made.
        let (buffer, short) = unsafe {
            (
                buffer.map(|b| b.assume_init()),
                short.map(|b| b.assume_init()),
            )
        };
        assert!(
            buffer[..PLUGIN_STRING_CAPACITY - 1]
                .iter()
                .all(|&b| b == b'x')
        );
        assert_eq!(buffer[PLUGIN_STRING_CAPACITY - 1], 0);
        assert_eq!(buffer[PLUGIN_STRING_CAPACITY], b'!');
        assert_eq!(short, *b"abc\0!!!!");
    }
}
