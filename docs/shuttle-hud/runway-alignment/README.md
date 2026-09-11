# Runway alignment — Rust 144

Release 144 aligns the HUD landing outline with the installed runway's marked landing surface. The former renderer used a fixed 300 ft width, a 15,000 ft length cap, a single elevation and a flat angular approximation from the aircraft position. At Edwards the configured runway is 44.20 m (145.01 ft) wide, and the old outline was too wide and vertically displaced.

The corrected renderer samples both runway edges at intervals of at most 250 m, converts those geographic surface points with `XPLMWorldToLocal`, and uses X-Plane's `world_matrix`, `acf_matrix` and `projection_matrix_3d`. Cockpit rays account for the actual eye position; the full-screen renderer uses the same 3D projection as the scenery. Terrain refreshes on runway selection, HUD-view selection and scenery reload. Invalid terrain or transform data suppresses the affected geometry. The read-only `fsim_hud/runway_projection_valid` reports available terrain and transforms; ordinary declutter still controls visibility.

## Displaced thresholds

The near edge is the landing threshold, measured from the configured pavement endpoint. The far edge is the opposite physical runway endpoint. Opposite-end threshold displacement does not subtract from the available rollout area. Approach guidance remains referenced to the same landing threshold.

| Runway | Width (m) | Near displacement (m) | Outline length in the configured coordinate model (m) |
|---|---:|---:|---:|
| KEDW 22L | 44.20 | 542 | 4,571.16 |
| KEDW 04R | 44.20 | 0 | 5,113.16 |
| KTTS 15 | 91.44 | 0 | 4,577.68 |
| KTTS 33 | 91.44 | 0 | 4,577.68 |

Coordinates, widths and displacements were checked against the two active custom airports' `apt.dat` runway rows. At Edwards the surrounding paved area extends beyond the runway's marked edges. At Kennedy, 305 m blast pads are outside the outline. The HUD represents the runway landing area.

## Verification

The [acceptance contract](ACCEPTANCE.md) covers centered, close, offset, banked and off-axis views, both directions at Edwards and Kennedy, native declutter, power/enable transitions, camera restoration and a short native glide. The [gallery](GALLERY.md) contains unmodified simulator captures with matching readbacks. These are initialized display checks and a three-second free glide, not new touchdown-performance trials.

All 39 workspace tests pass; one existing local-scenery database test remains ignored. Plugin Clippy with warnings denied and the optimized build pass. New independent projection tests compare the runway-ray route with direct world-to-clip matrix multiplication and check the configured width, displaced threshold, opposite endpoint, terrain slope and near-plane clipping. Existing Project regressions still pass for the landing path, 3,053 guidance/presentation frames and 24 display scenes; the latter use the offline flat geometry and do not validate the new native terrain projection.

No guidance equations, landing calibration, ACF, native HUD optics/meshes, source aircraft or landing-aid scenery changed. The existing light-landing rebound limitation remains documented in the [Rust 143 report](../rust-port/README.md). Visual checks used X-Plane 12.4.3, session-only graphics/plugins/art-controls safe mode, zero wind and temporary exclusion of the existing incomplete XPME base-mesh package. They do not establish VR behavior or a pixel-perfect match to arbitrary replacement scenery. The configured geographic lateral offsets use the existing local metric convention; the scenery transforms and heights come from X-Plane.

An initial setup captured the old build with an XPME missing-resource dialog because scenery was disabled after process startup. That session exited normally and was restarted with exclusion applied before launch. A subsequent old-build swap hit an orphaned Web API entry for the new diagnostic; the harness now filters that unavailable entry only when reading version 143. These are retained as setup issues, not accepted final-build checks.

See [validation-summary.json](validation-summary.json), [installation-audit.json](installation-audit.json) and [the clean shutdown excerpt](Log-144-clean-exit-excerpt.txt) for the final result and exact binary identity. The previous Rust 143 binary is retained locally under `Output/shuttle-runway-20260910/baseline-143.xpl` for restoration with X-Plane closed. The source build remains `cargo build -p shuttle-hud-rs --release`; the aircraft-local DLL is installed as `plugins/ShuttleHUD/64/win.xpl`.

Implementation: [runway geometry and projection](../../../plugins/shuttle-hud/src/runway.rs), [native integration](../../../plugins/shuttle-hud/src/runtime.rs), [scene generation](../../../plugins/shuttle-hud/src/scene.rs), [drawing](../../../plugins/shuttle-hud/src/graphics.rs). The transform conventions follow Laminar's [OpenGL drawing guidance](https://developer.x-plane.com/article/plugin-guidance-for-opengl-drawing/) and [world-to-local SDK definition](https://developer.x-plane.com/sdk/XPLMWorldToLocal/).

Simulator use only; this is not real-world flight guidance.
