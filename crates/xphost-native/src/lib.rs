//! The audited native boundary: DLL lifetimes, SDK pointers and SEH guards.
#![deny(unsafe_op_in_unsafe_fn)]
pub mod guard;
#[cfg(windows)]
mod loader;
#[cfg(windows)]
pub use loader::Host;
