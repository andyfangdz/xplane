"""Reproduce the timing diagnosis and charts from retained native flight traces.

Run with tools/flight-test-harness/.venv/Scripts/python.exe. Historical traces
lack authority readbacks, so their neutral-axis events are labeled observations,
not reconstructed release-reason measurements.
"""
import csv
import gzip
import json
from pathlib import Path

import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
SERIES = [
    ("12.4.3 · previous validation", REPO / "docs/poweroff180/verification/standard-units-20260911", "#386b91"),
    ("12.4.4 · before fix, recording", REPO / "docs/compatibility/xplane-12.4.4-b1/recorded-matrix", "#bd5132"),
    ("12.4.4 · before fix, no recording", REPO / "docs/compatibility/xplane-12.4.4-b1/no-recording", "#9c6847"),
    ("12.4.4 · timing fix only", HERE / "timing-only-matrix", "#947430"),
    ("12.4.4 · late-flare candidate 0.8.0", HERE / "float-candidate-1", "#8d637e"),
    ("12.4.4 · predictive candidate 0.8.1", HERE / "float-candidate-2", "#a06d9c"),
    ("12.4.4 · predictive candidate 0.8.2", HERE / "float-candidate-3", "#a06d9c"),
    ("12.4.4 · guidance 0.8.3 crosswind repeats", HERE / "crosswind-check", "#29805c"),
    ("12.4.4 · guidance 0.8.3 wind matrix", HERE / "wind-matrix", "#29805c"),
    ("12.4.4 · guidance 0.8.3 recording", HERE / "recording-check", "#29805c"),
]


def load(card):
    with gzip.open(card / "trace.csv.gz", "rt") as stream:
        return [{k: float(v) for k, v in row.items()} for row in csv.DictReader(stream)]


def summarize(label, card, rows):
    airborne = [r for r in rows if r["phase_id"] < 8]
    slow = [r for r in airborne if r["control_dt_s"] > 0.05]
    neutral = [r for r in slow if r["elevator_input"] == 0 and r["aileron_input"] == 0]
    assessment = json.loads((card / "assessment.json").read_text())
    first_base = next(r for r in rows if r["phase_id"] == 5)
    roundout = next(r for r in rows if r["roundout_sim_time"] >= 0)
    actual = None
    if "attitude_active" in rows[0]:
        actual = {
            "inactive_frames": sum(r["attitude_active"] != 1 for r in rows),
            "release_frames": sum(r["attitude_release_reason"] != 0 for r in rows),
            "unowned_frames": sum(any(r["attitude_override_" + axis] != 1 for axis in ("roll", "pitch", "yaw")) for r in rows),
            "actual_frame_dt_max_s": max(r["attitude_frame_dt_s"] for r in rows),
            "actual_frames_above_50ms": sum(r["attitude_frame_dt_s"] > 0.05 for r in rows),
        }
    floating = [r for r in airborne if r.get("flare_float_active") == 1]
    return {
        "series": label, "card": card.name, "airborne_frames": len(airborne),
        "sim_steps_above_50ms": len(slow), "neutral_axes_on_slow_frames": len(neutral),
        "base_start_along_ft": first_base["runway_along_ft"],
        "roundout_start_along_ft": roundout["runway_along_ft"],
        "touchdown_ft": assessment["metrics"]["touchdown_ft"],
        "kias": assessment["metrics"]["kias"],
        "physical_sink_fpm": assessment["metrics"]["physical_sink_fpm"],
        "max_flare_pitch_deg": assessment["metrics"]["max_flare_pitch_deg"],
        "max_flare_pitch_rate_deg_s": assessment["metrics"]["max_flare_pitch_rate_deg_s"],
        "measurement_valid": assessment["measurement_valid"], "passed": assessment["passed"],
        "reasons": assessment["reasons"],
        "measured_authority": actual,
        "float_correction": None if "flare_float_active" not in rows[0] else {
            "airborne_active_frames": len(floating),
            "first": {k: floating[0][k] for k in ("sim_time", "runway_along_ft", "agl_ft", "ias_kias", "vertical_speed_mps")} if floating else None,
        },
    }


