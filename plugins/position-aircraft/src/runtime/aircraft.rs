use std::f64::consts::PI;

use crate::pad::{normalize_heading, AutopilotData, Field, Form, PadData};
use uom::si::{
    f64::Length, f64::Velocity, length::foot, length::meter, velocity::foot_per_minute,
    velocity::knot, velocity::meter_per_second,
};
use xplane_plugin::{command_once, magnetic_variation, world_to_local};

use super::state::{PendingReapply, PluginState};

const MAX_ENGINES: usize = 16;
/// ISA sea-level air density, kg/m³. Indicated airspeed equals true airspeed here.
const SEA_LEVEL_DENSITY: f64 = 1.225;
/// Share of the propeller redline used when a PAD does not record prop speed.
/// For the TorqueSim SR20 this is 2430 RPM, close to its measured 2450 RPM downwind.
const DEFAULT_PROP_REDLINE_FRACTION: f64 = 0.9;

impl PluginState {
    pub(in crate::runtime) fn capture_current(&mut self) -> PadData {
        let mut throttle = 0.0_f32;
        self.datarefs
            .throttles
            .read_f32(std::slice::from_mut(&mut throttle));
        let mut prop_rpm = 0.0_f32;
        self.datarefs
            .prop_speed_rpm
            .read_f32(std::slice::from_mut(&mut prop_rpm));
        let data = PadData {
            latitude: self.datarefs.latitude.get_f64(),
            longitude: self.datarefs.longitude.get_f64(),
            altitude: Length::new::<meter>(self.datarefs.elevation.get_f64()).get::<foot>(),
            heading: normalize_heading(
                self.datarefs.psi.get_f32() as f64 + self.datarefs.magvar.get_f32() as f64,
            ),
            pitch: self.datarefs.theta.get_f32() as f64,
            roll: self.datarefs.phi.get_f32() as f64,
            speed: self.datarefs.ias.get_f32() as f64,
            throttle: throttle as f64,
            flaps: self.datarefs.flaps.get_f32() as f64,
            gear: self.datarefs.gear.get_i32(),
            prop_rpm: (prop_rpm > 1.0).then_some(prop_rpm as f64),
            elevator_trim: Some(self.datarefs.elevator_trim.get_f32() as f64),
            aileron_trim: Some(self.datarefs.aileron_trim.get_f32() as f64),
            rudder_trim: Some(self.datarefs.rudder_trim.get_f32() as f64),
            vertical_speed: Some(
                Velocity::new::<meter_per_second>(self.datarefs.local_vy.get_f32() as f64)
                    .get::<foot_per_minute>(),
            ),
            sideslip: Some(self.datarefs.beta.get_f32() as f64),
            use_ap: self.form.use_ap,
            ap: AutopilotData {
                mode: self.datarefs.ap_mode.get_i32(),
                altitude: self.datarefs.ap_altitude.get_f32() as f64,
                vertical_velocity: self.datarefs.ap_vvi.get_f32() as f64,
                heading: self.datarefs.ap_heading.get_f32() as f64,
                airspeed: self.datarefs.ap_airspeed.get_f32() as f64,
                state: self.datarefs.ap_state.get_i32(),
                heading_roll_mode: self.datarefs.ap_heading_roll_mode.get_i32(),
            },
        };
        let save_name = self.form.value(Field::SaveName).to_owned();
        self.form = Form::from_data(&data, &save_name);
        self.status = "Captured current aircraft data".to_owned();
        data
    }

    pub(in crate::runtime) fn position_loaded(&mut self) {
        let data = match self.form.to_data() {
            Ok(data) => data,
            Err(error) => {
                self.status = error;
                return;
            }
        };
        self.position_data(data.clone());
        self.status = format!(
            "Positioned: {:.5}, {:.5} at {:.0} ft",
            data.latitude, data.longitude, data.altitude
        );
    }

    pub(in crate::runtime) fn position_data(&mut self, data: PadData) {
        let (x, y, z) = world_to_local(
            data.latitude,
            data.longitude,
            Length::new::<foot>(data.altitude).get::<meter>(),
        );
        self.datarefs.local_x.set_f64(x);
        self.datarefs.local_y.set_f64(y);
        self.datarefs.local_z.set_f64(z);
        self.apply_aircraft_state(&data);
        self.release_parking_brake();
        self.pending = Some(PendingReapply {
            data,
            wait_frames: 2,
            remaining_frames: 6,
        });
    }

    /// Releases a parking brake left set on the ground so the aircraft does
    /// not later land with locked wheels. Aircraft plugins may drive the brake
    /// from their own control and ignore X-Plane's ratio and command; the
    /// TorqueSim SR20 only honors its own command.
    fn release_parking_brake(&self) {
        self.datarefs.parking_brake.set_f32(0.0);
        command_once("sim/flight_controls/park_brake_release");
        command_once("afm/sr/cmd/misc/parking_brake_off");
    }

