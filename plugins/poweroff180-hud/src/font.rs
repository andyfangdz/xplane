//! Native GDI font rasterization and ownership of the SR20's alpha texture.
//! Drawing policy and primitive generation stay in the safe renderer.
use std::{marker::PhantomData, rc::Rc};
use windows_sys::Win32::{
    Foundation::SIZE,
    Graphics::{Gdi::*, OpenGL::*},
};
use xplane_plugin::opengl::DrawContext;
use xplane_sdk_sys::{XPLMBindTexture2d, XPLMGenerateTextureNumbers};

pub struct FontAtlas {
    texture: i32,
    advance: [f32; 96],
    _thread: PhantomData<Rc<()>>,
}
impl Default for FontAtlas {
    fn default() -> Self {
        Self {
            texture: 0,
            advance: [0.0; 96],
            _thread: PhantomData,
        }
    }
}
impl FontAtlas {
    pub fn ready(&self) -> bool {
        self.texture != 0
    }
    fn make_font(&mut self, gl: &mut DrawContext) -> bool {
        // SAFETY: all GDI objects are local, checked after creation, restored to
        // the memory DC before deletion, and pixels are read only while the DIB
        // is live. X-Plane's draw callback supplies the current GL context.
        unsafe {
            let dc = CreateCompatibleDC(std::ptr::null_mut());
            if dc.is_null() {
                return false;
            }
            let mut info = BITMAPINFO::default();
            info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
            info.bmiHeader.biWidth = 1024;
            info.bmiHeader.biHeight = -384;
            info.bmiHeader.biPlanes = 1;
            info.bmiHeader.biBitCount = 32;
            info.bmiHeader.biCompression = BI_RGB;
            let mut pixels = std::ptr::null_mut();
            let bitmap = CreateDIBSection(
                dc,
                &info,
                DIB_RGB_COLORS,
                &mut pixels,
                std::ptr::null_mut(),
                0,
            );
            if bitmap.is_null() {
                DeleteDC(dc);
                return false;
            }
            let font = CreateFontA(
                -44,
                0,
                0,
                0,
                FW_NORMAL as i32,
                0,
                0,
                0,
                u32::from(ANSI_CHARSET),
                u32::from(OUT_DEFAULT_PRECIS),
                u32::from(CLIP_DEFAULT_PRECIS),
                u32::from(ANTIALIASED_QUALITY),
                u32::from(DEFAULT_PITCH | FF_SWISS),
                c"Arial".as_ptr().cast(),
            );
            if font.is_null() {
                DeleteObject(bitmap);
                DeleteDC(dc);
                return false;
            }
            let old_bitmap = SelectObject(dc, bitmap);
            let old_font = SelectObject(dc, font);
            std::ptr::write_bytes(pixels, 0, 1024 * 384 * 4);
            SetTextColor(dc, 0x00ff_ffff);
            SetBkMode(dc, TRANSPARENT as i32);
            for i in 0..96 {
                let ch = (i + 32) as u8;
                let mut size = SIZE { cx: 0, cy: 0 };
                GetTextExtentPoint32A(dc, &ch, 1, &mut size);
                self.advance[i] = size.cx as f32;
                TextOutA(
                    dc,
                    (i % 16) as i32 * 64 + 2,
                    (i / 16) as i32 * 64 + 2,
                    &ch,
                    1,
                );
            }
            GdiFlush();
            let bytes = std::slice::from_raw_parts(pixels.cast::<u8>(), 1024 * 384 * 4);
            let alpha: Vec<u8> = bytes.as_chunks::<4>().0.iter().map(|p| p[1]).collect();
            SelectObject(dc, old_font);
            SelectObject(dc, old_bitmap);
            DeleteObject(font);
            DeleteObject(bitmap);
            DeleteDC(dc);
            XPLMGenerateTextureNumbers(&mut self.texture, 1);
            XPLMBindTexture2d(self.texture, 0);
            upload_alpha(gl, &alpha);
        }
        self.ready()
    }
    pub fn ensure(&mut self, gl: &mut DrawContext) -> bool {
        self.ready() || self.make_font(gl)
    }
    pub fn bind(&self, _gl: &mut DrawContext) {
        xplane_plugin::bind_texture_2d(self.texture);
    }
    pub fn advance(&self, index: usize) -> f32 {
        self.advance[index]
    }
}
impl Drop for FontAtlas {
    fn drop(&mut self) {
        if self.texture != 0 {
            // SAFETY: X-Plane unloads the renderer on its plugin thread, outside
            // drawing. Skip deletion if the context has already been destroyed.
            unsafe { DrawContext::with_current(|gl| gl.delete_texture(self.texture as u32)) };
        }
    }
}