def main():
    records = []
    for label, directory, _ in SERIES:
        for card in sorted((directory / "cards").glob("*")):
            if (card / "trace.csv.gz").exists():
                records.append(summarize(label, card, load(card)))
    (HERE / "timing-diagnosis.json").write_text(json.dumps(records, indent=2) + "\n")

    # Directly observed turn-entry failure, with time relative to power cut.
    broken = load(SERIES[2][1] / "cards/calm-01")
    cut = next(r["cut_sim_time"] for r in broken if r["cut_sim_time"] >= 0)
    excerpt = [r for r in broken if 8.5 <= r["sim_time"] - cut <= 13.5]
    fig, axes = plt.subplots(3, 1, figsize=(10, 7), sharex=True, layout="constrained")
    x = [r["sim_time"] - cut for r in excerpt]
    for key, color, name in [("elevator_input", "#386b91", "Elevator"), ("aileron_input", "#29805c", "Aileron")]:
        axes[0].plot(x, [r[key] for r in excerpt], color=color, label=name)
    axes[0].set_ylabel("Control input ratio")
    axes[0].legend(frameon=False, ncol=2)
    axes[1].plot(x, [1000 * r["control_dt_s"] for r in excerpt], color="#bd5132")
    axes[1].axhline(50, color="#222222", linestyle="--", linewidth=1, label="Incorrect release limit: 50 ms")
    axes[1].set_ylabel("Simulator step (ms)")
    axes[1].legend(frameon=False)
    axes[2].plot(x, [r["bank_deg"] for r in excerpt], color="#386b91", label="Actual bank")
    axes[2].plot(x, [r["bank_command"] for r in excerpt], color="#29805c", linestyle="--", label="Commanded bank")
    axes[2].set_ylabel("Bank (degrees)")
    axes[2].set_xlabel("Seconds after the power cut")
    axes[2].legend(frameon=False, ncol=2)
    for ax in axes:
        ax.grid(alpha=0.16)
        ax.spines[["top", "right"]].set_visible(False)
    axes[0].set_title("50.25 ms frames repeatedly reset the controls during the base turn", loc="left", pad=15)
    fig.savefig(HERE / "control-reset.png", dpi=150)
    plt.close(fig)

    cases = ["calm", "head5", "head10", "head15", "tail5", "cross_left10", "cross_right10"]
    fig, ax = plt.subplots(figsize=(11, 6), layout="constrained")
    ax.axvspan(1000, 1200, color="#dceede", label="Unchanged acceptance interval")
    chart_groups = [
        ("12.4.3 · previous validation", [SERIES[0][0]], "#386b91", "o"),
        ("12.4.4 · before fixes", [s[0] for s in SERIES[1:3]], "#bd5132", "x"),
        ("12.4.4 · timing fix only", [SERIES[3][0]], "#947430", "D"),
        ("12.4.4 · flare candidates", [s[0] for s in SERIES[4:7]], "#8d637e", "v"),
        ("12.4.4 · timing + predictive flare", [s[0] for s in SERIES[7:]], "#29805c", "o"),
    ]
    for index, (label, sources, color, marker) in enumerate(chart_groups):
        points = [r for r in records if r["series"] in sources]
        y = [cases.index(r["card"].rsplit("-", 1)[0]) + (index - 2) * 0.12
             + (int(r["card"].rsplit("-", 1)[1]) - 1.5) * 0.035 for r in points]
        ax.scatter([r["touchdown_ft"] for r in points], y, color=color,
                   s=35, label=label, marker=marker, alpha=0.8)
    ax.set_yticks(range(len(cases)), ["Calm", "Headwind 5 kt", "Headwind 10 kt", "Headwind 15 kt", "Tailwind 5 kt", "Left crosswind 10 kt", "Right crosswind 10 kt"])
    ax.invert_yaxis()
    ax.set_xlabel("Touchdown distance past the usable runway threshold (ft)")
    ax.set_title("SR20 landing fixes across retained attempts", loc="left", pad=18)
    ax.spines[["top", "right", "left"]].set_visible(False)
    ax.grid(axis="x", alpha=0.16)
    ax.legend(loc="upper center", bbox_to_anchor=(0.5, -0.12), ncol=2, frameon=False, fontsize=9)
    fig.savefig(HERE / "touchdown-comparison.png", dpi=150)
    plt.close(fig)

    left = [("v7 · passing repeat", HERE / "timing-only-matrix/cards/cross_left10-01", "#386b91"),
            ("v7 · long landing", HERE / "timing-only-matrix/cards/cross_left10-02", "#bd5132")]
    left.extend(("0.8.3 · crosswind check " + str(i), HERE / f"crosswind-check/cards/cross_left10-{i:02}", color)
                for i, color in enumerate(("#175737", "#347b50", "#51a16a", "#79ae72"), 1))
    left.extend(("0.8.3 · wind matrix " + str(i), HERE / f"wind-matrix/cards/cross_left10-{i:02}", color)
                for i, color in enumerate(("#2c7775", "#58a6a2"), 1))
    if all((card / "trace.csv.gz").exists() for _, card, _ in left):
        fig, axes = plt.subplots(3, 1, figsize=(10, 8), sharex=True, layout="constrained")
        for label, card, color in left:
            rows = [r for r in load(card) if r["phase_id"] in (7, 8)
                    and r["roundout_sim_time"] >= 0 and r["runway_along_ft"] >= 850]
            contact = next(i for i, r in enumerate(rows) if r["contact_latched"] == 1)
            rows = rows[:contact + 1]
            along = [r["runway_along_ft"] for r in rows]
            axes[0].plot(along, [r["agl_ft"] for r in rows], color=color, label=label)
            axes[1].plot(along, [-r["vertical_speed_mps"] * 60 / .3048 for r in rows], color=color)
            axes[2].plot(along, [r["pitch_deg"] for r in rows], color=color)
            for ax, value in zip(axes, (rows[-1]["agl_ft"], -rows[-1]["vertical_speed_mps"] * 60 / .3048, rows[-1]["pitch_deg"])):
                ax.scatter(along[-1], value, color=color, s=22, zorder=4)
        for ax in axes:
            ax.axvspan(1000, 1200, color="#dceede", alpha=.45)
            ax.axvline(1000, color="#555555", linestyle=":", linewidth=1)
            ax.grid(alpha=.16)
            ax.spines[["top", "right"]].set_visible(False)
        axes[0].set_ylim(0, 5)
        axes[0].set_ylabel("Height AGL (ft)")
        axes[0].set_title("Left crosswind: the long v7 float and all six final 0.8.3 flights", loc="left", pad=15)
        axes[0].legend(frameon=False, ncol=2, fontsize=9)
        axes[1].set_ylabel("Physical descent (fpm)")
        axes[2].set_ylabel("Pitch (degrees)")
        axes[2].set_xlabel("Distance past the usable runway threshold (ft); dots mark first contact")
        axes[2].set_xlim(850, 1260)
        fig.savefig(HERE / "late-flare-comparison.png", dpi=150)
        plt.close(fig)
    print(json.dumps({label: {"flights": len(group), "passes": sum(r["passed"] for r in group)}
                      for label, _, _ in SERIES
                      if (group := [r for r in records if r["series"] == label])}, indent=2))


if __name__ == "__main__":
    main()
