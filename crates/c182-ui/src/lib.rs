//! Aircraft management views. Display data and actions contain no aircraft model.
#[cfg(all(windows, feature = "xplane"))]
mod ffi;
pub mod theme;
pub mod view;
pub const VERSION: &str = "c182-egui-1";
