use xplane_sdk_sys::{XPLMGetScreenBoundsGlobal, XPLMWorldToLocal};

/// Rectangle in X-Plane's global desktop coordinates.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Bounds {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Bounds {
    pub const fn new(left: i32, top: i32, right: i32, bottom: i32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    pub const fn width(self) -> i32 {
        self.right - self.left
    }

    pub const fn height(self) -> i32 {
        self.top - self.bottom
    }
}

/// Returns the bounds of X-Plane's global desktop.
pub fn screen_bounds() -> Bounds {
    let mut bounds = Bounds::default();
    // SAFETY: every output pointer refers to a live field in `bounds`.
    unsafe {
        XPLMGetScreenBoundsGlobal(
            &mut bounds.left,
            &mut bounds.top,
            &mut bounds.right,
            &mut bounds.bottom,
        );
    }
    bounds
}

/// Converts geographic coordinates to X-Plane's local coordinate system.
pub fn world_to_local(latitude: f64, longitude: f64, altitude_m: f64) -> (f64, f64, f64) {
    let (mut x, mut y, mut z) = (0.0, 0.0, 0.0);
    // SAFETY: all output pointers refer to live local variables.
    unsafe {
        XPLMWorldToLocal(latitude, longitude, altitude_m, &mut x, &mut y, &mut z);
    }
    (x, y, z)
}

#[cfg(test)]
mod tests {
    use super::Bounds;

    #[test]
    fn global_bounds_report_positive_dimensions() {
        let bounds = Bounds::new(-100, 500, 700, -100);
        assert_eq!(bounds.width(), 800);
        assert_eq!(bounds.height(), 600);
    }
}
