# Shuttle HUD symbology audit

Starting release: 136. Scope: displayed approach/landing symbols, their placement, and state transitions from TAEM acquisition through rollout. The accepted flight model and landing-guidance calibration are a preserved baseline.

## Sources and precedence

1. NASA **JSC-23266, Rev B, May 2005**, section 2.12 (printed pages 2-35 through 2-40; PDF pages 47–52), and sections 5.3.6.4–6: [Approach, Landing and Rollout Flight Procedures Handbook](https://www.ibiblio.org/apollo/Shuttle/Crew%20Training/Flight%20Procedures%20App%20Land%20Roll.pdf). This defines the real display sequence. The original PDF, extracted text, and hashes are retained under `../shuttle-correction-20260910/sources`; selected pages are rendered in this folder's `sources` directory.
2. [F-SIM HUD systems brief](https://fsim.com/help/ios/hud.html) and [landing tutorial](https://fsim.com/help/ios/landing.html). Useful explanations and simulator conveniences; the HUD brief gives automatic declutter at 10,000/4,000 ft, while the tutorial loosely says about 3,000 ft. Treat 4,000 ft as the F-SIM setting and manual declutter as the real crew control.
3. [STS-125 labelled HUD footage](https://www.youtube.com/watch?v=JBk6lCikqkQ), visually inspected in the browser at 0:00, 1:02, 1:22, 1:33, and 1:38. At 0:00, PRFNL is visible. At 1:02 the display has digital speed/radar height, horizon, a gear-transit cue, FLARE/CSS, and opposed speedbrake pointers, without an Nz readout. At 1:33 the airborne vector/numbers and mode labels have cleared; speed sits by the boresight, pitch references are restored and the right-hand deceleration scale is visible. At 1:38 speed has a G prefix and the pitch references are absent. The channel title identifies the mission; the footage is used for visual behavior, not exact trajectory reconstruction. Compression, perspective and video-frame timing prevent precise font/photometry measurement.
4. [STS-108 full approach HUD footage](https://www.youtube.com/watch?v=jgPR8R28WCo), a second recorded approach with a banked HAC segment. The 0:28 and 1:24 banked segments show a rotating attitude reference with fixed tape/flight-director layout, but cloud glare makes exact text unreadable. At 2:20 the circular vector, OGS/CSS labels, speedbrake scale and perspective runway/centerline overlay are readable enough to corroborate their topology. No exact numeric values were inferred from these blurred frames.

## Demonstrated gaps in release 136

| Element | Existing behavior | Required correction |
|---|---|---|
| Flight phases | Early labels depend on bank or a broad runway-heading test; CAPT/PRFNL absent | Explicit presentation phase transitions and stable state; document how X-Plane geometry estimates phases without Shuttle GPC flags |
| Flare indices | Extra pair only between 3,000 and 2,000 ft; both pairs use the corrected guidance command | Start near the handbook's 3,500–3,800 ft band; distinguish nominal OGS/open-loop flare reference from error-correcting guidance diamond; merge and retire OGS indices |
| Gear | GR-TR for movement, persistent GR-DN, no gear-up warning | GR transit, flashing GEAR below 300 ft/300 KEAS when up, GR-DN for five seconds after all three lock, then blank; all clear at WOW |
| Main-wheel contact | Raw contact can reverse the display during rebound; current declutter persists | Latch WOW for display sequencing; remove runway/tapes/altitude/VV/guidance/cues; move digital speed to boresight; restore pitch references; add deceleration scale |
| Nosewheel contact | G prefix only, horizon remains | Latch WONG; groundspeed with separated G prefix; blank pitch ladder and horizon |
| Declutter | Only 0/1/2, common rules before/after touchdown | Add boresight-only level and distinct ground cycle; retain separately selectable F-SIM automatic convenience |
| Digital altitude | Nearest integer at every height | Truncate at source-specified 200/100/10/1-ft steps; preserve radar-source R |
| Altitude tape | Only 100/1,000-ft increments, incorrect transition at 2,000 ft | 50-ft increments below 500, 100 below 1,000, 1,000 to 100,000, 10,000 above; consistent K suffix |
| Digital airspeed | Only declutter 2 or WOW | Also displayed at/below 1,000 ft when tapes remain |
| Nz | Displayed throughout flare | HAC/early approach only; remove at preflare per handbook, corroborated by footage; keep overload flashing while displayed |
| Speedbrake | Six marks and two triangles; no mismatch alert | Five marks, upper actual triangle, lower command arrow, actual pointer flashes for >20° discrepancy |
| Runway | Draws only if all corners project; stays in ground full-up format | Clip visible edges independently; blank at WOW |
| Views | Cockpit-only horizon and cue exceptions; full-screen ROLLOUT label | Shared symbol-state rules in both views |

## Boundaries

The original aircraft has no Shuttle GPC/TAEM flags, MLS navigation solution or complete Orbiter fault set. The local HUD may derive phase estimates from geometry and expose a documented external presentation-phase input, but must not claim to implement Shuttle flight software. Missing real navigation/fault signals must not be manufactured as alerts. Existing native collimation and aircraft geometry remain intact. Operational frame updates, pause/replay resets, contact latches, enable/disable and aircraft unload will be verified explicitly.
