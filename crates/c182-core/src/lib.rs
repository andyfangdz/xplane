#![deny(unsafe_op_in_unsafe_fn)]

pub mod calibration;
pub mod standby;
mod strict;

pub const IMPLEMENTATION: &str = "c182-rust-core-1";
