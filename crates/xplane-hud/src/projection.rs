use crate::{rad, Point};

#[derive(Clone, Copy, Debug, Default)]
pub struct Projected {
    pub p: Point,
    /// The ray is at or behind the forward-plane cutoff. Its coordinates use
    /// a clamped denominator so the caller can cage it or discard it.
    pub limited: bool,
}

/// A perspective view with independently calibrated horizontal and vertical
/// focal lengths and an optical center in the caller's Y-down design plane.
#[derive(Clone, Copy, Debug, Default)]
pub struct View {
    pub fx: f64,
    pub fy: f64,
    pub center: Point,
    pub heading: f64,
    pub pitch: f64,
    pub roll: f64,
}

impl View {
    /// Projects an absolute bearing and elevation, both in degrees.
    /// The caller owns field-of-view validation and display bounds.
    pub fn project(self, az: f64, el: f64) -> Projected {
        let (a, e, p, r) = (
            rad((az - self.heading + 180.0).rem_euclid(360.0) - 180.0),
            rad(el),
            rad(self.pitch),
            rad(self.roll),
        );
        let f = e.cos() * a.cos() * p.cos() + e.sin() * p.sin();
        let x = e.cos() * a.sin();
        let y = e.sin() * p.cos() - e.cos() * a.cos() * p.sin();
        Projected {
            p: Point::new(
                self.center.x + self.fx * (x * r.cos() - y * r.sin()) / f.max(0.01),
                self.center.y - self.fy * (x * r.sin() + y * r.cos()) / f.max(0.01),
            ),
            limited: f <= 0.01,
        }
    }

    /// Reprojects a point from this view into another view. Focal lengths must
    /// be finite and nonzero. Visibility remains the destination's decision.
    pub fn camera_point(self, camera: Self, point: Point) -> Projected {
        let (x, y) = (
            (point.x - self.center.x) / self.fx,
            -(point.y - self.center.y) / self.fy,
        );
        let a = x * rad(self.roll).cos() + y * rad(self.roll).sin();
        let b = -x * rad(self.roll).sin() + y * rad(self.roll).cos();
        let f = rad(self.pitch).cos() - b * rad(self.pitch).sin();
        let u = rad(self.pitch).sin() + b * rad(self.pitch).cos();
        let deg = |r: f64| r * 180.0 / std::f64::consts::PI;
        camera.project(self.heading + deg(a.atan2(f)), deg(u.atan2(a.hypot(f))))
    }
}
