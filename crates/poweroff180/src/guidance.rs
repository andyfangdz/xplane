use crate::Config;
use uom::si::angle::degree;
use uom::si::{
    f64::Velocity,
    velocity::{foot_per_second, knot},
};

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
    AttitudeLost,
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
    pub float_guard_active: bool,
    pub predicted_touchdown_ft: f64,
    pub gust: Option<GustEstimate>,
    /// Gust speed margins on the final glide and landing targets, knots.
    pub final_margin_kt: f64,
    pub landing_margin_kt: f64,
    /// Gust card whose base turn already starts at the cut.
    pub energy_limited: bool,
    /// Commanded forward-slip sideslip for energy dissipation, degrees.
    pub slip: f64,
    /// Gust flare: estimated holding-lead difference from the schedule, deg.
    pub flare_hold_correction: f64,
    /// Gust flare: pitch rate the current demand should be producing.
    pub flare_rate_reference: f64,
    pub last: Option<Sample>,
    pub steps: u64,
}

/// Mean wind for gust cards. X-Plane applies a gust to true airspeed on the
/// same frame (TAS equals groundspeed minus wind), while indicated airspeed
/// lags by about one second. Guidance therefore flies the airspeed the
/// aircraft would have in the filtered mean wind, rather than chasing each
/// gust through a lagged instrument.
#[derive(Debug, Clone, Copy)]
pub struct GustEstimate {
    /// Mean wind-from vector, knots north and east.
    pub north_kt: f64,
    pub east_kt: f64,
    /// Slowly filtered indicated/true airspeed ratio (density and position error).
    pub ias_ratio: f64,
}
impl Default for Controller {
    fn default() -> Self {
        Self::new(Config::default())
    }
}
impl Controller {
    pub fn new(c: Config) -> Self {
        let heading = c
            .runway_axis()
            .map_or(f64::NAN, |axis| axis.heading().get::<degree>());
        // Turn timing uses the expected mean wind. X-Plane gusts rise from the
        // steady speed toward the peak; the measured mean is a fraction of it.
        let mean_kt = c.wind_speed_kt + c.gust_mean_fraction * c.wind_gust_kt;
        let head = mean_kt * rad(c.wind_offset_deg).cos();
        let cross = mean_kt * rad(c.wind_offset_deg).sin();
        let full_margin_kt = c.gust_speed_additive * c.wind_gust_kt;
        // Headwind and crosswind gusts decay into an airspeed loss near the
        // ground; a tailwind gust decays into a gain and floats. Fade the
        // carried landing margin and the earlier turn that pays for it from
        // full at a direct crosswind to zero at a direct tailwind.
        let direction = clamp(1.0 + rad(c.wind_offset_deg).cos(), 0.0, 1.0);
        let delay_s = c.delay_base_s
            + c.delay_headwind_s_per_kt * head
            + c.delay_strong_headwind_s_per_kt * (head - c.flare_headwind_threshold_kt).max(0.0)
            + c.delay_tailwind_s_per_kt * (-head).max(0.0)
            + c.delay_crosswind_s_per_kt * cross
            + c.delay_crosswind_abs_s_per_kt * cross.abs()
            // A faster gust glide is steeper at idle; turn base earlier.
            + c.delay_gust_additive_s_per_kt * full_margin_kt * direction;
        // The base turn cannot start before the power cut. In a strong gusty
        // headwind, hold half flaps through base and the final turn instead;
        // that saves more energy than the missing seconds, so the full gust
        // margin is kept for the flare.
        let energy_limited = c.wind_gust_kt > 0.0 && delay_s < 0.0;
        let final_margin_kt = full_margin_kt;
        let landing_margin_kt =
            (c.gust_landing_additive * c.wind_gust_kt * direction).min(final_margin_kt);
        // Energy plan relative to the steady law at the mean wind. v10 turned
        // early to arrive with a reserve for the slip. With v11's flare, a
        // short plan is recoverable (holding off floats hundreds of feet)
        // while a long one is not (planting gains ~100 ft, and only once
        // slow enough), so gust cards turn slightly later instead; ~0.1 s
        // is ~4 ft of energy on final. It never makes a card energy limited.
        let reserve_s = if c.wind_gust_kt > 0.0 {
            c.gust_energy_reserve_s * direction
        } else {
            0.0
        };
        let delay_s = (delay_s - reserve_s).max(0.0);
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
            final_margin_kt,
            landing_margin_kt,
            energy_limited,
            slip: 0.0,
            flare_hold_correction: 0.0,
            flare_rate_reference: 0.0,
            speed_integral: 0.0,
            altitude_integral: 0.0,
            throttle_trim: c.initial_throttle,
            cross_accel: 0.0,
            predicted_cross: 0.0,
            actual_pitch_rate: 0.0,
            float_guard_active: false,
            predicted_touchdown_ft: 0.0,
            gust: None,
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
        self.slip = 0.0;
    }
    /// Speed margin added to the final glide target: a fraction of the
    /// configured gust increase, reduced when the turn timing cannot pay for it.
    pub fn gust_additive(&self) -> f64 {
        self.final_margin_kt
    }
    /// Margin on the landing target. X-Plane's gust decays to the steady wind
    /// in the last 50 ft, so a headwind gust always costs airspeed in the
    /// flare; carrying it to roundout keeps lift. The short-final deceleration
    /// from the final margin is kept: it raises the nose before roundout.
    pub fn gust_landing_additive(&self) -> f64 {
        self.landing_margin_kt
    }
    /// Established-final target before short-final deceleration.
    pub fn approach_kias(&self) -> f64 {
        self.c.final_kias + self.gust_additive()
    }
    /// Replace instantaneous wind and airspeed with their mean-wind values.
    /// Without a configured gust the sample is returned unchanged.
    pub fn gust_sample(&mut self, s: &Sample, dt: f64) -> Sample {
        let c = self.c;
        if c.wind_gust_kt <= 0.0 {
            return *s;
        }
        let ground = Velocity::new::<foot_per_second>(s.gs_fps).get::<knot>();
        let (ground_n, ground_e) = (ground * rad(s.track).cos(), ground * rad(s.track).sin());
        let (wind_n, wind_e) = (
            s.wind_kt * rad(s.wind_dir).cos(),
            s.wind_kt * rad(s.wind_dir).sin(),
        );
        // Horizontal air velocity is ground velocity plus the wind-from vector.
        let air = |n: f64, e: f64| (ground_n + n).hypot(ground_e + e);
        // Sea-level to pattern altitude with pitot error; bounded so the
        // one-second instrument lag cannot bias the estimate after a gust.
        let ratio = clamp(s.ias / air(wind_n, wind_e).max(30.0), 0.95, 1.0);
        let g = match self.gust {
            None => GustEstimate {
                north_kt: wind_n,
                east_kt: wind_e,
                ias_ratio: ratio,
            },
            Some(g) => {
                let a = dt / (c.gust_wind_filter_s + dt);
                let b = dt / (c.gust_ias_ratio_filter_s + dt);
                GustEstimate {
                    north_kt: g.north_kt + a * (wind_n - g.north_kt),
                    east_kt: g.east_kt + a * (wind_e - g.east_kt),
                    ias_ratio: g.ias_ratio + b * (ratio - g.ias_ratio),
                }
            }
        };
        self.gust = Some(g);
        let mean_air = air(g.north_kt, g.east_kt);
        Sample {
            ias: mean_air * g.ias_ratio,
            tas_fps: Velocity::new::<knot>(mean_air).get::<foot_per_second>(),
            wind_kt: g.north_kt.hypot(g.east_kt),
            wind_dir: deg(g.east_kt.atan2(g.north_kt)).rem_euclid(360.0),
            ..*s
        }
    }
    /// Measured wind is consistent with the card: steady speed and direction,
    /// or for a gust card any speed between the steady wind and its gust peak.
    pub fn wind_mismatch(&self, s: &Sample) -> bool {
        let c = self.c;
        let speed = if c.wind_gust_kt > 0.0 {
            s.wind_kt < c.wind_speed_kt - c.gust_wind_tolerance_kt
                || s.wind_kt > c.wind_speed_kt + c.wind_gust_kt + c.gust_wind_tolerance_kt
        } else {
            (s.wind_kt - c.wind_speed_kt).abs() > c.wind_tolerance_kt
        };
        speed
            || (c.wind_speed_kt > 0.0
                && wrap(s.wind_dir - self.heading - c.wind_offset_deg).abs() > c.wind_tolerance_deg)
    }
    pub fn wind_heading(&self, track: f64, s: &Sample, tas_kt: f64) -> f64 {
        let tas = (if tas_kt > 0.0 {
            tas_kt
        } else {
            Velocity::new::<foot_per_second>(s.tas_fps).get::<knot>()
        })
        .max(30.0);
        track + deg(clamp(s.wind_kt * rad(s.wind_dir - track).sin() / tas, -0.5, 0.5).asin())
    }
    pub fn turn_lead(&self, s: &Sample) -> f64 {
        let c = self.c;
        let ratio = Velocity::new::<foot_per_second>(s.tas_fps).get::<knot>() / s.ias.max(50.0);
        let approach = self.approach_kias();
        let v = Velocity::new::<knot>((0.4 * s.ias + 0.6 * (approach - 1.5)) * ratio)
            .get::<foot_per_second>();
        let begin = wrap(s.heading - self.heading);
        let end =
            wrap(self.wind_heading(self.heading, s, (approach - 1.5) * ratio) - self.heading) - 3.0;
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
            + Velocity::new::<knot>(cross).get::<foot_per_second>() * duration)
            .max(500.0)
    }
    pub fn geometry_bank(&self, s: &Sample, cross_v: f64) -> f64 {
        let c = self.c;
        let look = c.lateral_lookahead_s;
        let y = s.y + cross_v * look + 0.5 * self.cross_accel * look * look;
        let ratio = Velocity::new::<foot_per_second>(s.tas_fps).get::<knot>() / s.ias.max(50.0);
        let v = (Velocity::new::<knot>((0.4 * s.ias + 0.6 * (self.approach_kias() - 1.5)) * ratio)
            .get::<foot_per_second>())
        .max(60.0);
        let begin = rad(wrap(s.heading - self.heading))
            + 32.174 * rad(s.bank).tan() / s.tas_fps.max(60.0) * look;
        let end = rad(wrap(
            self.wind_heading(
                self.heading,
                s,
                Velocity::new::<foot_per_second>(v).get::<knot>(),
            ) - self.heading,
        ));
        let cross = Velocity::new::<knot>(s.wind_kt * rad(s.wind_dir - self.heading).sin())
            .get::<foot_per_second>();
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
        let approach = self.approach_kias();
        if self.phase != Phase::Final {
            return approach;
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
            .max(65.0)
            + self.gust_landing_additive();
        approach
            + c.final_strong_headwind_kias_per_kt * self.strong_headwind()
            + (landing - approach) * blend
    }
    pub fn roundout_rate_limit(&self) -> f64 {
        let c = self.c;
        let cross = (c.wind_speed_kt * rad(c.wind_offset_deg).sin()).abs();
        let base =
            (c.flare_max_pitch_rate_deg_s - c.flare_crosswind_rate_reduction * cross).max(0.3);
        let limit = base
            + (c.flare_headwind_rate_gain * self.strong_headwind() + self.wind_ff.max(0.0))
                .min(c.flare_wind_positive_limit);
        // Gust flares repeatedly saturate here; the measured attitude rate
        // then overshoots the command. Keep headroom under the motion limit.
        if c.wind_gust_kt > 0.0 {
            limit.min(c.gust_flare_max_rate_deg_s)
        } else {
            limit
        }
    }
    pub fn roundout_pitch_limit(&self) -> f64 {
        (self.c.flare_max_pitch_deg + self.c.flare_headwind_pitch_gain * self.strong_headwind())
            .min(12.0)
    }
    /// Touchdown aim. Gust cards landed 40 ft past it on average with energy
    /// management (two 18-flight matrices), so their aim is offset.
    pub fn touchdown_aim_ft(&self) -> f64 {
        let c = self.c;
        c.path_target_touchdown_ft
            + if c.wind_gust_kt > 0.0 {
                c.gust_aim_offset_ft
            } else {
                0.0
            }
    }
    /// Projected roundout position minus the wind-adjusted aim, feet, on the
    /// current descent angle. Positive means long (surplus energy).
    pub fn roundout_projection_error(&self, s: &Sample) -> f64 {
        let c = self.c;
        let head = s.wind_kt * rad(s.wind_dir - self.heading).cos();
        let cross = s.wind_kt * rad(s.wind_dir - self.heading).sin();
        // A decaying tailwind gust adds airspeed in the flare and floats
        // further at tailwind groundspeed; aim that roundout earlier.
        let tail = clamp(-rad(c.wind_offset_deg).cos(), 0.0, 1.0);
        let travel = c.path_flare_distance_calm_ft
            + c.path_flare_headwind_ft_per_kt * head
            + c.path_flare_headwind_sq_ft_per_kt2 * head * head
            + c.path_flare_crosswind_ft_per_kt * cross
            + c.gust_tail_travel_ft_per_kt * c.wind_gust_kt * tail;
        let along = (s.gs_fps * rad(s.track - self.heading).cos()).max(80.0);
        let projected = s.x + (s.h - c.flare_height_ft) * along / clamp(-s.vy, 10.0, 20.0);
        projected - (self.touchdown_aim_ft() - travel)
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
        let error = self.roundout_projection_error(s);
        if c.wind_gust_kt <= 0.0 {
            return clamp(
                -c.path_pitch_gain_deg_per_ft * error,
                -c.path_pitch_limit_deg,
                c.path_pitch_limit_deg,
            );
        }
        // Gust timing moves the energy on final by ~30 ft of height, and the
        // shallow pre-flare path turns that into ~500 ft of roundout position.
        // Nose-down corrections fade before roundout: one still active there
        // entered the flare steep and fast (680-750 fpm) and landed nose-low.
        // Nose-up corrections act as a pre-flare and keep full authority.
        let early = clamp((s.h - c.flare_height_ft) / c.gust_path_fade_ft, 0.0, 1.0);
        let down = c.gust_path_entry_limit_deg
            + (c.gust_path_pitch_limit_deg - c.gust_path_entry_limit_deg) * early;
        // Transition: a pilot starts rounding out higher when sinking faster.
        // v10 gust flights all reached the 35 ft roundout at ~685 fpm, where
        // the flare expects ~530 fpm, and the firm ones then met the gust
        // decay. Ease the sink onto a constant-deceleration profile that
        // arrives at the roundout height at the entry sink. This is outside
        // the path authority: it stops the descent, it does not steer it.
        let entry = c.gust_entry_sink_fpm / 60.0;
        let profile = (entry * entry
            + 2.0 * c.gust_transition_accel_fps2 * (s.h - c.flare_height_ft).max(0.0))
        .sqrt();
        let transition = if s.h < c.gust_preflare_height_ft {
            (c.gust_preflare_gain_deg_per_fps * (-s.vy - profile).max(0.0))
                .min(c.gust_transition_limit_deg)
        } else {
            0.0
        };
        clamp(
            -c.path_pitch_gain_deg_per_ft * error,
            -down,
            c.gust_path_pitch_limit_deg,
        ) + transition
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
        // Measurements stay raw for derivatives, the flare's wind-loss
        // feed-forward and every safety check. Guidance uses mean wind.
        let measured = s;
        let s = self.gust_sample(&measured, dt);
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
            if measured.h > 25.0 && self.wind_mismatch(&measured) {
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
                self.slip = 0.0;
                self.pitch += clamp(2.0 - self.pitch, -dt, dt);
                if s.t - self.contact_t >= c.rollout_s {
                    self.phase = Complete;
                }
                self.last = Some(measured);
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
                    target = self.approach_kias();
                    // Ground track answers a gust at once, unlike airspeed:
                    // the remaining arc uses the measured wind's drift.
                    let drift = Sample {
                        wind_kt: measured.wind_kt,
                        wind_dir: measured.wind_dir,
                        ..s
                    };
                    (1.0 - blend) * self.geometry_bank(&drift, cross_v)
                        + blend * self.centerline_bank(&s, cross_v)
                }
                _ => {
                    target = self.final_speed_target(&s);
                    self.centerline_bank(&s, cross_v)
                }
            };
            // Gust energy management: hold half flaps as a reserve through
            // base and the final turn, then extend full flaps on final once
            // the projected roundout shows a surplus, or at the latest by
            // 150 ft or 600 ft from the threshold; a slip dissipates the rest.
            let hold = self.energy_limited || (c.wind_gust_kt > 0.0 && c.gust_flap_hold == 1.0);
            let assured = self.phase == Final
                && (s.x >= -600.0
                    || (hold
                        && (s.h <= c.gust_flap_latest_height_ft
                            || self.roundout_projection_error(&s) >= c.gust_flap_extend_error_ft)));
            if self.phase >= Base && self.flap < 1.0 && ((s.y <= 3000.0 && !hold) || assured) {
                self.flap = 1.0;
            }
            let slip = if c.wind_gust_kt > 0.0
                && self.phase == Final
                && self.flap >= 1.0
                && self.roundout_t < 0.0
            {
                let fade = clamp(
                    (s.h - c.gust_slip_end_height_ft) / c.gust_slip_fade_ft,
                    0.0,
                    1.0,
                );
                let surplus = self.roundout_projection_error(&s) - c.gust_slip_deadband_ft;
                // Slip with the upwind wing low when there is a crosswind.
                let cross = s.wind_kt * rad(s.wind_dir - self.heading).sin();
                let side = if cross.abs() > 1.0 {
                    cross.signum()
                } else {
                    1.0
                };
                side * c.gust_slip_sign
                    * clamp(
                        c.gust_slip_gain_deg_per_ft * surplus,
                        0.0,
                        c.gust_slip_max_deg,
                    )
                    * fade
            } else {
                0.0
            };
            self.slip += clamp(
                slip - self.slip,
                -c.gust_slip_rate_deg_s * dt,
                c.gust_slip_rate_deg_s * dt,
            );
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
                self.flare_rate_reference = self.actual_pitch_rate;
                self.flare_hold_correction = 0.0;
            }
            if self.roundout_t >= 0.0 {
                // Anticipate the inner-loop response before a shallow flare
                // becomes a long float. Do not shorten a landing that is
                // already predicted to contact before the aim point.
                let look = c.flare_lookahead_s;
                let predicted_h = s.h + s.vy * look + 0.5 * self.accel * look * look;
                let predicted_vy = s.vy + self.accel * look;
                let along = (s.gs_fps * rad(s.track - self.heading).cos()).max(1.0);
                self.predicted_touchdown_ft = s.x
                    + along
                        * (look
                            + (predicted_h - c.flare_float_contact_height_ft).max(0.0)
                                / (-predicted_vy).max(c.flare_contact_sink_fps));
                if c.flare_float_enabled == 1.0
                    && s.x >= c.path_target_touchdown_ft - c.flare_float_margin_ft
                    && s.h <= c.flare_float_height_ft
                    && s.ias <= c.flare_float_max_kias + self.gust_landing_additive()
                    && predicted_h > c.flare_float_contact_height_ft
                    && predicted_vy > -c.flare_float_sink_fps
                    && self.predicted_touchdown_ft > c.path_target_touchdown_ft
                {
                    self.float_guard_active = true;
                }
                // A gust decays to the steady wind in the last ~10 ft; ask for
                // lower sink earlier, while pitch authority is unsaturated.
                let va = if c.wind_gust_kt > 0.0 {
                    c.gust_flare_vertical_accel_fps2
                } else {
                    c.flare_vertical_accel_fps2
                };
                let ph = (s.h + s.vy * c.flare_lookahead_s).max(0.0);
                let mut contact_sink = if self.float_guard_active {
                    c.flare_float_sink_fps
                } else {
                    c.flare_contact_sink_fps
                };
                let mut hold_off = None;
                if c.wind_gust_kt > 0.0 {
                    // Touchdown-point decision, as a pilot makes it in the
                    // flare. From the present sink, easing to a contact sink
                    // cs at a constant deceleration takes 2h / (|vy| + cs);
                    // on the planned profile that is exactly its time to go.
                    // Pick the cs that puts the wheels on the aim: hold off
                    // at the steady contact sink when short, put it down
                    // (up to the gust limit) when long. v10 extrapolated the
                    // unflared sink instead, which read short at roundout
                    // (1,054 ft predicted, 1,269 ft flown) until too late.
                    let h = s.h.max(0.0);
                    let available = (self.touchdown_aim_ft() - s.x) / along;
                    let needed = if available > 0.0 {
                        2.0 * h / available + s.vy
                    } else {
                        c.gust_touchdown_max_sink_fps
                    };
                    // Put it down only when slow enough to land; a tailwind
                    // gust decays into airspeed and must be floated off.
                    let landable =
                        measured.ias <= c.gust_touchdown_max_kias + self.gust_landing_additive();
                    if landable {
                        contact_sink = contact_sink.max(clamp(
                            needed,
                            c.flare_contact_sink_fps,
                            c.gust_touchdown_max_sink_fps,
                        ));
                    }
                    // Hold it off when even the gentlest contact lands short:
                    // descend only as fast as reaches the aim, down to level
                    // flight in ground effect, while there is flying speed.
                    // The float has far more authority than planting: a few
                    // knots of bleed are hundreds of feet.
                    hold_off = (available > 0.0
                        && needed < c.flare_contact_sink_fps
                        && measured.ias > c.gust_hold_off_min_kias)
                        .then(|| {
                            (2.0 * h / available - c.flare_contact_sink_fps)
                                .max(c.gust_hold_off_min_sink_fps)
                        });
                    self.predicted_touchdown_ft =
                        s.x + along * 2.0 * h / (contact_sink - s.vy).max(1.0);
                }
                // Preserve the roundout's deceleration profile; only raise its
                // terminal sink floor so a still-firm descent can soften.
                self.desired = -(contact_sink * contact_sink + 2.0 * va * ph).sqrt();
                if let Some(sink) = hold_off {
                    self.desired = self.desired.max(-sink);
                }
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
                let down_rate = if self.float_guard_active {
                    c.flare_float_down_rate_deg_s
                } else {
                    0.8
                };
                // Use the predicted vertical speed for the guarded correction,
                // too. Current descent can still look firm while the existing
                // pitch demand is already decelerating it into a long float.
                let feedback_vy = if self.float_guard_active {
                    predicted_vy
                } else {
                    s.vy
                };
                // Gusts decay to the steady wind in the last 50 ft, so the
                // flare meets a late headwind loss. Track the sink profile
                // more tightly while pitch authority is still unsaturated.
                let velocity_gain = if c.wind_gust_kt > 0.0 {
                    c.gust_flare_velocity_gain
                } else {
                    c.flare_velocity_gain
                };
                self.pitch_rate = clamp(
                    deg(ades / s.tas_fps.max(80.0)) + velocity_gain * (self.desired - feedback_vy)
                        - c.flare_acceleration_gain * (self.accel - ades)
                        + self.wind_ff,
                    -down_rate,
                    self.roundout_rate_limit(),
                );
                if c.wind_gust_kt > 0.0 {
                    // Ramp the demand in: a full step at roundout overshot
                    // to 2.8 deg/s about 0.75 s later in the helper.
                    self.pitch_rate = self.pitch_rate.min(
                        c.gust_flare_max_rate_deg_s * (s.t - self.roundout_t)
                            / c.gust_flare_entry_ramp_s,
                    );
                    // Command the achieved pitch rate, not an integrated
                    // attitude. Across 24 recorded flares the attitude helper
                    // answers a command lead as rate = (lead - L0(h)) / 0.4 s:
                    // a constant response time plus a holding lead that grows
                    // from ~0.2 deg at 30 ft to ~1.7 deg at the runway, as full
                    // flaps and ground effect pitch the nose down (R^2 0.8-0.9).
                    // An integrated command overshot early and fell behind
                    // late with the elevator near half authority; a lead
                    // without the holding term let a pitch-down run away.
                    // Feedback compares the achieved rate with the response
                    // the demand should be producing by now. Against the raw
                    // demand it added lead during every normal rise, which the
                    // helper delivered 0.25 s later as a 2.8 deg/s peak.
                    self.flare_rate_reference += dt / (c.gust_flare_reference_s + dt)
                        * (self.pitch_rate - self.flare_rate_reference);
                    let command_rate = self.pitch_rate
                        - c.gust_flare_rate_feedback
                            * (self.actual_pitch_rate - self.flare_rate_reference)
                        - c.flare_pitch_rate_feedback_gain
                            * (self.actual_pitch_rate - c.flare_pitch_rate_feedback_target)
                                .max(0.0);
                    // The holding lead varies 0.67-1.24x between flights
                    // (less in crosswinds, more in strong headwinds). Estimate
                    // the difference slowly against the same reference, so
                    // neither the initial rise nor the ~0.3 s helper lag can
                    // wind it up or make it oscillate.
                    self.flare_hold_correction = clamp(
                        self.flare_hold_correction
                            + c.gust_flare_hold_adapt_gain
                                * (self.flare_rate_reference - self.actual_pitch_rate)
                                * dt,
                        -c.gust_flare_hold_correction_deg,
                        c.gust_flare_hold_correction_deg,
                    );
                    let holding = c.gust_flare_hold_lead_deg
                        * clamp(
                            (c.gust_flare_hold_height_ft - s.h) / c.gust_flare_hold_height_ft,
                            0.0,
                            1.0,
                        )
                        + self.flare_hold_correction;
                    // Limit the attitude itself, not the command: near the
                    // runway the command must sit ~1.7 deg above the pitch
                    // just to hold it, so a command cap let the nose sag.
                    let command_rate = command_rate.min(
                        (c.gust_flare_max_pitch_deg - s.pitch) / c.gust_flare_lead_s.max(0.05),
                    );
                    let target = clamp(
                        s.pitch + holding + c.gust_flare_lead_s * command_rate,
                        -5.0,
                        c.gust_flare_max_pitch_deg + holding,
                    );
                    let slew = c.gust_flare_command_slew_deg_s * dt;
                    self.pitch += clamp(target - self.pitch, -slew, slew);
                } else {
                    // Damping must follow saturation, including at maximum authority.
                    self.pitch_rate = (self.pitch_rate
                        - c.flare_pitch_rate_feedback_gain
                            * (self.actual_pitch_rate - c.flare_pitch_rate_feedback_target)
                                .max(0.0))
                    .max(-down_rate);
                    self.pitch = clamp(
                        self.pitch + self.pitch_rate * dt,
                        -5.0,
                        self.roundout_pitch_limit(),
                    );
                }
            } else {
                self.pitch += clamp(raw - self.pitch, -1.25 * dt, 1.25 * dt);
            }
            if s.h < 60.0 && (s.x < -600.0 || s.y.abs() > 150.0) {
                self.abort(Reason::LowAlignment);
                return;
            }
            if measured.ias < c.minimum_ias_abort || measured.bank.abs() > c.maximum_bank_abort {
                self.abort(Reason::Envelope);
                return;
            }
        }
        self.last = Some(measured);
    }
}
