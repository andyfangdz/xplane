# Native HUD lifecycle

> Historical verification record. Paths below identify the original local
> experiments. The [repository report index](../../docs/shuttle-hud/README.md)
> describes the published subset; statements about transmission below describe
> the original validation sessions before this repository import.

## Current architecture

Release 144 adds a scenery-sampled runway surface and a read-only projection-valid diagnostic. Sampling and transforms remain on the simulator thread. The surface refreshes on runway/view selection and scenery reload. Missing terrain or invalid transforms suppress geometry; disable/power-off clears projection validity. No additional SDK resources or overrides are acquired. See the [alignment verification](../../docs/shuttle-hud/runway-alignment/README.md).

Versions 118 onward use native HUD projection from an emissive cockpit-panel region. The plugin registers a gauges draw callback, a full-screen draw callback, one main-thread flight-loop callback, diagnostic datarefs, commands, a menu and a terrain probe. It creates no `XPLMCreateAvionicsEx` device. The renderer is scoped to the exact derivative ACF path.

Startup reads and validates `hud-optics.txt` before registering drawing. Missing, non-finite or out-of-atlas bounds fail startup. Failed callback registration runs the same cleanup routine as unload. Cleanup unregisters owned callbacks and command handlers, removes custom datarefs, destroys the terrain probe and menu, and restores any owned camera settings.

The staged drag-chute model was added in version 120. It saves native full chute area before reducing that area during the documented reefing interval. It restores the saved value on jettison, pause, replay, landing-system disable, SDK disable or unload. It publishes its ratio and activity separately from HUD visibility. A mismatched aircraft does not receive writes. Simulation time governs deployment stages; a backwards clock reset clears the previous deployment.

The physical controls and aircraft trajectory are native. The temporary validation controller, installed only in the separate trial aircraft, owns joystick input while armed and releases it on stop, heartbeat timeout, error and unload. Its supervisor releases initial placement overrides before flight.

## Verification status

Release 127 passed the 16-card native optical matrix and the geometry/projection tests with compiler warnings treated as errors. Flights 124, 125 and 126 passed the landing checks, including native gear deployment, chute rollout and stopping. The separate test controller released its joystick override after stopping; aircraft-path overrides remained zero throughout each airborne run.

Live `chute-127.json` checks passed early deployment, the 16% reefed plateau and full opening; restoration on pause, landing-system disable, replay and jettison; and independence from HUD visibility. The saved full-area readback was 480 ft². SDK disable during the reefed interval and injected startup-failure branches were not separately exercised.

Actual Plugin Admin disable/re-enable of the release aircraft removed and restored the display. The re-enabled readback was version 127, renderer 2, aircraft match 1 and active 1. Full-screen/cockpit switching restored field of view to 65° and vertical frustum shift to zero. The before-exit scene was paused with joystick and aircraft-path overrides zero.

The final dedicated process (PID 48300, started 2026-09-10 15:47:06 UTC) exited normally through `sim/operation/quit`. Its log records native callback cleanup, presentation restoration, plugin unload completion, clean thread exit and the simulator shutdown marker. No forced termination was used. Evidence is `Output/shuttle-correction-20260910/clean-exit-127.json` and `Log-127-clean-exit.txt`; SDK cards are in `Output/shuttle-hud-20260909/cockpit/sdk-127-*`.

The final session log contains OpenXR startup failure, missing external XPME scenery-resource warnings, missing aircraft VR configuration and third-party scenery gamma warnings. It contains no already-destroyed avionics-device warning or Lua stack trace. Validation used session-only graphics/plugins/art-controls safe mode and temporary XPME scenery exclusion during flight loads. The saved scenery configuration was restored exactly; this test does not validate the complete plugin environment or VR.

An earlier development session crashed during an aircraft reload after flight 109, after approximately 52 reloads. Its log shows plugin cleanup followed by simulator off-screen rebuilding and a crash; the cause is unproven. Two later startup attempts also failed. Those logs are retained separately and are not clean-shutdown evidence. The successful final process completed the current release tests and normal shutdown described above.

## Historical avionics-device warning

