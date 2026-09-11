# Standard-unit conversion validation

Validated on September 11, 2026 after removing `xplane-units`. Plugins and shared
geometry now depend on `uom` 0.38 directly, using its standard unit definitions.
The rounded conversion-factor compatibility module and custom unit definitions
were removed. Control gains, landing limits and protocol layouts were retained.

## Recorded regression tests

The existing numerical fixtures remain unchanged. No baseline regeneration was
needed. Replay tolerances describe the physical output, and each test reports
its maximum observed errors.

| Comparison | Coverage | Largest relevant error | Limit |
|---|---|---:|---:|
| Maneuver bank command | 88,586 frames, seven winds | 4.37e-8 degrees | 0.001 degree |
| Maneuver turn lead | Same guidance replay | 3.70e-7 ft | 0.01 ft |
| Attitude floating diagnostics | 12,000 full flight-loop frames | 1.91e-6 | Existing 4e-6 |
| Shuttle path height | 737 path samples | 3.33e-9 m | 0.001 m |
| Shuttle guidance angle | 3,053 heavy/light frames | 1.89e-10 degrees | 0.001 degree |
| Shuttle drawing endpoints | 24 scenes, 10,163 segments | 4.25e-9 pixels | 0.01 pixel |

Guidance control ratios allow 0.00001 and speeds/accelerations allow 0.001 in
their documented units. Flight phases, abort reasons, ownership, gear/latches,
event timing and scene topology/visibility still match exactly. A frozen path
sample within one micrometre of a geometric join may use either adjacent
segment; its height, slope and curvature must still pass, and separate checks
enforce both sides of the current join. See the
[guidance replay](../../crates/poweroff180/tests/replay.rs) and
[Shuttle fixture notes](../../plugins/shuttle-hud/tests/fixtures/README.md).

`Test-Harness.ps1` passed: 64 Rust tests including one doctest, one local-scenery
test intentionally ignored, 13 Python tests, PowerShell parsing, process-identity
rejection checks and release builds of the three SR20 helpers. Workspace Clippy
with warnings denied and formatting checks also passed.

## Fresh simulator flights

**6/6 flights passed; 6 were measurement-valid.** The smoke campaign flew
fresh TorqueSim SR20 loads twice each in calm air, a 10 kt left crosswind and a
15 kt headwind at KCDW runway 22. It used the documented loading and automated
rendering profile, with `TelemFFB-XPP` temporarily isolated. Each flight included
the existing three-second supervision-gap probe. These six flights used one
dedicated simulator process.

Acceptance limits stayed at 1,000–1,200 ft touchdown distance, 63–67 KIAS,
200 fpm maximum physical descent, 7.5 degrees peak flare pitch and 2.8 degrees/s
peak flare pitch rate, plus the existing alignment, loading, power, authority,
roundout-height and rebound checks.

| Flight | Touchdown ft | KIAS | Physical sink fpm | Peak pitch degrees | Peak pitch rate degrees/s | Result |
|---|---:|---:|---:|---:|---:|---|
| calm-01 | 1029.7 | 66.08 | 55.2 | 6.85 | 2.47 | Pass |
| calm-02 | 1074.6 | 66.14 | 44.8 | 6.74 | 2.46 | Pass |
| cross_left10-01 | 1067.0 | 66.08 | 128.1 | 6.33 | 2.06 | Pass |
| cross_left10-02 | 1138.7 | 64.72 | 20.8 | 7.31 | 2.47 | Pass |
| head15-01 | 1064.6 | 64.99 | 170.6 | 7.35 | 2.71 | Pass |
| head15-02 | 1097.9 | 65.27 | 195.0 | 7.06 | 2.77 | Pass |

The [full flight report](verification/standard-units-20260911/report.md)
retains repeat overlays and any diagnostic divergence flags. Passing the
landing limits is distinct from matching another flight trajectory.

The crosswind pair exceeded the pitch-spread diagnostic (5.8 degrees at
80.9 seconds after power cut); the headwind pair exceeded the physical-sink
spread diagnostic (164.3 fpm at 80.0 seconds). The calm pair exceeded neither
diagnostic. Both landings in each pair passed their acceptance checks.

## Restoration and evidence

The harness verified restoration of the original aircraft, scenery,
preferences, aircraft state and isolated plugin. Every flight released
controller authority with the simulator paused and throttle idle before
supervisor cleanup. No runner or recovery errors were reported.

The [evidence package](verification/standard-units-20260911/) includes
compressed native traces, per-flight results and assessments, setup and
terminal readbacks, configurations, source/build hashes, restoration
results and plots. All compressed traces were decompressed and reassessed;
every saved assessment reproduced exactly. Current source and helper
binary hashes matched the campaign manifest. The
[validation record](verification/standard-units-20260911/validation.json)
records frame counts and checks; the
[SHA-256 index](verification/standard-units-20260911/evidence-sha256.json)
covers the retained evidence bytes.

Complete originals remain at `D:/X-Plane 12/Output/performance-tests/XPT_uom_20260911_132941`. The historical
[Rust migration report](README.md) retains its separate 14-flight baseline
and the exact source commit it validated.

From `tools/flight-test-harness`, with X-Plane closed:

```powershell
.\scripts\Test-Harness.ps1
.\Run-XPlaneTest.ps1 -Config .\configs\smoke.json -IsolateGlobalPlugin TelemFFB-XPP
```