    /// Establishes the saved flight state in the running flight model. Besides
    /// attitude and velocity this settles the systems X-Plane would otherwise
    /// move gradually after a reposition from the ground: flaps travelling
    /// from their landing setting and a propeller spooling up from idle.
    pub(in crate::runtime) fn apply_aircraft_state(&self, data: &PadData) {
        let true_heading = true_heading(data);
        let psi = true_heading.to_radians() * 0.5;
        let theta = data.pitch.to_radians() * 0.5;
        let phi = data.roll.to_radians() * 0.5;
        let (sin_psi, cos_psi) = psi.sin_cos();
        let (sin_theta, cos_theta) = theta.sin_cos();
        let (sin_phi, cos_phi) = phi.sin_cos();
        let q = [
            (cos_psi * cos_theta * cos_phi + sin_psi * sin_theta * sin_phi) as f32,
            (cos_psi * cos_theta * sin_phi - sin_psi * sin_theta * cos_phi) as f32,
            (cos_psi * sin_theta * cos_phi + sin_psi * cos_theta * sin_phi) as f32,
            (-cos_psi * sin_theta * sin_phi + sin_psi * cos_theta * cos_phi) as f32,
        ];
        self.datarefs.quaternion.write_f32(&q);

        // The PAD speed is indicated airspeed through the air mass. Convert it
        // with the local density and add the wind so the aircraft starts at
        // the saved airspeed and sideslip rather than settling into them.
        let airspeed = true_airspeed(
            Velocity::new::<knot>(data.speed),
            self.datarefs.air_density.get_f32() as f64,
        );
        let wind = [
            self.datarefs.wind_x.get_f32() as f64,
            self.datarefs.wind_y.get_f32() as f64,
            self.datarefs.wind_z.get_f32() as f64,
        ];
        let airspeed_mps = airspeed.get::<meter_per_second>();
        let flight_path = flight_path_deg(
            data.pitch,
            data.vertical_speed.map(Velocity::new::<foot_per_minute>),
            airspeed_mps,
            wind[1],
        );
        let track = air_track_deg(true_heading, data.sideslip);
        let [vx, vy, vz] = inertial_velocity(track, flight_path, airspeed_mps, wind);
        self.datarefs.local_vx.set_f32(vx as f32);
        self.datarefs.local_vy.set_f32(vy as f32);
        self.datarefs.local_vz.set_f32(vz as f32);
        self.datarefs.rate_p.set_f32(0.0);
        self.datarefs.rate_q.set_f32(0.0);
        self.datarefs.rate_r.set_f32(0.0);

        let throttles = [data.throttle.clamp(0.0, 1.0) as f32; MAX_ENGINES];
        self.datarefs.throttles.write_f32(&throttles);
        let flaps = data.flaps.clamp(0.0, 1.0) as f32;
        for flap in [
            self.datarefs.flaps,
            self.datarefs.flap1_ratio,
            self.datarefs.flap2_ratio,
            self.datarefs.flap1_deploy_ratio,
            self.datarefs.flap2_deploy_ratio,
            self.datarefs.flap_system_deploy_ratio,
        ] {
            flap.set_f32(flaps);
        }
        self.datarefs
            .gear
            .set_i32(if data.gear != 0 { 1 } else { 0 });
        for (trim, dataref) in [
            (data.elevator_trim, self.datarefs.elevator_trim),
            (data.aileron_trim, self.datarefs.aileron_trim),
            (data.rudder_trim, self.datarefs.rudder_trim),
        ] {
            if let Some(trim) = trim {
                dataref.set_f32(trim.clamp(-1.0, 1.0) as f32);
            }
        }
        self.establish_prop_speed(data);

        if data.use_ap {
            self.datarefs.ap_altitude.set_f32(data.ap.altitude as f32);
            self.datarefs
                .ap_vvi
                .set_f32(data.ap.vertical_velocity as f32);
            self.datarefs
                .ap_heading
                .set_f32(normalize_heading(data.ap.heading) as f32);
            self.datarefs.ap_airspeed.set_f32(data.ap.airspeed as f32);
            self.datarefs
                .ap_heading_roll_mode
                .set_i32(data.ap.heading_roll_mode);
            self.datarefs.ap_state.set_i32(data.ap.state);
            self.datarefs.ap_mode.set_i32(data.ap.mode);
        }
    }

