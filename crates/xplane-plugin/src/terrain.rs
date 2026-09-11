use crate::{local_to_world, world_to_local};
use std::{ffi::c_void, ptr::NonNull};
use xplane_sdk_sys::{
    xplm_ProbeHitTerrain, xplm_ProbeY, XPLMCreateProbe, XPLMDestroyProbe, XPLMProbeInfo_t,
    XPLMProbeTerrainXYZ,
};

/// Owns a vertical terrain probe. Creation, use and destruction stay on the
/// plugin thread; the raw handle is never exported or declared Send.
pub struct TerrainProbe(NonNull<c_void>);
impl TerrainProbe {
    pub fn new() -> Result<Self, String> {
        // SAFETY: this is called from a live X-Plane plugin callback.
        NonNull::new(unsafe { XPLMCreateProbe(xplm_ProbeY) })
            .map(Self)
            .ok_or_else(|| "terrain probe creation failed".to_owned())
    }
    pub fn elevation(&self, lat: f64, lon: f64, alt: f64) -> Option<f64> {
        let (x, y, z) = world_to_local(lat, lon, alt);
        let mut info = XPLMProbeInfo_t {
            structSize: std::mem::size_of::<XPLMProbeInfo_t>() as i32,
            locationX: 0.0,
            locationY: 0.0,
            locationZ: 0.0,
            normalX: 0.0,
            normalY: 0.0,
            normalZ: 0.0,
            velocityX: 0.0,
            velocityY: 0.0,
            velocityZ: 0.0,
            is_wet: 0,
        };
        // SAFETY: the probe is live and `info` has the documented structure size
        // and writable storage for the SDK's immediate output.
        let hit = unsafe {
            XPLMProbeTerrainXYZ(self.0.as_ptr(), x as f32, y as f32, z as f32, &mut info)
        };
        if hit != xplm_ProbeHitTerrain {
            return None;
        }
        Some(
            local_to_world(
                f64::from(info.locationX),
                f64::from(info.locationY),
                f64::from(info.locationZ),
            )
            .2,
        )
    }
}
impl Drop for TerrainProbe {
    fn drop(&mut self) {
        // SAFETY: this value exclusively owns this non-null probe handle.
        unsafe { XPLMDestroyProbe(self.0.as_ptr()) };
    }
}
