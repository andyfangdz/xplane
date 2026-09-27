use poweroff180::{Config, Controller, Phase, Reason, Sample};
use uom::si::{
    f64::Velocity,
    velocity::{foot_per_second, knot},
};

fn fps(kt: f64) -> f64 {
    Velocity::new::<knot>(kt).get::<foot_per_second>()
}

/// Established final into a 10 kt headwind gusting 10 kt more.
fn gusting_final() -> (Controller, Sample) {
    let mut c = Controller::new(Config {
        wind_speed_kt: 10.0,
        wind_gust_kt: 10.0,
        ..Config::default()
    });
    c.phase = Phase::Final;
    c.cut_t = 1.0;
    let s = Sample {
        t: 10.0,
        x: -2000.0,
        h: 300.0,
        ias: 78.0,
        tas_fps: fps(80.0),
        gs_fps: fps(70.0),
        mass_lb: c.c.mass_target_lb,
        heading: c.heading,
        track: c.heading,
        wind_kt: 10.0,
        wind_dir: c.heading,
        ..Sample::default()
    };
    c.step(s);
    (c, s)
}

#[test]
fn steady_cards_are_untouched_by_the_gust_estimator() {
    let mut c = Controller::default();
    let s = Sample {
        ias: 90.0,
        wind_kt: 7.0,
        wind_dir: 123.0,
        gs_fps: 150.0,
        ..Sample::default()
    };
    let g = c.gust_sample(&s, 0.02);
    assert_eq!((g.ias, g.wind_kt, g.wind_dir), (s.ias, s.wind_kt, s.wind_dir));
    assert!(c.gust.is_none());
    assert_eq!(c.approach_kias(), c.c.final_kias);
    assert_eq!(c.delay_s, Controller::default().delay_s);
}

#[test]
fn a_gust_step_is_not_flown_as_an_airspeed_step() {
    let (mut c, mut s) = gusting_final();
    let before = c.gust_sample(&s, 0.02);
    // X-Plane raises TAS with the headwind on the same frame; groundspeed and
    // aircraft energy are unchanged and indicated airspeed lags.
    s.wind_kt = 18.0;
    s.tas_fps = fps(88.0);
    let after = c.gust_sample(&s, 0.02);
    assert!((after.ias - before.ias).abs() < 0.05, "{} {}", before.ias, after.ias);
    // A persistent change is absorbed into the mean over the filter interval.
    for _ in 0..3000 {
        c.gust_sample(&s, 0.02);
    }
    let settled = c.gust_sample(&s, 0.02);
    assert!((settled.wind_kt - 18.0).abs() < 0.1);
    assert!((settled.ias - 88.0 * c.gust.unwrap().ias_ratio).abs() < 0.2);
}

#[test]
fn gust_band_replaces_steady_wind_tolerance_without_widening_steady_cards() {
    let (c, s) = gusting_final();
    for (kt, mismatch) in [(8.1, false), (10.0, false), (21.9, false), (7.9, true), (22.1, true)] {
        assert_eq!(c.wind_mismatch(&Sample { wind_kt: kt, ..s }), mismatch, "{kt}");
    }
    assert!(c.wind_mismatch(&Sample { wind_dir: c.heading + 1.5, ..s }));
    let steady = Controller::new(Config { wind_speed_kt: 10.0, ..Config::default() });
    assert!(steady.wind_mismatch(&Sample { wind_kt: 10.4, wind_dir: steady.heading, ..s }));
}

#[test]
fn gust_margins_raise_final_and_landing_targets_and_keep_the_deceleration() {
    let (mut c, s) = gusting_final();
    let p = c.c;
    let mut steady = Controller::new(Config { wind_speed_kt: 10.0, ..Config::default() });
    steady.phase = Phase::Final;
    let target = |c: &Controller, h| c.final_speed_target(&Sample { h, ..s });
    let (final_margin, landing_margin) = (c.final_margin_kt, c.landing_margin_kt);
    assert_eq!(final_margin, p.gust_speed_additive * p.wind_gust_kt);
    assert_eq!(landing_margin, p.gust_landing_additive * p.wind_gust_kt);
    assert!(final_margin > 0.0 && landing_margin > 0.0);
    assert!((target(&c, 300.0) - target(&steady, 300.0) - final_margin).abs() < 1e-9);
    assert!((target(&c, 35.0) - target(&steady, 35.0) - landing_margin).abs() < 1e-9);
    // The short-final deceleration that raises the nose before roundout stays.
    assert!(target(&c, 35.0) < target(&c, 300.0) - 4.0);
    c.phase = Phase::TurnFinal;
    assert_eq!(c.final_speed_target(&s), p.final_kias + final_margin);
}

