use poweroff180::{Config, Controller, Phase, Sample};

fn floating() -> (Controller, Sample) {
    let mut c = Controller {
        phase: Phase::Final,
        cut_t: 1.0,
        roundout_t: 8.0,
        pitch: 8.8,
        accel: 2.8,
        ..Controller::default()
    };
    let s = Sample {
        t: 10.0,
        x: 1001.0,
        h: 2.0,
        ias: 66.4,
        vy: -1.7,
        pitch: 7.3,
        tas_fps: 115.0,
        gs_fps: 113.0,
        mass_lb: c.c.mass_target_lb,
        heading: c.heading,
        track: c.heading,
        ..Sample::default()
    };
    c.last = Some(Sample { t: 9.98, ..s });
    (c, s)
}

#[test]
fn late_float_gets_bounded_steady_sink_without_chattering() {
    let (mut c, mut s) = floating();
    let mut legacy = c.clone();
    legacy.c.flare_float_enabled = 0.0;
    legacy.step(s);
    c.step(s);
    assert!(c.float_guard_active);
    assert!(c.desired <= -c.c.flare_float_sink_fps);
    assert!(c.pitch < legacy.pitch);
    assert!(c.pitch_rate < -0.8 && c.pitch_rate >= -1.25);
    assert_eq!(c.throttle, 0.0);
    assert_eq!(c.phase, Phase::Final);
    // Reaching the sink target must not switch back to the near-zero-sink
    // flare profile and start floating again.
    s.t += 0.02;
    s.vy = -3.0;
    s.x += 2.0;
    c.step(s);
    assert!(c.float_guard_active);
    assert_eq!(c.desired, -c.c.flare_float_sink_fps);
    assert!(c.pitch_rate >= -1.25 && c.pitch_rate <= c.roundout_rate_limit());
}

#[test]
fn correction_does_not_activate_early_high_fast_or_before_imminent_contact() {
    for case in 0..5 {
        let (mut c, mut s) = floating();
        match case {
            0 => s.x = 949.9,
            1 => s.h = 5.1,
            2 => s.ias = 69.1,
            3 => s.vy = -4.5,
            4 => c.c.flare_float_enabled = 0.0,
            _ => unreachable!(),
        }
        c.step(s);
        assert!(!c.float_guard_active, "case {case}");
    }
}

#[test]
fn measured_late_flare_is_anticipated_but_early_contact_is_left_alone() {
    // Representative frames from the retained 0.8.0 crosswind repeats:
    // the long landing still exceeded 67 KIAS when correction was needed.
    for (x, h, vy, accel, ias, expected) in [
        (999.6951, 2.9401, -3.0846, 2.1142, 67.6771, true),
        (1000.0214, 0.4605, -1.306, 1.4176, 65.2196, false),
    ] {
        let (mut c, mut s) = floating();
        c.accel = accel;
        s.x = x;
        s.h = h;
        s.vy = vy;
        s.ias = ias;
        c.last = Some(Sample { t: s.t - 0.02, ..s });
        c.step(s);
        assert_eq!(c.float_guard_active, expected);
        if expected {
            assert!(c.predicted_touchdown_ft > c.c.path_target_touchdown_ft);
            assert!(c.desired <= -c.c.flare_float_sink_fps);
            assert_eq!(c.throttle, 0.0);
        }
    }
}

#[test]
fn anticipated_deceleration_stops_late_pitch_buildup() {
    // The 0.8.2 fourth repeat still raised its pitch target at this point and
    // touched down at 1199.6 ft. Its projected descent was already too shallow.
    let (mut c, mut s) = floating();
    c.accel = 1.8575528;
    c.pitch = 7.094899;
    s.x = 974.37244;
    s.h = 4.978382;
    s.ias = 68.64661;
    s.vy = -1.2288043 / 0.3048;
    s.pitch = 5.285656;
    c.last = Some(Sample {
        t: s.t - 0.02,
        vy: s.vy - c.accel * 0.02,
        ..s
    });
    c.step(s);
    assert!(c.float_guard_active);
    assert!(c.predicted_touchdown_ft > c.c.path_target_touchdown_ft);
    assert!(c.pitch_rate <= 0.0 && c.pitch_rate >= -1.25);
    assert_eq!(c.throttle, 0.0);
}

#[test]
fn invalid_float_configuration_is_rejected() {
    let defaults = Config::default();
    for c in [
        Config {
            flare_float_enabled: 0.5,
            ..defaults
        },
        Config {
            flare_float_sink_fps: 0.5,
            ..defaults
        },
    ] {
        assert!(Config::parse(&c.text()).is_err());
    }
}
