use crate::{local_to_world, world_to_local};
use std::marker::PhantomData;
use xplane_sdk_sys::{XPLMProbeInfo_t, xplm_ProbeHitTerrain, xplm_ProbeY};

/// Owns a vertical terrain probe. Creation, use and destruction stay on the
/// plugin thread; the raw handle is never exported or declared Send. The
/// probe calls go through [crate::xplm], whose per-thread probe registry
/// holds this probe from creation to drop.
pub struct TerrainProbe(usize, PhantomData<*mut ()>);
impl TerrainProbe {
    pub fn new() -> Result<Self, String> {
        match crate::xplm::create_probe(xplm_ProbeY) {
            0 => Err("terrain probe creation failed".to_owned()),
            probe => Ok(Self(probe, PhantomData)),
        }
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
        let hit = crate::xplm::probe_terrain_xyz(self.0, x as f32, y as f32, z as f32, &mut info);
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
        crate::xplm::destroy_probe(self.0);
    }
}
