# Power-off 180 controller

Rust XPLM runtime for the [pure guidance crate](../../crates/poweroff180/src/guidance.rs).
Guidance v8 adds a bounded late-flare float correction to the v7 maneuver;
the 75-float snapshot remains protocol v1.
Unit conversions use standard `uom` definitions. Historical v7 replay disables
the float correction and checks recorded C++ flights with physical tolerances;
phases, abort reasons and event timestamps remain exact in that comparison.
`xpt/rust_implementation = 1` identifies the translated plugin.
Runtime 0.8.3 exposes snapshot `native_version = 8` and `xpt/version_patch = 3`.

The ordered [snapshot schema](../../crates/poweroff180/snapshot.csv) defines
the CSV fields and SDK input sources. Cargo generates named field indexes,
the header, and the input lookup list; the Python harness reads the same schema.
Controller and HUD code use `protocol::field` constants, and both languages
check the frozen protocol v1 header to prevent accidental layout changes.

The CSV appends seven inner-loop readbacks: armed, active, release reason,
actual frame interval, and roll/pitch/yaw override state. These do not change
the 75-float snapshot. Two more fields, `flare_float_active` and
`flare_predicted_touchdown_ft`, record the late-flare latch and prediction.
Result schema v3 identifies guidance v8; schemas v2 and
later require continuous attitude authority in the analysis. Old records remain
readable with their original evidence limits.

The float correction activates after roundout, at or beyond 950 ft along the
runway, at or below 5 ft AGL and at or below 69 KIAS. It requires a predicted contact
beyond the 1,100 ft aim point and a predicted descent slower than 165 fpm.
Height and vertical speed are projected 0.8 seconds using measured acceleration;
imminent contact suppresses the correction. It latches a 165 fpm terminal sink
floor in the existing flare curve and uses projected vertical speed in the
active correction's velocity feedback to anticipate continued deceleration.
It allows pitch-command reduction up to
1.25 degrees per second. Existing pitch feedback, positive pitch-rate limits
and idle power continue to apply. The turn-lead coefficients also add 40 ft
of lead for a 10 kt left crosswind to reduce final-capture overshoot.
It changes ordinary control targets; it does not write vertical velocity or
airborne position. Defaults and bounds live in
[parameters.csv](../../crates/poweroff180/parameters.csv).

Runway direction and along/cross-track telemetry use shared
[`xplane-airports` geometry](../../crates/xplane-airports/README.md), with the
card's usable threshold, fixed midpoint latitude and 60 nautical miles per
degree. The HUD uses the same axis/projection utilities for navigation legs in
nautical miles. The test card remains the runway authority.

The plugin starts inert. A complete staged `active-card.ini` is accepted only
while paused in the TorqueSim SR20; `effective-card.ini` is written for exact
readback. Start also requires the existing low-level attitude adapter to be
armed and the flight-path override to be released. An after-physics callback
records each frame, writes ordinary attitude targets/throttle/flaps, and stops
on the configured envelope, entry, timing, loading, wind or watchdog failure.
Every unpaused running frame must also show the attitude helper armed and
active, no release reason, and all three axis overrides engaged. A loss aborts
immediately with `attitude_authority_lost`, then pauses and releases authority.

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
