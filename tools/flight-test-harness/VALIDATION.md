# Harness validation — 8–9 September 2026

The current Rust migration and live verification are documented in [the migration report](../../docs/poweroff180/README.md). This page retains the earlier C++ infrastructure validation as history.

## Native v7 lower-flare validation

All 14 release flights passed, across seven wind cases with at least two flights per case. One release campaign was recorded, with three replacement exports covering calm air, left crosswind and strong headwind. The final frozen source and native v7 configuration readback are verified across all release campaigns.

Touchdowns were 1041–1132 ft at 64.44–66.15 KIAS. Maximum physical sink was 165 fpm, maximum roundout entry height 34.99 ft, maximum pitch 7.366 degrees and maximum pitch rate 2.739 degrees/s.

| Wind | Flights | Touchdown ft | KIAS | Max sink fpm | Max pitch | Max rate |
|---|---:|---:|---:|---:|---:|---:|
| calm | 2 | 1041–1075 | 66.12–66.15 | 55 | 6.85° | 2.47°/s |
| cross_left10 | 2 | 1073–1073 | 65.31–66.00 | 126 | 7.29° | 2.17°/s |
| cross_right10 | 2 | 1129–1132 | 64.79–64.80 | 21 | 7.25° | 2.27°/s |
| head10 | 2 | 1114–1119 | 64.44–64.52 | 131 | 7.23° | 2.46°/s |
| head15 | 2 | 1084–1120 | 65.06–65.29 | 165 | 7.37° | 2.74°/s |
| head5 | 2 | 1104–1108 | 64.93–64.94 | 61 | 7.30° | 2.45°/s |
| tail5 | 2 | 1087–1092 | 65.93–65.94 | 35 | 6.56° | 1.98°/s |

The v3 loading adapter uses measured full-fuel starting weight with the unchanged 3 lb drift limit. The temporary reduced-detail 2D rendering profile avoids the observed reload-memory failures. Preparation-failure probes verified exact restoration of all changed rendering preferences. Every release campaign verifies 176 scenery entries, five links, 122 preference files, 11 aircraft-state files, original aircraft/plugin hashes and protected installation state. No release log contains a crash, fatal error or E/SYS warning.

`Test-Harness.ps1` passes 11 Python tests, native guidance/HUD tests and process-identity checks. [The release report](../../docs/poweroff180/lower-flare-report.md) preserves development misses and startup/loading failures separately, and links the synchronized videos. This is simulation evidence for these conditions, not a universal wind or aircraft guarantee.

## Historical native v6 refinement

The frozen v6 campaign passed **21/21 flights**, three fresh loads in each of seven wind cases, with all measurements valid. The native controller adds continuous final-turn geometry, predictive centerline capture and bounded approach-path pitch feedback while preserving the 65 ft gradual roundout. Loading remains full balanced fuel and 200 lb in each front seat, approximately 2,942.5 lb. All acceptance limits are unchanged.

Touchdowns ranged from 1,059 to 1,190 ft. Maximum short-final offset was about 11.0 ft, maximum touchdown offset 2.53 ft, maximum physical sink 130 fpm and minimum touchdown IAS 68.3 kt. Peak measured flare pitch was 5.15° and pitch rate 2.38°/s. All 21 three-second supervision gaps preserved native control. Left-crosswind distances span 131 ft and the longest landing leaves only 10.2 ft below the upper limit; this does not establish a broad disturbance margin.

- Full refinement report, comparisons and development history (local archive: `Output/performance-tests/TORQUESIM_SR20_NATIVE_REFINEMENT_20260909/report.md`)
- Native v6 flight traces and per-flight results (local archive: `Output/performance-tests/XPT_PATH_V6_VALIDATION_20260909/report.md`)
- [Exact formulas and reproduction](POWER_OFF_180.md)

The final campaign's automatic restoration refused to stop the simulator because its original launch record contained a null executable path. PID, exact start time, parent PID and command line were independently verified before stopping only that simulator; the standard restoration then completed. The original error and independent identity evidence (local archive: `Output/performance-tests/XPT_PATH_V6_VALIDATION_20260909/independent-process-identity.json`) are retained.

A subsequent lifecycle-only fix obtains a fresh process object and requires a nonempty executable path with the original start time. Tests cover a stale null-path launch object and mismatched identity rejection. A real after-launch failure probe (local archive: `Output/performance-tests/XPT_PATH_PROCESS_IDENTITY_20260909/report.md`) captured complete identities and restored automatically. Its expected nonzero exit is fault-injection evidence, not a landing result. Guidance, parameters, adapter and native binary remain byte-identical to the frozen 21-flight version; released source hashes (local archive: `Output/performance-tests/TORQUESIM_SR20_NATIVE_REFINEMENT_20260909/released-source-manifest.json`) identify the two changed lifecycle/test files.

`Test-Harness.ps1` passes seven Python tests, native C++ tests, PowerShell syntax checks and process-identity regressions. Earlier failed landings and setup failures remain in the report. The earlier v1 infrastructure evidence below is historical.

The native guidance, single-command lifecycle, resolved configuration, live status, and automated comparisons were exercised in X-Plane 12 with the TorqueSim SR20 at KCDW runway 22. The six-flight campaign completed with exit code 0 and exact installation restoration. Landing accuracy remains a separate result: three of six flights met every configured limit.

