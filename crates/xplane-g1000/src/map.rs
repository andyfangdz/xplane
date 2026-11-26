//! SDK 4.4 base maps in a G1000 panel callback. All public pixel coordinates
//! use the display's upper-left origin, including those returned by projection.
use crate::{DISPLAY_SIZE, DisplayTarget};
use std::ptr::{self, NonNull};
use xplane_sdk_sys as sdk;

/// GPS and EFIS side consulted when following the simulator's navigation view.
#[derive(Clone, Copy, Debug)]
pub enum Side {
    Pilot,
    Copilot,
}

/// The SDK can display one of these overlays at a time.
#[derive(Clone, Copy, Debug)]
pub enum Overlay {
    Nexrad,
    CloudTops,
    Infrared,
    TerrainWarnings,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Layers {
    pub topography: bool,
    pub relative_terrain: bool,
    pub water: bool,
    pub airport_layout: bool,
    pub overlay: Option<Overlay>,
}

impl Layers {
    fn bits(self) -> i32 {
        let mut bits = match self.overlay {
            Some(Overlay::Nexrad) => sdk::xplm_Map_Nexrad,
            Some(Overlay::CloudTops) => sdk::xplm_Map_clouds,
            Some(Overlay::Infrared) => sdk::xplm_Map_color_IR,
            Some(Overlay::TerrainWarnings) => sdk::xplm_Map_EGPWS,
            None => 0,
        };
        for (enabled, flag) in [
            (self.topography, sdk::xplm_Map_Topo),
            (self.relative_terrain, sdk::xplm_Map_Terrain),
            (self.water, sdk::xplm_Map_Water),
            (self.airport_layout, sdk::xplm_Map_safe_taxi),
        ] {
            if enabled {
                bits |= flag;
            }
        }
        bits
    }
}

/// A geographic point in degrees. Constructors and projection reject invalid coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Location {
    pub latitude: f64,
    pub longitude: f64,
}

impl Location {
    pub fn is_valid(self) -> bool {
        (-90.0..=90.0).contains(&self.latitude) && (-180.0..=180.0).contains(&self.longitude)
    }
}

/// The same immutable rectangle, layers and camera are used for drawing, projection and picking.
#[derive(Clone, Copy, Debug)]
pub struct MapView {
    info: sdk::XPLMMapDrawInfo_t,
    custom: Option<sdk::XPLMMapCustomData_t>,
}

impl MapView {
    /// Follow X-Plane's navigation position, range knob and orientation.
    /// Bounds are [left, top, right, bottom] in G1000 texels.
    pub fn simulator(bounds: [u16; 4], layers: Layers) -> Option<Self> {
        let [left, top, right, bottom] = bounds;
        let [w, h] = DISPLAY_SIZE;
        if left >= right || top >= bottom || right > w || bottom > h {
            return None;
        }
        Some(Self {
            info: sdk::XPLMMapDrawInfo_t {
                structSize: size_of::<sdk::XPLMMapDrawInfo_t>() as _,
                layers: layers.bits(),
                left: left.into(),
                top: (h - top).into(),
                right: right.into(),
                bottom: (h - bottom).into(),
            },
            custom: None,
        })
    }

    /// North-up airport diagram. Range in nautical miles runs from the datum
    /// to the nearest edge, so a 1.25 NM range covers 2.5 NM on the short axis.
    pub fn airport_diagram(bounds: [u16; 4], datum: Location, range_nm: f32) -> Option<Self> {
        if !datum.is_valid() || !range_nm.is_finite() || range_nm <= 0. {
            return None;
        }
        let mut view = Self::simulator(
            bounds,
            Layers {
                topography: true,
                water: true,
                airport_layout: true,
                ..Default::default()
            },
        )?;
        let r = view.info;
        let radius = (r.right - r.left).min(r.top - r.bottom) / 2;
        if radius == 0 {
            return None;
        }
        view.custom = Some(sdk::XPLMMapCustomData_t {
            structSize: size_of::<sdk::XPLMMapCustomData_t>() as _,
            datLat: datum.latitude as f32,
            datLon: datum.longitude as f32,
            centerX: (r.left + r.right) / 2,
            centerY: (r.bottom + r.top) / 2,
            roseRadius: radius,
            mapRange: range_nm,
            orientation: 0,
            trueRotation: 0.,
            // The airport view requests neither relative terrain nor warning shading.
            terrainWarn: 0.,
            terrainCaution: 0.,
            acfAlt: 0.,
            gearDown: 0,
            nearestRwyElev: 0.,
            egpwsBrightness: 0.,
            egpwsStyle: sdk::xplm_EGPWS_Style_Smooth as _,
        });
        Some(view)
    }

    /// The right and bottom edges are exclusive, just as for a page blit.
    pub fn contains(&self, [x, y]: [f32; 2]) -> bool {
        let r = self.info;
        let h = f32::from(DISPLAY_SIZE[1]);
        (r.left as f32..r.right as f32).contains(&x)
            && (h - r.top as f32..h - r.bottom as f32).contains(&y)
    }

    fn with_sdk<R>(
        &self,
        f: impl FnOnce(&mut sdk::XPLMMapDrawInfo_t, *mut sdk::XPLMMapCustomData_t) -> R,
    ) -> R {
        let mut info = self.info;
        let mut custom = self.custom;
        f(
            &mut info,
            custom.as_mut().map_or(ptr::null_mut(), ptr::from_mut),
        )
    }
}

