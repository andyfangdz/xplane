//! Egui windows drawn in X-Plane's global desktop boxels.
#![deny(unsafe_op_in_unsafe_fn)]
mod transform;
pub use transform::{Geometry, ui_scale};
#[cfg(windows)]
mod panel;
#[cfg(windows)]
pub use panel::PanelPainter;
#[cfg(windows)]
mod surface;
#[cfg(windows)]
pub use surface::{Surface, map_key, modifiers_from_flags};
