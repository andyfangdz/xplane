/* SPDX-License-Identifier: GPL-3.0-or-later
 * Temporary SR20 G6 Chandelle video-evidence controller.
 */
#include <algorithm>
#include <cmath>
#include <cstdint>
#include <cstring>

#include "XPLMDataAccess.h"
#include "XPLMPlugin.h"
#include "XPLMProcessing.h"
#include "XPLMUtilities.h"
#include "ap_roll_controller_adapter.hpp"

namespace {
constexpr float kKnotToMps = 0.5144444444444445f;
constexpr float kRadToDeg = 57.29577951308232f;
constexpr const char *kCustomAcf = "SR20_G6_Custom_FM.acf";
constexpr const char *kTorqueSimOriginalAcf = "TorqueSim SR20/SR20.acf";
enum Mode { kNeutral = 0, kBank = 1, kAttitude = 2, kChandelle = 3 };
enum Release { kNone = 0, kDisarmed = 1, kWrongAircraft = 2, kPaused = 3,
    kReplay = 4, kBadTiming = 5, kConflict = 6, kDisabled = 7 };

XPLMDataRef acf_ref, paused_ref, replay_ref, bank_ref, pitch_ref, heading_ref;
XPLMDataRef p_ref, q_ref, r_ref, beta_ref, ias_ref, tas_ref, frame_ref;
XPLMDataRef gear_ground_ref, vvi_ref;
XPLMDataRef latitude_ref, longitude_ref, elevation_ref, throttle_ref;
XPLMDataRef override_roll_ref, override_pitch_ref, override_yaw_ref;
XPLMDataRef yoke_roll_ref, yoke_pitch_ref, yoke_yaw_ref;
XPLMFlightLoopID loop_id = nullptr;
XPLMDataRef registered[80] = {};
int registered_count = 0;

sr20g6_test::RollController roll_controller, pitch_controller;
sr20g6_test::RollControllerParams roll_params, pitch_params;
sr20g6_test::RollControllerOutput roll_output, pitch_output;

int enabled = 0, match = 0, armed = 0, active = 0;
int owns_roll = 0, owns_pitch = 0, owns_yaw = 0;
int mode = kBank, release_reason = kDisarmed, phase = 0, complete = 0;
int first_contact_latched = 0, first_contact_mask = 0, saw_airborne = 0;
float target_bank = 0.0f, target_pitch = 1.5f;
float roll_authority = 0.72f, pitch_authority = 0.95f, yaw_authority = 0.45f;
float start_heading = 60.0f, entry_pitch = 0.0f, maximum_pitch = 15.0f, direction = 1.0f;
float progress = 0.0f, scheduled_bank = 0.0f, scheduled_pitch = 0.0f;
float rollout_mid_progress = 150.0f, rollout_mid_bank = 20.0f;
float rollout_end_progress = 180.0f, rollout_terminal_shape = 0.30f;
float bank = 0.0f, pitch = 0.0f, heading = 0.0f, beta = 0.0f;
float roll_rate = 0.0f, pitch_rate = 0.0f, yaw_rate = 0.0f;
float eas = 0.0f, eas_to_tas = 1.0f, dt = 0.0f, loop_hz = 0.0f;
float roll_command = 0.0f, pitch_command = 0.0f, yaw_command = 0.0f, yaw_i = 0.0f;
float first_contact_pitch = 0.0f, first_contact_vvi = 0.0f;
float first_contact_bank = 0.0f, first_contact_heading = 0.0f, first_contact_beta = 0.0f;
float first_contact_ias = 0.0f, first_contact_throttle = 0.0f;
float ground_target_heading = 0.0f;
double first_contact_latitude = 0.0, first_contact_longitude = 0.0, first_contact_elevation = 0.0;

void reset_first_contact() {
    first_contact_latched = 0;
    first_contact_mask = 0;
    saw_airborne = 0;
    first_contact_pitch = 0.0f;
    first_contact_vvi = 0.0f;
    first_contact_bank = first_contact_heading = first_contact_beta = 0.0f;
    first_contact_ias = first_contact_throttle = 0.0f;
    first_contact_latitude = first_contact_longitude = first_contact_elevation = 0.0;
}

float limit(float value, float low, float high) { return std::max(low, std::min(high, value)); }
float wrap360(float value) {
    float result = std::fmod(value, 360.0f);
    return result < 0.0f ? result + 360.0f : result;
}
float wrap180(float value) {
    float result = wrap360(value);
    return result > 180.0f ? result - 360.0f : result;
}
float smooth(float value) {
    const float x = limit(value, 0.0f, 1.0f);
    return x * x * (3.0f - 2.0f * x);
}
bool ends_with_ci(const char *text, const char *suffix) {
    const std::size_t n = std::strlen(text), m = std::strlen(suffix);
    if (m > n) return false;
    for (std::size_t i = 0; i < m; ++i) {
        char a = text[n - m + i], b = suffix[i];
        if (a >= 'A' && a <= 'Z') a = static_cast<char>(a - 'A' + 'a');
        if (b >= 'A' && b <= 'Z') b = static_cast<char>(b - 'A' + 'a');
        if (a != b) return false;
    }
    return true;
}
void refresh_match() {
    char path[1024] = {};
    XPLMGetDatab(acf_ref, path, 0, static_cast<int>(sizeof(path) - 1));
    match = (ends_with_ci(path, kCustomAcf) || ends_with_ci(path, kTorqueSimOriginalAcf)) ? 1 : 0;
    if (!match) armed = 0;
}
void reset_controllers() {
    roll_controller.reset(bank, roll_rate / kRadToDeg);
    pitch_controller.reset(pitch, pitch_rate / kRadToDeg);
    yaw_i = 0.0f;
}
void release_all(int reason) {
    if (owns_roll) { XPLMSetDataf(yoke_roll_ref, 0.0f); XPLMSetDatai(override_roll_ref, 0); }
    if (owns_pitch) { XPLMSetDataf(yoke_pitch_ref, 0.0f); XPLMSetDatai(override_pitch_ref, 0); }
    if (owns_yaw) { XPLMSetDataf(yoke_yaw_ref, 0.0f); XPLMSetDatai(override_yaw_ref, 0); }
    owns_roll = owns_pitch = owns_yaw = active = 0;
    roll_command = pitch_command = yaw_command = 0.0f;
    release_reason = reason;
    reset_controllers();
}
bool acquire(bool all_axes) {
    if (!owns_roll) {
        if (XPLMGetDatai(override_roll_ref)) return false;
        XPLMSetDatai(override_roll_ref, 1); owns_roll = XPLMGetDatai(override_roll_ref) ? 1 : 0;
        if (!owns_roll) return false;
    }
    if (all_axes && !owns_pitch) {
        if (XPLMGetDatai(override_pitch_ref)) return false;
        XPLMSetDatai(override_pitch_ref, 1); owns_pitch = XPLMGetDatai(override_pitch_ref) ? 1 : 0;
        if (!owns_pitch) return false;
    }
    if (all_axes && !owns_yaw) {
        if (XPLMGetDatai(override_yaw_ref)) return false;
        XPLMSetDatai(override_yaw_ref, 1); owns_yaw = XPLMGetDatai(override_yaw_ref) ? 1 : 0;
        if (!owns_yaw) return false;
    }
    return true;
}

int read_int(void *refcon) {
    switch (reinterpret_cast<std::intptr_t>(refcon)) {
        case 0: return 1; case 1: return 9; case 2: return enabled; case 3: return match;
        case 4: return armed; case 5: return active; case 6: return owns_roll; case 7: return mode;
        case 8: return release_reason; case 9: return phase; case 10: return complete;
        case 11: return first_contact_latched; case 12: return first_contact_mask;
        case 13: return saw_airborne;
        default: return 0;
    }
}
void write_int(void *refcon, int value) {
    const auto slot = reinterpret_cast<std::intptr_t>(refcon);
    if (slot == 4) {
        if (value && !armed) reset_first_contact();
        armed = value ? 1 : 0;
        if (!armed) release_all(kDisarmed);
    } else if (slot == 7) {
        const int next = static_cast<int>(limit(static_cast<float>(value), 0.0f, 3.0f));
        if (next != mode) reset_controllers();
        mode = next; phase = 0; complete = 0;
    }
}
float read_float(void *refcon) {
    switch (reinterpret_cast<std::intptr_t>(refcon)) {
        case 0: return target_bank; case 1: return roll_authority; case 2: return target_pitch;
        case 3: return pitch_authority; case 4: return yaw_authority; case 5: return start_heading;
        case 6: return entry_pitch; case 7: return maximum_pitch; case 8: return direction;
        case 9: return progress; case 10: return scheduled_bank; case 11: return scheduled_pitch;
        case 12: return roll_command; case 13: return pitch_command; case 14: return yaw_command;
        case 15: return loop_hz; case 16: return bank; case 17: return pitch; case 18: return heading;
        case 19: return beta; case 20: return rollout_mid_progress; case 21: return rollout_mid_bank;
        case 22: return rollout_end_progress; case 23: return rollout_terminal_shape;
        case 24: return first_contact_pitch; case 25: return first_contact_vvi;
        case 26: return first_contact_bank; case 27: return first_contact_heading;
        case 28: return first_contact_beta; case 29: return first_contact_ias;
        case 30: return first_contact_throttle;
        case 31: return ground_target_heading;
        default: return 0.0f;
    }
}
void write_float(void *refcon, float value) {
    switch (reinterpret_cast<std::intptr_t>(refcon)) {
        case 0: target_bank = limit(value, -60.0f, 60.0f); break;
        case 1: roll_authority = limit(value, 0.0f, 1.0f); break;
        case 2: target_pitch = limit(value, -20.0f, 35.0f); break;
        case 3: pitch_authority = limit(value, 0.0f, 1.0f); break;
        case 4: yaw_authority = limit(value, 0.0f, 1.0f); break;
        case 5: start_heading = wrap360(value); break;
        case 6: entry_pitch = limit(value, -10.0f, 15.0f); break;
        case 7: maximum_pitch = limit(value, 5.0f, 35.0f); break;
        case 8: direction = value < 0.0f ? -1.0f : 1.0f; break;
        case 20: rollout_mid_progress = limit(value, 120.0f, 165.0f); break;
        case 21: rollout_mid_bank = limit(value, 10.0f, 28.0f); break;
        case 22: rollout_end_progress = limit(value, 177.0f, 180.0f); break;
        case 23: rollout_terminal_shape = limit(value, 0.15f, 1.0f); break;
        case 31: ground_target_heading = wrap360(value); break;
        default: break;
    }
}
double read_double(void *refcon) {
    switch (reinterpret_cast<std::intptr_t>(refcon)) {
        case 0: return first_contact_latitude;
        case 1: return first_contact_longitude;
        case 2: return first_contact_elevation;
        default: return 0.0;
    }
}
void remember(XPLMDataRef ref) { if (ref && registered_count < 80) registered[registered_count++] = ref; }
void reg_int(const char *name, int slot, bool writable) {
    remember(XPLMRegisterDataAccessor(name, xplmType_Int, writable ? 1 : 0, read_int,
        writable ? write_int : nullptr, nullptr, nullptr, nullptr, nullptr, nullptr, nullptr,
        nullptr, nullptr, nullptr, nullptr, reinterpret_cast<void *>(static_cast<std::intptr_t>(slot)),
        reinterpret_cast<void *>(static_cast<std::intptr_t>(slot))));
}
void reg_float(const char *name, int slot, bool writable) {
    remember(XPLMRegisterDataAccessor(name, xplmType_Float, writable ? 1 : 0, nullptr, nullptr,
        read_float, writable ? write_float : nullptr, nullptr, nullptr, nullptr, nullptr, nullptr,
        nullptr, nullptr, nullptr, reinterpret_cast<void *>(static_cast<std::intptr_t>(slot)),
        reinterpret_cast<void *>(static_cast<std::intptr_t>(slot))));
}
void reg_double(const char *name, int slot) {
    remember(XPLMRegisterDataAccessor(name, xplmType_Double, 0, nullptr, nullptr,
        nullptr, nullptr, read_double, nullptr, nullptr, nullptr, nullptr, nullptr,
        nullptr, nullptr, reinterpret_cast<void *>(static_cast<std::intptr_t>(slot)), nullptr));
}
void register_datarefs() {
    const char *ints[] = {"version_major", "version_minor", "plugin_enabled", "aircraft_match",
        "armed", "active", "owns_roll_override", "mode", "release_reason", "phase", "complete",
        "first_contact_latched", "first_contact_mask", "saw_airborne"};
    for (int i = 0; i < 14; ++i) {
        char name[128] = "sr20g6/test_controller/"; std::strcat(name, ints[i]);
        reg_int(name, i, i == 4 || i == 7);
    }
    const char *floats[] = {"target_bank_deg", "authority", "target_pitch_deg", "pitch_authority",
        "yaw_authority", "start_heading_deg", "entry_pitch_deg", "maximum_pitch_deg", "direction",
        "heading_progress_deg", "scheduled_bank_deg", "scheduled_pitch_deg", "command_ratio",
        "pitch_command_ratio", "yaw_command_ratio", "loop_hz", "bank_deg", "pitch_deg",
        "heading_deg", "beta_deg", "rollout_mid_progress_deg", "rollout_mid_bank_deg",
        "rollout_end_progress_deg", "rollout_terminal_shape", "first_contact_pitch_deg",
        "first_contact_vvi_fpm", "first_contact_bank_deg", "first_contact_heading_deg",
        "first_contact_beta_deg", "first_contact_ias_kias", "first_contact_throttle_ratio",
        "ground_target_heading_deg"};
    for (int i = 0; i < 32; ++i) {
        char name[128] = "sr20g6/test_controller/"; std::strcat(name, floats[i]);
        reg_float(name, i, i <= 8 || (i >= 20 && i <= 23) || i == 31);
    }
    reg_double("sr20g6/test_controller/first_contact_latitude", 0);
    reg_double("sr20g6/test_controller/first_contact_longitude", 1);
    reg_double("sr20g6/test_controller/first_contact_elevation_m", 2);
}
bool resolve() {
    acf_ref = XPLMFindDataRef("sim/aircraft/view/acf_relative_path");
    paused_ref = XPLMFindDataRef("sim/time/paused"); replay_ref = XPLMFindDataRef("sim/time/is_in_replay");
    bank_ref = XPLMFindDataRef("sim/flightmodel/position/phi"); pitch_ref = XPLMFindDataRef("sim/flightmodel/position/theta");
    heading_ref = XPLMFindDataRef("sim/flightmodel/position/psi"); p_ref = XPLMFindDataRef("sim/flightmodel/position/Prad");
    q_ref = XPLMFindDataRef("sim/flightmodel/position/Qrad"); r_ref = XPLMFindDataRef("sim/flightmodel/position/Rrad");
    beta_ref = XPLMFindDataRef("sim/flightmodel/position/beta"); ias_ref = XPLMFindDataRef("sim/flightmodel/position/indicated_airspeed");
    tas_ref = XPLMFindDataRef("sim/flightmodel/position/true_airspeed"); frame_ref = XPLMFindDataRef("sim/operation/misc/frame_rate_period");
    gear_ground_ref = XPLMFindDataRef("sim/flightmodel2/gear/on_ground");
    vvi_ref = XPLMFindDataRef("sim/flightmodel/position/vh_ind_fpm");
    latitude_ref = XPLMFindDataRef("sim/flightmodel/position/latitude");
    longitude_ref = XPLMFindDataRef("sim/flightmodel/position/longitude");
    elevation_ref = XPLMFindDataRef("sim/flightmodel/position/elevation");
    throttle_ref = XPLMFindDataRef("sim/cockpit2/engine/actuators/throttle_ratio_all");
    override_roll_ref = XPLMFindDataRef("sim/operation/override/override_joystick_roll");
    override_pitch_ref = XPLMFindDataRef("sim/operation/override/override_joystick_pitch");
    override_yaw_ref = XPLMFindDataRef("sim/operation/override/override_joystick_heading");
    yoke_roll_ref = XPLMFindDataRef("sim/joystick/yoke_roll_ratio"); yoke_pitch_ref = XPLMFindDataRef("sim/joystick/yoke_pitch_ratio");
    yoke_yaw_ref = XPLMFindDataRef("sim/joystick/yoke_heading_ratio");
    return acf_ref && paused_ref && replay_ref && bank_ref && pitch_ref && heading_ref && p_ref && q_ref && r_ref &&
        beta_ref && ias_ref && tas_ref && frame_ref && override_roll_ref && override_pitch_ref && override_yaw_ref &&
        yoke_roll_ref && yoke_pitch_ref && yoke_yaw_ref && gear_ground_ref && vvi_ref &&
        latitude_ref && longitude_ref && elevation_ref && throttle_ref;
}

float callback(float, float, int, void *) {
    bank = XPLMGetDataf(bank_ref); pitch = XPLMGetDataf(pitch_ref); heading = XPLMGetDataf(heading_ref);
    const float p_rad = XPLMGetDataf(p_ref), q_rad = XPLMGetDataf(q_ref);
    roll_rate = p_rad * kRadToDeg; pitch_rate = q_rad * kRadToDeg; yaw_rate = XPLMGetDataf(r_ref) * kRadToDeg;
    beta = XPLMGetDataf(beta_ref); eas = std::max(0.0f, XPLMGetDataf(ias_ref)) * kKnotToMps;
    const float tas = std::max(0.0f, XPLMGetDataf(tas_ref)); eas_to_tas = eas > 1.0f ? std::max(1.0f, tas / eas) : 1.0f;
    dt = XPLMGetDataf(frame_ref); loop_hz = dt > 0.0f ? 1.0f / dt : 0.0f;
    if (armed) {
        int gear_ground[10] = {};
        XPLMGetDatavi(gear_ground_ref, gear_ground, 0, 10);
        const int mask = (gear_ground[0] ? 1 : 0) | (gear_ground[1] ? 2 : 0) | (gear_ground[2] ? 4 : 0);
        if (mask == 0) saw_airborne = 1;
        else if (saw_airborne && !first_contact_latched) {
            first_contact_latched = 1;
            first_contact_mask = mask;
            first_contact_pitch = pitch;
            first_contact_vvi = XPLMGetDataf(vvi_ref);
            first_contact_bank = bank;
            first_contact_heading = heading;
            first_contact_beta = beta;
            first_contact_ias = XPLMGetDataf(ias_ref);
            first_contact_throttle = XPLMGetDataf(throttle_ref);
            first_contact_latitude = XPLMGetDatad(latitude_ref);
            first_contact_longitude = XPLMGetDatad(longitude_ref);
            first_contact_elevation = XPLMGetDatad(elevation_ref);
        }
    }
    if (!enabled) { release_all(kDisabled); return -1.0f; }
    if (!match) { release_all(kWrongAircraft); return -1.0f; }
    if (!armed) { release_all(kDisarmed); return -1.0f; }
    if (XPLMGetDatai(paused_ref)) { release_all(kPaused); return -1.0f; }
    if (XPLMGetDatai(replay_ref)) { release_all(kReplay); return -1.0f; }
    // Native 1080p recording can occasionally produce a 50-75 ms frame even
    // when the steady controller rate is much higher.  Retain fail-safe
    // release for genuinely stale timing, but do not drop all three axes for a
    // single capture-induced frame stall.
    if (!std::isfinite(dt) || dt < 0.002f || dt > 0.075f) { release_all(kBadTiming); return -1.0f; }
    const bool all_axes = mode == kAttitude || mode == kChandelle;
    if (!acquire(all_axes)) { release_all(kConflict); return -1.0f; }
    active = 1; release_reason = kNone;
    scheduled_bank = target_bank; scheduled_pitch = target_pitch;
    if (mode == kChandelle) {
        progress = wrap360(direction * (heading - start_heading));
        if (progress <= 90.0f) {
            phase = 1; scheduled_bank = direction * 30.0f;
            scheduled_pitch = entry_pitch + (maximum_pitch - entry_pitch) * smooth(progress / 90.0f);
        } else if (progress < 180.0f) {
            phase = 2;
            float bank_magnitude = 0.0f;
            const float mid = std::min(rollout_mid_progress, rollout_end_progress - 3.0f);
            if (progress <= mid) {
                const float u = (progress - 90.0f) / std::max(1.0f, mid - 90.0f);
                bank_magnitude = 30.0f + (rollout_mid_bank - 30.0f) * smooth(u);
            } else if (progress < rollout_end_progress) {
                const float remaining = (rollout_end_progress - progress) /
                    std::max(1.0f, rollout_end_progress - mid);
                bank_magnitude = rollout_mid_bank * std::pow(limit(remaining, 0.0f, 1.0f),
                    rollout_terminal_shape);
            }
            scheduled_bank = direction * bank_magnitude;
            scheduled_pitch = maximum_pitch;
        } else {
            phase = 3; complete = 1; scheduled_bank = 0.0f; scheduled_pitch = maximum_pitch;
        }
    } else { progress = 0.0f; phase = 0; }
    if (mode == kNeutral) roll_command = roll_controller.neutral(roll_params.output_slew_ratio_s, dt);
    else {
        roll_output = roll_controller.update(roll_params, scheduled_bank, bank, p_rad, eas, eas_to_tas, roll_authority, dt);
        roll_command = roll_output.limited_output_ratio;
    }
    if (all_axes) {
        pitch_output = pitch_controller.update(pitch_params, scheduled_pitch, pitch, q_rad, eas, eas_to_tas, pitch_authority, dt);
        pitch_command = pitch_output.limited_output_ratio;
        if (mode == kChandelle) {
            const float q_scale = limit(40.0f / std::max(eas, 20.0f), 0.55f, 1.45f);
            pitch_command = limit(pitch_command + 0.90f * smooth(progress / 90.0f) * q_scale * q_scale,
                -pitch_authority, pitch_authority);
        }
        float desired_yaw = 0.0f;
        if (first_contact_latched) {
            yaw_i = 0.0f;
            const float heading_error = wrap180(ground_target_heading - heading);
            desired_yaw = limit(0.030f * heading_error - 0.015f * yaw_rate - 0.015f * beta,
                -0.55f, 0.55f);
            yaw_command += limit(desired_yaw - yaw_command, -1.5f * dt, 1.5f * dt);
        } else {
            const float yaw_error = -beta;
            yaw_i = limit(yaw_i + yaw_error * 0.10f * dt, -0.25f, 0.25f);
            desired_yaw = limit(yaw_authority * (0.085f * yaw_error - 0.012f * yaw_rate + yaw_i), -1.0f, 1.0f);
            yaw_command += limit(desired_yaw - yaw_command, -0.9f * dt, 0.9f * dt);
        }
    } else { pitch_command = yaw_command = yaw_i = 0.0f; }
    XPLMSetDataf(yoke_roll_ref, roll_command);
    if (all_axes) { XPLMSetDataf(yoke_pitch_ref, pitch_command); XPLMSetDataf(yoke_yaw_ref, yaw_command); }
    return -1.0f;
}
}  // namespace

