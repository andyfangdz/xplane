//! Preserve the accepted curves and defaults; this port introduces no new tuning.
use crate::strict::f;

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct Calibration {
    pub fuel_cruise: f64,
    pub thrust_low: f64,
    pub thrust_high: f64,
}
pub const DEFAULT: Calibration = Calibration {
    fuel_cruise: 0.814,
    thrust_low: 1.17,
    thrust_high: 0.87,
};
pub const VARIANT2: Calibration = Calibration {
    fuel_cruise: 0.814,
    thrust_low: 1.10,
    thrust_high: 0.87,
};
pub const VARIANT3: Calibration = Calibration {
    fuel_cruise: 0.814,
    thrust_low: 1.24,
    thrust_high: 0.87,
};

fn smooth(x: f64, lo: f64, hi: f64) -> f64 {
    if x <= lo {
        return 0.0;
    }
    if x >= hi {
        return 1.0;
    }
    let t = (f(x) - lo) / (f(hi) - lo);
    (t * t * (3.0 - 2.0 * t)).0
}
pub fn fuel_scale(power: f64, c: Calibration) -> f64 {
    if !power.is_finite() {
        return 1.0;
    }
    (c.fuel_cruise + (f(1.0) - c.fuel_cruise) * smooth(power, 0.80, 0.90)).0
}
pub fn thrust_scale(knots: f64, c: Calibration) -> f64 {
    if !knots.is_finite() {
        return 1.0;
    }
    (c.thrust_low + (f(c.thrust_high) - c.thrust_low) * smooth(knots, 60.0, 94.0)).0
}
pub fn stall_lift_scale(alpha: f64, flap: f64, mode: i32) -> f64 {
    if mode == 1 || !alpha.is_finite() || !flap.is_finite() {
        return 1.0;
    }
    let deg = (f(flap) * 57.29577951308232 * (f(30.0) / 38.0)).0;
    let clean = match mode {
        2 => 0.10,
        3 => 0.30,
        _ => 0.20,
    };
    let gain = if deg <= 20.0 {
        (clean + (f(0.025) - clean) * smooth(deg, 0.0, 20.0)).0
    } else {
        (0.025 + (f(-0.035) - 0.025) * smooth(deg, 20.0, 30.0)).0
    };
    (1.0 + f(gain) * smooth((f(alpha) * 57.29577951308232).0, 8.0, 14.0)).0
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
    (a.1 + (f(cas) - a.0) * (f(b.1) - a.1) / (f(b.0) - a.0)).0
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
    let cas = (f(forward)
        / alpha
            .clamp((f(-30.0) / DEGREES).0, (f(30.0) / DEGREES).0)
            .cos())
    .0;
    let flap_degrees = (f(flap) * DEGREES).0.clamp(0.0, 30.0);
    let (i0, i20, i30) = (curve(cas, CLEAN), curve(cas, TWENTY), curve(cas, FULL));
    let normal = if flap_degrees <= 20.0 {
        f(i0) + (f(i20) - i0) * flap_degrees / 20.0
    } else {
        f(i20) + (f(i30) - i20) * (f(flap_degrees) - 20.0) / 10.0
    };
    let ratio = if flap_degrees <= 20.0 {
        f(50.5) / 55.0 + (f(43.5) / 51.0 - f(50.5) / 55.0) * flap_degrees / 20.0
    } else {
        f(43.5) / 51.0 + (f(40.5) / 49.5 - f(43.5) / 51.0) * (f(flap_degrees) - 20.0) / 10.0
    };
    let t = f(((f(alpha) * DEGREES - 10.0) / 4.0).0.clamp(0.0, 1.0));
    let blend = t * t * (3.0 - 2.0 * t);
    (normal + (f(cas) * ratio - normal) * blend).0
}
