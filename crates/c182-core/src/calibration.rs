//! Preserve the accepted curves and defaults. One retune since the port: the
//! thrust curve's upper knee moved from 94 kt (`THRUST_KNEE_KT_PORTED`) to
//! 100 kt. The 94 kt default was fitted to a Vy climb timed by the wall clock
//! while the simulator's clock ran about 7 % fast (745 fpm per wall second,
//! about 696 per simulator second). With the climb leaned to the placard it made
//! 678-681 fpm against the POH's 747. Thrust below 60 kt (the takeoff roll) and
//! from 100 kt up (cruise) is unchanged.

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct Calibration {
    pub fuel_cruise: f64,
    pub thrust_low: f64,
    pub thrust_high: f64,
    /// True airspeed (kt) where the thrust scale reaches `thrust_high`; it
    /// leaves `thrust_low` at 60 kt.
    pub thrust_knee_kt: f64,
}
/// The upper thrust knee of the ported (remediation-4) curves.
pub const THRUST_KNEE_KT_PORTED: f64 = 94.0;
pub const THRUST_KNEE_KT: f64 = 100.0;
pub const DEFAULT: Calibration = Calibration {
    fuel_cruise: 0.814,
    thrust_low: 1.17,
    thrust_high: 0.87,
    thrust_knee_kt: THRUST_KNEE_KT,
};
pub const VARIANT2: Calibration = Calibration {
    fuel_cruise: 0.814,
    thrust_low: 1.10,
    thrust_high: 0.87,
    thrust_knee_kt: THRUST_KNEE_KT,
};
pub const VARIANT3: Calibration = Calibration {
    fuel_cruise: 0.814,
    thrust_low: 1.24,
    thrust_high: 0.87,
    thrust_knee_kt: THRUST_KNEE_KT,
};

fn smooth(x: f64, lo: f64, hi: f64) -> f64 {
    if x <= lo {
        return 0.0;
    }
    if x >= hi {
        return 1.0;
    }
    let t = (x - lo) / (hi - lo);
    t * t * (3.0 - 2.0 * t)
}
pub fn fuel_scale(power: f64, c: Calibration) -> f64 {
    if !power.is_finite() {
        return 1.0;
    }
    c.fuel_cruise + (1.0 - c.fuel_cruise) * smooth(power, 0.80, 0.90)
}
pub fn thrust_scale(knots: f64, c: Calibration) -> f64 {
    if !knots.is_finite() {
        return 1.0;
    }
    c.thrust_low + (c.thrust_high - c.thrust_low) * smooth(knots, 60.0, c.thrust_knee_kt)
}
pub fn stall_lift_scale(alpha: f64, flap: f64, mode: i32) -> f64 {
    if mode == 1 || !alpha.is_finite() || !flap.is_finite() {
        return 1.0;
    }
    let deg = flap * 57.29577951308232 * (30.0 / 38.0);
    let clean = match mode {
        2 => 0.10,
        3 => 0.30,
        _ => 0.20,
    };
    let gain = if deg <= 20.0 {
        clean + (0.025 - clean) * smooth(deg, 0.0, 20.0)
    } else {
        0.025 + (-0.035 - 0.025) * smooth(deg, 20.0, 30.0)
    };
    1.0 + gain * smooth(alpha * 57.29577951308232, 8.0, 14.0)
}

fn curve(cas: f64, points: &[(f64, f64)]) -> f64 {
    if cas <= 0.0 {
        return 0.0;
    }
    let mut hi = 1;
    while hi < points.len() - 1 && cas > points[hi].0 {
        hi += 1;
    }
    let (a, b) = (points[hi - 1], points[hi]);
    a.1 + (cas - a.0) * (b.1 - a.1) / (b.0 - a.0)
}

