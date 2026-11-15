//! `XPLMScenery.h` and `XPLMWeather.h`: terrain probes, objects and their
//! instances, magnetic variation, and weather at a location.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::CStr;
use std::hash::BuildHasherDefault;

use xplane_sdk_sys as xp;

use super::{AddressHash, Held};

thread_local! {
    /// Terrain probes [`create_probe`] returned that [`destroy_probe`] has not
    /// destroyed yet. A probe is a plain `usize` here; checking it against
    /// this registry keeps the probe calls safe without changing what they pass.
    static PROBES: RefCell<Held> = RefCell::new(Held::default());
    static OBJECTS: RefCell<Objects> = RefCell::new(Objects::default());
}

/// `XPLMGetMagneticVariation`, through [`crate::magnetic_variation`]; that
/// widens the SDK's `float` to `f64`, which narrows back exactly.
pub fn magnetic_variation(latitude: f64, longitude: f64) -> f32 {
    crate::magnetic_variation(latitude, longitude) as f32
}

/// `XPLMDegTrueToDegMagnetic`.
pub fn degrees_true_to_magnetic(heading: f32) -> f32 {
    // SAFETY: plain values in and out; no pointers.
    unsafe { xp::XPLMDegTrueToDegMagnetic(heading) }
}

/// `XPLMCreateProbe(kind)`: the new probe as an integer, or 0.
///
/// [`crate::TerrainProbe`] is the owned form, destroyed on drop.
pub fn create_probe(kind: xp::XPLMProbeType) -> usize {
    // SAFETY: a plain probe type value; X-Plane returns a new probe or null.
    let probe = unsafe { xp::XPLMCreateProbe(kind) } as usize;
    if probe != 0 {
        PROBES.with_borrow_mut(|probes| probes.add(probe));
    }
    probe
}

/// `XPLMDestroyProbe(probe)`.
///
/// # Panics
/// If `probe` is not a probe [`create_probe`] returned on this thread and not
/// yet destroyed.
pub fn destroy_probe(probe: usize) {
    assert!(
        PROBES.with_borrow_mut(|probes| probes.release(probe)),
        "not a live XPLM probe: {probe:#x}"
    );
    // SAFETY: `probe` was live (checked above) and that hold is now out of the registry, so
    // each probe X-Plane returned is destroyed exactly once.
    unsafe { xp::XPLMDestroyProbe(probe as xp::XPLMProbeRef) }
}

/// `XPLMProbeTerrainXYZ(probe, x, y, z, info)`; X-Plane reads `info.structSize`
/// and writes the result into `info`.
///
/// # Panics
/// If `probe` is not live (as [`destroy_probe`]) or `info.structSize` is
/// negative or larger than `XPLMProbeInfo_t`, which would let X-Plane write
/// past `info`.
pub fn probe_terrain_xyz(
    probe: usize,
    x: f32,
    y: f32,
    z: f32,
    info: &mut xp::XPLMProbeInfo_t,
) -> xp::XPLMProbeResult {
    assert!(
        PROBES.with_borrow(|probes| probes.count(probe) != 0),
        "not a live XPLM probe: {probe:#x}"
    );
    assert!(
        usize::try_from(info.structSize).is_ok_and(|size| size <= size_of::<xp::XPLMProbeInfo_t>()),
        "XPLMProbeInfo_t structSize out of range"
    );
    // SAFETY: the probe is live (checked); `info` is exclusively borrowed and X-Plane writes at
    // most `structSize` bytes of it, which is within the struct (checked).
    unsafe { xp::XPLMProbeTerrainXYZ(probe as xp::XPLMProbeRef, x, y, z, info) }
}

/// `XPLMGetWeatherAtLocation(latitude, longitude, altitude, info)`; X-Plane
/// reads `info.structSize` and writes the observation into `info`.
///
/// # Panics
/// If `info.structSize` is negative or larger than `XPLMWeatherInfo_t`.
pub fn weather_at_location(
    latitude: f64,
    longitude: f64,
    altitude: f64,
    info: &mut xp::XPLMWeatherInfo_t,
) -> i32 {
    assert!(
        usize::try_from(info.structSize)
            .is_ok_and(|size| size <= size_of::<xp::XPLMWeatherInfo_t>()),
        "XPLMWeatherInfo_t structSize out of range"
    );
    // SAFETY: `info` is exclusively borrowed and X-Plane writes at most `structSize` bytes of
    // it, which is within the struct (checked); plain values otherwise.
    unsafe { xp::XPLMGetWeatherAtLocation(latitude, longitude, altitude, info) }
}

