use poweroff180::{protocol, Config, Controller, Phase};
use std::{fs, io::Read, path::Path};

#[test]
fn accepted_seven_wind_flights_match_frozen_cpp_frame_by_frame() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let indices = [
        0, 1, 2, 3, 4, 5, 6, 7, 11, 15, 16, 23, 24, 25, 26, 29, 30, 31,
    ];
    let mut total = 0;
    let mut maximum_error = 0.0_f64;
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
                maximum_error = maximum_error.max(error);
                // Different platform libm implementations may vary by a few ulps.
                // Absolute bound is far below one float telemetry increment.
                assert!(
                    error <= 1e-8,
                    "{card} frame {frame} column {i}: Rust {value}, C++ {expected}, error {error}"
                );
            }
            total += 1;
        }
        assert_eq!(control.phase, Phase::Complete, "{card}");
    }
    println!("{total} C++ reference frames matched; max absolute error {maximum_error:e}");
}
