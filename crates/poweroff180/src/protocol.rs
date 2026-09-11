//! Stable 75-float snapshot and CSV schema shared by plugins and flight analysis.
use crate::Sample;
pub const LENGTH: usize = 75;
pub type Snapshot = [f32; LENGTH];
pub const NAMES: [&str; 29] = [
    "sim/time/total_flight_time_sec",
    "sim/flightmodel/position/y_agl",
    "sim/flightmodel/position/indicated_airspeed",
    "sim/flightmodel/position/groundspeed",
    "sim/flightmodel/position/vh_ind_fpm",
    "sim/flightmodel/position/psi",
    "sim/flightmodel/position/phi",
    "sim/flightmodel/position/theta",
    "sim/flightmodel/position/beta",
    "sim/flightmodel/position/Q",
    "sim/flightmodel/forces/g_nrml",
    "sim/cockpit2/engine/actuators/throttle_ratio_all",
    "sim/cockpit2/controls/flap_ratio",
    "sim/cockpit2/controls/elevator_trim",
    "sim/flightmodel2/controls/flap_handle_deploy_ratio",
    "sim/flightmodel2/gear/on_ground",
    "sim/flightmodel/weight/m_total",
    "sim/flightmodel/engine/ENGN_power",
    "sim/flightmodel/engine/ENGN_FF_",
    "sim/cockpit2/engine/indicators/engine_speed_rpm",
    "sim/joystick/yoke_roll_ratio",
    "sim/joystick/yoke_pitch_ratio",
    "sim/joystick/yoke_heading_ratio",
    "sim/flightmodel/position/local_vy",
    "sim/weather/aircraft/wind_now_speed_msc",
    "sim/weather/aircraft/wind_now_direction_degt",
    "sim/flightmodel/position/true_airspeed",
    "sim/cockpit2/gauges/indicators/slip_deg",
    "sim/operation/override/override_planepath",
];
pub const HEADER: &str = "sim_time,agl_ft,ias_kias,groundspeed_kt,vvi_fpm,heading_true_deg,bank_deg,pitch_deg,beta_deg,pitch_rate_raw,normal_g,throttle_ratio,flap_handle_ratio,elevator_trim_ratio,flap_actual_ratio,ground_any,mass_kg,engine_power_w,fuel_flow_kg_s,engine_rpm,aileron_input,elevator_input,rudder_input,vertical_speed_mps,wind_speed_mps,wind_direction_true_deg,true_airspeed_mps,native_slip_deg,override_path,runway_along_ft,runway_cross_ft,ground_track_true_deg,elevation_msl_ft,contact_latched,first_sim_time,first_along_ft,first_cross_ft,first_kias,first_indicated_fpm,first_physical_fpm,first_pitch_deg,first_normal_g,first_local_wind_kt,last_airborne_along_ft,last_airborne_sim_time,post_contact_max_g,post_contact_max_agl_ft,post_contact_air_frames,predicted_cross_ft,cross_accel_fps2,telemetry_ready,telemetry_version,phase_id,sequence,bank_command,pitch_command,flap_command,throttle_command,turn_lead_ft,desired_vertical_fps,vertical_accel_fps2,wind_pitch_rate_ff,roundout_pitch_rate_command,control_dt_s,cut_sim_time,roundout_sim_time,reason_id,native_running,configured,native_version,entry_gate_s,config_token,native_steps,heartbeat_age_s,run_token";
pub fn sample(s: &Snapshot) -> Sample {
    Sample {
        t: s[0] as f64,
        x: s[29] as f64,
        y: s[30] as f64,
        h: s[1] as f64,
        ias: s[2] as f64,
        tas_fps: s[26] as f64 * 3.280839895,
        gs_fps: s[3] as f64 * 1.68780986,
        heading: s[5] as f64,
        track: s[31] as f64,
        bank: s[6] as f64,
        pitch: s[7] as f64,
        vvi: s[4] as f64,
        vy: s[23] as f64 * 3.280839895,
        wind_kt: s[24] as f64 * 1.94384449,
        wind_dir: s[25] as f64,
        mass_lb: s[16] as f64 * 2.20462262185,
        throttle: s[11] as f64,
        ground: s[15] != 0.0,
    }
}