// The borrowed context prevents calls inside a primitive. Fixed dimensions and
// explicit pixel-store layout keep GL reads inside the supplied alpha buffer.
fn upload_alpha(_gl: &mut DrawContext, alpha: &[u8]) {
    assert_eq!(alpha.len(), 1024 * 384);
    let parameters = [
        GL_UNPACK_ALIGNMENT,
        GL_UNPACK_ROW_LENGTH,
        GL_UNPACK_SKIP_ROWS,
        GL_UNPACK_SKIP_PIXELS,
    ];
    // SAFETY: each query writes one integer; with tight rows and no skips,
    // GL reads exactly the validated byte count. All pixel-store values are
    // restored after the immediate upload, which retains no client pointer.
    unsafe {
        let previous = parameters.map(|parameter| {
            let mut value = 0;
            glGetIntegerv(parameter, &mut value);
            value
        });
        for (parameter, value) in parameters.into_iter().zip([1, 0, 0, 0]) {
            glPixelStorei(parameter, value);
        }
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR as i32);
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR as i32);
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP as i32);
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP as i32);
        glTexImage2D(
            GL_TEXTURE_2D,
            0,
            GL_ALPHA as i32,
            1024,
            384,
            0,
            GL_ALPHA,
            GL_UNSIGNED_BYTE,
            alpha.as_ptr().cast(),
        );
        for (parameter, value) in parameters.into_iter().zip(previous) {
            glPixelStorei(parameter, value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alpha_upload_ignores_external_row_layout_and_restores_it() {
        let _context = crate::gl_test_support::Context::new();
        let pixels: Vec<u8> = (0..1024 * 384).map(|i| (i % 251) as u8).collect();
        // SAFETY: the test owns the current context; the readback buffer has
        // exactly the texture's dimensions and pack state stays at its defaults.
        unsafe {
            let mut texture = 0;
            glGenTextures(1, &mut texture);
            glBindTexture(GL_TEXTURE_2D, texture);
            for (parameter, value) in [
                (GL_UNPACK_ALIGNMENT, 8),
                (GL_UNPACK_ROW_LENGTH, 2048),
                (GL_UNPACK_SKIP_ROWS, 2),
                (GL_UNPACK_SKIP_PIXELS, 5),
            ] {
                glPixelStorei(parameter, value);
            }
            DrawContext::with_current(|gl| upload_alpha(gl, &pixels)).unwrap();
            let mut actual = vec![0_u8; pixels.len()];
            glGetTexImage(
                GL_TEXTURE_2D,
                0,
                GL_ALPHA,
                GL_UNSIGNED_BYTE,
                actual.as_mut_ptr().cast(),
            );
            assert_eq!(glGetError(), GL_NO_ERROR, "upload and readback");
            assert!(
                actual == pixels,
                "texture mismatch at {:?}",
                actual.iter().zip(&pixels).position(|(a, b)| a != b)
            );
            for (parameter, expected) in [
                (GL_UNPACK_ALIGNMENT, 8),
                (GL_UNPACK_ROW_LENGTH, 2048),
                (GL_UNPACK_SKIP_ROWS, 2),
                (GL_UNPACK_SKIP_PIXELS, 5),
            ] {
                let mut actual = 0;
                glGetIntegerv(parameter, &mut actual);
                assert_eq!(actual, expected);
            }
            glDeleteTextures(1, &texture);
            assert_eq!(glGetError(), GL_NO_ERROR);
        }
    }
}
