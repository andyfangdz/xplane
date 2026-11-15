//! `XPLMGraphics.h`: graphics state, textures, 2-D drawing and coordinate
//! conversion. The drawing calls belong in a draw callback.

use std::ffi::CStr;

use xplane_sdk_sys as xp;

use super::count;

/// `XPLMWorldToLocal(latitude, longitude, altitude)` as `[x, y, z]`, through
/// [`crate::world_to_local`].
pub fn world_to_local(latitude: f64, longitude: f64, altitude: f64) -> [f64; 3] {
    let (x, y, z) = crate::world_to_local(latitude, longitude, altitude);
    [x, y, z]
}

/// `XPLMLocalToWorld(x, y, z)` as `[latitude, longitude, altitude]`, through
/// [`crate::local_to_world`].
pub fn local_to_world(x: f64, y: f64, z: f64) -> [f64; 3] {
    let (latitude, longitude, altitude) = crate::local_to_world(x, y, z);
    [latitude, longitude, altitude]
}

/// `XPLMSetGraphicsState`, arguments in SDK order. Call from a draw callback.
pub fn set_graphics_state(
    fog: i32,
    texture_units: i32,
    lighting: i32,
    alpha_testing: i32,
    alpha_blending: i32,
    depth_testing: i32,
    depth_writing: i32,
) {
    // SAFETY: plain integer flags; X-Plane changes only its own GL state cache.
    unsafe {
        xp::XPLMSetGraphicsState(
            fog,
            texture_units,
            lighting,
            alpha_testing,
            alpha_blending,
            depth_testing,
            depth_writing,
        )
    }
}

/// `XPLMBindTexture2d(texture, unit)`. Call from a draw callback.
pub fn bind_texture_2d(texture: i32, unit: i32) {
    // SAFETY: plain integer texture and unit numbers; no pointers.
    unsafe { xp::XPLMBindTexture2d(texture, unit) }
}

/// `XPLMGenerateTextureNumbers(out, count)`, `count` being `out.len()`
/// saturated at `i32::MAX`.
pub fn generate_texture_numbers(out: &mut [i32]) {
    // SAFETY: `out` is exclusively borrowed and XPLM writes at most the count passed, which is
    // non-negative and at most its length.
    unsafe { xp::XPLMGenerateTextureNumbers(out.as_mut_ptr(), count(out.len())) }
}

/// `XPLMDrawTranslucentDarkBox(left, top, right, bottom)`. Call from a draw callback.
pub fn draw_translucent_dark_box(left: i32, top: i32, right: i32, bottom: i32) {
    // SAFETY: plain coordinates; draws with X-Plane's current GL context.
    unsafe { xp::XPLMDrawTranslucentDarkBox(left, top, right, bottom) }
}

/// `XPLMDrawString(color, x, y, text, wrap, font)`; `None` passes a null
/// word-wrap width. Call from a draw callback.
pub fn draw_string(color: &[f32; 3], x: i32, y: i32, text: &CStr, wrap: Option<&i32>, font: i32) {
    let wrap = wrap.map_or(std::ptr::null_mut(), |w| std::ptr::from_ref(w).cast_mut());
    // SAFETY: X-Plane only reads the 3 colour floats, the NUL-terminated text and the optional
    // wrap width, all borrowed for the call, despite the `*mut` in its signature.
    unsafe { xp::XPLMDrawString(color.as_ptr().cast_mut(), x, y, text.as_ptr(), wrap, font) }
}
