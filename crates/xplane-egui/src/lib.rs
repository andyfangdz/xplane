//! Shared egui/X-Plane rendering, extracted from Position Aircraft's adapter.
#![deny(unsafe_op_in_unsafe_fn)]
mod transform;
pub use transform::{multiply_matrix_vector, transform_primitives, Geometry, RenderTransform};
#[cfg(windows)]
mod surface;
#[cfg(windows)]
pub use surface::{gl_proc_address, restore_xplane_gl_state, Surface};
