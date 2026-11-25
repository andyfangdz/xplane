use std::{marker::PhantomData, ptr, rc::Rc, sync::OnceLock};
use windows_sys::Win32::Graphics::GdiPlus::*;

/// GDI+ session shared by canvases on the plugin thread.
pub struct Gdi(usize, PhantomData<Rc<()>>);
impl Gdi {
    pub fn start() -> Result<Self, String> {
        let mut token = 0;
        let input = GdiplusStartupInput {
            GdiplusVersion: 1,
            ..Default::default()
        };
        if unsafe { GdiplusStartup(&mut token, &input, ptr::null_mut()) } != 0 {
            return Err("Cannot start GDI+".into());
        }
        Result::Ok(Self(token, PhantomData))
    }
}
impl Drop for Gdi {
    fn drop(&mut self) {
        unsafe { GdiplusShutdown(self.0) }
    }
}

struct Transfer {
    linear: [f32; 256],
    encoded: [u8; 4097],
}
fn transfer() -> &'static Transfer {
    static TABLE: OnceLock<Transfer> = OnceLock::new();
    TABLE.get_or_init(|| Transfer {
        linear: std::array::from_fn(|i| {
            let v = i as f32 / 255.;
            if v <= 0.04045 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        }),
        encoded: std::array::from_fn(|i| {
            let v = i as f32 / 4096.;
            (255.
                * if v <= 0.0031308 {
                    12.92 * v
                } else {
                    1.055 * v.powf(1. / 2.4) - 0.055
                })
            .round() as u8
        }),
    })
}

/// Fixed-size glyphs stored consecutively, one byte of coverage per texel in row order.
pub struct BitmapFont<'a> {
    cell: [u16; 2],
    masks: &'a [u8],
}

impl<'a> BitmapFont<'a> {
    /// Rejects empty strikes and incomplete glyphs.
    pub fn new(cell: [u16; 2], masks: &'a [u8]) -> Option<Self> {
        let area = usize::from(cell[0]) * usize::from(cell[1]);
        (area > 0 && !masks.is_empty() && masks.len().is_multiple_of(area))
            .then_some(Self { cell, masks })
    }
}

/// Top-down, tightly packed BGRA raster. All drawing coordinates are in texels.
pub struct Canvas<'a> {
    pixels: &'a mut [u8],
    width: usize,
    height: usize,
    g: *mut GpGraphics,
    bitmap: *mut GpBitmap,
    _session: PhantomData<&'a Gdi>,
}
impl<'a> Canvas<'a> {
    pub fn new(_gdi: &'a Gdi, pixels: &'a mut [u8], [w, h]: [u16; 2]) -> Option<Self> {
        let (w, h) = (usize::from(w), usize::from(h));
        if w == 0 || h == 0 || pixels.len() != w.checked_mul(h)?.checked_mul(4)? {
            return None;
        }
        let mut bitmap = ptr::null_mut();
        let mut g = ptr::null_mut();
        // GDI+ borrows the fixed pixel allocation until the graphics and bitmap are dropped.
        unsafe {
            if GdipCreateBitmapFromScan0(
                w as _,
                h as _,
                (w * 4) as _,
                0x000e200b,
                pixels.as_mut_ptr(),
                &mut bitmap,
            ) != 0
            {
                return None;
            }
            if GdipGetImageGraphicsContext(bitmap.cast(), &mut g) != 0 {
                GdipDisposeImage(bitmap.cast());
                return None;
            }
            GdipSetSmoothingMode(g, SmoothingModeAntiAlias);
        }
        Some(Self {
            pixels,
            width: w,
            height: h,
            g,
            bitmap,
            _session: PhantomData,
        })
    }

    pub fn clear(&self, color: u32) {
        unsafe {
            GdipGraphicsClear(self.g, color);
        }
    }
    fn brush(&self, col: u32, draw: impl FnOnce(*mut GpBrush)) {
        unsafe {
            let mut b = ptr::null_mut();
            if GdipCreateSolidFill(col, &mut b) == 0 {
                draw(b.cast());
                GdipDeleteBrush(b.cast());
            }
        }
    }
    fn pen(&self, col: u32, weight: f32, draw: impl FnOnce(*mut GpPen)) {
        unsafe {
            let mut p = ptr::null_mut();
            if GdipCreatePen1(col, weight, UnitWorld, &mut p) == 0 {
                draw(p);
                GdipDeletePen(p);
            }
        }
    }
    pub fn rect(&self, x: f32, y: f32, w: f32, h: f32, col: u32) {
        if w <= 0. || h <= 0. {
            return;
        }
        let l = x.round() as i32;
        let t = y.round() as i32;
        self.brush(col, |b| unsafe {
            GdipSetSmoothingMode(self.g, SmoothingModeNone);
            GdipFillRectangleI(
                self.g,
                b,
                l,
                t,
                ((x + w).round() as i32).saturating_sub(l),
                ((y + h).round() as i32).saturating_sub(t),
            );
            GdipSetSmoothingMode(self.g, SmoothingModeAntiAlias);
        });
    }
    pub fn line(&self, a: [f32; 2], b: [f32; 2], col: u32, w: f32) {
        self.pen(col, w, |p| unsafe {
            GdipDrawLine(self.g, p, a[0], a[1], b[0], b[1]);
        });
    }
    pub fn arc(&self, center: [f32; 2], r: f32, start: f32, sweep: f32, col: u32, w: f32) {
        self.pen(col, w, |p| unsafe {
            GdipDrawArc(
                self.g,
                p,
                center[0] - r,
                center[1] - r,
                2. * r,
                2. * r,
                start,
                sweep,
            );
        });
    }
    pub fn tri(&self, pts: [[f32; 2]; 3], col: u32) {
        let pts = pts.map(|[x, y]| PointF { X: x, Y: y });
        self.brush(col, |b| unsafe {
            GdipFillPolygon(self.g, b, pts.as_ptr(), 3, FillModeAlternate);
        });
    }
    fn flush(&self) {
        unsafe {
            GdipFlush(self.g, FlushIntentionSync);
        }
    }

