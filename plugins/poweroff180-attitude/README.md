# Rust attitude helper

Temporary, aircraft-local bank, pitch and yaw control for the power-off 180 test
harness. Version 1.9.1 restores the original C++ adapter's **75 ms** maximum
frame interval. The initial Rust port incorrectly used the inner PID's **50 ms**
integration cap as the release threshold, resetting the controls on the
approximately 50.25 ms frames observed in X-Plane 12.4.4 beta 1.
Gains, four modes, axis limits, contact latch and the PID integration cap are
unchanged. It retains the `sr20g6/test_controller/*` datarefs, with
`rust_implementation = 1` and `version_patch = 1` identifying this fix.

The before-physics loop supplies ordinary joystick ratios. It releases only the
axis overrides it acquired, including on disarm, pause, replay, invalid timing,
aircraft mismatch, disable and unload. The separate guidance plugin owns the
supervisor watchdog and maneuver envelope.

Timing tests cover 50.25, 60 and 75 ms continuity, and verify release for invalid
timing and intervals above 75 ms. The frozen C++ replay remains unchanged; its
original input set skipped the interval between 50 and 75 ms.

Build and deploy with [the harness](../../tools/flight-test-harness/README.md).
It installs this helper only for the test session and verifies restoration.

This plugin and its [pure controller](../../crates/xplane-attitude/NOTICE.md) are
GPL-3.0-or-later because they derive from the original ArduPilot-based adapter.
