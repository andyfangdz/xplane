"""KCDW runway 22 power-off 180 accuracy-landing test card."""

from __future__ import annotations

import argparse
from dataclasses import asdict, dataclass
import json
import math
from pathlib import Path
import sys
import time
from typing import Any, Iterable

from .api import XPlaneApi
from .geometry import FEET_PER_METER, KCDW_RUNWAY_22, RunwayGeometry, limit, wrap_180
from .landing import REFERENCE_CONTROL_PERIOD_S, metric_range
from .short_field import (
    EvidenceOptions,
    KNOT_TO_MPS,
    ShortFieldRunner,
    evidence_from_args,
    utc_now,
)


@dataclass(slots=True)
class PowerOff180Config:
    """Repeatable, zero-wind commercial-ACS maneuver card."""

    aircraft_path: str = "Aircraft/SR20 G6 Custom FM/SR20_G6_Custom_FM.acf"
    require_custom_mod: int = 1
    use_fixed_mass: int = 0
    use_torquesim_mass: int = 0
    use_torquesim_flaps: int = 0
    use_torquesim_engine: int = 0
    selected_touchdown_ft: float = 1000.0
    target_touchdown_ft: float = 600.0
    downwind_altitude_agl_ft: float = 1000.0
    # Positive cross-track is the right side of runway 22. One statute mile
    # matches the user's requested normal visual-pattern spacing.
    downwind_cross_ft: float = 5280.0
    pattern_turn_direction: int = 1
    start_along_ft: float = 9000.0
    abeam_along_ft: float = 1000.0
    turn_start_along_ft: float = -1700.0
    final_turn_cross_ft: float = 1510.0
    entry_kias: float = 100.0
    entry_speed_tolerance_kias: float = 2.0
    entry_gate_seconds: float = 2.0
    turn_kias: float = 85.0
    final_kias: float = 75.0
    initial_throttle: float = 0.68
    initial_pitch_deg: float = -2.5
    initial_flap_ratio: float = 0.0
    turn_flap_ratio: float = 0.5
    downwind_level_pitch_deg: float = -2.5
    downwind_altitude_gain_deg_per_ft: float = 0.012
    downwind_altitude_integral_gain_deg_per_ft_s: float = 0.0008
    downwind_altitude_integral_limit_deg: float = 2.0
    downwind_vvi_gain_deg_per_fpm: float = 0.003
    downwind_throttle_gain_ratio_per_kt_s: float = 0.0005
    downwind_throttle_proportional_gain_ratio_per_kt: float = 0.02
    minimum_downwind_throttle: float = 0.15
    maximum_downwind_throttle: float = 0.90
    downwind_seed_terrain_offset_ft: float = 10.0
    full_flap_along_ft: float = -3000.0
    turn_bank_deg: float = 28.0
    final_turn_bank_deg: float = 25.0
    flare_lead_ft: float = 1500.0
    flare_capture_height_agl_ft: float = 34.0
    flare_pitch_rate_deg_s: float = 3.8
    flare_pitch_relax_rate_deg_s: float = 3.0
    maximum_flare_pitch_deg: float = 12.0
    touchdown_pitch_deg: float = 6.0
    approach_trim_ratio: float = 0.18
    trim_slew_ratio_s: float = 0.06
    normal_g_damping_deg_per_g: float = 30.0
    low_speed_pitch_support_deg_per_kt: float = 0.4
    maximum_touchdown_kias: float = 70.0
    minimum_touchdown_pitch_deg: float = 2.5
    # CA.IV.M has no numeric sink-rate tolerance.  This user-required ceiling is
    # therefore an additional hard acceptance gate, not an attributed ACS value.
    maximum_touchdown_sink_fpm: float = 200.0
    timeout_seconds: int = 240
    target_mass_kg: float = 1428.81551


@dataclass(slots=True)
class ManeuverCommand:
    phase: str
    bank_target_deg: float
    pitch_target_deg: float
    throttle_ratio: float
    flap_ratio: float
    elevator_trim_ratio: float
    flare_active: bool


