#![deny(unsafe_op_in_unsafe_fn)]

pub mod calibration;
/// Compatibility path for numerical helpers; new callers can use `xplane_numerics`.
pub use xplane_numerics as native;
pub mod standby;
mod strict;

pub const IMPLEMENTATION: &str = "c182-rust-core-1";
