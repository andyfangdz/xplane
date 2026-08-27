use crate::pad::{normalize_heading, AutopilotData, Field, Form, PadData};
use xplane_plugin::world_to_local;

use super::state::{PendingReapply, PluginState};

const METERS_TO_FEET: f64 = 3.280_839_895_013_1;
const KNOTS_TO_MPS: f64 = 0.514_444_444_444_44;

impl PluginState {
    pub(in crate::runtime) fn capture_current(&mut self) -> PadData {
        let mut throttle = 0.0_f32;
        self.datarefs
            .throttles
            .read_f32(std::slice::from_mut(&mut throttle));
        let data = PadData {
            latitude: self.datarefs.latitude.get_f64(),
            longitude: self.datarefs.longitude.get_f64(),
            altitude: self.datarefs.elevation.get_f64() * METERS_TO_FEET,
            heading: normalize_heading(
                self.datarefs.psi.get_f32() as f64 + self.datarefs.magvar.get_f32() as f64,
            ),
            pitch: self.datarefs.theta.get_f32() as f64,
            roll: self.datarefs.phi.get_f32() as f64,
            speed: self.datarefs.ias.get_f32() as f64,
            throttle: throttle as f64,
            flaps: self.datarefs.flaps.get_f32() as f64,
            gear: self.datarefs.gear.get_i32(),
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
            data.altitude / METERS_TO_FEET,
        );
        self.datarefs.local_x.set_f64(x);
        self.datarefs.local_y.set_f64(y);
        self.datarefs.local_z.set_f64(z);
        self.apply_attitude_velocity_controls(&data);
        self.pending = Some(PendingReapply {
            data: data.clone(),
            wait_frames: 2,
            remaining_frames: 6,
        });
    }

    pub(in crate::runtime) fn apply_attitude_velocity_controls(&self, data: &PadData) {
        let true_heading = normalize_heading(data.heading - self.datarefs.magvar.get_f32() as f64);
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

        let speed_mps = data.speed * KNOTS_TO_MPS;
        let heading_rad = true_heading.to_radians();
        let pitch_rad = data.pitch.to_radians();
        let horizontal_speed = speed_mps * pitch_rad.cos();
        self.datarefs
            .local_vx
            .set_f32((horizontal_speed * heading_rad.sin()) as f32);
        self.datarefs
            .local_vy
            .set_f32((speed_mps * pitch_rad.sin()) as f32);
        self.datarefs
            .local_vz
            .set_f32((-horizontal_speed * heading_rad.cos()) as f32);
        self.datarefs.rate_p.set_f32(0.0);
        self.datarefs.rate_q.set_f32(0.0);
        self.datarefs.rate_r.set_f32(0.0);

        let throttles = [data.throttle.clamp(0.0, 1.0) as f32; 16];
        self.datarefs.throttles.write_f32(&throttles);
        self.datarefs
            .flaps
            .set_f32(data.flaps.clamp(0.0, 1.0) as f32);
        self.datarefs
            .gear
            .set_i32(if data.gear != 0 { 1 } else { 0 });

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
