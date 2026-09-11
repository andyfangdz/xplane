# Shuttle HUD reports and evidence

The [Shuttle HUD project](../../plugins/shuttle-hud/README.md) is a native Rust
plugin in this repository's Cargo workspace. Read the [source](../../plugins/shuttle-hud/src)
and [architecture](rust-port/ARCHITECTURE.md) for its renderer, landing guidance,
display state and simulator integration.

Current release **144** corrects runway scenery alignment and displaced-threshold
geometry, checked in X-Plane 12.4.3 on 10 September 2026. Start with the
[alignment report](runway-alignment/README.md) and [native comparisons](runway-alignment/GALLERY.md).
The [public gallery](https://shuttle-hud-gallery.halfteatree.chatgpt.site/)
shows the release-143 approach-to-rollout captures with diagnostic readbacks.

| Project report | Scope |
| --- | --- |
| [Release 144: runway alignment](runway-alignment/README.md) | Scenery projection, dimensions, displaced thresholds and native view checks |
| [Release 143: native validation](rust-port/README.md) | Rust tests, gallery, flight regressions and SDK lifecycle |
| [Release 142: symbology](reports/shuttle-symbology-20260910/README.md) | Phase gallery, director transition, gear/flare cues, contact latches and rollout |
| [Release 136: flare and ball/bar](reports/shuttle-flare-20260910/README.md) | Landing calibration, native landing lights and recorded approach |
| [Release 127: native collimation](reports/shuttle-correction-20260910/README.md) | Optical correction, chute tests and recorded approach |

Each report identifies the project release tested. Flight results, screenshots
and telemetry retain those release numbers. Display and lifecycle checks passed;
several light-weight landings narrowly exceeded the established rebound limit.
Those failures remain in the reports and raw traces.

## Reproduce and inspect

- [Build and install](../../plugins/shuttle-hud/README.md#build-and-install)
- [Regression fixtures and tolerances](../../plugins/shuttle-hud/tests/fixtures/README.md)
- [Native API contract](../../plugins/shuttle-hud/validation/api-contract.json)
- [Current acceptance status](../../plugins/shuttle-hud/ACCEPTANCE.md)
- [Display requirements](reports/shuttle-symbology-20260910/ACCEPTANCE_SPEC.md)
- [Symbology research](reports/shuttle-symbology-20260910/RESEARCH.md)

The root build script runs the Rust workspace tests and produces the plugin.
Native verification uses X-Plane and a dedicated trial aircraft. Archived
orchestration under `plugins/shuttle-hud/validation` requires the original local
flight-test harness and is never installed in the release aircraft.

Recompute flight metrics with `python analyze_trial.py 302` in `rust-port`, or
`python analyze_trial.py 206` in `reports/shuttle-symbology-20260910`. Included CSV
traces, result readbacks and analyses retain every landing and its outcome.

## Evidence scope

Linked media, telemetry, failed-test outcomes, installation audits and recorded
build hashes are included. Release-143 and release-144 reports also retain their
tested Rust binaries. Historical manifests describe bytes used in those
experiments; they are not inventories of the current source tree. Implementation
links lead to this project's Rust source.

Full logs, rejected captures, aircraft backups and the local development archive
remain under the original X-Plane `Output` directory. Aircraft textures/meshes,
downloaded manuals and third-party tools are not redistributed. The installer
generates the derivative from a separately obtained Space Shuttle-FX-V12 aircraft.

The plugin uses original vector lettering. No F-SIM code, fonts or artwork is
included. NASA manuals, F-SIM documentation and reference videos link to their
original sources. Screenshots and videos are simulator captures; simulator and
aircraft artwork remain the property of their respective owners.
