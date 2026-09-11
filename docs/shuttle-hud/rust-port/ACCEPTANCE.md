# Shuttle HUD native validation criteria

These criteria describe the release-143 validation recorded in this directory.
The project uses Rust and shared SDK ownership utilities. That session held
aircraft geometry, HUD atlas/optics, runway data, scenery, flight calibration
and the validation pilot fixed. The [release-144 alignment checks](../runway-alignment/README.md)
add scenery projection and displaced-threshold coverage.

1. Verify projection, vector lettering, symbol topology, phases, timers, contact
   latches, landing equations and chute stages against project requirements and
   regression fixtures. Preserve public commands, diagnostic types and view behavior.
2. Keep SDK access on the plugin thread. Release callbacks, datarefs, commands,
   menus and probes on failed startup and unload. Restore acquired view and chute
   state on disable and pause/replay as applicable. Never write flight-path or
   force overrides from the HUD.
3. Pass Rust workspace tests and warning checks. Compare numeric and rendering
   outputs with the frozen project baseline across representative display states.
4. Verify native cockpit/full-screen rendering, HAC director and prefinal
   transition, flare/gear/speedbrake cues, contact/rollout, declutter, brightness,
   replay, SDK disable/re-enable and aircraft reload/mismatch. Retain screenshots,
   readbacks and a normal shutdown log.
5. Run heavy and light native landing regressions with the established pilot.
   Preserve every run and the fixed 0.750 m native-AGL rebound limit. Known narrow
   light-weight failures remain failures; do not change physics or thresholds
   to obtain a passing result.
6. Install the verified project binary in the release aircraft, retain its hashes
   and restoration instructions, and publish results beside the source and
   build/install documentation.
