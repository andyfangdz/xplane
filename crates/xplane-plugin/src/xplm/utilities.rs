//! `XPLMUtilities.h`, `XPLMPlanes.h` and `XPLMProcessing.h`: logging, paths,
//! aircraft models and the cycle number.

use std::ffi::CStr;

use xplane_sdk_sys as xp;

str_calls! {
    /// `XPLMDebugString(text)`.
    pub fn debug_string(text: &CStr) = XPLMDebugString;
    /// `XPLMSpeakString(text)`: X-Plane speaks the text (and, depending on
    /// the user's settings, shows it on screen). It needs no plugin window or
    /// callback, so it still reaches the pilot after a plugin fault.
    pub fn speak_string(text: &CStr) = XPLMSpeakString;
}

/// `XPLMDebugString` of `text` plus a trailing NUL (an owned copy). An
/// interior NUL only shortens the logged line.
pub fn debug_bytes(text: &[u8]) {
    let mut owned = Vec::with_capacity(text.len() + 1);
    owned.extend_from_slice(text);
    owned.push(0);
    // The same pointer as before: `owned`, whose string X-Plane reads up to the first NUL.
    debug_string(CStr::from_bytes_until_nul(&owned).expect("a NUL was pushed"));
}

/// Wrappers for the class of XPLM calls that fill one path buffer without a
/// length: the buffer is exclusively borrowed and checked to be at least the
/// 512 bytes `XPLMUtilities.h` asks for, the bound relied on. Members:
/// `XPLMGetSystemPath`, `XPLMGetPrefsPath`.
macro_rules! path_calls {
    ($($(#[$attr:meta])* pub fn $name:ident(out) = $sdk:ident, $message:literal;)+) => {$(
        $(#[$attr])*
        ///
        /// # Panics
        /// If `out` is shorter than the SDK's 512 bytes.
        pub fn $name(out: &mut [u8]) {
            assert!(out.len() >= 512, $message);
            // SAFETY: a path call (macro note): `out` is exclusively borrowed and at least 512
            // bytes, the size the header asks for.
            unsafe { xp::$sdk(out.as_mut_ptr().cast()) }
        }
    )+};
}

path_calls! {
    /// `XPLMGetSystemPath(out)`.
    pub fn system_path(out) = XPLMGetSystemPath, "XPLM system path buffer too small";
    /// `XPLMGetPrefsPath(out)`.
    pub fn prefs_path(out) = XPLMGetPrefsPath, "XPLM preferences path buffer too small";
}

/// `XPLMGetNthAircraftModel(index, name, path)`.
///
/// # Panics
/// If `name` is shorter than the SDK's 256 bytes or `path` than its 512.
pub fn nth_aircraft_model(index: i32, name: &mut [u8], path: &mut [u8]) {
    assert!(
        name.len() >= 256 && path.len() >= 512,
        "XPLM model buffers too small"
    );
    // SAFETY: both buffers are exclusively borrowed and at least 256 and 512 bytes, the sizes
    // XPLMPlanes.h asks for. The call takes no length, so that documented minimum is the bound
    // relied on.
    unsafe {
        xp::XPLMGetNthAircraftModel(index, name.as_mut_ptr().cast(), path.as_mut_ptr().cast())
    }
}

value_calls! {
    /// `XPLMGetCycleNumber()`.
    pub fn cycle_number() -> i32 = XPLMGetCycleNumber;
}
