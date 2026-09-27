# Performance investigation

Use the existing transaction to launch a short paused SR20 camera sweep:

```powershell

.\scripts\Install-PerformanceTools.ps1

.\Run-XPlaneTest.ps1 -Mode Performance -Config .\configs\performance-overhead.json -Name XPT_overhead -RenderingProfile User2D -KeepCustomScenery -VisibleSimulator -NoDefragment -TextureCacheGB 10

```

The default performance configuration requires a WPR host. On this machine, run `scripts/Start-InstalledWprHost.ps1` from an ordinary PowerShell and wait for the CPU privilege probe and a successful fresh-nonce ping. The protected on-demand task `\CodexXPlaneTracing\XPlaneTraceHost` was installed and verified on 2026-09-14; subsequent starts need no UAC prompt. The installed action has a 180-minute lifetime. Protected helper/profile updates still need explicit administrator approval. The portable fallback is `scripts/Start-WprHost.ps1` in an administrator PowerShell before the run. Keep `Session-Common.ps1` and `XPT-Performance.wprp` beside it. Only this helper needs elevation; keep X-Plane and the harness unelevated. It accepts a fixed, hash-checked profile for verified harness sessions, owns named WPR instances, and exits after two hours. Stop it by writing `stop-<nonce>.request` in the host queue named in `D:/X-Plane 12/Output/performance-tests/.xpt-wpr-host/host.json`. It saves any active trace before exit. `performance-2d-validation.json` explicitly omits WPR; its separate live PresentMon path remains diagnostic until it passes the same event coverage checks.

