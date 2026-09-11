//! Geometry, projection and unit conversions for the Shuttle HUD.
use uom::si::{
    f64::Velocity,
    velocity::{knot, meter_per_second},
};
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

pub use xplane_hud::{rotate, Point, Projected, View};

pub fn constrain(mut p: Projected, l: f64, t: f64, r: f64, b: f64) -> Projected {
    p.limited |= p.p.x < l || p.p.x > r || p.p.y < t || p.p.y > b;
    p.p.x = p.p.x.clamp(l, r);
    p.p.y = p.p.y.clamp(t, b);
    p
}
pub fn eas(tas: f64, rho: f64) -> f64 {
    Velocity::new::<meter_per_second>(tas.max(0.0) * (rho.max(0.0) / 1.225).sqrt()).get::<knot>()
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
