//! Low-level native compatibility primitives, independent of aircraft data.
//!
//! Explicit arithmetic grouping, operand selection and record access support
//! migrations that must preserve an existing implementation's numerical behavior.
//! Callers own coordinate conventions, policies and the floating-point environment.
pub mod fp;
pub mod integration;
pub mod interpolation;
pub mod rotation;
pub mod storage;
pub mod vector;
