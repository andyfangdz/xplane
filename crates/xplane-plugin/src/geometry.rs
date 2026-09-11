use xplane_sdk_sys::{XPLMGetMagneticVariation, XPLMGetScreenBoundsGlobal, XPLMWorldToLocal};

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

pub fn local_to_world(x: f64, y: f64, z: f64) -> (f64, f64, f64) {
    let (mut latitude, mut longitude, mut altitude) = (0.0, 0.0, 0.0);
    // SAFETY: all output pointers refer to live local variables.
    unsafe {
        xplane_sdk_sys::XPLMLocalToWorld(x, y, z, &mut latitude, &mut longitude, &mut altitude)
    };
    (latitude, longitude, altitude)
}

pub fn screen_size() -> (i32, i32) {
    let (mut width, mut height) = (0, 0);
    // SAFETY: both output pointers refer to live integers.
    unsafe { xplane_sdk_sys::XPLMGetScreenSize(&mut width, &mut height) };
    (width, height)
}

/// Returns X-Plane's magnetic declination at a geographic coordinate.
pub fn magnetic_variation(latitude: f64, longitude: f64) -> f64 {
    // SAFETY: this SDK function takes values and returns a value; it retains
    // no pointers and is called from X-Plane's plugin thread.
    unsafe { XPLMGetMagneticVariation(latitude, longitude) as f64 }
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
