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
    let [left, top, right, bottom] = crate::xplm::screen_bounds_global();
    Bounds {
        left,
        top,
        right,
        bottom,
    }
}

/// Converts geographic coordinates to X-Plane's local coordinate system.
pub fn world_to_local(latitude: f64, longitude: f64, altitude_m: f64) -> (f64, f64, f64) {
    let [x, y, z] = crate::xplm::world_to_local(latitude, longitude, altitude_m);
    (x, y, z)
}

pub fn local_to_world(x: f64, y: f64, z: f64) -> (f64, f64, f64) {
    let [latitude, longitude, altitude] = crate::xplm::local_to_world(x, y, z);
    (latitude, longitude, altitude)
}

pub fn screen_size() -> (i32, i32) {
    let [width, height] = crate::xplm::screen_size_array();
    (width, height)
}

/// Returns X-Plane's magnetic declination at a geographic coordinate.
pub fn magnetic_variation(latitude: f64, longitude: f64) -> f64 {
    f64::from(crate::xplm::magnetic_variation(latitude, longitude))
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
