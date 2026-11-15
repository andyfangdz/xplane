//! `XPLMUtilities.h`, `XPLMPlanes.h` and `XPLMProcessing.h`: logging, paths,
//! aircraft models and the cycle number.

use std::ffi::CStr;

use xplane_sdk_sys as xp;

/// `XPLMDebugString(text)`.
pub fn debug_string(text: &CStr) {
    // SAFETY: `text` is NUL-terminated and outlives the call; X-Plane copies it.
    unsafe { xp::XPLMDebugString(text.as_ptr()) }
}

/// `XPLMDebugString` of `text` plus a trailing NUL (an owned copy). An
/// interior NUL only shortens the logged line.
pub fn debug_bytes(text: &[u8]) {
    let mut owned = Vec::with_capacity(text.len() + 1);
    owned.extend_from_slice(text);
    owned.push(0);
    // SAFETY: `owned` ends in the NUL pushed above and outlives the call; X-Plane copies it.
    unsafe { xp::XPLMDebugString(owned.as_ptr().cast()) }
}

/// `XPLMGetSystemPath(out)`.
///
/// # Panics
/// If `out` is shorter than the SDK's 512 bytes.
pub fn system_path(out: &mut [u8]) {
    assert!(out.len() >= 512, "XPLM system path buffer too small");
    // SAFETY: `out` is exclusively borrowed and at least 512 bytes, the size XPLMUtilities.h
    // asks for. The call takes no length, so that documented minimum is the bound relied on.
    unsafe { xp::XPLMGetSystemPath(out.as_mut_ptr().cast()) }
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

/// `XPLMGetCycleNumber()`.
pub fn cycle_number() -> i32 {
    // SAFETY: no arguments; returns the current flight-loop cycle number.
    unsafe { xp::XPLMGetCycleNumber() }
}