    /// Draw printable ASCII with space in slot zero. The origin is the upper-left texel.
    pub fn text(&mut self, origin: [i32; 2], text: &str, font: &BitmapFont<'_>, color: u32) {
        self.text_mapped(origin, text, font, color, |ch| {
            (' '..='~').contains(&ch).then(|| (ch as usize - 32, 0))
        });
    }

    /// Draw a fixed-cell run with a caller-defined (glyph slot, vertical offset) mapping.
    /// Missing glyphs advance one cell; coverage blends the RGB ink in linear light.
    /// The destination alpha is preserved, for drawing on opaque page backgrounds.
    pub fn text_mapped(
        &mut self,
        [left, top]: [i32; 2],
        text: &str,
        font: &BitmapFont<'_>,
        color: u32,
        glyph: impl Fn(char) -> Option<(usize, i32)>,
    ) {
        let [w, h] = font.cell.map(usize::from);
        let ink = color.to_le_bytes();
        self.flush();
        let table = transfer();
        let mut x0 = i64::from(left);
        for ch in text.chars() {
            if x0 >= self.width as i64 {
                break;
            }
            let origin = x0;
            x0 += w as i64;
            let Some((slot, offset)) = glyph(ch) else {
                continue;
            };
            if slot >= font.masks.len() / (w * h) || x0 <= 0 {
                continue;
            }
            let start = slot * w * h;
            for dy in 0..h {
                let y = i64::from(top) + i64::from(offset) + dy as i64;
                if !(0..self.height as i64).contains(&y) {
                    continue;
                }
                for dx in 0..w {
                    let x = origin + dx as i64;
                    let a = font.masks[start + dy * w + dx];
                    if a == 0 || !(0..self.width as i64).contains(&x) {
                        continue;
                    }
                    let p = 4 * (y as usize * self.width + x as usize);
                    for (k, src) in ink[..3].iter().enumerate() {
                        let dst = &mut self.pixels[p + k];
                        *dst = if a == 255 {
                            *src
                        } else {
                            let d = table.linear[*dst as usize];
                            table.encoded[((d
                                + (table.linear[*src as usize] - d) * (f32::from(a) / 255.))
                                * 4096.)
                                .round() as usize]
                        };
                    }
                }
            }
        }
    }

    /// Copy opaque grayscale artwork into the canvas, clipping at its right and bottom edges.
    pub fn grayscale(&mut self, [x, y]: [u16; 2], [w, h]: [u16; 2], bytes: &[u8]) -> bool {
        let (x, y, w, h) = (
            usize::from(x),
            usize::from(y),
            usize::from(w),
            usize::from(h),
        );
        if bytes.len() != w * h || w == 0 || h == 0 {
            return false;
        }
        self.flush();
        let count = w.min(self.width.saturating_sub(x));
        if count == 0 {
            return true;
        }
        for row in 0..h.min(self.height.saturating_sub(y)) {
            let start = 4 * ((y + row) * self.width + x);
            for (px, &v) in self.pixels[start..start + 4 * count]
                .as_chunks_mut::<4>()
                .0
                .iter_mut()
                .zip(&bytes[row * w..])
            {
                px.copy_from_slice(&[v, v, v, 255]);
            }
        }
        true
    }
}
impl Drop for Canvas<'_> {
    fn drop(&mut self) {
        self.flush();
        unsafe {
            GdipDeleteGraphics(self.g);
            GdipDisposeImage(self.bitmap.cast());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clipped_masks_and_artwork_preserve_the_remaining_texels() {
        let gdi = Gdi::start().unwrap();
        let mut pixels = [0; 24];
        {
            let mut c = Canvas::new(&gdi, &mut pixels, [3, 2]).unwrap();
            c.clear(0xff000000);
            let font = BitmapFont::new([2, 1], &[255, 128]).unwrap();
            c.text_mapped([-1, 0], "xx", &font, 0xffffffff, |_| Some((0, 0)));
            assert!(!c.grayscale([2, 1], [2, 2], &[5, 6, 7]));
            assert!(c.grayscale([2, 1], [2, 2], &[5, 6, 7, 8]));
        }
        assert_eq!(
            pixels,
            [
                188, 188, 188, 255, 255, 255, 255, 255, 188, 188, 188, 255, 0, 0, 0, 255, 0, 0, 0,
                255, 5, 5, 5, 255
            ]
        );
        assert!(BitmapFont::new([0, 1], &[1]).is_none());
    }
}
