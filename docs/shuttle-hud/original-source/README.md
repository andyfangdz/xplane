# Space Shuttle — native HUD, release 142

Select **Space Shuttle - F-SIM HUD**, then press **Shift+W**.

Release 142 rebuilds the approach-to-rollout symbology against NASA JSC-23266 Rev B §2.12, the F-SIM HUD brief and inspected STS-125/STS-108 footage. It adds explicit phase sequencing, a five-second flight-director transition and ATT REF cage; distinct outer-path/flare indices; timed GR/GR-DN and flashing GEAR; handbook altitude steps; five-mark speedbrake pointers and mismatch flashing; and separate airborne/ground declutter cycles.

Main-wheel contact latches the rollout format, clears airborne symbols, moves speed beside the boresight and adds the deceleration scale. Nose-wheel contact selects G-prefixed groundspeed and removes pitch references. CSS final flare clears the guidance diamond and gamma triangles while keeping the velocity vector. Low airborne reloads, replay and time discontinuities reset the presentation state.

The native collimated combiner, ACF, cockpit geometry, atlas, `landing_guidance.hpp`, validation control laws and landing-aid scenery remain byte-identical to release 136. The source `Shuttle_Init.lua` remains unchanged. Presentation state advances once per simulator frame; drawing reads that state. The aircraft-local plugin owns native display callbacks and the existing temporary chute-area adjustment, and does not command airborne position, attitude, velocity or forces.

## Controls

- **Shift+W** or **Plugins → Shuttle HUD → Show shuttle cockpit HUD**: commander HUD view.
- **Cycle manual HUD declutter**: airborne full → runway off → digital data → boresight only → full. After main-wheel contact: normal ground format → attitude off → boresight only → normal.
- **Use F-SIM automatic declutter**: restore the convenience mode (10,000/4,000 ft transitions). Default is automatic.
- **Toggle ATT REF horizontal cage**: cage lateral flight-director movement; the symbol remains square while caged.
- Display toggle, runway selection and optional full-screen HUD remain in the menu. Cockpit and full-screen views use the same phase rules. Native HUD power and electrical brightness apply; `fsim_hud/brightness` scales brightness.

Supported runways: KEDW 04R/22L and KTTS 15/33. Hiding the HUD does not disable the separate chute model. The release contains no validation pilot.

## Evidence

Read `D:/X-Plane 12/Output/shuttle-symbology-20260910/REPORT.html` for the native phase gallery, source decisions and complete test ledger. Each image links to telemetry and identifies an initialized card or an observed flown state. The older release-136 video remains historical evidence of the preserved landing model.

| Flight / build | Mass (lb) | Touchdown KEAS | Distance (m) | Sink (ft/s) | Max native AGL after contact (m) | Result |
|---|---:|---:|---:|---:|---:|---|
| 201 / 140 | 226040 | 201.01 | 806.8 | 2.28 | 0.686 | Pass |
| 202 / 140 | 184000 | 198.26 | 639.3 | 1.19 | 0.756 | Rebound limit missed |
| 203 / 140 | 184000 | 197.77 | 655.4 | 1.14 | 0.755 | Rebound limit missed |
| 204 / 140 | 184000 | 196.80 | 685.1 | 1.29 | 0.745 | Pass |
| 206 / 142 | 226040 | 200.86 | 804.7 | 2.29 | 0.682 | Pass |
| 207 / 142 | 184000 | 198.52 | 632.4 | 1.23 | 0.755 | Rebound limit missed |

## Limits

The original port does not provide Shuttle GPC/TAEM phase words, a full HAC solution, MLS failure states or authentic braking guidance. ACQ/HDG/PRFNL are geometry estimates; CAPT uses the published broad capture gates with a forced transition at 5,000 ft, OGS uses path/gamma capture, FLARE begins at 2,000 ft and FNLFL follows the existing sink-dependent final-flare model. S-TRN and unsupported fault annunciations are not fabricated.

The flare preview uses a continuous local interpolation from the lower display edge at 3,500 ft to the nominal OGS cue at 2,000 ft, then the reconstructed nominal landing profile. The deceleration guide is v²/(2 × remaining stopping distance × g), targeting 1,000 ft before the selected runway end; its 0–0.4 g scale is a local reconstruction. Speedbrake discrepancy uses normalized native deflection × 98.6° as an approximation. Nz clears at PRFNL, an interpretation of the handbook wording corroborated by the absence of Nz in the approach footage. CSS/AUTO follows native autopilot mode plus servo engagement; the test pilot commands ordinary controls in CSS and is not Shuttle AUTO flight software.

Font shape, brightness, compressed-video optics and mission software differences remain approximate. The unchanged flight model has a small rebound: build-140 light repeats ranged from 0.745 to 0.756 m against a 0.750 m native-aircraft-AGL limit, with two failures retained. This metric is not wheel clearance. Heavy preflare still reaches about 1.66 g versus the handbook's approximate 1.3 g nominal. No physics or landing calibration was adjusted for this symbology work. These dry, zero-wind Edwards tests do not establish crosswind, wet-runway, entry/orbital, emergency, VR or full-global-plugin compatibility.

## Build and restoration

Run `Support/Shuttle-HUD/build.ps1` (optional `-OutputPath`). Zig/XPSDK430 builds the plugin and runs math, guidance and presentation tests with warnings treated as errors. The final immutable build/source snapshot is `Output/shuttle-symbology-20260910/build-142`; installed and original-file hashes are in the final audit and manifest there. `install_native.py` regenerates the derivative from the untouched source; the current display update replaces only the aircraft-local plugin and documentation.

Select the untouched **Space Shuttle-FX-V12** to revert to the source aircraft. To restore just release 136, close X-Plane and copy the binary from `Output/shuttle-symbology-20260910/baseline-136/aircraft/plugins/ShuttleHUD/64/win.xpl` back into the derivative. The full baseline is retained outside `Aircraft`.

This recreation contains original vector lettering and no F-SIM code, fonts or copied artwork. Simulator use only; not real-flight guidance or a qualified Shuttle training system.
