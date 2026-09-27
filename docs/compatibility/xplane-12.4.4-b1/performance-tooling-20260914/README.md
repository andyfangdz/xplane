# Performance investigation tooling — validated

**Later coverage correction:** The [full-flight investigation](../full-flight-stutters-20260914/README.md) found that LIVE05 retained only the last approximately 7.60 seconds of CPU/scheduling events within its 20-second measured window because the system buffer wrapped. Its zero-loss counter and complete presentation-event coverage do not establish full-window kernel coverage. The original presentation and native timing results below remain separate from CPU attribution.

The five requested improvements are implemented and validated. Recovery, two buffered landing regressions, synchronized Windows presentation capture, all six recorder A/B segments and the final two-launch flag comparison passed. The installation is restored and the elevated tracing helper is closed. These results validate the investigation tools; they do not establish a general stutter fix.

| Improvement | Implementation | Validation |
|---|---|---|
| Buffered recording | Native numeric rows in preallocated, prefaulted memory; CSV formatting and export after the measured interval. Landing controller is 0.8.5; guidance is unchanged. | Valid normal-profile recording, two passing landing regressions, and 6/6 valid recorder A/B segments. |
| Synchronized tracing | Integer Windows QPC anchors, native interval boundaries, fixed WPR profile and offline PresentMon decoding. | Revised profile captured all 1,460 X-Plane presentations in the native QPC window, with zero reported event loss. |
| Readiness and recovery | Direct flight JSON at launch; aircraft/runway, scenery, callback, pause, focus and actual 2D checks. Exact-process shutdown, worker handoff gate and atomic-record retries. | Preparation, launch, worker-handoff and owner-loss recovery probes restored the original installation. Failed development attempts are retained. |
| Repeatable short workload | Paused KCDW 22 SR20 camera sweep, with warmup and repeated segments in one loaded scene. | Native position, focus, frame count and buffer checks passed. This workload does not test moving-aircraft streaming or VR. |
| Repeatable comparisons and evidence | Three alternating AB/BA pairs; fresh processes for flag/cache tests; separate load timing; failed attempts retained; native stutter markers. | All three recorder pairs pass; synthetic marker aligned; final fresh-process flag pair passes. |

## What the live tests established

`XPT_PERF_LIVE_05_20260914` retained all 1,460 native frames and matching X-Plane presentations in the measured 20-second QPC window. PresentMon mean interval was 13.698 ms, p99 20.321 ms, maximum 26.651 ms and zero intervals over 50 ms. Native callback mean was 13.695 ms and maximum 28.484 ms. Both WPR collectors reported zero lost events. The explicitly labeled synthetic marker was received at QPC 5892208585972, 5.193 seconds into measurement. Native QPC frequency was 10 MHz.

The earlier LIVE04 record is rejected as normal-profile evidence: its log explicitly says Safe Mode limited graphics, aircraft, scenery, plugins and art controls. Return accepted Safe Mode despite the attempted No Thanks selection. Its recorder-integrity checks alone did not detect that profile change; startup now rejects the Safe Mode banner automatically. LIVE02 remains rejected for losing focus near its end. Both attempts and their original errors remain retained.

The broad WPR profile saved an 8,968,470,528-byte ETL after about 235 seconds of export work. The client timed out after 60 seconds, so the attempt remains failed despite eventual trace completion and reported zero dropped events. PresentMon recovered DWM frames, not X-Plane frames. Those data are not substituted for simulator presentation timing.

