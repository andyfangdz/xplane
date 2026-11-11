#![doc = include_str!("../README.md")]
#![deny(unsafe_op_in_unsafe_fn)]

pub mod fp;
pub mod integration;
pub mod interpolation;
pub mod rotation;
pub mod storage;
#[cfg(all(windows, target_env = "msvc"))]
pub mod stream;
pub mod vector;
