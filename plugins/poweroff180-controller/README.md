# Power-off 180 controller

Rust XPLM runtime for the [pure guidance crate](../../crates/poweroff180/src/guidance.rs).
The algorithm remains v7 and the 75-float snapshot remains protocol v1.
`xpt/rust_implementation = 1` identifies the translated plugin.

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
