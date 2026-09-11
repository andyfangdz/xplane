use std::{collections::BTreeSet, io::Read};
use xplane_attitude::flight::{FlightController, Sample};
fn i32_at(input: &mut &[u8]) -> i32 {
    let (a, b) = input.split_at(4);
    *input = b;
    i32::from_le_bytes(a.try_into().unwrap())
}
fn f32_at(input: &mut &[u8]) -> f32 {
    f32::from_bits(i32_at(input) as u32)
}
fn f64_at(input: &mut &[u8]) -> f64 {
    let (a, b) = input.split_at(8);
    *input = b;
    f64::from_le_bytes(a.try_into().unwrap())
}
#[test]
fn full_v19_flight_loop_matches_cpp_modes_latches_and_release_conditions() {
    let mut bytes = Vec::new();
    flate2::read::GzDecoder::new(&include_bytes!("fixtures/v19-flight-loop.bin.gz")[..])
        .read_to_end(&mut bytes)
        .unwrap();
    let mut input = &bytes[..];
    assert_eq!(&input[..8], b"XPTAXIS1");
    input = &input[8..];
    let frames = i32_at(&mut input);
    let mut c = FlightController::default();
    let mut reasons = BTreeSet::new();
    let mut modes = BTreeSet::new();
    let mut contacts = 0;
    let mut max_difference = 0.0_f32;
    for frame in 0..frames {
        let flags: [i32; 8] = std::array::from_fn(|_| i32_at(&mut input));
        let s: [f32; 12] = std::array::from_fn(|_| f32_at(&mut input));
        let geo: [f64; 3] = std::array::from_fn(|_| f64_at(&mut input));
        let targets: [f32; 14] = std::array::from_fn(|_| f32_at(&mut input));
        c.ints[2] = flags[0];
        c.ints[3] = flags[1];
        if c.ints[4] != flags[2] {
            c.write_int(4, flags[2]);
        }
        if c.ints[7] != flags[3] {
            c.write_int(7, flags[3]);
        }
        for (i, v) in [0, 1, 2, 3, 4, 5, 6, 7, 8, 20, 21, 22, 23, 31]
            .into_iter()
            .zip(targets)
        {
            c.write_float(i, v);
        }
        c.step(Sample {
            bank: s[0],
            pitch: s[1],
            heading: s[2],
            p_rad: s[3],
            q_rad: s[4],
            r_rad: s[5],
            beta: s[6],
            ias: s[7],
            tas: s[8],
            dt: s[9],
            vvi: s[10],
            throttle: s[11],
            latitude: geo[0],
            longitude: geo[1],
            elevation: geo[2],
            paused: flags[4] != 0,
            replay: flags[5] != 0,
            ground_mask: flags[6],
            overrides: std::array::from_fn(|i| flags[7] & (1 << i) != 0),
        });
        let expected_i: [i32; 14] = std::array::from_fn(|_| i32_at(&mut input));
        let expected_f: [f32; 32] = std::array::from_fn(|_| f32_at(&mut input));
        let expected_d: [f64; 3] = std::array::from_fn(|_| f64_at(&mut input));
        let expected_owned: [bool; 3] = std::array::from_fn(|_| i32_at(&mut input) != 0);
        assert_eq!(c.ints, expected_i, "integer state at frame {frame}");
        assert_eq!(
            c.doubles, expected_d,
            "first-contact coordinates at {frame}"
        );
        assert_eq!(c.owns, expected_owned, "ownership at {frame}");
        for (i, (a, b)) in c.floats.into_iter().zip(expected_f).enumerate() {
            let diff = (a - b).abs();
            max_difference = max_difference.max(diff);
            assert!(
                diff <= 0.000_004,
                "frame {frame} slot {i}: Rust={a}, C++={b}, difference={diff}"
            );
        }
        reasons.insert(c.ints[8]);
        modes.insert(c.ints[7]);
        contacts += i32::from(c.ints[11] != 0);
    }
    assert!(input.is_empty());
    assert_eq!(reasons, (0..8).collect());
    assert_eq!(modes, (0..4).collect());
    assert!(contacts > 100);
    println!("{frames} complete C++ flight-loop frames; maximum float difference={max_difference}");
}
