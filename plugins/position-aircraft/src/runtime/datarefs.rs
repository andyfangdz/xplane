use std::ffi::CString;

use xplane_sdk_sys::*;

#[derive(Copy, Clone)]
pub(in crate::runtime) struct DataRef(XPLMDataRef);

impl DataRef {
    fn required(name: &str) -> Result<Self, String> {
        let name_c = CString::new(name).unwrap();
        // SAFETY: `name_c` is NUL-terminated and lives for the duration of the
        // call. A non-null XPLM dataref remains owned by X-Plane.
        let data_ref = unsafe { XPLMFindDataRef(name_c.as_ptr()) };
        if data_ref.is_null() {
            Err(format!("Missing required dataref: {name}"))
        } else {
            Ok(Self(data_ref))
        }
    }

    pub(in crate::runtime) fn get_i32(self) -> i32 {
        // SAFETY: `DataRef` can only be constructed from a successful XPLM lookup.
        unsafe { XPLMGetDatai(self.0) }
    }

    pub(in crate::runtime) fn get_f32(self) -> f32 {
        // SAFETY: `DataRef` can only be constructed from a successful XPLM lookup.
        unsafe { XPLMGetDataf(self.0) }
    }

    pub(in crate::runtime) fn get_f64(self) -> f64 {
        // SAFETY: `DataRef` can only be constructed from a successful XPLM lookup.
        unsafe { XPLMGetDatad(self.0) }
    }

    pub(in crate::runtime) fn read_f32(self, values: &mut [f32]) -> usize {
        let count = i32::try_from(values.len()).expect("XPLM array length exceeds i32::MAX");
        // SAFETY: the slice supplies a valid writable buffer of the advertised length.
        let read = unsafe { XPLMGetDatavf(self.0, values.as_mut_ptr(), 0, count) };
        usize::try_from(read).unwrap_or(0).min(values.len())
    }

    pub(in crate::runtime) fn read_i32(self, values: &mut [i32]) -> usize {
        let count = i32::try_from(values.len()).expect("XPLM array length exceeds i32::MAX");
        // SAFETY: the slice supplies a valid writable buffer of the advertised length.
        let read = unsafe { XPLMGetDatavi(self.0, values.as_mut_ptr(), 0, count) };
        usize::try_from(read).unwrap_or(0).min(values.len())
    }

    pub(in crate::runtime) fn set_i32(self, value: i32) {
        // SAFETY: `DataRef` can only be constructed from a successful XPLM lookup.
        unsafe { XPLMSetDatai(self.0, value) }
    }

    pub(in crate::runtime) fn set_f32(self, value: f32) {
        // SAFETY: `DataRef` can only be constructed from a successful XPLM lookup.
        unsafe { XPLMSetDataf(self.0, value) }
    }

    pub(in crate::runtime) fn set_f64(self, value: f64) {
        // SAFETY: `DataRef` can only be constructed from a successful XPLM lookup.
        unsafe { XPLMSetDatad(self.0, value) }
    }

    pub(in crate::runtime) fn write_f32(self, values: &[f32]) {
        let count = i32::try_from(values.len()).expect("XPLM array length exceeds i32::MAX");
        // SAFETY: the slice supplies a valid readable buffer of the advertised length.
        unsafe { XPLMSetDatavf(self.0, values.as_ptr().cast_mut(), 0, count) }
    }
}

