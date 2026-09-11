//! Named, read-only instrument samples, captured once per HUD frame.
use std::collections::HashMap;
#[derive(Default)]
pub struct Values(pub HashMap<&'static str, f64>);
impl Values {
    pub fn get(&self, name: &str) -> f64 {
        self.0.get(name).copied().unwrap_or(f64::NAN)
    }
}
pub const NAMES: &[&str] = &[
    "sim/aircraft/view/acf_Vne",
    "sim/aircraft/view/acf_Vno",
    "sim/aircraft/view/acf_Vs",
    "sim/aircraft/view/acf_Vso",
    "sim/cockpit/radios/gps_cdi_sensitivity",
    "sim/cockpit/radios/gps_course_degtm",
    "sim/cockpit/radios/gps_fromto",
    "sim/cockpit/radios/gps_hdef_dot",
    "sim/cockpit/radios/gps_sequencing",
    "sim/cockpit2/autopilot/altitude_dial_ft",
    "sim/cockpit2/autopilot/heading_dial_deg_mag_pilot",
    "sim/cockpit2/engine/indicators/MPR_in_hg",
    "sim/cockpit2/gauges/actuators/barometer_setting_in_hg_pilot",
    "sim/cockpit2/gauges/indicators/airspeed_kts_pilot",
    "sim/cockpit2/gauges/indicators/altitude_ft_pilot",
    "sim/cockpit2/gauges/indicators/ground_track_mag_pilot",
    "sim/cockpit2/gauges/indicators/heading_AHARS_deg_mag_pilot",
    "sim/cockpit2/gauges/indicators/vvi_fpm_pilot",
    "sim/cockpit2/radios/indicators/gps_dme_distance_nm",
    "sim/flightmodel/position/latitude",
    "sim/flightmodel/position/longitude",
    "sim/graphics/view/field_of_view_deg",
    "sim/graphics/view/view_heading",
    "sim/graphics/view/view_pitch",
    "sim/graphics/view/view_roll",
    "sim/time/paused",
];
