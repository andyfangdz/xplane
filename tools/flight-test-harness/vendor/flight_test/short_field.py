"""Live and replayable KCDW runway 22 short-field landing orchestration."""

from __future__ import annotations

import argparse
from dataclasses import dataclass
from datetime import datetime, timezone
import json
import math
from pathlib import Path
import subprocess
import sys
import time
from typing import Any, Iterable

from .api import XPlaneApi, XPlaneConnectionError
from .geometry import (
    FEET_PER_METER,
    FEET_PER_NM,
    KCDW_RUNWAY_22,
    RunwayGeometry,
    limit,
    quaternion,
    wrap_180,
)
from .landing import (
    REFERENCE_CONTROL_PERIOD_S,
    LandingConfig,
    LandingGuidance,
    evaluate_result,
    flare_dynamics,
    metric_range,
)

KNOT_TO_MPS = 0.5144444444444445
PLANEPATH_RELEASED = [0] * 20
PLANEPATH_OWNED = [1] + [0] * 19

SOUND_DATAREFS = (
    "sim/operation/sound/master_volume_ratio",
    "sim/operation/sound/engine_volume_ratio",
    "sim/operation/sound/prop_volume_ratio",
    "sim/operation/sound/interior_volume_ratio",
    "sim/operation/sound/exterior_volume_ratio",
    "sim/operation/sound/warning_volume_ratio",
)

CUSTOM_MOD_DATAREFS = (
    "sr20g6/custom_fm/active",
    "sr20g6/custom_fm/enabled",
    "sr20g6/custom_fm/aircraft_match",
    "sr20g6/custom_fm/version_minor",
    "sr20g6/io390/active",
    "sr20g6/io390/enabled",
    "sr20g6/io390/aircraft_match",
    "sr20g6/io390/version_minor",
)

TORQUESIM_MASS_DATAREFS = (
    "afm/sr/mass/pilot_kg",
    "afm/sr/mass/copilot_kg",
    "afm/sr/mass/paxL_kg",
    "afm/sr/mass/paxR_kg",
    "afm/sr/mass/baggage_kg",
    "afm/sr/mass/payload_kg",
    "afm/sr/mass/total_kg",
    "afm/sr/mass/cg_in",
)

TORQUESIM_ENGINE_DATAREFS = (
    "sim/time/total_flight_time_sec",
    "sim/flightmodel/engine/ENGN_power",
    "sim/flightmodel/engine/ENGN_FF_",
    "sim/cockpit2/engine/indicators/engine_speed_rpm",
)

REQUIRED_DATAREFS = (
    "sim/time/paused",
    "sim/operation/override/override_planepath",
    "sim/operation/override/override_artstab",
    "sim/operation/override/override_toe_brakes",
    "sim/flightmodel/position/q",
    "sim/flightmodel/position/local_x",
    "sim/flightmodel/position/local_y",
    "sim/flightmodel/position/local_z",
    "sim/flightmodel/position/local_vx",
    "sim/flightmodel/position/local_vy",
    "sim/flightmodel/position/local_vz",
    "sim/flightmodel/position/P",
    "sim/flightmodel/position/Q",
    "sim/flightmodel/position/R",
    "sim/flightmodel/position/latitude",
    "sim/flightmodel/position/longitude",
    "sim/flightmodel/position/elevation",
    "sim/flightmodel/position/y_agl",
    "sim/flightmodel/position/psi",
    "sim/flightmodel/position/phi",
    "sim/flightmodel/position/theta",
    "sim/flightmodel/position/beta",
    "sim/flightmodel/position/groundspeed",
    "sim/flightmodel/position/indicated_airspeed",
    "sim/flightmodel/position/vh_ind_fpm",
    "sim/flightmodel/forces/g_nrml",
    "sim/cockpit2/controls/flap_ratio",
    "sim/cockpit2/controls/elevator_trim",
    "sim/flightmodel2/controls/flap_handle_deploy_ratio",
    "sim/cockpit2/engine/actuators/throttle_ratio_all",
    "sim/cockpit2/engine/actuators/mixture_ratio_all",
    "sim/cockpit2/controls/left_brake_ratio",
    "sim/cockpit2/controls/right_brake_ratio",
    "sim/cockpit2/controls/wheel_brake_ratio_applied",
    "sim/flightmodel/weight/m_total",
    "sim/flightmodel/weight/m_stations",
    "sim/aircraft/weight/acf_m_station_max",
    "sim/flightmodel/parts/tire_x_no_deflection",
    "sim/flightmodel/parts/tire_y_no_deflection",
    "sim/flightmodel/parts/tire_z_no_deflection",
    "sim/flightmodel2/gear/on_ground",
    "sim/flightmodel2/gear/tire_skid_ratio",
    "sr20g6/test_controller/version_minor",
    "sr20g6/test_controller/plugin_enabled",
    "sr20g6/test_controller/aircraft_match",
    "sr20g6/test_controller/armed",
    "sr20g6/test_controller/active",
    "sr20g6/test_controller/mode",
    "sr20g6/test_controller/release_reason",
    "sr20g6/test_controller/target_bank_deg",
    "sr20g6/test_controller/target_pitch_deg",
    "sr20g6/test_controller/authority",
    "sr20g6/test_controller/pitch_authority",
    "sr20g6/test_controller/yaw_authority",
    "sr20g6/test_controller/command_ratio",
    "sr20g6/test_controller/pitch_command_ratio",
    "sr20g6/test_controller/yaw_command_ratio",
    "sr20g6/test_controller/loop_hz",
    "sr20g6/test_controller/first_contact_latched",
    "sr20g6/test_controller/first_contact_mask",
    "sr20g6/test_controller/first_contact_pitch_deg",
    "sr20g6/test_controller/first_contact_vvi_fpm",
    "sr20g6/test_controller/first_contact_bank_deg",
    "sr20g6/test_controller/first_contact_heading_deg",
    "sr20g6/test_controller/first_contact_beta_deg",
    "sr20g6/test_controller/first_contact_ias_kias",
    "sr20g6/test_controller/first_contact_throttle_ratio",
    "sr20g6/test_controller/first_contact_latitude",
    "sr20g6/test_controller/first_contact_longitude",
    "sr20g6/test_controller/first_contact_elevation_m",
    "sr20g6/test_controller/ground_target_heading_deg",
)

CONTROL_DATAREFS = (
    "sim/flightmodel/position/latitude",
    "sim/flightmodel/position/longitude",
    "sim/flightmodel/position/elevation",
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
    "sim/flightmodel2/gear/tire_skid_ratio",
    "sim/flightmodel/weight/m_total",
)

FULL_SAMPLE_DATAREFS = (
    "sim/operation/override/override_planepath",
    "sim/flightmodel/position/latitude",
    "sim/flightmodel/position/longitude",
    "sim/flightmodel/position/elevation",
    "sim/flightmodel/position/y_agl",
    "sim/flightmodel/position/indicated_airspeed",
    "sim/flightmodel/position/groundspeed",
    "sim/flightmodel/position/vh_ind_fpm",
    "sim/flightmodel/position/psi",
    "sim/flightmodel/position/phi",
    "sim/flightmodel/position/theta",
    "sim/flightmodel/position/beta",
    "sim/flightmodel/forces/g_nrml",
    "sim/cockpit2/engine/actuators/throttle_ratio_all",
    "sim/cockpit2/controls/flap_ratio",
    "sim/cockpit2/controls/elevator_trim",
    "sim/flightmodel2/controls/flap_handle_deploy_ratio",
    "sim/cockpit2/controls/left_brake_ratio",
    "sim/cockpit2/controls/right_brake_ratio",
    "sim/cockpit2/controls/wheel_brake_ratio_applied",
    "sim/flightmodel2/gear/on_ground",
    "sim/flightmodel2/gear/tire_skid_ratio",
    "sr20g6/test_controller/active",
    "sr20g6/test_controller/target_bank_deg",
    "sr20g6/test_controller/target_pitch_deg",
    "sr20g6/test_controller/command_ratio",
    "sr20g6/test_controller/pitch_command_ratio",
    "sr20g6/test_controller/yaw_command_ratio",
    "sr20g6/test_controller/loop_hz",
    "sr20g6/test_controller/first_contact_latched",
    "sr20g6/test_controller/first_contact_mask",
    "sr20g6/test_controller/first_contact_pitch_deg",
    "sr20g6/test_controller/first_contact_vvi_fpm",
    "sr20g6/test_controller/first_contact_bank_deg",
    "sr20g6/test_controller/first_contact_heading_deg",
    "sr20g6/test_controller/first_contact_beta_deg",
    "sr20g6/test_controller/first_contact_ias_kias",
    "sr20g6/test_controller/first_contact_throttle_ratio",
    "sr20g6/test_controller/first_contact_latitude",
    "sr20g6/test_controller/first_contact_longitude",
    "sr20g6/test_controller/first_contact_elevation_m",
    "sim/flightmodel/weight/m_total",
)


@dataclass(slots=True)
class EvidenceOptions:
    record_video: bool = False
    record_audio: bool = False
    prepare_video_view: bool = False
    pre_release_hold_seconds: float = 0.0
    video_fov_deg: float = 82.0
    video_head_pitch_deg: float = -4.0
    audio_capture_script: Path | None = None
    audio_module_dir: Path | None = None
    audio_output_path: Path | None = None
    audio_stop_file: Path | None = None
    audio_ready_file: Path | None = None
    audio_metadata_path: Path | None = None
    audio_sample_rate: int = 48000


def utc_now() -> datetime:
    return datetime.now(timezone.utc)


def scalar(value: Any) -> Any:
    return value[0] if isinstance(value, list) else value


