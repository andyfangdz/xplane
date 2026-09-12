# X-Plane flight-test harness

This package runs temporary native guidance, records every simulator frame, supervises a test matrix, produces comparison plots, and restores the installation. It currently includes the TorqueSim SR20 power-off 180 adapter at KCDW runway 22, full fuel and 200 lb in each front seat.

On X-Plane 12.4.3, the Rust release passed 14/14 retained flights across seven wind cases, each flown twice. Touchdown speeds were 64.39–66.11 KIAS and roundout began no higher than 34.99 ft AGL. The [migration report](../../docs/poweroff180/README.md) includes the live results, C++ parity checks, restoration records and left-crosswind repeat variation. See [POWER_OFF_180.md](POWER_OFF_180.md) for the controls and [VALIDATION.md](VALIDATION.md) for the earlier C++ validation history.

The [SR20 landing repair](../../docs/poweroff180/timing-fix-20260911/README.md) restores the original 75 ms attitude release threshold, records actual control authority every frame, and adds a bounded late-flare correction. The eight failed flights in the [initial 12.4.4 beta 1 compatibility run](../../docs/compatibility/xplane-12.4.4-b1/README.md) remain separate diagnostic evidence; their historical measurement-valid flags predate the continuous-authority check. Cold startup has a 180-second resume window, with menu recovery attempted only after the first resume fails. Airborne entry and landing limits are unchanged.

The final build passed 20/20 flown landings: 14 wind-matrix flights, four additional left-crosswind repeats and two recorded calm flights. The recordings used broader optional-plugin isolation after a separate startup crash and API-readiness timeout; both failures are retained, and neither is a landing pass. The report includes exact profiles, observed margins, continuous-authority evidence and verified restoration.

## Run a campaign

Close X-Plane, then run from this folder:

```powershell
.\Run-XPlaneTest.ps1 -Config .\configs\smoke.json
```

The command validates configuration, builds the temporary plugin, snapshots the installation, isolates Custom Scenery, deploys the two temporary helpers, launches a dedicated hidden X-Plane process, flies fresh loads, restores everything and generates the report. Global plugins are retained. It refuses to start over an active X-Plane process or an existing helper directory.

`configs/smoke.json` flies calm, left crosswind and strong headwind twice, including a deliberate three-second supervision gap. `configs/wind-matrix.json` flies all seven wind cases twice. Change a named parameter or acceptance limit in a copied JSON card; unknown fields, strings where numbers are expected, and invalid ranges fail before installation changes.

Results go to `Output/performance-tests/XPT_<timestamp>` under the simulator root. `-Name` supplies a unique explicit name. Existing runs are never overwritten. Use `-XPlaneRoot` and `-ApiPort` for another compatible installation. The current setup adapter fixes the SR20 loading and atmospheric setup explicitly; other aircraft require an adapter rather than pretending generic controls own their state.

The launcher bootstraps its Python 3.13 environment if needed. Dependencies are pinned in `requirements.lock`. All three native helpers build from the parent Cargo workspace using the SDK bindings/import libraries supplied by `xplane-sdk-sys`; a separate SDK or Zig installation is unnecessary. Stable Rust with the MSVC target and Visual Studio Build Tools are required. The attitude helper preserves the original v1.9 behavior and its GPL-3.0-or-later provenance; see [the notice](../../crates/xplane-attitude/NOTICE.md).

If external hardware software writes control axes despite `--no_joysticks`, isolate its plugin explicitly for a test. For example, the Rust verification campaigns use `-IsolateGlobalPlugin TelemFFB-XPP`. The launcher records every file before the atomic move and recovery verifies the exact files, hashes, and restored plugin names. The default retains all global plugins.

## Observe and cancel

The worker publishes `status.json` atomically and appends phase changes to `events.jsonl`. Its read-only browser status URL appears in `worker.log` and `status.json`. The server binds only to loopback and exposes `/status` and `/events`; it cannot operate the aircraft or read arbitrary files.