#[test]
fn turn_timing_uses_the_expected_mean_wind_and_the_faster_glide() {
    let (c, _) = gusting_final();
    let p = c.c;
    let mean = Controller::new(Config {
        wind_speed_kt: p.wind_speed_kt + p.gust_mean_fraction * p.wind_gust_kt,
        ..Config::default()
    });
    let faster = p.delay_gust_additive_s_per_kt * p.gust_speed_additive * p.wind_gust_kt;
    assert!(faster < 0.0);
    assert!((c.delay_s - (mean.delay_s + faster - p.gust_energy_reserve_s).max(0.0)).abs() < 1e-9);
}

#[test]
fn margins_follow_available_energy_and_wind_direction() {
    let card = |speed, offset, gust| {
        Controller::new(Config {
            wind_speed_kt: speed,
            wind_offset_deg: offset,
            wind_gust_kt: gust,
            ..Config::default()
        })
    };
    let (head, cross, quartering_tail, tail) =
        (card(10.0, 0.0, 8.0), card(10.0, 90.0, 8.0), card(5.0, 135.0, 5.0), card(5.0, 180.0, 5.0));
    assert_eq!(head.landing_margin_kt, head.final_margin_kt);
    assert_eq!(cross.landing_margin_kt, cross.final_margin_kt);
    assert!(quartering_tail.landing_margin_kt > 0.0);
    assert!(quartering_tail.landing_margin_kt < 0.5 * quartering_tail.final_margin_kt);
    assert_eq!(tail.landing_margin_kt, 0.0);
    assert!(tail.final_margin_kt > 0.0);
    // 15 kt gusting 23: the turn already starts at the cut; the card is
    // energy limited and saves energy with flaps instead of margin.
    let strong = card(15.0, 0.0, 8.0);
    assert_eq!(strong.delay_s, 0.0);
    assert!(strong.energy_limited && !head.energy_limited);
    assert_eq!(strong.final_margin_kt, strong.c.gust_speed_additive * 8.0);
    assert!(strong.landing_margin_kt <= strong.final_margin_kt);
    // A tailwind gust neither carries a landing margin nor turns earlier.
    let steady_tail = Controller::new(Config {
        wind_speed_kt: 5.0 + tail.c.gust_mean_fraction * 5.0,
        wind_offset_deg: 180.0,
        ..Config::default()
    });
    assert!((tail.delay_s - steady_tail.delay_s).abs() < 1e-9);
}

#[test]
fn energy_limited_card_holds_half_flaps_until_established_on_final() {
    let mut c = Controller::new(Config {
        wind_speed_kt: 15.0,
        wind_gust_kt: 8.0,
        ..Config::default()
    });
    assert!(c.energy_limited);
    c.phase = Phase::Base;
    c.cut_t = 1.0;
    c.flap = 0.5;
    let mut s = Sample {
        t: 30.0,
        x: -2000.0,
        y: 2500.0,
        h: 500.0,
        ias: 80.0,
        tas_fps: fps(82.0),
        gs_fps: fps(70.0),
        mass_lb: c.c.mass_target_lb,
        heading: c.heading - 90.0,
        track: c.heading - 90.0,
        wind_kt: 15.0,
        wind_dir: c.heading,
        ..Sample::default()
    };
    c.last = Some(Sample { t: 29.98, ..s });
    c.step(s);
    assert_eq!(c.flap, 0.5);
    c.phase = Phase::Final;
    s = Sample { t: 30.02, x: -1200.0, y: 0.0, h: 250.0, heading: c.heading, track: c.heading, ..s };
    c.step(s);
    assert_eq!(c.flap, 1.0);
}

