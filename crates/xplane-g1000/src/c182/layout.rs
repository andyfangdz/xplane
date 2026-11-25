//! Native 1024 × 768 C182 strip geometry with bottom-up device coordinates.
use xplane_numerics::fp as f;
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rgba {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}
pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Rgba {
    Rgba { r, g, b, a }
}
#[repr(u8)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Kind {
    #[default]
    Rect,
    Line,
    Arc,
    Label,
    Tri,
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Prim {
    pub kind: Kind,
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
    pub r: f32,
    pub start: f32,
    pub sweep: f32,
    pub width: f32,
    pub color: Rgba,
    pub text: i32,
}
pub const BACKGROUND: Rgba = rgba(32. / 255., 32. / 255., 32. / 255., 1.);
pub const WHITE: Rgba = rgba(1., 1., 1., 1.);
pub const CYAN: Rgba = rgba(0., 1., 1., 1.);
pub const CYLINDERS: usize = 6;
pub const PI: f32 = std::f32::consts::PI;
pub fn finite(v: f32) -> bool {
    v.to_bits() & 0x7f800000 != 0x7f800000
}
pub fn hottest(values: &[f32]) -> i32 {
    let mut best = -1;
    for (i, v) in values.iter().enumerate() {
        if finite(*v) && (best < 0 || f::ordered_gt_f32(*v, values[best as usize])) {
            best = i as i32
        }
    }
    best
}
