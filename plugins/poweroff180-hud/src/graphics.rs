//! GDI builds a cached Arial alpha atlas; all rendering occurs inside X-Plane's
//! OpenGL draw callback. Strokes are quads because the Vulkan bridge limits
//! glLineWidth. Attribute, matrix, clipping and texture upload state is restored.
use crate::scene::{Color, Draw, Scene};
use poweroff180::{
    guidance::{rad, PI},
    hud::{point, Point},
};
use windows_sys::Win32::{
    Foundation::SIZE,
    Graphics::{Gdi::*, OpenGL::*},
};
use xplane_sdk_sys::{XPLMBindTexture2d, XPLMGenerateTextureNumbers, XPLMSetGraphicsState};

pub struct Graphics {
    texture: i32,
    advance: [f32; 96],
}
impl Default for Graphics {
    fn default() -> Self {
        Self {
            texture: 0,
            advance: [0.0; 96],
        }
    }
}
impl Graphics {
    pub fn ready(&self) -> bool {
        self.texture != 0
    }
    fn make_font(&mut self) -> bool {
        // SAFETY: all GDI objects are local, checked after creation, restored to
        // the memory DC before deletion, and pixels are read only while the DIB
        // is live. X-Plane's draw callback supplies the current GL context.
        unsafe {
            let dc = CreateCompatibleDC(std::ptr::null_mut());
            if dc.is_null() {
                return false;
            }
            let mut info: BITMAPINFO = std::mem::zeroed();
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
            let mut unpack = 4;
            glGetIntegerv(GL_UNPACK_ALIGNMENT, &mut unpack);
            glPixelStorei(GL_UNPACK_ALIGNMENT, 1);
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
            glPixelStorei(GL_UNPACK_ALIGNMENT, unpack);
        }
        self.ready()
    }
    pub fn paint(&mut self, scene: &Scene, size: (i32, i32)) -> bool {
        let (w, h) = size;
        if w <= 0 || h <= 0 {
            return false;
        }
        // SAFETY: the SDK draw callback owns the active compatibility context.
        // Every push/pop is paired, including nested scene clipping. Scene data
        // contains owned values and no client pointers escape this call.
        unsafe { XPLMSetGraphicsState(0, 1, 0, 0, 1, 0, 0) };
        if !self.ready() && !self.make_font() {
            return false;
        }
        unsafe {
            XPLMSetGraphicsState(0, 0, 0, 0, 1, 0, 0);
            glPushAttrib(GL_LINE_BIT | GL_CURRENT_BIT | GL_SCISSOR_BIT);
            glDisable(GL_SCISSOR_TEST);
            let mut old_mode = GL_MODELVIEW as i32;
            glGetIntegerv(GL_MATRIX_MODE, &mut old_mode);
            glMatrixMode(GL_MODELVIEW);
            glPushMatrix();
            glTranslated(0.0, f64::from(h), 0.0);
            glScaled(f64::from(w) / 1920.0, -f64::from(h) / 1080.0, 1.0);
            let mut viewport = [0; 4];
            glGetIntegerv(GL_VIEWPORT, viewport.as_mut_ptr());
            for command in &scene.commands {
                match command {
                    Draw::Line(a, b, c, width) => {
                        color(*c);
                        glBegin(GL_QUADS);
                        stroke(*a, *b, *width);
                        glEnd();
                    }
                    Draw::Polygon(points, c, fill, width) => {
                        color(*c);
                        glBegin(if *fill { GL_POLYGON } else { GL_QUADS });
                        if *fill {
                            for p in points {
                                glVertex2d(p.x, p.y);
                            }
                        } else if let Some(mut previous) = points.last().copied() {
                            for p in points {
                                stroke(previous, *p, *width);
                                previous = *p;
                            }
                        }
                        glEnd();
                    }
                    Draw::Circle(center, r, c, width, fill) => {
                        color(*c);
                        glBegin(if *fill { GL_POLYGON } else { GL_QUADS });
                        let mut previous = point(center.x + r, center.y);
                        for i in 1..=48 {
                            let a = f64::from(i) * 2.0 * PI / 48.0;
                            let p = point(center.x + r * a.cos(), center.y + r * a.sin());
                            if *fill {
                                glVertex2d(p.x, p.y);
                            } else {
                                stroke(previous, p, *width);
                            }
                            previous = p;
                        }
                        glEnd();
                    }
                    Draw::Arc(center, r, from, to, c, width) => {
                        color(*c);
                        glBegin(GL_QUADS);
                        let count = (((to - from).abs() / 3.0) as i32).max(2);
                        let mut previous = point(
                            center.x + r * rad(*from).cos(),
                            center.y + r * rad(*from).sin(),
                        );
                        for i in 1..=count {
                            let a = rad(from + (to - from) * f64::from(i) / f64::from(count));
                            let p = point(center.x + r * a.cos(), center.y + r * a.sin());
                            stroke(previous, p, *width);
                            previous = p;
                        }
                        glEnd();
                    }
                    Draw::Text {
                        at,
                        text,
                        size,
                        color,
                        anchor,
                        rotation,
                    } => {
                        if let Some(angle) = rotation {
                            glPushMatrix();
                            glTranslated(at.x, at.y, 0.0);
                            glRotated(*angle, 0.0, 0.0, 1.0);
                            self.text(0.0, -14.0, text, *size, *color, *anchor);
                            glPopMatrix();
                        } else {
                            self.text(at.x, at.y, text, *size, *color, *anchor);
                        }
                    }
                    Draw::Clip { x, y, w, h } => {
                        glPushAttrib(GL_SCISSOR_BIT);
                        glEnable(GL_SCISSOR_TEST);
                        glScissor(
                            viewport[0] + (x * f64::from(viewport[2]) / 1920.0) as i32,
                            viewport[1]
                                + ((1080.0 - y - h) * f64::from(viewport[3]) / 1080.0) as i32,
                            (w * f64::from(viewport[2]) / 1920.0) as i32,
                            (h * f64::from(viewport[3]) / 1080.0) as i32,
                        );
                    }
                    Draw::Unclip => glPopAttrib(),
                }
            }
            glPopMatrix();
            glMatrixMode(old_mode as u32);
            glPopAttrib();
        }
        true
    }
    fn text(&self, x: f64, y: f64, text: &str, size: f64, c: Color, anchor: u8) {
        let index = |ch: u8| {
            if (32..128).contains(&ch) {
                usize::from(ch - 32)
            } else {
                31
            }
        };
        let width: f64 = text
            .bytes()
            .map(|ch| f64::from(self.advance[index(ch)]) * size / 44.0)
            .sum();
        let scale = size / 44.0;
        let start = x - width * f64::from(anchor) * 0.5;
        // SAFETY: called only inside paint's active context. Atlas is live,
        // characters are bounds checked and immediate vertices copy all values.
        unsafe {
            XPLMSetGraphicsState(0, 1, 0, 0, 1, 0, 0);
            XPLMBindTexture2d(self.texture, 0);
            for pass in 0..2 {
                let offset = if pass == 0 { 1.2 } else { 0.0 };
                let mut x = start + offset;
                color(if pass == 0 { [0.0, 0.0, 0.0, 0.8] } else { c });
                glBegin(GL_QUADS);
                for ch in text.bytes() {
                    let i = index(ch);
                    let u = (i % 16) as f64 * 64.0 / 1024.0;
                    let v = (i / 16) as f64 * 64.0 / 384.0;
                    let l = x - 2.0 * scale;
                    let t = y - 2.0 * scale + offset;
                    let r = l + 64.0 * scale;
                    let b = t + 64.0 * scale;
                    glTexCoord2d(u, v);
                    glVertex2d(l, t);
                    glTexCoord2d(u + 64.0 / 1024.0, v);
                    glVertex2d(r, t);
                    glTexCoord2d(u + 64.0 / 1024.0, v + 64.0 / 384.0);
                    glVertex2d(r, b);
                    glTexCoord2d(u, v + 64.0 / 384.0);
                    glVertex2d(l, b);
                    x += f64::from(self.advance[i]) * scale;
                }
                glEnd();
            }
            XPLMSetGraphicsState(0, 0, 0, 0, 1, 0, 0);
        }
    }
}
impl Drop for Graphics {
    fn drop(&mut self) {
        if self.texture != 0 {
            // SAFETY: X-Plane unloads this renderer on its plugin thread with the same
            // context lifetime as the native HUD. The texture is uniquely owned.
            unsafe { glDeleteTextures(1, &(self.texture as u32)) };
        }
    }
}
unsafe fn color(c: Color) {
    // SAFETY: callers are inside the active draw callback.
    unsafe { glColor4f(c[0], c[1], c[2], c[3]) };
}
unsafe fn stroke(a: Point, b: Point, width: f64) {
    let length = (b.x - a.x).hypot(b.y - a.y);
    if length < 1e-9 {
        return;
    }
    let x = -(b.y - a.y) * width / (2.0 * length);
    let y = (b.x - a.x) * width / (2.0 * length);
    // SAFETY: caller opened GL_QUADS in the active drawing context.
    unsafe {
        glVertex2d(a.x + x, a.y + y);
        glVertex2d(b.x + x, b.y + y);
        glVertex2d(b.x - x, b.y - y);
        glVertex2d(a.x - x, a.y - y);
    }
}
