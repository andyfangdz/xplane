//! `XPLMGraphics.h`: graphics state, textures, 2-D drawing and coordinate
//! conversion. The drawing calls belong in a draw callback.

use std::ffi::CStr;

use xplane_sdk_sys as xp;

use super::count;

out_calls! {
    /// `XPLMWorldToLocal(latitude, longitude, altitude)` as `[x, y, z]`.
    pub fn world_to_local(latitude: f64, longitude: f64, altitude: f64) -> [f64; 3] =
        XPLMWorldToLocal(x, y, z);
    /// `XPLMLocalToWorld(x, y, z)` as `[latitude, longitude, altitude]`.
    pub fn local_to_world(x: f64, y: f64, z: f64) -> [f64; 3] =
        XPLMLocalToWorld(latitude, longitude, altitude);
}

value_calls! {
    /// `XPLMSetGraphicsState`, arguments in SDK order. Call from a draw callback.
    pub fn set_graphics_state(
        fog: i32,
        texture_units: i32,
        lighting: i32,
        alpha_testing: i32,
        alpha_blending: i32,
        depth_testing: i32,
        depth_writing: i32,
    ) = XPLMSetGraphicsState;
    /// `XPLMBindTexture2d(texture, unit)`. Call from a draw callback.
    pub fn bind_texture_2d(texture: i32, unit: i32) = XPLMBindTexture2d;
    /// `XPLMDrawTranslucentDarkBox(left, top, right, bottom)`. Call from a draw callback.
    pub fn draw_translucent_dark_box(left: i32, top: i32, right: i32, bottom: i32) =
        XPLMDrawTranslucentDarkBox;
}

/// `XPLMGenerateTextureNumbers(out, count)`, `count` being `out.len()`
/// saturated at `i32::MAX`.
pub fn generate_texture_numbers(out: &mut [i32]) {
    // SAFETY: `out` is exclusively borrowed and XPLM writes at most the count passed, which is
    // non-negative and at most its length.
    unsafe { xp::XPLMGenerateTextureNumbers(out.as_mut_ptr(), count(out.len())) }
}

/// `XPLMDrawString(color, x, y, text, wrap, font)`; `None` passes a null
/// word-wrap width. Call from a draw callback.
pub fn draw_string(color: &[f32; 3], x: i32, y: i32, text: &CStr, wrap: Option<&i32>, font: i32) {
    let wrap = wrap.map_or(std::ptr::null_mut(), |w| std::ptr::from_ref(w).cast_mut());
    // SAFETY: X-Plane only reads the 3 colour floats, the NUL-terminated text and the optional
    // wrap width, all borrowed for the call, despite the `*mut` in its signature.
    unsafe { xp::XPLMDrawString(color.as_ptr().cast_mut(), x, y, text.as_ptr(), wrap, font) }
}
