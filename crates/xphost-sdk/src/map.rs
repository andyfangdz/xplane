//! Deterministic panel-map contracts without scenery, weather, or a GPU.
//! One synthetic host frame stands in for each display's initial tile load.
use crate::panel::intersection;
use crate::state::{State, handle, index, output, state};
use serde_json::{Value, json};
use xplane_sdk_sys::*;

pub struct Map {
    pilot: i32,
    created: u64,
}

struct View {
    rect: [i32; 4],
    layers: i32,
    datum: [f64; 2],
    center: [f64; 2],
    radius: f64,
    range: f64,
    orientation: i32,
    up: f64,
    pilot: i32,
    defaults: bool,
    terrain_settings: Value,
}

// FFI records start with an int byte count. Read just that prefix before
// accessing the rest; the map SDK rejects unrecognized input structure sizes.
unsafe fn sized<T>(ptr: *const T) -> bool {
    !ptr.is_null() && unsafe { ptr.cast::<i32>().read() } == size_of::<T>() as i32
}

fn geographic(lat: f64, lon: f64) -> bool {
    (-90.0..=90.0).contains(&lat) && (-180.0..=180.0).contains(&lon)
}

impl View {
    unsafe fn read(
        s: &State,
        map: &Map,
        info: *const XPLMMapDrawInfo_t,
        custom: *const XPLMMapCustomData_t,
    ) -> Option<Self> {
        if !unsafe { sized(info) } {
            return None;
        }
        let i = unsafe { &*info };
        if i.left >= i.right || i.bottom >= i.top {
            return None;
        }
        let mut v = Self {
            pilot: map.pilot,
            rect: [i.left, i.top, i.right, i.bottom],
            layers: i.layers,
            datum: [
                s.store.scalar("sim/flightmodel/position/latitude"),
                s.store.scalar("sim/flightmodel/position/longitude"),
            ],
            center: [
                (i.left as f64 + i.right as f64) * 0.5,
                (i.bottom as f64 + i.top as f64) * 0.5,
            ],
            radius: (i.right as f64 - i.left as f64).min(i.top as f64 - i.bottom as f64) * 0.5,
            range: 0.0,
            orientation: 0,
            up: 0.0,
            defaults: custom.is_null(),
            terrain_settings: Value::Null,
        };
        if custom.is_null() {
            // Use side-specific EFIS inputs when seeded. An asset-free fixture
            // without these datarefs gets a centered north-up 40 NM display.
            let (range, mode, altitude) = if map.pilot == 0 {
                (
                    "sim/cockpit2/EFIS/map_range_nm",
                    "sim/cockpit2/EFIS/map_mode",
                    "sim/cockpit2/gauges/indicators/altitude_ft_pilot",
                )
            } else {
                (
                    "sim/cockpit2/EFIS/map_range_nm_copilot",
                    "sim/cockpit2/EFIS/map_mode_copilot",
                    "sim/cockpit2/gauges/indicators/altitude_ft_copilot",
                )
            };
            v.range = s.store.scalar(range);
            if v.range <= 0.0 {
                v.range = 40.0;
            }
            v.orientation = i32::from(s.store.find(mode).is_some() && s.store.scalar(mode) != 4.0);
            v.terrain_settings = json!({"aircraft_alt_ft":s.store.scalar(altitude)});
        } else {
            if !unsafe { sized(custom) } {
                return None;
            }
            let c = unsafe { &*custom };
            v.datum = [c.datLat.into(), c.datLon.into()];
            v.center = [c.centerX.into(), c.centerY.into()];
            v.radius = c.roseRadius.into();
            v.range = c.mapRange.into();
            v.orientation = c.orientation;
            v.up = c.trueRotation.into();
            v.terrain_settings = json!({"aircraft_alt_ft":c.acfAlt,"warn_ft":c.terrainWarn,
                "caution_ft":c.terrainCaution,"gear_down":c.gearDown,"nearest_runway_ft":c.nearestRwyElev,
                "egpws_brightness":c.egpwsBrightness,"egpws_style":c.egpwsStyle});
        }
        v.up = match v.orientation {
            0 => 0.0,
            1 => s.store.scalar("sim/flightmodel/position/hpath"),
            2 => s.store.scalar("sim/flightmodel/position/true_psi"),
            3 => v.up,
            _ => return None,
        };
        (geographic(v.datum[0], v.datum[1])
            && v.up.is_finite()
            && v.radius > 0.0
            && v.range.is_finite()
            && v.range > 0.0)
            .then_some(v)
    }