    fn establish_prop_speed(&self, data: &PadData) {
        let Some(target) =
            prop_target_rad_s(data.prop_rpm, self.datarefs.prop_redline.get_f32() as f64)
        else {
            return;
        };
        let count = usize::try_from(self.datarefs.engine_count.get_i32())
            .unwrap_or(0)
            .min(MAX_ENGINES);
        if count == 0 {
            return;
        }
        let mut types = [0_i32; MAX_ENGINES];
        let mut running = [0_i32; MAX_ENGINES];
        let mut gear_ratios = [1.0_f32; MAX_ENGINES];
        let mut prop = [0.0_f32; MAX_ENGINES];
        let mut engine = [0.0_f32; MAX_ENGINES];
        self.datarefs.engine_type.read_i32(&mut types[..count]);
        self.datarefs.engine_running.read_i32(&mut running[..count]);
        self.datarefs
            .prop_gear_ratio
            .read_f32(&mut gear_ratios[..count]);
        self.datarefs.prop_rotation.read_f32(&mut prop[..count]);
        self.datarefs.engine_rotation.read_f32(&mut engine[..count]);
        for index in 0..count {
            if running[index] == 0 || !drives_propeller(types[index]) {
                continue;
            }
            let gear_ratio = if gear_ratios[index] > 0.0 {
                gear_ratios[index]
            } else {
                1.0
            };
            prop[index] = target as f32;
            engine[index] = target as f32 * gear_ratio;
        }
        self.datarefs.prop_rotation.write_f32(&prop[..count]);
        self.datarefs.engine_rotation.write_f32(&engine[..count]);
    }

    pub(in crate::runtime) fn toggle_window(&mut self) {
        let Some(window) = self.window.as_ref() else {
            return;
        };
        if window.is_visible() {
            window.set_visible(false);
            if let Some(ui) = self.ui.as_mut() {
                ui.hide();
            }
            window.set_keyboard_focus(false);
        } else {
            window.set_visible(true);
            window.bring_to_front();
        }
    }
}

/// PAD headings are magnetic; convert with the variation at the target.
fn true_heading(data: &PadData) -> f64 {
    normalize_heading(data.heading - magnetic_variation(data.latitude, data.longitude))
}

/// True airspeed for an indicated (equivalent) airspeed at the given air density.
fn true_airspeed(indicated: Velocity, density: f64) -> Velocity {
    let density = if density.is_finite() && density > 0.05 {
        density
    } else {
        SEA_LEVEL_DENSITY
    };
    indicated * (SEA_LEVEL_DENSITY / density).sqrt()
}

/// Climb angle through the air mass, degrees. A recorded vertical speed sets
/// it, leaving the saved pitch above the path as the angle of attack; without
/// one the aircraft flies along its pitch.
fn flight_path_deg(
    pitch_deg: f64,
    vertical_speed: Option<Velocity>,
    airspeed_mps: f64,
    wind_up_mps: f64,
) -> f64 {
    match vertical_speed {
        Some(vertical_speed) if airspeed_mps > 1.0 => {
            let climb = (vertical_speed.get::<meter_per_second>() - wind_up_mps) / airspeed_mps;
            climb.clamp(-1.0, 1.0).asin().to_degrees()
        }
        _ => pitch_deg,
    }
}

/// Direction flown through the air mass, degrees true. X-Plane's sideslip
/// (beta) is the heading minus this path.
fn air_track_deg(true_heading_deg: f64, sideslip_deg: Option<f64>) -> f64 {
    normalize_heading(true_heading_deg - sideslip_deg.unwrap_or(0.0))
}

/// Local OpenGL velocity (east, up, south) for flight along the track and
/// climb angle at the given true airspeed through an air mass moving with `wind`.
fn inertial_velocity(
    track_deg: f64,
    climb_deg: f64,
    airspeed_mps: f64,
    wind: [f64; 3],
) -> [f64; 3] {
    let track = track_deg.to_radians();
    let climb = climb_deg.to_radians();
    let horizontal = airspeed_mps * climb.cos();
    [
        horizontal * track.sin() + wind[0],
        airspeed_mps * climb.sin() + wind[1],
        -horizontal * track.cos() + wind[2],
    ]
}

/// Propeller speed to establish, rad/s: the PAD's recorded RPM, or a fixed
/// share of the redline for PADs saved before prop speed was recorded.
fn prop_target_rad_s(recorded_rpm: Option<f64>, redline_rad_s: f64) -> Option<f64> {
    match recorded_rpm.filter(|rpm| *rpm > 0.0) {
        Some(rpm) => Some(rpm * 2.0 * PI / 60.0),
        None => (redline_rad_s > 0.0).then_some(redline_rad_s * DEFAULT_PROP_REDLINE_FRACTION),
    }
}

