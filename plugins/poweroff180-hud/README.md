# Power-off 180 HUD

The accepted G1000-style HUD v5 draws in a click-through X-Plane 12.4.4
flight-overlay window. The scene uses 1920 × 1080 design coordinates and
scales into the window's current bounds. Native panel polygons draw the
instrument geometry. Nested scissors clip the rolling tapes.

The plug-in loads X-Plane's installed Roboto Regular through the SDK font API.
Text uses the ascent and character advances measured from the original 44-pixel
Arial strike. Fixed labels retain each letter at its original position. The SDK
places dynamic text a word at a time, with its original shadow and anchor. The fixed panels, control frames, path grid, bank
scale, aircraft symbol and rotating compass card use retained drawings.

`runtime.rs` samples instruments and owns the paused FMS initialization
commands. `hud.rs` owns SR20 display policies, projection visibility and trends.
Shared points, rotations, projection math and stroke quads live in
[`xplane-hud`](../../crates/xplane-hud).

The public diagnostics retain `xpt/video_hud/version = 5`, font readiness,
frame count, navigation readiness and the loaded ACF's full-flap speed limit.
`xpt/video_hud/rust_implementation = 1` distinguishes this port.

The full-flap white-band limit comes from `acf/_Vfem_kts` in the loaded ACF
(104 KIAS in the tested SR20); the generic first-detent `acf_Vfe` is not used
for that mark. GPS course/CDI and magnetic heading come from pilot-side
instruments. Drawing never changes navigation or flight controls.

Run `python tools/xphost/tools/hud_panel_regression.py --out tmp/hud-panels --font "<X-Plane>/Resources/fonts/Roboto-Regular.ttf"` with the built host and both release HUD plug-ins to check the configured snapshot, native text bounds, retained artwork and tape clip regions.

Build and validate through the [flight-test harness](../../tools/flight-test-harness/README.md).
See [display semantics](../../tools/flight-test-harness/CUSTOM_HUD.md) and the
[migration report](../../docs/poweroff180/README.md).
