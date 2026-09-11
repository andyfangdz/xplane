# Power-off 180 controller

Rust XPLM runtime for the [pure guidance crate](../../crates/poweroff180/src/guidance.rs).
The algorithm remains v7 and the 75-float snapshot remains protocol v1.
`xpt/rust_implementation = 1` identifies the translated plugin.

The ordered [snapshot schema](../../crates/poweroff180/snapshot.csv) defines
the CSV fields and SDK input sources. Cargo generates named field indexes,
the header, and the input lookup list; the Python harness reads the same schema.
Controller and HUD code use `protocol::field` constants, and both languages
check the frozen protocol v1 header to prevent accidental layout changes.

Runway direction and along/cross-track telemetry use shared
[`xplane-airports` geometry](../../crates/xplane-airports/README.md), with the
card's usable threshold, fixed midpoint latitude and original feet-per-degree
scale. The HUD uses the same axis/projection utilities for navigation legs in
nautical miles. The test card remains the runway authority.

The plugin starts inert. A complete staged `active-card.ini` is accepted only
while paused in the TorqueSim SR20; `effective-card.ini` is written for exact
readback. Start also requires the existing low-level attitude adapter to be
armed and the flight-path override to be released. An after-physics callback
records each frame, writes ordinary attitude targets/throttle/flaps, and stops
on the configured envelope, entry, timing, loading, wind or watchdog failure.

`xpt/heartbeat` writes refresh a monotonic timer even when the integer value
does not change. The watchdog runs while paused. Abort, disable and stop
release test authority; normal completion also pauses before supervisor
cleanup. Commands, callbacks, snapshot storage and diagnostics have owned
lifetimes and are unregistered on unload.

Use the [test harness](../../tools/flight-test-harness/README.md) for build,
temporary aircraft-local installation, flights and verified restoration.
The [Rust attitude helper](../poweroff180-attitude/README.md) preserves the
original v1.9 inner-loop behavior. It is built alongside guidance and the HUD
and separately hashed in each campaign's source manifest.
