//! Custom pages and overlays for the built-in X-Plane G1000.
#![doc = include_str!("../README.md")]

mod text;
pub use text::{GlyphGrid, TextAlign};

/// Native G1000 page size, in texels.
pub const DISPLAY_SIZE: [u16; 2] = [1024, 768];

#[cfg(windows)]
mod canvas;
#[cfg(windows)]
mod display;
#[cfg(windows)]
mod page;
#[cfg(windows)]
mod texture;

#[cfg(windows)]
pub use canvas::{BitmapFont, Canvas, Gdi};
#[cfg(windows)]
pub use display::{Display, DisplayTarget};
#[cfg(windows)]
pub use page::{Layer, Page};
