// SPDX-License-Identifier: GPL-3.0-or-later
// Pure state machine for the original v1.9 aircraft-local attitude helper.
use crate::{limit, Controller, Params};
use uom::si::{
    f32::Velocity,
    velocity::{knot, meter_per_second},
};

pub const INT_NAMES: [&str; 14] = [
    "version_major",
    "version_minor",
    "plugin_enabled",
    "aircraft_match",
    "armed",
    "active",
    "owns_roll_override",
    "mode",
    "release_reason",
    "phase",
    "complete",
    "first_contact_latched",
    "first_contact_mask",
    "saw_airborne",
];
pub const FLOAT_NAMES: [&str; 32] = [
    "target_bank_deg",
    "authority",
    "target_pitch_deg",
    "pitch_authority",
    "yaw_authority",
    "start_heading_deg",
    "entry_pitch_deg",
    "maximum_pitch_deg",
    "direction",
    "heading_progress_deg",
    "scheduled_bank_deg",
    "scheduled_pitch_deg",
    "command_ratio",
    "pitch_command_ratio",
    "yaw_command_ratio",
    "loop_hz",
    "bank_deg",
    "pitch_deg",
    "heading_deg",
    "beta_deg",
    "rollout_mid_progress_deg",
    "rollout_mid_bank_deg",
    "rollout_end_progress_deg",
    "rollout_terminal_shape",
    "first_contact_pitch_deg",
    "first_contact_vvi_fpm",
    "first_contact_bank_deg",
    "first_contact_heading_deg",
    "first_contact_beta_deg",
    "first_contact_ias_kias",
    "first_contact_throttle_ratio",
    "ground_target_heading_deg",
];
pub const DOUBLE_NAMES: [&str; 3] = [
    "first_contact_latitude",
    "first_contact_longitude",
    "first_contact_elevation_m",
];
pub fn float_writable(slot: usize) -> bool {
    slot <= 8 || (20..=23).contains(&slot) || slot == 31
}
pub fn wrap360(value: f32) -> f32 {
    let v = value % 360.0;
    if v < 0.0 {
        v + 360.0
    } else {
        v
    }
}
fn wrap180(value: f32) -> f32 {
    let v = wrap360(value);
    if v > 180.0 {
        v - 360.0
    } else {
        v
    }
}
fn smooth(value: f32) -> f32 {
    let x = limit(value, 0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum Release {
    None = 0,
    Disarmed = 1,
    WrongAircraft = 2,
    Paused = 3,
    Replay = 4,
    BadTiming = 5,
    Conflict = 6,
    Disabled = 7,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Sample {
    pub bank: f32,
    pub pitch: f32,
    pub heading: f32,
    pub p_rad: f32,
    pub q_rad: f32,
    pub r_rad: f32,
    pub beta: f32,
    pub ias: f32,
    pub tas: f32,
    pub dt: f32,
    pub paused: bool,
    pub replay: bool,
    pub ground_mask: i32,
    pub vvi: f32,
    pub latitude: f64,
    pub longitude: f64,
    pub elevation: f64,
    pub throttle: f32,
    pub overrides: [bool; 3],
}
/// State and exported dataref values share the original slot ordering.
#[derive(Clone, Debug)]
pub struct FlightController {
    pub ints: [i32; 14],
    pub floats: [f32; 32],
    pub doubles: [f64; 3],
    pub owns: [bool; 3],
    roll: Controller,
    pitch: Controller,
    yaw_i: f32,
}
impl Default for FlightController {
    fn default() -> Self {
        let mut ints = [0; 14];
        ints[0] = 1;
        ints[1] = 9;
        ints[7] = 1;
        ints[8] = Release::Disarmed as i32;
        let mut floats = [0.0; 32];
        for (i, v) in [
            (1, 0.72),
            (2, 1.5),
            (3, 0.95),
            (4, 0.45),
            (5, 60.0),
            (7, 15.0),
            (8, 1.0),
            (20, 150.0),
            (21, 20.0),
            (22, 180.0),
            (23, 0.30),
        ] {
            floats[i] = v;
        }
        Self {
            ints,
            floats,
            doubles: [0.0; 3],
            owns: [false; 3],
            roll: Controller::default(),
            pitch: Controller::default(),
            yaw_i: 0.0,
        }
    }
}
impl FlightController {
    pub fn reset_controllers(&mut self) {
        self.roll.reset();
        self.pitch.reset();
        self.yaw_i = 0.0;
    }
    pub fn reset_contact(&mut self) {
        self.ints[11..14].fill(0);
        self.floats[24..31].fill(0.0);
        self.doubles.fill(0.0);
    }
    pub fn release(&mut self, reason: Release) {
        self.owns.fill(false);
        self.ints[5] = 0;
        self.ints[6] = 0;
        self.ints[8] = reason as i32;
        self.floats[12..15].fill(0.0);
        self.reset_controllers();
    }
    pub fn write_int(&mut self, slot: usize, value: i32) {
        if slot == 4 {
            if value != 0 && self.ints[4] == 0 {
                self.reset_contact();
            }
            self.ints[4] = i32::from(value != 0);
            if value == 0 {
                self.release(Release::Disarmed);
            }
        } else if slot == 7 {
            let next = value.clamp(0, 3);
            if next != self.ints[7] {
                self.reset_controllers();
            }
            self.ints[7] = next;
            self.ints[9] = 0;
            self.ints[10] = 0;
        }
    }
    pub fn write_float(&mut self, slot: usize, value: f32) {
        if !value.is_finite() {
            return;
        }
        let v = match slot {
            0 => limit(value, -60.0, 60.0),
            1 | 3 | 4 => limit(value, 0.0, 1.0),
            2 => limit(value, -20.0, 35.0),
            5 | 31 => wrap360(value),
            6 => limit(value, -10.0, 15.0),
            7 => limit(value, 5.0, 35.0),
            8 => {
                if value < 0.0 {
                    -1.0
                } else {
                    1.0
                }
            }
            20 => limit(value, 120.0, 165.0),
            21 => limit(value, 10.0, 28.0),
            22 => limit(value, 177.0, 180.0),
            23 => limit(value, 0.15, 1.0),
            _ => return,
        };
        self.floats[slot] = v;
    }
    pub fn step(&mut self, s: Sample) {
        self.floats[16..20].copy_from_slice(&[s.bank, s.pitch, s.heading, s.beta]);
        self.floats[15] = if s.dt > 0.0 { 1.0 / s.dt } else { 0.0 };
        if self.ints[4] != 0 {
            if s.ground_mask == 0 {
                self.ints[13] = 1;
            } else if self.ints[13] != 0 && self.ints[11] == 0 {
                self.ints[11] = 1;
                self.ints[12] = s.ground_mask;
                self.floats[24..31].copy_from_slice(&[
                    s.pitch, s.vvi, s.bank, s.heading, s.beta, s.ias, s.throttle,
                ]);
                self.doubles = [s.latitude, s.longitude, s.elevation];
            }
        }
        let reason = if self.ints[2] == 0 {
            Release::Disabled
        } else if self.ints[3] == 0 {
            Release::WrongAircraft
        } else if self.ints[4] == 0 {
            Release::Disarmed
        } else if s.paused {
            Release::Paused
        } else if s.replay {
            Release::Replay
        // The v1.9 adapter permits frames through 75 ms. Its inner PID caps
        // integration at 50 ms; that cap is not the override-release limit.
        // Confusing them resets all axes on X-Plane's ~50.25 ms slow frames.
        } else if !s.dt.is_finite() || s.dt < 0.002 || s.dt > 0.075 {
            Release::BadTiming
        } else {
            Release::None
        };
        if reason != Release::None {
            self.release(reason);
            return;
        }
        let all_axes = self.ints[7] == 2 || self.ints[7] == 3;
        for axis in 0..if all_axes { 3 } else { 1 } {
            if !self.owns[axis] {
                if s.overrides[axis] {
                    self.release(Release::Conflict);
                    return;
                }
                self.owns[axis] = true;
            }
        }
        self.ints[5] = 1;
        self.ints[6] = i32::from(self.owns[0]);
        self.ints[8] = 0;
        let f = &mut self.floats;
        f[10] = f[0];
        f[11] = f[2];
        if self.ints[7] == 3 {
            f[9] = wrap360(f[8] * (s.heading - f[5]));
            if f[9] <= 90.0 {
                self.ints[9] = 1;
                f[10] = f[8] * 30.0;
                f[11] = f[6] + (f[7] - f[6]) * smooth(f[9] / 90.0);
            } else if f[9] < 180.0 {
                self.ints[9] = 2;
                let mid = f[20].min(f[22] - 3.0);
                let magnitude = if f[9] <= mid {
                    30.0 + (f[21] - 30.0) * smooth((f[9] - 90.0) / (mid - 90.0).max(1.0))
                } else if f[9] < f[22] {
                    f[21] * limit((f[22] - f[9]) / (f[22] - mid).max(1.0), 0.0, 1.0).powf(f[23])
                } else {
                    0.0
                };
                f[10] = f[8] * magnitude;
                f[11] = f[7];
            } else {
                self.ints[9] = 3;
                self.ints[10] = 1;
                f[10] = 0.0;
                f[11] = f[7];
            }
        } else {
            f[9] = 0.0;
            self.ints[9] = 0;
        }
        let eas = Velocity::new::<knot>(s.ias.max(0.0)).get::<meter_per_second>();
        let ratio = if eas > 1.0 {
            (s.tas.max(0.0) / eas).max(1.0)
        } else {
            1.0
        };
        let roll_params = Params::roll();
        let pitch_params = Params::pitch();
        f[12] = if self.ints[7] == 0 {
            self.roll.neutral(roll_params.output_slew_ratio_s, s.dt)
        } else {
            self.roll
                .update(&roll_params, f[10], s.bank, s.p_rad, eas, ratio, f[1], s.dt)
                .limited_output_ratio
        };
        if all_axes {
            f[13] = self
                .pitch
                .update(
                    &pitch_params,
                    f[11],
                    s.pitch,
                    s.q_rad,
                    eas,
                    ratio,
                    f[3],
                    s.dt,
                )
                .limited_output_ratio;
            if self.ints[7] == 3 {
                let scale = limit(40.0 / eas.max(20.0), 0.55, 1.45);
                f[13] = limit(
                    f[13] + 0.90 * smooth(f[9] / 90.0) * scale * scale,
                    -f[3],
                    f[3],
                );
            }
            let yaw_rate = s.r_rad * 57.295_78_f32;
            if self.ints[11] != 0 {
                self.yaw_i = 0.0;
                let desired = limit(
                    0.030 * wrap180(f[31] - s.heading) - 0.015 * yaw_rate - 0.015 * s.beta,
                    -0.55,
                    0.55,
                );
                f[14] += limit(desired - f[14], -1.5 * s.dt, 1.5 * s.dt);
            } else {
                let error = -s.beta;
                self.yaw_i = limit(self.yaw_i + error * 0.10 * s.dt, -0.25, 0.25);
                let desired = limit(
                    f[4] * (0.085 * error - 0.012 * yaw_rate + self.yaw_i),
                    -1.0,
                    1.0,
                );
                f[14] += limit(desired - f[14], -0.9 * s.dt, 0.9 * s.dt);
            }
        } else {
            f[13] = 0.0;
            f[14] = 0.0;
            self.yaw_i = 0.0;
        }
    }
}
