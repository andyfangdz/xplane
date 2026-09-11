//! Regression coverage for the Shuttle HUD landing model and vector scenes.
//! Frozen project baselines and tolerances are documented in fixtures/README.md.
use shuttle_hud::{
    config::{Optics, Runway},
    guidance::{GuidanceInput, LandingGuidance, LandingPath},
    math::deg,
    presentation::{HudInput, HudPhase, HudPresentation},
    scene::{self, Frame},
};
use uom::si::{
    f64::Length,
    length::{foot, meter},
};
use xplane_airports::RunwayAxis;

fn rows(source: &str) -> impl Iterator<Item = Vec<f64>> + '_ {
    source
        .lines()
        .map(|line| line.split(',').map(|s| s.parse().unwrap()).collect())
}
fn near(actual: f64, expected: f64, tolerance: f64, label: &str) -> f64 {
    assert!(
        (actual - expected).abs() <= tolerance,
        "{label}: actual {actual:.17}, expected {expected:.17}, difference {}",
        actual - expected
    );
    (actual - expected).abs()
}

#[test]
fn landing_path_regression_including_segment_joins() {
    let mut maximum = [0.0_f64; 3];
    for v in rows(include_str!("fixtures/path-baseline.csv")) {
        let p = LandingPath::at(v[0]);
        for (i, (actual, tolerance)) in [
            (p.height, 0.001),
            (p.slope, 0.000001),
            (p.curvature, 0.00000001),
        ]
        .into_iter()
        .enumerate()
        {
            maximum[i] = maximum[i].max(near(
                actual,
                v[i + 1],
                tolerance,
                &format!("path x={}, field {i}", v[0]),
            ));
        }
        // A saved sample within a micrometre of a join may land on either
        // adjacent segment after changing unit conversions. Its geometry above
        // must still agree; segment IDs everywhere else must match exactly.
        if (v[0] - LandingPath::CIRCLE_START).abs() <= 0.000001 {
            assert!([1, 2].contains(&p.segment) && [1.0, 2.0].contains(&v[4]));
        } else if (v[0] - LandingPath::EXP_START).abs() <= 0.000001 {
            assert!([2, 3].contains(&p.segment) && [2.0, 3.0].contains(&v[4]));
        } else {
            assert_eq!(f64::from(p.segment), v[4], "segment at {}", v[0]);
        }
    }
    // Independently enforce the intended side of each current join.
    assert_eq!(LandingPath::at(LandingPath::CIRCLE_START).segment, 1);
    assert_eq!(
        LandingPath::at(LandingPath::CIRCLE_START + 0.0001).segment,
        2
    );
    assert_eq!(LandingPath::at(LandingPath::EXP_START - 0.0001).segment, 2);
    assert_eq!(LandingPath::at(LandingPath::EXP_START).segment, 3);
    println!("Shuttle path max errors [height_m, slope, curvature_per_m]: {maximum:?}");
}

#[test]
fn heavy_and_light_flight_model_regression() {
    let mut g = LandingGuidance::default();
    let mut h = HudPresentation::default();
    let mut count = 0;
    let mut maximum = [0.0_f64; 19];
    // Control ratio, metres, degrees, and seconds; gear, latches, phase, cues,
    // and recorded event times must agree exactly.
    let tolerances = [
        0.00001, 0.0, 0.001, 0.001, 0.001, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.000001, 0.0,
        0.000001, 0.001, 0.001, 0.0,
    ];
    for (frame, v) in rows(include_str!("fixtures/model-baseline.csv")).enumerate() {
        if v[0] != 0.0 {
            g.reset();
            h.reset();
        }
        g.update(GuidanceInput {
            time: v[1],
            along: v[2],
            height: v[3],
            groundspeed: v[4],
            vy: v[5],
            eas: v[6],
            mass_lb: v[7],
            main_wow: v[8] != 0.0,
        });
        h.update(HudInput {
            time: v[1],
            along: v[2],
            height_ft: Length::new::<meter>(v[3]).get::<foot>(),
            groundspeed: v[4],
            eas: v[6],
            main: v[8] != 0.0,
            nose: v[9] != 0.0,
            bank: v[10],
            cross_ft: v[11],
            gear: [v[12], v[13], v[14]],
            final_flare: v[15] != 0.0,
            path_error_ft: Length::new::<meter>(v[3]).get::<foot>()
                - Length::new::<meter>(LandingPath::at(v[2]).height).get::<foot>(),
            gamma_error: deg(v[5].atan2(v[4].max(1.0))) + 20.0,
            stop_distance: Length::new::<foot>(14500.0).get::<meter>() - v[2],
            ..Default::default()
        });
        let actual = [
            g.speedbrake,
            g.gear,
            g.target_height,
            g.gamma,
            g.flare_height,
            g.touch_time,
            f64::from(g.retract_3000),
            f64::from(g.adjust_500),
            f64::from(g.final_flare),
            f64::from(g.phase),
            f64::from(h.phase as i32),
            f64::from(h.main),
            f64::from(h.nose),
            h.fade,
            h.gear_lock_time,
            h.capture_seconds,
            h.deceleration,
            h.required_deceleration,
            f64::from(h.gear_cue),
        ];
        assert_eq!(v.len(), 16 + actual.len());
        for (field, (actual, expected)) in actual.into_iter().zip(&v[16..]).enumerate() {
            maximum[field] = maximum[field].max(near(
                actual,
                *expected,
                tolerances[field],
                &format!("frame {frame}, field {field}"),
            ));
        }
        count += 1;
    }
    assert_eq!(count, 3053);
    println!("Shuttle model max errors by field: {maximum:?}");
}