PLUGIN_API int XPluginStart(char *name, char *signature, char *description) {
    std::strncpy(name, "SR20 G6 Chandelle Video Controller", 255); name[255] = '\0';
    std::strncpy(signature, "andyf.sr20g6.testcontroller.ardupilot", 255); signature[255] = '\0';
    std::strncpy(description, "Temporary high-rate controller for complete Chandelle video evidence.", 255); description[255] = '\0';
    roll_params.time_constant_s = 0.50f; roll_params.p = 0.14f; roll_params.i = 0.30f;
    roll_params.ff = 0.90f; roll_params.output_slew_ratio_s = 1.20f;
    pitch_params.time_constant_s = 0.50f; pitch_params.maximum_rate_deg_s = 22.0f;
    pitch_params.p = 0.30f; pitch_params.i = 1.50f; pitch_params.ff = 2.50f;
    pitch_params.output_slew_ratio_s = 1.20f;
    if (!resolve()) return 0;
    register_datarefs();
    XPLMCreateFlightLoop_t params = {};
    params.structSize = sizeof(params); params.phase = xplm_FlightLoop_Phase_BeforeFlightModel; params.callbackFunc = callback;
    loop_id = XPLMCreateFlightLoop(&params); if (!loop_id) return 0;
    refresh_match(); XPLMDebugString("[SR20G6TestController] v1.9 exact custom/TorqueSim test-aircraft allow-list loaded inert.\n");
    return 1;
}
PLUGIN_API void XPluginStop(void) {
    release_all(kDisabled); if (loop_id) XPLMDestroyFlightLoop(loop_id);
    for (int i = 0; i < registered_count; ++i) XPLMUnregisterDataAccessor(registered[i]);
}
PLUGIN_API int XPluginEnable(void) { enabled = 1; refresh_match(); XPLMScheduleFlightLoop(loop_id, -1.0f, 1); return 1; }
PLUGIN_API void XPluginDisable(void) { enabled = armed = 0; release_all(kDisabled); XPLMScheduleFlightLoop(loop_id, 0.0f, 1); }
PLUGIN_API void XPluginReceiveMessage(XPLMPluginID, int message, void *parameter) {
    if (message == XPLM_MSG_PLANE_LOADED && reinterpret_cast<std::intptr_t>(parameter) == 0) {
        armed = 0; phase = complete = 0; reset_first_contact(); release_all(kDisarmed); refresh_match();
    }
}
