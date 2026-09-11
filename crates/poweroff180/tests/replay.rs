use poweroff180::{protocol, Config, Controller, Phase};
use std::{fs, io::Read, path::Path};

#[test]
fn accepted_seven_wind_flights_match_frozen_cpp_frame_by_frame() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let indices = [
        0, 1, 2, 3, 4, 5, 6, 7, 11, 15, 16, 23, 24, 25, 26, 29, 30, 31,
    ];
    let mut total = 0;
    // Small fractions of pilot/display resolution: 0.001 degree, 0.01 foot,
    // 0.001 ft/s or ft/s^2, and 0.00001 of full control travel.
    // State transitions and timestamps must still match on the same frame.
    let fields = [
        ("phase", 0.0),
        ("reason", 0.0),
        ("bank_deg", 0.001),
        ("pitch_deg", 0.001),
        ("flap_ratio", 0.00001),
        ("throttle_ratio", 0.00001),
        ("lead_ft", 0.01),
        ("desired_fps", 0.001),
        ("accel_fps2", 0.001),
        ("wind_ff_deg_s", 0.001),
        ("pitch_rate_deg_s", 0.001),
        ("dt_s", 0.0),
        ("cut_t_s", 0.0),
        ("roundout_t_s", 0.0),
        ("gate_s", 0.0),
        ("steps", 0.0),
        ("predicted_cross_ft", 0.01),
        ("cross_accel_fps2", 0.001),
    ];
    let mut maximum_errors = [0.0_f64; 18];
    let mut failures = Vec::new();
    for card in [
        "calm",
        "head5",
        "head10",
        "head15",
        "tail5",
        "cross_left10",
        "cross_right10",
    ] {
        let config =
            Config::parse(&fs::read_to_string(root.join(format!("{card}.ini"))).unwrap()).unwrap();
        let mut control = Controller::new(config);
        let mut bytes = Vec::new();
        flate2::read::GzDecoder::new(fs::File::open(root.join(format!("{card}.bin.gz"))).unwrap())
            .read_to_end(&mut bytes)
            .unwrap();
        assert_eq!(bytes.len() % 216, 0);
        assert!(bytes.len() > 216 * 1000);
        for (frame, row) in bytes.as_chunks::<216>().0.iter().enumerate() {
            let mut snapshot = [0.0; 75];
            for (i, index) in indices.iter().enumerate() {
                snapshot[*index] = f32::from_le_bytes(row[4 * i..4 * i + 4].try_into().unwrap());
            }
            let sample = protocol::sample(&snapshot);
            if frame == 0 {
                control.start(sample.t);
            }
            control.step(sample);
            let actual = [
                control.phase as u8 as f64,
                control.reason as u8 as f64,
                control.bank,
                control.pitch,
                control.flap,
                control.throttle,
                control.lead,
                control.desired,
                control.accel,
                control.wind_ff,
                control.pitch_rate,
                control.dt,
                control.cut_t,
                control.roundout_t,
                control.gate_s,
                control.steps as f64,
                control.predicted_cross,
                control.cross_accel,
            ];
            for (i, value) in actual.iter().enumerate() {
                let expected = f64::from_le_bytes(row[72 + 8 * i..80 + 8 * i].try_into().unwrap());
                let error = (value - expected).abs();
                maximum_errors[i] = maximum_errors[i].max(error);
                let (name, tolerance) = fields[i];
                if !error.is_finite() || error > tolerance {
                    failures.push(format!("{card} frame {frame} {name}: Rust {value}, C++ {expected}, error {error}, tolerance {tolerance}"));
                }
            }
            total += 1;
        }
        assert_eq!(control.phase, Phase::Complete, "{card}");
    }
    println!("{total} C++ reference frames compared");
    for ((name, tolerance), error) in fields.into_iter().zip(maximum_errors) {
        println!("{name}: max error {error:e}, tolerance {tolerance:e}");
    }
    assert!(
        failures.is_empty(),
        "{} replay mismatches; first: {:?}",
        failures.len(),
        failures.first()
    );
}