#[test]
fn symbol_segment_and_clipping_regression_across_24_states() {
    let optics = Optics::parse(include_str!("../hud-optics.txt")).unwrap();
    let runway = Runway {
        name: "KEDW 22L".into(),
        lat: 0.0,
        lon: 0.0,
        end_lat: 0.0,
        end_lon: 0.0,
        width: 91.44,
        displaced: 542.0,
        elev: 694.69,
        heading: 238.11574214786276,
        axis: RunwayAxis::new(
            Length::new::<meter>(-0.8491168816625587 * 4570.0),
            Length::new::<meter>(-0.528204999290666 * 4570.0),
        )
        .unwrap(),
    };
    let mut expected = rows(include_str!("fixtures/scene-baseline.csv")).peekable();
    let mut cases = 0;
    let mut max_pixel_error = 0.0_f64;
    for mut q in rows(include_str!("fixtures/scene-inputs.csv")) {
        // Match the f32 precision of the simulator datarefs sampled by the runtime.
        for k in [17, 18, 20, 21, 22, 23, 24, 25, 26] {
            q[k] = f64::from(q[k] as f32);
        }
        let d = HudPresentation {
            phase: [
                HudPhase::Acq,
                HudPhase::Hdg,
                HudPhase::Prfnl,
                HudPhase::Capt,
                HudPhase::Ogs,
                HudPhase::Flare,
                HudPhase::Fnlfl,
            ][q[3] as usize],
            main: q[4] != 0.0,
            nose: q[5] != 0.0,
            automatic: q[6] != 0.0,
            fade: q[7],
            gear_cue: q[8] as i32,
            deceleration: q[9],
            required_deceleration: q[10],
            ..Default::default()
        };
        let scene = scene::build(&Frame {
            optics,
            runway: &runway,
            runway_rays: None,
            display: &d,
            panel: q[1] != 0.0,
            level: q[2] as i32,
            heading: q[11],
            pitch: q[12],
            roll: q[13],
            along: q[14],
            cross: q[15],
            altitude: q[16],
            height_ft: q[17],
            radar_height_ft: q[18],
            use_radar: q[19] != 0.0,
            equivalent: q[20],
            groundspeed: q[21],
            ground_track: q[22],
            vertical_velocity: q[23],
            command_gamma: q[24],
            command_height: q[25],
            command_speedbrake: q[26],
            actual_speedbrake: q[27],
            horizontal_cage: q[28] != 0.0,
            time: q[29],
            nz: q[30],
        });
        for (group, segments) in scene.layers.iter().enumerate() {
            for (index, s) in segments.iter().enumerate() {
                let v = expected.next().expect("baseline has fewer segments");
                assert_eq!(
                    [v[0] as usize, v[1] as usize, v[2] as usize],
                    [cases, group, index],
                    "segment topology differs"
                );
                let label = format!("case {cases}, group {group}, segment {index}");
                for (actual, expected) in [s.a.x, s.a.y, s.b.x, s.b.y].into_iter().zip(&v[3..7]) {
                    max_pixel_error = max_pixel_error.max(near(actual, *expected, 0.01, &label));
                }
                let clipped = s.clipped(
                    482.0,
                    102.0,
                    1438.0,
                    if group == 0 { 835.0 } else { 1005.0 },
                );
                assert_eq!(clipped.is_some(), v[7] != 0.0, "clipping differs: {label}");
                if let Some(s) = clipped {
                    for (actual, expected) in [s.a.x, s.a.y, s.b.x, s.b.y].into_iter().zip(&v[8..])
                    {
                        max_pixel_error =
                            max_pixel_error.max(near(actual, *expected, 0.01, &label));
                    }
                }
            }
        }
        assert!(
            expected.peek().is_none_or(|v| v[0] as usize != cases),
            "scene has fewer segments in case {cases}"
        );
        cases += 1;
    }
    assert_eq!(cases, 24);
    println!("Shuttle scene maximum endpoint error: {max_pixel_error} pixels");
    assert!(expected.next().is_none());
}