class ShortFieldRunner:
    def __init__(
        self,
        api: XPlaneApi,
        config: LandingConfig,
        output_path: Path,
        evidence: EvidenceOptions | None = None,
        runway: RunwayGeometry = KCDW_RUNWAY_22,
    ) -> None:
        self.api = api
        self.config = config
        self.output_path = output_path.resolve()
        self.evidence = evidence or EvidenceOptions()
        self.runway = runway
        self.video_recording_started = False
        self.video_start_utc: datetime | None = None
        self.video_stop_utc: datetime | None = None
        self.audio_process: subprocess.Popen[bytes] | None = None
        self.audio_recording_started = False
        self.audio_ready: Any = None
        self.audio_capture_failure: str | None = None
        self.sound_verification: dict[str, float] = {}
        self.mass_setup: dict[str, Any] = {}
        self.engine_setup: dict[str, Any] = {}
        self._torquesim_flap_detent: int | None = None

    def _requires_custom_mod(self) -> bool:
        return bool(int(getattr(self.config, "require_custom_mod", 1)))

    def _uses_torquesim_engine(self) -> bool:
        return bool(int(getattr(self.config, "use_torquesim_engine", 0)))

    def _initial_ias_target(self) -> float:
        return float(getattr(self.config, "approach_kias", 75.0))

    def _safe_set(self, name: str, value: Any) -> None:
        try:
            self.api.set_dataref(name, value)
        except Exception:
            pass

    def _set_pause(self, paused: bool) -> None:
        command = "sim/operation/pause_on" if paused else "sim/operation/pause_off"
        expected = 1 if paused else 0
        for _ in range(40):
            try:
                self.api.command(command)
            except XPlaneConnectionError:
                # X-Plane can retire an idle keep-alive connection while an
                # evidence gate is holding.  pause_on/pause_off are
                # idempotent, so verify the achieved state and retry on the
                # fresh connection installed by XPlaneApi if needed.
                pass
            time.sleep(0.15)
            if int(self.api.get_scalar("sim/time/paused")) == expected:
                return
        raise RuntimeError(f"Could not set paused={paused}")

    def _request_torquesim_native_mfd(self) -> None:
        """Request TorqueSim's documented startup bypass before EIS owns the MFD."""
        if not (
            self._uses_torquesim_engine()
            and (self.evidence.record_video or self.evidence.prepare_video_view)
        ):
            return
        if "sim/GPS/g1000n3_ent" in self.api.commands:
            self.api.command("sim/GPS/g1000n3_ent", 0.2)
            time.sleep(2.0)
        if "sim/GPS/g1000n3_softkey12" in self.api.commands:
            for _ in range(3):
                self.api.command("sim/GPS/g1000n3_softkey12", 0.2)
                time.sleep(0.75)

    def _wait_for_catalog(self, timeout: float = 180.0) -> None:
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            time.sleep(1.0)
            try:
                self.api.refresh_catalogs()
                if len(self.api.datarefs) > 1000:
                    return
            except Exception:
                pass
        raise RuntimeError("Fresh POST /flight did not repopulate the catalog.")

    def _flight_payload(self) -> tuple[dict[str, Any], tuple[float, float], float, float]:
        cfg = self.config
        start_along = cfg.commanded_touchdown_ft - cfg.start_distance_nm * FEET_PER_NM
        start_point = self.runway.point(start_along)
        distance = cfg.commanded_touchdown_ft - start_along
        start_agl_ft = 5.0 + math.tan(math.radians(cfg.glidepath_deg)) * distance
        start_elevation_m = (self.runway.elevation_ft + start_agl_ft) / FEET_PER_METER
        start_tas_mps = cfg.approach_kias * 1.018 * KNOT_TO_MPS
        payload = {
            "data": {
                "aircraft": {"path": "Aircraft/SR20 G6 Custom FM/SR20_G6_Custom_FM.acf"},
                "lle_air_start": {
                    "latitude": start_point[0],
                    "longitude": start_point[1],
                    "elevation_in_meters": start_elevation_m,
                    "heading_true": self.runway.heading_true_deg,
                    "speed_in_meters_per_second": start_tas_mps,
                    "pitch_in_degrees": cfg.initial_pitch_deg,
                },
                "engine_status": {"all_engines": {"running": True}},
                "local_time": {"day_of_year": 234, "time_in_24_hours": 12.0},
                "weather": {
                    "definition": {
                        "latitude_in_degrees": self.runway.threshold_latitude,
                        "longitude_in_degrees": self.runway.threshold_longitude,
                        "elevation_in_meters": self.runway.elevation_ft / FEET_PER_METER,
                        "visibility_in_kilometers": 50.0,
                        "temperature_in_degrees_celsius": 15.0,
                        "altimeter_setting_in_hpa": 1013.25,
                        "precipitation_ratio": 0.0,
                        "wind": [
                            {
                                "altitude_in_feet_msl": 0,
                                "speed_in_knots": 0,
                                "direction_in_degrees_true": 0,
                                "turbulence_ratio": 0,
                            },
                            {
                                "altitude_in_feet_msl": 16000,
                                "speed_in_knots": 0,
                                "direction_in_degrees_true": 0,
                                "turbulence_ratio": 0,
                            },
                        ],
                    },
                    "vertical_speed_in_thermal_in_feet_per_minute": 0,
                    "wave_height_in_meters": 0,
                    "wave_direction_in_degrees": 0,
                    "terrain_state": "dry",
                    "variation_across_region_percentage": 0,
                    "evolution_over_time_enum": "static",
                },
            }
        }
        return payload, start_point, start_agl_ft, start_tas_mps

    def _configure_sound(self) -> None:
        if not (self.evidence.record_video or self.evidence.record_audio):
            return
        self.api.set_dataref("sim/operation/sound/sound_on", 1)
        for name in SOUND_DATAREFS:
            self.api.set_dataref(name, 1.0)
        self.sound_verification = {
            "sound_on": float(self.api.get_scalar("sim/operation/sound/sound_on")),
            **{name: float(self.api.get_scalar(name)) for name in SOUND_DATAREFS},
        }
        if self.sound_verification["sound_on"] < 0.5 or any(
            self.sound_verification[name] < 0.95 for name in SOUND_DATAREFS
        ):
            raise RuntimeError("X-Plane sound enable/volume readback failed.")

    def _controller_setup(self) -> None:
        values = {
            "sr20g6/test_controller/authority": 0.72,
            "sr20g6/test_controller/pitch_authority": 0.95,
            "sr20g6/test_controller/yaw_authority": 0.45,
            "sr20g6/test_controller/ground_target_heading_deg": self.runway.heading_true_deg,
            "sr20g6/test_controller/target_bank_deg": 0.0,
            "sr20g6/test_controller/target_pitch_deg": self.config.initial_pitch_deg,
            "sr20g6/test_controller/mode": 2,
            "sr20g6/test_controller/armed": 1,
        }
        for name, value in values.items():
            self.api.set_dataref(name, value)

    def _initial_flap_ratio(self) -> float:
        """Configuration hook for dynamic landing maneuvers."""

        return 1.0

    def _set_flap_ratio(self, ratio: float) -> None:
        if bool(int(getattr(self.config, "use_torquesim_flaps", 0))):
            detent = 0 if ratio < 0.25 else 1 if ratio < 0.75 else 2
            if detent == self._torquesim_flap_detent:
                return
            self.api.set_dataref("sim/cockpit2/controls/flap_ratio", ratio)
            self.api.set_dataref("afm/sr/switches/flaps", detent)
            self._torquesim_flap_detent = detent
        else:
            self.api.set_dataref("sim/cockpit2/controls/flap_ratio", ratio)

    def _initial_kinematics(self, start_tas_mps: float) -> tuple[float, float, float, float]:
        """Return heading, pitch, bank, and flight-path angle for the released air start."""

        return (
            self.runway.heading_true_deg,
            self.config.initial_pitch_deg,
            0.0,
            -self.config.glidepath_deg,
        )

    def _repeat_configuration(self, stations: list[float], count: int) -> None:
        for _ in range(count):
            self.api.set_dataref("sim/flightmodel/weight/m_stations", stations)
            self._set_flap_ratio(self._initial_flap_ratio())
            time.sleep(0.1)

    def _gear_layout(self) -> dict[str, Any]:
        raw = self.api.get_batch(
            (
                "sim/flightmodel/parts/tire_x_no_deflection",
                "sim/flightmodel/parts/tire_y_no_deflection",
                "sim/flightmodel/parts/tire_z_no_deflection",
            )
        )
        x = list(raw["sim/flightmodel/parts/tire_x_no_deflection"])
        y = list(raw["sim/flightmodel/parts/tire_y_no_deflection"])
        z = list(raw["sim/flightmodel/parts/tire_z_no_deflection"])
        active = [
            index
            for index in range(min(10, len(x)))
            if abs(float(x[index])) + abs(float(y[index])) + abs(float(z[index])) > 0.1
        ]
        candidates: list[tuple[float, int, int]] = []
        for left in active:
            for right in active:
                if right <= left or float(x[left]) * float(x[right]) >= 0.0:
                    continue
                score = abs(float(z[left]) - float(z[right])) + abs(
                    abs(float(x[left])) - abs(float(x[right]))
                )
                candidates.append((score, left, right))
        if not candidates:
            raise RuntimeError("Could not identify the main-gear pair from tire geometry.")
        _, first, second = min(candidates)
        nose_candidates = [index for index in active if index not in (first, second)]
        if not nose_candidates:
            raise RuntimeError("Could not identify the nose gear from tire geometry.")
        main_z = (float(z[first]) + float(z[second])) / 2.0
        nose = max(nose_candidates, key=lambda index: abs(float(z[index]) - main_z))
        return {
            "active_indices": active,
            "main_indices": [first, second],
            "nose_index": nose,
            "tire_x_m": x,
            "tire_y_m": y,
            "tire_z_m": z,
        }

    def _wait_for_air_start(
        self, start_point: tuple[float, float], start_agl_ft: float, timeout: float = 180.0
    ) -> tuple[float, float, float]:
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            try:
                # A reload can clear a pause commanded against the preceding
                # flight. Reassert and verify pause before accepting position.
                if int(self.api.get_scalar("sim/time/paused")) != 1:
                    self.api.command("sim/operation/pause_on")
                    time.sleep(0.15)
                raw = self.api.get_batch(
                    (
                        "sim/flightmodel/position/latitude",
                        "sim/flightmodel/position/longitude",
                        "sim/flightmodel/position/elevation",
                    )
                )
                latitude = float(scalar(raw["sim/flightmodel/position/latitude"]))
                longitude = float(scalar(raw["sim/flightmodel/position/longitude"]))
                elevation_ft = float(scalar(raw["sim/flightmodel/position/elevation"])) * FEET_PER_METER
                if (
                    abs(latitude - start_point[0]) < 0.01
                    and abs(longitude - start_point[1]) < 0.01
                    and abs(elevation_ft - (self.runway.elevation_ft + start_agl_ft)) < 150.0
                ):
                    local = self.api.get_batch(
                        (
                            "sim/flightmodel/position/local_x",
                            "sim/flightmodel/position/local_y",
                            "sim/flightmodel/position/local_z",
                        )
                    )
                    return (
                        float(scalar(local["sim/flightmodel/position/local_x"])),
                        float(scalar(local["sim/flightmodel/position/local_y"])),
                        float(scalar(local["sim/flightmodel/position/local_z"])),
                    )
            except Exception:
                pass
            time.sleep(1.0)
        raise RuntimeError(
            "Fresh flight did not establish the requested KCDW air-start position before setup."
        )

    def _prepare_flight(self) -> tuple[float, dict[str, Any]]:
        payload, start_point, start_agl_ft, start_tas_mps = self._flight_payload()
        try:
            self.api.load_flight(payload)
        except XPlaneConnectionError:
            # Reload can close the request connection after accepting the card.
            pass
        self._wait_for_catalog()
        # TorqueSim accepts its native-MFD bypass only during early startup.
        # Request it immediately after the reloaded catalog appears, before
        # mass, engine, and controller setup consume that window.
        self._request_torquesim_native_mfd()
        required = list(REQUIRED_DATAREFS)
        if self._requires_custom_mod():
            required.extend(CUSTOM_MOD_DATAREFS)
        if bool(int(getattr(self.config, "use_fixed_mass", 0))):
            required.append("sim/flightmodel/weight/m_fixed")
        if bool(int(getattr(self.config, "use_torquesim_mass", 0))):
            required.extend(TORQUESIM_MASS_DATAREFS)
            required.append("sim/flightmodel/weight/m_fixed")
        if bool(int(getattr(self.config, "use_torquesim_flaps", 0))):
            required.append("afm/sr/switches/flaps")
            required.append("sim/time/total_flight_time_sec")
        if self._uses_torquesim_engine():
            required.extend(TORQUESIM_ENGINE_DATAREFS)
            if self.evidence.record_video or self.evidence.prepare_video_view:
                required.append("afm/sr/avionics/g1000MFD")
        if self.evidence.record_video or self.evidence.record_audio:
            required.extend(
                (
                    "sim/graphics/view/field_of_view_deg",
                    "sim/graphics/view/pilots_head_the",
                    "sim/operation/sound/sound_on",
                    *SOUND_DATAREFS,
                )
            )
        self.api.require_datarefs(required)
        if int(self.api.get_scalar("sr20g6/test_controller/version_minor")) < 7:
            raise RuntimeError("Attitude controller v1.7 or newer is not loaded.")
        # Freeze the fresh flight before waiting on aircraft-specific state.
        # _wait_for_air_start also reasserts this if reload clears it late.
        self._set_pause(True)
        self._configure_sound()
        start_local = self._wait_for_air_start(start_point, start_agl_ft)

        self._set_pause(True)
        self.api.set_dataref("sr20g6/test_controller/armed", 0)
        self.api.set_dataref("sim/operation/override/override_planepath", PLANEPATH_OWNED)
        self.api.set_dataref("sim/operation/override/override_artstab", 0)
        self.api.set_dataref("sim/operation/override/override_toe_brakes", 0)
        self._set_flap_ratio(self._initial_flap_ratio())
        self.api.set_dataref("sim/cockpit2/engine/actuators/mixture_ratio_all", 1.0)
        self.api.set_dataref(
            "sim/cockpit2/engine/actuators/throttle_ratio_all", self.config.initial_throttle
        )

        zero_stations = [0.0] * 9
        self.api.set_dataref("sim/flightmodel/weight/m_stations", zero_stations)
        self._set_pause(False)
        self._repeat_configuration(zero_stations, 25)
        self._controller_setup()
        self.api.set_dataref("sim/operation/override/override_planepath", PLANEPATH_RELEASED)
        self._repeat_configuration(zero_stations, 10)
        self._set_pause(True)
        self.api.set_dataref("sim/operation/override/override_planepath", PLANEPATH_OWNED)

        if bool(int(getattr(self.config, "use_torquesim_mass", 0))):
            # TorqueSim continuously derives X-Plane station weights from its
            # own payload model. Set a plausible four-occupant profile through
            # the native datarefs, then use baggage for the small exact-mass
            # correction. Each update is allowed one flight-loop interval and
            # is verified against the simulator's achieved total mass.
            # Harness adapter hook: dataref presence can precede vendor physics
            # activation by tens of seconds on a fresh simulator process.
            if hasattr(self, "_before_native_mass_setup"):
                self._before_native_mass_setup()
            mass_sync_offset = float(
                self.api.get_scalar("sim/flightmodel/weight/m_total")
            ) - float(self.api.get_scalar("afm/sr/mass/total_kg"))
            if abs(mass_sync_offset) > 5.0:
                self._set_pause(False)
                time.sleep(4.0)
                self._set_pause(True)
                mass_sync_offset = float(
                    self.api.get_scalar("sim/flightmodel/weight/m_total")
                ) - float(self.api.get_scalar("afm/sr/mass/total_kg"))
            if abs(mass_sync_offset) > 5.0:
                raise RuntimeError(
                    f"TorqueSim and X-Plane mass totals did not synchronize: {mass_sync_offset} kg"
                )

            def settle_torquesim_mass() -> None:
                # m_total can remain stale while paused, but TorqueSim's native
                # payload and X-Plane's m_fixed physics term update together.
                # Prove those authoritative components here; the sustained
                # airborne entry gate later verifies the full public total.
                for _ in range(3):
                    self._set_pause(False)
                    time.sleep(1.0)
                    self._set_pause(True)
                    time.sleep(0.2)
                    xplane_fixed = float(
                        self.api.get_scalar("sim/flightmodel/weight/m_fixed")
                    )
                    torquesim_payload = float(
                        self.api.get_scalar("afm/sr/mass/payload_kg")
                    )
                    if abs(xplane_fixed - torquesim_payload) <= 0.75:
                        return
                raise RuntimeError(
                    "TorqueSim payload did not propagate to X-Plane's fixed-mass term"
                )

            native_weights = {
                "afm/sr/mass/pilot_kg": 90.718474,
                "afm/sr/mass/copilot_kg": 90.718474,
                "afm/sr/mass/paxL_kg": 0.0,
                "afm/sr/mass/paxR_kg": 0.0,
                "afm/sr/mass/baggage_kg": 0.0,
            }
            for name, value in native_weights.items():
                self.api.set_dataref(name, value)
            settle_torquesim_mass()
            full_fuel_kg = float(self.api.get_scalar("sim/aircraft/weight/acf_m_fuel_tot"))
            self.api.set_dataref("afm/sr/fuel/massL_kg", full_fuel_kg / 2)
            self.api.set_dataref("afm/sr/fuel/massR_kg", full_fuel_kg / 2)
            settle_torquesim_mass()
            if hasattr(self, "_after_native_fuel_setup"):
                self._after_native_fuel_setup()
            self.config.target_mass_kg = float(self.api.get_scalar("afm/sr/mass/total_kg")) + mass_sync_offset
            self.mass_setup = {
                "mode": "TorqueSim native payload datarefs",
                "pilot_kg": float(self.api.get_scalar("afm/sr/mass/pilot_kg")),
                "copilot_kg": float(self.api.get_scalar("afm/sr/mass/copilot_kg")),
                "left_rear_kg": float(self.api.get_scalar("afm/sr/mass/paxL_kg")),
                "right_rear_kg": float(self.api.get_scalar("afm/sr/mass/paxR_kg")),
                "baggage_kg": float(self.api.get_scalar("afm/sr/mass/baggage_kg")),
                "torquesim_total_kg": float(self.api.get_scalar("afm/sr/mass/total_kg")),
                "torquesim_cg_in": float(self.api.get_scalar("afm/sr/mass/cg_in")),
                "xplane_minus_torquesim_mass_kg": mass_sync_offset,
            }

        if not bool(int(getattr(self.config, "use_torquesim_mass", 0))):
            base_mass = float(self.api.get_scalar("sim/flightmodel/weight/m_total"))
            payload_kg = self.config.target_mass_kg - base_mass
            station_max = list(self.api.get_raw("sim/aircraft/weight/acf_m_station_max"))
            stations = [0.0] * 9
            stations[0] = min(payload_kg, float(station_max[0]))
            remaining = payload_kg - stations[0]
            stations[1] = min(remaining, float(station_max[1]))
            stations[2] = remaining - stations[1]
            self.api.set_dataref("sim/flightmodel/weight/m_stations", stations)
            self._set_pause(False)
            self._repeat_configuration(stations, 25)
            self._controller_setup()
            self.api.set_dataref("sim/operation/override/override_planepath", PLANEPATH_RELEASED)
            self._repeat_configuration(stations, 60)
            self._set_pause(True)
            self.api.set_dataref("sim/operation/override/override_planepath", PLANEPATH_OWNED)

        if bool(int(getattr(self.config, "use_fixed_mass", 0))):
            # Some aircraft plugins continuously own m_stations. For a
            # controlled comparison, trim the simulator's writable fixed-mass
            # term instead and verify the achieved total mass. This is a
            # runtime-only test setup and does not edit the source ACF.
            for _ in range(6):
                actual_mass = float(
                    self.api.get_scalar("sim/flightmodel/weight/m_total")
                )
                if abs(actual_mass - self.config.target_mass_kg) <= 0.5:
                    break
                fixed_mass = float(
                    self.api.get_scalar("sim/flightmodel/weight/m_fixed")
                )
                self.api.set_dataref(
                    "sim/flightmodel/weight/m_fixed",
                    fixed_mass + self.config.target_mass_kg - actual_mass,
                )
                time.sleep(0.5)

        loaded_mass = (
            float(self.api.get_scalar("afm/sr/mass/total_kg")) + mass_sync_offset
            if bool(int(getattr(self.config, "use_torquesim_mass", 0)))
            else float(self.api.get_scalar("sim/flightmodel/weight/m_total"))
        )
        if abs(loaded_mass - self.config.target_mass_kg) > 5.0:
            raise RuntimeError(f"Mass load failed: {loaded_mass} kg")
        if not self.mass_setup:
            self.mass_setup = {"mode": "X-Plane station weights"}
        if bool(int(getattr(self.config, "use_torquesim_flaps", 0))):
            self._torquesim_flap_detent = None
            self._set_flap_ratio(self._initial_flap_ratio())
            self.api.set_dataref(
                "sim/operation/override/override_planepath", PLANEPATH_RELEASED
            )
            self._set_pause(False)
            start_sim_time = float(
                self.api.get_scalar("sim/time/total_flight_time_sec")
            )
            flap_ready = False
            wall_deadline = time.monotonic() + 20.0
            while time.monotonic() < wall_deadline:
                actual_flap = float(
                    self.api.get_scalar(
                        "sim/flightmodel2/controls/flap_handle_deploy_ratio"
                    )
                )
                if abs(actual_flap - self._initial_flap_ratio()) <= 0.05:
                    flap_ready = True
                    break
                elapsed_sim_time = float(
                    self.api.get_scalar("sim/time/total_flight_time_sec")
                ) - start_sim_time
                if elapsed_sim_time > 6.0:
                    break
                time.sleep(0.1)
            self._set_pause(True)
            time.sleep(0.2)
            self.api.set_dataref(
                "sim/operation/override/override_planepath", PLANEPATH_OWNED
            )
            if not flap_ready:
                raise RuntimeError(
                    "TorqueSim flaps did not reach the requested initial detent within six simulator seconds"
                )
        actual_flap = float(
            self.api.get_scalar("sim/flightmodel2/controls/flap_handle_deploy_ratio")
        )
        if abs(actual_flap - self._initial_flap_ratio()) > 0.08:
            raise RuntimeError(
                "Initial flap configuration did not deploy: "
                f"command={self._initial_flap_ratio():.2f}, actual={actual_flap:.2f}"
            )

        heading_deg, pitch_deg, bank_deg, flight_path_deg = self._initial_kinematics(
            start_tas_mps
        )
        flight_path = math.radians(flight_path_deg)
        heading = math.radians(heading_deg)
        for name, value in zip(
            (
                "sim/flightmodel/position/local_x",
                "sim/flightmodel/position/local_y",
                "sim/flightmodel/position/local_z",
            ),
            start_local,
        ):
            self.api.set_dataref(name, value)
        self.api.set_dataref(
            "sim/flightmodel/position/q",
            quaternion(heading_deg, pitch_deg, bank_deg),
        )
        self.api.set_dataref(
            "sim/flightmodel/position/local_vx",
            start_tas_mps * math.cos(flight_path) * math.sin(heading),
        )
        self.api.set_dataref(
            "sim/flightmodel/position/local_vy", start_tas_mps * math.sin(flight_path)
        )
        self.api.set_dataref(
            "sim/flightmodel/position/local_vz",
            -start_tas_mps * math.cos(flight_path) * math.cos(heading),
        )
        for name in (
            "sim/flightmodel/position/P",
            "sim/flightmodel/position/Q",
            "sim/flightmodel/position/R",
        ):
            self.api.set_dataref(name, 0.0)
        if self._uses_torquesim_engine():
            # The TorqueSim engine and pitot/static model need live simulator
            # frames after an API air start. Hold the flight path fixed while
            # those systems settle, and fail closed if the aircraft-local
            # engine is merely windmilling because a required dependency was
            # omitted from the clean profile.
            self._set_pause(False)
            start_sim_time = float(
                self.api.get_scalar("sim/time/total_flight_time_sec")
            )
            wall_deadline = time.monotonic() + 15.0
            engine_ready = False
            instrument_settle_complete = False
            engine_power_w = 0.0
            fuel_flow_kg_s = 0.0
            rpm = 0.0
            ias_kias = 0.0
            elapsed_sim_time = 0.0
            while time.monotonic() < wall_deadline:
                engine_power_w = float(
                    list(self.api.get_raw("sim/flightmodel/engine/ENGN_power"))[0]
                )
                fuel_flow_kg_s = float(
                    list(self.api.get_raw("sim/flightmodel/engine/ENGN_FF_"))[0]
                )
                rpm = float(
                    list(
                        self.api.get_raw(
                            "sim/cockpit2/engine/indicators/engine_speed_rpm"
                        )
                    )[0]
                )
                ias_kias = float(
                    self.api.get_scalar("sim/flightmodel/position/indicated_airspeed")
                )
                elapsed_sim_time = float(
                    self.api.get_scalar("sim/time/total_flight_time_sec")
                ) - start_sim_time
                engine_ready = (
                    engine_power_w > 10000.0
                    and fuel_flow_kg_s > 0.0001
                    and rpm > 800.0
                )
                instrument_settle_complete = elapsed_sim_time >= 7.0
                if instrument_settle_complete and engine_ready:
                    break
                time.sleep(0.1)
            self._set_pause(True)
            self.engine_setup = {
                "mode": "TorqueSim live-frame air-start settle",
                "settle_sim_seconds": elapsed_sim_time,
                "engine_power_w": engine_power_w,
                "fuel_flow_kg_s": fuel_flow_kg_s,
                "rpm": rpm,
                "indicated_airspeed_kias": ias_kias,
                "target_airspeed_kias": self._initial_ias_target(),
            }
            if not engine_ready:
                raise RuntimeError(
                    "TorqueSim engine did not produce positive power and fuel flow; "
                    "verify the X-Aviation and Gizmo clean-profile dependencies"
                )
        self._controller_setup()
        gear_layout = self._gear_layout()
        self.api.set_dataref("sim/operation/override/override_planepath", PLANEPATH_RELEASED)
        return loaded_mass, gear_layout

    def _start_media(self) -> None:
        evidence = self.evidence
        if evidence.record_audio:
            paths = (
                evidence.audio_capture_script,
                evidence.audio_module_dir,
                evidence.audio_output_path,
                evidence.audio_stop_file,
                evidence.audio_ready_file,
                evidence.audio_metadata_path,
            )
            if any(path is None for path in paths):
                raise ValueError("All audio-capture paths are required with --record-audio.")
            script, module_dir, output, stop, ready, metadata = paths
            assert script and module_dir and output and stop and ready and metadata
            if not script.is_file():
                raise FileNotFoundError(f"Audio capture script not found: {script}")
            if not module_dir.is_dir():
                raise FileNotFoundError(f"Audio module directory not found: {module_dir}")
            for path in (output, stop, ready, metadata):
                if path.exists():
                    raise FileExistsError(f"Refusing to reuse evidence path: {path}")
            self.audio_process = subprocess.Popen(
                [
                    sys.executable,
                    str(script),
                    "--module-dir",
                    str(module_dir),
                    "--output",
                    str(output),
                    "--stop-file",
                    str(stop),
                    "--ready-file",
                    str(ready),
                    "--metadata",
                    str(metadata),
                    "--sample-rate",
                    str(evidence.audio_sample_rate),
                ],
                creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0),
            )
            # From this point cleanup owns the process even if readiness fails.
            self.audio_recording_started = True
            deadline = time.monotonic() + 10.0
            while time.monotonic() < deadline and not ready.exists():
                if self.audio_process.poll() is not None:
                    raise RuntimeError(
                        "Audio capture process exited before ready with code "
                        f"{self.audio_process.returncode}."
                    )
                time.sleep(0.1)
            if not ready.exists():
                raise RuntimeError("Audio capture process did not become ready within 10 seconds.")
            self.audio_ready = json.loads(ready.read_text(encoding="utf-8"))

        if evidence.record_video or evidence.prepare_video_view:
            if (
                evidence.record_video
                and "sim/operation/video_record_toggle" not in self.api.commands
            ):
                raise RuntimeError("X-Plane video recording command is unavailable.")
            # TorqueSim inserts a database page and two limitation pages before
            # handing the MFD to the native G1000. Advance those pages while the
            # flight is still paused; sending the same keys after an already-
            # initialized reload is harmless because the rightmost map softkey is
            # blank. A subsequent long CLR press deterministically returns to NAV
            # MAP before recording begins.
            if self._uses_torquesim_engine():
                # Native G1000 boot timers advance only while the simulator is
                # running.  Hold the airplane's path fixed while giving the
                # avionics a short live-frame interval, then return to the
                # paused evidence gate without changing the initialized state.
                self.api.set_dataref(
                    "sim/operation/override/override_planepath", PLANEPATH_OWNED
                )
                self._set_pause(False)
                # Both native G1000 units need live frames to complete their
                # power-on self tests.  Their ENT prompts do not accept input
                # during the early splash, so wait for the full boot interval,
                # return to pause, and only then acknowledge each possible
                # native unit.  Planepath is owned throughout, so this cannot
                # advance the maneuver.
                time.sleep(75.0)
                self._set_pause(True)
                for command in (
                    "sim/GPS/g1000n1_ent",
                    "sim/GPS/g1000n2_ent",
                    "sim/GPS/g1000n3_ent",
                ):
                    if command in self.api.commands:
                        self.api.command(command, 0.2)
                        time.sleep(0.5)
                time.sleep(3.0)
                self.api.set_dataref(
                    "sim/operation/override/override_planepath", PLANEPATH_RELEASED
                )
                # The TorqueSim splash/limitation stack is populated
                # asynchronously even while the flight is paused.  Let it
                # finish before advancing pages or a late page can replace
                # the NAV MAP after this setup sequence has completed.
                time.sleep(8.0)
                startup_commands = (
                    "sim/GPS/g1000n3_ent",
                    "sim/GPS/g1000n3_softkey12",
                    "sim/GPS/g1000n3_softkey12",
                )
                for command in startup_commands:
                    if command in self.api.commands:
                        self.api.command(command, 0.2)
                        time.sleep(2.0)
                time.sleep(4.0)
            if "sim/GPS/g1000n3_clr" in self.api.commands:
                # A long CLR is the native G1000 shortcut back to NAV MAP.
                # Repeat it to cover TorqueSim's delayed hand-off to the
                # native MFD without depending on its current page depth.
                for _ in range(2):
                    self.api.command("sim/GPS/g1000n3_clr", 1.5)
                    time.sleep(2.0)
            if self._uses_torquesim_engine():
                # Mode 3 is TorqueSim's normal/native-MFD state.  ENGINE opens
                # its mode-4 fuel/EIS stack; accept the fuel page with softkey
                # 11, then ENGINE toggles the resulting EIS back to NAV MAP.
                # (Pressing ENGINE before entering mode 4 merely opens EIS.)
                mode = int(self.api.get_scalar("afm/sr/avionics/g1000MFD"))
                if mode == 3:
                    if "sim/GPS/g1000n3_softkey1" not in self.api.commands:
                        raise RuntimeError("TorqueSim ENGINE softkey command is unavailable.")
                    self.api.command("sim/GPS/g1000n3_softkey1", 0.2)
                    time.sleep(3.0)
                eis_deadline = time.monotonic() + 15.0
                while time.monotonic() < eis_deadline:
                    if int(self.api.get_scalar("afm/sr/avionics/g1000MFD")) == 4:
                        required_commands = (
                            "sim/GPS/g1000n3_softkey11",
                            "sim/GPS/g1000n3_softkey1",
                        )
                        missing_commands = [
                            command
                            for command in required_commands
                            if command not in self.api.commands
                        ]
                        if missing_commands:
                            raise RuntimeError(
                                "TorqueSim MFD setup commands are unavailable: "
                                + ", ".join(missing_commands)
                            )
                        self.api.command("sim/GPS/g1000n3_softkey11", 0.2)
                        time.sleep(3.0)
                        self.api.command("sim/GPS/g1000n3_softkey1", 0.2)
                        time.sleep(3.0)
                        break
                    time.sleep(0.5)
            self.api.command("sim/view/3d_cockpit_cmnd_look", 0.2)
            time.sleep(1.0)
            self.api.set_dataref("sim/graphics/view/field_of_view_deg", evidence.video_fov_deg)
            self.api.set_dataref("sim/graphics/view/pilots_head_the", evidence.video_head_pitch_deg)
            time.sleep(1.0)
            if evidence.record_video:
                self.video_start_utc = utc_now()
                self.api.command("sim/operation/video_record_toggle", 0.2)
                self.video_recording_started = True
                time.sleep(2.0)

    def _stop_media(self) -> None:
        if self.video_recording_started:
            try:
                self.api.command("sim/operation/video_record_toggle", 0.2)
                self.video_stop_utc = utc_now()
                time.sleep(1.0)
            except Exception as error:
                print(f"warning: could not stop X-Plane video recording: {error}", file=sys.stderr)
            self.video_recording_started = False
        if self.audio_recording_started and self.audio_process is not None:
            try:
                assert self.evidence.audio_stop_file is not None
                self.evidence.audio_stop_file.write_text("stop", encoding="utf-8")
                try:
                    return_code = self.audio_process.wait(timeout=15.0)
                except subprocess.TimeoutExpired:
                    self.audio_process.kill()
                    self.audio_capture_failure = (
                        "Audio capture process did not stop within 15 seconds."
                    )
                else:
                    if return_code != 0:
                        self.audio_capture_failure = (
                            f"Audio capture process exited with code {return_code}."
                        )
            except Exception as error:
                self.audio_capture_failure = f"Could not stop audio capture cleanly: {error}"
            self.audio_recording_started = False

    def _control_sample(self) -> dict[str, Any]:
        control_datarefs = list(CONTROL_DATAREFS)
        if self._uses_torquesim_engine():
            control_datarefs.extend(TORQUESIM_ENGINE_DATAREFS[1:])
        raw = self.api.get_batch(control_datarefs)
        latitude = float(scalar(raw["sim/flightmodel/position/latitude"]))
        longitude = float(scalar(raw["sim/flightmodel/position/longitude"]))
        along, cross = self.runway.position(latitude, longitude)
        result = {
            "latitude": latitude,
            "longitude": longitude,
            "elevation_msl_ft": float(scalar(raw["sim/flightmodel/position/elevation"]))
            * FEET_PER_METER,
            "agl_ft": float(scalar(raw["sim/flightmodel/position/y_agl"])) * FEET_PER_METER,
            "runway_along_ft": along,
            "runway_cross_ft": cross,
            "ias_kias": float(scalar(raw["sim/flightmodel/position/indicated_airspeed"])),
            "groundspeed_kt": float(scalar(raw["sim/flightmodel/position/groundspeed"]))
            / KNOT_TO_MPS,
            "vvi_fpm": float(scalar(raw["sim/flightmodel/position/vh_ind_fpm"])),
            "heading_true_deg": float(scalar(raw["sim/flightmodel/position/psi"])),
            "bank_deg": float(scalar(raw["sim/flightmodel/position/phi"])),
            "pitch_deg": float(scalar(raw["sim/flightmodel/position/theta"])),
            "beta_deg": float(scalar(raw["sim/flightmodel/position/beta"])),
            "pitch_rate_deg_s": float(scalar(raw["sim/flightmodel/position/Q"])),
            "normal_g": float(scalar(raw["sim/flightmodel/forces/g_nrml"])),
            "throttle_ratio": float(
                scalar(raw["sim/cockpit2/engine/actuators/throttle_ratio_all"])
            ),
            "flap_handle_ratio": float(
                scalar(raw["sim/cockpit2/controls/flap_ratio"])
            ),
            "elevator_trim_ratio": float(
                scalar(raw["sim/cockpit2/controls/elevator_trim"])
            ),
            "flap_actual_ratio": float(
                scalar(raw["sim/flightmodel2/controls/flap_handle_deploy_ratio"])
            ),
            "on_ground": list(raw["sim/flightmodel2/gear/on_ground"]),
            "tire_skid_ratio": list(raw["sim/flightmodel2/gear/tire_skid_ratio"]),
            "mass_kg": float(scalar(raw["sim/flightmodel/weight/m_total"])),
        }
        if self._uses_torquesim_engine():
            result.update(
                {
                    "engine_power_w": float(raw["sim/flightmodel/engine/ENGN_power"][0]),
                    "fuel_flow_kg_s": float(raw["sim/flightmodel/engine/ENGN_FF_"][0]),
                    "engine_rpm": float(
                        raw["sim/cockpit2/engine/indicators/engine_speed_rpm"][0]
                    ),
                }
            )
        return result

    def _sample(self, elapsed: float, stage: str) -> dict[str, Any]:
        sample_datarefs = list(FULL_SAMPLE_DATAREFS)
        if self._requires_custom_mod():
            sample_datarefs.extend(CUSTOM_MOD_DATAREFS)
        if self._uses_torquesim_engine():
            sample_datarefs.extend(TORQUESIM_ENGINE_DATAREFS[1:])
        raw = self.api.get_batch(sample_datarefs)

        def value(name: str) -> Any:
            return scalar(raw[name])

        latitude = float(value("sim/flightmodel/position/latitude"))
        longitude = float(value("sim/flightmodel/position/longitude"))
        along, cross = self.runway.position(latitude, longitude)
        heading = float(value("sim/flightmodel/position/psi"))
        result = {
            "elapsed_s": round(elapsed, 3),
            "stage": stage,
            "planepath_override_0": int(
                value("sim/operation/override/override_planepath")
            ),
            "latitude": latitude,
            "longitude": longitude,
            "elevation_msl_ft": float(value("sim/flightmodel/position/elevation"))
            * FEET_PER_METER,
            "agl_ft": float(value("sim/flightmodel/position/y_agl")) * FEET_PER_METER,
            "runway_along_ft": along,
            "runway_cross_ft": cross,
            "ias_kias": float(value("sim/flightmodel/position/indicated_airspeed")),
            "groundspeed_kt": float(value("sim/flightmodel/position/groundspeed")) / KNOT_TO_MPS,
            "vvi_fpm": float(value("sim/flightmodel/position/vh_ind_fpm")),
            "heading_true_deg": heading,
            "runway_heading_error_deg": wrap_180(heading - self.runway.heading_true_deg),
            "bank_deg": float(value("sim/flightmodel/position/phi")),
            "pitch_deg": float(value("sim/flightmodel/position/theta")),
            "beta_deg": float(value("sim/flightmodel/position/beta")),
            "normal_g": float(value("sim/flightmodel/forces/g_nrml")),
            "throttle_ratio": float(
                value("sim/cockpit2/engine/actuators/throttle_ratio_all")
            ),
            "flap_handle_ratio": float(value("sim/cockpit2/controls/flap_ratio")),
            "elevator_trim_ratio": float(
                value("sim/cockpit2/controls/elevator_trim")
            ),
            "flap_actual_ratio": float(
                value("sim/flightmodel2/controls/flap_handle_deploy_ratio")
            ),
            "left_brake_ratio": float(value("sim/cockpit2/controls/left_brake_ratio")),
            "right_brake_ratio": float(value("sim/cockpit2/controls/right_brake_ratio")),
            "brake_applied_ratio": float(
                value("sim/cockpit2/controls/wheel_brake_ratio_applied")
            ),
            "on_ground": list(raw["sim/flightmodel2/gear/on_ground"]),
            "tire_skid_ratio": list(raw["sim/flightmodel2/gear/tire_skid_ratio"]),
            "controller_active": int(value("sr20g6/test_controller/active")),
            "target_bank_deg": float(value("sr20g6/test_controller/target_bank_deg")),
            "target_pitch_deg": float(value("sr20g6/test_controller/target_pitch_deg")),
            "roll_command": float(value("sr20g6/test_controller/command_ratio")),
            "pitch_command": float(value("sr20g6/test_controller/pitch_command_ratio")),
            "yaw_command": float(value("sr20g6/test_controller/yaw_command_ratio")),
            "controller_loop_hz": float(value("sr20g6/test_controller/loop_hz")),
            "first_contact_latched": int(
                value("sr20g6/test_controller/first_contact_latched")
            ),
            "first_contact_mask": int(value("sr20g6/test_controller/first_contact_mask")),
            "first_contact_pitch_deg": float(
                value("sr20g6/test_controller/first_contact_pitch_deg")
            ),
            "first_contact_vvi_fpm": float(
                value("sr20g6/test_controller/first_contact_vvi_fpm")
            ),
            "first_contact_bank_deg": float(
                value("sr20g6/test_controller/first_contact_bank_deg")
            ),
            "first_contact_heading_deg": float(
                value("sr20g6/test_controller/first_contact_heading_deg")
            ),
            "first_contact_beta_deg": float(
                value("sr20g6/test_controller/first_contact_beta_deg")
            ),
            "first_contact_ias_kias": float(
                value("sr20g6/test_controller/first_contact_ias_kias")
            ),
            "first_contact_throttle_ratio": float(
                value("sr20g6/test_controller/first_contact_throttle_ratio")
            ),
            "first_contact_latitude": float(
                value("sr20g6/test_controller/first_contact_latitude")
            ),
            "first_contact_longitude": float(
                value("sr20g6/test_controller/first_contact_longitude")
            ),
            "first_contact_elevation_m": float(
                value("sr20g6/test_controller/first_contact_elevation_m")
            ),
            "custom_fm_active": int(value("sr20g6/custom_fm/active")) if self._requires_custom_mod() else 0,
            "custom_fm_enabled": int(value("sr20g6/custom_fm/enabled")) if self._requires_custom_mod() else 0,
            "custom_fm_aircraft_match": int(value("sr20g6/custom_fm/aircraft_match")) if self._requires_custom_mod() else 0,
            "custom_fm_version_minor": int(value("sr20g6/custom_fm/version_minor")) if self._requires_custom_mod() else 0,
            "io390_active": int(value("sr20g6/io390/active")) if self._requires_custom_mod() else 0,
            "io390_enabled": int(value("sr20g6/io390/enabled")) if self._requires_custom_mod() else 0,
            "io390_aircraft_match": int(value("sr20g6/io390/aircraft_match")) if self._requires_custom_mod() else 0,
            "io390_version_minor": int(value("sr20g6/io390/version_minor")) if self._requires_custom_mod() else 0,
            "mass_kg": float(value("sim/flightmodel/weight/m_total")),
        }
        if self._uses_torquesim_engine():
            result.update(
                {
                    "engine_power_w": float(raw["sim/flightmodel/engine/ENGN_power"][0]),
                    "fuel_flow_kg_s": float(raw["sim/flightmodel/engine/ENGN_FF_"][0]),
                    "engine_rpm": float(
                        raw["sim/cockpit2/engine/indicators/engine_speed_rpm"][0]
                    ),
                }
            )
        return result

    def _threshold_row(self, control: dict[str, Any], elapsed: float, stage: str) -> dict[str, Any]:
        return {
            "elapsed_s": round(elapsed, 3),
            "stage": stage,
            "latitude": float(control["latitude"]),
            "longitude": float(control["longitude"]),
            "elevation_msl_ft": float(control["elevation_msl_ft"]),
            "agl_ft": float(control["agl_ft"]),
            "runway_along_ft": float(control["runway_along_ft"]),
            "runway_cross_ft": float(control["runway_cross_ft"]),
            "ias_kias": float(control["ias_kias"]),
            "groundspeed_kt": float(control["groundspeed_kt"]),
            "vvi_fpm": float(control["vvi_fpm"]),
            "heading_true_deg": float(control["heading_true_deg"]),
            "runway_heading_error_deg": wrap_180(
                float(control["heading_true_deg"]) - self.runway.heading_true_deg
            ),
            "pitch_deg": float(control["pitch_deg"]),
            "throttle_ratio": float(control["throttle_ratio"]),
        }

    def _touchdown_summary(
        self, touchdown: dict[str, Any] | None, last_airborne: dict[str, Any] | None
    ) -> dict[str, Any] | None:
        if touchdown is None:
            return None
        contact_mask = int(touchdown["first_contact_mask"])
        latched = int(touchdown["first_contact_latched"]) == 1
        main_contact_count = sum(1 for mask in (2, 4) if contact_mask & mask)
        nose_contact = bool(contact_mask & 1)
        contact_vvi = (
            float(touchdown["first_contact_vvi_fpm"])
            if latched
            else (float(last_airborne["vvi_fpm"]) if last_airborne else None)
        )
        if latched:
            along, cross = self.runway.position(
                float(touchdown["first_contact_latitude"]),
                float(touchdown["first_contact_longitude"]),
            )
            heading_error = wrap_180(
                float(touchdown["first_contact_heading_deg"]) - self.runway.heading_true_deg
            )
        else:
            along = float(touchdown["runway_along_ft"])
            cross = float(touchdown["runway_cross_ft"])
            heading_error = float(touchdown["runway_heading_error_deg"])
        return {
            "runway_along_ft": along,
            "error_from_1000_ft_markers_ft": along - 1000.0,
            "runway_cross_ft": cross,
            "ias_kias": float(
                touchdown["first_contact_ias_kias"] if latched else touchdown["ias_kias"]
            ),
            "groundspeed_kt": float(touchdown["groundspeed_kt"]),
            "heading_error_deg": heading_error,
            "bank_deg": float(
                touchdown["first_contact_bank_deg"] if latched else touchdown["bank_deg"]
            ),
            "pitch_deg": float(
                touchdown["first_contact_pitch_deg"] if latched else touchdown["pitch_deg"]
            ),
            "beta_deg": float(
                touchdown["first_contact_beta_deg"] if latched else touchdown["beta_deg"]
            ),
            "throttle_ratio": float(
                touchdown["first_contact_throttle_ratio"]
                if latched
                else touchdown["throttle_ratio"]
            ),
            "main_contact_count": main_contact_count,
            "nose_contact": nose_contact,
            "first_contact_latched": latched,
            "first_contact_mask": contact_mask,
            "first_contact_pitch_deg": (
                float(touchdown["first_contact_pitch_deg"]) if latched else None
            ),
            "last_airborne_vvi_fpm": contact_vvi,
        }

    def _build_result(
        self,
        *,
        loaded_mass_kg: float,
        gear_layout: dict[str, Any],
        trace: list[dict[str, Any]],
        control_trace: list[dict[str, Any]],
        touchdown: dict[str, Any] | None,
        last_airborne: dict[str, Any] | None,
        threshold_crossing: dict[str, Any] | None,
        stopped: bool,
        guidance: LandingGuidance,
    ) -> dict[str, Any]:
        cfg = self.config
        final = [
            row
            for row in trace
            if row["stage"] == "approach"
            and -FEET_PER_NM <= float(row["runway_along_ft"]) <= 0.0
            and float(row["agl_ft"]) >= 50.0
        ]
        touchdown_summary = self._touchdown_summary(touchdown, last_airborne)
        dynamics = flare_dynamics(control_trace)
        rollout_rows = [row for row in trace if row["stage"] == "rollout"]
        audited: list[dict[str, Any]] = []
        if touchdown_summary:
            for index in range(2, len(rollout_rows)):
                candidate = rollout_rows[index]
                if (
                    float(candidate["runway_along_ft"])
                    < float(touchdown_summary["runway_along_ft"]) + 100.0
                    or float(candidate["groundspeed_kt"]) < 10.0
                ):
                    continue
                continuous = all(
                    int(rollout_rows[history]["on_ground"][main]) != 0
                    for history in range(index - 2, index + 1)
                    for main in gear_layout["main_indices"]
                )
                if continuous:
                    audited.append(candidate)
        max_skid = max(
            (
                max(float(row["tire_skid_ratio"][main]) for main in gear_layout["main_indices"])
                for row in audited
            ),
            default=None,
        )
        directional = [row for row in rollout_rows if float(row["groundspeed_kt"]) >= 15.0]
        max_cross = max(
            (abs(float(row["runway_cross_ft"])) for row in directional), default=None
        )
        max_heading = max(
            (abs(float(row["runway_heading_error_deg"])) for row in directional), default=None
        )
        stop_row = trace[-1] if trace else None
        ground_roll = (
            float(stop_row["runway_along_ft"]) - float(touchdown_summary["runway_along_ft"])
            if stop_row and touchdown_summary
            else None
        )
        first = trace[0] if trace else None
        result: dict[str, Any] = {
            "schema_version": 4,
            "generated_utc": utc_now().isoformat(),
            "accepted": False,
            "rejection_reasons": [],
            "orchestration": {
                "implementation": "flight_test.short_field",
                "language": "Python",
                "python_version": sys.version.split()[0],
                "web_api_parallel_workers": 8,
                "controller_boundary": "C++ plugin inner loop; Python Web API outer loop",
            },
            "standard": {
                "source": "FAA-S-ACS-7B CA.IV.F and SR20 G6 POH P/N 11934-005 Original Issue",
                "nominated_touchdown_point_ft": 1000.0,
                "acceptable_touchdown_window_ft": [1000.0, 1100.0],
                "approach_speed_kias": cfg.approach_kias,
                "approach_speed_tolerance_kias": 5.0,
                "maximum_touchdown_kias": cfg.maximum_touchdown_kias,
                "minimum_touchdown_pitch_deg": cfg.minimum_touchdown_pitch_deg,
                "maximum_touchdown_sink_fpm": cfg.maximum_touchdown_sink_fpm,
                "maximum_target_pitch_rate_deg_s": cfg.maximum_target_pitch_rate_deg_s,
                "maximum_actual_pitch_rate_deg_s": cfg.maximum_actual_pitch_rate_deg_s,
                "minimum_idle_lead_ft": 600.0,
                "maximum_ground_roll_ft": cfg.maximum_ground_roll_ft,
            },
            "configuration": {
                "airport": self.runway.airport,
                "runway": self.runway.runway,
                "runway_heading_true_deg": self.runway.heading_true_deg,
                "runway_length_ft": self.runway.length_ft,
                "runway22_displaced_threshold_ft": self.runway.displaced_threshold_ft,
                "runway22_physical_end_latitude": self.runway.physical_end_latitude,
                "runway22_physical_end_longitude": self.runway.physical_end_longitude,
                "runway_elevation_ft": self.runway.elevation_ft,
                "weather": "ISA sea-level reference, zero wind, dry runway",
                "mass_kg": loaded_mass_kg,
                "mass_lb": loaded_mass_kg * 2.20462262185,
                "flap_ratio": 1.0,
                "controller": "v1.8 ArduPilot-derived attitude controller with capture-tolerant timing, first-contact latch, and frame-rate ground heading hold; Python Web API outer loop with instrumented pre-touchdown idle energy management",
                "custom_fm_version_minor": (
                    int(first["custom_fm_version_minor"]) if first else None
                ),
                "io390_version_minor": int(first["io390_version_minor"]) if first else None,
            },
            "command": {
                "commanded_touchdown_ft": cfg.commanded_touchdown_ft,
                "flare_touchdown_target_ft": cfg.flare_touchdown_target_ft,
                "start_distance_nm": cfg.start_distance_nm,
                "glidepath_deg": cfg.glidepath_deg,
                "idle_gate_runway_along_ft": cfg.idle_gate_along_ft,
                "flare_lead_ft": cfg.flare_lead_ft,
                "flare_capture_height_agl_ft": cfg.flare_capture_height_agl_ft,
                "flare_vertical_profile": "runway-referenced glide-to-contact VVI schedule; Kp 0.008 deg/fpm; pitch relaxation limited to 1.25 deg/s",
                "touchdown_pitch_target_deg": cfg.touchdown_pitch_deg,
                "target_pitch_rate_limit_deg_s": cfg.maximum_target_pitch_rate_deg_s,
                "initial_throttle": cfg.initial_throttle,
                "initial_pitch_deg": cfg.initial_pitch_deg,
            },
            "evidence_capture": {
                "video_requested": self.evidence.record_video,
                "video_start_utc": self.video_start_utc.isoformat()
                if self.video_start_utc
                else None,
                "video_stop_utc": self.video_stop_utc.isoformat() if self.video_stop_utc else None,
                "audio_requested": self.evidence.record_audio,
                "audio_ready": self.audio_ready,
                "audio_output_path": str(self.evidence.audio_output_path or ""),
                "audio_metadata_path": str(self.evidence.audio_metadata_path or ""),
                "audio_capture_failure": self.audio_capture_failure,
                "audio_files_verified": None,
                "sound": self.sound_verification,
                "video_view": {
                    "field_of_view_deg": self.evidence.video_fov_deg,
                    "head_pitch_deg": self.evidence.video_head_pitch_deg,
                },
            },
            "gear_layout": gear_layout,
            "threshold_crossing": threshold_crossing,
            "approach": {
                "sample_count": len(final),
                "ias_kias": metric_range(final, "ias_kias"),
                "cross_track_ft": metric_range(final, "runway_cross_ft"),
                "maximum_absolute_bank_deg": max(
                    (abs(float(row["bank_deg"])) for row in final), default=None
                ),
                "maximum_absolute_beta_deg": max(
                    (abs(float(row["beta_deg"])) for row in final), default=None
                ),
            },
            "energy_management": {
                "idle_command": guidance.idle_command,
                "idle_established": guidance.idle_established,
                "idle_lead_to_touchdown_ft": (
                    float(touchdown_summary["runway_along_ft"])
                    - float(guidance.idle_established["runway_along_ft"])
                    if guidance.idle_established and touchdown_summary
                    else None
                ),
                "idle_lead_to_touchdown_s": (
                    float(touchdown["elapsed_s"])
                    - float(guidance.idle_established["elapsed_s"])
                    if guidance.idle_established and touchdown
                    else None
                ),
            },
            "flare_dynamics": dynamics,
            "touchdown": touchdown_summary,
            "rollout": {
                "stopped": stopped,
                "stop_runway_along_ft": float(stop_row["runway_along_ft"])
                if stop_row
                else None,
                "distance_from_touchdown_to_stop_ft": ground_roll,
                "maximum_main_tire_skid_ratio": max_skid,
                "maximum_absolute_cross_track_ft_above_15_kt": max_cross,
                "maximum_absolute_heading_error_deg_above_15_kt": max_heading,
                "final_groundspeed_kt": float(stop_row["groundspeed_kt"])
                if stop_row
                else None,
            },
            "control_trace": control_trace,
            "trace": trace,
        }
        reasons = evaluate_result(result)
        result["rejection_reasons"] = reasons
        result["accepted"] = not reasons
        return result

    def _write_result(self, result: dict[str, Any]) -> None:
        self.output_path.parent.mkdir(parents=True, exist_ok=True)
        self.output_path.write_text(json.dumps(result, indent=2), encoding="utf-8")

    def _evaluate_result(self, result: dict[str, Any]) -> list[str]:
        return evaluate_result(result)

    def run(self) -> dict[str, Any]:
        loaded_mass = 0.0
        gear_layout: dict[str, Any] = {}
        try:
            loaded_mass, gear_layout = self._prepare_flight()
            self._start_media()
            if self.evidence.pre_release_hold_seconds > 0.0:
                time.sleep(self.evidence.pre_release_hold_seconds)
            self._set_pause(False)
            result = self._fly(loaded_mass, gear_layout)
        finally:
            self._stop_media()
            self._safe_set("sim/cockpit2/engine/actuators/throttle_ratio_all", 0.0)
            self._safe_set("sim/cockpit2/controls/left_brake_ratio", 0.0)
            self._safe_set("sim/cockpit2/controls/right_brake_ratio", 0.0)
            self._safe_set("sim/operation/override/override_toe_brakes", 0)
            self._safe_set("sim/operation/override/override_planepath", PLANEPATH_RELEASED)
            self._safe_set("sim/operation/override/override_artstab", 0)
            self._safe_set("sr20g6/test_controller/armed", 0)
            try:
                self._set_pause(True)
            except Exception:
                pass
        # Media stop timestamps/failures are known only after the finally block.
        result["evidence_capture"]["video_stop_utc"] = (
            self.video_stop_utc.isoformat() if self.video_stop_utc else None
        )
        result["evidence_capture"]["audio_capture_failure"] = self.audio_capture_failure
        audio_files_verified: bool | None = None
        if self.evidence.record_audio:
            audio_files_verified = bool(
                self.evidence.audio_output_path
                and self.evidence.audio_output_path.is_file()
                and self.evidence.audio_metadata_path
                and self.evidence.audio_metadata_path.is_file()
            )
        result["evidence_capture"]["audio_files_verified"] = audio_files_verified
        reasons = self._evaluate_result(result)
        result["rejection_reasons"] = reasons
        result["accepted"] = not reasons
        self._write_result(result)
        return result

    def _fly(self, loaded_mass: float, gear_layout: dict[str, Any]) -> dict[str, Any]:
        cfg = self.config
        guidance = LandingGuidance(cfg, self.runway)
        trace: list[dict[str, Any]] = []
        control_trace: list[dict[str, Any]] = []
        start = time.monotonic()
        deadline = start + cfg.timeout_seconds
        next_full_sample = start
        touchdown: dict[str, Any] | None = None
        last_airborne: dict[str, Any] | None = None
        threshold_crossing: dict[str, Any] | None = None
        touchdown_time: float | None = None
        touchdown_pitch_command = cfg.initial_pitch_deg
        nose_down = False
        stopped = False
        brake_command = 0.0
        previous_along = -1.0e9
        previous_loop_elapsed: float | None = None

        while time.monotonic() < deadline:
            now = time.monotonic()
            elapsed = now - start
            loop_dt = (
                0.10
                if previous_loop_elapsed is None
                else limit(elapsed - previous_loop_elapsed, 0.02, 1.0)
            )
            previous_loop_elapsed = elapsed
            stage = "approach" if touchdown is None else "rollout"
            control = self._control_sample()
            distance_to_touchdown = cfg.commanded_touchdown_ft - float(
                control["runway_along_ft"]
            )
            in_critical_flare = touchdown is None and distance_to_touchdown <= max(
                600.0, cfg.flare_lead_ft + 400.0
            )
            row: dict[str, Any] | None = None
            if (
                threshold_crossing is None
                and previous_along < 0.0 <= float(control["runway_along_ft"])
            ):
                threshold_crossing = self._threshold_row(control, elapsed, stage)
            previous_along = float(control["runway_along_ft"])

            if not in_critical_flare and now >= next_full_sample:
                row = self._sample(elapsed, stage)
                trace.append(row)
                next_full_sample = time.monotonic() + 0.75

            nose_on_ground = int(control["on_ground"][gear_layout["nose_index"]]) != 0
            any_on_ground = any(int(value) != 0 for value in control["on_ground"])
            if touchdown is None:
                if not any_on_ground and row is not None:
                    last_airborne = row
                if any_on_ground:
                    if row is None:
                        row = self._sample(elapsed, "rollout")
                        trace.append(row)
                        next_full_sample = time.monotonic() + 0.75
                    touchdown = row
                    touchdown_time = time.monotonic()
                    touchdown_pitch_command = guidance.commanded_pitch_target
                    self.api.set_dataref(
                        "sim/cockpit2/engine/actuators/throttle_ratio_all", 0.0
                    )
                    self.api.set_dataref("sr20g6/test_controller/target_bank_deg", 0.0)
                    self.api.set_dataref(
                        "sr20g6/test_controller/target_pitch_deg", touchdown_pitch_command
                    )
                    self.api.set_dataref("sim/operation/override/override_toe_brakes", 1)
                    brake_command = 0.15
                    self.api.set_dataref(
                        "sim/cockpit2/controls/left_brake_ratio", brake_command
                    )
                    self.api.set_dataref(
                        "sim/cockpit2/controls/right_brake_ratio", brake_command
                    )
                else:
                    command = guidance.step(control, elapsed)
                    if command.control_trace_row is not None:
                        control_trace.append(command.control_trace_row)
                    self.api.set_dataref(
                        "sr20g6/test_controller/target_bank_deg", command.bank_target_deg
                    )
                    self.api.set_dataref(
                        "sr20g6/test_controller/target_pitch_deg", command.pitch_target_deg
                    )
                    if command.power_idle:
                        self.api.set_dataref(
                            "sim/cockpit2/engine/actuators/throttle_ratio_all", 0.0
                        )
                    else:
                        self.api.set_dataref(
                            "sim/cockpit2/engine/actuators/throttle_ratio_all",
                            command.throttle_ratio,
                        )
            else:
                assert touchdown_time is not None
                since_touchdown = time.monotonic() - touchdown_time
                self.api.set_dataref(
                    "sim/cockpit2/engine/actuators/throttle_ratio_all", 0.0
                )
                if not nose_down:
                    post_touchdown_pitch = touchdown_pitch_command * limit(
                        1.0 - since_touchdown / 2.5, 0.0, 1.0
                    )
                    self.api.set_dataref(
                        "sr20g6/test_controller/target_pitch_deg", post_touchdown_pitch
                    )
                if not nose_down and (nose_on_ground or since_touchdown >= 2.5):
                    nose_down = True
                    self._set_flap_ratio(0.0)
                    self.api.set_dataref("sr20g6/test_controller/target_pitch_deg", 0.0)

                main_skid = max(
                    float(control["tire_skid_ratio"][index])
                    for index in gear_layout["main_indices"]
                )
                cadence_scale = loop_dt / REFERENCE_CONTROL_PERIOD_S
                if main_skid > 0.12:
                    brake_command = limit(
                        brake_command - 0.18 * cadence_scale, 0.10, 1.0
                    )
                else:
                    brake_command = limit(
                        brake_command + 0.055 * cadence_scale, 0.10, 1.0
                    )
                cross_correction = math.degrees(
                    math.atan2(-float(control["runway_cross_ft"]), 500.0)
                )
                desired_heading = self.runway.heading_true_deg + cross_correction
                heading_error = wrap_180(
                    desired_heading - float(control["heading_true_deg"])
                )
                self.api.set_dataref(
                    "sr20g6/test_controller/ground_target_heading_deg", desired_heading
                )
                steering_scale = limit(float(control["groundspeed_kt"]) / 20.0, 0.0, 1.0)
                differential = limit(0.005 * heading_error, -0.05, 0.05) * steering_scale
                self.api.set_dataref(
                    "sim/cockpit2/controls/left_brake_ratio",
                    limit(brake_command + differential, 0.0, 1.0),
                )
                self.api.set_dataref(
                    "sim/cockpit2/controls/right_brake_ratio",
                    limit(brake_command - differential, 0.0, 1.0),
                )
                if float(control["groundspeed_kt"]) <= 3.0 and since_touchdown >= 2.0:
                    if row is None:
                        trace.append(self._sample(elapsed, "rollout"))
                    stopped = True
                    break
            time.sleep(0.075)

        return self._build_result(
            loaded_mass_kg=loaded_mass,
            gear_layout=gear_layout,
            trace=trace,
            control_trace=control_trace,
            touchdown=touchdown,
            last_airborne=last_airborne,
            threshold_crossing=threshold_crossing,
            stopped=stopped,
            guidance=guidance,
        )


