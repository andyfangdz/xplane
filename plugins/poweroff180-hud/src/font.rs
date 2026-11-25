//! Original Arial strike and native X-Plane texture-atlas ownership.
use std::ptr::NonNull;
use windows_sys::Win32::{Foundation::SIZE, Graphics::Gdi::*};
use xplane_sdk_sys as sdk;

pub struct FontAtlas {
    texture: NonNull<std::ffi::c_void>,
    first_glyph: i32,
    advance: [f32; 96],
}
fn atlas_cells_from_dib(bgra: &[u8], width: usize, cell_height: usize) -> Vec<u8> {
    // AddImageSet keeps the top-down cell numbering, but DrawIn samples
    // each cell from its bottom scanline in X-Plane 12.4.4. Reverse the
    // scanlines inside each glyph without moving it to another glyph row.
    bgra.chunks_exact(width * cell_height * 4)
        .flat_map(|band| band.chunks_exact(width * 4).rev())
        .flat_map(|scanline| scanline.chunks_exact(4))
        .flat_map(|pixel| [255, 255, 255, pixel[1]])
        .collect()
}
impl FontAtlas {
    pub fn new() -> Option<Self> {
        let mut advance = [0.; 96];
        // GDI supplies the exact accepted Arial face and metrics, once at startup.
        // Its antialiased mask becomes a straight-alpha SDK atlas.
        unsafe {
            let dc = CreateCompatibleDC(std::ptr::null_mut());
            if dc.is_null() {
                return None;
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
                return None;
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
                return None;
            }
            let old_bitmap = SelectObject(dc, bitmap);
            let old_font = SelectObject(dc, font);
            std::ptr::write_bytes(pixels, 0, 1024 * 384 * 4);
            SetTextColor(dc, 0x00ff_ffff);
            SetBkMode(dc, TRANSPARENT as i32);
            for (i, advance) in advance.iter_mut().enumerate() {
                let ch = (i + 32) as u8;
                let mut size = SIZE { cx: 0, cy: 0 };
                GetTextExtentPoint32A(dc, &ch, 1, &mut size);
                *advance = size.cx as f32;
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
            let rgba = atlas_cells_from_dib(bytes, 1024, 64);
            SelectObject(dc, old_font);
            SelectObject(dc, old_bitmap);
            DeleteObject(font);
            DeleteObject(bitmap);
            DeleteDC(dc);
            let texture = NonNull::new(sdk::XPLMCreateTextureAtlas())?;
            let first_glyph =
                sdk::XPLMTextureAtlasAddImageSet(texture.as_ptr(), rgba.as_ptr(), 1024, 384, 16, 6);
            if first_glyph < 0 {
                sdk::XPLMDestroyTextureAtlas(texture.as_ptr());
                return None;
            }
            sdk::XPLMTextureAtlasBake(texture.as_ptr());
            Some(Self {
                texture,
                first_glyph,
                advance,
            })
        }
    }
    pub fn advance(&self, index: usize) -> f64 {
        f64::from(self.advance[index])
    }

    pub fn draw_glyph(&self, i: usize, color: u32, left: f64, top: f64, side: f64) {
        // The design uses y-down coordinates, the SDK uses y-up.
        unsafe {
            sdk::XPLMTextureAtlasDrawIn(
                self.texture.as_ptr(),
                self.first_glyph + i as i32,
                color,
                left as f32,
                -top as f32,
                (left + side) as f32,
                -(top + side) as f32,
            );
        }
    }
}
impl Drop for FontAtlas {
    fn drop(&mut self) {
        unsafe { sdk::XPLMDestroyTextureAtlas(self.texture.as_ptr()) }
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn each_glyph_is_upright_without_swapping_neighboring_cells() {
        // Each two-row band has a different asymmetric mask in each column.
        let dib: Vec<_> = [0, 8, 73, 64, 192, 201, 255, 240]
            .into_iter()
            .flat_map(|mask| [9, mask, 7, 99])
            .collect();
        let atlas = super::atlas_cells_from_dib(&dib, 2, 2);
        let expected = [73, 64, 0, 8, 255, 240, 192, 201];
        for (pixel, mask) in atlas.chunks_exact(4).zip(expected) {
            assert_eq!(pixel, [255, 255, 255, mask]);
        }
    }
}
