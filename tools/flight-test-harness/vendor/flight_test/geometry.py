"""Runway-relative geometry used by dynamic maneuver harnesses."""

from __future__ import annotations

from dataclasses import dataclass, field
import math

FEET_PER_NM = 6076.12
FEET_PER_METER = 3.280839895


@dataclass(frozen=True, slots=True)
class RunwayGeometry:
    """A locally planar runway frame with the landing threshold as origin."""

    airport: str
    runway: str
    threshold_latitude: float
    threshold_longitude: float
    opposite_latitude: float
    opposite_longitude: float
    elevation_ft: float
    displaced_threshold_ft: float = 0.0
    physical_end_latitude: float | None = None
    physical_end_longitude: float | None = None
    latitude_scale_ft: float = field(init=False)
    longitude_scale_ft: float = field(init=False)
    unit_north: float = field(init=False)
    unit_east: float = field(init=False)
    length_ft: float = field(init=False)
    heading_true_deg: float = field(init=False)

    def __post_init__(self) -> None:
        latitude_scale = 60.0 * FEET_PER_NM
        mean_latitude = (self.threshold_latitude + self.opposite_latitude) / 2.0
        longitude_scale = latitude_scale * math.cos(math.radians(mean_latitude))
        north = (self.opposite_latitude - self.threshold_latitude) * latitude_scale
        east = (self.opposite_longitude - self.threshold_longitude) * longitude_scale
        length = math.hypot(north, east)
        heading = math.degrees(math.atan2(east, north)) % 360.0
        object.__setattr__(self, "latitude_scale_ft", latitude_scale)
        object.__setattr__(self, "longitude_scale_ft", longitude_scale)
        object.__setattr__(self, "unit_north", north / length)
        object.__setattr__(self, "unit_east", east / length)
        object.__setattr__(self, "length_ft", length)
        object.__setattr__(self, "heading_true_deg", heading)

    def point(self, along_ft: float, cross_ft: float = 0.0) -> tuple[float, float]:
        """Return latitude/longitude at runway-relative along/cross coordinates."""

        right_east = self.unit_north
        right_north = -self.unit_east
        north = along_ft * self.unit_north + cross_ft * right_north
        east = along_ft * self.unit_east + cross_ft * right_east
        return (
            self.threshold_latitude + north / self.latitude_scale_ft,
            self.threshold_longitude + east / self.longitude_scale_ft,
        )

    def position(self, latitude: float, longitude: float) -> tuple[float, float]:
        """Return runway-relative along/cross coordinates in feet."""

        north = (latitude - self.threshold_latitude) * self.latitude_scale_ft
        east = (longitude - self.threshold_longitude) * self.longitude_scale_ft
        along = north * self.unit_north + east * self.unit_east
        cross = east * self.unit_north - north * self.unit_east
        return along, cross


KCDW_RUNWAY_22 = RunwayGeometry(
    airport="KCDW",
    runway="22",
    threshold_latitude=40.878303013554174,
    threshold_longitude=-74.27844499363563,
    opposite_latitude=40.8677611,
    opposite_longitude=-74.2863376,
    elevation_ft=171.0,
    displaced_threshold_ft=41.0 * FEET_PER_METER,
    physical_end_latitude=40.8786241,
    physical_end_longitude=-74.2782046,
)


def limit(value: float, low: float, high: float) -> float:
    return max(low, min(high, value))


def wrap_180(value: float) -> float:
    return (value + 180.0) % 360.0 - 180.0


def quaternion(heading_deg: float, pitch_deg: float, bank_deg: float) -> list[float]:
    """Return X-Plane's world-attitude quaternion for Euler angles."""

    psi = math.radians(heading_deg) / 2.0
    theta = math.radians(pitch_deg) / 2.0
    phi = math.radians(bank_deg) / 2.0
    cp, sp = math.cos(psi), math.sin(psi)
    ct, st = math.cos(theta), math.sin(theta)
    cf, sf = math.cos(phi), math.sin(phi)
    return [
        cp * ct * cf + sp * st * sf,
        cp * ct * sf - sp * st * cf,
        cp * st * cf + sp * ct * sf,
        -cp * st * sf + sp * ct * cf,
    ]
