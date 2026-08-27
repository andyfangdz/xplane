use std::ffi::{c_void, CString};

use xplane_sdk_sys::*;

#[derive(Copy, Clone)]
pub(super) struct DataRef(XPLMDataRef);

impl DataRef {
    fn find(name: &str) -> Option<Self> {
        let name = CString::new(name).expect("dataref name contains no NUL");
        // SAFETY: `name` is a live NUL-terminated string.
        let dataref = unsafe { XPLMFindDataRef(name.as_ptr()) };
        (!dataref.is_null()).then_some(Self(dataref))
    }

    fn required(name: &str) -> Result<Self, String> {
        Self::find(name).ok_or_else(|| format!("missing required dataref {name}"))
    }

    pub(super) fn i32(self) -> i32 {
        // SAFETY: this wrapper only contains a successful XPLM lookup.
        unsafe { XPLMGetDatai(self.0) }
    }

    pub(super) fn f32(self) -> f32 {
        // SAFETY: this wrapper only contains a successful XPLM lookup.
        unsafe { XPLMGetDataf(self.0) }
    }

    pub(super) fn read_f32(self, offset: i32, values: &mut [f32]) -> usize {
        let count = i32::try_from(values.len()).expect("dataref slice exceeds i32::MAX");
        // SAFETY: `values` is writable for `count` floats.
        let read = unsafe { XPLMGetDatavf(self.0, values.as_mut_ptr(), offset, count) };
        usize::try_from(read).unwrap_or(0).min(values.len())
    }

    pub(super) fn read_string(self, limit: usize) -> String {
        let mut bytes = vec![0_u8; limit];
        let count = i32::try_from(limit).expect("dataref string limit exceeds i32::MAX");
        // SAFETY: `bytes` is writable for `count` bytes.
        let read = unsafe { XPLMGetDatab(self.0, bytes.as_mut_ptr().cast::<c_void>(), 0, count) };
        bytes.truncate(usize::try_from(read).unwrap_or(0).min(limit));
        String::from_utf8_lossy(&bytes)
            .trim_end_matches('\0')
            .to_owned()
    }
}

pub(super) struct DataRefs {
    pub(super) gear_force: DataRef,
    pub(super) flight_time: DataRef,
    pub(super) aircraft_icao: DataRef,
    pub(super) aircraft_tail_number: DataRef,
    pub(super) latitude: DataRef,
    pub(super) longitude: DataRef,
    pub(super) elevation: DataRef,
    pub(super) height_agl: DataRef,
    pub(super) true_heading: DataRef,
    pub(super) local_vy: DataRef,
    pub(super) pitch: DataRef,
    pub(super) vr_enabled: DataRef,
    pub(super) replay: DataRef,
    pub(super) ias: DataRef,
    pub(super) toliss_vls: Option<DataRef>,
    pub(super) toliss_strut: Option<DataRef>,
    pub(super) toliss_a340: bool,
    pub(super) ias_multiplier: f32,
    pub(super) ias_unit: &'static str,
}

impl DataRefs {
    pub(super) fn find() -> Result<Self, String> {
        Ok(Self {
            gear_force: DataRef::required("sim/flightmodel/forces/fnrml_gear")?,
            flight_time: DataRef::required("sim/time/total_flight_time_sec")?,
            aircraft_icao: DataRef::required("sim/aircraft/view/acf_ICAO")?,
            aircraft_tail_number: DataRef::required("sim/aircraft/view/acf_tailnum")?,
            latitude: DataRef::required("sim/flightmodel/position/latitude")?,
            longitude: DataRef::required("sim/flightmodel/position/longitude")?,
            elevation: DataRef::required("sim/flightmodel/position/elevation")?,
            height_agl: DataRef::required("sim/flightmodel/position/y_agl")?,
            true_heading: DataRef::required("sim/flightmodel/position/true_psi")?,
            local_vy: DataRef::required("sim/flightmodel/position/local_vy")?,
            pitch: DataRef::required("sim/flightmodel/position/theta")?,
            vr_enabled: DataRef::required("sim/graphics/VR/enabled")?,
            replay: DataRef::required("sim/time/is_in_replay")?,
            ias: DataRef::required("sim/flightmodel/position/indicated_airspeed")?,
            toliss_vls: None,
            toliss_strut: None,
            toliss_a340: false,
            ias_multiplier: 1.0,
            ias_unit: "kts",
        })
    }

    pub(super) fn refresh_aircraft_specific(&mut self, icao: &str) {
        self.ias = DataRef::find("sim/flightmodel/position/indicated_airspeed")
            .expect("standard indicated airspeed dataref disappeared");
        self.ias_multiplier = if icao == "AS21" { 1.852 } else { 1.0 };
        self.ias_unit = if icao == "AS21" { "km/h" } else { "kts" };
        self.toliss_vls = DataRef::find("toliss_airbus/pfdoutputs/general/VLS_value");
        self.toliss_a340 = icao.starts_with("A34");
        self.toliss_strut = None;
        if self.toliss_vls.is_some() {
            if let Some(ias) = DataRef::find("AirbusFBW/IASCapt") {
                self.ias = ias;
            }
            self.toliss_strut = if self.toliss_a340 {
                DataRef::find("AirbusFBW/GearStrutCompressDist_m")
            } else {
                DataRef::find("sim/flightmodel2/gear/tire_vertical_deflection_mtr")
            };
        }
    }

    pub(super) fn on_ground(&self) -> bool {
        if let Some(strut) = self.toliss_strut {
            let mut compression = [0.0; 2];
            strut.read_f32(1, &mut compression);
            compression.into_iter().any(|value| value > 0.01)
        } else {
            self.gear_force.f32() != 0.0
        }
    }

    pub(super) fn nose_wheel_down(&self) -> bool {
        let Some(strut) = self.toliss_strut else {
            return false;
        };
        let mut compression = [0.0];
        let offset = if self.toliss_a340 { 3 } else { 0 };
        strut.read_f32(offset, &mut compression);
        compression[0] > 0.01
    }
}