/// Owns one simulator map and its terrain tile cache. Create and drop it in
/// simulator-thread callbacks. Recreate it on aircraft changes to pick up
/// aircraft-specific color stripes. The handle cannot cross threads.
pub struct MapDisplay(NonNull<std::ffi::c_void>);

impl MapDisplay {
    pub fn new(side: Side) -> Option<Self> {
        let mut params = sdk::XPLMCreateMap_t {
            structSize: size_of::<sdk::XPLMCreateMap_t>() as _,
            pilotIndex: match side {
                Side::Pilot => 0,
                Side::Copilot => 1,
            },
        };
        // SAFETY: the initialized creation record is live for the synchronous call.
        NonNull::new(unsafe { sdk::XPLMCreateMapDisplay(&mut params) }).map(Self)
    }

    /// Draw into the active avionics surface. X-Plane leaves it untouched while tiles load.
    pub fn draw(&self, _target: &mut DisplayTarget, view: &MapView) {
        view.with_sdk(|info, custom| {
            // SAFETY: DisplayTarget is only lent inside the registered panel callback.
            unsafe { sdk::XPLMMapDisplayDrawIn(self.0.as_ptr(), info, custom) }
        });
    }

    /// Project even off-screen points (use `MapView::contains` to cull symbols).
    /// Returns None while tiles load or when X-Plane cannot project the point.
    pub fn project(&self, view: &MapView, at: Location) -> Option<[f32; 2]> {
        if !at.is_valid() {
            return None;
        }
        let (mut x, mut y) = (0., 0.);
        let ok = view.with_sdk(|info, custom| {
            // SAFETY: handle, draw description and both outputs live through this call.
            unsafe {
                sdk::XPLMMapDisplayProject(
                    self.0.as_ptr(),
                    info,
                    custom,
                    at.latitude,
                    at.longitude,
                    &mut x,
                    &mut y,
                )
            }
        });
        (ok != 0 && x.is_finite() && y.is_finite()).then_some([x, f32::from(DISPLAY_SIZE[1]) - y])
    }

    /// Pick within this view (outside a draw, e.g. from a main-thread input callback, is valid).
    pub fn unproject(&self, view: &MapView, p: [f32; 2]) -> Option<Location> {
        if !view.contains(p) {
            return None;
        }
        let mut at = Location {
            latitude: 0.,
            longitude: 0.,
        };
        let ok = view.with_sdk(|info, custom| {
            // SAFETY: both outputs are writable; SDK leaves them untouched when tiles are missing.
            unsafe {
                sdk::XPLMMapDisplayUnproject(
                    self.0.as_ptr(),
                    info,
                    custom,
                    p[0],
                    f32::from(DISPLAY_SIZE[1]) - p[1],
                    &mut at.latitude,
                    &mut at.longitude,
                )
            }
        });
        (ok != 0 && at.is_valid()).then_some(at)
    }
}

impl Drop for MapDisplay {
    fn drop(&mut self) {
        // SAFETY: this owner exclusively destroys its live map once on the simulator thread.
        unsafe { sdk::XPLMDestroyMapDisplay(self.0.as_ptr()) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simulator_view_selects_one_overlay_and_keeps_full_geographic_range() {
        for (overlay, bit) in [
            (Overlay::Nexrad, sdk::xplm_Map_Nexrad),
            (Overlay::CloudTops, sdk::xplm_Map_clouds),
            (Overlay::Infrared, sdk::xplm_Map_color_IR),
            (Overlay::TerrainWarnings, sdk::xplm_Map_EGPWS),
        ] {
            let layers = Layers {
                water: true,
                overlay: Some(overlay),
                ..Default::default()
            };
            let view = MapView::simulator([0, 0, 1024, 768], layers).unwrap();
            assert_eq!(view.info.layers, bit | sdk::xplm_Map_Water);
            assert!(view.custom.is_none());
        }
        assert!(
            Location {
                latitude: -90.,
                longitude: 180.
            }
            .is_valid()
        );
        assert!(
            !Location {
                latitude: 91.,
                longitude: 0.
            }
            .is_valid()
        );
    }
    #[test]
    fn offset_bounds_and_range_use_the_sdk_panel_coordinates() {
        let datum = Location {
            latitude: 47.53,
            longitude: -122.3,
        };
        let view = MapView::airport_diagram([122, 84, 900, 680], datum, 1.25).unwrap();
        assert_eq!(
            [
                view.info.left,
                view.info.top,
                view.info.right,
                view.info.bottom
            ],
            [122, 684, 900, 88]
        );
        let p = view.custom.unwrap();
        assert_eq!([p.centerX, p.centerY, p.roseRadius], [511, 386, 298]);
        assert_eq!(p.mapRange, 1.25);
        assert_eq!(view.info.layers, 8 | 32 | 256);
        assert!(view.contains([122., 84.]));
        assert!(!view.contains([900., 200.]));
        assert!(!view.contains([300., 680.]));
        assert!(
            MapView::simulator([122, 84, 900, 680], Layers::default())
                .unwrap()
                .custom
                .is_none()
        );
        assert!(MapView::airport_diagram([0, 0, 1, 1], datum, 1.).is_none());
        assert!(MapView::airport_diagram([0, 0, 20, 20], datum, f32::NAN).is_none());
        assert!(MapView::simulator([0, 0, 1025, 768], Layers::default()).is_none());
        assert!(
            !Location {
                latitude: 0.,
                longitude: f64::NAN
            }
            .is_valid()
        );
    }
}