#[test]
fn gust_flare_keeps_rate_headroom_steady_flare_does_not_change() {
    let (mut c, _) = gusting_final();
    c.wind_ff = 1.0;
    assert_eq!(c.roundout_rate_limit(), c.c.gust_flare_max_rate_deg_s);
    let mut steady = Controller::new(Config { wind_speed_kt: 10.0, ..Config::default() });
    steady.wind_ff = 1.0;
    assert!(steady.roundout_rate_limit() > c.c.gust_flare_max_rate_deg_s);
}

/// Fly a flare from 30 ft (saturated, or floating when `vy` >= 0) against
/// the attitude-loop behavior fitted from 24 flares, rate = (lead - L0(h)) /
/// 0.4 s, with both the holding lead and response time scaled by `spread`,
/// plus a 0.15 s response lag. Returns (achieved rate, commanded pitch).
fn flare_run(gust: f64, hold: f64, response: f64, vy: f64) -> Vec<(f64, f64)> {
    let (mut c, mut s) = gusting_final();
    c.c.wind_gust_kt = gust;
    c.roundout_t = 9.0;
    c.pitch = -2.0;
    s.x = 700.0;
    s.h = 30.0;
    s.vy = vy;
    s.ias = 78.0;
    s.pitch = -2.2;
    c.last = Some(s);
    let mut out = Vec::new();
    let mut rate = 0.0;
    for _ in 0..280 {
        let dt = 0.01;
        let holding = hold * 1.65 * ((33.0 - s.h) / 33.0).clamp(0.0, 1.0);
        let lead = c.pitch - s.pitch;
        rate += ((lead - holding) / (response * 0.4) - rate) * dt / 0.15;
        s.pitch += rate * dt;
        s.h = (s.h - 10.0 * dt).max(1.0);
        s.t += dt;
        c.step(s);
        assert!(c.running());
        out.push((rate, c.pitch));
    }
    out
}

#[test]
fn gust_flare_pitch_down_does_not_run_away() {
    // A floating aircraft gets a bounded pitch-down demand; the achieved
    // rate must stay near it despite the helper's integrator.
    let down = Controller::default().c.flare_float_down_rate_deg_s;
    for spread in [0.7, 1.0, 1.3] {
        let run = flare_run(10.0, spread, spread, 2.0);
        let worst = run.iter().map(|(r, _)| *r).fold(0.0, f64::min);
        assert!(worst > -2.0 * down, "spread {spread}: {worst}");
    }
}

#[test]
fn gust_flare_commands_the_achieved_pitch_rate_into_ground_effect() {
    let c = Controller::new(Config { wind_gust_kt: 10.0, ..Config::default() });
    let limit = c.c.gust_flare_max_rate_deg_s;
    let late_mean = |run: &[(f64, f64)]| {
        let late = &run[150..];
        late.iter().map(|(r, _)| *r).sum::<f64>() / late.len() as f64
    };
    // Measured flights needed 0.67-1.24x the scheduled holding lead; cover
    // +/-30% in both the holding lead and the response time, mixed.
    for (hold, response) in [(0.7, 0.7), (1.0, 1.0), (1.3, 1.3), (0.7, 1.3), (1.3, 0.7)] {
        let run = flare_run(10.0, hold, response, -12.0);
        let peak = run.iter().map(|(r, _)| *r).fold(0.0, f64::max);
        assert!(peak < 2.8, "hold {hold} response {response}: peak {peak}");
        // Into ground effect the estimate restores the demanded rate, where
        // the integrated steady command falls behind.
        let mean = late_mean(&run);
        assert!(mean > limit - 0.4, "hold {hold} response {response}: late mean {mean}");
        assert!(mean > late_mean(&flare_run(0.0, hold, response, -12.0)));
        assert!(run.iter().all(|(_, p)| *p <= c.c.gust_flare_max_pitch_deg + c.c.gust_flare_hold_lead_deg + 1e-9));
    }
}

