use std::ffi::CStr;

use xplane_sdk_sys::{XPLMFontID, XPLMMeasureString};

/// Measures a NUL-terminated string using an X-Plane font.
pub fn measure_string(font: XPLMFontID, text: &CStr) -> f32 {
    let length = i32::try_from(text.to_bytes().len()).expect("text length exceeds i32::MAX");
    // SAFETY: `text` is NUL-terminated and remains live for the call.
    unsafe { XPLMMeasureString(font, text.as_ptr(), length) }
}

/// Draws a NUL-terminated string during an X-Plane drawing callback.
pub fn draw_string(color: [f32; 3], x: i32, y: i32, text: &CStr, font: XPLMFontID) {
    // A null word-wrap pointer disables wrapping.
    crate::xplm::draw_string(&color, x, y, text, None, font);
}

/// Restores the basic graphics state expected for X-Plane 2-D drawing.
pub fn set_2d_graphics_state() {
    set_graphics_state(true);
}

/// Selects the untextured 2-D SDK graphics state, optionally enabling blending.
pub fn set_graphics_state(blending: bool) {
    graphics_state(false, blending);
}

/// Selects one textured, blended 2-D texture unit in an X-Plane draw callback.
pub fn set_textured_graphics_state() {
    graphics_state(true, true);
}

fn graphics_state(textured: bool, blending: bool) {
    crate::xplm::set_graphics_state(0, i32::from(textured), 0, 0, i32::from(blending), 0, 0);
}

/// Binds an SDK texture name to unit zero during an X-Plane draw callback.
pub fn bind_texture_2d(texture: i32) {
    crate::xplm::bind_texture_2d(texture, 0);
}
