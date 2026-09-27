# HAGS off: no demonstrated consistent benefit

Follow-up: [A separate system trace](HAGS-TRACE.md) directly identifies the blocking chain behind the two largest captured HAGS-off pauses. Those traced counts are kept separate from this untraced comparison.

Six post-reboot flights completed across three fresh simulator processes. All six passed native timing validity checks. Two of the three second flights still had long pauses. The flag `--gfx_no_defragment` stayed enabled throughout the HAGS comparison; this was not another flag-on/off test.

| Primary measure | HAGS on, before reboot | HAGS off, after reboot |
|---|---:|---:|
| Flights / fresh processes | 2 / 1 | 6 / 3 |
| Measured exposure | 5.17 min | 15.52 min |
| Intervals >50 ms | 7 | 15 |
| Intervals >50 ms per minute | 1.35 | 0.97 |
| Intervals >100 ms | 3 | 5 |
| Worst interval | 148.58 ms | 174.61 ms |
| Intervals >25 ms per minute | 12.18 | 14.69 |

The observed >50 ms rate was about 29% lower after reboot, but the worst pause was longer and shorter delays were more frequent. One prior launch, three later launches, and substantial variation between launches do not establish a repeatable HAGS benefit. HAGS off did not eliminate the reproduced stutter.

| HAGS-off launch | First flight >50 ms / worst | Second flight >50 ms / worst |
|---|---:|---:|
| `XPT_HAGSOFF_DXGI_02_20260915` | 0 / 31.07 ms | 0 / 49.94 ms |
| `XPT_HAGSOFF_DXGI_03_20260915` | 0 / 32.05 ms | 9 / 174.61 ms |
| `XPT_HAGSOFF_DXGI_04_20260915` | 0 / 30.35 ms | 6 / 127.41 ms |

## What was controlled

All compared flights used the flag, 10 GB disk texture cache, maximum texture detail, full custom scenery, User2D rendering, the same temporary TelemFFB isolation, the same calm SR20 power-off 180 card, and NVIDIA DXGI presentation override 1. Source manifests, including native binaries and original aircraft, match exactly. The NVIDIA driver stayed at 616.92 and the display at 2560×1440/59 Hz. The existing harness tests passed before flight. No system trace, GPU observer, or video was active in these timing runs.

DxDiag verified RTX 5090 HAGS inactive after reboot (`Enabled:False`); the registry override is `HwSchMode=1`. Windows also updated from 26200.9278 to 26200.9445 during that reboot. Restarting Windows and the scenery streamer changed cache/runtime state as well. Therefore a causal HAGS comparison still needs HAGS-on repetitions on this same updated Windows build, with the flag and all other test settings held fixed. Another Windows restart requires the user to be ready; none was performed after this series.

## Measurement and recovery limits

These are native after-physics callback intervals, not display presentation or VR-compositor timing. Primary statistics exclude the first five unpaused seconds; activation-window measurements remain in the JSON. All six flights stayed in foreground 2D with continuous control authority and complete, monotonic QPC records. First-flight landings passed; second-flight landings missed the touchdown-speed criterion, separately from their valid timing records.

The pre-reboot diagnostic `XPT_DXGI_B1_20260914` and final post-reboot launch retain a shutdown recovery guard error: the guard refused filesystem moves while X-Plane was still exiting. In both cases the process then exited and the existing recovery entry point verified complete restoration. Their individual timing records remain usable diagnostic evidence, but these sessions are not clean runner passes. The result JSON preserves their `complete:false` and original error records; this is not represented as a clean completed A/B campaign.

`XPT_HAGSOFF_DXGI_01_20260915` is excluded: XPlane Map Enhancement had not restarted, its virtual terrain folders were absent, and startup stopped before measurement. The scenery app was started normally with the user's approval, its mounts verified, and the same scenery loaded in subsequent runs. No scenery package was removed to obtain a passing run.

## Current settings and evidence

The flag remains in xOrganizer's default launch arguments. Cache remains 10 GB and texture detail remains maximum. All three persistent preference hashes match their pre-test values. All simulator/plugin/scenery changes and temporary NVIDIA overrides are restored; HAGS remains off pending a decision about the next restart. X-Plane is closed and the reusable tracing task is idle. No further HAGS or Windows setting was changed.

The completed series does not justify claiming HAGS is the stutter fix. To identify its independent effect, repeat the same three-process sequence with HAGS on under Windows 26200.9445. Keep the flag on. The previous memory-wait evidence remains relevant, but these untraced runs do not identify the exact cause of each new pause.

Machine-readable results, per-flight table, preflight, restoration audit, NVIDIA readback and the analysis script are retained in [hags-evidence](hags-evidence/). Complete raw runs are under `D:\X-Plane 12\Output\performance-tests\`; the native CSV records are preserved there. The scratch analysis is `V:\src\xplane\build\stutter-hags-20260915`.