Versions 116/117 used an SDK custom avionics device. Their logs recorded one destruction call per created device and successful disable/re-enable, but X-Plane later warned about already-destroyed records during aircraft unload. The exact cause was never established. Historical evidence is retained under `Output/shuttle-hud-20260909/cockpit`, including `Log-116-clean-exit.txt` and the disable/re-enable captures.

The native architecture removes that device dependency. This is an architectural change, not proof that every simulator lifecycle issue has been eliminated. Historical device warnings must not be attributed to a new native-HUD session without a log showing the corresponding code actually loaded.

No log or report has been transmitted externally. Final validation must distinguish a clean session from development logs containing earlier device experiments and failed reloads.

## Flare refinement, release 136





The native projection, callback ownership, chute-area ownership and cleanup code are unchanged from release 127. Changes add a read-only radar-height diagnostic and CSS final-flare cue blanking. The guidance header changes calibrated energy and flare targets. Current flight/load/shutdown evidence is in `Output/shuttle-flare-20260910`; the original 127 lifecycle evidence above remains historical.





A separate validation-only SceneryProbe used an SDK terrain probe and a paused camera to inspect ball/bar scenery. It releases its camera on unpause, view change, disable and unload. Its camera-off readback and zero path overrides are retained with the photographic cards. This helper is never installed in the release.




Current release 136 passed a paused aircraft reload and cockpit/full-screen restoration, then normal simulator shutdown. The log records native callback cleanup and clean stop. `release-reload-136.json` records version 136, renderer 2, zero overrides, and unregistered validation-helper IDs; `Log-136-clean-exit.txt` retains the shutdown. A startup banner still says v127; the runtime version dataref and installed binary hash identify release 136.







## Symbology refinement, release 142

The pure presentation state adds phase sequencing, contact latches, timers and per-phase declutter. It resets on replay crossings, backwards simulation time and large repositioning. Stale contact during a low airborne reload clears above 50 ft, or above 5 ft within the first three simulation seconds; ordinary low rebounds retain the ground format. This changes presentation only. Startup logging now reports the runtime version dynamically.

The build-140 native cards verify the five-second fade, ATT REF, manual/automatic declutter, gear timing, CSS/AUTO distinction, power, dimming, off-axis clipping, view restoration and replay entry/exit. Flown telemetry records raw contact release during rebounds while displayed WOW stays latched. The only subsequent renderer-independent change is the low-start reset correction in release 142. Final build tests cover that correction.

Several setup sessions stalled in loading/graphics, including the later reload after build-140 flights. They are retained under `Log-*startup*` and `Log-session-140-reload-stall.txt`. The latter needed identity-checked termination after a normal quit request did not complete. Their cause is unproven. They are not clean-exit evidence. The final release readback, SDK enable/disable and clean-exit status are recorded separately in `release-reload-142.json`, `sdk-lifecycle-142.json` and `clean-exit-142.json`.

## Rust migration, release 143

The current implementation uses the shared Rust SDK owners for draw callbacks,
exported datarefs, flight loop, commands, menu and terrain probe. Callback storage
is stable and independent from the main plugin state borrow. All SDK access stays
on the plugin thread. Exact-path matching prevents activity for another ACF.
Partial startup failures drop already-owned registrations.

Current Rust-native verification covers actual Plugin Admin disable/re-enable,
view restoration, chute stages/restoration on pause/replay/landing-system disable
and jettison, same-folder ACF mismatch, correct-aircraft reload, release-aircraft
load and stock Shift+W interception. Full native chute area read back as
44.593460 in this session (ACF area 480); the model scales and restores the
native value rather than substituting an area constant.

The dedicated process started at 2026-09-11 00:16:15 UTC and exited normally
through `sim/operation/quit`. Its final log records the Rust 143 registration,
callback cleanup, presentation restoration, plugin unload, clean thread exit
and the simulator shutdown marker. No forced termination was used. Saved scenery
bytes and protected installation state were restored, the trial was retired
outside Aircraft, and the release contains no validation pilot.

Startup failure injection and SDK disable during active reefing were not separately
tested. The session used safe mode and temporary XPME exclusion; third-party and
environment warnings remain documented. Current evidence is in the
[Rust migration report](../../docs/shuttle-hud/rust-port/README.md); all earlier
release-specific sections above remain historical.
