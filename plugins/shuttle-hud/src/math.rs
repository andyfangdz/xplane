//! Geometry and unit conventions preserved from C++ release 142.
pub const FT: f64 = 3.280839895;
pub const KT: f64 = 1.943844492;
pub const PI: f64 = std::f64::consts::PI;
pub fn rad(d: f64) -> f64 {
    d * PI / 180.0
}
pub fn deg(r: f64) -> f64 {
    r * 180.0 / PI
}
pub fn wrap(d: f64) -> f64 {
    let d = (d + 180.0) % 360.0;
    (if d < 0.0 { d + 360.0 } else { d }) - 180.0
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}
impl Point {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}
#[derive(Clone, Copy, Debug, Default)]
pub struct Projected {
    pub p: Point,
    pub limited: bool,
}
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
    pub fn project(self, az: f64, el: f64) -> Projected {
        let (a, e, p, r) = (
            rad(wrap(az - self.heading)),
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
    pub fn camera_point(self, camera: Self, point: Point) -> Projected {
        let (x, y) = (
            (point.x - self.center.x) / self.fx,
            -(point.y - self.center.y) / self.fy,
        );
        let a = x * rad(self.roll).cos() + y * rad(self.roll).sin();
        let b = -x * rad(self.roll).sin() + y * rad(self.roll).cos();
        let f = rad(self.pitch).cos() - b * rad(self.pitch).sin();
        let u = rad(self.pitch).sin() + b * rad(self.pitch).cos();
        camera.project(self.heading + deg(a.atan2(f)), deg(u.atan2(a.hypot(f))))
    }
}
pub fn rotate(p: Point, c: Point, d: f64) -> Point {
    let (a, x, y) = (rad(d), p.x - c.x, p.y - c.y);
    Point::new(
        c.x + x * a.cos() - y * a.sin(),
        c.y + x * a.sin() + y * a.cos(),
    )
}
pub fn constrain(mut p: Projected, l: f64, t: f64, r: f64, b: f64) -> Projected {
    p.limited |= p.p.x < l || p.p.x > r || p.p.y < t || p.p.y > b;
    p.p.x = p.p.x.clamp(l, r);
    p.p.y = p.p.y.clamp(t, b);
    p
}
pub fn eas(tas: f64, rho: f64) -> f64 {
    tas.max(0.0) * (rho.max(0.0) / 1.225).sqrt() * KT
}
pub fn clutter(h: f64, visible: bool, mode: i32) -> i32 {
    if mode >= 0 {
        mode.clamp(0, 2)
    } else if h < 4000.0 {
        2
    } else if h < 10000.0 && visible {
        1
    } else {
        0
    }
}
pub fn advance_fade(fade: f64, previous: f64, now: f64, prefinal: bool) -> f64 {
    if previous < 0.0 || now < previous {
        return if prefinal { 5.0 } else { 0.0 };
    }
    let dt = (now - previous).clamp(0.0, 0.2);
    (fade + if prefinal { dt } else { -dt }).clamp(0.0, 5.0)
}
pub fn matches(loaded: &str, expected: &str) -> bool {
    !expected.is_empty() && loaded == expected
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn projection_units_and_caging() {
        assert!((eas(100.0, 1.225) - 194.3844492).abs() < 1e-6);
        assert!((eas(100.0, 1.225 * 0.25) - 97.1922246).abs() < 1e-6);
        assert_eq!(
            [
                clutter(3999.0, true, -1),
                clutter(4000.0, true, -1),
                clutter(10000.0, true, -1),
                clutter(6000.0, false, -1)
            ],
            [2, 1, 0, 0]
        );
        let v = View {
            fx: 1000.0,
            fy: 1000.0,
            center: Point::new(960.0, 300.0),
            heading: 238.0,
            pitch: -10.0,
            roll: 0.0,
        };
        let p = v.project(238.0, -10.0);
        assert!((p.p.x - 960.0).abs() < 1e-8 && (p.p.y - 300.0).abs() < 1e-8);
        assert!(v.project(243.0, -10.0).p.x > 960.0 && v.project(238.0, -20.0).p.y > 300.0);
        let p = constrain(
            Projected {
                p: Point::new(3000.0, -20.0),
                limited: false,
            },
            500.0,
            100.0,
            1400.0,
            900.0,
        );
        assert!(p.limited);
        assert_eq!(p.p, Point::new(1400.0, 100.0));
        let p = rotate(Point::new(100.0, 0.0), Point::default(), 90.0);
        assert!(p.x.abs() < 1e-8 && (p.y - 100.0).abs() < 1e-8);
        assert!(matches("a/Orbiter_Glider.acf", "a/Orbiter_Glider.acf"));
        assert!(!matches("b/Orbiter_Glider.acf", "a/Orbiter_Glider.acf"));
        assert!(!matches("", ""));
        assert_eq!(advance_fade(0.0, 12.0, 0.0, true), 5.0);
        assert_eq!(advance_fade(5.0, 12.0, 0.0, false), 0.0);
        assert_eq!(advance_fade(2.5, 12.0, 12.0, true), 2.5);
        assert!((advance_fade(2.5, 12.0, 12.1, true) - 2.6).abs() < 1e-9);
    }
}
