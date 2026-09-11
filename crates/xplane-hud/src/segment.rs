use crate::Point;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Segment {
    pub a: Point,
    pub b: Point,
}

impl Segment {
    /// Clips a finite segment to an inclusive, ordered Y-down rectangle.
    /// This clips the centerline; stroke width and GL scissoring are separate.
    pub fn clipped(self, l: f64, t: f64, r: f64, b: f64) -> Option<Self> {
        let (mut lo, mut hi) = (0.0_f64, 1.0_f64);
        let (dx, dy) = (self.b.x - self.a.x, self.b.y - self.a.y);
        for (p, q) in [
            (-dx, self.a.x - l),
            (dx, r - self.a.x),
            (-dy, self.a.y - t),
            (dy, b - self.a.y),
        ] {
            if p.abs() < 1e-12 {
                if q < 0.0 {
                    return None;
                }
            } else {
                let u = q / p;
                if p < 0.0 {
                    lo = lo.max(u);
                } else {
                    hi = hi.min(u);
                }
                if lo > hi {
                    return None;
                }
            }
        }
        Some(Self {
            a: Point::new(self.a.x + lo * dx, self.a.y + lo * dy),
            b: Point::new(self.a.x + hi * dx, self.a.y + hi * dy),
        })
    }

    /// Expands a finite centerline into a butt-ended stroke quad. Segments
    /// shorter than 1e-9 design units do not produce geometry.
    pub fn quad(self, width: f64) -> Option<[Point; 4]> {
        let length = (self.b.x - self.a.x).hypot(self.b.y - self.a.y);
        if length < 1e-9 {
            return None;
        }
        let x = -(self.b.y - self.a.y) * width / (2.0 * length);
        let y = (self.b.x - self.a.x) * width / (2.0 * length);
        Some([
            Point::new(self.a.x + x, self.a.y + y),
            Point::new(self.b.x + x, self.b.y + y),
            Point::new(self.b.x - x, self.b.y - y),
            Point::new(self.a.x - x, self.a.y - y),
        ])
    }
}
