use xplane_attitude::flight::{FlightController, Sample};

fn airborne(target_beta: Option<f32>) -> FlightController {
    let mut c = FlightController::default();
    c.ints[2] = 1;
    c.ints[3] = 1;
    c.write_int(7, 2);
    c.write_int(4, 1);
    if let Some(beta) = target_beta {
        c.write_float(32, beta);
    }
    let s = Sample {
        ias: 80.0,
        tas: 42.0,
        dt: 0.01,
        ..Sample::default()
    };
    for _ in 0..200 {
        c.step(s);
    }
    c
}

#[test]
fn zero_sideslip_target_is_the_original_coordinated_yaw_law() {
    assert_eq!(airborne(None).floats, airborne(Some(0.0)).floats);
}

#[test]
fn sideslip_target_drives_rudder_toward_it_and_is_bounded() {
    let right = airborne(Some(6.0));
    let left = airborne(Some(-6.0));
    // With measured sideslip still zero, the yaw error is the target.
    assert!(right.floats[14] > 0.1, "{}", right.floats[14]);
    assert!(left.floats[14] < -0.1, "{}", left.floats[14]);
    let mut c = FlightController::default();
    c.write_float(32, 40.0);
    assert_eq!(c.floats[32], 12.0);
    c.write_float(32, f32::NAN);
    assert_eq!(c.floats[32], 12.0);
}
