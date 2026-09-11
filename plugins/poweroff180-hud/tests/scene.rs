use poweroff180::{
    hud::point,
    protocol::{field, Snapshot, LENGTH},
    Config,
};
use poweroff180_hud::{
    scene::{Draw, Hud, PINK, WHITE},
    values::{Values, NAMES},
};
fn values(speed: f64, alt: f64) -> Values {
    let mut v = Values(NAMES.iter().map(|name| (*name, 0.0)).collect());
    for (name, value) in [
        ("sim/cockpit2/gauges/indicators/airspeed_kts_pilot", speed),
        ("sim/cockpit2/gauges/indicators/altitude_ft_pilot", alt),
        ("sim/aircraft/view/acf_Vso", 56.0),
        ("sim/aircraft/view/acf_Vs", 64.0),
        ("sim/aircraft/view/acf_Vno", 163.0),
        ("sim/aircraft/view/acf_Vne", 200.0),
        ("sim/graphics/view/field_of_view_deg", 75.0),
        (
            "sim/cockpit2/gauges/actuators/barometer_setting_in_hg_pilot",
            29.92,
        ),
        (
            "sim/cockpit2/gauges/indicators/heading_AHARS_deg_mag_pilot",
            223.0,
        ),
        ("sim/cockpit/radios/gps_course_degtm", 223.0),
        ("sim/cockpit/radios/gps_fromto", 1.0),
        ("sim/cockpit/radios/gps_hdef_dot", 1.0),
        ("sim/flightmodel/position/latitude", 40.87),
        ("sim/flightmodel/position/longitude", -74.28),
    ] {
        v.0.insert(name, value);
    }
    v
}
fn finite(draw: &Draw) -> bool {
    let finite_point = |p: &poweroff180::hud::Point| p.x.is_finite() && p.y.is_finite();
    match draw {
        Draw::Line(a, b, _, width) => finite_point(a) && finite_point(b) && width.is_finite(),
        Draw::Polygon(p, _, _, width) => p.iter().all(finite_point) && width.is_finite(),
        Draw::Circle(p, r, _, w, _) => finite_point(p) && r.is_finite() && w.is_finite(),
        Draw::Arc(p, r, a, b, _, w) => {
            finite_point(p) && [r, a, b, w].into_iter().all(|v| v.is_finite())
        }
        Draw::Text {
            at, size, rotation, ..
        } => finite_point(at) && size.is_finite() && rotation.is_none_or(f64::is_finite),
        Draw::Clip { x, y, w, h } => [x, y, w, h].into_iter().all(|v| v.is_finite()),
        Draw::Unclip => true,
    }
}
#[test]
fn tapes_digits_camera_and_missing_instruments_produce_finite_balanced_frames() {
    let mut hud = Hud {
        full_flap_limit: 104.0,
        ..Hud::default()
    };
    for (i, (speed, alt, bank)) in [
        (19.0, -100.0, -35.0),
        (65.0, 99.9, 0.0),
        (104.0, 999.9, 25.0),
        (205.0, 20000.0, 60.0),
        (f64::NAN, f64::NAN, 0.0),
    ]
    .into_iter()
    .enumerate()
    {
        let mut s: Snapshot = [0.0; LENGTH];
        s[field::SIM_TIME] = i as f32;
        s[field::IAS_KIAS] = speed as f32;
        s[field::BANK_DEG] = bank as f32;
        s[field::TRUE_AIRSPEED_MPS] = 50.0;
        s[field::PHASE_ID] = 7.0;
        let scene = hud.frame(&s, &values(speed, alt));
        let mut depth = 0;
        for draw in &scene.commands {
            assert!(finite(draw), "non-finite display command: {draw:?}");
            match draw {
                Draw::Clip { .. } => depth += 1,
                Draw::Unclip => {
                    depth -= 1;
                    assert!(depth >= 0);
                }
                _ => {}
            }
        }
        assert_eq!(depth, 0);
        assert!(scene.commands.len() > 200);
    }
}
#[test]
fn full_flap_band_uses_104_kias_and_live_cdi_moves_one_dot_right() {
    let mut hud = Hud {
        full_flap_limit: 104.0,
        nav_ready: true,
        leg: [40.9, -74.27, 40.87, -74.28],
        ..Hud::default()
    };
    let scene = hud.frame(&[0.0; LENGTH], &values(100.0, 1000.0));
    assert!(scene.commands.iter().any(|draw| match draw {
        Draw::Polygon(points, c, true, _) if *c == WHITE =>
            points[0].x == 638.0 && (points[0].y - 501.3333333333333).abs() < 1e-8,
        _ => false,
    }));
    assert!(scene.commands.iter().any(|draw|matches!(draw,Draw::Line(a,b,c,w) if *a==point(990.0,825.0)&&*b==point(990.0,933.0)&&*c==PINK&&*w==4.5)));
}
#[test]
fn trail_and_recording_clock_reset_between_cards() {
    let mut hud = Hud::default();
    let mut s = [0.0; LENGTH];
    s[field::PHASE_ID] = 2.0;
    s[field::RUNWAY_ALONG_FT] = 2700.0;
    s[field::SIM_TIME] = 10.0;
    hud.frame(&s, &values(100.0, 1000.0));
    s[field::SIM_TIME] = 11.0;
    s[field::RUNWAY_ALONG_FT] = 2600.0;
    hud.frame(&s, &values(100.0, 1000.0));
    assert_eq!(hud.first_time, 10.0);
    assert!(hud.path_distance > 0.0);
    hud.new_flight(Config::default());
    assert!(hud.trail.is_empty());
    assert_eq!(hud.first_time, -1.0);
    assert_eq!(hud.trend.time, -1.0);
}

#[test]
fn navigation_leg_keeps_nautical_miles_right_positive_and_missing_data_behavior() {
    let mut hud = Hud {
        nav_ready: true,
        leg: [40.0, -75.0, 40.1, -75.0],
        ..Hud::default()
    };
    let mut v = values(100.0, 1000.0);
    v.0.insert("sim/flightmodel/position/latitude", 40.05);
    v.0.insert("sim/flightmodel/position/longitude", -74.99);
    let expected_nm = 0.01 * 60.0 * poweroff180::guidance::rad(40.05).cos();
    assert!((hud.leg_cross_track(&v) - expected_nm).abs() < 1e-12);
    hud.leg = [40.1, -75.0, 40.0, -75.0];
    assert!((hud.leg_cross_track(&v) + expected_nm).abs() < 1e-12);
    hud.leg = [40.0, -75.0, 40.0, -75.0];
    assert!(hud.leg_cross_track(&v).is_nan());
    hud.leg = [40.0, -75.0, 40.1, -75.0];
    assert!(hud.leg_cross_track(&Values::default()).is_nan());
    hud.nav_ready = false;
    assert!(hud.leg_cross_track(&v).is_nan());
}
