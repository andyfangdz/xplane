use crate::Config;

pub const PI: f64 = std::f64::consts::PI;
pub fn rad(x: f64) -> f64 {
    x * PI / 180.0
}
pub fn deg(x: f64) -> f64 {
    x * 180.0 / PI
}
pub fn wrap(x: f64) -> f64 {
    (x + 180.0).rem_euclid(360.0) - 180.0
}
pub fn clamp(x: f64, lo: f64, hi: f64) -> f64 {
    x.clamp(lo, hi)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum Phase {
    Idle,
    Ready,
    Downwind,
    Delay,
    TurnBase,
    Base,
    TurnFinal,
    Final,
    Rollout,
    Complete,
    Aborted,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Reason {
    None,
    Entry,
    LowAlignment,
    Envelope,
    Timeout,
    SupervisorLost,
    InvalidConfig,
    FrameGap,
    WindMismatch,
    MassMismatch,
    OverrideConflict,
    Cancelled,
    MissingDataref,
    TraceError,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Sample {
    pub t: f64,
    pub x: f64,
    pub y: f64,
    pub h: f64,
    pub ias: f64,
    pub tas_fps: f64,
    pub gs_fps: f64,
    pub heading: f64,
    pub track: f64,
    pub bank: f64,
    pub pitch: f64,
    pub vvi: f64,
    pub vy: f64,
    pub wind_kt: f64,
    pub wind_dir: f64,
    pub mass_lb: f64,
    pub throttle: f64,
    pub ground: bool,
}
impl Sample {
    pub fn cross_velocity(&self, runway: f64) -> f64 {
        self.gs_fps * rad(self.track - runway).sin()
    }
}

/// Pure, after-physics state machine. Inputs deliberately retain the original
/// float telemetry quantization; no SDK calls or wall clock enter this logic.
#[derive(Debug, Clone)]
pub struct Controller {
    pub c: Config,
    pub phase: Phase,
    pub reason: Reason,
    pub heading: f64,
    pub pitch: f64,
    pub bank: f64,
    pub throttle: f64,
    pub flap: f64,
    pub lead: f64,
    pub desired: f64,
    pub accel: f64,
    pub wind_rate: f64,
    pub wind_ff: f64,
    pub pitch_rate: f64,
    pub start_t: f64,
    pub cut_t: f64,
    pub roundout_t: f64,
    pub contact_t: f64,
    pub gate_s: f64,
    pub dt: f64,
    pub delay_s: f64,
    pub speed_integral: f64,
    pub altitude_integral: f64,
    pub throttle_trim: f64,
    pub cross_accel: f64,
    pub predicted_cross: f64,
    pub actual_pitch_rate: f64,
    pub last: Option<Sample>,
    pub steps: u64,
}
impl Default for Controller {
    fn default() -> Self {
        Self::new(Config::default())
    }
}
impl Controller {
    pub fn new(c: Config) -> Self {
        let n = (c.end_lat - c.threshold_lat) * 60.0 * 6076.12;
        let e = (c.end_lon - c.threshold_lon)
            * 60.0
            * 6076.12
            * rad((c.threshold_lat + c.end_lat) * 0.5).cos();
        let mut heading = deg(e.atan2(n));
        if heading < 0.0 {
            heading += 360.0;
        }
        let head = c.wind_speed_kt * rad(c.wind_offset_deg).cos();
        let cross = c.wind_speed_kt * rad(c.wind_offset_deg).sin();
        let delay_s = (c.delay_base_s
            + c.delay_headwind_s_per_kt * head
            + c.delay_strong_headwind_s_per_kt * (head - c.flare_headwind_threshold_kt).max(0.0)
            + c.delay_tailwind_s_per_kt * (-head).max(0.0)
            + c.delay_crosswind_s_per_kt * cross
            + c.delay_crosswind_abs_s_per_kt * cross.abs())
        .max(0.0);
        Self {
            c,
            phase: Phase::Ready,
            reason: Reason::None,
            heading,
            pitch: c.initial_pitch_deg,
            bank: 0.0,
            throttle: c.initial_throttle,
            flap: 0.0,
            lead: 0.0,
            desired: 0.0,
            accel: 0.0,
            wind_rate: 0.0,
            wind_ff: 0.0,
            pitch_rate: 0.0,
            start_t: 0.0,
            cut_t: -1.0,
            roundout_t: -1.0,
            contact_t: -1.0,
            gate_s: 0.0,
            dt: 0.0,
            delay_s,
            speed_integral: 0.0,
            altitude_integral: 0.0,
            throttle_trim: c.initial_throttle,
            cross_accel: 0.0,
            predicted_cross: 0.0,
            actual_pitch_rate: 0.0,
            last: None,
            steps: 0,
        }
    }
    pub fn reset(&mut self, c: Config) {
        *self = Self::new(c);
    }
    pub fn start(&mut self, time: f64) {
        self.phase = Phase::Downwind;
        self.start_t = time;
    }
    pub fn running(&self) -> bool {
        (Phase::Downwind..=Phase::Rollout).contains(&self.phase)
    }
    pub fn abort(&mut self, why: Reason) {
        self.reason = why;
        self.phase = Phase::Aborted;
        self.throttle = 0.0;
        self.bank = 0.0;
    }
    pub fn wind_heading(&self, track: f64, s: &Sample, tas_kt: f64) -> f64 {
        let tas = (if tas_kt > 0.0 {
            tas_kt
        } else {
            s.tas_fps / 1.68780986
        })
        .max(30.0);
        track + deg(clamp(s.wind_kt * rad(s.wind_dir - track).sin() / tas, -0.5, 0.5).asin())
    }
    pub fn turn_lead(&self, s: &Sample) -> f64 {
        let c = self.c;
        let ratio = s.tas_fps / 1.68780986 / s.ias.max(50.0);
        let v = (0.4 * s.ias + 0.6 * (c.final_kias - 1.5)) * ratio * 1.68780986;
        let begin = wrap(s.heading - self.heading);
        let end =
            wrap(self.wind_heading(self.heading, s, (c.final_kias - 1.5) * ratio) - self.heading)
                - 3.0;
        let duration =
            (rad(wrap(end - begin)) * v / (32.174 * rad(c.turn_mean_bank_deg).tan())).max(0.0);
        let radius = v * v / (32.174 * rad(c.turn_bank_deg).tan());
        let wa = rad(s.wind_dir - self.heading);
        let cross = s.wind_kt * wa.sin();
        let head = s.wind_kt * wa.cos();
        (radius * (rad(end).cos() - rad(begin).cos())
            + c.turn_lead_extra_ft
            + c.turn_lead_headwind_ft_per_kt * head
            + c.turn_lead_crosswind_ft_per_kt * cross
            + c.turn_lead_crosswind_abs_ft_per_kt * cross.abs()
            + cross * 1.68780986 * duration)
            .max(500.0)
    }
    pub fn geometry_bank(&self, s: &Sample, cross_v: f64) -> f64 {
        let c = self.c;
        let look = c.lateral_lookahead_s;
        let y = s.y + cross_v * look + 0.5 * self.cross_accel * look * look;
        let ratio = s.tas_fps / 1.68780986 / s.ias.max(50.0);
        let v = ((0.4 * s.ias + 0.6 * (c.final_kias - 1.5)) * ratio * 1.68780986).max(60.0);
        let begin = rad(wrap(s.heading - self.heading))
            + 32.174 * rad(s.bank).tan() / s.tas_fps.max(60.0) * look;
        let end = rad(wrap(
            self.wind_heading(self.heading, s, v / 1.68780986) - self.heading,
        ));
        let cross = s.wind_kt * rad(s.wind_dir - self.heading).sin() * 1.68780986;
        let numerator = v * v * (end.cos() - begin.cos()) + cross * v * (end - begin).max(0.0);
        clamp(
            deg(numerator.max(0.0).atan2(32.174 * y.max(10.0))),
            0.0,
            c.turn_capture_bank_max_deg,
        )
    }
    pub fn centerline_bank(&mut self, s: &Sample, cross_v: f64) -> f64 {
        let c = self.c;
        let look = c.lateral_lookahead_s;
        self.predicted_cross = s.y + cross_v * look + 0.5 * self.cross_accel * look * look;
        let velocity = cross_v + self.cross_accel * look;
        clamp(
            deg(
                (-c.final_position_gain * self.predicted_cross - c.final_velocity_gain * velocity)
                    .atan2(32.174),
            ) + c.bank_bias_deg,
            if s.h < 50.0 { -7.0 } else { -20.0 },
            if s.h < 50.0 { 7.0 } else { 20.0 },
        )
    }
    pub fn strong_headwind(&self) -> f64 {
        (self.c.wind_speed_kt * rad(self.c.wind_offset_deg).cos()
            - self.c.flare_headwind_threshold_kt)
            .max(0.0)
    }
    pub fn final_speed_target(&self, s: &Sample) -> f64 {
        let c = self.c;
        if self.phase != Phase::Final {
            return c.final_kias;
        }
        let start = (c.deceleration_start_height_ft
            - c.deceleration_headwind_start_ft_per_kt * self.strong_headwind())
        .max(c.deceleration_end_height_ft + 20.0);
        let fraction = clamp(
            (start - s.h) / (start - c.deceleration_end_height_ft),
            0.0,
            1.0,
        );
        let blend = fraction * fraction * (3.0 - 2.0 * fraction);
        let tail = (-c.wind_speed_kt * rad(c.wind_offset_deg).cos()).max(0.0);
        let cross = (c.wind_speed_kt * rad(c.wind_offset_deg).sin()).abs();
        let landing = (c.landing_entry_kias
            - c.landing_tailwind_kias_per_kt * tail
            - c.landing_crosswind_kias_per_kt * cross)
            .max(65.0);
        c.final_kias
            + c.final_strong_headwind_kias_per_kt * self.strong_headwind()
            + (landing - c.final_kias) * blend
    }
    pub fn roundout_rate_limit(&self) -> f64 {
        let c = self.c;
        let cross = (c.wind_speed_kt * rad(c.wind_offset_deg).sin()).abs();
        let base =
            (c.flare_max_pitch_rate_deg_s - c.flare_crosswind_rate_reduction * cross).max(0.3);
        base + (c.flare_headwind_rate_gain * self.strong_headwind() + self.wind_ff.max(0.0))
            .min(c.flare_wind_positive_limit)
    }
    pub fn roundout_pitch_limit(&self) -> f64 {
        (self.c.flare_max_pitch_deg + self.c.flare_headwind_pitch_gain * self.strong_headwind())
            .min(12.0)
    }
    pub fn approach_path_bias(&self, s: &Sample) -> f64 {
        let c = self.c;
        if self.phase != Phase::Final
            || self.roundout_t >= 0.0
            || s.h <= c.flare_height_ft
            || s.h >= c.path_start_height_ft
        {
            return 0.0;
        }
        let head = s.wind_kt * rad(s.wind_dir - self.heading).cos();
        let cross = s.wind_kt * rad(s.wind_dir - self.heading).sin();
        let travel = c.path_flare_distance_calm_ft
            + c.path_flare_headwind_ft_per_kt * head
            + c.path_flare_headwind_sq_ft_per_kt2 * head * head
            + c.path_flare_crosswind_ft_per_kt * cross;
        let along = (s.gs_fps * rad(s.track - self.heading).cos()).max(80.0);
        let projected = s.x + (s.h - c.flare_height_ft) * along / clamp(-s.vy, 10.0, 20.0);
        let error = projected - (c.path_target_touchdown_ft - travel);
        clamp(
            -c.path_pitch_gain_deg_per_ft * error,
            -c.path_pitch_limit_deg,
            c.path_pitch_limit_deg,
        )
    }
    pub fn step(&mut self, s: Sample) {
        use Phase::*;
        if !self.running() {
            return;
        }
        self.dt = self.last.map_or(0.0, |last| s.t - last.t);
        let dt = self.dt;
        let c = self.c;
        if dt < 0.0 {
            self.abort(Reason::FrameGap);
            return;
        }
        if dt == 0.0 {
            self.last = Some(s);
            return;
        }
        if dt > c.maximum_frame_dt_s {
            self.abort(Reason::FrameGap);
            return;
        }
        self.steps += 1;
        if s.t - self.start_t > c.timeout_sim_s {
            self.abort(Reason::Timeout);
            return;
        }
        let last = self.last.expect("nonzero dt requires previous sample");
        let cross_v = s.cross_velocity(self.heading);
        self.cross_accel += dt / (c.lateral_accel_filter_s + dt)
            * ((cross_v - last.cross_velocity(self.heading)) / dt - self.cross_accel);
        self.actual_pitch_rate +=
            dt / (0.06 + dt) * ((s.pitch - last.pitch) / dt - self.actual_pitch_rate);
        self.accel += dt / (0.4 + dt) * ((s.vy - last.vy) / dt - self.accel);
        let head = s.wind_kt * rad(s.wind_dir - s.heading).cos();
        let oldhead = last.wind_kt * rad(last.wind_dir - last.heading).cos();
        self.wind_rate += dt / (0.25 + dt) * ((head - oldhead) / dt - self.wind_rate);
        if self.phase == Downwind {
            let stable = (s.ias - c.entry_kias).abs() <= c.entry_speed_tolerance
                && (s.h - c.entry_agl_ft).abs() <= c.entry_height_tolerance
                && (s.y - c.entry_cross_ft).abs() <= c.entry_cross_tolerance
                && s.bank.abs() <= c.entry_bank_tolerance;
            self.gate_s = if stable { self.gate_s + dt } else { 0.0 };
            let ae = c.entry_agl_ft - s.h;
            self.altitude_integral = clamp(
                self.altitude_integral + c.altitude_integral_gain * ae * dt,
                -c.altitude_integral_limit,
                c.altitude_integral_limit,
            );
            if s.vvi.abs() <= 150.0 {
                self.throttle_trim = clamp(
                    self.throttle_trim + c.throttle_integral_gain * (c.entry_kias - s.ias) * dt,
                    c.throttle_min,
                    c.throttle_max,
                );
                self.throttle = clamp(
                    self.throttle_trim + c.throttle_proportional_gain * (c.entry_kias - s.ias),
                    c.throttle_min,
                    c.throttle_max,
                );
            }
            let raw = clamp(
                c.level_pitch_deg + c.altitude_gain * ae + self.altitude_integral
                    - c.vvi_gain * s.vvi,
                -7.0,
                8.0,
            );
            self.pitch += clamp(raw - self.pitch, -1.25 * dt, 1.25 * dt);
            self.bank = clamp(
                0.9 * wrap(self.wind_heading(self.heading + 180.0, &s, 0.0) - s.heading)
                    - 0.003 * (c.entry_cross_ft - s.y),
                -12.0,
                12.0,
            );
            if last.x > c.cut_along_ft && s.x <= c.cut_along_ft {
                if self.gate_s < c.entry_gate_s {
                    self.abort(Reason::Entry);
                    return;
                }
                self.cut_t = s.t;
                self.phase = Delay;
                self.pitch = s.pitch;
                self.throttle = 0.0;
            }
            if s.ground {
                self.abort(Reason::Entry);
                return;
            }
        }
        if self.cut_t >= 0.0 {
            if (s.mass_lb - c.mass_target_lb).abs() > c.mass_tolerance_lb {
                self.abort(Reason::MassMismatch);
                return;
            }
            if s.h > 25.0
                && ((s.wind_kt - c.wind_speed_kt).abs() > c.wind_tolerance_kt
                    || (c.wind_speed_kt > 0.0
                        && wrap(s.wind_dir - self.heading - c.wind_offset_deg).abs()
                            > c.wind_tolerance_deg))
            {
                self.abort(Reason::WindMismatch);
                return;
            }
            if s.ground && self.contact_t < 0.0 {
                self.contact_t = s.t;
                self.phase = Rollout;
            }
            if self.phase == Rollout {
                self.throttle = 0.0;
                self.bank = 0.0;
                self.pitch += clamp(2.0 - self.pitch, -dt, dt);
                if s.t - self.contact_t >= c.rollout_s {
                    self.phase = Complete;
                }
                self.last = Some(s);
                return;
            }
            self.throttle = 0.0;
            let mut target = c.turn_kias;
            self.lead = self.turn_lead(&s);
            if self.phase == Delay && s.t - self.cut_t >= self.delay_s {
                self.phase = TurnBase;
            }
            if self.phase == TurnBase
                && wrap(self.wind_heading(self.heading - 90.0, &s, 0.0) - s.heading).abs() < 5.0
            {
                self.phase = Base;
                self.flap = 0.5;
            }
            if self.phase == Base && s.y <= self.lead {
                self.phase = TurnFinal;
            }
            if self.phase == TurnFinal
                && wrap(self.wind_heading(self.heading, &s, 0.0) - s.heading).abs() < 3.0
            {
                self.phase = Final;
            }
            self.bank = match self.phase {
                Delay => clamp(
                    0.9 * wrap(self.wind_heading(self.heading + 180.0, &s, 0.0) - s.heading),
                    -12.0,
                    12.0,
                ),
                TurnBase => c.turn_bank_deg,
                Base => clamp(
                    wrap(self.wind_heading(self.heading - 90.0, &s, 0.0) - s.heading),
                    -20.0,
                    20.0,
                ),
                TurnFinal => {
                    let error = wrap(self.wind_heading(self.heading, &s, 0.0) - s.heading);
                    let weight = clamp(
                        (c.capture_blend_start_deg - error.abs())
                            / (c.capture_blend_start_deg - c.capture_blend_full_deg),
                        0.0,
                        1.0,
                    );
                    let blend = weight * weight * (3.0 - 2.0 * weight);
                    target = c.final_kias;
                    (1.0 - blend) * self.geometry_bank(&s, cross_v)
                        + blend * self.centerline_bank(&s, cross_v)
                }
                _ => {
                    target = self.final_speed_target(&s);
                    self.centerline_bank(&s, cross_v)
                }
            };
            if self.phase >= Base
                && self.flap < 1.0
                && (s.y <= 3000.0 || (self.phase == Final && s.x >= -600.0))
            {
                self.flap = 1.0;
            }
            self.speed_integral = clamp(
                self.speed_integral + 0.025 * (s.ias - target) * dt,
                -4.0,
                4.0,
            );
            let full_ff = -3.5 - (target - 75.0) / 6.0;
            let ff = if self.flap <= 0.5 {
                -9.0 * self.flap
            } else {
                -4.5 + (full_ff + 4.5) * 2.0 * (self.flap - 0.5)
            };
            let raw = -0.8
                + 0.35 * (s.ias - target)
                + self.speed_integral
                + ff
                + self.approach_path_bias(&s);
            if self.phase == Final
                && s.h <= c.flare_height_ft
                && s.y.abs() < 80.0
                && self.roundout_t < 0.0
            {
                self.roundout_t = s.t;
            }
            if self.roundout_t >= 0.0 {
                let va = c.flare_vertical_accel_fps2;
                let ph = (s.h + s.vy * c.flare_lookahead_s).max(0.0);
                self.desired =
                    -(c.flare_contact_sink_fps * c.flare_contact_sink_fps + 2.0 * va * ph).sqrt();
                let ades = if ph > 0.0 {
                    clamp(
                        -va * (s.vy + c.flare_lookahead_s * self.accel)
                            / self.desired.abs().max(1.0),
                        0.0,
                        3.0,
                    )
                } else {
                    0.0
                };
                self.wind_ff = clamp(
                    -c.flare_wind_loss_gain * self.wind_rate,
                    -c.flare_wind_negative_limit,
                    c.flare_wind_positive_limit,
                );
                self.pitch_rate = clamp(
                    deg(ades / s.tas_fps.max(80.0)) + c.flare_velocity_gain * (self.desired - s.vy)
                        - c.flare_acceleration_gain * (self.accel - ades)
                        + self.wind_ff,
                    -0.8,
                    self.roundout_rate_limit(),
                );
                // Damping must follow saturation, including at maximum authority.
                self.pitch_rate = (self.pitch_rate
                    - c.flare_pitch_rate_feedback_gain
                        * (self.actual_pitch_rate - c.flare_pitch_rate_feedback_target).max(0.0))
                .max(-0.8);
                self.pitch = clamp(
                    self.pitch + self.pitch_rate * dt,
                    -5.0,
                    self.roundout_pitch_limit(),
                );
            } else {
                self.pitch += clamp(raw - self.pitch, -1.25 * dt, 1.25 * dt);
            }
            if s.h < 60.0 && (s.x < -600.0 || s.y.abs() > 150.0) {
                self.abort(Reason::LowAlignment);
                return;
            }
            if s.ias < c.minimum_ias_abort || s.bank.abs() > c.maximum_bank_abort {
                self.abort(Reason::Envelope);
                return;
            }
        }
        self.last = Some(s);
    }
}
