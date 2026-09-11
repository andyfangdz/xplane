use std::ffi::CStr;
use std::ptr;

use xplane_sdk_sys::{XPLMDrawString, XPLMFontID, XPLMMeasureString, XPLMSetGraphicsState};

/// Measures a NUL-terminated string using an X-Plane font.
pub fn measure_string(font: XPLMFontID, text: &CStr) -> f32 {
    let length = i32::try_from(text.to_bytes().len()).expect("text length exceeds i32::MAX");
    // SAFETY: `text` is NUL-terminated and remains live for the call.
    unsafe { XPLMMeasureString(font, text.as_ptr(), length) }
}

/// Draws a NUL-terminated string during an X-Plane drawing callback.
pub fn draw_string(color: [f32; 3], x: i32, y: i32, text: &CStr, font: XPLMFontID) {
    let mut color = color;
    // SAFETY: the color and string buffers remain live for this immediate
    // drawing call. A null word-wrap pointer disables wrapping.
    unsafe {
        XPLMDrawString(
            color.as_mut_ptr(),
            x,
            y,
            text.as_ptr(),
            ptr::null_mut(),
            font,
        );
    }
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
    // SAFETY: called on X-Plane's drawing thread while its GL context is current.
    unsafe { XPLMSetGraphicsState(0, i32::from(textured), 0, 0, i32::from(blending), 0, 0) };
}

/// Binds an SDK texture name to unit zero during an X-Plane draw callback.
pub fn bind_texture_2d(texture: i32) {
    // SAFETY: only a texture name is passed; no client pointer is retained.
    unsafe { xplane_sdk_sys::XPLMBindTexture2d(texture, 0) };
}