/// X-Plane engine types that turn a propeller: reciprocating (carburetted and
/// injected), free and fixed turbines, and electric motors.
fn drives_propeller(engine_type: i32) -> bool {
    matches!(engine_type, 0 | 1 | 2 | 3 | 8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn true_airspeed_grows_with_altitude_and_rejects_bad_density() {
        let indicated = Velocity::new::<knot>(100.0);
        assert!((true_airspeed(indicated, SEA_LEVEL_DENSITY).get::<knot>() - 100.0).abs() < 1e-9);
        // The KMGJ reposition measured 1.184 kg/m³; TAS is 1.7% above IAS.
        let tas = true_airspeed(indicated, 1.184).get::<knot>();
        assert!((tas - 101.72).abs() < 0.01, "{tas}");
        assert_eq!(true_airspeed(indicated, f64::NAN).get::<knot>(), 100.0);
        assert_eq!(true_airspeed(indicated, 0.0).get::<knot>(), 100.0);
    }

    #[test]
    fn inertial_velocity_adds_air_mass_motion() {
        // Level flight due south at 50 m/s moves toward +z in OpenGL coordinates.
        let still = inertial_velocity(180.0, 0.0, 50.0, [0.0; 3]);
        assert!(still[0].abs() < 1e-9 && still[1].abs() < 1e-9);
        assert!((still[2] - 50.0).abs() < 1e-9);

        // Wind from 244° true moves air toward 064°: +x (east) and -z (north).
        // Heading 202° true into it, the air-relative vector keeps the full
        // airspeed and the ground vector is shortened by the headwind.
        let wind_from = 244.0_f64.to_radians();
        let wind = [-5.29 * wind_from.sin(), 0.0, 5.29 * wind_from.cos()];
        assert!(wind[0] > 0.0 && wind[2] < 0.0);
        let ground = inertial_velocity(202.0, 2.0, 55.0, wind);
        let air = [
            ground[0] - wind[0],
            ground[1] - wind[1],
            ground[2] - wind[2],
        ];
        let air_speed = air.iter().map(|value| value * value).sum::<f64>().sqrt();
        assert!((air_speed - 55.0).abs() < 1e-9);
        assert!(ground[0].hypot(ground[2]) < air[0].hypot(air[2]));
    }

    #[test]
    fn sideslip_offsets_the_air_track_from_the_heading() {
        assert_eq!(air_track_deg(202.0, None), 202.0);
        // The KMGJ reposition that ignored a wind from the right flew 6.5° right
        // of the nose; X-Plane reported beta -6.5.
        assert!((air_track_deg(202.0, Some(-6.5)) - 208.5).abs() < 1e-9);
        assert!((air_track_deg(1.0, Some(2.0)) - 359.0).abs() < 1e-9);
    }

    #[test]
    fn recorded_vertical_speed_sets_the_flight_path() {
        // Legacy PADs fly along the saved pitch.
        assert_eq!(flight_path_deg(3.0, None, 53.0, 0.0), 3.0);
        // Level flight at 3° pitch keeps the 3° as angle of attack.
        let level = flight_path_deg(3.0, Some(Velocity::new::<foot_per_minute>(0.0)), 53.0, 0.0);
        assert!(level.abs() < 1e-12);
        // 500 fpm at 103 kt true is a 2.75° climb.
        let climb = flight_path_deg(
            3.0,
            Some(Velocity::new::<foot_per_minute>(500.0)),
            Velocity::new::<knot>(103.0).get::<meter_per_second>(),
            0.0,
        );
        assert!((climb - 2.75).abs() < 0.01, "{climb}");
        // Rising air supplies part of the climb.
        let updraft = flight_path_deg(0.0, Some(Velocity::new::<meter_per_second>(1.0)), 50.0, 1.0);
        assert!(updraft.abs() < 1e-12);
    }

    #[test]
    fn prop_target_prefers_recorded_rpm() {
        let redline = 2_700.0 * 2.0 * PI / 60.0;
        let recorded = prop_target_rad_s(Some(2_450.0), redline).unwrap();
        assert!((recorded * 60.0 / (2.0 * PI) - 2_450.0).abs() < 1e-9);
        let fallback = prop_target_rad_s(None, redline).unwrap();
        assert!((fallback * 60.0 / (2.0 * PI) - 2_430.0).abs() < 1e-9);
        assert!(prop_target_rad_s(Some(0.0), redline).is_some());
        assert_eq!(prop_target_rad_s(None, 0.0), None);
    }

    #[test]
    fn only_propeller_engines_are_spun_up() {
        assert!([0, 1, 2, 3, 8].into_iter().all(drives_propeller));
        assert!(![4, 5, 6, 7].into_iter().any(drives_propeller));
    }
}
