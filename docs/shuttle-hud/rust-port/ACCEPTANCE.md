# Rust migration contract

Baseline: native C++ release 142, commit 104c010. This migration replaces the
plugin implementation and adopts the repository's Rust SDK ownership utilities.
The aircraft's ACF, geometry, HUD atlas/optics, runway table, scenery, flight
calibration and validation pilot remain fixed.

Before release:

1. Port projection, vector lettering, symbol topology, phase/timer/contact logic,
   landing-path equations, guidance latches and chute stages without retuning.
   Preserve the `fsim_hud` commands, diagnostic names/types and view behavior.
2. Put SDK registrations and handles behind reusable shared Rust ownership
   wrappers. Keep SDK access on the plugin thread. Release draw callbacks,
   exported datarefs, commands, menus and probes on failed startup and unload.
   Restore acquired view and chute state on disable, pause/replay as applicable,
   and unload; never write flight-path or force overrides from the HUD.
3. Pass the Rust workspace tests and warning checks, and compare numerical and
   rendering outputs with the C++ reference across representative phase states.
4. Verify native cockpit/full-screen rendering, HAC square director and prefinal
   transition, flare/gear/speedbrake cues, contact/rollout, declutter, brightness,
   replay, SDK disable/re-enable and aircraft reload/mismatch. Retain screenshots,
   diagnostic readbacks and a normal shutdown log.
5. Run heavy and light native landing regressions using the established pilot.
   Preserve every run. The established 0.750 m native-AGL rebound limit has known
   narrow light-weight failures; report those honestly, without changing physics
   or thresholds to make the port pass.
6. Install only the accepted Rust binary into the release aircraft and keep the
   exact C++ binary available for restoration. Update build/install documentation
   and add the migration results beside the existing reports before committing
   and publishing the repository change.