/// Objects [`load_object`] returned, each with its loads not yet unloaded
/// (X-Plane counts references: a second load of a file returns the same
/// handle and needs its own unload), and the instances [`create_instance`]
/// made of them (instance -> object), not yet destroyed.
#[derive(Default)]
pub(super) struct Objects {
    pub(super) objects: Held,
    pub(super) instances: HashMap<usize, usize, BuildHasherDefault<AddressHash>>,
}

impl Objects {
    /// Records one successful load of `object`.
    pub(super) fn loaded(&mut self, object: usize) {
        self.objects.add(object);
    }

    /// Takes one load of `object` out of the registry, before its unload.
    ///
    /// # Panics
    /// If `object` holds no load, or this is its last load and an instance
    /// made of it is still alive. The registry is unchanged then.
    pub(super) fn release_load(&mut self, object: usize) {
        let loads = self.objects.count(object);
        assert!(loads != 0, "not a loaded XPLM object: {object:#x}");
        assert!(
            loads > 1 || !self.instances.values().any(|&of| of == object),
            "XPLM object unloaded with live instances"
        );
        self.objects.release(object);
    }
}

/// `XPLMLoadObject(path)`: the loaded object as an integer, or 0.
pub fn load_object(path: &CStr) -> usize {
    // SAFETY: `path` is NUL-terminated and outlives this synchronous load.
    let object = unsafe { xp::XPLMLoadObject(path.as_ptr()) } as usize;
    if object != 0 {
        OBJECTS.with_borrow_mut(|registry| registry.loaded(object));
    }
    object
}

/// `XPLMUnloadObject(object)`.
///
/// # Panics
/// If `object` is not a loaded object from [`load_object`] on this thread, or
/// this is its last unload and an instance made of it is still alive.
pub fn unload_object(object: usize) {
    OBJECTS.with_borrow_mut(|registry| registry.release_load(object));
    // SAFETY: the object held a load of ours (checked) that is now out of the registry, so
    // each successful load is unloaded exactly once; the unload that gives up our last
    // reference is refused while an instance made of the object is alive (checked).
    unsafe { xp::XPLMUnloadObject(object as xp::XPLMObjectRef) }
}

/// `XPLMCreateInstance(object, {NULL})`: an instance with no datarefs, as an
/// integer, or 0.
///
/// # Panics
/// If `object` is not a loaded object from [`load_object`] on this thread.
pub fn create_instance(object: usize) -> usize {
    assert!(
        OBJECTS.with_borrow(|registry| registry.objects.count(object) != 0),
        "not a loaded XPLM object: {object:#x}"
    );
    let mut datarefs = [std::ptr::null()];
    // SAFETY: the object is loaded (checked) and only `unload_object`, on this same thread
    // (the registry is per thread), can give up that load, so it stays loaded for the call;
    // `datarefs` is the empty null-terminated name list, live for the call.
    let instance =
        unsafe { xp::XPLMCreateInstance(object as xp::XPLMObjectRef, datarefs.as_mut_ptr()) }
            as usize;
    if instance != 0 {
        OBJECTS.with_borrow_mut(|registry| registry.instances.insert(instance, object));
    }
    instance
}

/// `XPLMDestroyInstance(instance)`.
///
/// # Panics
/// If `instance` is not a live instance from [`create_instance`] on this
/// thread.
pub fn destroy_instance(instance: usize) {
    assert!(
        OBJECTS.with_borrow_mut(|registry| registry.instances.remove(&instance).is_some()),
        "not a live XPLM instance: {instance:#x}"
    );
    // SAFETY: the instance was live (checked) and is now out of the registry, so it is
    // destroyed exactly once.
    unsafe { xp::XPLMDestroyInstance(instance as xp::XPLMInstanceRef) }
}

/// `XPLMInstanceSetPosition(instance, pose, &0.0)` for an instance made with
/// no datarefs, which reads no data values.
///
/// # Panics
/// If `instance` is not a live instance from [`create_instance`] on this
/// thread, or `pose.structSize` is negative or larger than `XPLMDrawInfo_t`.
pub fn instance_set_position(instance: usize, pose: &xp::XPLMDrawInfo_t) {
    assert!(
        OBJECTS.with_borrow(|registry| registry.instances.contains_key(&instance)),
        "not a live XPLM instance: {instance:#x}"
    );
    assert!(
        usize::try_from(pose.structSize).is_ok_and(|size| size <= size_of::<xp::XPLMDrawInfo_t>()),
        "XPLMDrawInfo_t structSize out of range"
    );
    // SAFETY: the instance is live (checked) and has no datarefs (`create_instance` passes an
    // empty list), so X-Plane reads no floats from the 'static `&0.` data pointer; it reads at
    // most `structSize` bytes of the borrowed pose, within the struct (checked).
    unsafe { xp::XPLMInstanceSetPosition(instance as xp::XPLMInstanceRef, pose, &0.) }
}
