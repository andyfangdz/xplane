# Power-off 180 controller

Rust XPLM runtime for the [pure guidance crate](../../crates/poweroff180/src/guidance.rs).
Guidance v8 adds a bounded late-flare float correction to the v7 maneuver;
the 75-float snapshot remains protocol v1.
Unit conversions use standard `uom` definitions. Historical v7 replay disables
the float correction and checks recorded C++ flights with physical tolerances;
phases, abort reasons and event timestamps remain exact in that comparison.
`xpt/rust_implementation = 1` identifies the translated plugin.
Guidance v9 adds gust handling and v10 gust energy management; with
`wind_gust_kt = 0` every steady card flies exactly as v8 (the frozen replay
still matches frame by frame). Runtime 0.8.7 exposes snapshot
`native_version = 10` and `xpt/version_patch = 7`, writes the attitude
helper's `target_beta_deg`, and appends `slip_command_deg` to the trace.

## Gusts

A card's `wind_gust_kt` is X-Plane's gust increase above the steady wind.
Measured on 12.4.4 beta 1: gusts raise the wind from the steady speed toward
the peak over 1–2 s and hold for several seconds (mean ≈ steady + 35% of the
gust); direction does not change. True airspeed follows a gust on the same
frame, while indicated airspeed lags about a second. Near the surface the gust
decays to the steady wind in the last ~50 ft, so every gusty landing flares
into a headwind loss.

- **Mean-wind guidance.** Airspeed control, the entry gate and speed-based
  logic use the airspeed the aircraft would have in an 8 s mean wind,
  |ground velocity + mean wind| scaled by a slowly filtered IAS/TAS ratio.
  Guidance flies the average instead of chasing each gust. The final-turn arc
  uses the measured wind, because ground track answers a gust at once.
- **Speed margins.** 70% of the gust is added to the final glide and landing
  targets, keeping the short-final deceleration that raises the nose before
  roundout. The landing margin fades from full at a crosswind to zero at a
  tailwind (a decaying tailwind gust adds airspeed and floats).
- **Energy.** Turn timing uses the expected mean wind and turns base earlier
  in proportion to the faster glide. When that would need a turn before the
  power cut (15 kt gusting 23), half flaps are held until established on final.
- **Energy management (v10).** Gust timing moves energy on final by ~30 ft,
  and the shallow pre-flare path turns that into ~500 ft of roundout
  position. Gust cards turn base 0.3 s earlier as a reserve (none in a
  tailwind), and path feedback on final may bias pitch by ±1.3° (steady
  ±0.8°). Nose-down bias fades to ±0.4° by roundout, and below 100 ft a
  pre-flare adds nose-up bias when sink exceeds 560 fpm. On final with full
  flaps, a forward slip (up to 8°, upwind wing low) dissipates a projected
  surplus beyond 50 ft of roundout position and fades out by 110 ft. The
  energy-limited 15G23 card holds half flaps until the projected roundout
  shows 150 ft of surplus, or 150 ft AGL, or 600 ft from the threshold. The
  aim is 40 ft short of the 1,100 ft target, correcting the measured mean;
  tailwind gust floats aim the roundout 30 ft per knot of gust earlier.
- **Rate-commanded flare.** The steady flare integrates a pitch command. On
  gust cards that overshot early and fell behind late: the attitude helper
  answers a command lead with rate = (lead − L₀(h)) / 0.4 s, where the holding
  lead L₀ grows from ~0.2° at 30 ft to ~1.7° at the runway as full flaps and
  ground effect pitch the nose down (fitted from 24 flares, R² 0.8–0.9), with
  the elevator near half authority. Gust flares therefore command
  pitch + L₀(h) + 0.4 s × rate, estimate each flight's holding-lead
  difference slowly against a 0.3 s reference response, and feed back the
  achieved rate. The rate demand is capped at 2.2°/s and ramped in over
  0.8 s, attitude is capped at 7°, and a gentler 1.1 ft/s² sink profile
  asks for lower sink before the gust decays.
- **Touchdown-point flare (v10).** Floats after roundout vary by ~±90 ft in
  gusts, and long ones were soft. The terminal sink target rises by
  0.013 ft/s per foot of predicted overrun past the aim, up to 4 ft/s
  (240 fpm), so a long float settles sooner.
- **Safety.** Envelope, wind-consistency and all derivatives use raw
  measurements. The wind guard accepts speeds from steady − 2 kt to
  steady + gust + 2 kt, with the unchanged direction tolerance.

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
