---
name: test-xplane-plugin
description: Run fast headless X-Plane plugin integration tests with xphost. Use for native plugin lifecycle, flight-loop scheduling, dataref and command contracts, aircraft profiles, and Web API drivers before simulator trials. Flight performance, rendered avionics, VR, scenery, and audio require X-Plane testing.
---

# Test X-Plane Plugin

Use xphost to shorten the edit/build/test loop for plugin logic. It loads a real
Windows x64 `win.xpl` against a fake SDK and advances fixed simulation steps
without launching X-Plane. A passing headless test establishes only the contract
it asserts; retain simulator acceptance checks for behavior outside this host.

## Locate and prepare

Find the X-Plane repository checkout containing `tools/xphost/Cargo.toml` and run
commands from its root. Read `tools/xphost/README.md` for current CLI, profile
schema and supported SDK behavior. The maintained implementation lives in
`crates/xphost-core`, `crates/xphost-native`, `crates/xphost-sdk` and
`tools/xphost/src`; avoid archived copies under build/evidence directories.

Build with stable Rust/MSVC and Visual Studio C++ Build Tools on Windows:

```powershell
.\tools\xphost\build.ps1
if ($LASTEXITCODE -ne 0) { throw 'xphost build failed' }
```

The script discovers MSVC and uses the workspace's `xplane-sdk-sys` headers.
Keep the staged `tools/xphost/build/xphost.exe` and `XPLM_64.dll` together;
they share a versioned private protocol. `cargo build` alone does not stage that
DLL under its required SDK name. Rebuild after changing the host or embedded
profiles; custom profile JSON and scenario scripts need no host rebuild.

For a target plugin, build its affected package rather than the whole workspace
on every edit. Use a disposable package outside the live simulator installation;
copy the target's required dependencies/assets there and select the newly built
plugin explicitly with `--xpl`. An `.acf` must exist under `--pkg` and have the
identity the plugin expects, but xphost does not parse its aircraft model.
Native plugins may write package state; the host is not a sandbox. Keep vendor
content, local captures and run outputs in ignored/private directories.

## Make the test discriminate

- Translate the reported bug into observable initial state, a trigger and
  expected plugin outputs. Assert a failing scenario before fixing it when
  practical; use unit tests for pure calculations and xphost for the SDK boundary.
- Choose `--profile generic` unless aircraft assumptions are needed. `c182`
  retains legacy fixture defaults, not book performance. For another aircraft,
  supply custom JSON with its mass, parked height, engine count, bus settings,
  indicator mappings and simulator-owned seeds. Profiles seed only `sim/`
  datarefs; the plugin registers its own namespace.
- Provide only the needed simulator datarefs via the profile or an explicit
  `--datarefs` catalog. Supply `--commands`/`--navdat` when the scenario needs
  them; all three default to `none`. Investigate `dataref-misses.txt` before
  interpreting a missing-input failure as a plugin defect.
- Run with `--clock fast` and the intended `--dt` (default 1/60 s). Use bounded
  `wait_until` checks instead of long wall-clock sleeps. If the plugin depends
  on wall time, threads or I/O, test that scenario with `--clock real`; do not
  increase the timestep or weaken assertions to manufacture a pass.
- Assert command effects and command begin/continue/end behavior where relevant.
  Unknown command lookup may synthesize a no-op. Finding a name or receiving
  HTTP 200 is not evidence that the intended plugin action happened.
- Seed simulator inputs explicitly and assert plugin outputs. The fake store
  permits fixture writes that X-Plane may prohibit. A dataref seed or derived
  engine indication does not prove a physical flight/engine result.

Use `.xps` scripts for deterministic sequences: `set`, `sets`, `cmd`,
`cmd_begin`, `cmd_end`, `wait`, `wait_until`, `expect`, `trace`, `dump` and `get`.
Array elements use `name[index]`; `#` begins a comment. `expect ref low high`
collects failures and exits 3. Optional `dbg` requires paired explicit byte
datarefs via `--debug-request` and `--debug-result`.

Start with the bundled asset-free fixture to prove the host works:

```powershell
$runOut = Join-Path (Get-Location) ('tmp/xphost/smoke-' + [guid]::NewGuid().ToString('N'))
& .\tools\xphost\build\xphost.exe `
    --pkg .\tools\xphost\build\fixture --acf fixture.acf `
    --profile generic --clock fast `
    --script .\tools\xphost\tests\fixture\smoke.xps --out $runOut
if ($LASTEXITCODE -ne 0) { throw "xphost failed; inspect $runOut" }
```

For the target, replace the fixture package, ACF and script; add the explicit
`--xpl` and relevant profile. Use a fresh `--out` for each retained run. Scripts
end when their instructions finish unless `--seconds` requests additional time.
Bound subprocess wall time when automating, so a blocked native callback cannot
hang a test campaign. Capture stdout and the exit status.

## Reuse HTTP drivers when useful

Use `serve` for an existing Web API driver or interactive scenarios; prefer a
script for a short deterministic reproduction. Choose an unused loopback port
and point the driver at that private host instead of a running simulator.
Supported routes are dataref/command enumeration, dataref value GET/PATCH,
command activation with a finite duration, and `POST /xphost/stop`.

Discover IDs for each process; never carry IDs across runs. Byte values use
base64. Plugin accessors and commands run on the simulation thread. A 504 means
that queued work timed out; cancelled work cannot begin later, while an already
running operation may complete. There is no `/flight`, aircraft reload, or full
X-Plane Web API.

Wait for this process's API to become ready before assertions. Stop it with
`POST /xphost/stop` and wait for its exit; on timeout, terminate only the exact
child process retained by the test runner. Do not discover/kill processes by
name or displace another service to free a port. `serve --selftest` is for the
bundled fixture, not an arbitrary target plugin.

## Verify proportionately and report

After a target-plugin edit, rerun its affected scenario and relevant regression
cases. Run the host suites after changing the host, its profiles or transport,
or when distinguishing a host defect from a plugin defect:

```powershell
cargo test -p xphost-core -p xphost
python tools/xphost/tools/transport_regression.py --out tmp/xphost/transport
python tools/xphost/tools/native_regression.py --out tmp/xphost/native
python tools/xphost/tools/profile_regression.py --out tmp/xphost/profiles
```

Rebuild first when testing changed host code. The Python suites launch only
their own asset-free fixtures and write `results.json`. Inspect exit statuses:
0 success, 1 input/setup failure, 2 caught native plugin fault, 3 failed script
or selftest. Classify the failure before rerunning; fix a missing fixture or
unsupported import rather than tuning the aircraft to it. The optional
`tools/xphost/tools/check_exports.py` checks symbol coverage, not SDK semantics.

Report the plugin build, profile, timestep/clock, scenario and commands used,
assertion result, and output directory (`aircraft-profile.json`, `Log.txt`,
`commands.tsv`, dataref misses and selected traces). Keep public reproductions
synthetic. Hand off remaining flight-model, rendered avionics, VR, audio,
scenery or other-plugin integration checks to X-Plane; use the existing
`test-xplane-aircraft` workflow when flight performance is the question.