```powershell
.\.venv\Scripts\python.exe -m xpt.cli status --run 'D:\X-Plane 12\Output\performance-tests\XPT_<timestamp>' --watch
.\.venv\Scripts\python.exe -m xpt.cli cancel --run 'D:\X-Plane 12\Output\performance-tests\XPT_<timestamp>'
```

Cancellation pauses and disarms the native test, then restores the session. Native guidance does not depend on status polling. A missing supervisor heartbeat pauses the simulator and releases test authority. A separate hidden owner watchdog restores the installation if the launcher process dies. Recovery verifies executable path and start time before stopping recorded processes, so a reused PID is never enough authority to stop a process.

## Recovery

Normal completion, a Python exception, and launcher errors all call the same idempotent recovery procedure. A recovery manifest exists before the first installation mutation. If a collision or mismatch prevents automatic restoration, the quarantine is preserved and `recovery-error.json` identifies the problem.

```powershell
.\scripts\Restore-Session.ps1 -RunDirectory 'D:\X-Plane 12\Output\performance-tests\XPT_<timestamp>'
```

Recovery atomically restores the original scenery tree and its links, restores all recorded preference and aircraft-state files, retires temporary helpers into the run directory, and checks original aircraft hashes and the protected-installation snapshot. It never recursively merges scenery or silently blesses a changed baseline. `restoration.json` is required before calling a session complete.

## Evidence and comparisons

- `resolved-config.json`: one fully resolved campaign configuration.
- `cards/<name>/effective-config.json`: exactly the native and setup settings for that flight.
- `native-effective.ini`: the plugin’s parsed configuration, read back and compared before flight.
- `startup-resume.json`: initial pause readback and count of menu-recovery attempts; zero means ordinary resume succeeded.
- `trace.csv`: coherent native-frame telemetry, phase transitions, commands, contact latch and timing.
- `supervision.json`: polling observations and supervision-gap evidence, separate from flight truth.
- `native-terminal-safety.json`: paused state, controller authority, throttle and path override read back before Python cleanup.
- `assessment.json`: landing acceptance and measurement validity, with explicit reasons.
- `report.md`, `summary.json`, `charts/*.png` and `charts/*.svg`: repeated path, airspeed, pitch and physical-sink overlays with divergence times.

The report includes every attempt. It distinguishes a functioning harness from a landing that meets its precision limits. Native-frame pitch-rate measurements can reveal peaks missed by the older HTTP polling, so comparisons retain measurement-method labels. The previous SR20 campaign is overlaid when available; it is not pooled into native repeat statistics. Result schema v3 adds v8 guidance identification; schemas v2 and later require healthy attitude armed/active/release and axis-override readbacks on every recorded frame. The native runtime aborts immediately on lost attitude authority.

Rebuild a report without running X-Plane:

```powershell
.\.venv\Scripts\python.exe -m xpt.cli report --run '<run directory>' --baseline '<earlier campaign directory>'
```

## Development and verification

```powershell
.\scripts\Test-Harness.ps1
```

This checks strict configuration, native/Python protocol agreement, native entry gates and timing failures, live-status behavior, and comparison calculations. Rust tests replay 88,586 frozen C++ guidance frames across all seven winds, plus 12,000 complete attitude-loop frames covering all four modes and all eight release reasons. The historical guidance replay explicitly disables the new float correction; separate tests and actual simulator flights verify that behavior. Timing-boundary tests cover 50.25–75 ms frames missed by the frozen attitude fixture. Other checks cover entry/abort/roundout behavior, projection, drums, clipping and readbacks.

Failure probes exercise real installation preparation and recovery; run them only with X-Plane closed:

```powershell
.\Run-XPlaneTest.ps1 -Name XPT_prepare_failure -InjectFailure after_prepare
.\Run-XPlaneTest.ps1 -Name XPT_owner_death -InjectFailure owner_exit_after_prepare
.\Run-XPlaneTest.ps1 -Name XPT_launch_failure -InjectFailure after_launch
.\Run-XPlaneTest.ps1 -Name XPT_watchdog -Config .\configs\watchdog-probe.json
```

