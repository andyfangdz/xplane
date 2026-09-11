//! Stable 75-float snapshot and CSV schema shared by plugins and flight analysis.
use crate::{
    calibration::{FEET_PER_METER, FPS_PER_KNOT, KNOTS_PER_MPS, POUNDS_PER_KILOGRAM},
    Sample,
};
// snapshot.csv defines field order, CSV names, and SDK input sources for both
// Rust and the Python harness. Keep the wire layout stable; use field constants
// at call sites instead of reproducing numeric offsets.
include!(concat!(env!("OUT_DIR"), "/protocol.rs"));
pub type Snapshot = [f32; LENGTH];

pub fn sample(s: &Snapshot) -> Sample {
    Sample {
        t: s[field::SIM_TIME] as f64,
        x: s[field::RUNWAY_ALONG_FT] as f64,
        y: s[field::RUNWAY_CROSS_FT] as f64,
        h: s[field::AGL_FT] as f64,
        ias: s[field::IAS_KIAS] as f64,
        tas_fps: s[field::TRUE_AIRSPEED_MPS] as f64 * FEET_PER_METER,
        gs_fps: s[field::GROUNDSPEED_KT] as f64 * FPS_PER_KNOT,
        heading: s[field::HEADING_TRUE_DEG] as f64,
        track: s[field::GROUND_TRACK_TRUE_DEG] as f64,
        bank: s[field::BANK_DEG] as f64,
        pitch: s[field::PITCH_DEG] as f64,
        vvi: s[field::VVI_FPM] as f64,
        vy: s[field::VERTICAL_SPEED_MPS] as f64 * FEET_PER_METER,
        wind_kt: s[field::WIND_SPEED_MPS] as f64 * KNOTS_PER_MPS,
        wind_dir: s[field::WIND_DIRECTION_TRUE_DEG] as f64,
        mass_lb: s[field::MASS_KG] as f64 * POUNDS_PER_KILOGRAM,
        throttle: s[field::THROTTLE_RATIO] as f64,
        ground: s[field::GROUND_ANY] != 0.0,
    }
}
