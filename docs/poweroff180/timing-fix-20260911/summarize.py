"""Summarize the final guidance build separately from development candidates."""
import csv
import gzip
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
STAGES = ("crosswind-check", "wind-matrix", "recording-check")
METRICS = ("touchdown_ft", "kias", "physical_sink_fpm", "max_flare_pitch_deg",
           "max_flare_pitch_rate_deg_s", "short_final_max_cross_ft")
AUTHORITY = {"attitude_armed": 1, "attitude_active": 1, "attitude_release_reason": 0,
             "attitude_override_roll": 1, "attitude_override_pitch": 1,
             "attitude_override_yaw": 1}


def main():
    cards = []
    for stage in STAGES:
        assert (HERE / stage / "restoration.json").exists(), stage
        for card in sorted((HERE / stage / "cards").iterdir()):
            result = json.loads((card / "assessment.json").read_text())
            with gzip.open(card / "trace.csv.gz", "rt") as stream:
                rows = [{k: float(v) for k, v in row.items()} for row in csv.DictReader(stream)]
            cards.append({"stage": stage, "card": card.name, "passed": result["passed"],
                          "metrics": {k: result["metrics"][k] for k in METRICS},
                          "frames": len(rows),
                          "slow_frames_over_50_ms": sum(r["attitude_frame_dt_s"] > .05 for r in rows),
                          "unhealthy_authority_frames": sum(any(r.get(k) != v for k, v in AUTHORITY.items()) for r in rows),
                          "float_correction_used": any(r["flare_float_active"] == 1 for r in rows)})
    assert len(cards) == 20
    ranges = {k: {"minimum": min(c["metrics"][k] for c in cards),
                  "maximum": max(c["metrics"][k] for c in cards)} for k in METRICS}
    output = {"flights": len(cards), "passes": sum(c["passed"] for c in cards),
              "ranges": ranges,
              "native_frames": sum(c["frames"] for c in cards),
              "slow_frames_over_50_ms": sum(c["slow_frames_over_50_ms"] for c in cards),
              "unhealthy_authority_frames": sum(c["unhealthy_authority_frames"] for c in cards),
              "float_correction_flights": sum(c["float_correction_used"] for c in cards),
              "cards": cards}
    (HERE / "final-results.json").write_text(json.dumps(output, indent=2) + "\n")
    print(json.dumps({k: v for k, v in output.items() if k != "cards"}, indent=2))


if __name__ == "__main__":
    main()
