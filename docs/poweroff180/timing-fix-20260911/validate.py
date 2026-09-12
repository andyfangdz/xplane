"""Reassess every retained attempt and verify evidence/source hashes.

Use the harness Python environment. --current-source additionally requires
this checkout and its built helpers to match the final wind-matrix manifest.
Historical stages keep their own manifests and are never pooled with release
pass counts. The authority-loss probe must fail the maneuver as intended.
"""
import argparse
import csv
import gzip
import hashlib
import json
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
sys.path.insert(0, str(REPO / "tools/flight-test-harness"))
from xpt.analysis import assess

STAGES = [
    ("timing-only-matrix", 14, 13),
    ("float-candidate-1", 4, 3),
    ("float-candidate-2", 4, 3),
    ("float-candidate-3", 4, 4),
    ("crosswind-check", 4, 4),
    ("wind-matrix", 14, 14),
    ("recording-check", 2, 2),
    ("authority-guard", 1, 0),
]


def read(path):
    return json.loads(path.read_text(encoding="utf-8-sig"))


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--current-source", action="store_true")
    args = parser.parse_args()
    result = {"stages": [], "current_source_verified": False}
    final_manifest = read(HERE / "wind-matrix/source-manifest.json")
    final_builds = {k: v for k, v in final_manifest["harness_sha256"].items() if k.startswith("build/")}
    assert len(final_builds) == 3
    offline = read(HERE / "final-offline-validation.json")
    for name, expected in offline["logs"].items():
        assert digest(HERE / name) == expected, name
    counts = re.findall(r"test result: ok\. (\d+) passed; 0 failed; (\d+) ignored",
                        (HERE / "final-offline-tests.log").read_text())
    assert sum(int(p) for p, _ in counts) == offline["rust_tests_passed"] == 86
    assert sum(int(i) for _, i in counts) == offline["ignored_rust_tests"] == 1
    assert "Ran 15 tests" in (HERE / "final-offline-tests.log").read_text()
    result["offline_validation"] = offline
    for name in ("setup-failure", "setup-menu-candidate"):
        failed_setup = HERE / name
        for relative, expected in read(failed_setup / "evidence-sha256.json").items():
            assert digest(failed_setup / relative) == expected, relative
        setup_summary = read(failed_setup / "summary.json")
        assert setup_summary["flights"] == 0 and setup_summary["restoration"]["restored"]
        assert any(e["error"] == "Could not set paused=False" for e in setup_summary["errors"])
    result["setup_failure_retained"] = True
    setup_crash = HERE / "recording-setup-crash"
    for relative, expected in read(setup_crash / "evidence-sha256.json").items():
        assert digest(setup_crash / relative) == expected, relative
    crash_summary = read(setup_crash / "summary.json")
    assert crash_summary["flights"] == 0 and crash_summary["restoration"]["restored"]
    assert read(setup_crash / "log-review.json")["crash_confirmed"]
    assert "This application has crashed!" in (setup_crash / "selected-simulator.log").read_text()
    result["recording_setup_crash_retained"] = True
    setup_timeout = HERE / "recording-setup-timeout"
    for relative, expected in read(setup_timeout / "evidence-sha256.json").items():
        assert digest(setup_timeout / relative) == expected, relative
    timeout_summary = read(setup_timeout / "summary.json")
    assert timeout_summary["flights"] == 0 and timeout_summary["restoration"]["restored"]
    assert any(e["error"] == "Simulator API did not become ready" for e in timeout_summary["errors"])
    result["recording_startup_timeout_retained"] = True
    for name, expected_count, expected_passes in STAGES:
        stage = HERE / name
        index = read(stage / "evidence-sha256.json")
        for relative, expected in index.items():
            path = (stage / relative).resolve()
            assert path.is_relative_to(stage.resolve()), relative
            assert digest(path) == expected, str(path)
        restore = read(stage / "restoration.json")
        for key in ("restored", "source_hashes_match", "helpers_removed", "exact_name_sets_verified"):
            assert restore[key], (name, key)
        assert restore["scenery_count"] == 179 and restore["scenery_links"] == 5
        if name in ("crosswind-check", "wind-matrix", "recording-check", "authority-guard"):
            stage_manifest = read(stage / "source-manifest.json")
            assert {k: v for k, v in stage_manifest["harness_sha256"].items() if k.startswith("build/")} == final_builds
            log_review = read(stage / "log-review.json")
            assert log_review["crash_markers_found"] == []
            native_log = (stage / "native-plugin.log").read_text()
            assert "v1.9.1" in native_log and "runtime 0.8.3" in native_log
        cards = []
        for card in sorted((stage / "cards").iterdir()):
            with gzip.open(card / "trace.csv.gz", "rt") as stream:
                rows = [{k: float(v) for k, v in r.items()} for r in csv.DictReader(stream)]
            document = read(card / "result.json")
            assessment = assess(document, rows)
            assert assessment == read(card / "assessment.json"), str(card)
            terminal = read(card / "native-terminal-safety.json")["readback"]
            assert terminal["sim/time/paused"] == 1
            assert terminal["sr20g6/test_controller/armed"] == 0
            assert terminal["sr20g6/test_controller/active"] == 0
            assert not any(terminal["sim/operation/override/override_planepath"])
            for axis in ("roll", "pitch", "heading"):
                assert terminal["sim/operation/override/override_joystick_" + axis] == 0
            if name in ("crosswind-check", "wind-matrix", "recording-check", "authority-guard"):
                setup = read(card / "setup-readback.json")
                assert setup["axis_controller_version_patch"] == 1
                assert setup["guidance_runtime_version_patch"] == 3
                assert setup["guidance_algorithm_version"] == 8
            if name in ("wind-matrix", "recording-check", "authority-guard"):
                resume = read(card / "startup-resume.json")
                assert resume["recovery_command"] == "sim/operation/toggle_main_menu"
                assert resume["duration_seconds"] == .2 and resume["recovery_attempts"] >= 0
                assert resume["paused_after"] == 0
            if name == "recording-check":
                gap = document["gap_probe"]
                assert gap["requested_wall_seconds"] == 3
                assert gap["after_steps"] > gap["before_steps"]
                assert gap["after_sim_time"] > gap["before_sim_time"]
                assert gap["after_reason"] == "none"
            cards.append({"card": card.name, "frames": len(rows), "passed": assessment["passed"],
                          "measurement_valid": assessment["measurement_valid"], "reasons": assessment["reasons"],
                          "metrics": assessment["metrics"]})
        assert len(cards) == expected_count, name
        assert sum(c["passed"] for c in cards) == expected_passes, name
        result["stages"].append({"stage": name, "indexed_files": len(index), "restoration_verified": True, "cards": cards})

    probe = read(HERE / "authority-guard/authority-loss-injection.json")
    assert probe["passed"]
    assert probe["observations"][-1]["phase"] == "aborted"
    assert probe["observations"][-1]["reason"] == "attitude_authority_lost"
    result["authority_loss_guard_verified"] = True
    recordings = read(HERE / "recording-check/recording-validation.json")
    assert len(recordings) == 2
    assert all(r["landing_passed"] and r["audio_covers_movie"] and r["sound_readback_passed"] and r["audio_endpoint_verified"] and r["raw_audio_full_decode_passed"]
               and all(m["full_decode_passed"] for m in r["movies"]) for r in recordings)
    visual = read(HERE / "recording-check/visual-validation.json")
    assert len(visual["cards"]) == 2
    assert all(c["event_sequence_consistent"] and c["hud_values_match_trace_rounding"] for c in visual["cards"])
    profile = read(HERE / "recording-check/recording-profile.json")
    assert len(profile["optional_global_plugins_isolated"]) == 18
    assert profile["optional_global_plugins_isolated"] == read(HERE / "recording-check/restoration.json")["isolated_plugins_restored"]
    result["recording_profile_verified"] = True
    result["final_native_binaries_identical"] = True

    if args.current_source:
        manifest = final_manifest
        for name, expected in manifest["harness_sha256"].items():
            if name.startswith("workspace/"):
                path = REPO / name.removeprefix("workspace/")
            else:
                path = REPO / "tools/flight-test-harness" / name
            assert digest(path) == expected, str(path)
        result["current_source_verified"] = True
        result["current_source_files"] = len(manifest["harness_sha256"])
    (HERE / "validation.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps({"stages": [{"stage": s["stage"], "flights": len(s["cards"]),
                                 "passes": sum(c["passed"] for c in s["cards"])} for s in result["stages"]],
                      "current_source_verified": result["current_source_verified"],
                      "authority_loss_guard_verified": True, "recording_profile_verified": True}, indent=2))


if __name__ == "__main__":
    main()