def add_config_arguments(parser: argparse.ArgumentParser) -> None:
    defaults = LandingConfig()
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--port", type=int, default=8142)
    for flag, attribute, value_type in (
        ("--commanded-touchdown-feet", "commanded_touchdown_ft", float),
        ("--flare-touchdown-target-feet", "flare_touchdown_target_ft", float),
        ("--start-distance-nm", "start_distance_nm", float),
        ("--glidepath-degrees", "glidepath_deg", float),
        ("--approach-kias", "approach_kias", float),
        ("--idle-gate-along-feet", "idle_gate_along_ft", float),
        ("--flare-lead-feet", "flare_lead_ft", float),
        ("--flare-capture-height-agl-feet", "flare_capture_height_agl_ft", float),
        ("--touchdown-pitch-degrees", "touchdown_pitch_deg", float),
        ("--maximum-touchdown-kias", "maximum_touchdown_kias", float),
        ("--minimum-touchdown-pitch-degrees", "minimum_touchdown_pitch_deg", float),
        ("--maximum-touchdown-sink-fpm", "maximum_touchdown_sink_fpm", float),
        (
            "--maximum-target-pitch-rate-degrees-per-second",
            "maximum_target_pitch_rate_deg_s",
            float,
        ),
        (
            "--maximum-actual-pitch-rate-degrees-per-second",
            "maximum_actual_pitch_rate_deg_s",
            float,
        ),
        ("--maximum-ground-roll-feet", "maximum_ground_roll_ft", float),
        ("--initial-throttle", "initial_throttle", float),
        ("--initial-pitch-degrees", "initial_pitch_deg", float),
        ("--timeout-seconds", "timeout_seconds", int),
    ):
        parser.add_argument(flag, dest=attribute, type=value_type, default=getattr(defaults, attribute))
    parser.add_argument("--record-video", action="store_true")
    parser.add_argument("--record-audio", action="store_true")
    parser.add_argument("--prepare-video-view", action="store_true")
    parser.add_argument("--pre-release-hold-seconds", type=float, default=0.0)
    parser.add_argument("--video-fov-degrees", type=float, default=82.0)
    parser.add_argument("--video-head-pitch-degrees", type=float, default=-4.0)
    parser.add_argument("--audio-capture-script", type=Path)
    parser.add_argument("--audio-module-dir", type=Path)
    parser.add_argument("--audio-output-path", type=Path)
    parser.add_argument("--audio-stop-file", type=Path)
    parser.add_argument("--audio-ready-file", type=Path)
    parser.add_argument("--audio-metadata-path", type=Path)
    parser.add_argument("--audio-sample-rate", type=int, choices=range(44100, 96001), default=48000)