The Windows 11 profile combines sampled CPU stacks, scheduling, hard faults, file/disk I/O and filtered presentation events. The current profile has 4 GiB of system buffers and 256 MiB of event buffers. Two full-flight diagnostic captures validated complete coverage of their approximately 38-second accepted windows with zero reported event loss; verify each future capture independently. The earlier 512 MiB system buffer retained only about nine seconds of kernel events in a 42-second capture on this host, despite zero reported event loss. Verify actual QPC coverage for CPU/scheduling events as well as presentation events; event-loss counters alone cannot detect ring-buffer wrap. Presentation event IDs and keywords follow [PresentMon 2.5.1's provider setup](https://github.com/GameTechDev/PresentMon/blob/v2.5.1/PresentData/PresentMonTraceSession.cpp), including its exclusion of the DxgKrnl Performance keyword. The bounded memory trace can wrap on a long or busy run: event-loss and full-window coverage checks are mandatory. The broad built-in GPU/CPU/FileIO profiles previously created an 8.4 GiB trace and omitted events needed to recover X-Plane presentations.

Bring the simulator to the foreground while it loads, then leave its window and controls alone during recording. Performance mode supplies the flight initialization JSON at launch and disables the startup menu. It verifies the aircraft, KCDW runway 22 position and heading, completed scenery loading, advancing callbacks, pause, parked state, focus and actual 2D mode. It invalidates a segment that violates the continuous gates. Fatal startup messages and the Safe Mode log banner stop setup. A post-crash Safe Mode prompt requires declining Safe Mode to test normal settings; such interaction contaminates launch-time comparisons. The launcher suppresses the separate crash reporter window. Workers wait for their recovery identity to be saved before contacting X-Plane, and record updates tolerate temporary Windows file-sharing conflicts.

`performance-overhead.json` runs three alternating AB/BA pairs in one scene: the same native camera sweep with frame storage disabled or enabled. Both variants use the same WPR profile; PresentMon decodes each ETL after measurement. This measures the incremental native frame-storage cost; it does not measure the total overhead of every enabled observer. `performance-stutters.json` records three longer sweeps. Both reuse the scene. The scenario exercises camera-driven rendering and texture demand; it does not reproduce moving-aircraft scenery loading or VR compositor behavior.

For launch-flag or cache-capacity comparisons:

```powershell

.\scripts\Run-PerformanceMatrix.ps1 -Compare Flag -Name XPT_flag_matrix

.\scripts\Run-PerformanceMatrix.ps1 -Compare Cache -Name XPT_cache_matrix

```

Every matrix attempt delegates installation changes to `Run-XPlaneTest.ps1`, starts a fresh process, and verifies restoration before continuing. `Flag` compares defragmentation on/off with a 10 GB cache. `Cache` compares 0/10 GB with the no-defragment flag enabled. Three alternating AB/BA pairs are the default. Cache contents and operating-system caches are retained, so load comparisons are warm-cache observations. Existing preferences and xOrganizer settings are preserved by the transaction.

During a running segment, mark a stutter from another terminal:

```powershell

.\.venv\Scripts\python.exe -m xpt.perf_cli mark --run 'D:\X-Plane 12\Output\performance-tests\XPT_overhead' --note 'felt a hitch turning left'

```

The request timestamp and native receipt timestamp are both retained. For immediate in-simulator marks, bind the X-Plane command `xpt/perf/mark` to a spare key or joystick button while the recorder plugin is loaded. `xpt/perf/start` and `xpt/perf/stop` support passive recording outside the benchmark: a fresh, unconfigured plugin defaults to a bounded five-minute capture without moving the camera. The helper is removed when a harness transaction ends; persistent manual installation is not required for automated benchmarks.

Each run writes `PERFORMANCE.md`, `performance-report.json`, and one `segments/<label>` folder per attempt containing its system ETL and decoded PresentMon CSV. Native CSV/JSON include absolute QPC ticks, the clock frequency, the exact measured interval, validity flags, and markers. Formatting and CSV export occur after measurement; memory pages are touched before measurement. Landing controller 0.8.5 also buffers its existing trace rows until completion, preserving its CSV schema and native guidance. Scripts that tail a live landing CSV must instead use the status server; the final CSV appears at completion or an orderly abort.

Reports distinguish native callback intervals from PresentMon CPU-start intervals. Neither should be described as actual monitor scan-out or VR presentation. Assessments retain hitch QPC ranges for correlation with `system.etl`. Inspect WPR event-loss/status information before drawing causal conclusions. Load timing is separate: launch to the paused, plugin-ready scene is the primary metric; subsequent focus wait is reported separately. Failed attempts, partial exports, plugin-load failures, tool logs, and source manifests remain in the run directory. A crash can lose an unexported native memory buffer; external traces and retired helper files are retained.

Validation evidence and remaining limitations are tracked in the compatibility report. Primary API references: [X-Plane flight initialization](https://developer.x-plane.com/article/flight-initialization-api/), [PresentMon](https://github.com/GameTechDev/PresentMon), and [WPR command line](https://learn.microsoft.com/en-us/windows-hardware/test/wpt/wpr-command-line-options).

## Complete moving-flight stutter comparisons

Use the complete maneuver to reproduce moving-flight hitches:

```powershell

.\scripts\Run-FlightStutterMatrix.ps1 -Name XPT_full_flight_comparison -Pairs 3

```

This runs twelve flights in six fresh processes, AB/BA/AB flag order, with 10 GB cache, full scenery, the saved 2D graphics profile and temporary TelemFFB isolation. It uses the existing Flight transaction and passive recorder with no system tracing or video. Keep X-Plane foreground during each flight. Each maneuver must pass uninterrupted exposure, focus, actual 2D, QPC, complete-buffer and native-authority checks. Primary statistics exclude the first five seconds; activation evidence is retained. A landing precision miss remains a miss even if its timing is valid. The matrix stops at the first invalid session after verified recovery.

The [twelve-flight results](../../docs/compatibility/xplane-12.4.4-b1/full-flight-stutters-20260914/README.md) reproduced the baseline across all three launches. The flag lowered the observed >50 ms rate by about 27%, but residual stutters persisted. Both this workload and the paused camera benchmark measure different portions of the user's experience; neither establishes VR behavior.

A separate diagnostic observer is prepared for an existing `-TraceFlight` session after the restricted WPR host is ready:

```powershell

.\.venv\Scripts\python.exe .\scripts\Trace-FlightWindow.py --run 'D:\X-Plane 12\Output\performance-tests\XPT_diagnostic'

```

It watches the second calm flight's native-clock interval 90–140 seconds, roughly 70–120 seconds after unpause, without writing simulator controls or heartbeats. The exact QPC request/acknowledgement/stop timestamps determine the accepted diagnostic interval afterward. Exclude ten seconds after WPR start acknowledgement and everything after the stop request; keep this capture separate from untraced A/B rates. Recovery recognizes its owned segment records. This observer and the larger buffer passed offline checks, a worker-handoff recovery probe, and two live full-flight captures using the installed on-demand task. Both restored successfully. See the [WPR/VRAM diagnosis](../../docs/compatibility/xplane-12.4.4-b1/full-flight-stutters-20260914/WPR-VRAM.md).

## Retrospective profiler capture without elevation

`scripts/Capture-FlightHitch.py --run <existing-or-starting-TraceFlight-run>` observes an existing two-flight calm session using read-only status and dataref requests. It makes no control or heartbeat writes. Run it as a separate process; use the existing `Run-XPlaneTest.ps1` transaction for all simulator installation changes.

Passive recorder 0.1.1 adds `xpt/perf/hitch`, an eight-float readback: token, latched >50 ms count, one-based recorder row index, interval milliseconds, and four little-endian 16-bit pieces of the exact QPC end tick. It excludes paused frames and the first five seconds after unpause. The latch performs no disk I/O. The original 16-field performance snapshot is unchanged.

The observer requests at most one retrospective 240-frame Microprofile dump per flight. Record the native trigger, observed QPC and actual HTTP request start/end. Only frames before the first request support unperturbed-flight interpretation. Validate the trigger against the retained pacing CSV and verify that the exported profiler frames actually include it; Microprofile's published frame ring can lag the native callback.

The [six-flight diagnostic](../../docs/compatibility/xplane-12.4.4-b1/full-flight-stutters-20260914/DIAGNOSTIC.md) validated the native latch and immediate observer, capturing one 53 ms render-stage pause and detecting another 73 ms pause. The second dump ended before its trigger and is not causal evidence for that event. A 250 ms publication delay was added afterward; compilation/argument checks pass, but that delayed variant still awaits live validation. The diagnostic tests and all three installation restorations passed. The later installed-task retry succeeded; the WPR/VRAM diagnosis above supersedes the earlier elevation blocker and buffer-validation status.

## GPU memory observer

`scripts/Observe-GpuMemory.py --run <existing-or-starting-TraceFlight-run>` starts a bounded read-only NVIDIA sampler alongside the existing transaction. It records board-wide total/reserved/used/free FB memory and GPU utilization every 500 ms, with receipt QPC timestamps. It buffers rows in memory, stops its exact subprocess and writes the CSV after restoration. These counters are not X-Plane residency, per-process Windows budget, or exact instantaneous memory use. Keep WPR/NVIDIA diagnostic counts separate from untraced A/B rates. Both observers were live-validated on maximum and one-step-lower texture settings; all four flights were timing-valid.
