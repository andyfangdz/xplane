use xplane_attitude::flight::{FlightController, Release, Sample};

fn established() -> (FlightController, Sample) {
    let mut c = FlightController::default();
    c.ints[2] = 1;
    c.ints[3] = 1;
    c.write_int(7, 2);
    c.write_int(4, 1);
    c.write_float(0, 20.0);
    c.write_float(2, 6.0);
    let s = Sample {
        ias: 80.0,
        tas: 42.0,
        dt: 0.01,
        ..Sample::default()
    };
    for _ in 0..100 {
        c.step(s);
    }
    assert!(c.floats[12] > 0.2 && c.floats[13] > 0.2);
    (c, s)
}

#[test]
fn slow_frames_keep_authority_and_controller_memory_through_75ms() {
    let (mut c, s) = established();
    // Includes the measured beta timing spike and both sides of the PID cap.
    for dt in [0.0499, 0.05, 0.050251, 0.06, 0.075, 0.008] {
        let previous = c.floats;
        c.step(Sample {
            dt,
            overrides: [true; 3],
            ..s
        });
        assert_eq!(c.ints[5], 1, "inactive at {dt}");
        assert_eq!(c.ints[8], Release::None as i32);
        assert_eq!(c.owns, [true; 3]);
        for axis in [12, 13] {
            assert!(c.floats[axis] > 0.2, "axis {axis} reset at {dt}");
            assert!((c.floats[axis] - previous[axis]).abs() <= 1.2 * dt.min(0.05) + 1e-6);
        }
        assert!((c.floats[15] - 1.0 / dt).abs() < 1e-4);
    }
}

#[test]
fn invalid_timing_still_releases_every_owned_axis() {
    for dt in [
        f32::NAN,
        f32::INFINITY,
        -0.01,
        0.0,
        0.0019,
        0.07501,
        0.1,
        0.25,
    ] {
        let (mut c, s) = established();
        c.step(Sample { dt, ..s });
        assert_eq!(c.ints[5], 0);
        assert_eq!(c.ints[8], Release::BadTiming as i32);
        assert_eq!(c.owns, [false; 3]);
        assert_eq!(&c.floats[12..15], &[0.0; 3]);
    }
}
