"""Pure guidance and scoring for the SR20 G6 short-field landing card."""

from __future__ import annotations

from dataclasses import dataclass
import math
from typing import Any

from .geometry import FEET_PER_NM, KCDW_RUNWAY_22, RunwayGeometry, limit, wrap_180

REFERENCE_CONTROL_PERIOD_S = 0.142


@dataclass(slots=True)
class LandingConfig:
    """Accepted KCDW runway 22 test card and rejection bounds."""

    commanded_touchdown_ft: float = 1100.0
    flare_touchdown_target_ft: float = 950.0
    start_distance_nm: float = 1.2
    glidepath_deg: float = 3.0
    approach_kias: float = 78.0
    idle_gate_along_ft: float = -934.5
    flare_lead_ft: float = 1400.0
    flare_capture_height_agl_ft: float = 45.0
    touchdown_pitch_deg: float = 6.0
    maximum_touchdown_kias: float = 68.0
    minimum_touchdown_pitch_deg: float = 2.5
    maximum_touchdown_sink_fpm: float = 200.0
    maximum_target_pitch_rate_deg_s: float = 1.25
    maximum_actual_pitch_rate_deg_s: float = 3.5
    maximum_ground_roll_ft: float = 1000.0
    initial_throttle: float = 0.24
    initial_pitch_deg: float = -3.0
    timeout_seconds: int = 240
    target_mass_kg: float = 1428.81551

    def __post_init__(self) -> None:
        ranges = {
            "commanded_touchdown_ft": (600.0, 1500.0),
            "flare_touchdown_target_ft": (900.0, 1500.0),
            "start_distance_nm": (1.2, 3.5),
            "glidepath_deg": (2.5, 3.5),
            "approach_kias": (73.0, 83.0),
            "idle_gate_along_ft": (-1200.0, 500.0),
            "flare_lead_ft": (100.0, 1400.0),
            "flare_capture_height_agl_ft": (25.0, 70.0),
            "touchdown_pitch_deg": (2.0, 8.0),
            "maximum_touchdown_kias": (58.0, 72.0),
            "minimum_touchdown_pitch_deg": (1.0, 6.0),
            "maximum_touchdown_sink_fpm": (100.0, 300.0),
            "maximum_target_pitch_rate_deg_s": (0.5, 4.0),
            "maximum_actual_pitch_rate_deg_s": (2.0, 8.0),
            "maximum_ground_roll_ft": (700.0, 1300.0),
            "initial_throttle": (0.05, 0.6),
            "initial_pitch_deg": (-6.0, 8.0),
            "timeout_seconds": (90.0, 300.0),
        }
        for name, (low, high) in ranges.items():
            value = float(getattr(self, name))
            if not low <= value <= high:
                raise ValueError(f"{name}={value} is outside [{low}, {high}]")


@dataclass(slots=True)
class GuidanceOutput:
    bank_target_deg: float
    pitch_target_deg: float
    throttle_ratio: float
    desired_flare_vvi_fpm: float | None
    flare_active: bool
    power_idle: bool
    idle_command: dict[str, float] | None
    idle_established: dict[str, float] | None
    control_trace_row: dict[str, Any] | None


