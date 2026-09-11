# Rust plugin architecture

The shipping plugin is the Cargo package `shuttle-hud-rs`, version `0.144.0`.
It produces a Windows x64 `cdylib` using the repository's release profile and
the same `xplane-plugin`, `xplane-sdk-sys` and `windows-sys` dependencies as the
other native plugins.

| Component | Responsibility |
| --- | --- |
| [config.rs](../../../plugins/shuttle-hud/src/config.rs) | Validate the existing optics file and parse the four runway definitions |
| [math.rs](../../../plugins/shuttle-hud/src/math.rs) | Earth/view angles, projection, clipping limits and unit conversions |
| [guidance.rs](../../../plugins/shuttle-hud/src/guidance.rs) | Accepted landing path, flare targets, gear/speedbrake latches and chute inflation curve |
| [presentation.rs](../../../plugins/shuttle-hud/src/presentation.rs) | Display phases, five-second director transition, contact latches, gear timers and declutter |
| [glyphs.rs](../../../plugins/shuttle-hud/src/glyphs.rs), [scene.rs](../../../plugins/shuttle-hud/src/scene.rs) | Original vector lettering and symbol segments, shared by cockpit and full-screen views |
| [graphics.rs](../../../plugins/shuttle-hud/src/graphics.rs) | Clipped stroke quads through X-Plane's OpenGL compatibility bridge and native HUD panel region |
| [runtime.rs](../../../plugins/shuttle-hud/src/runtime.rs) | Sample native state, update the pure models, publish diagnostics and manage the temporary view/chute state |

Source lives in [plugins/shuttle-hud/src](../../../plugins/shuttle-hud/src).

## Shared SDK ownership

The plugin uses three reusable owners from
[xplane-plugin](../../../crates/xplane-plugin/src): `DrawCallback`, `OwnedDataRef`
and `TerrainProbe`. Each releases its SDK registration or handle on `Drop`.
`OwnedDataRef` keeps callback storage at a stable heap address and unregisters
the accessor before freeing that storage. Its getters read independent `Cell`
values, so an SDK readback cannot reborrow the plugin's main state. Non-finite
external float writes are ignored; the HUD's existing brightness bounds remain.

The plugin also reuses `PluginStateSlot`, `FlightLoop`, `Command`, `PluginMenu`,
metadata exports, logging and native path/coordinate helpers. Small shared API
extensions support scalar type selection, single array elements, existing
command interception, command execution and local-to-world conversion.

All runtime state lives in the existing thread-local `PluginStateSlot`.
Reentrant callbacks skip mutation when the outer callback owns the state.
No worker thread performs XPLM or graphics calls. The draw callback and new
native handle owners cannot be sent across threads.

Startup validates configuration before display registration. A partially
constructed runtime owns its successfully registered resources, so a later
startup error drops them. Normal shutdown restores any acquired chute area and
full-screen view settings, unregisters drawing and flight-loop callbacks,
destroys the menu, unregisters command handlers and exported datarefs, and
destroys the probe. A second aircraft does not receive Shuttle writes.

## Native integration and regression coverage

The `fsim_hud` command names, existing diagnostic names/types, plugin signature,
native atlas region and rendering conventions are retained. The display name
is **Shuttle HUD Rust**. `fsim_hud/version` is 144 and
`fsim_hud/rust_implementation` is 1. Only the primary aircraft's load message
resets presentation state; AI-aircraft load messages are ignored.

The [public API contract](../../../plugins/shuttle-hud/validation/api-contract.json)
records 47 datarefs with scalar types and writability, plus eight handled commands.
The [Rust regression suite](../../../plugins/shuttle-hud/tests/regression.rs)
consumes frozen project results described in the
[fixture guide](../../../plugins/shuttle-hud/tests/fixtures/README.md).

The Rust regression tests check 3,053 recorded heavy/light frames, a path sweep
including both segment joins, and every symbol segment and clipped endpoint
in 24 cockpit/full-screen states. Integer state and segment topology must agree
exactly; numeric tolerance is `1e-8 + abs(expected) * 2e-12` in the quantity's
native units. Simulator float `atan2` and camera-matrix arithmetic retain their float
rounding before promotion to double precision. Native simulator tests cover
the SDK, OpenGL and aircraft integration that pure fixtures cannot exercise.

The project keeps the existing flight calibration and approximations. It
does not introduce Shuttle GPC/TAEM software, a new flight model, a new font or
new landing-light assets. The temporary validation pilot belongs exclusively
to the test aircraft and is never installed in the release aircraft.

Runway outlines use configured endpoints, width and displaced landing thresholds.
The runtime samples scenery terrain and transforms points using X-Plane's native
eye/camera matrices. Separate geometry tests check direct world-to-clip projection
and runway dimensions. The [alignment report](../runway-alignment/README.md)
records native checks at Edwards and Kennedy.
