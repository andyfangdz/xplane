//! Native G1000 text metrics in device texels, independent of desktop DPI.
use ab_glyph::{Font, FontRef, point};
use std::{fs, io, path::Path};

/// Relative to the simulator's system directory.
pub const PROFONT_PATH: &str = "Resources/fonts/ProFontWindows.ttf";

/// Captured EIS labels, numeric readouts/page headings, and large dial values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FontSize {
    Small,
    Medium,
    Large,
}
impl FontSize {
    pub const ALL: [Self; 3] = [Self::Small, Self::Medium, Self::Large];

    /// The bitmap width is also the advance, including for spaces, periods and degrees.
    pub const fn cell(self) -> [u16; 2] {
        match self {
            Self::Small => [8, 18],
            Self::Medium => [11, 24],
            Self::Large => [14, 30],
        }
    }

    /// Distance from the first bitmap row to the typographic baseline.
    pub const fn baseline(self) -> u16 {
        match self {
            Self::Small => 13,
            Self::Medium => 17,
            Self::Large => 22,
        }
    }
}

/// One top-down coverage mask per printable ASCII glyph, plus the degree, delta and underlined temperature symbols.
pub struct FontStrike {
    size: FontSize,
    masks: Vec<u8>,
}
impl FontStrike {
    pub const COUNT: usize = 98;

    pub fn characters() -> impl Iterator<Item = char> {
        (' '..='~').chain(['°', 'Δ', 'º'])
    }

    pub fn slot(ch: char) -> Option<usize> {
        match ch {
            ' '..='~' => Some(ch as usize - 32),
            '°' => Some(95),
            'Δ' => Some(96),
            'º' => Some(97),
            _ => None,
        }
    }

    pub fn glyph(&self, ch: char) -> Option<&[u8]> {
        let slot = Self::slot(ch)?;
        let [w, h] = self.size.cell().map(usize::from);
        let area = w * h;
        Some(&self.masks[slot * area..(slot + 1) * area])
    }

    fn rasterize(font: &FontRef<'_>, size: FontSize) -> Self {
        let [w, h] = size.cell().map(usize::from);
        let mut masks = vec![0; Self::COUNT * w * h];
        let natural = font.h_advance_unscaled(font.glyph_id('0'));
        let scale = w as f32 * font.height_unscaled() / natural;
        let mut coverage = vec![0u16; (w + 2) * (h + 2)];
        for (ch, cell) in Self::characters().zip(masks.chunks_exact_mut(w * h)) {
            let id = font.glyph_id(ch);
            let advance = font.h_advance_unscaled(id) * w as f32 / natural;
            let origin = point(1.0 + (w as f32 - advance) / 2., f32::from(size.baseline()));
            let Some(outline) = font.outline_glyph(id.with_scale_and_position(scale, origin))
            else {
                continue;
            };
            let bounds = outline.px_bounds().min;
            coverage.fill(0);
            outline.draw(|x, y, alpha| {
                let x = bounds.x as i32 + x as i32;
                let y = bounds.y as i32 + y as i32;
                if (0..w as i32).contains(&x) && (0..h as i32).contains(&y) {
                    coverage[(y as usize + 1) * (w + 2) + x as usize + 1] =
                        u16::from((alpha * 255.).round() as u8);
                }
            });
            // Approximate the native texture's subpixel sampling in linear coverage.
            // The left edge and the adjacent scanlines carry ink even on grid-aligned stems.
            let stride = w + 2;
            for (row, out) in cell.chunks_exact_mut(w).enumerate() {
                for (x, pixel) in out.iter_mut().enumerate() {
                    let i = (row + 1) * stride + x + 1;
                    *pixel = ((12 * coverage[i]
                        + 2 * coverage[i + 1]
                        + coverage[i - stride]
                        + coverage[i + stride]
                        + 8)
                        / 16) as u8;
                }
            }
        }
        Self { size, masks }
    }
}

/// Cached strikes built once from the owner's installed TrueType file.
pub struct ProFont([FontStrike; 3]);
impl ProFont {
    pub fn load(system_directory: &Path) -> io::Result<Self> {
        let path = system_directory.join(PROFONT_PATH);
        let data = fs::read(&path)
            .map_err(|e| io::Error::new(e.kind(), format!("{}: {e}", path.display())))?;
        let font = FontRef::try_from_slice(&data).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{}: {e}", path.display()),
            )
        })?;
        let zero = font.glyph_id('0');
        if zero.0 == 0 || font.h_advance_unscaled(zero) <= 0. || font.height_unscaled() <= 0. {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{} has no usable digit metrics", path.display()),
            ));
        }
        Ok(Self(
            FontSize::ALL.map(|size| FontStrike::rasterize(&font, size)),
        ))
    }

    pub fn strike(&self, size: FontSize) -> &FontStrike {
        &self.0[size as usize]
    }
}

#[cfg(windows)]
impl FontStrike {
    /// Draw a run anchored at a baseline in top-down device coordinates.
    pub fn draw(
        &self,
        canvas: &mut crate::Canvas<'_>,
        x: f32,
        baseline: i32,
        align: crate::TextAlign,
        text: &str,
        color: u32,
    ) {
        let cell = self.size.cell();
        let left = align
            .left(x, text.chars().count() as f32 * f32::from(cell[0]))
            .round() as i32;
        let top = baseline.saturating_sub(i32::from(self.size.baseline()));
        let font = crate::BitmapFont::new(cell, &self.masks).unwrap();
        canvas.text_mapped([left, top], text, &font, color, |ch| {
            Self::slot(ch).map(|slot| (slot, 0))
        });
    }
}
