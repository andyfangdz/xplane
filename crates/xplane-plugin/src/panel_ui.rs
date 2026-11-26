//! Small native drawing primitives for SDK 4.4 panel-graphics windows.
pub use crate::retained::{RetainedDrawing, RetainedPrimitive};
use std::{
    ffi::{CStr, c_void},
    ptr::NonNull,
};
use xplane_sdk_sys as sdk;

/// Fill a rectangle in global window boxels. Colors are straight RGBA bytes.
pub fn fill_rect([l, t, r, b]: [i32; 4], color: [u8; 4]) {
    let vertex = |x, y| sdk::XPLMVertex_t {
        x: x as f32,
        y: y as f32,
    };
    let corners = [vertex(l, b), vertex(r, b), vertex(r, t), vertex(l, t)];
    // SAFETY: the four vertices remain live for the panel drawing call.
    unsafe { sdk::XPLMPolygon(u32::from_le_bytes(color), corners.as_ptr(), 4) }
}

/// A TrueType face and its size in window boxels. Keep it on the SDK thread.
pub struct Font {
    handle: NonNull<c_void>,
    size: f32,
}

impl Font {
    /// Create the font from a simulator-relative path with forward slashes.
    pub fn load(path: &CStr, size: f32) -> Option<Self> {
        // SAFETY: the SDK creates an opaque, uniquely owned font handle.
        let handle = NonNull::new(unsafe { sdk::XPLMCreateFont(sdk::xplm_CharSetUnicode) })?;
        let font = Self { handle, size };
        // SAFETY: the path and font are live through the call. Drop handles failure.
        (unsafe { sdk::XPLMFontAddFace(handle.as_ptr(), path.as_ptr()) } != 0).then_some(font)
    }

    pub fn measure(&self, text: &CStr) -> f32 {
        self.measure_scaled(1., text)
    }

    /// Measure the horizontal advance at the same size used by `draw_scaled`.
    pub fn measure_scaled(&self, scale: f32, text: &CStr) -> f32 {
        // SAFETY: both the face and the NUL-terminated string are live.
        unsafe {
            sdk::XPLMFontMeasureString(self.handle.as_ptr(), self.size * scale, text.as_ptr())
        }
    }

    /// Adjust the SDK size so that a reference string has the specified advance in boxels.
    pub fn fit_width(mut self, sample: &CStr, width: f32) -> Option<Self> {
        let measured = self.measure(sample);
        if !measured.is_finite() || measured <= 0. || !width.is_finite() || width <= 0. {
            return None;
        }
        self.size *= width / measured;
        Some(self)
    }

    pub fn draw(
        &self,
        [x, y]: [f32; 2],
        text: &CStr,
        color: [u8; 4],
        justify: sdk::XPLMJustification_t,
    ) {
        self.draw_scaled(1., [x, y], text, color, justify);
    }

    /// Draw the same calibrated face at a multiple of its reference size.
    pub fn draw_scaled(
        &self,
        scale: f32,
        [x, y]: [f32; 2],
        text: &CStr,
        color: [u8; 4],
        justify: sdk::XPLMJustification_t,
    ) {
        // SAFETY: the font and text remain live throughout the window draw.
        unsafe {
            sdk::XPLMFontDrawString(
                self.handle.as_ptr(),
                u32::from_le_bytes(color),
                self.size * scale,
                x,
                y,
                text.as_ptr(),
                justify,
            )
        }
    }

    pub fn draw_wrapped(&self, [x, y]: [f32; 2], text: &CStr, width: i32, color: [u8; 4]) {
        // SAFETY: the font and text remain live throughout the window draw.
        unsafe {
            sdk::XPLMFontDrawStringWordWrapped(
                self.handle.as_ptr(),
                u32::from_le_bytes(color),
                self.size,
                x,
                y,
                text.as_ptr(),
                width,
                sdk::xplm_JustLeft,
            )
        }
    }
}

impl Drop for Font {
    fn drop(&mut self) {
        // SAFETY: this owner destroys the handle once on the plugin thread.
        unsafe { sdk::XPLMDestroyFont(self.handle.as_ptr()) }
    }
}