pub(in crate::runtime) struct DataRefs {
    pub(in crate::runtime) latitude: DataRef,
    pub(in crate::runtime) longitude: DataRef,
    pub(in crate::runtime) elevation: DataRef,
    pub(in crate::runtime) theta: DataRef,
    pub(in crate::runtime) phi: DataRef,
    pub(in crate::runtime) psi: DataRef,
    pub(in crate::runtime) magvar: DataRef,
    pub(in crate::runtime) ias: DataRef,
    pub(in crate::runtime) local_x: DataRef,
    pub(in crate::runtime) local_y: DataRef,
    pub(in crate::runtime) local_z: DataRef,
    pub(in crate::runtime) local_vx: DataRef,
    pub(in crate::runtime) local_vy: DataRef,
    pub(in crate::runtime) local_vz: DataRef,
    pub(in crate::runtime) rate_p: DataRef,
    pub(in crate::runtime) rate_q: DataRef,
    pub(in crate::runtime) rate_r: DataRef,
    pub(in crate::runtime) quaternion: DataRef,
    pub(in crate::runtime) throttles: DataRef,
    pub(in crate::runtime) flaps: DataRef,
    pub(in crate::runtime) gear: DataRef,
    pub(in crate::runtime) ap_mode: DataRef,
    pub(in crate::runtime) ap_altitude: DataRef,
    pub(in crate::runtime) ap_vvi: DataRef,
    pub(in crate::runtime) ap_heading: DataRef,
    pub(in crate::runtime) ap_airspeed: DataRef,
    pub(in crate::runtime) ap_state: DataRef,
    pub(in crate::runtime) ap_heading_roll_mode: DataRef,
    pub(in crate::runtime) vr_enabled: DataRef,
    pub(in crate::runtime) projection_matrix: DataRef,
    pub(in crate::runtime) modelview_matrix: DataRef,
    pub(in crate::runtime) viewport: DataRef,
}

impl DataRefs {
    pub(super) fn find() -> Result<Self, String> {
        Ok(Self {
            latitude: DataRef::required("sim/flightmodel/position/latitude")?,
            longitude: DataRef::required("sim/flightmodel/position/longitude")?,
            elevation: DataRef::required("sim/flightmodel/position/elevation")?,
            theta: DataRef::required("sim/flightmodel/position/theta")?,
            phi: DataRef::required("sim/flightmodel/position/phi")?,
            psi: DataRef::required("sim/flightmodel/position/psi")?,
            magvar: DataRef::required("sim/flightmodel/position/magnetic_variation")?,
            ias: DataRef::required("sim/flightmodel/position/indicated_airspeed")?,
            local_x: DataRef::required("sim/flightmodel/position/local_x")?,
            local_y: DataRef::required("sim/flightmodel/position/local_y")?,
            local_z: DataRef::required("sim/flightmodel/position/local_z")?,
            local_vx: DataRef::required("sim/flightmodel/position/local_vx")?,
            local_vy: DataRef::required("sim/flightmodel/position/local_vy")?,
            local_vz: DataRef::required("sim/flightmodel/position/local_vz")?,
            rate_p: DataRef::required("sim/flightmodel/position/P")?,
            rate_q: DataRef::required("sim/flightmodel/position/Q")?,
            rate_r: DataRef::required("sim/flightmodel/position/R")?,
            quaternion: DataRef::required("sim/flightmodel/position/q")?,
            throttles: DataRef::required("sim/flightmodel/engine/ENGN_thro")?,
            flaps: DataRef::required("sim/flightmodel/controls/flaprqst")?,
            gear: DataRef::required("sim/cockpit/switches/gear_handle_status")?,
            ap_mode: DataRef::required("sim/cockpit/autopilot/autopilot_mode")?,
            ap_altitude: DataRef::required("sim/cockpit/autopilot/altitude")?,
            ap_vvi: DataRef::required("sim/cockpit/autopilot/vertical_velocity")?,
            ap_heading: DataRef::required("sim/cockpit/autopilot/heading_mag")?,
            ap_airspeed: DataRef::required("sim/cockpit/autopilot/airspeed")?,
            ap_state: DataRef::required("sim/cockpit/autopilot/autopilot_state")?,
            ap_heading_roll_mode: DataRef::required("sim/cockpit/autopilot/heading_roll_mode")?,
            vr_enabled: DataRef::required("sim/graphics/VR/enabled")?,
            projection_matrix: DataRef::required("sim/graphics/view/projection_matrix")?,
            modelview_matrix: DataRef::required("sim/graphics/view/modelview_matrix")?,
            viewport: DataRef::required("sim/graphics/view/viewport")?,
        })
    }
}