class PowerOff180Guidance:
    """Low-rate maneuver sequencer; the aircraft plugin owns all axis inner loops."""

    def __init__(
        self, config: PowerOff180Config, runway: RunwayGeometry = KCDW_RUNWAY_22
    ) -> None:
        if config.pattern_turn_direction not in (-1, 1):
            raise ValueError("pattern_turn_direction must be -1 (left) or +1 (right)")
        self.config = config
        self.runway = runway
        self.phase = "downwind"
        self.previous_elapsed: float | None = None
        self.previous_along_ft = float("inf")
        self.pitch_target_deg = config.initial_pitch_deg
        self.power_off_command: dict[str, float] | None = None
        self.idle_established: dict[str, float] | None = None
        self.phase_events: list[dict[str, Any]] = []
        self.flare_active = False
        self.trim_command_ratio = 0.0
        self.throttle_command_ratio = config.initial_throttle
        self.downwind_throttle_trim_ratio = config.initial_throttle
        self.downwind_altitude_integral_deg = 0.0

    @property
    def downwind_heading(self) -> float:
        return (self.runway.heading_true_deg + 180.0) % 360.0

    @property
    def base_heading(self) -> float:
        return (
            self.runway.heading_true_deg
            - 90.0 * self.config.pattern_turn_direction
        ) % 360.0

    def _transition(self, phase: str, control: dict[str, Any], elapsed: float) -> None:
        if phase == self.phase:
            return
        self.phase = phase
        self.phase_events.append(
            {
                "phase": phase,
                "elapsed_s": round(elapsed, 3),
                "runway_along_ft": float(control["runway_along_ft"]),
                "runway_cross_ft": float(control["runway_cross_ft"]),
                "agl_ft": float(control["agl_ft"]),
                "ias_kias": float(control["ias_kias"]),
                "mass_kg": float(control.get("mass_kg", self.config.target_mass_kg)),
                "heading_true_deg": float(control["heading_true_deg"]),
            }
        )

    def step(self, control: dict[str, Any], elapsed: float) -> ManeuverCommand:
        cfg = self.config
        dt = 0.10 if self.previous_elapsed is None else limit(
            elapsed - self.previous_elapsed, 0.02, 1.0
        )
        self.previous_elapsed = elapsed
        along = float(control["runway_along_ft"])
        cross = float(control["runway_cross_ft"])
        heading = float(control["heading_true_deg"])

        if self.phase == "downwind" and self.previous_along_ft > cfg.abeam_along_ft >= along:
            self.power_off_command = {
                "elapsed_s": round(elapsed, 3),
                "runway_along_ft": along,
                "runway_cross_ft": cross,
                "agl_ft": float(control["agl_ft"]),
                "ias_kias": float(control["ias_kias"]),
                "mass_kg": float(control["mass_kg"]),
                "bank_deg": float(control["bank_deg"]),
                "heading_true_deg": heading,
                "throttle_before_ratio": float(control["throttle_ratio"]),
                "flap_actual_ratio": float(control["flap_actual_ratio"]),
                "engine_power_w": float(control.get("engine_power_w", 0.0)),
                "fuel_flow_kg_s": float(control.get("fuel_flow_kg_s", 0.0)),
                "engine_rpm": float(control.get("engine_rpm", 0.0)),
            }
            self._transition("power_off_downwind", control, elapsed)
        self.previous_along_ft = along

        if self.power_off_command and self.idle_established is None and float(
            control["throttle_ratio"]
        ) <= 0.03:
            self.idle_established = {
                "elapsed_s": round(elapsed, 3),
                "runway_along_ft": along,
                "agl_ft": float(control["agl_ft"]),
                "throttle_ratio": float(control["throttle_ratio"]),
            }

        if self.phase == "power_off_downwind" and along <= cfg.turn_start_along_ft:
            self._transition("turn_to_base", control, elapsed)
        if self.phase == "turn_to_base":
            turn_progress = cfg.pattern_turn_direction * wrap_180(
                heading - self.downwind_heading
            )
            if turn_progress >= 80.0:
                self._transition("base", control, elapsed)
        if (
            self.phase == "base"
            and cfg.pattern_turn_direction * cross
            <= cfg.pattern_turn_direction * cfg.final_turn_cross_ft
        ):
            self._transition("turn_to_final", control, elapsed)
        # Begin the final-mode blend before the heading is nearly aligned.
        # At the former eight-degree boundary the airplane still carried about
        # 17 degrees of bank under native video cadence and overshot centerline
        # during rollout.
        if self.phase == "turn_to_final" and abs(
            wrap_180(self.runway.heading_true_deg - heading)
        ) <= 12.0:
            self._transition("final", control, elapsed)

        throttle = self.throttle_command_ratio if self.phase == "downwind" else 0.0
        flap = cfg.initial_flap_ratio
        bank_target = 0.0
        target_kias = cfg.entry_kias
        if self.phase in ("downwind", "power_off_downwind"):
            heading_error = wrap_180(self.downwind_heading - heading)
            cross_error = cfg.downwind_cross_ft - cross
            bank_target = limit(0.9 * heading_error - 0.003 * cross_error, -12.0, 12.0)
        elif self.phase == "turn_to_base":
            bank_target = cfg.pattern_turn_direction * cfg.turn_bank_deg
            target_kias = cfg.turn_kias
            flap = cfg.turn_flap_ratio
        elif self.phase == "base":
            heading_error = wrap_180(self.base_heading - heading)
            bank_target = limit(1.1 * heading_error, -18.0, 18.0)
            target_kias = cfg.turn_kias
            flap = cfg.turn_flap_ratio
        elif self.phase == "turn_to_final":
            heading_error = wrap_180(self.runway.heading_true_deg - heading)
            minimum_bank = (
                -8.0 if cfg.pattern_turn_direction > 0 else -cfg.final_turn_bank_deg
            )
            maximum_bank = (
                cfg.final_turn_bank_deg if cfg.pattern_turn_direction > 0 else 8.0
            )
            bank_target = limit(0.95 * heading_error, minimum_bank, maximum_bank)
            target_kias = cfg.final_kias
            flap = cfg.turn_flap_ratio
        else:
            # Fly a shallow intercept instead of pointing directly at a nearby
            # centerline point.  The latter became increasingly aggressive in
            # the roundout and produced an avoidable bank reversal.  Five
            # degrees is enough to remove the residual rollout offset while
            # preserving runway alignment for touchdown.
            cross_correction = limit(
                math.degrees(math.atan2(-cross, 500.0)), -5.0, 5.0
            )
            desired_heading = self.runway.heading_true_deg + cross_correction
            bank_target = limit(
                1.15 * wrap_180(desired_heading - heading), -15.0, 15.0
            )
            target_kias = cfg.final_kias
            flap = 1.0 if along >= cfg.full_flap_along_ft else cfg.turn_flap_ratio

        if self.phase == "downwind":
            # Before the maneuver, use the two available energy controls for
            # distinct jobs: pitch maintains pattern altitude while power
            # acquires the requested IAS.  The former speed-only pitch loop
            # converted added power into a climb and arrived abeam both slow
            # and high.
            altitude_error = cfg.downwind_altitude_agl_ft - float(control["agl_ft"])
            self.downwind_altitude_integral_deg = limit(
                self.downwind_altitude_integral_deg
                + cfg.downwind_altitude_integral_gain_deg_per_ft_s
                * altitude_error
                * dt,
                -cfg.downwind_altitude_integral_limit_deg,
                cfg.downwind_altitude_integral_limit_deg,
            )
            if abs(float(control["vvi_fpm"])) <= 150.0:
                # Once the initial vertical transient is controlled, power may
                # acquire airspeed independently of the remaining altitude
                # error. Pitch, including its integral term, rejects the climb
                # that would otherwise accompany the added power.
                speed_error = cfg.entry_kias - float(control["ias_kias"])
                self.downwind_throttle_trim_ratio = limit(
                    self.downwind_throttle_trim_ratio
                    + cfg.downwind_throttle_gain_ratio_per_kt_s
                    * speed_error
                    * dt,
                    cfg.minimum_downwind_throttle,
                    cfg.maximum_downwind_throttle,
                )
                self.throttle_command_ratio = limit(
                    self.downwind_throttle_trim_ratio
                    + cfg.downwind_throttle_proportional_gain_ratio_per_kt
                    * speed_error,
                    cfg.minimum_downwind_throttle,
                    cfg.maximum_downwind_throttle,
                )
            throttle = self.throttle_command_ratio
            raw_pitch = (
                cfg.downwind_level_pitch_deg
                + cfg.downwind_altitude_gain_deg_per_ft * altitude_error
                + self.downwind_altitude_integral_deg
                - cfg.downwind_vvi_gain_deg_per_fpm
                * float(control["vvi_fpm"])
            )
        else:
            raw_pitch = -0.8 + 0.14 * (float(control["ias_kias"]) - target_kias)
        distance = cfg.target_touchdown_ft - along
        if self.phase == "final":
            if distance > 0.0:
                desired_altitude = self.runway.elevation_ft + 5.0 + math.tan(
                    math.radians(3.2)
                ) * distance
                altitude_error = desired_altitude - float(control["elevation_msl_ft"])
                desired_vvi = -float(control["groundspeed_kt"]) * 6076.12 / 60.0 * math.tan(
                    math.radians(3.2)
                )
                raw_pitch += 0.022 * altitude_error + 0.0022 * (
                    desired_vvi - float(control["vvi_fpm"])
                )
            if self.flare_active or (
                distance <= cfg.flare_lead_ft
                and float(control["agl_ft"]) <= cfg.flare_capture_height_agl_ft
            ):
                # Once begun, the flare is irreversible.  Passing the nominal
                # target must never return the controller to a nose-down glide.
                self.flare_active = True
                # Schedule the desired descent rate by height, not time remaining
                # to the touchdown point.  The former schedule could demand as
                # much as -350 fpm within the last ten feet and produced a firm
                # arrival despite an otherwise stable roundout.
                agl_ft = max(0.0, float(control["agl_ft"]))
                desired_flare_vvi = limit(
                    (
                        -100.0 - 8.0 * agl_ft
                        if agl_ft > 25.0
                        else -80.0 - 15.0 * max(0.0, agl_ft - 10.0)
                    ),
                    -650.0,
                    -80.0,
                )
                raw_pitch = limit(
                    cfg.touchdown_pitch_deg
                    + 0.012 * (desired_flare_vvi - float(control["vvi_fpm"]))
                    + cfg.low_speed_pitch_support_deg_per_kt
                    * max(0.0, 67.0 - float(control["ias_kias"]))
                    - cfg.normal_g_damping_deg_per_g
                    * max(0.0, float(control["normal_g"]) - 1.0),
                    0.0,
                    cfg.maximum_flare_pitch_deg,
                )

        raw_pitch = limit(
            raw_pitch,
            -7.0,
            cfg.maximum_flare_pitch_deg if self.flare_active else 8.0,
        )
        pitch_rate = cfg.flare_pitch_rate_deg_s if self.flare_active else 1.25
        pitch_relax_rate = (
            cfg.flare_pitch_relax_rate_deg_s if self.flare_active else 1.25
        )
        self.pitch_target_deg += limit(
            raw_pitch - self.pitch_target_deg,
            -pitch_relax_rate * dt,
            pitch_rate * dt,
        )
        trim_target = cfg.approach_trim_ratio if self.phase == "final" else 0.0
        self.trim_command_ratio += limit(
            trim_target - self.trim_command_ratio,
            -cfg.trim_slew_ratio_s * dt,
            cfg.trim_slew_ratio_s * dt,
        )
        return ManeuverCommand(
            phase=self.phase,
            bank_target_deg=bank_target,
            pitch_target_deg=self.pitch_target_deg,
            throttle_ratio=throttle,
            flap_ratio=flap,
            elevator_trim_ratio=self.trim_command_ratio,
            flare_active=self.flare_active,
        )


