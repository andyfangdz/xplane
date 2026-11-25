use egui::Pos2;
use xplane_plugin::Bounds;

#[derive(Clone, Copy, Debug)]
pub struct Geometry {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}
impl Geometry {
    pub fn width(self) -> i32 {
        self.right - self.left
    }
    pub fn height(self) -> i32 {
        self.top - self.bottom
    }
    pub fn local(self, x: i32, y: i32) -> Pos2 {
        Pos2::new((x - self.left) as f32, (self.top - y) as f32)
    }
}
impl From<Bounds> for Geometry {
    fn from(b: Bounds) -> Self {
        Self {
            left: b.left,
            top: b.top,
            right: b.right,
            bottom: b.bottom,
        }
    }
}
/// A measured scale as egui's pixels per point: a quarter step from 0.5 to
/// 4, and 1 when the measurement is not a number. The steps keep a scale
/// that wobbles in the last bits from rebuilding the font atlas every frame.
pub fn ui_scale(measured: f32) -> f32 {
    if !measured.is_finite() || measured <= 0. {
        return 1.;
    }
    ((measured * 4.).round() / 4.).clamp(0.5, 4.)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_measured_scale_is_quantised_and_bounded() {
        assert_eq!(ui_scale(1.49), 1.5);
        assert_eq!(ui_scale(1.12), 1.0);
        assert_eq!(ui_scale(1.13), 1.25);
        assert_eq!(ui_scale(0.1), 0.5);
        assert_eq!(ui_scale(9.), 4.);
        assert_eq!(ui_scale(f32::NAN), 1.);
        assert_eq!(ui_scale(0.), 1.);
    }
}