class LandingGuidance:
    """Deterministic low-rate outer loop; attitude control remains in the plugin."""

    def __init__(
        self,
        config: LandingConfig,
        runway: RunwayGeometry = KCDW_RUNWAY_22,
    ) -> None:
        self.config = config
        self.runway = runway
        self.glide_slope = math.tan(math.radians(config.glidepath_deg))
        self.previous_elapsed: float | None = None
        self.previous_vvi_fpm: float | None = None
        self.filtered_vvi_acceleration_fpm_s = 0.0
        self.last_control_dt = 0.10
        self.previous_along_ft = -1.0e9
        self.speed_throttle_trim = 0.45
        self.commanded_pitch_target = config.initial_pitch_deg
        self.power_idle = False
        self.flare_active = False
        self.idle_command: dict[str, float] | None = None
        self.idle_established: dict[str, float] | None = None

    def step(self, control: dict[str, Any], elapsed: float) -> GuidanceOutput:
        cfg = self.config
        control_dt = 0.10 if self.previous_elapsed is None else limit(
            elapsed - self.previous_elapsed, 0.02, 1.0
        )
        self.last_control_dt = control_dt
        self.previous_elapsed = elapsed
        vvi_fpm = float(control["vvi_fpm"])
        if self.previous_vvi_fpm is not None:
            raw_acceleration = (vvi_fpm - self.previous_vvi_fpm) / control_dt
            self.filtered_vvi_acceleration_fpm_s = (
                0.75 * self.filtered_vvi_acceleration_fpm_s + 0.25 * raw_acceleration
            )
        self.previous_vvi_fpm = vvi_fpm

        along_ft = float(control["runway_along_ft"])
        if (
            not self.power_idle
            and self.previous_along_ft < cfg.idle_gate_along_ft <= along_ft
        ):
            self.power_idle = True
            self.idle_command = {
                "elapsed_s": round(elapsed, 3),
                "runway_along_ft": along_ft,
                "agl_ft": float(control["agl_ft"]),
                "throttle_before_ratio": float(control["throttle_ratio"]),
            }
        self.previous_along_ft = along_ft
        if (
            self.power_idle
            and self.idle_established is None
            and float(control["throttle_ratio"]) <= 0.03
        ):
            self.idle_established = {
                "elapsed_s": round(elapsed, 3),
                "runway_along_ft": along_ft,
                "agl_ft": float(control["agl_ft"]),
                "throttle_ratio": float(control["throttle_ratio"]),
            }

        distance_to_touchdown = cfg.commanded_touchdown_ft - along_ft
        distance_to_flare_target = cfg.flare_touchdown_target_ft - along_ft
        desired_elevation_ft = (
            self.runway.elevation_ft
            + 5.0
            + self.glide_slope * max(0.0, distance_to_touchdown)
        )
        altitude_error_ft = desired_elevation_ft - float(control["elevation_msl_ft"])
        desired_vvi_fpm = (
            -float(control["groundspeed_kt"]) * FEET_PER_NM / 60.0 * self.glide_slope
        )
        vvi_error_fpm = desired_vvi_fpm - vvi_fpm

        cross_correction = math.degrees(
            math.atan2(-float(control["runway_cross_ft"]), 700.0)
        )
        desired_heading = self.runway.heading_true_deg + cross_correction
        heading_error = wrap_180(desired_heading - float(control["heading_true_deg"]))
        bank_target = limit(1.15 * heading_error, -15.0, 15.0)

        pitch_target = limit(
            -3.0 + 0.030 * altitude_error_ft + 0.0025 * vvi_error_fpm,
            -6.0,
            8.0,
        )
        speed_error_kias = cfg.approach_kias - float(control["ias_kias"])
        self.speed_throttle_trim = limit(
            self.speed_throttle_trim
            + 0.0015 * speed_error_kias * control_dt / REFERENCE_CONTROL_PERIOD_S,
            0.10,
            0.75,
        )
        throttle = limit(
            self.speed_throttle_trim + 0.015 * speed_error_kias,
            0.08,
            0.75,
        )

        desired_flare_vvi: float | None = None
        if (
            self.power_idle
            and distance_to_flare_target <= cfg.flare_lead_ft
            and (
                self.flare_active
                or float(control["agl_ft"]) <= cfg.flare_capture_height_agl_ft
            )
        ):
            self.flare_active = True
            bounded_agl = limit(
                float(control["agl_ft"]), 0.0, cfg.flare_capture_height_agl_ft
            )
            remaining_distance = max(80.0, distance_to_flare_target)
            groundspeed_fps = max(
                60.0, float(control["groundspeed_kt"]) * FEET_PER_NM / 3600.0
            )
            time_to_target = remaining_distance / groundspeed_fps
            desired_flare_vvi = limit(-60.0 * bounded_agl / time_to_target, -350.0, -80.0)
            flare_vvi_error = desired_flare_vvi - vvi_fpm
            pitch_target = limit(
                cfg.touchdown_pitch_deg + 0.008 * flare_vvi_error,
                0.0,
                8.0,
            )

        if self.power_idle:
            throttle = 0.0
        raw_pitch_target = pitch_target
        maximum_increase = cfg.maximum_target_pitch_rate_deg_s * control_dt
        maximum_relaxation = 1.25 * control_dt
        self.commanded_pitch_target += limit(
            raw_pitch_target - self.commanded_pitch_target,
            -maximum_relaxation,
            maximum_increase,
        )

        trace_row = None
        if distance_to_touchdown <= cfg.flare_lead_ft + 300.0:
            trace_row = {
                "elapsed_s": round(elapsed, 3),
                "runway_along_ft": along_ft,
                "distance_to_command_ft": distance_to_touchdown,
                "distance_to_flare_target_ft": distance_to_flare_target,
                "agl_ft": float(control["agl_ft"]),
                "ias_kias": float(control["ias_kias"]),
                "vvi_fpm": vvi_fpm,
                "actual_pitch_deg": float(control["pitch_deg"]),
                "actual_pitch_rate_deg_s": float(control["pitch_rate_deg_s"]),
                "normal_g": float(control["normal_g"]),
                "raw_pitch_target_deg": raw_pitch_target,
                "target_pitch_deg": self.commanded_pitch_target,
                "desired_flare_vvi_fpm": desired_flare_vvi,
                "flare_active": self.flare_active,
                "throttle_ratio": throttle,
            }
        return GuidanceOutput(
            bank_target_deg=bank_target,
            pitch_target_deg=self.commanded_pitch_target,
            throttle_ratio=throttle,
            desired_flare_vvi_fpm=desired_flare_vvi,
            flare_active=self.flare_active,
            power_idle=self.power_idle,
            idle_command=self.idle_command,
            idle_established=self.idle_established,
            control_trace_row=trace_row,
        )


