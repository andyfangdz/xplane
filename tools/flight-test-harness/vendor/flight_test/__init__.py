"""Reusable X-Plane flight-test orchestration.

This package is simulator-only and must not be used for real-world flight
planning or training decisions.
"""

from .geometry import KCDW_RUNWAY_22, RunwayGeometry
from .landing import LandingConfig, LandingGuidance, evaluate_result

__all__ = [
    "KCDW_RUNWAY_22",
    "LandingConfig",
    "LandingGuidance",
    "RunwayGeometry",
    "evaluate_result",
]
