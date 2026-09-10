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

Read the [current illustrated report](../../docs/shuttle-hud/reports/shuttle-symbology-20260910/README.md)
for the native phase gallery, source decisions and complete test ledger. Each
image links to telemetry and identifies an initialized card or an observed flown
state. The [report index](../../docs/shuttle-hud/README.md) also includes the
historical native-optics and flare/ball-bar reports with their flight recordings.

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

## Build and install

Requirements: Windows x64, Zig 0.16.0, XPSDK430, Python 3.11+ with Pillow, and
the separately obtained **Space Shuttle-FX-V12** aircraft used by this work.
The source aircraft's required geometry/ACF hashes are recorded in
`source-requirements.json`; a different revision is rejected before writing.

From the repository root:

```powershell
./plugins/shuttle-hud/build.ps1 -Zig "C:/tools/zig/zig.exe" -SdkPath "C:/tools/XPSDK430/SDK"
python -m pip install Pillow
python ./plugins/shuttle-hud/install_native.py --xplane "D:/X-Plane 12"
```

The build runs the math, guidance and presentation suites with warnings treated
as errors, then writes `target/shuttle-hud/win.xpl` and build hashes. These
parameters are examples: supply the paths on your machine. `-OutputPath` and
installer `--binary` select alternate build locations. The exact originally
tested binary is also retained under the report's `build-142` directory.

Close X-Plane before installation. The installer reads the original under
`Aircraft/OrgForum/Space Shuttle-FX-V12`, creates a derivative under `Output`,
then places the completed aircraft in `Aircraft/OrgForum/Space Shuttle F-SIM HUD`.
It never overwrites an existing derivative or changes the original. Use
`--dry-run` to validate inputs without writing, `--stage-only` to retain the
generated aircraft under `Output`, and `--source` for an alternate original
aircraft location. Validation scripts and the test pilot are never installed.

The portable installer preserves the accepted native ACF/mesh/atlas changes:
native HUD glass and panel region, pilot eye, 18-degree down-elevon limit and
five-times original linear strut damping with custom damping enabled. These
configuration changes predate the release-142 symbology work.

## Landing lights

Copy `scenery/Shuttle Landing Aids` to X-Plane's `Custom Scenery` directory with
X-Plane closed. It is an additive native OBJ8/DSF overlay for KEDW 04R/22L and
KTTS 15/33. Existing airports are retained. To regenerate it:

```powershell
python ./plugins/shuttle-hud/build_landing_aids.py --dsftool "C:/tools/xptools/tools/DSFTool.exe"
```

Generated scenery goes under `target/Shuttle Landing Aids`; DSF intermediates
and the manifest go under `target/shuttle-landing-aids-build`. `--output`,
`--work` and `--runways` can override these locations. `runways.csv` records the
runway endpoints and displaced thresholds from the tested airport versions;
use a revised table if your airport geometry differs. Lamp brightness remains
a documented visual approximation.

## Restoration and test history

Select the untouched **Space Shuttle-FX-V12** to return to the original aircraft.
With X-Plane closed, the derivative and landing-light overlay can be moved out
of their respective scan directories. The local development archive still
contains the full release-136 backup; aircraft assets are not part of this repo.

The C++ runtime, headers, optics and validation control laws were imported
unchanged. The build, installer and scenery-generator entry points were made
portable. Original entry points are retained in
`docs/shuttle-hud/original-source`. Files under `validation/archive` retain the
original environment-specific test orchestration; they are historical records,
not a ready-to-run flight automation package.

This recreation contains original vector lettering and no F-SIM code, fonts or copied artwork. Simulator use only; not real-flight guidance or a qualified Shuttle training system.
