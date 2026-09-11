//! Numerical calibration of the accepted v7 controller and its f32 wire adapter.
//!
//! These rounded factors preserve the frozen C++ replay and snapshot quantization.
//! They are not definitions of physical units. New geometry and presentation use
//! `xplane_units` quantities; changes here require flight validation.

pub const FEET_PER_NAUTICAL_MILE: f64 = 6076.12;
pub const FEET_PER_METER: f64 = 3.280839895;
pub const FPS_PER_KNOT: f64 = 1.68780986;
pub const KNOTS_PER_MPS: f64 = 1.94384449;
pub const POUNDS_PER_KILOGRAM: f64 = 2.20462262185;
pub const SNAPSHOT_FEET_PER_METER: f32 = 3.280_84;
pub const SNAPSHOT_KNOTS_PER_MPS: f32 = 1.943_844_4;
pub const SNAPSHOT_FPM_PER_MPS: f32 = 196.850_39;
