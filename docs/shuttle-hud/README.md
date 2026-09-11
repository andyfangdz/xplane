# Shuttle HUD reports and evidence

Current release **143** is the Rust migration, checked in X-Plane 12.4.3 on 10 September 2026.

Start with the [Rust migration report](rust-port/README.md), [native gallery](rust-port/GALLERY.md) and [architecture](rust-port/ARCHITECTURE.md). It includes C++ parity, both native landing regressions and current lifecycle/installation evidence.

The material below describes the earlier C++ releases and original repository import.

The [release-142 illustrated report](reports/shuttle-symbology-20260910/README.md)
includes all 18 native screenshots, phase descriptions, source comparisons,
the complete six-flight landing ledger, and the remaining approximations.
The [HTML gallery](reports/shuttle-symbology-20260910/REPORT.html) can be opened
locally after cloning this repository; GitHub displays its source.

| Report | Scope |
| --- | --- |
| [Release 143: Rust migration](rust-port/README.md) | Current Rust implementation, native gallery, parity, flight and lifecycle checks |
| [Release 142: symbology](reports/shuttle-symbology-20260910/README.md) | Release-142 phase gallery, flight-director transition, gear/flare cues, contact latches and rollout |
| [Release 136: flare and ball/bar](reports/shuttle-flare-20260910/README.md) | Historical landing calibration, native landing lights and recorded approach |
| [Release 127: native collimation](reports/shuttle-correction-20260910/README.md) | Historical optical correction, chute tests and recorded approach |

The current display and lifecycle checks passed. Landing results remain mixed:
several light-weight runs narrowly exceeded the existing rebound threshold.
Those failures remain in the report and raw traces. Importing this work did not
change the C++ runtime, optics, flight guidance, or validation control laws.

## Reproduce and inspect

- [Plugin source, build and installation](../../plugins/shuttle-hud/README.md)
- [Predeclared acceptance contract](reports/shuttle-symbology-20260910/ACCEPTANCE_SPEC.md)
- [Detailed results](reports/shuttle-symbology-20260910/RESULTS.md)
- [Reference research](reports/shuttle-symbology-20260910/RESEARCH.md)
- [Installed release manifest](reports/shuttle-symbology-20260910/release-manifest-142.json)
- [Original tested binary](reports/shuttle-symbology-20260910/build-142/win.xpl)
- [Final shutdown excerpt](reports/shuttle-symbology-20260910/Log-142-clean-exit-excerpt.txt)
- [Repository import verification](IMPORT.md)

All six current flight traces, result readbacks and derived analyses are next to
the current report. To recompute a flight, run `python analyze_trial.py 206`
from that report directory. The archived validation orchestration scripts are in
`plugins/shuttle-hud/validation/archive`; they describe the original local test
environment and require its flight-test harness and temporary trial aircraft.
They are not installed into the release aircraft.

## Provenance and archive boundaries

The repository includes all media linked by the three reports, plus current
telemetry, failed-test outcomes, source/build hashes and the exact tested 142
binary. That binary has SHA-256
`a9f0ee3640299255c46b52df0ff1c442c977e907de31cf74d9082d9d8f3ff83d`.
HTML reports gain an archive-scope note; Markdown versions contain the same
substantive report content. Simulator log excerpts retain the HUD lifecycle and
shutdown lines; full logs remain local and their original hashes are retained.

Historical paths in telemetry, manifests and archived scripts identify the
original experiment. The complete development archive remains under the original
X-Plane `Output` directory, including rejected screenshots and aircraft backups.
The source aircraft, its textures/meshes, downloaded manuals and third-party
tools are not redistributed here. The installer regenerates the derivative from
the user's separately obtained Space Shuttle-FX-V12 installation. The locally
installed aircraft remains usable; release 143 replaces its plugin binary and user documents while preserving the aircraft inputs.

The plugin uses original vector lettering. No F-SIM code, fonts or artwork is
included. NASA manuals, F-SIM documentation and reference videos remain linked
to their original sources. Screenshots and videos are simulator captures;
simulator and aircraft artwork remain the property of their respective owners.
