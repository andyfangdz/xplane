//! Trapezoidal integration with explicit arithmetic grouping.
#![forbid(unsafe_code)]

/// Advance a value using two endpoint derivatives.
///
/// Evaluates `((first + second) * 0.5) * dt + value` without fused operations.
/// The caller supplies derivative history, restart decisions and any limits.
pub fn trapezoid(value: f64, first: f64, second: f64, dt: f64) -> f64 {
    ((first + second) * 0.5) * dt + value
}
