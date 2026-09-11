// SPDX-License-Identifier: GPL-3.0-or-later
// Rust translation of the local ArduPilot-derived adapter. See NOTICE.md.
#![forbid(unsafe_code)]

pub mod flight;

#[derive(Clone, Copy, Debug)]
pub struct Params {
    pub time_constant_s: f32,
    pub maximum_rate_deg_s: f32,
    pub p: f32,
    pub i: f32,
    pub d: f32,
    pub ff: f32,
    pub integrator_max: f32,
    pub target_filter_hz: f32,
    pub error_filter_hz: f32,
    pub derivative_filter_hz: f32,
    pub pd_max: f32,
    pub slew_rate_max_deg_s: f32,
    pub slew_rate_tau_s: f32,
    pub scaling_speed_m_s: f32,
    pub airspeed_min_m_s: f32,
    pub airspeed_max_m_s: f32,
    pub output_slew_ratio_s: f32,
}
impl Default for Params {
    fn default() -> Self {
        Self {
            time_constant_s: 0.70,
            maximum_rate_deg_s: 35.0,
            p: 0.08,
            i: 0.15,
            d: 0.0,
            ff: 0.345,
            integrator_max: 0.666,
            target_filter_hz: 3.0,
            error_filter_hz: 0.0,
            derivative_filter_hz: 12.0,
            pd_max: 0.0,
            slew_rate_max_deg_s: 150.0,
            slew_rate_tau_s: 1.0,
            scaling_speed_m_s: 30.0,
            airspeed_min_m_s: 29.0,
            airspeed_max_m_s: 90.0,
            output_slew_ratio_s: 0.80,
        }
    }
}
impl Params {
    pub fn roll() -> Self {
        Self {
            time_constant_s: 0.50,
            p: 0.14,
            i: 0.30,
            ff: 0.90,
            output_slew_ratio_s: 1.20,
            ..Self::default()
        }
    }
    pub fn pitch() -> Self {
        Self {
            time_constant_s: 0.50,
            maximum_rate_deg_s: 22.0,
            p: 0.30,
            i: 1.50,
            ff: 2.50,
            output_slew_ratio_s: 1.20,
            ..Self::default()
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Output {
    pub angle_error_deg: f32,
    pub desired_rate_deg_s: f32,
    pub measured_rate_deg_s: f32,
    pub airspeed_scaler: f32,
    pub p_deg: f32,
    pub i_deg: f32,
    pub d_deg: f32,
    pub ff_deg: f32,
    pub d_modifier: f32,
    pub detected_slew_rate_deg_s: f32,
    pub raw_output_ratio: f32,
    pub limited_output_ratio: f32,
    pub saturated: bool,
    pub underspeed: bool,
}
#[derive(Clone, Debug)]
pub struct Controller {
    filters_reset: bool,
    filtered_target: f32,
    filtered_error: f32,
    filtered_derivative: f32,
    integrator: f32,
    output_ratio: f32,
    elapsed_ms: f32,
    slew_filter_output: f32,
    slew_last_sample: f32,
    max_positive_slew: f32,
    max_negative_slew: f32,
    max_positive_event_ms: f32,
    max_negative_event_ms: f32,
    output_slew_rate: f32,
    modifier_slew_rate: f32,
    positive_events_ms: [u32; 2],
    negative_events_ms: [u32; 2],
    positive_event_index: usize,
    negative_event_index: usize,
    positive_event_stored: bool,
    negative_event_stored: bool,
}
impl Default for Controller {
    fn default() -> Self {
        Self {
            filters_reset: true,
            filtered_target: 0.0,
            filtered_error: 0.0,
            filtered_derivative: 0.0,
            integrator: 0.0,
            output_ratio: 0.0,
            elapsed_ms: 0.0,
            slew_filter_output: 0.0,
            slew_last_sample: 0.0,
            max_positive_slew: 0.0,
            max_negative_slew: 0.0,
            max_positive_event_ms: 0.0,
            max_negative_event_ms: 0.0,
            output_slew_rate: 0.0,
            modifier_slew_rate: 0.0,
            positive_events_ms: [0; 2],
            negative_events_ms: [0; 2],
            positive_event_index: 0,
            negative_event_index: 0,
            positive_event_stored: false,
            negative_event_stored: false,
        }
    }
}
pub fn limit(value: f32, low: f32, high: f32) -> f32 {
    value.min(high).max(low)
}
pub fn wrap180(mut value: f32) -> f32 {
    while value > 180.0 {
        value -= 360.0;
    }
    while value < -180.0 {
        value += 360.0;
    }
    value
}
fn alpha(dt: f32, cutoff: f32) -> f32 {
    if dt <= 0.0 {
        return 0.0;
    }
    if cutoff <= 0.0 {
        return 1.0;
    }
    dt / (dt + 1.0 / (std::f32::consts::TAU * cutoff))
}
impl Controller {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    pub fn current_output(&self) -> f32 {
        self.output_ratio
    }
    fn slew_modifier(&mut self, sample: f32, dt: f32, p: &Params) -> f32 {
        if dt <= 0.0 {
            return 1.0;
        }
        self.elapsed_ms += dt * 1000.0;
        let derivative = (sample - self.slew_last_sample) / dt;
        self.slew_last_sample = sample;
        self.slew_filter_output += alpha(dt, 25.0) * (derivative - self.slew_filter_output);
        let slew = self.slew_filter_output;
        let tau = p.slew_rate_tau_s.max(0.01);
        let decay = dt.min(tau) / tau;
        let attack = (2.0 * decay).min(1.0);
        if slew > self.max_positive_slew {
            self.max_positive_slew = slew;
            self.max_positive_event_ms = self.elapsed_ms;
        } else if self.elapsed_ms - self.max_positive_event_ms > 300.0 {
            self.max_positive_slew *= 1.0 - decay;
        }
        if -slew > self.max_negative_slew {
            self.max_negative_slew = -slew;
            self.max_negative_event_ms = self.elapsed_ms;
        } else if self.elapsed_ms - self.max_negative_event_ms > 300.0 {
            self.max_negative_slew *= 1.0 - decay;
        }
        let raw = 0.5 * (self.max_positive_slew + self.max_negative_slew);
        self.output_slew_rate = (1.0 - attack) * self.output_slew_rate + attack * raw;
        self.output_slew_rate = self.output_slew_rate.min(raw);
        let bound = p.slew_rate_max_deg_s;
        if bound <= 0.0 {
            return 1.0;
        }
        let limited = 0.5
            * (self.max_positive_slew.min(10.0 * bound) + self.max_negative_slew.min(10.0 * bound));
        let now = self.elapsed_ms.max(0.0) as u32;
        if !self.positive_event_stored && slew > bound {
            self.positive_events_ms[self.positive_event_index] = now;
            self.positive_event_index = (self.positive_event_index + 1) % 2;
            self.positive_event_stored = true;
            self.negative_event_stored = false;
        }
        if !self.negative_event_stored && -slew > bound {
            self.negative_events_ms[self.negative_event_index] = now;
            self.negative_event_index = (self.negative_event_index + 1) % 2;
            self.negative_event_stored = true;
            self.positive_event_stored = false;
        }
        let oldest = self
            .positive_events_ms
            .iter()
            .chain(&self.negative_events_ms)
            .copied()
            .fold(now, u32::min);
        let mut modifier_input = limited;
        if now.wrapping_sub(oldest) as f32 > 900.0 {
            let outside = 0.001 * (now.wrapping_sub(oldest) as f32 - 900.0);
            modifier_input *= (-outside / tau).exp();
        }
        self.modifier_slew_rate =
            (1.0 - attack) * self.modifier_slew_rate + attack * modifier_input;
        self.modifier_slew_rate = self.modifier_slew_rate.min(modifier_input);
        if self.modifier_slew_rate <= bound {
            1.0
        } else {
            bound / (bound + 1.5 * (self.modifier_slew_rate - bound))
        }
    }
    // This signature matches the reference controller's explicit physical inputs.
    #[allow(clippy::too_many_arguments)]
    pub fn update(
        &mut self,
        p: &Params,
        target: f32,
        angle: f32,
        rate_rad: f32,
        eas: f32,
        eas_to_tas: f32,
        authority: f32,
        dt: f32,
    ) -> Output {
        let dt = limit(dt, 0.002, 0.05);
        let authority = limit(authority, 0.0, 1.0);
        let angle_error = wrap180(target - angle);
        let mut desired_rate = angle_error / p.time_constant_s.max(0.05);
        if p.maximum_rate_deg_s > 0.0 {
            desired_rate = limit(desired_rate, -p.maximum_rate_deg_s, p.maximum_rate_deg_s);
        }
        let minimum = p.airspeed_min_m_s.max(1.0);
        let maximum = p.airspeed_max_m_s.max(minimum + 1.0);
        let scale_min = 0.5_f32.min(p.scaling_speed_m_s / (2.0 * maximum));
        let scale_max = 2.0_f32.max(p.scaling_speed_m_s / (0.7 * minimum));
        let scaler = limit(
            if eas > 0.0001 {
                p.scaling_speed_m_s / eas
            } else {
                scale_max
            },
            scale_min,
            scale_max,
        );
        let underspeed = eas <= minimum;
        let eas_to_tas = limit(eas_to_tas, 1.0, 3.0);
        let squared = scaler * scaler;
        const DEG_TO_RAD: f32 = 0.017_453_292;
        let raw_target = desired_rate * DEG_TO_RAD * squared;
        let measurement = rate_rad * squared;
        let old_integrator = self.integrator;
        if self.filters_reset {
            self.filters_reset = false;
            self.filtered_target = raw_target;
            self.filtered_error = self.filtered_target - measurement;
            self.filtered_derivative = 0.0;
        } else {
            self.filtered_target +=
                alpha(dt, p.target_filter_hz) * (raw_target - self.filtered_target);
            let previous_error = self.filtered_error;
            let raw_error = self.filtered_target - measurement;
            self.filtered_error += alpha(dt, p.error_filter_hz) * (raw_error - self.filtered_error);
            let derivative = (self.filtered_error - previous_error) / dt;
            self.filtered_derivative +=
                alpha(dt, p.derivative_filter_hz) * (derivative - self.filtered_derivative);
        }
        let output_limited = self.output_ratio.abs() >= authority - 0.0001;
        let may_grow = !output_limited
            || (self.integrator > 0.0 && self.filtered_error < 0.0)
            || (self.integrator < 0.0 && self.filtered_error > 0.0);
        if p.i != 0.0 && may_grow {
            self.integrator += self.filtered_error * p.i * dt;
            self.integrator = limit(
                self.integrator,
                -p.integrator_max.abs(),
                p.integrator_max.abs(),
            );
        }
        if underspeed {
            self.integrator = old_integrator;
        }
        let mut proportional = self.filtered_error * p.p;
        let mut derivative = self.filtered_derivative * p.d;
        let modifier = self.slew_modifier((proportional + derivative) * 45.0, dt, p);
        proportional *= modifier;
        derivative *= modifier;
        if p.pd_max > 0.0 && (proportional + derivative).abs() > p.pd_max {
            let scale = p.pd_max / (proportional + derivative).abs();
            proportional *= scale;
            derivative *= scale;
        }
        const RAD_TO_DEG: f32 = 57.295_78;
        let mut result = Output {
            angle_error_deg: angle_error,
            desired_rate_deg_s: desired_rate,
            measured_rate_deg_s: rate_rad * RAD_TO_DEG,
            airspeed_scaler: scaler,
            p_deg: proportional * RAD_TO_DEG,
            i_deg: self.integrator * RAD_TO_DEG,
            d_deg: derivative * RAD_TO_DEG,
            ff_deg: self.filtered_target * p.ff * RAD_TO_DEG / (scaler * eas_to_tas).max(0.01),
            d_modifier: modifier,
            detected_slew_rate_deg_s: self.output_slew_rate,
            underspeed,
            ..Output::default()
        };
        result.raw_output_ratio =
            (result.p_deg + result.i_deg + result.d_deg + result.ff_deg) / 45.0;
        let limited = limit(result.raw_output_ratio, -authority, authority);
        result.saturated = (result.raw_output_ratio - limited).abs() > 0.0001;
        let delta = p.output_slew_ratio_s.max(0.0) * dt;
        self.output_ratio += limit(limited - self.output_ratio, -delta, delta);
        self.output_ratio = limit(self.output_ratio, -authority, authority);
        result.limited_output_ratio = self.output_ratio;
        result
    }
    pub fn neutral(&mut self, slew: f32, dt: f32) -> f32 {
        let delta = slew.max(0.0) * limit(dt, 0.002, 0.05);
        self.output_ratio += limit(-self.output_ratio, -delta, delta);
        self.integrator = 0.0;
        self.filters_reset = true;
        self.output_ratio
    }
}
