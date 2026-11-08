"""Aircraft profile integration tests using only the independent synthetic plugin."""
import argparse
import copy
import json
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PROFILES = ROOT.parents[1] / "crates/xphost-core/profiles"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path, default=ROOT / "build/profile-results")
    args = parser.parse_args()
    args.out = args.out.resolve()
    args.out.mkdir(parents=True, exist_ok=True)
    checks = []

    def check(name, ok):
        checks.append({"name": name, "passed": bool(ok)})
        print(f"{'PASS' if ok else 'FAIL'} {name}", flush=True)
        if not ok:
            raise AssertionError(name)

    def run(name, extra, script="", expected=0):
        script_path = args.out / (name + ".xps")
        script_path.write_text(script, encoding="utf-8")
        folder = args.out / name
        command = [str(ROOT / "build/xphost.exe"), "--pkg", str(ROOT / "build/fixture"),
                   "--acf", "fixture.acf", "--script", str(script_path), "--out", str(folder), *extra]
        result = subprocess.run(command, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                                timeout=20, creationflags=subprocess.CREATE_NO_WINDOW)
        (args.out / (name + ".log")).write_bytes(result.stdout)
        check(name + " exit", result.returncode == expected)
        return folder, result.stdout.decode("utf-8", errors="replace")

    try:
        folder, output = run("generic", [], "expect sim/flightmodel/position/local_y 0 0\nget sim/flightmodel/weight/m_total\n")
        check("generic leaves aircraft mass unspecified", "sim/flightmodel/weight/m_total = (absent)" in output)
        check("generic is the default profile", json.loads((folder / "aircraft-profile.json").read_text())["name"] == "generic")
        for state, key in (("cold", 0), ("running", 3)):
            folder, _ = run("c182-" + state, ["--profile", "c182", "--flight-start", state],
                            "expect sim/flightmodel/weight/m_total 1200 1200\n"
                            "expect sim/flightmodel/position/local_y 1 1\n"
                            "expect xphost/test/profile_engines_at_start 1 1\n"
                            f"expect sim/cockpit2/engine/actuators/ignition_key[0] {key} {key}\n")
            check("c182 profile snapshot " + state, json.loads((folder / "aircraft-profile.json").read_text())["name"] == "c182")

        twin = json.loads((PROFILES / "c182.json").read_text())
        twin.update(name="synthetic-twin", description="Synthetic profile, no real aircraft data",
                    parked_height_m=2.5, mass_kg=3456, tail_number="NTEST", engine_count=2,
                    avionics={"bus_index": 1, "nominal_volts": 12})
        twin["engine_indicators"] = [dict(source="sim/test/engine_source", destination="sim/test/engine_display", scale=2)]
        twin["datarefs"] = {
            "sim/test/engine_source": {"type": "float_array", "value": [100, 200]},
            "sim/test/engine_display": {"type": "float_array", "value": [0, 0]},
            "sim/cockpit2/electrical/bus_volts": {"type": "float_array", "value": [0, 6]},
            "sim/cockpit2/engine/actuators/ignition_key": {"type": "int_array", "value": [0, 0]},
            "sim/test/label": {"type": "bytes", "value": "synthetic"},
        }
        twin["cold"]["sim/cockpit2/engine/actuators/ignition_key"]["value"] = [0, 0]
        twin["running"]["sim/cockpit2/engine/actuators/ignition_key"]["value"] = [3, 3]
        profile_path = args.out / "synthetic twin.json"
        profile_path.write_text(json.dumps(twin), encoding="utf-8")
        folder, output = run("custom-twin", ["--profile", str(profile_path), "--flight-start", "running", "--elev", "100"],
                            "expect sim/flightmodel/weight/m_total 3456 3456\n"
                            "expect sim/flightmodel/position/local_y 2.5 2.5\n"
                            "expect sim/flightmodel/position/elevation 102.5 102.5\n"
                            "expect xphost/test/profile_engines_at_start 2 2\n"
                            "expect xphost/test/bus_ratio 0.5 0.5\n"
                            "expect sim/cockpit2/engine/actuators/ignition_key[0] 3 3\n"
                            "expect sim/cockpit2/engine/actuators/ignition_key[1] 3 3\n"
                            "get sim/aircraft/view/acf_tailnum\nget sim/test/label\nwait 0.05\n"
                            "expect sim/test/engine_display[0] 200 200\n"
                            "expect sim/test/engine_display[1] 400 400\n")
        check("custom profile persisted exactly", json.loads((folder / "aircraft-profile.json").read_text()) == twin)
        check("profile strings are byte datarefs", '"NTEST"' in output and '"synthetic"' in output)

        invalid = [dict(schema_version=99), dict(engine_count=17), dict(parked_height_m=-1),
                   dict(avionics={"bus_index": 0, "nominal_volts": 0}), dict(mass_kg=-1), dict(unknown_field=True),
                   dict(datarefs={"plugin/owned": {"type": "int", "value": 1}})]
        for i, change in enumerate(invalid):
            profile = copy.deepcopy(twin)
            profile.update(change)
            path = args.out / f"invalid-{i}.json"
            path.write_text(json.dumps(profile), encoding="utf-8")
            folder, output = run("invalid-profile-" + str(i), ["--profile", str(path)], expected=1)
            check("invalid profile rejected before plugin start " + str(i), "XPluginStart" not in output and not (folder / "Log.txt").exists())
        run("missing-profile", ["--profile", str(args.out / "absent.json")], expected=1)
        catalog = args.out / "conflicting-datarefs.txt"
        catalog.write_text("sim/test/engine_source\tint[2]\ty\n")
        _, output = run("catalog-type-conflict", ["--profile", str(profile_path), "--datarefs", str(catalog)], expected=1)
        check("catalog mismatch rejected before loading plugin", "XPluginStart" not in output)
    finally:
        (args.out / "results.json").write_text(json.dumps({"passed": bool(checks) and all(c["passed"] for c in checks), "checks": checks}, indent=2) + "\n")


if __name__ == "__main__":
    main()
