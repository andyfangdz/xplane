use poweroff180::{guidance::rad, Config, Controller, Phase, Reason, Sample};

fn airborne(c: &Controller) -> Sample {
    Sample {
        x: 2000.0,
        h: 1000.0,
        y: c.c.entry_cross_ft,
        ias: 100.0,
        mass_lb: c.c.mass_target_lb,
        tas_fps: 170.0,
        gs_fps: 170.0,
        heading: c.heading + 180.0,
        track: c.heading + 180.0,
        ..Sample::default()
    }
}
fn cut() -> (Controller, Sample) {
    let mut c = Controller::default();
    c.start(0.0);
    let mut s = airborne(&c);
    c.step(s);
    for i in 1..=150 {
        s.t = f64::from(i) * 0.02;
        c.step(s);
    }
    s.t = 3.02;
    s.x = 999.0;
    c.step(s);
    assert_eq!(c.phase, Phase::Delay);
    (c, s)
}
#[test]
fn configuration_is_complete_bounded_and_roundtrips_all_floats() {
    let c = Config::default();
    assert_eq!(Config::parse(&c.text()).unwrap(), c);
    for suffix in ["final_kias=80\n", "typo=1\n"] {
        assert!(Config::parse(&(c.text() + suffix)).is_err());
    }
    for number in ["nan", "inf", "-inf", "200", "75garbage", ""] {
        assert!(Config::parse(
            &c.text()
                .replace("final_kias=75\n", &format!("final_kias={number}\n"))
        )
        .is_err());
    }
    assert!(Config::parse("run_token=1\n").is_err());
    for bad in [
        Config {
            run_token: 1.5,
            ..c
        },
        Config {
            end_lat: c.threshold_lat,
            end_lon: c.threshold_lon,
            ..c
        },
        Config {
            capture_blend_full_deg: c.capture_blend_start_deg,
            ..c
        },
        Config {
            deceleration_end_height_ft: 80.0,
            deceleration_start_height_ft: 80.0,
            ..c
        },
        Config {
            flare_height_ft: 40.0,
            ..c
        },
    ] {
        assert!(Config::parse(&bad.text()).is_err());
    }
}
#[test]
fn entry_gate_requires_continuous_stability_and_exact_cut_crossing() {
    let (c, _) = cut();
    assert_eq!(c.throttle, 0.0);
    assert!(c.gate_s > 3.0);
    let mut c = Controller::default();
    c.start(0.0);
    let mut s = airborne(&c);
    s.x = 1001.0;
    c.step(s);
    s.t = 0.02;
    s.x = 999.0;
    c.step(s);
    assert_eq!(c.reason, Reason::Entry);
    let mut c = Controller::default();
    c.start(0.0);
    s = airborne(&c);
    c.step(s);
    for i in 1..=150 {
        s.t = f64::from(i) * 0.02;
        c.step(s);
    }
    s.t += 0.02;
    s.ias = 90.0;
    c.step(s);
    assert_eq!(c.gate_s, 0.0);
}
#[test]
fn frame_gap_reverse_time_wind_mass_envelope_alignment_and_timeout_abort() {
    for reason in [
        Reason::FrameGap,
        Reason::WindMismatch,
        Reason::MassMismatch,
        Reason::Envelope,
        Reason::LowAlignment,
        Reason::Timeout,
    ] {
        let (mut c, mut s) = cut();
        s.t += 0.02;
        match reason {
            Reason::FrameGap => s.t += 0.5,
            Reason::WindMismatch => s.wind_kt = 1.0,
            Reason::MassMismatch => s.mass_lb += 10.0,
            Reason::Envelope => s.bank = 41.0,
            Reason::LowAlignment => s.h = 50.0,
            Reason::Timeout => c.start_t = -1000.0,
            _ => unreachable!(),
        }
        c.step(s);
        assert_eq!(c.reason, reason);
        assert!(!c.running());
        assert_eq!(c.throttle, 0.0);
    }
    let (mut c, mut s) = cut();
    s.t -= 0.1;
    c.step(s);
    assert_eq!(c.reason, Reason::FrameGap);
}
#[test]
fn roundout_starts_at_35ft_and_damps_after_saturation() {
    let mut c = Controller::new(Config {
        wind_speed_kt: 15.0,
        ..Config::default()
    });
    c.phase = Phase::Final;
    c.cut_t = 1.0;
    let mut s = Sample {
        t: 10.0,
        x: 800.0,
        h: 36.0,
        ias: 72.0,
        mass_lb: c.c.mass_target_lb,
        tas_fps: 120.0,
        gs_fps: 120.0,
        vy: -15.0,
        wind_kt: 15.0,
        heading: c.heading,
        track: c.heading,
        wind_dir: c.heading,
        ..Sample::default()
    };
    c.step(s);
    s.t += 0.02;
    s.h = 34.9;
    c.step(s);
    assert_eq!(c.roundout_t, s.t);
    c.pitch = 5.0;
    s.h = 10.0;
    c.last = Some(s);
    let mut fast = c.clone();
    fast.actual_pitch_rate = 3.0;
    fast.last.as_mut().unwrap().pitch = -0.06;
    s.t += 0.02;
    c.step(s);
    fast.step(s);
    assert!((c.pitch_rate - c.roundout_rate_limit()).abs() < 1e-8);
    assert!(fast.pitch_rate < c.pitch_rate - 0.8);
}
#[test]
fn wind_drifted_arc_and_centerline_braking_have_correct_sign() {
    let mut c = Controller::new(Config {
        lateral_lookahead_s: 0.0,
        ..Config::default()
    });
    let mut s = Sample {
        h: 200.0,
        ias: c.c.final_kias - 1.5,
        heading: c.heading - 90.0,
        ..Sample::default()
    };
    s.tas_fps = s.ias * 1.68780986;
    s.y = s.tas_fps * s.tas_fps / (32.174 * rad(20.0).tan());
    assert!((c.geometry_bank(&s, -s.tas_fps) - 20.0).abs() < 1e-8);
    s.y = 30.0;
    assert!(c.centerline_bank(&s, 0.0) < 0.0);
    assert!(c.centerline_bank(&s, -15.0) > 0.0);
}
#[test]
fn rollout_completes_and_commands_remain_inert_after_abort() {
    let (mut c, mut s) = cut();
    s.ground = true;
    s.h = 1.0;
    s.t += 0.02;
    c.step(s);
    assert_eq!(c.phase, Phase::Rollout);
    for _ in 0..151 {
        s.t += 0.02;
        c.step(s);
    }
    assert_eq!(c.phase, Phase::Complete);
    c.abort(Reason::Cancelled);
    let pitch = c.pitch;
    s.t += 1.0;
    c.step(s);
    assert_eq!(c.pitch, pitch);
    assert_eq!(c.throttle, 0.0);
}