The replacement `XPT-Performance.wprp` uses 512 MB of system buffers and 256 MB of event buffers. It collects CPU/scheduling/file/disk information and filtered presentation providers based on [PresentMon 2.5.1's source](https://github.com/GameTechDev/PresentMon/blob/v2.5.1/PresentData/PresentMonTraceSession.cpp). It excludes the DxgKrnl Performance keyword, as that source directs. Its LIVE05 ETL was 627,048,448 bytes and saved in about 37 seconds. The host checks the profile hash, uses a distinct named instance per run, saves completion evidence directly, and handles repeated stop requests. Export waits allow up to 300 seconds. The user approved showing the revised administrator prompt again; tracing passed, and the helper subsequently [stopped cleanly](wpr-host-stopped.json).

The four deliberate recovery probes verified 122 preference files, 11 aircraft-state files, 179 scenery entries and five links, with original source hashes matching and temporary helpers removed. An intentional failure is a recovery pass, not a performance pass.

The first fresh-process matrix exposed a worker handoff failure with a Windows file-already-exists error. Its worker identity was still unpublished; that worker later reached the next simulator process, where WPR rejected the mismatched simulator identity. The original error lacked a stack, so its precise failing call is not established. The complete matrix remains failed. Record updates now retry atomic replacement while preserving the previous record on a sharing conflict. Workers wait for their recovery identity before contacting the simulator, and the launcher also stops a newly created worker directly on handoff failure. An intentional failure at that exact handoff restored the installation, left no worker, and produced no API/status or capture artifacts.

The next matrix retained a valid flag-off segment and rejected its flag-on startup when Python's status-file replacement encountered a Windows sharing conflict. Its traceback establishes that failure directly. Python record replacement now has the same retry protection; real held-reader tests cover both recovery after release and preservation of the previous record when blocking persists. Both failed matrices and their restoration evidence remain retained.

The [landing regression](landing-regression.json) passed both calm flights with uninterrupted native authority and complete CSV exports (14,582 and 14,658 frames). Touchdown was at 1,029.49 and 1,080.37 feet, 66.10 and 66.12 KIAS, with physical sink rates of 55.41 and 43.06 feet/minute. These used the reduced 2D test profile and temporary TelemFFB isolation; they verify the logger regression, not normal-profile stutter reduction. Restoration completed at 10:43 EDT.

## Recorder overhead

`XPT_PERF_OVERHEAD_20260914` passed all six 40-second measured segments, with zero reported event loss and no presentation intervals over 50 ms. The same WPR profile and camera workload ran on both sides. Values below are PresentMon start-to-start intervals; differences are buffered minus recorder-off.

| Pair | Off mean ms | Buffered mean ms | Difference ms | Off p99 ms | Buffered p99 ms |
|---|---:|---:|---:|---:|---:|
| 1, off then buffered | 13.567 | 12.290 | -1.277 | 19.450 | 18.048 |
| 2, buffered then off | 11.693 | 11.779 | +0.086 | 16.469 | 16.461 |
| 3, off then buffered | 11.735 | 11.701 | -0.035 | 16.727 | 16.253 |

This run does not show a consistent frame-time penalty from native storage. It also does not prove zero overhead: the first pair shows a substantial warmup trend, and three pairs cannot resolve a very small cost reliably. The benchmark measures the incremental frame-collection/storage path while WPR and the camera controller remain active. It does not measure the cost of all instrumentation, or directly compare the old per-frame CSV writer with the new buffer. The installation was restored at 11:13 EDT.

Offline checks: 30 Python tests, the Rust workspace suite (one pre-existing ignored test), release build, PowerShell process and atomic-record checks, and `git diff --check` passed. The fixed WPR profile also passed Windows' parser and the live checks above. Compact live evidence is retained in [live-validation.json](live-validation.json).

## Fresh-launch flag comparison

`XPT_PERF_MATRIX_FINAL_20260914` passed both launches with identical source hashes, full scenery, User2D, 10 GB cache and a 20-second measured sweep after 20 seconds of warmup. Both collectors reported zero lost events. Native buffers retained all 1,422 and 1,566 frames, with no integrity violations. Both installations restored successfully; the complete wrapper returned exit code 0.

| Flag | Mean interval ms | p99 ms | Maximum ms | Intervals >50 ms | Scene load seconds |
|---|---:|---:|---:|---:|---:|
| Off | 14.061 | 20.399 | 26.949 | 0 | 218.187 |
| On | 12.771 | 18.006 | 25.000 | 0 | 218.906 |

These are PresentMon CPU-start intervals. Flag-on was 1.290 ms lower in this pair, but one off-then-on pair does not separate the flag from order, warmup or ordinary run variation. Neither side reproduced the earlier >50 ms stutters in this short parked workload. The 0.720-second load difference is descriptive; both runs used the same cache capacity. The Cache 0/10 GB matrix option is implemented but was not measured in this tooling validation. See [flag-comparison.json](flag-comparison.json) and [live-validation.json](live-validation.json).

## Run and inspect

Use the commands and requirements in [PERFORMANCE.md](../../../../tools/flight-test-harness/PERFORMANCE.md). All installation changes still go through `Run-XPlaneTest.ps1`. The elevated helper accepts only the fixed trace profile for a verified test session; X-Plane and Codex stay unelevated.

Each attempt retains its settings, source hashes, startup information, native data, collector logs, assessment and restoration record. QPC hitch ranges and marker timestamps let subsequent analysis locate the matching interval in an ETL. Inspect CPU stacks, scheduling, GPU events and I/O before attributing a stutter to any component.

Native callback intervals, presentation intervals, approximate GPU timing, load time and landing precision remain separate measurements. The primary load metric ends at the paused, plugin-ready scene; subsequent focus wait is separate. Startup dialogs invalidate a load-time comparison. Cache state can evolve between launches, so these are not controlled cold-disk tests.

The user-approved persistent defaults remain the no-defragment launch flag and 10 GB cache. Final [hash verification](settings-verification.json) confirms the xOrganizer database, its default preference profile and the active X-Plane preferences all match their approved values. All temporary rendering profiles, plugin isolation and scenery changes were restored. No X-Plane, harness worker or owned WPR helper process remains. No additional permanent graphics change is justified by this tooling validation.

All 17 retained attempt records: [attempts.json](attempts.json). Development history: [WORKLOG.md](WORKLOG.md). Offline test output: [offline-tests.log](offline-tests.log).
