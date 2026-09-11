# Power-off 180 HUD

Rust port of the accepted native G1000-style HUD v5. `scene.rs` and its modules
produce display commands without SDK writes. `graphics.rs` owns the cached
Arial atlas and the OpenGL boundary; `runtime.rs` samples instruments and owns
the paused FMS initialization commands. Geometry, tape spans, rolling carries,
colors and clipping follow the original display. Tailwind labeling is corrected.

`hud.rs` owns SR20 display policies, projection visibility, instrument helpers,
and trends. Shared points, rotations, projection math, and stroke quads live in
[`xplane-hud`](../../crates/xplane-hud); scoped GL state restoration comes from
[`xplane-plugin`](../../crates/xplane-plugin). The guidance crate owns no HUD code.

The public diagnostics retain `xpt/video_hud/version = 5`, font readiness,
frame count, navigation readiness and the loaded ACF's full-flap speed limit.
`xpt/video_hud/rust_implementation = 1` distinguishes this port.

The full-flap white-band limit comes from `acf/_Vfem_kts` in the loaded ACF
(104 KIAS in the tested SR20); the generic first-detent `acf_Vfe` is not used
for that mark. GPS course/CDI and magnetic heading come from pilot-side
instruments. Drawing never changes navigation or flight controls.

Build and validate through the [flight-test harness](../../tools/flight-test-harness/README.md).
See [display semantics](../../tools/flight-test-harness/CUSTOM_HUD.md) and the
[migration report](../../docs/poweroff180/README.md).
