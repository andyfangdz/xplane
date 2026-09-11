#pragma once

/*
 * SR20 G6 flight-test roll controller.
 *
 * SPDX-License-Identifier: GPL-3.0-or-later
 *
 * This controller is a focused X-Plane adapter of the ArduPilot fixed-wing
 * roll-controller path in AP_FW_Controller, AP_RollController, AC_PID, and
 * Filter/SlewLimiter.  It intentionally retains ArduPilot's angle-to-rate,
 * airspeed-scaling, filtered PID+FF, integrator limiting, and oscillation
 * slew-limiter structure while removing the AP_HAL/AP_Param dependencies.
 *
 * Upstream: https://github.com/ArduPilot/ardupilot
 * Pinned source commit: 4fe7ad4fab8c1bf4ade7cbc7ae85a73c81d73e05
 */

#include <cstdint>

namespace sr20g6_test {

struct RollControllerParams {
    float time_constant_s = 0.70f;
    float maximum_rate_deg_s = 35.0f;
    float p = 0.08f;
    float i = 0.15f;
    float d = 0.0f;
    float ff = 0.345f;
    float integrator_max = 0.666f;
    float target_filter_hz = 3.0f;
    float error_filter_hz = 0.0f;
    float derivative_filter_hz = 12.0f;
    float pd_max = 0.0f;
    float slew_rate_max_deg_s = 150.0f;
    float slew_rate_tau_s = 1.0f;
    float scaling_speed_m_s = 30.0f;
    float airspeed_min_m_s = 29.0f;
    float airspeed_max_m_s = 90.0f;
    float output_slew_ratio_s = 0.80f;
};

struct RollControllerOutput {
    float angle_error_deg = 0.0f;
    float desired_rate_deg_s = 0.0f;
    float measured_rate_deg_s = 0.0f;
    float airspeed_scaler = 1.0f;
    float p_deg = 0.0f;
    float i_deg = 0.0f;
    float d_deg = 0.0f;
    float ff_deg = 0.0f;
    float d_modifier = 1.0f;
    float detected_slew_rate_deg_s = 0.0f;
    float raw_output_ratio = 0.0f;
    float limited_output_ratio = 0.0f;
    bool saturated = false;
    bool underspeed = false;
};

class RollController {
public:
    void reset(float bank_deg, float roll_rate_rad_s);

    RollControllerOutput update(
        const RollControllerParams &params,
        float target_bank_deg,
        float bank_deg,
        float roll_rate_rad_s,
        float equivalent_airspeed_m_s,
        float eas_to_tas,
        float authority,
        float dt_s);

    float neutral(float output_slew_ratio_s, float dt_s);
    float current_output_ratio() const { return output_ratio_; }

private:
    static float constrain(float value, float minimum, float maximum);
    static float wrap_180(float angle_deg);
    static float lowpass_alpha(float dt_s, float cutoff_hz);
    float slew_modifier(float sample, float dt_s, const RollControllerParams &params);

    bool filters_reset_ = true;
    float filtered_target_ = 0.0f;
    float filtered_error_ = 0.0f;
    float filtered_derivative_ = 0.0f;
    float target_derivative_ = 0.0f;
    float integrator_ = 0.0f;
    float output_ratio_ = 0.0f;

    float elapsed_ms_ = 0.0f;
    float slew_filter_output_ = 0.0f;
    float slew_last_sample_ = 0.0f;
    float max_positive_slew_ = 0.0f;
    float max_negative_slew_ = 0.0f;
    float max_positive_event_ms_ = 0.0f;
    float max_negative_event_ms_ = 0.0f;
    float output_slew_rate_ = 0.0f;
    float modifier_slew_rate_ = 0.0f;
    std::uint32_t positive_events_ms_[2] = {0U, 0U};
    std::uint32_t negative_events_ms_[2] = {0U, 0U};
    int positive_event_index_ = 0;
    int negative_event_index_ = 0;
    bool positive_event_stored_ = false;
    bool negative_event_stored_ = false;
};

}  // namespace sr20g6_test
