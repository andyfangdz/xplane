use crate::{DISPLAY_SIZE, texture::Texture};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layer {
    Background,
    Indications,
}

/// A native-size page or rectangular overlay. CPU rendering also works outside X-Plane.
pub struct Page {
    pub(crate) size: [u16; 2],
    background_color: u32,
    interval: Duration,
    last: Option<Instant>,
    background: Vec<u8>,
    pub(crate) frame: Vec<u8>,
    pub(crate) dirty: bool,
    pub(crate) texture: Option<Texture>,
}

impl Page {
    /// The size must fit on the native display. A zero interval refreshes every draw.
    pub fn new(size: [u16; 2], background_color: u32, interval: Duration) -> Option<Self> {
        let [w, h] = size;
        if w == 0 || h == 0 || w > DISPLAY_SIZE[0] || h > DISPLAY_SIZE[1] {
            return None;
        }
        Some(Self {
            size,
            background_color,
            interval,
            last: None,
            background: Vec::new(),
            frame: Vec::new(),
            texture: None,
            dirty: false,
        })
    }

    /// Repaint both layers on the next update, even inside the refresh interval.
    pub fn invalidate(&mut self) {
        self.last = None;
    }

    /// Most recent completed frame, in BGRA scanline order.
    pub fn pixels(&self) -> Option<&[u8]> {
        self.last.map(|_| self.frame.as_slice())
    }

    pub(crate) fn update_pixels(
        &mut self,
        now: Instant,
        mut paint: impl FnMut(&mut [u8], Layer) -> bool,
    ) -> bool {
        if self
            .last
            .is_some_and(|last| now.duration_since(last) < self.interval)
        {
            return true;
        }
        let len = usize::from(self.size[0]) * usize::from(self.size[1]) * 4;
        if self.last.take().is_none() {
            self.background.resize(len, 0);
            self.frame.resize(len, 0);
            self.background
                .as_chunks_mut::<4>()
                .0
                .fill(self.background_color.to_le_bytes());
            if !paint(&mut self.background, Layer::Background) {
                return false;
            }
        }
        self.frame.copy_from_slice(&self.background);
        if !paint(&mut self.frame, Layer::Indications) {
            return false;
        }
        self.last = Some(now);
        self.dirty = true;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn repaint_starts_from_background_and_invalidation_bypasses_throttle() {
        let time = Instant::now();
        let rate = Duration::from_secs(1);
        let mut page = Page::new([2, 1], 0xff112233, rate).unwrap();
        let mut calls = vec![];
        assert!(page.pixels().is_none());
        assert!(page.update_pixels(time, |pixels, l| {
            calls.push(l);
            if l == Layer::Indications {
                pixels[..4].copy_from_slice(&0xffabcdef_u32.to_le_bytes());
            }
            true
        }));
        assert_eq!(calls, [Layer::Background, Layer::Indications]);
        assert_eq!(
            page.pixels().unwrap(),
            &[0xef, 0xcd, 0xab, 255, 0x33, 0x22, 0x11, 255]
        );
        assert!(page.update_pixels(time, |_, _| panic!("unexpected paint")));
        assert!(page.update_pixels(time + rate, |_, layer| {
            assert_eq!(layer, Layer::Indications);
            true
        }));
        assert_eq!(&page.pixels().unwrap()[..4], &[0x33, 0x22, 0x11, 255]);
        page.invalidate();
        assert!(page.pixels().is_none());
        let mut calls = vec![];
        assert!(page.update_pixels(time + rate, |_, layer| {
            calls.push(layer);
            true
        }));
        assert_eq!(calls, [Layer::Background, Layer::Indications]);
    }
}