def config_from_args(args: argparse.Namespace) -> LandingConfig:
    fields = set(LandingConfig.__dataclass_fields__)
    return LandingConfig(**{name: getattr(args, name) for name in fields if hasattr(args, name)})


def evidence_from_args(args: argparse.Namespace) -> EvidenceOptions:
    return EvidenceOptions(
        record_video=args.record_video,
        record_audio=args.record_audio,
        prepare_video_view=getattr(args, "prepare_video_view", False),
        pre_release_hold_seconds=getattr(args, "pre_release_hold_seconds", 0.0),
        video_fov_deg=args.video_fov_degrees,
        video_head_pitch_deg=args.video_head_pitch_degrees,
        audio_capture_script=args.audio_capture_script,
        audio_module_dir=args.audio_module_dir,
        audio_output_path=args.audio_output_path,
        audio_stop_file=args.audio_stop_file,
        audio_ready_file=args.audio_ready_file,
        audio_metadata_path=args.audio_metadata_path,
        audio_sample_rate=args.audio_sample_rate,
    )


def diagnostic_document(error: Exception) -> dict[str, Any]:
    return {
        "schema_version": 4,
        "generated_utc": utc_now().isoformat(),
        "accepted": False,
        "rejection_reasons": ["setup or runtime failure"],
        "diagnostic": {
            "exception_type": type(error).__name__,
            "message": str(error),
            "orchestrator": "flight_test.short_field",
            "python_version": sys.version.split()[0],
        },
    }


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Simulator-only SR20 G6 short-field landing harness"
    )
    subparsers = parser.add_subparsers(dest="action", required=True)
    run = subparsers.add_parser("run", help="fly a live X-Plane test card")
    add_config_arguments(run)
    replay = subparsers.add_parser("replay", help="re-evaluate stored result JSON")
    replay.add_argument("results", nargs="+", type=Path)
    return parser