These intentionally return failure (the owner-death probe exits abruptly). Success means the expected error/abort is retained and `restoration.json` verifies recovery. The watchdog flight must show `supervisor_lost`; it is not performance evidence. Any native abort stops the matrix so the next attempt starts in a new clean session.

## Extension points

`../../crates/poweroff180/src/guidance.rs` contains the SDK-independent maneuver state machine. `../../plugins/poweroff180-controller/src/runtime.rs` owns X-Plane datarefs, the watchdog and frame logging. `xpt/adapter.py` owns aircraft readiness and paused setup. `../../crates/poweroff180/parameters.csv` defines every native parameter; Cargo and `xpt/config.py` both consume it. Python validates complete campaign cards. `xpt/analysis.py` and `xpt/report.py` assess and compare evidence. `scripts/Session-Common.ps1` and `Restore-Session.ps1` own installation transactions.

Native guidance writes ordinary bank/pitch targets, throttle, flaps and rollout controls; it does not set forces or airborne position. Setup uses the existing bounded paused air-start initialization and explicitly releases it before flight.

`../../crates/poweroff180/snapshot.csv` defines the ordered telemetry fields and native dataref sources. Cargo generates the Rust indexes and CSV header, and `xpt/protocol.py` reads the same schema. Both test suites check the frozen protocol v1 header. Campaign source manifests include this schema alongside the parameter schema.

## HUD video capture

Add `-RecordVideo` to a campaign command to record the maneuver from stabilized downwind through touchdown and initial rollout. This temporarily selects the unobstructed forward view, the native custom HUD, 1920-pixel native movie capture at 30 fps, and audible simulator sound. Original preferences, including VR, are restored with the session.

The temporary `XPTVideoHUD` v5 helper draws the custom instrument layout inside X-Plane. Its shallow magenta command bars, faceted yellow aircraft symbol, continuous bank arc and solid pitch marks follow the supplied G1000 reference. Its airspeed/altitude tapes, rolling digits, speed bands, selected bugs, tapered VSI and heading-up GPS HSI follow Garmin pilot-guide illustrations. It includes camera-projected attitude and flight-path cues, engine/flap/control indications, live RNAV navigation and accumulated flight paths. Drawing only reads guidance. See [CUSTOM_HUD.md](CUSTOM_HUD.md) for display semantics and reproduction details.

Each card retains native AVI segments, WASAPI loopback audio, audio metadata and `capture.json` with measured recording times and errors. Final MP4 export must align the audio and video using those times and verify non-silent audio and complete decoding. Current lower-flare exports and verification records are in `Output/performance-tests/TORQUESIM_SR20_LOWER_FLARE_20260909`. Earlier HSI/tape recordings remain in `TORQUESIM_SR20_G1000_HSI_TAPES_20260909`. Earlier v4 command-bar exports remain in `TORQUESIM_SR20_G1000_MATCH_20260909`, and v3 exports remain in `TORQUESIM_SR20_NATIVE_CUSTOM_HUD_20260909`.


## Automated rendering and loading profile

All automated trials temporarily select 2D rendering with reduced object, vegetation, shadow and texture detail to avoid the observed memory failures during reload. The exact preference map is stored in each session manifest. Original preferences, including VR and graphics detail, are restored and hash-verified. The HUD remains native v5.

The v3 SR20 adapter verifies the requested full-tank loading, tank-mass stability and 200 lb in each front seat. Its native mass guard is bound to the measured starting load, with the existing 3 lb drift limit. Wind is added to the initial velocity. Achieved airspeed is verified by the sustained native 100 KIAS airborne entry gate; held-path airspeed readbacks are not used as flight evidence. The nominal weight alone is not used as proof of loading; each card retains the actual weight and configuration readback.

## Rust migration

Guidance, inner attitude control, and the HUD are Rust plugins in the parent workspace. Python performs paused setup, supervision, capture and analysis; PowerShell owns reversible installation transactions. [The migration report](../../docs/poweroff180/README.md) records tests and live evidence. Frozen C++ files are replay-test references only; no C++ controller binary is required or deployed.
