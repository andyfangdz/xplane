"""Reject invalid reference captures; descriptive statistics are not parity proof.

The observer reads native simulator variables. Before accepting any variable as
an A2A measurement, cross-check it with the aircraft's own displayed/model state.
This tool never promotes a capture to an accepted fidelity or performance test.
"""
from __future__ import annotations

import argparse
import json
import math
from pathlib import Path
from statistics import fmean


def audit(records: list[dict], expected_title: str) -> dict:
    errors: list[str] = []
    if not records or records[0].get("type") != "metadata" or records[0].get("schema") != 1:
        errors.append("missing supported metadata header")
    if sum(r.get("type") == "metadata" for r in records) != 1:
        errors.append("expected exactly one metadata header")
    endings = [r for r in records if r.get("type") == "end"]
    if len(endings) != 1 or not records or records[-1].get("type") != "end":
        errors.append("capture did not terminate cleanly")
    samples = [r for r in records if r.get("type") == "sample"]
    if not samples:
        errors.append("no samples")
    if len(samples) < 15:
        errors.append("fewer than 15 samples")
    if any(r.get("type") == "exception" for r in records):
        errors.append("SimConnect reported exceptions")
    if endings:
        if endings[-1].get("samples") != len(samples):
            errors.append("sample count does not match footer")
        if endings[-1].get("errors") != 0 or endings[-1].get("simulator_quit") is not False:
            errors.append("observer reported errors or simulator exit")
    servers = [r for r in records if r.get("type") == "server"]
    if len(servers) != 1 or servers[0].get("major") != 5 or "Prepar3D" not in servers[0].get("name", ""):
        errors.append("server identity is not a verified Prepar3D v5 instance")

    required = ("wall_elapsed_s", "sim_time_s", "sim_rate", "paused", "running",
                "on_ground", "weight_lb", "oat_c", "rpm", "map_inhg", "fuel_flow_gph")
    bad_fields = set()
    for row in samples:
        for key in required:
            value = row.get(key)
            if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(value):
                bad_fields.add(key)
        for key, value in row.items():
            if value is None or isinstance(value, float) and not math.isfinite(value):
                bad_fields.add(key)
    if bad_fields:
        errors.append("missing/non-finite numeric fields: " + ", ".join(sorted(bad_fields)))
    if any(r.get("title") != expected_title for r in samples):
        errors.append("aircraft title missing, changed, or does not exactly match expected title")
    if any(r.get("paused") != 0 for r in samples):
        errors.append("pause state was paused or unknown")
    if any(r.get("running") != 1 for r in samples):
        errors.append("simulation running state was stopped or unknown")
    if any(r.get("sim_rate") != 1 for r in samples):
        errors.append("simulation rate was not continuously 1x")
    if not bad_fields and samples:
        for key in ("wall_elapsed_s", "sim_time_s"):
            if any(b[key] <= a[key] for a, b in zip(samples, samples[1:])):
                errors.append(f"{key} did not increase strictly")
        if samples[-1]["sim_time_s"] - samples[0]["sim_time_s"] < 10:
            errors.append("less than 10 seconds of simulation time")
    statistics = {}
    keys = sorted(set.intersection(*(set(r) for r in samples))) if samples else []
    for key in keys:
        values = [r[key] for r in samples]
        if all(type(v) in (int, float) and math.isfinite(v) for v in values):
            statistics[key] = {"mean": fmean(values), "min": min(values), "max": max(values)}
    return {
        "schema": 1,
        "capture_usable_for_review": not errors,
        "accepted_baseline": False,
        "fidelity_matched": False,
        "rejection_reasons": errors,
        "sample_count": len(samples),
        "statistics": statistics,
        "remaining_requirements": [
            "cross-check native variables against aircraft gauges and private model states",
            "verify source version/hash, Accu-Sim enabled, damage/maintenance state, weather, mass and CG",
            "define and satisfy a controlled test card including convergence and stabilization",
            "collect repeated reference runs and compare with the X-Plane implementation",
        ],
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("capture", type=Path)
    parser.add_argument("--expected-title", required=True)
    args = parser.parse_args()
    def invalid_constant(value: str) -> None:
        raise ValueError(f"Non-standard JSON numeric value: {value}")
    records = [json.loads(line, parse_constant=invalid_constant)
               for line in args.capture.read_text(encoding="utf-8").splitlines() if line.strip()]
    if any(not isinstance(record, dict) for record in records):
        raise ValueError("Each JSONL record must be an object")
    report = audit(records, args.expected_title)
    print(json.dumps(report, indent=2, allow_nan=False))
    raise SystemExit(0 if report["capture_usable_for_review"] else 1)


if __name__ == "__main__":
    main()
