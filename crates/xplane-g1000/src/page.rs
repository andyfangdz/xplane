use crate::{DISPLAY_SIZE, panel::Batch, widgets::raster};
use std::cell::OnceCell;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layer {
    Background,
    Indications,
}

/// A native-size page or rectangular overlay. CPU rendering also works outside X-Plane.
pub struct Page {
    pub(crate) size: [u16; 2],
    pub(crate) background_color: u32,
    interval: Duration,
    last: Option<Instant>,
    pub(crate) background: Vec<Batch>,
    pub(crate) recorded: std::collections::VecDeque<crate::panel::Background>,
    pub(crate) indications: Vec<Batch>,
    pixels: OnceCell<Vec<u8>>,
    pub(crate) fallback: OnceCell<std::rc::Rc<crate::panel::Atlas>>,
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
            recorded: Default::default(),
            indications: Vec::new(),
            pixels: OnceCell::new(),
            fallback: OnceCell::new(),
        })
    }

    /// Repaint both layers on the next update, even inside the refresh interval.
    pub fn invalidate(&mut self) {
        self.recorded.clear();
        self.last = None;
    }

    /// Rasterize the current page on demand for a native-size, top-down BGRA readback.
    pub fn pixels(&self) -> Option<&[u8]> {
        self.last?;
        Some(self.pixels.get_or_init(|| {
            let mut pixels = vec![0; usize::from(self.size[0]) * usize::from(self.size[1]) * 4];
            raster::paint(self.size, &self.background, &mut pixels);
            raster::paint(self.size, &self.indications, &mut pixels);
            pixels
        }))
    }

    pub(crate) fn ready(&self) -> bool {
        self.last.is_some()
    }

    pub(crate) fn update_layers(
        &mut self,
        now: Instant,
        mut prepare: impl FnMut(Layer) -> Option<Vec<Batch>>,
    ) -> bool {
        if self
            .last
            .is_some_and(|last| now.duration_since(last) < self.interval)
        {
            return true;
        }
        self.pixels.take();
        self.fallback.take();
        if self.last.take().is_none() {
            let Some(background) = prepare(Layer::Background) else {
                return false;
            };
            self.recorded.clear();
            self.background = background;
        }
        let Some(indications) = prepare(Layer::Indications) else {
            return false;
        };
        self.indications = indications;
        self.last = Some(now);
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
        assert!(page.update_layers(time, |layer| {
            calls.push(layer);
            Some(vec![])
        }));
        assert_eq!(calls, [Layer::Background, Layer::Indications]);
        assert!(page.update_layers(time + rate / 2, |_| panic!("unexpected paint")));
        assert!(page.update_layers(time + rate, |layer| {
            assert_eq!(layer, Layer::Indications);
            Some(vec![])
        }));
        page.invalidate();
        assert!(page.pixels().is_none());
        assert!(!page.update_layers(time + rate, |_| None));
        let mut calls = vec![];
        assert!(page.update_layers(time + rate, |layer| {
            calls.push(layer);
            Some(vec![])
        }));
        assert_eq!(calls, [Layer::Background, Layer::Indications]);
    }
}
