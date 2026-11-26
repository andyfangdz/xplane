use super::InstrumentPainter;
use crate::Layer;

/// Instrument units mapped to display coordinates. Reversed positions produce an upward scale.
#[derive(Clone, Copy)]
pub struct LinearScale {
    pub range: [f32; 2],
    pub positions: [f32; 2],
}
impl LinearScale {
    fn valid(&self) -> bool {
        let span = self.range[1] - self.range[0];
        span.is_finite() && span > 0. && self.positions.iter().all(|p| p.is_finite())
    }
    pub fn fraction(&self, value: f32) -> f32 {
        ((value - self.range[0]) / (self.range[1] - self.range[0])).clamp(0., 1.)
    }
    pub fn position(&self, value: f32) -> f32 {
        self.positions[0] + (self.positions[1] - self.positions[0]) * self.fraction(value)
    }
}

/// Triangle on a gauge. The base can lie on either side of the tip.
pub struct GaugePointer {
    pub x: f32,
    pub tip: f32,
    pub base: f32,
    pub half_width: f32,
}
impl GaugePointer {
    pub fn vertices(&self) -> [[f32; 2]; 3] {
        [
            [self.x - self.half_width, self.base],
            [self.x + self.half_width, self.base],
            [self.x, self.tip],
        ]
    }
}

/// Horizontal operating bands, end stops, and a live pointer.
pub struct BarGauge<'a> {
    pub scale: LinearScale,
    pub bands: &'a [(f32, f32, u32)],
    pub frame_color: u32,
}
impl BarGauge<'_> {
    /// Returns the moving pointer's x coordinate for an optional cylinder label.
    pub fn paint_at(
        &self,
        draw: &InstrumentPainter,
        y: f32,
        layer: Layer,
        value: Option<(f32, u32)>,
    ) -> Option<f32> {
        if !self.scale.valid() || !y.is_finite() {
            return None;
        }
        let [left, right] = self.scale.positions;
        if layer == Layer::Background {
            for &(lo, hi, color) in self.bands {
                if hi > lo {
                    let a = self.scale.position(lo);
                    let b = self.scale.position(hi);
                    draw.rect(a.min(b), y, (b - a).abs(), 3., color);
                }
            }
            draw.rects(
                [
                    [left - 1., y - 8., 2., 15.],
                    [right - 1., y - 8., 2., 15.],
                    [left.min(right) - 1., y + 5., (right - left).abs() + 2., 2.],
                ],
                self.frame_color,
            );
            return None;
        }
        let (value, color) = value.filter(|(v, _)| v.is_finite())?;
        let x = self.scale.position(value);
        draw.tri(
            GaugePointer {
                x,
                tip: y + 2.,
                base: y - 14.,
                half_width: 11.,
            }
            .vertices(),
            color,
        );
        Some(x)
    }
}

/// Equally spaced segments of a vertical gauge. Negative pitch grows upward.
pub struct ColumnStack {
    pub first: [f32; 2],
    pub size: [f32; 2],
    pub pitch: f32,
}
impl ColumnStack {
    pub fn rectangles(&self, count: usize) -> impl Iterator<Item = [f32; 4]> + '_ {
        (0..count).map(|i| {
            [
                self.first[0],
                self.first[1] + self.pitch * i as f32,
                self.size[0],
                self.size[1],
            ]
        })
    }
}

/// Continuous cylinder indication with a dotted leader and optional lean-peak marker.
pub struct TemperatureColumn {
    pub x: f32,
    pub scale: LinearScale,
    pub leader_top: f32,
    pub leader_color: u32,
    pub width: f32,
}
impl TemperatureColumn {
    pub fn paint(
        &self,
        draw: &InstrumentPainter,
        value: f32,
        color: u32,
        peak: Option<(f32, u32)>,
    ) {
        if !value.is_finite() || !self.scale.valid() || !self.leader_top.is_finite() {
            return;
        }
        let y = self.scale.position(value);
        let leader = ColumnStack {
            first: [self.x, self.leader_top],
            size: [1., 2.],
            pitch: 7.,
        };
        let clip = draw.painter.clip_rect();
        let clip_top = clip.top() - draw.origin.y;
        let clip_bottom = clip.bottom() - draw.origin.y;
        let first = ((clip_top - self.leader_top - leader.size[1] - 0.5) / leader.pitch)
            .ceil()
            .max(0.) as usize;
        let end = (((y - 2.).min(clip_bottom + 0.5) - self.leader_top) / leader.pitch)
            .ceil()
            .max(0.) as usize;
        draw.rects(leader.rectangles(end).skip(first), self.leader_color);
        draw.rect(
            self.x - self.width / 2.,
            y,
            self.width,
            self.scale.positions[0] - y,
            color,
        );
        if let Some((peak, ink)) = peak.filter(|(v, _)| v.is_finite()) {
            draw.rect(
                self.x - self.width / 2. - 1.,
                self.scale.position(peak) - 2.,
                self.width + 2.,
                3.,
                ink,
            );
        }
    }
}
