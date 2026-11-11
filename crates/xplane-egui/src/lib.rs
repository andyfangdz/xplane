//! Shared egui/X-Plane rendering, extracted from Position Aircraft's adapter.
#![deny(unsafe_op_in_unsafe_fn)]
mod transform;
pub use transform::{Geometry, RenderTransform, multiply_matrix_vector, transform_primitives};
#[cfg(windows)]
mod surface;
#[cfg(windows)]
pub use surface::{Surface, gl_proc_address, restore_xplane_gl_state};
