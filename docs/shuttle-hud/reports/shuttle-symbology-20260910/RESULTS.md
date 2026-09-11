# Shuttle HUD acceptance — release 142

[Shuttle HUD project](../../../../plugins/shuttle-hud/README.md) ·
[Rust source](../../../../plugins/shuttle-hud/src) · [Current reports](../../README.md)

Release 142 rebuilds the approach-to-rollout symbology against NASA JSC-23266 Rev B §2.12, the F-SIM HUD brief and inspected STS-125/STS-108 footage. It adds explicit phase sequencing, a five-second flight-director transition and ATT REF cage; distinct outer-path/flare indices; timed GR/GR-DN and flashing GEAR; handbook altitude steps; five-mark speedbrake pointers and mismatch flashing; and separate airborne/ground declutter cycles.

Main-wheel contact latches the rollout format, clears airborne symbols, moves speed beside the boresight and adds the deceleration scale. Nose-wheel contact selects G-prefixed groundspeed and removes pitch references. CSS final flare clears the guidance diamond and gamma triangles while keeping the velocity vector. Low airborne reloads, replay and time discontinuities reset the presentation state.

The native collimated combiner, ACF, cockpit geometry, atlas, landing guidance calibration, validation control laws and landing-aid scenery remain byte-identical to release 136. The source `Shuttle_Init.lua` remains unchanged. Presentation state advances once per simulator frame; drawing reads that state. The aircraft-local plugin owns native display callbacks and the existing temporary chute-area adjustment, and does not command airborne position, attitude, velocity or forces.

| Flight / build | Mass (lb) | Touchdown KEAS | Distance (m) | Sink (ft/s) | Max native AGL after contact (m) | Result |
|---|---:|---:|---:|---:|---:|---|
| 201 / 140 | 226040 | 201.01 | 806.8 | 2.28 | 0.686 | Pass |
| 202 / 140 | 184000 | 198.26 | 639.3 | 1.19 | 0.756 | Rebound limit missed |
| 203 / 140 | 184000 | 197.77 | 655.4 | 1.14 | 0.755 | Rebound limit missed |
| 204 / 140 | 184000 | 196.80 | 685.1 | 1.29 | 0.745 | Pass |
| 206 / 142 | 226040 | 200.86 | 804.7 | 2.29 | 0.682 | Pass |
| 207 / 142 | 184000 | 198.52 | 632.4 | 1.23 | 0.755 | Rebound limit missed |

The fixed landing checks remain 195–205 KEAS, touchdown 1,500–3,500 ft beyond the displaced threshold, sink below 5 ft/s, cross-track within 10 m, gear locked for at least five seconds, final flare 30–80 ft, a brief inner-path transition, native AGL below 0.75 m after contact and a controlled stop. Failed repeats remain failed; they are not hidden by accepted results.

The aircraft does not provide Shuttle GPC/TAEM phase words, a full HAC solution, MLS failure states or authentic braking guidance. ACQ/HDG/PRFNL are geometry estimates; CAPT uses the published broad capture gates with a forced transition at 5,000 ft, OGS uses path/gamma capture, FLARE begins at 2,000 ft and FNLFL follows the existing sink-dependent final-flare model. S-TRN and unsupported fault annunciations are not fabricated.

The flare preview uses a continuous local interpolation from the lower display edge at 3,500 ft to the nominal OGS cue at 2,000 ft, then the reconstructed nominal landing profile. The deceleration guide is v²/(2 × remaining stopping distance × g), targeting 1,000 ft before the selected runway end; its 0–0.4 g scale is a local reconstruction. Speedbrake discrepancy uses normalized native deflection × 98.6° as an approximation. Nz clears at PRFNL, an interpretation of the handbook wording corroborated by the absence of Nz in the approach footage. CSS/AUTO follows native autopilot mode plus servo engagement; the test pilot commands ordinary controls in CSS and is not Shuttle AUTO flight software.

Font shape, brightness, compressed-video optics and mission software differences remain approximate. The unchanged flight model has a small rebound: build-140 light repeats ranged from 0.745 to 0.756 m against a 0.750 m native-aircraft-AGL limit, with two failures retained. This metric is not wheel clearance. Heavy preflare still reaches about 1.66 g versus the handbook's approximate 1.3 g nominal. No physics or landing calibration was adjusted for this symbology work. These dry, zero-wind Edwards tests do not establish crosswind, wet-runway, entry/orbital, emergency, VR or full-global-plugin compatibility.

The predeclared display contract is `ACCEPTANCE_SPEC.md`; verified native cards and raw traces are retained in `Output/shuttle-symbology-20260910`. The final binary was built with all model and presentation tests passing. The report distinguishes current lifecycle evidence from historical checks and failed setup sessions.
