"""Independent ABI, re-entry, scheduling and Windows SEH tests for Rust xphost."""
import argparse
import ctypes
import json
import os
from pathlib import Path
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

    def run(name, extra, expected=0, fault=None):
        environment = os.environ.copy()
        environment.pop("XPHOST_FIXTURE_FAULT", None)
        if fault:
            environment["XPHOST_FIXTURE_FAULT"] = fault
        result = subprocess.run([*common, *extra, "--out", str(args.out / name)],
                                env=environment, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                                timeout=20, creationflags=subprocess.CREATE_NO_WINDOW)
        (args.out / f"{name}.log").write_bytes(result.stdout)
        check(name, result.returncode == expected, result.returncode)

    try:
        library = ctypes.WinDLL(str(ROOT / "build/XPLM_64.dll"))
        required = (ROOT / "tests/sdk-exports.txt").read_text().splitlines()
        missing = [name for name in required if not hasattr(library, name)]
        check("all original SDK exports", not missing, missing or len(required))
        run("contracts", ["--script", str(fixture / "contracts.xps"), "--datarefs", str(fixture / "datarefs.txt"),
                          "--navdat", str(fixture / "nav.dat"), "--lat", "40.8", "--lon", "-74.2", "--elev", "100"])
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
