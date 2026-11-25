//! Layout in device texels for raster strikes and textured glyph quads.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextAlign {
    Left,
    Center,
    Right,
}
impl TextAlign {
    /// Left edge of a run anchored at `x`, before texel snapping.
    pub fn left(self, x: f32, width: f32) -> f32 {
        match self {
            Self::Left => x,
            Self::Center => x - width * 0.5,
            Self::Right => x - width,
        }
    }

    /// Center each glyph's bitmap cell in its advance and snap its left edge to a texel.
    /// A narrow punctuation advance can be smaller than the bitmap cell.
    pub fn place<T: Copy>(
        self,
        x: f32,
        cell_width: f32,
        glyphs: impl Iterator<Item = T> + Clone,
        advance: impl Fn(T) -> f32,
        mut visit: impl FnMut(T, f32),
    ) {
        let mut pen = match self {
            Self::Left => x,
            _ => self.left(x, glyphs.clone().map(&advance).sum()),
        };
        for glyph in glyphs {
            let step = advance(glyph);
            visit(glyph, (pen + (step - cell_width) * 0.5).round());
            pen += step;
        }
    }
}

/// One fixed-cell strike packed across rows of a texture atlas. Consecutive strikes can
/// start at `next_row()` and use different cell sizes.
pub struct GlyphGrid {
    cell: [u16; 2],
    columns: u16,
    top: u16,
    count: u16,
    next: u16,
}
impl GlyphGrid {
    pub fn new(atlas_width: u16, top: u16, cell: [u16; 2], count: u16) -> Option<Self> {
        let columns = atlas_width.checked_div(cell[0])?;
        if columns == 0 || cell[1] == 0 || count == 0 {
            return None;
        }
        let next = top.checked_add(count.div_ceil(columns).checked_mul(cell[1])?)?;
        Some(Self {
            cell,
            columns,
            top,
            count,
            next,
        })
    }

    /// Upper-left texel of the glyph's mask.
    pub fn origin(&self, slot: u16) -> Option<[u16; 2]> {
        (slot < self.count).then(|| {
            [
                slot % self.columns * self.cell[0],
                self.top + slot / self.columns * self.cell[1],
            ]
        })
    }

    pub fn next_row(&self) -> u16 {
        self.next
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn packs_strikes_with_different_metrics() {
        let first = GlyphGrid::new(11, 4, [3, 5], 7).unwrap();
        assert_eq!(first.origin(2), Some([6, 4]));
        assert_eq!(first.origin(3), Some([0, 9]));
        assert_eq!(first.origin(6), Some([0, 14]));
        assert_eq!(first.origin(7), None);
        let second = GlyphGrid::new(11, first.next_row(), [5, 7], 4).unwrap();
        assert_eq!(second.origin(3), Some([5, 26]));
        assert_eq!(second.next_row(), 33);
        assert!(GlyphGrid::new(2, 0, [3, 5], 7).is_none());
        assert!(GlyphGrid::new(5, u16::MAX, [3, 5], 1).is_none());
    }
    #[test]
    fn places_fractional_advances_before_snapping() {
        for (align, want) in [
            (TextAlign::Left, [10., 14., 16.]),
            (TextAlign::Center, [5., 8., 11.]),
            (TextAlign::Right, [0., 3., 6.]),
        ] {
            let mut placed = vec![];
            align.place(
                10.25,
                5.,
                [5., 1.5, 4.].into_iter(),
                |step| step,
                |_, x| placed.push(x),
            );
            assert_eq!(placed, want);
        }
    }
}
