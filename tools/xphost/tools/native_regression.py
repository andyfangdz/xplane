"""Independent ABI, re-entry, scheduling and Windows SEH tests for Rust xphost."""
import argparse
import ctypes
import json
import os
from pathlib import Path
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path, default=ROOT / "build/native-results")
    args = parser.parse_args()
    args.out = args.out.resolve()
    args.out.mkdir(parents=True, exist_ok=True)
    host = ROOT / "build/xphost.exe"
    fixture = ROOT / "tests/fixture"
    common = [str(host), "--pkg", str(ROOT / "build/fixture"), "--acf", "fixture.acf", "--clock", "fast"]
    checks = []

    def check(name, ok, detail=None):
        checks.append({"name": name, "passed": bool(ok), "detail": detail})
        print(f"{'PASS' if ok else 'FAIL'} {name}: {detail}", flush=True)
        if not ok:
            raise AssertionError(name)

    def run(name, extra, expected=0, fault=None, invalid_panel=None):
        environment = os.environ.copy()
        environment.pop("XPHOST_FIXTURE_FAULT", None)
        environment.pop("XPHOST_FIXTURE_STALE_TEXTURE", None)
        environment.pop("XPHOST_FIXTURE_BAD_INDEX", None)
        if invalid_panel:
            environment[invalid_panel] = "1"
        if fault:
            environment["XPHOST_FIXTURE_FAULT"] = fault
        result = subprocess.run([*common, *extra, "--out", str(args.out / name)],
                                env=environment, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                                timeout=20, creationflags=subprocess.CREATE_NO_WINDOW)
        (args.out / f"{name}.log").write_bytes(result.stdout)
        check(name, result.returncode == expected, result.returncode)

    def panel_case(name, *, expected=0, fault=None, invalid_panel=None):
        system = args.out / name / "xplane"
        fonts = system / "Resources/fonts"
        fonts.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(Path(os.environ["WINDIR"]) / "Fonts/segoeui.ttf", fonts / "fixture.ttf")
        (system / "fixture.obj").write_text("I\n800\nOBJ\nTEXTURE\nPOINT_COUNTS 0 0 0 0\n")
        script = args.out / f"{name}.xps"
        script.write_text("cmd xphost/test/open_panel\nexpect xphost/test/contract_failures 0 0\n"
                          "draw first.json\ndraw second.json\ncmd xphost/test/open_panel\ndraw third.json\n")
        run(name, ["--script", str(script)], expected=expected, fault=fault, invalid_panel=invalid_panel)

    try:
        library = ctypes.WinDLL(str(ROOT / "build/XPLM_64.dll"))
        required = (ROOT / "tests/sdk-exports.txt").read_text().splitlines()
        missing = [name for name in required if not hasattr(library, name)]
        check("all original SDK exports", not missing, missing or len(required))
        run("contracts", ["--script", str(fixture / "contracts.xps"), "--datarefs", str(fixture / "datarefs.txt"),
                          "--navdat", str(fixture / "nav.dat"), "--lat", "40.8", "--lon", "-74.2", "--elev", "100"])
        panel_case("panel")
        first, second, third = [json.loads((args.out / "panel" / f"{frame}.json").read_text())
                                for frame in ("first", "second", "third")]
        check("no legacy context required", "GL context off" in (args.out / "panel.log").read_text())
        check("native callback eligibility", [[s.get("phase", "window") for s in f["surfaces"]]
              for f in (first, second, third)] == [
                  ["screen", "before", "after", "window"], ["before", "after"],
                  ["screen", "before", "after", "window"]])
        commands = first["surfaces"][-1]["commands"]
        check("recorded window operations", [c["op"] for c in commands] == ["polygon", "mesh", "text", "atlas_in"])
        check("blackout geometry", commands[0]["args"]["points"] == [[30,40],[42,40],[42,50],[30,50]]
              and commands[0]["args"]["color"] == 0xff010101)
        mesh = commands[1]["args"]
        check("indexed texture and clip", mesh["scissor"] == [12,38,18,46] and mesh["bounds"] == [12,40,20,46]
              and mesh["uv_bounds"] == [0,0,1,1] and mesh["texture"]["first_rgba"] == [12,34,56,255]
              and mesh["first_vertex"] == {"xy":[12,46], "uv":[0,0], "rgba":[16,32,48,255]})
        check("wrapped text and atlas slice", commands[2]["args"]["text"] == "Cockpit"
              and commands[2]["args"]["wrap"] == 80 and commands[2]["args"]["advance"] > 0
              and commands[3]["args"]["image"]["cell"] == [1,0])
        check("world instance after unload", len(first["instances"]) == 1 and
              first["instances"][0]["pose"] == [1,2,3,4,5,6] and
              first["instances"][0]["values"] == [0.75] and first["instances"][0]["datarefs"] == ["xphost/test/value"])
        panel_case("bad-texture", expected=3, invalid_panel="XPHOST_FIXTURE_STALE_TEXTURE")
        check("stale texture rejected", "mesh: unknown texture" in json.loads(
            (args.out / "bad-texture/first.json").read_text())["errors"])
        panel_case("bad-index", expected=3, invalid_panel="XPHOST_FIXTURE_BAD_INDEX")
        check("base vertex bounds enforced", "mesh: vertex index outside buffer" in json.loads(
            (args.out / "bad-index/first.json").read_text())["errors"])
        panel_case("draw-fault", expected=2, fault="draw")
        # Each injected native exception must become documented exit code 2,
        # including callback faults that used to be trapped inconsistently.
        for stage in ("start", "enable", "message", "loop", "read", "write", "command", "disable", "stop"):
            script = args.out / f"fault-{stage}.xps"
            script.write_text("get xphost/test/value\nset xphost/test/value 3\ncmd xphost/test/pulse\nwait 0.05\n")
            run("fault-" + stage, ["--script", str(script)], expected=2, fault=stage)
        malformed = args.out / "malformed.xps"
        malformed.write_text("wait NaN\nset xphost/test/value nope\nexpect xphost/test/value 1.25 1.25\n")
        run("invalid-script-numbers", ["--script", str(malformed)], expected=3)
        run("recovery-after-faults", ["--seconds", "0.1"])
    finally:
        (args.out / "results.json").write_text(json.dumps({"passed": bool(checks) and all(c["passed"] for c in checks), "checks": checks}, indent=2) + "\n")


if __name__ == "__main__":
    main()
