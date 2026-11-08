//! Fake SDK ABI. All foreign buffers and callbacks are confined to this crate.
//! Handles are checked integer IDs, never dereferenced foreign pointers.
#![deny(unsafe_op_in_unsafe_fn)]
#![allow(non_snake_case)]
mod access;
mod control;
mod exports;
mod state;
