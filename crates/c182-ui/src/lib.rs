//! Aircraft management views. Display data and actions contain no aircraft model.
#![deny(unsafe_op_in_unsafe_fn)]
#[cfg(all(windows, feature = "xplane"))]
mod ffi;
#[cfg(all(windows, feature = "xplane"))]
pub mod surface;
pub mod theme;
pub mod view;
pub const VERSION: &str = "c182-egui-1";