pub fn indicated_airspeed(forward: f64, alpha: f64, flap: f64) -> f64 {
    if !forward.is_finite() || !alpha.is_finite() || !flap.is_finite() || forward <= 0.0 {
        return forward;
    }
    const CLEAN: &[(f64, f64)] = &[
        (0., 0.),
        (61., 55.),
        (65., 60.),
        (73., 70.),
        (82., 80.),
        (90., 90.),
        (99., 100.),
        (108., 110.),
        (117., 120.),
        (126., 130.),
        (136., 140.),
        (146., 150.),
        (156., 160.),
    ];
    const TWENTY: &[(f64, f64)] = &[
        (0., 0.),
        (52., 40.),
        (57., 50.),
        (64., 60.),
        (72., 70.),
        (80., 80.),
        (88., 90.),
        (98., 100.),
        (108., 110.),
        (118., 120.),
    ];
    const FULL: &[(f64, f64)] = &[
        (0., 0.),
        (51., 40.),
        (56., 50.),
        (63., 60.),
        (71., 70.),
        (80., 80.),
        (89., 90.),
        (94., 95.),
    ];
    const DEGREES: f64 = 57.29577951308232;
    let cas = forward / alpha.clamp(-30.0 / DEGREES, 30.0 / DEGREES).cos();
    let flap_degrees = (flap * DEGREES).clamp(0.0, 30.0);
    let (i0, i20, i30) = (curve(cas, CLEAN), curve(cas, TWENTY), curve(cas, FULL));
    let normal = if flap_degrees <= 20.0 {
        i0 + (i20 - i0) * flap_degrees / 20.0
    } else {
        i20 + (i30 - i20) * (flap_degrees - 20.0) / 10.0
    };
    let ratio = if flap_degrees <= 20.0 {
        50.5 / 55.0 + (43.5 / 51.0 - 50.5 / 55.0) * flap_degrees / 20.0
    } else {
        43.5 / 51.0 + (40.5 / 49.5 - 43.5 / 51.0) * (flap_degrees - 20.0) / 10.0
    };
    let t = ((alpha * DEGREES - 10.0) / 4.0).clamp(0.0, 1.0);
    let blend = t * t * (3.0 - 2.0 * t);
    normal + (cas * ratio - normal) * blend
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thrust_holds_its_ends_and_blends_monotonically_to_the_knee() {
        for c in [DEFAULT, VARIANT2, VARIANT3] {
            for v in [0.0, 30.0, 60.0] {
                assert_eq!(thrust_scale(v, c), c.thrust_low);
            }
            for v in [c.thrust_knee_kt, 110.0, 138.0, 200.0] {
                assert_eq!(thrust_scale(v, c), c.thrust_high);
            }
            let middle = (60.0 + c.thrust_knee_kt) / 2.0;
            assert!((thrust_scale(middle, c) - (c.thrust_low + c.thrust_high) / 2.0).abs() < 1e-12);
            let mut last = f64::INFINITY;
            for i in 0..=2000 {
                let v = thrust_scale(f64::from(i) * 0.1, c);
                assert!(v <= last + 1e-15);
                last = v;
            }
        }
        assert_eq!(thrust_scale(f64::NAN, DEFAULT), 1.0);
    }

    #[test]
    fn the_ported_knee_reproduces_the_ported_curve_and_the_default_climbs_harder() {
        let ported = Calibration {
            thrust_knee_kt: THRUST_KNEE_KT_PORTED,
            ..DEFAULT
        };
        assert!((thrust_scale(83.4, ported) - 0.9396).abs() < 0.0005);
        assert!((thrust_scale(64.2, ported) - 1.1574).abs() < 0.0005);
        // The 2026-11-11 leaned Vy climb flew at 84.75 KTAS: 0.9245 ported, 0.9676 now.
        assert!((thrust_scale(84.75, ported) - 0.9245).abs() < 0.0005);
        assert!((thrust_scale(84.75, DEFAULT) - 0.9676).abs() < 0.0005);
        assert_eq!(thrust_scale(94.0, ported), DEFAULT.thrust_high);
        assert!(thrust_scale(94.0, DEFAULT) > DEFAULT.thrust_high);
    }
}