def evaluate_power_off_180(document: dict[str, Any]) -> list[str]:
    reasons: list[str] = []
    touchdown = document.get("touchdown")
    maneuver = document.get("maneuver", {})
    energy = document.get("energy_management", {})
    trace = document.get("trace", [])
    standard = document["standard"]

    if touchdown is None:
        reasons.append("no touchdown detected before timeout")
    if energy.get("power_off_command") is None or energy.get("idle_established") is None:
        reasons.append("power was not instrumented at idle from the abeam point")
    maximum_throttle = energy.get("maximum_throttle_after_power_off")
    if maximum_throttle is None or float(maximum_throttle) > 0.06:
        reasons.append("power was reintroduced after the abeam power reduction")
    abeam = energy.get("power_off_command")
    if abeam:
        if abs(float(abeam["runway_along_ft"]) - 1000.0) > 120.0:
            reasons.append("power was not reduced abeam the selected touchdown point")
        if abs(float(abeam["agl_ft"]) - 1000.0) > 100.0:
            reasons.append("downwind altitude at the abeam point exceeded the +/-100 ft card gate")
        commanded_entry_kias = float(
            document.get("command", {}).get("entry_kias", 100.0)
        )
        entry_speed_tolerance = float(
            document.get("command", {}).get("entry_speed_tolerance_kias", 2.0)
        )
        if abs(float(abeam["ias_kias"]) - commanded_entry_kias) > entry_speed_tolerance:
            reasons.append("power was not reduced at the required 100 KIAS entry speed")
        commanded_cross = document.get("command", {}).get("downwind_cross_ft")
        if commanded_cross is not None and abs(
            float(abeam["runway_cross_ft"]) - float(commanded_cross)
        ) > 300.0:
            reasons.append(
                "abeam downwind spacing was not within 300 feet of the commanded distance"
            )
        if abs(wrap_180(float(abeam["heading_true_deg"]) - float(maneuver["downwind_heading_true_deg"]))) > 10.0:
            reasons.append("downwind was not parallel to runway 22 at the abeam point")
        commanded_initial_flap = float(
            document.get("command", {}).get("initial_flap_ratio", 0.5)
        )
        if "flap_actual_ratio" in abeam and abs(
            float(abeam["flap_actual_ratio"]) - commanded_initial_flap
        ) > 0.08:
            reasons.append("the airplane was not at the commanded abeam flap detent")
        if bool(int(document.get("command", {}).get("use_torquesim_engine", 0))) and (
            float(abeam.get("engine_power_w", 0.0)) <= 10000.0
            or float(abeam.get("fuel_flow_kg_s", 0.0)) <= 0.0001
        ):
            reasons.append("the TorqueSim engine was not producing power before the abeam reduction")
        gate_seconds = float(
            document.get("command", {}).get("entry_gate_seconds", 2.0)
        )
        abeam_elapsed = float(abeam.get("elapsed_s", 0.0))
        entry_rows = [
            row
            for row in trace
            if row.get("stage") != "rollout"
            and abeam_elapsed - gate_seconds <= float(row.get("elapsed_s", -1.0))
            <= abeam_elapsed
        ]
        if (
            len(entry_rows) < 5
            or float(entry_rows[-1]["elapsed_s"])
            - float(entry_rows[0]["elapsed_s"])
            < gate_seconds - 0.5
        ):
            reasons.append("the 100 KIAS abeam entry was not sustained for two seconds")
        elif any(
            abs(float(row["ias_kias"]) - commanded_entry_kias)
            > entry_speed_tolerance
            or abs(float(row["agl_ft"]) - 1000.0) > 100.0
            or abs(float(row["runway_cross_ft"]) - float(commanded_cross)) > 300.0
            or abs(
                wrap_180(
                    float(row["heading_true_deg"])
                    - float(maneuver["downwind_heading_true_deg"])
                )
            )
            > 10.0
            or abs(float(row["bank_deg"])) > 3.0
            or abs(float(row["flap_actual_ratio"]) - commanded_initial_flap) > 0.08
            or abs(
                float(row.get("mass_kg", 0.0))
                - float(document.get("command", {}).get("target_mass_kg", 0.0))
            )
            > 5.0
            for row in entry_rows
        ):
            reasons.append("the 100 KIAS abeam entry gate was not continuously satisfied")
    if float(maneuver.get("heading_change_deg") or 0.0) < 170.0:
        reasons.append("the descending approach did not complete a 180-degree heading change")
    if float(maneuver.get("maximum_absolute_bank_deg") or 99.0) > 35.0:
        reasons.append("bank exceeded the 35-degree maneuver quality limit")
    if float(maneuver.get("maximum_absolute_beta_deg") or 99.0) > 5.0:
        reasons.append("the maneuver was not acceptably coordinated")
    maneuver_start_elapsed = (
        float(abeam["elapsed_s"])
        if abeam and abeam.get("elapsed_s") is not None
        else None
    )
    airborne = [
        row
        for row in trace
        if row.get("stage") != "rollout"
        and (
            maneuver_start_elapsed is None
            or float(row.get("elapsed_s", maneuver_start_elapsed))
            >= maneuver_start_elapsed
        )
    ]
    require_custom_mod = bool(
        int(document.get("command", {}).get("require_custom_mod", 1))
    )
    if any(
        int(row.get("controller_active", 0)) != 1
        or (
            require_custom_mod
            and (
                int(row.get("custom_fm_enabled", 0)) != 1
                or int(row.get("custom_fm_aircraft_match", 0)) != 1
                or int(row.get("io390_enabled", 0)) != 1
                or int(row.get("io390_aircraft_match", 0)) != 1
            )
        )
        for row in airborne
    ):
        reasons.append("controller or SR20 G6 runtime lost active state while airborne")
    if any(
        "planepath_override_0" in row
        and int(row["planepath_override_0"]) != 0
        for row in airborne
    ):
        reasons.append("planepath override was active during the measured maneuver")
    touchdown_along = (
        float(touchdown["runway_along_ft"]) if touchdown is not None else 1000.0
    )
    full_flap_rows = [
        row
        for row in airborne
        if row.get("stage") == "final"
        and touchdown_along - 500.0
        <= float(row.get("runway_along_ft", -9999.0))
        <= touchdown_along + 50.0
    ]
    # Three consecutive 4 Hz samples in the final 500 feet before contact are
    # enough to prove that flap transit completed before touchdown. Anchor the
    # window to actual contact so a short landing is not falsely double-failed
    # merely because it never reached the fixed 500-foot runway station.
    if len(full_flap_rows) < 3 or any(
        float(row.get("flap_actual_ratio", 0.0)) < 0.90 for row in full_flap_rows
    ):
        reasons.append("full flaps were not established on short final and for touchdown")

    if touchdown is not None:
        low, high = standard["acceptable_touchdown_window_ft"]
        along = float(touchdown["runway_along_ft"])
        if along < float(low):
            reasons.append("touchdown was short of the specified 1,000-foot point")
        if along > float(high):
            reasons.append("touchdown was more than 200 feet beyond the specified point")
        if abs(float(touchdown["runway_cross_ft"])) > 12.0:
            reasons.append("touchdown was not over the runway centerline")
        if abs(float(touchdown["heading_error_deg"])) > 3.0:
            reasons.append("longitudinal axis was not aligned with runway 22")
        if abs(float(touchdown["beta_deg"])) > 3.0:
            reasons.append("touchdown had excessive side drift or sideslip")
        if not touchdown.get("first_contact_latched"):
            reasons.append("frame-rate first contact was not latched")
        elif int(touchdown["main_contact_count"]) < 1 or touchdown["nose_contact"]:
            reasons.append("touchdown was not main-wheels first")
        if not 55.0 <= float(touchdown["ias_kias"]) <= float(
            standard["maximum_touchdown_kias"]
        ):
            reasons.append("touchdown airspeed was outside the bounded quality envelope")
        if not float(standard["minimum_touchdown_pitch_deg"]) <= float(
            touchdown["pitch_deg"]
        ) <= 8.0:
            reasons.append("touchdown did not use a proper pitch attitude")
        if float(touchdown["throttle_ratio"]) > 0.06:
            reasons.append("power was not idle at touchdown")
        if abs(float(touchdown.get("last_airborne_vvi_fpm") or 9999.0)) > float(
            standard["maximum_touchdown_sink_fpm"]
        ):
            reasons.append(
                "touchdown exceeded the user-required 200 fpm sink-rate limit; "
                "FAA-S-ACS-7B CA.IV.M specifies no numeric sink-rate tolerance"
            )
    return reasons