## Earlier native v1 flight campaign

Run: XPT_NATIVE_SMOKE_D_20260909 (local archive: `Output/performance-tests/XPT_NATIVE_SMOKE_D_20260909/report.md`).

The campaign flew calm, a 10 kt left crosswind, and a 15 kt headwind twice, with a fresh flight load for each card. Every card used full balanced fuel and 200 lb in each front seat, verified native configuration readback, and the sustained entry gate. All six flights completed without a native abort and produced valid measurements.

| Flight | Touchdown ft | Physical sink fpm | Landing result |
|---|---:|---:|---|
| Calm 1 | 1075.1 | 83.1 | Alignment limits exceeded |
| Calm 2 | 1144.8 | 76.4 | Pass |
| Left crosswind 1 | 1098.7 | 130.1 | Short-final alignment exceeded |
| Left crosswind 2 | 1140.2 | 103.6 | Short-final alignment exceeded |
| Headwind 1 | 1169.6 | 106.8 | Pass |
| Headwind 2 | 1085.9 | 96.1 | Pass |

There are 30,126 native-frame samples. The median control interval is 0.03011 seconds (about 33 Hz); the largest is 0.05025 seconds. In each flight, Python deliberately stopped polling and heartbeats for three seconds. Native guidance continued for another 99–100 control steps, advanced about three simulator seconds, and reported no fault. These measurements demonstrate independence from Python polling for the tested gaps.

Both calm and crosswind repeat overlays stayed below the configured diagnostic thresholds. The headwind comparison flagged a 4.1° pitch difference at 77.3 seconds after the cut, near the differently timed touchdowns. This diagnostic includes rollout and should be interpreted with the plotted contact timing. The PNG and SVG plots retain the previous HTTP-guided campaign as a separately labeled baseline. Calm and headwind plots were visually inspected for labels, legibility, and overlays.

The browser status page was checked during the active campaign. It displayed the current flight and phase, changing native telemetry, completed count, and fresh-data age. Unit tests also checked the loopback JSON endpoint and rejection of unrelated file paths.

## Recovery and fault evidence

- Preparation failure (local archive: `Output/performance-tests/XPT_RECOVERY_PREP_20260909/restoration.json`): intentionally threw after installation preparation; the launcher's finally block restored the session.
- Launcher death (local archive: `Output/performance-tests/XPT_OWNER_DEATH_20260909/restoration.json`): intentionally exited without running finally; the independent watcher restored the session.
- Initial native smoke attempts exposed cold-start sequencing issues. Dataref registration preceded vendor physics activation, and X-Plane's total mass stayed stale while the flight path was held. The setup now waits for native activation, briefly releases the flight model during setup to refresh mass, verifies agreement twice, then lets the existing setup reseed the start. Failed and cancelled attempts are retained as XPT_NATIVE_SMOKE_20260909, XPT_NATIVE_SMOKE_B_20260909, and XPT_NATIVE_SMOKE_C_20260909; each restored successfully.
- The completed six-flight campaign restored 176 scenery entries, including five links, and verified exact name sets, 122 preference files, 11 aircraft-state files, original source hashes, and removal of both temporary helpers.

- Native heartbeat loss (local archive: `Output/performance-tests/XPT_WATCHDOG_20260909/report.md`): stopped heartbeats for four seconds with a two-second native timeout. The native trace records `supervisor_lost` at 2.0187 seconds of heartbeat age, after 66 further control steps. Simulator time then stopped. The terminal readback (local archive: `Output/performance-tests/XPT_WATCHDOG_20260909/cards/calm-01/native-terminal-safety.json`), taken before Python cleanup, proves paused = 1, controller armed = 0, throttle = 0, and all flight-path overrides = 0. The worker returned the expected failure, classified the aborted landing measurement invalid, and restored the full installation automatically.
- Watcher reporting retest (local archive: `Output/performance-tests/XPT_OWNER_REPORT_20260909/report.md`): terminated the launcher immediately after preparation with exit code 23. The independent watcher recorded launcher loss, restored the full installation, generated the error report, and exited without watchdog errors.

The final audit verified restoration records for all eight sessions, rechecked the live scenery inventory, state files and source hashes, and confirmed that no recorded worker, watcher, simulator, or temporary live helper remained. The final executable harness files match the live watchdog run's source manifest.

## Automated checks and scope

`scripts/Test-Harness.ps1` passes six Python tests, native C++ configuration/entry/timing tests with assertions enabled, and PowerShell syntax checks. The tests cover strict typed configuration, complete native roundtrip, the 75-field protocol, live status, repeat divergence, and invalid classification of aborted landing trials.

The tested native flight binary and guidance source are unchanged after the six-flight campaign. Subsequent changes add terminal-state evidence, classify native aborts as invalid landing measurements, shade plots using the actual configured touchdown interval, and generate a report after launcher-loss recovery. Each run retains its own source hashes, so these versions remain distinguishable.

This validates the infrastructure on the installed TorqueSim SR20 and documented loading. It does not establish universal landing precision, behavior on another aircraft, or improved repeatability over a statistically matched old-controller campaign. Global plugins remain installed; the harness isolates Custom Scenery. Temporary runtime helpers use ordinary control targets and are removed during restoration.