#[test]
fn envelope_abort_uses_measured_not_mean_wind_airspeed() {
    let (mut c, mut s) = gusting_final();
    s.t += 0.02;
    s.ias = 59.0;
    c.step(s);
    assert_eq!(c.reason, Reason::Envelope);
}

#[test]
fn a_fast_sink_starts_the_roundout_early_and_a_normal_one_does_not() {
    let (c, s) = gusting_final();
    let p = c.c;
    // Isolate the transition: put the projected roundout on the aim.
    let at_aim = |h: f64, fpm: f64| {
        let mut t = Sample { h, vy: -fpm / 60.0, ..s };
        t.x -= c.roundout_projection_error(&t);
        t
    };
    // v10 gust flights reached the 35 ft roundout at ~685 fpm.
    let steep = c.approach_path_bias(&at_aim(60.0, 850.0));
    assert!(steep > p.gust_path_pitch_limit_deg - 0.5, "{steep}");
    assert!(c.approach_path_bias(&at_aim(60.0, p.gust_entry_sink_fpm)).abs() < 1e-9);
    assert!(c.approach_path_bias(&at_aim(p.gust_preflare_height_ft + 20.0, 850.0)).abs() < 1e-9);
    let limit = c.approach_path_bias(&at_aim(40.0, 3000.0));
    assert!((limit - p.gust_transition_limit_deg).abs() < 1e-9, "{limit}");
}

/// One guidance step in the gust flare at the given state.
fn flare_step(x: f64, h: f64, vy: f64, ias: f64) -> Controller {
    let (mut c, mut s) = gusting_final();
    c.roundout_t = s.t;
    s.t += 0.02;
    (s.x, s.h, s.vy, s.vvi, s.ias) = (x, h, vy, vy * 60.0, ias);
    (s.gs_fps, s.tas_fps) = (fps(60.0), fps(70.0));
    c.last = Some(Sample { t: s.t - 0.02, ..s });
    c.step(s);
    assert!(c.running());
    c
}

#[test]
fn the_flare_puts_it_down_when_long_only_if_slow_enough_to_land() {
    let p = Controller::new(Config { wind_speed_kt: 10.0, wind_gust_kt: 10.0, ..Config::default() });
    let gentle = |c: &Controller| {
        let va = c.c.gust_flare_vertical_accel_fps2;
        let ph = (8.0 - 3.0 * c.c.flare_lookahead_s).max(0.0);
        -(c.c.flare_contact_sink_fps.powi(2) + 2.0 * va * ph).sqrt()
    };
    let limit = p.c.gust_touchdown_max_kias + p.gust_landing_additive();
    // 60 ft short of the aim at 8 ft: settle now, at up to the gust limit.
    let long = flare_step(p.touchdown_aim_ft() - 60.0, 8.0, -3.0, limit - 2.0);
    assert!(long.desired < gentle(&long) - 1.0, "{}", long.desired);
    assert!(long.desired >= -(p.c.gust_touchdown_max_sink_fps.powi(2) + 2.0 * 1.1 * 8.0).sqrt());
    // Still too fast to land: keep flaring and bleed the speed off.
    let fast = flare_step(p.touchdown_aim_ft() - 60.0, 8.0, -3.0, limit + 1.0);
    assert!((fast.desired - gentle(&fast)).abs() < 1e-9, "{}", fast.desired);
}

#[test]
fn the_flare_holds_off_when_short_only_while_there_is_flying_speed() {
    let p = Controller::new(Config { wind_speed_kt: 10.0, wind_gust_kt: 10.0, ..Config::default() });
    let short = flare_step(0.0, 8.0, -3.0, 72.0);
    // Descend only as fast as reaches the aim: well under the planned profile.
    assert!(short.desired > -1.5, "{}", short.desired);
    assert!(short.desired <= -p.c.gust_hold_off_min_sink_fps);
    assert!(short.predicted_touchdown_ft < p.touchdown_aim_ft());
    let slow = flare_step(0.0, 8.0, -3.0, p.c.gust_hold_off_min_kias - 1.0);
    assert!(slow.desired < short.desired - 1.0, "{}", slow.desired);
}
