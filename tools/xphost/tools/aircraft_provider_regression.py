"""Build and exercise the synthetic provider with the public aircraft harness."""

import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys


ROOT = Path(__file__).resolve().parents[1]
WORKSPACE = ROOT.parents[1]


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run(command, log, timeout):
    with log.open("wb") as output:
        return subprocess.run(
            command, cwd=WORKSPACE, stdout=output, stderr=subprocess.STDOUT,
            timeout=timeout, creationflags=subprocess.CREATE_NO_WINDOW,
        ).returncode


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", required=True, type=Path, help="fresh evidence directory")
    parser.add_argument("--cargo", default="cargo")
    args = parser.parse_args()
    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=False)
    result = {"passed": False, "output": str(out)}
    try:
        host = ROOT / "build/xphost.exe"
        sdk = ROOT / "build/XPLM_64.dll"
        if not host.is_file() or not sdk.is_file():
            raise ValueError("Build the staged host and fake SDK with tools/xphost/build.ps1 first.")
        cargo = shutil.which(args.cargo) or args.cargo
        rustup_cargo = Path.home() / ".cargo/bin/cargo.exe"
        if cargo == "cargo" and rustup_cargo.is_file():
            cargo = str(rustup_cargo)
        metadata = json.loads(subprocess.check_output(
            [cargo, "metadata", "--locked", "--no-deps", "--format-version", "1"],
            cwd=WORKSPACE, timeout=30, creationflags=subprocess.CREATE_NO_WINDOW,
        ))
        build = [cargo, "build", "--locked", "--release", "-p", "xphost", "--example", "aircraft_provider"]
        result["build_command"] = build
        result["build_exit_code"] = run(build, out / "build.log", 300)
        if result["build_exit_code"] != 0:
            raise ValueError("Synthetic provider build failed. Inspect build.log.")
        artifact = Path(metadata["target_directory"]) / "release/examples/aircraft_provider.dll"
        package = out / "package"
        plugin = package / "plugins/aircraft-provider/64/win.xpl"
        plugin.parent.mkdir(parents=True)
        shutil.copyfile(artifact, plugin)
        shutil.copyfile(ROOT / "tests/fixture/fixture.acf", package / "fixture.acf")
        script = ROOT / "tests/aircraft-provider/lifecycle.xps"
        command = [str(host), "--pkg", str(package), "--acf", "fixture.acf", "--xpl", str(plugin),
                   "--profile", "generic", "--clock", "fast", "--dt", "0.0166666666667",
                   "--script", str(script), "--out", str(out / "host")]
        result.update(plugin_sha256=sha256(plugin), xphost_sha256=sha256(host),
                      fake_sdk_sha256=sha256(sdk), command=command)
        result["host_exit_code"] = run(command, out / "host.log", 30)
        if result["host_exit_code"] != 0:
            raise ValueError("Synthetic provider scenario failed. Inspect host.log.")
        log = (out / "host/Log.txt").read_text(encoding="utf-8", errors="replace")
        expected = ["destroyed instance 1", "destroyed instance 2", "stopped cleanly"]
        markers = [f"aircraft-provider fixture: {event}" for event in expected]
        positions = [log.find(marker) for marker in markers]
        if -1 in positions or positions != sorted(positions):
            raise ValueError("Retirement or shutdown was incomplete. Inspect host/Log.txt.")
        result["shutdown_order"] = expected
        result["script_assertions"] = sum(
            line.startswith("expect ") for line in script.read_text().splitlines()
        )
        result["passed"] = True
        print(f"Synthetic aircraft passed {result['script_assertions']} assertions and shutdown checks.")
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        result["error"] = str(error)
        print(str(error), file=sys.stderr)
    finally:
        (out / "results.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(f"Evidence: {out}")
    return 0 if result["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