def evaluate_quality(document: dict[str, Any]) -> list[str]:
    """Supplemental, explicitly non-ACS handling-quality observations."""

    reasons: list[str] = []
    flare = document.get("flare_quality", {})
    touchdown = document.get("touchdown")
    if flare.get("maximum_actual_pitch_rate_deg_s") is not None and float(
        flare["maximum_actual_pitch_rate_deg_s"]
    ) > 4.5:
        reasons.append("roundout pitch rate exceeded the 4.5 deg/s quality ceiling")
    if flare.get("maximum_normal_g") is not None and float(flare["maximum_normal_g"]) > 1.25:
        reasons.append("roundout normal acceleration exceeded 1.25 g")
    if flare.get("minimum_normal_g") is not None and float(flare["minimum_normal_g"]) < 0.75:
        reasons.append("roundout normal acceleration fell below 0.75 g")
    return reasons


class PowerOff180Runner(ShortFieldRunner):
    def __init__(
        self,
        api: XPlaneApi,
        config: PowerOff180Config,
        output_path: Path,
        evidence: EvidenceOptions | None = None,
        runway: RunwayGeometry = KCDW_RUNWAY_22,
    ) -> None:
        super().__init__(api, config, output_path, evidence, runway)  # type: ignore[arg-type]

    def _initial_flap_ratio(self) -> float:
        return self.config.initial_flap_ratio

    def _initial_ias_target(self) -> float:
        return self.config.entry_kias

    def _initial_kinematics(self, start_tas_mps: float) -> tuple[float, float, float, float]:
        return (
            (self.runway.heading_true_deg + 180.0) % 360.0,
            self.config.initial_pitch_deg,
            0.0,
            0.0,
        )

    def _controller_setup(self) -> None:
        values = {
            "sr20g6/test_controller/authority": 0.72,
            "sr20g6/test_controller/pitch_authority": 0.95,
            "sr20g6/test_controller/yaw_authority": 0.45,
            "sr20g6/test_controller/ground_target_heading_deg": self.runway.heading_true_deg,
            "sr20g6/test_controller/target_bank_deg": 0.0,
            "sr20g6/test_controller/target_pitch_deg": self.config.initial_pitch_deg,
            "sim/cockpit2/controls/elevator_trim": 0.0,
            "sr20g6/test_controller/mode": 2,
            "sr20g6/test_controller/armed": 1,
        }
        for name, value in values.items():
            self.api.set_dataref(name, value)

    def _flight_payload(self) -> tuple[dict[str, Any], tuple[float, float], float, float]:
        cfg = self.config
        point = self.runway.point(cfg.start_along_ft, cfg.downwind_cross_ft)
        # Seed close to the local terrain under the start point.  The downwind
        # altitude loop then follows measured AGL as terrain changes toward the
        # abeam point.
        elevation_m = (
            self.runway.elevation_ft
            + cfg.downwind_altitude_agl_ft
            + cfg.downwind_seed_terrain_offset_ft
        ) / FEET_PER_METER
        # The load-flight API accepts TAS. This measured multiplier initializes
        # the aircraft at 100 KIAS at the KCDW pattern altitude in the fixed ISA
        # test weather, avoiding an artificial acceleration transient.
        tas_mps = cfg.entry_kias * 1.06 * KNOT_TO_MPS
        payload = {
            "data": {
                "aircraft": {"path": cfg.aircraft_path},
                "lle_air_start": {
                    "latitude": point[0],
                    "longitude": point[1],
                    "elevation_in_meters": elevation_m,
                    "heading_true": (self.runway.heading_true_deg + 180.0) % 360.0,
                    "speed_in_meters_per_second": tas_mps,
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
                            {"altitude_in_feet_msl": 0, "speed_in_knots": 0, "direction_in_degrees_true": 0, "turbulence_ratio": 0},
                            {"altitude_in_feet_msl": 16000, "speed_in_knots": 0, "direction_in_degrees_true": 0, "turbulence_ratio": 0},
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
        return payload, point, cfg.downwind_altitude_agl_ft, tas_mps

    def _evaluate_result(self, result: dict[str, Any]) -> list[str]:
        return evaluate_power_off_180(result)

    def _fly(self, loaded_mass: float, gear_layout: dict[str, Any]) -> dict[str, Any]:
        cfg = self.config
        guidance = PowerOff180Guidance(cfg, self.runway)
        trace: list[dict[str, Any]] = []
        control_trace: list[dict[str, Any]] = []
        start = time.monotonic()
        deadline = start + cfg.timeout_seconds
        next_sample = start
        touchdown: dict[str, Any] | None = None
        last_airborne: dict[str, Any] | None = None
        threshold_crossing: dict[str, Any] | None = None
        touchdown_time: float | None = None
        brake_command = 0.0
        stopped = False
        previous_along: float | None = None
        previous_elapsed: float | None = None

        while time.monotonic() < deadline:
            now = time.monotonic()
            elapsed = now - start
            dt = 0.10 if previous_elapsed is None else limit(elapsed - previous_elapsed, 0.02, 1.0)
            previous_elapsed = elapsed
            control = self._control_sample()
            stage = guidance.phase if touchdown is None else "rollout"
            row: dict[str, Any] | None = None
            if now >= next_sample:
                row = self._sample(elapsed, stage)
                trace.append(row)
                next_sample = now + (0.20 if touchdown is None else 0.50)
            if (
                threshold_crossing is None
                and guidance.phase == "final"
                and previous_along is not None
                and previous_along < 0.0 <= float(control["runway_along_ft"])
            ):
                threshold_crossing = self._threshold_row(control, elapsed, stage)
            previous_along = float(control["runway_along_ft"])

            nose_on_ground = int(control["on_ground"][gear_layout["nose_index"]]) != 0
            any_on_ground = any(int(value) != 0 for value in control["on_ground"])
            if touchdown is None:
                if not any_on_ground and row is not None:
                    last_airborne = row
                if any_on_ground:
                    if row is None:
                        row = self._sample(elapsed, "rollout")
                        trace.append(row)
                    touchdown = row
                    touchdown_time = now
                    self.api.set_dataref("sim/cockpit2/engine/actuators/throttle_ratio_all", 0.0)
                    self.api.set_dataref("sr20g6/test_controller/target_bank_deg", 0.0)
                    self.api.set_dataref("sim/operation/override/override_toe_brakes", 1)
                    brake_command = 0.12
                else:
                    command = guidance.step(control, elapsed)
                    self.api.set_dataref("sr20g6/test_controller/target_bank_deg", command.bank_target_deg)
                    self.api.set_dataref("sr20g6/test_controller/target_pitch_deg", command.pitch_target_deg)
                    self.api.set_dataref("sim/cockpit2/engine/actuators/throttle_ratio_all", command.throttle_ratio)
                    self._set_flap_ratio(command.flap_ratio)
                    self.api.set_dataref(
                        "sim/cockpit2/controls/elevator_trim",
                        command.elevator_trim_ratio,
                    )
                    control_trace.append({
                        "elapsed_s": round(elapsed, 3), "phase": command.phase,
                        "runway_along_ft": float(control["runway_along_ft"]),
                        "runway_cross_ft": float(control["runway_cross_ft"]),
                        "agl_ft": float(control["agl_ft"]), "ias_kias": float(control["ias_kias"]),
                        "heading_true_deg": float(control["heading_true_deg"]),
                        "bank_deg": float(control["bank_deg"]), "pitch_deg": float(control["pitch_deg"]),
                        "beta_deg": float(control["beta_deg"]), "vvi_fpm": float(control["vvi_fpm"]),
                        "normal_g": float(control["normal_g"]),
                        "throttle_ratio": float(control["throttle_ratio"]),
                        "bank_target_deg": command.bank_target_deg,
                        "pitch_target_deg": command.pitch_target_deg,
                        "flap_target_ratio": command.flap_ratio, "flare_active": command.flare_active,
                        "elevator_trim_ratio": float(control["elevator_trim_ratio"]),
                        "elevator_trim_target_ratio": command.elevator_trim_ratio,
                    })
            else:
                assert touchdown_time is not None
                since_touchdown = now - touchdown_time
                self.api.set_dataref("sim/cockpit2/engine/actuators/throttle_ratio_all", 0.0)
                self.api.set_dataref("sim/cockpit2/controls/elevator_trim", 0.0)
                self.api.set_dataref("sr20g6/test_controller/ground_target_heading_deg", self.runway.heading_true_deg)
                self.api.set_dataref("sr20g6/test_controller/target_pitch_deg", max(0.0, guidance.pitch_target_deg * (1.0 - since_touchdown / 2.5)))
                if nose_on_ground or since_touchdown >= 2.5:
                    self._set_flap_ratio(0.0)
                main_skid = max(float(control["tire_skid_ratio"][index]) for index in gear_layout["main_indices"])
                cadence = dt / REFERENCE_CONTROL_PERIOD_S
                brake_command = limit(
                    brake_command + (-0.18 if main_skid > 0.12 else 0.055) * cadence,
                    0.10, 1.0,
                )
                heading_error = wrap_180(self.runway.heading_true_deg - float(control["heading_true_deg"]))
                differential = limit(0.005 * heading_error, -0.05, 0.05)
                self.api.set_dataref("sim/cockpit2/controls/left_brake_ratio", limit(brake_command + differential, 0.0, 1.0))
                self.api.set_dataref("sim/cockpit2/controls/right_brake_ratio", limit(brake_command - differential, 0.0, 1.0))
                if float(control["groundspeed_kt"]) <= 3.0 and since_touchdown >= 2.0:
                    stopped = True
                    if row is None:
                        trace.append(self._sample(elapsed, "rollout"))
                    break
            time.sleep(0.075)

        touchdown_summary = self._touchdown_summary(touchdown, last_airborne)
        airborne = [row for row in trace if row["stage"] != "rollout"]
        after_idle = []
        if guidance.power_off_command:
            after_idle = [
                row
                for row in airborne
                if float(row["elapsed_s"])
                >= float(guidance.power_off_command["elapsed_s"]) + 0.5
            ]
        heading_change = 0.0
        if guidance.power_off_command and airborne:
            start_heading = float(guidance.power_off_command["heading_true_deg"])
            heading_change = max(
                (
                    cfg.pattern_turn_direction
                    * wrap_180(float(row["heading_true_deg"]) - start_heading)
                    for row in after_idle
                ),
                default=0.0,
            )
        flare_rows = [row for row in control_trace if bool(row["flare_active"])]
        pitch_rates = []
        for previous, current in zip(flare_rows, flare_rows[1:]):
            sample_dt = float(current["elapsed_s"]) - float(previous["elapsed_s"])
            if sample_dt > 0.0:
                pitch_rates.append(
                    abs(float(current["pitch_deg"]) - float(previous["pitch_deg"]))
                    / sample_dt
                )
        result: dict[str, Any] = {
            "schema_version": 1,
            "generated_utc": utc_now().isoformat(),
            "accepted": False,
            "rejection_reasons": [],
            "orchestration": {"implementation": "flight_test.power_off_180", "language": "Python", "python_version": sys.version.split()[0], "controller_boundary": "C++ attitude inner loop; Python maneuver outer loop; no planepath override after release"},
            "standard": {"source": "FAA-S-ACS-7B CA.IV.M", "specified_touchdown_point_ft": cfg.selected_touchdown_ft, "acceptable_touchdown_window_ft": [cfg.selected_touchdown_ft, cfg.selected_touchdown_ft + 200.0], "maximum_touchdown_kias": cfg.maximum_touchdown_kias, "minimum_touchdown_pitch_deg": cfg.minimum_touchdown_pitch_deg, "maximum_touchdown_sink_fpm": cfg.maximum_touchdown_sink_fpm},
            "configuration": {"aircraft_path": cfg.aircraft_path, "airport": self.runway.airport, "runway": self.runway.runway, "runway_heading_true_deg": self.runway.heading_true_deg, "traffic_pattern": "right" if cfg.pattern_turn_direction > 0 else "left", "weather": "ISA, zero wind, dry runway", "mass_kg": loaded_mass, "mass_lb": loaded_mass * 2.20462262185, "mass_setup": self.mass_setup, "engine_setup": self.engine_setup, "controller": "v1.9 ArduPilot-derived attitude controller"},
            "command": asdict(cfg),
            "evidence_capture": {"video_requested": self.evidence.record_video, "video_start_utc": self.video_start_utc.isoformat() if self.video_start_utc else None, "video_stop_utc": self.video_stop_utc.isoformat() if self.video_stop_utc else None, "audio_requested": self.evidence.record_audio, "audio_capture_failure": self.audio_capture_failure, "audio_files_verified": None, "sound": self.sound_verification},
            "energy_management": {"power_off_command": guidance.power_off_command, "idle_established": guidance.idle_established, "maximum_throttle_after_power_off": max((float(row["throttle_ratio"]) for row in after_idle), default=None)},
            "maneuver": {"downwind_heading_true_deg": guidance.downwind_heading, "phase_events": guidance.phase_events, "heading_change_deg": heading_change, "maximum_absolute_bank_deg": max((abs(float(row["bank_deg"])) for row in after_idle), default=None), "maximum_absolute_beta_deg": max((abs(float(row["beta_deg"])) for row in after_idle), default=None), "ias_kias": metric_range(after_idle, "ias_kias")},
            "flare_quality": {"sample_count": len(flare_rows), "maximum_target_pitch_rate_deg_s": cfg.flare_pitch_rate_deg_s, "maximum_actual_pitch_rate_deg_s": max(pitch_rates, default=None), "maximum_normal_g": max((float(row["normal_g"]) for row in flare_rows), default=None), "minimum_normal_g": min((float(row["normal_g"]) for row in flare_rows), default=None)},
            "threshold_crossing": threshold_crossing,
            "touchdown": touchdown_summary,
            "rollout": {"stopped": stopped, "final_groundspeed_kt": float(trace[-1]["groundspeed_kt"]) if trace else None},
            "gear_layout": gear_layout,
            "control_trace": control_trace,
            "trace": trace,
        }
        result["rejection_reasons"] = evaluate_power_off_180(result)
        result["accepted"] = not result["rejection_reasons"]
        result["quality_advisories"] = evaluate_quality(result)
        result["quality_accepted"] = not result["quality_advisories"]
        return result


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="SR20 G6 power-off 180 accuracy-landing harness")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--port", type=int, default=8143)
    for field in PowerOff180Config.__dataclass_fields__.values():
        if isinstance(field.default, str):
            value_type = str
        elif isinstance(field.default, int):
            value_type = int
        else:
            value_type = float
        parser.add_argument(
            "--" + field.name.replace("_", "-"),
            dest=field.name,
            type=value_type,
            default=field.default,
        )
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
    parser.add_argument("--audio-sample-rate", type=int, default=48000)
    return parser


def main(argv: Iterable[str] | None = None) -> int:
    args = build_parser().parse_args(list(argv) if argv is not None else None)
    config = PowerOff180Config(**{name: getattr(args, name) for name in PowerOff180Config.__dataclass_fields__})
    try:
        with XPlaneApi(port=args.port) as api:
            result = PowerOff180Runner(api, config, args.output, evidence_from_args(args)).run()
    except Exception as error:
        result = {"schema_version": 1, "generated_utc": utc_now().isoformat(), "accepted": False, "rejection_reasons": ["setup or runtime failure"], "diagnostic": {"exception_type": type(error).__name__, "message": str(error), "orchestrator": "flight_test.power_off_180"}}
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(result, indent=2), encoding="utf-8")
    print(json.dumps({"output": str(args.output.resolve()), "accepted": result["accepted"], "rejection_reasons": result["rejection_reasons"]}))
    return 0 if result["accepted"] else 2


if __name__ == "__main__":
    raise SystemExit(main())

