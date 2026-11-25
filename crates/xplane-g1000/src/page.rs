use crate::{Canvas, DISPLAY_SIZE, Gdi, texture::Texture};
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

    /// Paint when due. Returns true when a complete frame is available, including cache hits.
    pub fn update(&mut self, gdi: &Gdi, paint: impl FnMut(&mut Canvas<'_>, Layer)) -> bool {
        self.update_at(Instant::now(), gdi, paint)
    }

    fn update_at(
        &mut self,
        now: Instant,
        gdi: &Gdi,
        mut paint: impl FnMut(&mut Canvas<'_>, Layer),
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
            let Some(mut canvas) = Canvas::new(gdi, &mut self.background, self.size) else {
                return false;
            };
            canvas.clear(self.background_color);
            paint(&mut canvas, Layer::Background);
        }
        self.frame.copy_from_slice(&self.background);
        {
            let Some(mut canvas) = Canvas::new(gdi, &mut self.frame, self.size) else {
                return false;
            };
            paint(&mut canvas, Layer::Indications);
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
        let gdi = Gdi::start().unwrap();
        let mut calls = vec![];
        assert!(page.pixels().is_none());
        assert!(page.update_at(time, &gdi, |c, l| {
            calls.push(l);
            if l == Layer::Indications {
                c.rect(0., 0., 1., 1., 0xffabcdef);
            }
        }));
        assert_eq!(calls, [Layer::Background, Layer::Indications]);
        assert_eq!(
            page.pixels().unwrap(),
            &[0xef, 0xcd, 0xab, 255, 0x33, 0x22, 0x11, 255]
        );
        assert!(page.update_at(time, &gdi, |_, _| panic!("unexpected paint")));
        assert!(page.update_at(time + rate, &gdi, |_, l| assert_eq!(l, Layer::Indications)));
        assert_eq!(&page.pixels().unwrap()[..4], &[0x33, 0x22, 0x11, 255]);
        page.invalidate();
        assert!(page.pixels().is_none());
        let mut calls = vec![];
        assert!(page.update_at(time + rate, &gdi, |_, l| calls.push(l)));
        assert_eq!(calls, [Layer::Background, Layer::Indications]);
    }
}