def main(argv: Iterable[str] | None = None) -> int:
    args = build_parser().parse_args(list(argv) if argv is not None else None)
    if args.action == "replay":
        failed = False
        for path in args.results:
            document = json.loads(path.read_text(encoding="utf-8-sig"))
            reasons = evaluate_result(document)
            matches = reasons == list(document.get("rejection_reasons", []))
            print(
                json.dumps(
                    {
                        "path": str(path.resolve()),
                        "recorded_accepted": bool(document.get("accepted")),
                        "replayed_accepted": not reasons,
                        "rejection_reasons": reasons,
                        "matches_recorded_reasons": matches,
                    }
                )
            )
            failed = failed or not matches
        return 1 if failed else 0

    config = config_from_args(args)
    evidence = evidence_from_args(args)
    try:
        with XPlaneApi(port=args.port) as api:
            result = ShortFieldRunner(api, config, args.output, evidence).run()
    except Exception as error:
        diagnostic = diagnostic_document(error)
        args.output.resolve().parent.mkdir(parents=True, exist_ok=True)
        args.output.resolve().write_text(json.dumps(diagnostic, indent=2), encoding="utf-8")
        print(json.dumps(diagnostic, indent=2), file=sys.stderr)
        return 2
    print(
        json.dumps(
            {
                "output": str(args.output.resolve()),
                "accepted": result["accepted"],
                "rejection_reasons": result["rejection_reasons"],
                "touchdown": result["touchdown"],
                "rollout": result["rollout"],
            },
            indent=2,
        )
    )
    return 0 if result["accepted"] else 1


if __name__ == "__main__":
    raise SystemExit(main())

