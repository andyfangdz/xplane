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
    // SAFETY: this function only changes XPLM-managed graphics state and is
    // called by plugins from X-Plane's drawing callback.
    unsafe { XPLMSetGraphicsState(0, 0, 0, 0, 1, 0, 0) };
}