def metric_range(rows: list[dict[str, Any]], key: str) -> dict[str, float] | None:
    if not rows:
        return None
    values = [float(row[key]) for row in rows]
    return {
        "mean": sum(values) / len(values),
        "minimum": min(values),
        "maximum": max(values),
        "range": max(values) - min(values),
    }


def flare_dynamics(control_trace: list[dict[str, Any]]) -> dict[str, Any]:
    rows = [row for row in control_trace if bool(row.get("flare_active"))]
    target_rates: list[float] = []
    vvi_accelerations: list[float] = []
    for previous, current in zip(rows, rows[1:]):
        dt = float(current["elapsed_s"]) - float(previous["elapsed_s"])
        if dt <= 0.0:
            continue
        target_rates.append(
            abs(float(current["target_pitch_deg"]) - float(previous["target_pitch_deg"])) / dt
        )
        vvi_accelerations.append(
            abs(float(current["vvi_fpm"]) - float(previous["vvi_fpm"])) / dt
        )
    return {
        "sample_count": len(rows),
        "maximum_absolute_target_pitch_rate_deg_s": max(target_rates, default=None),
        "maximum_absolute_actual_pitch_rate_deg_s": max(
            (abs(float(row["actual_pitch_rate_deg_s"])) for row in rows), default=None
        ),
        "maximum_normal_g": max((float(row["normal_g"]) for row in rows), default=None),
        "minimum_normal_g": min((float(row["normal_g"]) for row in rows), default=None),
        "maximum_absolute_vvi_acceleration_fpm_s": max(vvi_accelerations, default=None),
    }


