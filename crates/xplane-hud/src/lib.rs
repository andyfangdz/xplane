//! Pure HUD geometry in display coordinates with Y pointing down.
//!
//! View angles are degrees; focal lengths, points, and stroke widths use the
//! caller's design-plane units. Fonts, layouts, and display policies belong
//! to the consuming plugin. This crate has no SDK or graphics dependencies.
#![forbid(unsafe_code)]

mod projection;
mod segment;

pub use projection::{Projected, View};
pub use segment::Segment;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

pub const fn point(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn rad(degrees: f64) -> f64 {
    degrees * std::f64::consts::PI / 180.0
}

/// Rotates clockwise around `center` in the Y-down display plane.
pub fn rotate(p: Point, center: Point, degrees: f64) -> Point {
    let a = rad(degrees);
    let x = p.x - center.x;
    let y = p.y - center.y;
    point(
        center.x + x * a.cos() - y * a.sin(),
        center.y + x * a.sin() + y * a.cos(),
    )
}
