"""Exercise the original public fixture; no X-Plane installation or aircraft assets needed."""
import argparse
import base64
import ctypes
import http.client
import json
from pathlib import Path
import shutil
import socket
import subprocess
import time


ROOT = Path(__file__).resolve().parents[1]
NS = "xphost/test/"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path, default=ROOT / "build/test-results")
    parser.add_argument("--port", type=int, default=0, help="localhost test port; 0 chooses an unused port")
    args = parser.parse_args()
    if not 0 <= args.port <= 65535:
        parser.error("--port must be in 0..65535")
    host = ROOT / "build/xphost.exe"
    package = ROOT / "build/fixture"
    plugin = package / "plugins/test/64/win.xpl"
    if not host.is_file() or not plugin.is_file():
        parser.error("run build.ps1 with external X-Plane SDK headers first")
    args.out = args.out.resolve()
    args.out.mkdir(parents=True, exist_ok=True)
    results = {"passed": False, "checks": [], "uses_xplane_installation": False}
    flags = subprocess.BELOW_NORMAL_PRIORITY_CLASS | subprocess.CREATE_NO_WINDOW
    common = ["--pkg", str(package), "--acf", "fixture.acf", "--datarefs", "none",
              "--commands", "none", "--navdat", "none"]

    def check(name, ok, detail=None):
        results["checks"].append({"name": name, "passed": bool(ok), "detail": detail})
        print(f"{'PASS' if ok else 'FAIL'} {name}" + (f": {detail}" if detail is not None else ""), flush=True)
        if not ok:
            raise AssertionError(name)

    def lifecycle(folder):
        log = (folder / "Log.txt").read_text(encoding="utf-8", errors="replace")
        expected = ["fixture: start", "fixture: enable", "fixture: disable", "fixture: stop"]
        observed = [line for line in log.splitlines() if line.startswith("fixture:")]
        return observed == expected and "Stopped:" not in log and "fault" not in log.lower()

    def run_cli(name, extra, expected=0, *, base=None, check_lifecycle=True):
        folder = args.out / name
        command = [str(host), *(common if base is None else base), *extra, "--out", str(folder)]
        completed = subprocess.run(command, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                                   timeout=40, creationflags=flags)
        (args.out / f"{name}-stdout.txt").write_bytes(completed.stdout)
        check(name + " exit", completed.returncode == expected, completed.returncode)
        if check_lifecycle:
            check(name + " lifecycle", lifecycle(folder))
        return completed.stdout.decode("utf-8", errors="replace")

    process = None
    try:
        run_cli("script", ["--clock", "fast", "--script", str(ROOT / "tests/fixture/smoke.xps")])
        negative = args.out / "expected-failure.xps"
        negative.write_text("expect xphost/test/value 999 999\n", encoding="utf-8", newline="\n")
        run_cli("script-negative", ["--script", str(negative)], expected=3)
        run_cli("required-acf", ["--pkg", str(package)], expected=1, base=[], check_lifecycle=False)
        for invalid in ("nope", "nan", "inf", "-0.1", "0"):
            run_cli("invalid-dt-" + invalid, ["--dt", invalid], expected=1, check_lifecycle=False)
        for flag in ("--datarefs", "--commands", "--navdat"):
            run_cli("missing-" + flag[2:], [flag, str(args.out / "absent-input")], expected=1, check_lifecycle=False)
        ambiguous = args.out / "ambiguous fixture"
        ambiguous.mkdir(exist_ok=True)
        shutil.copyfile(package / "fixture.acf", ambiguous / "fixture.acf")
        for label in ("one", "two"):
            destination = ambiguous / "plugins" / label / "64/win.xpl"
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(plugin, destination)
        run_cli("ambiguous-plugins", ["--pkg", str(ambiguous)], expected=1, check_lifecycle=False)
        run_cli("explicit-plugin", ["--pkg", str(ambiguous), "--xpl", str(ambiguous / "plugins/one/64/win.xpl"), "--seconds", "0.1"])
        debug = args.out / "debug-echo.xps"
        debug.write_text("dbg public generic echo\n", encoding="utf-8", newline="\n")
        run_cli("unconfigured-debug", ["--script", str(debug)], expected=3)
        output = run_cli("configured-debug", ["--script", str(debug), "--debug-request", NS + "bytes", "--debug-result", NS + "bytes"])
        check("explicit generic debug channels echo", "dbg public generic echo -> public generic echo" in output.splitlines())
        output = run_cli("selftest", ["serve", "--selftest"])
        check("selftest all checks", "selftest done: 0 failure(s)" in output)

        # Probe/reserve only loopback. No existing process is stopped or reused.
        with socket.socket() as reservation:
            reservation.bind(("127.0.0.1", args.port))
            port = reservation.getsockname()[1]
        results["port"] = port
        host_out = args.out / "serve"

        def request(connection, method, path, value=None, *, raw=None, expected=200):
            body = raw if raw is not None else None if value is None else json.dumps(value)
            connection.request(method, path, body=body, headers={"Content-Type": "application/json"})
            response = connection.getresponse()
            data = json.loads(response.read())
            if response.status != expected:
                raise AssertionError(f"{method} {path}: expected {expected}, got {response.status}: {data}")
            return data

        def fresh(method, path, value=None, *, raw=None, expected=200):
            connection = http.client.HTTPConnection("127.0.0.1", port, timeout=12)
            try:
                return request(connection, method, path, value, raw=raw, expected=expected)
            finally:
                connection.close()

        def handles():
            count = ctypes.c_ulong()
            kernel = ctypes.WinDLL("kernel32", use_last_error=True)
            kernel.GetProcessHandleCount.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_ulong)]
            kernel.GetProcessHandleCount.restype = ctypes.c_int
            if not kernel.GetProcessHandleCount(int(process._handle), ctypes.byref(count)):
                raise ctypes.WinError(ctypes.get_last_error())
            return count.value

        with (args.out / "serve-stdout.txt").open("wb") as stdout:
            process = subprocess.Popen([str(host), "serve", "--port", str(port), *common,
                                        "--clock", "real", "--out", str(host_out)],
                                       stdout=stdout, stderr=subprocess.STDOUT, creationflags=flags)
            deadline = time.monotonic() + 15
            while True:
                if process.poll() is not None:
                    raise AssertionError("private host exited before listening")
                try:
                    refs = fresh("GET", "/api/v3/datarefs")["data"]
                    break
                except (ConnectionRefusedError, TimeoutError):
                    if time.monotonic() >= deadline:
                        raise AssertionError("private host listen timeout")
                    time.sleep(0.05)
            ids = {ref["name"]: ref["id"] for ref in refs}
            commands = {cmd["name"]: cmd["id"] for cmd in fresh("GET", "/api/v3/commands")["data"]}

            def path(name):
                return f"/api/v3/datarefs/{ids[name]}/value"

            def get(name):
                return fresh("GET", path(name))["data"]

            def put(name, value):
                return fresh("PATCH", path(name), {"data": value})

            def cmd(name, duration=0):
                return fresh("POST", f"/api/v3/command/{commands[name]}/activate", {"duration": duration})

            def eventually(predicate, timeout=3):
                end = time.monotonic() + timeout
                while time.monotonic() < end:
                    if predicate():
                        return True
                    time.sleep(0.025)
                return False

            check("fixture readiness and lifecycle messages", get(NS + "ready") == 1 and get(NS + "messages") == 3)
            first = get(NS + "frames")
            check("owned flight loop advances", eventually(lambda: get(NS + "frames") > first))
            put(NS + "value", 12.5)
            check("owned scalar roundtrip", get(NS + "value") == 12.5)
            put(NS + "array", [8, 7, 6, 5])
            check("owned array roundtrip", get(NS + "array") == [8, 7, 6, 5])
            payload = b"public fixture \x00 bytes"
            put(NS + "bytes", base64.b64encode(payload).decode("ascii"))
            check("owned bytes roundtrip", base64.b64decode(get(NS + "bytes")) == payload)

            # Invalid names/IDs/body types must not silently address ID zero or coerce values.
            initial_scalar = get(NS + "value")
            for invalid in ("abc", "-1", "0junk", "999999999999999999999"):
                fresh("GET", f"/api/v3/datarefs/{invalid}/value", expected=400)
                fresh("PATCH", f"/api/v3/datarefs/{invalid}/value", {"data": 99}, expected=400)
                fresh("POST", f"/api/v3/command/{invalid}/activate", {"duration": 0}, expected=400)
            check("malformed IDs rejected", get(NS + "value") == initial_scalar)
            for body in ('{"data":"not-a-number"}', '{"data":NaN}', '{"data":1e999}',
                         '{"data":12garbage}', '{"data":false}', '{"data":[]}'):
                fresh("PATCH", path(NS + "value"), raw=body, expected=400)
            check("malformed scalar writes rejected", get(NS + "value") == initial_scalar)
            fresh("PATCH", path(NS + "array"), raw='{"data":[1,NaN]}', expected=400)
            check("malformed array rejected atomically", get(NS + "array") == [8, 7, 6, 5])

            cmd(NS + "pulse", 0.12)
            check("finite command begins and ends", eventually(lambda: get(NS + "command_end") == 1)
                  and get(NS + "command_begin") == 1 and get(NS + "command_continue") > 0)
            after = get(NS + "command_continue")
            time.sleep(0.15)
            check("command stops continuing after end", get(NS + "command_continue") == after)

            chain_refs = [NS + suffix for suffix in ("chain_before", "chain_second", "chain_after", "chain_after_second")]

            def chain_counts():
                return [get(name) for name in chain_refs]

            cmd(NS + "chain")
            check("before and after return1 continue", chain_counts() == [1, 1, 1, 1])
            put(NS + "consume", 1)
            cmd(NS + "chain")
            check("before return0 consumes", chain_counts() == [2, 1, 1, 1])
            put(NS + "consume", 0)
            put(NS + "consume_after", 1)
            cmd(NS + "chain")
            check("after return0 stops later after callback", chain_counts() == [3, 2, 2, 1])
            put(NS + "consume_after", 0)
            put(NS + "consume", 1)
            cmd("sim/operation/pause_on")
            check("before return0 suppresses simulator default", get("sim/time/paused") == 0)
            put(NS + "consume", 0)
            cmd("sim/operation/pause_on")
            check("before return1 permits simulator default", get("sim/time/paused") == 1)
            put("sim/time/paused", 0)

            before = handles()
            for _ in range(200):
                if not isinstance(get("sim/time/total_running_time_sec"), (float, int)):
                    raise AssertionError("clock read was not numeric")
            check("fresh connection reads", True, 200)
            connection = http.client.HTTPConnection("127.0.0.1", port, timeout=12)
            try:
                for _ in range(40):
                    request(connection, "GET", path("sim/time/total_running_time_sec"))
            finally:
                connection.close()
            check("keepalive reads", True, 40)
            connection = http.client.HTTPConnection("127.0.0.1", port, timeout=12)
            try:
                connection.connect()
                time.sleep(0.3)
                request(connection, "GET", path(NS + "value"))
            finally:
                connection.close()
            check("delayed request on open socket", True)
            time.sleep(0.2)
            after = handles()
            check("completed request handles bounded", after <= before + 16, [before, after])
            idle = [socket.create_connection(("127.0.0.1", port), timeout=3) for _ in range(2)]
            try:
                fresh("POST", "/xphost/stop", {})
                process.wait(12)
            finally:
                for connection in idle:
                    connection.close()
            check("graceful exit with idle clients", process.returncode == 0, process.returncode)
            check("serve lifecycle and log", lifecycle(host_out))
        results["passed"] = True
    except Exception as exc:
        results["error"] = f"{type(exc).__name__}: {exc}"
        raise
    finally:
        if process is not None and process.poll() is None:
            process.kill()  # Only this exact child; never discover or stop another process.
            process.wait(10)
        (args.out / "results.json").write_text(json.dumps(results, indent=2) + "\n", encoding="utf-8", newline="\n")


if __name__ == "__main__":
    main()
