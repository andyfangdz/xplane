use super::InstrumentPainter;
use crate::Layer;
use egui::{Pos2, Response, Sense, Ui, Vec2, Widget};

/// Radial geometry shared by scale ticks, label positions and dial needles.
pub struct DialRay {
    pub center: [f32; 2],
    pub direction: [f32; 2],
}
impl DialRay {
    pub fn clockwise(center: [f32; 2], degrees: f32) -> Self {
        let (sin, cos) = degrees.to_radians().sin_cos();
        Self {
            center,
            direction: [cos, sin],
        }
    }
    pub fn point(&self, radius: f32) -> [f32; 2] {
        [
            self.center[0] + radius * self.direction[0],
            self.center[1] + radius * self.direction[1],
        ]
    }
    pub fn tick(&self, outer: f32, inner: f32) -> [[f32; 2]; 2] {
        [self.point(outer), self.point(inner)]
    }
    /// Stem and diamond outline of a hollow needle, in drawing order.
    pub fn outlined_needle(
        &self,
        root: f32,
        stem: f32,
        shoulder: f32,
        tip: f32,
        half_width: f32,
    ) -> [[[f32; 2]; 2]; 5] {
        let [x, y] = self.point(shoulder);
        let [dx, dy] = self.direction;
        let a = self.point(root);
        let b = self.point(stem);
        let c = [x - half_width * dy, y + half_width * dx];
        let d = self.point(tip);
        let e = [x + half_width * dy, y - half_width * dx];
        [[a, b], [b, c], [c, d], [d, e], [e, b]]
    }
}

/// A G1000 radial scale with a live needle. Values and band limits are in instrument units.
/// Angles start at three o'clock and increase clockwise. Distances are display texels.
pub struct RadialGauge<'a> {
    pub radius: f32,
    pub range: [f32; 2],
    pub start_degrees: f32,
    pub sweep_degrees: f32,
    /// Ordered (low, high, ARGB) operating ranges.
    pub bands: &'a [(f32, f32, u32)],
    /// Equal intervals with a major tick every fifth interval. Zero omits graduations.
    pub tick_intervals: u16,
    /// Distance between the tip and the radius of the colored bands.
    pub needle_inset: f32,
    /// Missing and nonfinite readings suppress the needle. Finite readings stop at the ends.
    pub value: Option<f32>,
    pub layer: Layer,
}

/// Graduation values and their distances from the dial center, in texels.
pub struct DialTicks<'a> {
    pub values: &'a [f32],
    pub outer: f32,
    pub inner: f32,
    pub color: u32,
    pub width: f32,
}

impl RadialGauge<'_> {
    fn fraction(&self, value: f32) -> f32 {
        ((value - self.range[0]) / (self.range[1] - self.range[0])).clamp(0., 1.)
    }

    /// Position a calibrated scale label on the same arc as the needle.
    pub fn point_at(&self, center: [f32; 2], radius: f32, value: f32) -> [f32; 2] {
        DialRay::clockwise(
            center,
            self.start_degrees + self.sweep_degrees * self.fraction(value),
        )
        .point(radius)
    }

    fn valid(&self) -> bool {
        let span = self.range[1] - self.range[0];
        span > 0.
            && span.is_finite()
            && self.radius > 0.
            && self.radius.is_finite()
            && self.start_degrees.is_finite()
            && self.sweep_degrees.is_finite()
            && self.sweep_degrees != 0.
    }
    fn needle(&self, draw: &InstrumentPainter, center: [f32; 2], color: u32) {
        if let Some(value) = self.value.filter(|v| v.is_finite()) {
            let length = self.radius - self.needle_inset;
            if length > 0. && length <= self.radius {
                draw.line(center, self.point_at(center, length, value), color, 3.);
            }
        }
    }
    fn rim(&self, draw: &InstrumentPainter, center: [f32; 2]) {
        draw.arc(
            center,
            self.radius + 2.5,
            self.start_degrees,
            self.sweep_degrees,
            u32::MAX,
            1.2,
        );
    }
    /// Draw individually calibrated marks, including gaps in the operating bands.
    /// The marks are drawn over the bands in the supplied order.
    pub fn paint_marked(
        &self,
        draw: &InstrumentPainter,
        center: [f32; 2],
        marks: &[DialTicks<'_>],
        needle_color: u32,
    ) {
        if !self.valid() {
            return;
        }
        if self.layer == Layer::Indications {
            self.needle(draw, center, needle_color);
            return;
        }
        let angle = |v| self.start_degrees + self.sweep_degrees * self.fraction(v);
        self.rim(draw, center);
        for &(lo, hi, ink) in self.bands {
            if hi > lo {
                draw.arc(
                    center,
                    self.radius,
                    angle(lo),
                    angle(hi) - angle(lo),
                    ink,
                    3.,
                );
            }
        }
        for tick in marks {
            draw.segments(
                tick.values
                    .iter()
                    .map(|&v| DialRay::clockwise(center, angle(v)).tick(tick.outer, tick.inner)),
                tick.color,
                tick.width,
            );
        }
    }
    /// Place the gauge at a measured center in a fixed instrument layout.
    pub fn paint_at(&self, draw: &InstrumentPainter, center: [f32; 2]) {
        if !self.valid() {
            return;
        }
        if self.layer == Layer::Indications {
            self.needle(draw, center, u32::MAX);
            return;
        }

        for &(lo, hi, color) in self.bands {
            let (lo, hi) = (self.fraction(lo), self.fraction(hi));
            if hi > lo {
                draw.arc(
                    center,
                    self.radius,
                    self.start_degrees + lo * self.sweep_degrees,
                    (hi - lo) * self.sweep_degrees,
                    color,
                    3.,
                );
            }
        }
        self.rim(draw, center);
        if self.tick_intervals > 0 {
            let ticks = (0..=self.tick_intervals).map(|i| {
                let inner = self.radius - if i % 5 == 0 { 9. } else { 3. };
                let angle = self.start_degrees
                    + self.sweep_degrees * f32::from(i) / f32::from(self.tick_intervals);
                DialRay::clockwise(center, angle).tick(self.radius + 2., inner.max(0.))
            });
            draw.segments(ticks, u32::MAX, 1.);
        }
    }
}

impl Widget for RadialGauge<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        // Include the outer rim and its antialiased stroke inside the allocated box.
        let diameter = if self.radius.is_finite() {
            2. * (self.radius.max(0.) + 4.)
        } else {
            0.
        };
        let (rect, response) = ui.allocate_exact_size(Vec2::splat(diameter), Sense::hover());
        let draw = InstrumentPainter {
            painter: ui.painter_at(rect),
            origin: Pos2::ZERO,
        };
        self.paint_at(&draw, rect.center().into());
        response
    }
}