    fn px_per_meter(&self) -> f64 {
        self.radius / (self.range * 1852.0)
    }

    // Spherical transverse Mercator about the datum's meridian. Only use the
    // local hemisphere, where the projection is single-valued and continuous.
    fn project(&self, lat: f64, lon: f64) -> Option<[f32; 2]> {
        if !geographic(lat, lon) {
            return None;
        }
        let lat = lat.to_radians();
        let dl = (lon - self.datum[1]).to_radians();
        if dl.cos() <= 1e-12 {
            return None;
        }
        let east = (lat.cos() * dl.sin()).atanh();
        let north = lat.tan().atan2(dl.cos()) - self.datum[0].to_radians();
        let (s, c) = self.up.to_radians().sin_cos();
        let k = 6_371_000.0 * self.px_per_meter();
        let xy = [
            (self.center[0] + k * (east * c - north * s)) as f32,
            (self.center[1] + k * (east * s + north * c)) as f32,
        ];
        xy.iter().all(|v| v.is_finite()).then_some(xy)
    }

    fn unproject(&self, x: f32, y: f32) -> Option<[f64; 2]> {
        let (s, c) = self.up.to_radians().sin_cos();
        let dx = x as f64 - self.center[0];
        let dy = y as f64 - self.center[1];
        let k = 6_371_000.0 * self.px_per_meter();
        let east = (dx * c + dy * s) / k;
        let d = self.datum[0].to_radians() + (dy * c - dx * s) / k;
        if !east.is_finite()
            || !(-std::f64::consts::FRAC_PI_2..=std::f64::consts::FRAC_PI_2).contains(&d)
            || !east.cosh().is_finite()
        {
            return None;
        }
        let lat = (d.sin() / east.cosh()).asin().to_degrees();
        let lon = self.datum[1] + east.sinh().atan2(d.cos()).to_degrees();
        Some([lat, (lon + 180.0).rem_euclid(360.0) - 180.0])
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn XPLMCreateMapDisplay(params: *mut XPLMCreateMap_t) -> XPLMMapDisplayRef {
    if !unsafe { sized(params) } {
        return std::ptr::null_mut();
    }
    let pilot = unsafe { (*params).pilotIndex };
    if !(0..=1).contains(&pilot) {
        return std::ptr::null_mut();
    }
    let mut s = state();
    let created = s.frame;
    let id = s.handle();
    s.maps.insert(id, Map { pilot, created });
    handle(id)
}

#[unsafe(no_mangle)]
pub extern "C" fn XPLMDestroyMapDisplay(map: XPLMMapDisplayRef) {
    let mut s = state();
    let id = index(map);
    if s.maps.remove(&id).is_some() {
        s.panel.retire_resource(id);
    }
}

unsafe fn ready_view(
    s: &State,
    id: usize,
    info: *const XPLMMapDrawInfo_t,
    custom: *const XPLMMapCustomData_t,
) -> Option<View> {
    let m = s.maps.get(&id)?;
    if m.created >= s.frame {
        return None;
    }
    unsafe { View::read(s, m, info, custom) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn XPLMMapDisplayDrawIn(
    map: XPLMMapDisplayRef,
    info: *mut XPLMMapDrawInfo_t,
    custom: *mut XPLMMapCustomData_t,
) {
    let mut s = state();
    let id = index(map);
    let Some(m) = s.maps.get(&id) else {
        s.panel.error("map", "unknown map display");
        return;
    };
    if m.created >= s.frame {
        return;
    }
    let Some(v) = (unsafe { View::read(&s, m, info, custom) }) else {
        s.panel.error("map", "invalid map view");
        return;
    };
    s.panel.draw_resource("map", id, move |drawing| {
        if drawing.transform.rotated { return Err("rotated map display"); }
        let bounds = drawing.transform.rect(v.rect.map(|n| n as f32));
        let scissor = drawing.clip.map_or(bounds, |clip| intersection(clip,bounds));
        if scissor[0] < scissor[2] && scissor[1] < scissor[3] {
            drawing.record("map", json!({"map":id,"pilot_index":v.pilot,"layers":v.layers,"rect":v.rect,
                "bounds":bounds,"scissor":scissor,"datum":v.datum,"center":v.center,"range_nm":v.range,
                "rose_radius":v.radius,"orientation":v.orientation,"true_up_degrees":v.up,
                "simulator_defaults":v.defaults,"terrain_settings":v.terrain_settings}));
        }
        Ok(())
    });
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn XPLMMapDisplayProject(
    map: XPLMMapDisplayRef,
    info: *mut XPLMMapDrawInfo_t,
    custom: *mut XPLMMapCustomData_t,
    lat: f64,
    lon: f64,
    x: *mut f32,
    y: *mut f32,
) -> i32 {
    let Some([px, py]) = (unsafe { ready_view(&state(), index(map), info, custom) })
        .and_then(|v| v.project(lat, lon))
    else {
        return 0;
    };
    unsafe {
        output(x, px);
        output(y, py);
    }
    1
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn XPLMMapDisplayUnproject(
    map: XPLMMapDisplayRef,
    info: *mut XPLMMapDrawInfo_t,
    custom: *mut XPLMMapCustomData_t,
    x: f32,
    y: f32,
    lat: *mut f64,
    lon: *mut f64,
) -> i32 {
    let Some([latitude, longitude]) =
        (unsafe { ready_view(&state(), index(map), info, custom) }).and_then(|v| v.unproject(x, y))
    else {
        return 0;
    };
    unsafe {
        output(lat, latitude);
        output(lon, longitude);
    }
    1
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn XPLMMapDisplayScaleMeter(
    map: XPLMMapDisplayRef,
    info: *mut XPLMMapDrawInfo_t,
    custom: *mut XPLMMapCustomData_t,
    x: f32,
    y: f32,
) -> f32 {
    let Some(v) = (unsafe { ready_view(&state(), index(map), info, custom) }) else {
        return 0.;
    };
    let Some([lat, lon]) = v.unproject(x, y) else {
        return 0.;
    };
    let b = lat.to_radians().cos() * (lon - v.datum[1]).to_radians().sin();
    (v.px_per_meter() / (1.0 - b * b).sqrt()) as f32
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn XPLMMapDisplayGetNorthHeading(
    map: XPLMMapDisplayRef,
    info: *mut XPLMMapDrawInfo_t,
    custom: *mut XPLMMapCustomData_t,
    x: f32,
    y: f32,
) -> f32 {
    let Some(v) = (unsafe { ready_view(&state(), index(map), info, custom) }) else {
        return 0.;
    };
    let Some([lat, lon]) = v.unproject(x, y) else {
        return 0.;
    };
    let dl = (lon - v.datum[1]).to_radians();
    let convergence = (lat.to_radians().sin() * dl.sin())
        .atan2(dl.cos())
        .to_degrees();
    (-v.up - convergence).rem_euclid(360.0) as f32
}

#[unsafe(no_mangle)]
pub extern "C" fn XPLMMapDisplayGetTerrainAltitudes(
    _: XPLMMapDisplayRef,
    _: *mut f32,
    _: *mut f32,
) -> i32 {
    // No scenery is loaded by this host. The documented unavailable case leaves
    // both output pointers unchanged, even when EGPWS was requested.
    0
}