def evaluate_result(document: dict[str, Any]) -> list[str]:
    """Re-evaluate all automatic gates from a schema-3/4 result document."""

    reasons: list[str] = []
    standard = document["standard"]
    approach = document["approach"]
    touchdown = document.get("touchdown")
    rollout = document["rollout"]
    energy = document["energy_management"]
    flare = document["flare_dynamics"]
    trace = document.get("trace", [])
    capture = document.get("evidence_capture", {})

    if touchdown is None:
        reasons.append("no touchdown detected before timeout")
    if not bool(rollout.get("stopped")):
        reasons.append("aircraft did not stop below 3 knots before timeout")
    if int(approach.get("sample_count", 0)) < 20:
        reasons.append("insufficient stabilized final-approach samples")
    ias = approach.get("ias_kias")
    if ias and (
        float(ias["minimum"])
        < float(standard["approach_speed_kias"]) - float(standard["approach_speed_tolerance_kias"])
        or float(ias["maximum"])
        > float(standard["approach_speed_kias"]) + float(standard["approach_speed_tolerance_kias"])
    ):
        reasons.append("published 78 KIAS approach speed exceeded the ACS +/-5 knot tolerance")

    final = [
        row
        for row in trace
        if row.get("stage") == "approach"
        and -FEET_PER_NM <= float(row["runway_along_ft"]) <= 0.0
        and float(row["agl_ft"]) >= 50.0
    ]
    if any(float(row["flap_actual_ratio"]) < 0.95 for row in final):
        reasons.append("full flaps were not maintained on final")
    if any(
        int(row["controller_active"]) != 1
        or int(row["custom_fm_enabled"]) != 1
        or int(row["custom_fm_aircraft_match"]) != 1
        or int(row["io390_enabled"]) != 1
        or int(row["io390_aircraft_match"]) != 1
        for row in final
    ):
        reasons.append("controller or SR20 G6 runtime lost active state on final")
    if document.get("threshold_crossing") is None:
        reasons.append("runway threshold crossing was not instrumented")
    if energy.get("idle_established") is None:
        reasons.append("idle power was not instrumented before touchdown")

    if int(flare.get("sample_count", 0)) < 10:
        reasons.append("insufficient high-rate flare-control samples")
    target_rate = flare.get("maximum_absolute_target_pitch_rate_deg_s")
    if target_rate is not None and float(target_rate) > float(
        standard["maximum_target_pitch_rate_deg_s"]
    ) + 0.15:
        reasons.append("flare target pitch rate exceeded the smooth-command limit")
    actual_rate = flare.get("maximum_absolute_actual_pitch_rate_deg_s")
    if actual_rate is not None and float(actual_rate) > float(
        standard["maximum_actual_pitch_rate_deg_s"]
    ):
        reasons.append("actual flare pitch rate was abrupt")
    max_g = flare.get("maximum_normal_g")
    min_g = flare.get("minimum_normal_g")
    if max_g is not None and float(max_g) > 1.25:
        reasons.append("flare normal acceleration was excessive")
    if min_g is not None and float(min_g) < 0.75:
        reasons.append("flare normal acceleration had an excessive unloading transient")

    if touchdown is not None:
        along = float(touchdown["runway_along_ft"])
        if along < float(standard["acceptable_touchdown_window_ft"][0]):
            reasons.append("touchdown was short of the nominated 1,000-foot point")
        if along > float(standard["acceptable_touchdown_window_ft"][1]):
            reasons.append("touchdown was more than 100 feet beyond the nominated point")
        if abs(float(touchdown["runway_cross_ft"])) > 12.0:
            reasons.append("touchdown was not over the runway centerline")
        if abs(float(touchdown["heading_error_deg"])) > 3.0:
            reasons.append("longitudinal axis was not aligned with runway 22")
        if abs(float(touchdown["beta_deg"])) > 3.0:
            reasons.append("touchdown had excessive side drift/sideslip")
        if not bool(touchdown["first_contact_latched"]):
            reasons.append("frame-rate first-contact event was not latched")
        elif int(touchdown["main_contact_count"]) < 1 or bool(touchdown["nose_contact"]):
            reasons.append("touchdown was not main-wheels first")
        if float(touchdown["throttle_ratio"]) > 0.06:
            reasons.append("power was not idle at touchdown")
        if float(touchdown["ias_kias"]) > float(standard["maximum_touchdown_kias"]):
            reasons.append("touchdown airspeed retained excessive approach energy")
        if float(touchdown["ias_kias"]) < 55.0:
            reasons.append("touchdown airspeed was below the bounded short-field test envelope")
        if float(touchdown["pitch_deg"]) < float(standard["minimum_touchdown_pitch_deg"]):
            reasons.append("touchdown attitude was too flat")
        if float(touchdown["pitch_deg"]) > 8.0:
            reasons.append("touchdown pitch attitude was excessive")
        idle_lead = energy.get("idle_lead_to_touchdown_ft")
        if idle_lead is not None and float(idle_lead) < float(standard["minimum_idle_lead_ft"]):
            reasons.append("idle power was not established far enough before touchdown")
        contact_vvi = touchdown.get("last_airborne_vvi_fpm")
        if contact_vvi is not None and abs(float(contact_vvi)) > float(
            standard["maximum_touchdown_sink_fpm"]
        ):
            reasons.append("touchdown sink rate was excessive")

    max_skid = rollout.get("maximum_main_tire_skid_ratio")
    if max_skid is not None and float(max_skid) > 0.35:
        reasons.append("braking produced excessive main-wheel skid")
    max_cross = rollout.get("maximum_absolute_cross_track_ft_above_15_kt")
    if max_cross is not None and float(max_cross) > 20.0:
        reasons.append("directional control did not maintain the runway centerline during rollout")
    ground_roll = rollout.get("distance_from_touchdown_to_stop_ft")
    if ground_roll is not None and float(ground_roll) > float(standard["maximum_ground_roll_ft"]):
        reasons.append("ground roll exceeded the bounded short-field target")
    if bool(capture.get("audio_requested")) and (
        capture.get("audio_capture_failure")
        or capture.get("audio_files_verified") is False
    ):
        reasons.append("audio capture failed or did not produce required files")
    return reasons
