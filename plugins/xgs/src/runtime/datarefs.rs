use xplane_plugin::DataRef;

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
    pub(super) ground_track: DataRef,
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
            ground_track: DataRef::required("sim/flightmodel/position/hpath")?,
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
            strut.read_f32_from(1, &mut compression);
            compression.into_iter().any(|value| value > 0.01)
        } else {
            self.gear_force.get_f32() != 0.0
        }
    }

    pub(super) fn nose_wheel_down(&self) -> bool {
        let Some(strut) = self.toliss_strut else {
            return false;
        };
        let mut compression = [0.0];
        let offset = if self.toliss_a340 { 3 } else { 0 };
        strut.read_f32_from(offset, &mut compression);
        compression[0] > 0.01
    }
}
