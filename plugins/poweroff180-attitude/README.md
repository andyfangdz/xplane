# Rust attitude helper

Temporary, aircraft-local bank, pitch and yaw control for the power-off 180 test
harness. This translates the original v1.9 adapter without changing its gains,
four modes, axis limits, contact latch, or release conditions. It retains the
`sr20g6/test_controller/*` datarefs and adds `rust_implementation = 1`.

The before-physics loop supplies ordinary joystick ratios. It releases only the
axis overrides it acquired, including on disarm, pause, replay, invalid timing,
aircraft mismatch, disable and unload. The separate guidance plugin owns the
supervisor watchdog and maneuver envelope.

Build and deploy with [the harness](../../tools/flight-test-harness/README.md).
It installs this helper only for the test session and verifies restoration.

This plugin and its [pure controller](../../crates/xplane-attitude/NOTICE.md) are
GPL-3.0-or-later because they derive from the original ArduPilot-based adapter.
