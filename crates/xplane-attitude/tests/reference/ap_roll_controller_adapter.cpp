/*
 * SPDX-License-Identifier: GPL-3.0-or-later
 * Derived from ArduPilot commit 4fe7ad4fab8c1bf4ade7cbc7ae85a73c81d73e05.
 * See ap_roll_controller_adapter.hpp and NOTICE.md for provenance.
 */

#include "ap_roll_controller_adapter.hpp"

#include <algorithm>
#include <cmath>

namespace sr20g6_test {
namespace {
constexpr float kRadiansToDegrees = 57.29577951308232f;
constexpr float kDegreesToRadians = 0.017453292519943295f;
constexpr float kTwoPi = 6.283185307179586f;
constexpr float kSlewWindowMs = 300.0f;
constexpr float kSlewModifierGain = 1.5f;
constexpr float kSlewDerivativeCutoffHz = 25.0f;
}

float RollController::constrain(float value, float minimum, float maximum) {
    return std::max(minimum, std::min(maximum, value));
}

float RollController::wrap_180(float angle_deg) {
    while (angle_deg > 180.0f) angle_deg -= 360.0f;
    while (angle_deg < -180.0f) angle_deg += 360.0f;
    return angle_deg;
}

float RollController::lowpass_alpha(float dt_s, float cutoff_hz) {
    if (dt_s <= 0.0f) return 0.0f;
    if (cutoff_hz <= 0.0f) return 1.0f;
    const float rc = 1.0f / (kTwoPi * cutoff_hz);
    return dt_s / (dt_s + rc);
}

void RollController::reset(float bank_deg, float roll_rate_rad_s) {
    (void)bank_deg;
    (void)roll_rate_rad_s;
    filters_reset_ = true;
    filtered_target_ = 0.0f;
    filtered_error_ = 0.0f;
    filtered_derivative_ = 0.0f;
    target_derivative_ = 0.0f;
    integrator_ = 0.0f;
    output_ratio_ = 0.0f;
    elapsed_ms_ = 0.0f;
    slew_filter_output_ = 0.0f;
    slew_last_sample_ = 0.0f;
    max_positive_slew_ = 0.0f;
    max_negative_slew_ = 0.0f;
    max_positive_event_ms_ = 0.0f;
    max_negative_event_ms_ = 0.0f;
    output_slew_rate_ = 0.0f;
    modifier_slew_rate_ = 0.0f;
    positive_events_ms_[0] = positive_events_ms_[1] = 0U;
    negative_events_ms_[0] = negative_events_ms_[1] = 0U;
    positive_event_index_ = negative_event_index_ = 0;
    positive_event_stored_ = negative_event_stored_ = false;
}

float RollController::slew_modifier(
    float sample,
    float dt_s,
    const RollControllerParams &params) {
    if (dt_s <= 0.0f) return 1.0f;

    elapsed_ms_ += dt_s * 1000.0f;
    const float derivative = (sample - slew_last_sample_) / dt_s;
    slew_last_sample_ = sample;
    slew_filter_output_ += lowpass_alpha(dt_s, kSlewDerivativeCutoffHz) *
        (derivative - slew_filter_output_);
    const float slew_rate = slew_filter_output_;
    const float tau = std::max(params.slew_rate_tau_s, 0.01f);
    const float decay_alpha = std::min(dt_s, tau) / tau;
    const float attack_alpha = std::min(2.0f * decay_alpha, 1.0f);

    if (slew_rate > max_positive_slew_) {
        max_positive_slew_ = slew_rate;
        max_positive_event_ms_ = elapsed_ms_;
    } else if (elapsed_ms_ - max_positive_event_ms_ > kSlewWindowMs) {
        max_positive_slew_ *= 1.0f - decay_alpha;
    }

    if (-slew_rate > max_negative_slew_) {
        max_negative_slew_ = -slew_rate;
        max_negative_event_ms_ = elapsed_ms_;
    } else if (elapsed_ms_ - max_negative_event_ms_ > kSlewWindowMs) {
        max_negative_slew_ *= 1.0f - decay_alpha;
    }

    const float raw_slew_rate = 0.5f * (max_positive_slew_ + max_negative_slew_);
    output_slew_rate_ = (1.0f - attack_alpha) * output_slew_rate_ +
        attack_alpha * raw_slew_rate;
    output_slew_rate_ = std::min(output_slew_rate_, raw_slew_rate);

    if (params.slew_rate_max_deg_s <= 0.0f) return 1.0f;

    const float limit = params.slew_rate_max_deg_s;
    const float limited_raw_slew_rate = 0.5f * (
        std::min(max_positive_slew_, 10.0f * limit) +
        std::min(max_negative_slew_, 10.0f * limit));
    const auto now_ms = static_cast<std::uint32_t>(std::max(elapsed_ms_, 0.0f));

    if (!positive_event_stored_ && slew_rate > limit) {
        positive_events_ms_[positive_event_index_] = now_ms;
        positive_event_index_ = (positive_event_index_ + 1) % 2;
        positive_event_stored_ = true;
        negative_event_stored_ = false;
    }
    if (!negative_event_stored_ && -slew_rate > limit) {
        negative_events_ms_[negative_event_index_] = now_ms;
        negative_event_index_ = (negative_event_index_ + 1) % 2;
        negative_event_stored_ = true;
        positive_event_stored_ = false;
    }

    std::uint32_t oldest_ms = now_ms;
    for (int index = 0; index < 2; ++index) {
        oldest_ms = std::min(oldest_ms, positive_events_ms_[index]);
        oldest_ms = std::min(oldest_ms, negative_events_ms_[index]);
    }

    float modifier_input = limited_raw_slew_rate;
    constexpr float event_window_ms = 3.0f * kSlewWindowMs;
    if (static_cast<float>(now_ms - oldest_ms) > event_window_ms) {
        const float outside_s = 0.001f *
            (static_cast<float>(now_ms - oldest_ms) - event_window_ms);
        modifier_input *= std::exp(-outside_s / tau);
    }

    modifier_slew_rate_ = (1.0f - attack_alpha) * modifier_slew_rate_ +
        attack_alpha * modifier_input;
    modifier_slew_rate_ = std::min(modifier_slew_rate_, modifier_input);

    if (modifier_slew_rate_ <= limit) return 1.0f;
    return limit / (limit + kSlewModifierGain * (modifier_slew_rate_ - limit));
}

RollControllerOutput RollController::update(
    const RollControllerParams &params,
    float target_bank_deg,
    float bank_deg,
    float roll_rate_rad_s,
    float equivalent_airspeed_m_s,
    float eas_to_tas,
    float authority,
    float dt_s) {
    RollControllerOutput result;
    dt_s = constrain(dt_s, 0.002f, 0.05f);
    authority = constrain(authority, 0.0f, 1.0f);

    result.angle_error_deg = wrap_180(target_bank_deg - bank_deg);
    const float tau = std::max(params.time_constant_s, 0.05f);
    result.desired_rate_deg_s = result.angle_error_deg / tau;
    if (params.maximum_rate_deg_s > 0.0f) {
        result.desired_rate_deg_s = constrain(
            result.desired_rate_deg_s,
            -params.maximum_rate_deg_s,
            params.maximum_rate_deg_s);
    }
    result.measured_rate_deg_s = roll_rate_rad_s * kRadiansToDegrees;

    const float airspeed_min = std::max(params.airspeed_min_m_s, 1.0f);
    const float airspeed_max = std::max(params.airspeed_max_m_s, airspeed_min + 1.0f);
    const float scale_min = std::min(0.5f, params.scaling_speed_m_s / (2.0f * airspeed_max));
    const float scale_max = std::max(2.0f, params.scaling_speed_m_s / (0.7f * airspeed_min));
    result.airspeed_scaler = equivalent_airspeed_m_s > 0.0001f
        ? params.scaling_speed_m_s / equivalent_airspeed_m_s
        : scale_max;
    result.airspeed_scaler = constrain(result.airspeed_scaler, scale_min, scale_max);
    result.underspeed = equivalent_airspeed_m_s <= airspeed_min;
    eas_to_tas = constrain(eas_to_tas, 1.0f, 3.0f);

    const float scaler_squared = result.airspeed_scaler * result.airspeed_scaler;
    const float raw_target = result.desired_rate_deg_s * kDegreesToRadians * scaler_squared;
    const float measurement = roll_rate_rad_s * scaler_squared;
    const float old_integrator = integrator_;

    if (filters_reset_) {
        filters_reset_ = false;
        filtered_target_ = raw_target;
        filtered_error_ = filtered_target_ - measurement;
        filtered_derivative_ = 0.0f;
        target_derivative_ = 0.0f;
    } else {
        const float previous_target = filtered_target_;
        filtered_target_ += lowpass_alpha(dt_s, params.target_filter_hz) *
            (raw_target - filtered_target_);
        const float previous_error = filtered_error_;
        const float raw_error = filtered_target_ - measurement;
        filtered_error_ += lowpass_alpha(dt_s, params.error_filter_hz) *
            (raw_error - filtered_error_);
        const float raw_derivative = (filtered_error_ - previous_error) / dt_s;
        filtered_derivative_ += lowpass_alpha(dt_s, params.derivative_filter_hz) *
            (raw_derivative - filtered_derivative_);
        target_derivative_ = (filtered_target_ - previous_target) / dt_s;
    }

    const bool output_limited = std::fabs(output_ratio_) >= authority - 0.0001f;
    const bool integrator_may_grow = !output_limited ||
        ((integrator_ > 0.0f && filtered_error_ < 0.0f) ||
         (integrator_ < 0.0f && filtered_error_ > 0.0f));
    if (params.i != 0.0f && integrator_may_grow) {
        integrator_ += filtered_error_ * params.i * dt_s;
        integrator_ = constrain(
            integrator_,
            -std::fabs(params.integrator_max),
            std::fabs(params.integrator_max));
    }
    if (result.underspeed) integrator_ = old_integrator;

    float p_rad = filtered_error_ * params.p;
    float d_rad = filtered_derivative_ * params.d;
    result.d_modifier = slew_modifier(
        (p_rad + d_rad) * 45.0f,
        dt_s,
        params);
    p_rad *= result.d_modifier;
    d_rad *= result.d_modifier;

    if (params.pd_max > 0.0f) {
        const float pd_sum = std::fabs(p_rad + d_rad);
        if (pd_sum > params.pd_max) {
            const float pd_scale = params.pd_max / pd_sum;
            p_rad *= pd_scale;
            d_rad *= pd_scale;
        }
    }

    result.p_deg = p_rad * kRadiansToDegrees;
    result.i_deg = integrator_ * kRadiansToDegrees;
    result.d_deg = d_rad * kRadiansToDegrees;
    result.ff_deg = filtered_target_ * params.ff * kRadiansToDegrees /
        std::max(result.airspeed_scaler * eas_to_tas, 0.01f);
    result.detected_slew_rate_deg_s = output_slew_rate_;

    const float raw_output_deg = result.p_deg + result.i_deg + result.d_deg + result.ff_deg;
    result.raw_output_ratio = raw_output_deg / 45.0f;
    const float authority_limited = constrain(result.raw_output_ratio, -authority, authority);
    result.saturated = std::fabs(result.raw_output_ratio - authority_limited) > 0.0001f;

    const float maximum_delta = std::max(params.output_slew_ratio_s, 0.0f) * dt_s;
    output_ratio_ += constrain(authority_limited - output_ratio_, -maximum_delta, maximum_delta);
    output_ratio_ = constrain(output_ratio_, -authority, authority);
    result.limited_output_ratio = output_ratio_;
    return result;
}

float RollController::neutral(float output_slew_ratio_s, float dt_s) {
    const float maximum_delta = std::max(output_slew_ratio_s, 0.0f) *
        constrain(dt_s, 0.002f, 0.05f);
    output_ratio_ += constrain(-output_ratio_, -maximum_delta, maximum_delta);
    integrator_ = 0.0f;
    filters_reset_ = true;
    return output_ratio_;
}

}  // namespace sr20g6_test
