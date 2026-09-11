"""Plot the retained native landing measurements; run with the harness venv."""
import json
from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
CASES = ["calm", "cross_left10", "head15"]
SERIES = [
    (
        "12.4.3 r2 · earlier native validation",
        REPO / "docs/poweroff180/verification/standard-units-20260911",
        "#386b91",
        "o",
        0.21,
    ),
    ("12.4.4 b1 · recording on", HERE / "recorded-matrix", "#bd5132", "x", 0.0),
    ("12.4.4 b1 · recording off", HERE / "no-recording", "#74549a", "D", -0.21),
]


def main():
    fig, ax = plt.subplots(figsize=(11, 4.8), layout="constrained")
    ax.axvspan(1000, 1200, color="#dceede", label="Accepted touchdown interval")
    measurements = []
    for label, directory, color, marker, offset in SERIES:
        first = True
        for index, case in enumerate(CASES):
            for repeat, path in enumerate(sorted((directory / "cards").glob(case + "-*/assessment.json"))):
                result = json.loads(path.read_text())
                assert result["measurement_valid"], path
                value = result["metrics"]["touchdown_ft"]
                y = index + offset + (repeat - 0.5) * 0.075
                ax.scatter(value, y, s=62, color=color, marker=marker,
                           linewidths=1.8, zorder=3, label=label if first else None)
                first = False
                measurements.append({"series": label, "card": path.parent.name,
                                     "touchdown_ft": value, "passed": result["passed"]})
    assert len(measurements) == 14, "Expected six earlier, six recorded, and two unrecorded flights"
    ax.set_yticks(range(3), ["Calm", "Left crosswind · 10 kt", "Headwind · 15 kt"])
    ax.set_xlim(min(500, min(row["touchdown_ft"] for row in measurements) - 40), 1250)
    ax.set_ylim(-0.5, 2.55)
    ax.invert_yaxis()
    ax.set_xlabel("Touchdown distance past the usable runway threshold (ft)")
    ax.set_title("SR20 precision landing checks at KCDW runway 22", loc="left", pad=20, fontsize=16)
    ax.grid(axis="x", alpha=0.18)
    ax.spines[["top", "right", "left"]].set_visible(False)
    ax.tick_params(axis="y", length=0)
    ax.legend(loc="upper center", bbox_to_anchor=(0.5, -0.17), ncol=2,
              frameon=False, fontsize=9)
    fig.savefig(HERE / "touchdown-comparison.png", dpi=160)
    fig.savefig(HERE / "touchdown-comparison.svg")
    (HERE / "touchdown-comparison.json").write_text(json.dumps(measurements, indent=2) + "\n")
    plt.close(fig)


if __name__ == "__main__":
    main()
