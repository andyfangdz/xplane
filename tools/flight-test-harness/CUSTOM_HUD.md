# Native custom HUD v5

`../../plugins/poweroff180-hud/src/graphics.rs` renders the custom HUD in X-Plane's SDK draw callback. The display is recorded directly by X-Plane's native movie recorder. MP4 encoding adds no HUD graphics or text. The earlier custom layout is retained. Version 5 adds Garmin-style tapes and HSI while preserving the approved central symbols. The command-bar geometry follows the user's G1000 reference with shallow opaque magenta command bars, squared caps, black outlines, faceted yellow aircraft wings and rounded side references. The bank scale is a continuous arc with filled index/pointer triangles and a slip trapezoid; the pitch marks are solid at 2.5-degree intervals with alternating lengths. The diamond steering cue and its label are absent.

Reference geometry is scaled uniformly by 1.4 from the supplied image's aircraft apex (217,294) to the HUD apex (960,540). Command-bar tips are +/-158 reference pixels with a 20-to-32-pixel drop, and fixed aircraft tips are +/-138 pixels with a 32-pixel drop. The bank arc has a 216-pixel reference radius. Major pitch-mark widths are 60 and 122 reference pixels, with 30-pixel minor marks. `Output/performance-tests/TORQUESIM_SR20_G1000_MATCH_20260909/reference-geometry.md` records these proportions alongside the supplied image.

The V-bars show errors against the native maneuver controller's bank and pitch commands. They are a custom presentation of native v7 guidance, not Garmin autopilot outputs. On the 1920 x 1080 design plane, pitch displacement is 22 pixels per degree of commanded-minus-actual pitch, limited to 170 pixels. Roll is commanded-minus-actual bank, wrapped and limited to 35 degrees. Match the yellow aircraft symbol to the magenta bars. `../../plugins/poweroff180-hud/src/hud.rs` implements these display-only formulas and the SR20 visibility policy; `../../crates/xplane-hud` provides the shared camera projection.

White attitude markings and the outlined green circular flight-path marker are projected using camera heading, pitch, roll and horizontal field of view. Pitch marks are clipped clear of the bank pointer, and the flight-path marker draws over the command and aircraft symbols. The green marker represents current motion, not a touchdown prediction. The left panel shows power, RPM, manifold pressure, fuel flow and selected/actual flap positions. The right panel shows aileron, elevator and rudder input. Speed, MSL altitude, VSI, ground speed, AGL and load factor are live values. The touchdown footer uses the native contact latch and physical sink measurement; indicated VSI can differ because it is filtered.

The bottom-left plan and vertical paths accumulate samples already flown, starting near recording entry. They do not display a future trajectory. The wind label identifies the configured case; LOCAL shows instantaneous local wind, which changes near the surface.

The HSI uses live GPS course and CDI deflection. Paused setup loads the local RNAV 22 approach and explicitly activates KOLLI to RW22. The displayed numeric LEG XTK is calculated from the actual leg endpoint coordinates and aircraft position; CENTERLINE uses the maneuver's runway coordinates. They are separate references. Navigation commands are accepted only while paused and while native guidance is not running. The drawing callback does not write flight controls, flight-model state or navigation.

## Garmin tapes and HSI

`../../plugins/poweroff180-hud/src/scene/instruments.rs` follows the primary illustrations in Garmin's [G1000 Pilot's Guide, 190-00498-07 Rev A](https://static.garmin.com/pumac/190-00498-07_0A_Web.pdf), printed pages 50, 53, 56, 57, 60 and 62. The guide provides display conventions; aircraft-specific speed limits come from the loaded SR20. The retained PDF, rendered pages and design notes are in `Output/performance-tests/TORQUESIM_SR20_G1000_HSI_TAPES_20260909/references` and `reference-notes.md`.

The airspeed tape shows 60 knots with major marks every 10 and minor marks every 5. The altitude tape shows 600 feet with 100-foot major and 20-foot minor marks. Both are centered on the fixed aircraft apex at y=540, and use stepped black pointers facing inward. Airspeed's final digit and altitude's 20-foot pairs scroll; higher digits carry during the final minor increment. TAS and barometric setting sit below their tapes. Selected altitude and its edge-clamped bug are cyan. The attached tapered VSI has 1000/2000-fpm labels and 500-fpm minor ticks; its black pointer displays a number above 100 fpm magnitude.

IAS, barometric altitude, VSI, AHARS magnetic heading and magnetic ground track read pilot-side instrument datarefs. Selected altitude/heading and barometric setting read the actual cockpit selections; they do not claim an autopilot mode is engaged. The white speed band ends at the loaded ACF's `acf/_Vfem_kts`: 104 KIAS on this SR20. The generic `acf_Vfe` dataref reads 119 KIAS for the first detent, so it is diagnostic only. The helper locates the loaded ACF through `XPLMGetNthAircraftModel`, exposes the parsed full-flap limit and saves its source path. Other ranges use `acf_Vso`, `acf_Vs`, `acf_Vno` and `acf_Vne` (62, 69, 163 and 200 KIAS here).

Six-second magenta airspeed and heading trends use causal simulator-time derivatives with 0.7-second exponential smoothing. Heading differences wrap through north. Altitude trend uses indicated VSI times 0.1 minute. The turn arc marks half-standard and standard rates at +/-9 and +/-18 degrees; rates above 4 degrees/second display an end arrow. Trend state resets on each run.

The HSI compass labels rotate with its heading-up card. It includes a heading window and lubber triangle, cyan selected heading and bug, a magenta magnetic-track diamond, a magenta GPS course arrow with separated shaft sections, and a sliding CDI with two reference dots on each side. The displayed deflection clips at +/-2 raw dots. TO/FROM, GPS phase and OBS/SUSP reflect live native GPS datarefs. The white aircraft planform remains fixed. Off-scale GPS deflection also shows the calculated leg XTK below the aircraft; a separate readout retains its signed side and runway-centerline comparison.

## Reproduce

With X-Plane closed, use the standard launcher from this folder:

```powershell
.\scripts\Test-Harness.ps1
.\Run-XPlaneTest.ps1 -Config 'D:\X-Plane 12\Output\performance-tests\TORQUESIM_SR20_G1000_HSI_TAPES_20260909\video-cards.json' -RecordVideo
```

Capture preparation disables VR temporarily, zeros pilot camera offsets, then selects `forward_with_nothing` last, because offset writes can select the cockpit view. Capture requires view type 1024, HUD version 5, a ready font atlas, at least 30 rendered frames and a ready RNAV leg. Capture also requires a valid parsed full-flap marking and retains the instrument/navigation readbacks and source path. The 75-degree horizontal field of view is retained in each capture record. The native helper uses an in-process font atlas and filled stroke geometry for consistent Vulkan-bridge rendering.

Keep source frozen during a campaign. The standard session transaction restores scenery, original preference/state files and temporary helpers. Do not install the helper manually to reproduce these runs.

## Verification and scope

HUD v5 campaign `XPT_G1000_HSI_TAPES_B_20260909` passed calm, 10 kt left crosswind and 15 kt headwind with frozen source. Touchdowns were 1068.3, 1144.2, 1100.9 ft and physical sink rates were 75.3, 102.7, 109.0 ft/min. All 35 source/binary hashes matched after the campaign, including the unchanged four v6 guidance hashes. The seven Python checks, native guidance tests, HUD projection/director tests, tape/digit/heading/CDI checks and exact-process checks passed before flight. Each final MP4 passed full decode, non-silent audio and representative frame review. Preview attempt A failed the unchanged mass gate before the maneuver and remains rejected; its display also exposed the generic-versus-full-flap limit mismatch corrected before B. Both attempts restored successfully.

The preflight harness checks passed, including native guidance and HUD projection/director direction and saturation tests. `XPT_G1000_MATCH_B_20260909` passed calm, 10 kt left crosswind and 15 kt headwind flights with the frozen HUD v4 source. Touchdown distances were 1066.5, 1148.6 and 1099.0 ft, with physical sink rates 76.5, 101.9 and 112.5 ft/min. Preview attempt A was cancelled before touchdown to correct pitch-mark clipping and line rendering; its evidence and verified recovery are retained. All four v6 guidance source/binary hashes match the previous release. This three-flight display campaign does not replace the separate 21-flight guidance validation. The earlier three HUD v3 flights remain in `XPT_CUSTOM_HUD_C_20260909`.

Each export has a verification JSON, audio check, full decode check and entry/turn/final/contact/rollout images. The finalizer maps native movie timing to the measured simulator span and adjusts loopback audio using measured wall/simulator timing. Existing global plugin widgets remain visible, including a small landing-results panel after contact that can overlap the lower navigation text. The central flight director and touchdown footer remain readable. The display was validated in this 1920 x 1080 forward-view capture configuration; VR rendering was not tested.


## Lower-flare replacement recordings

The v7 guidance recordings in [the lower-flare report](../../docs/poweroff180/lower-flare-report.md) supersede the earlier faster landings. The native HUD v5 source and binary remain unchanged. Each replacement has accepted flight telemetry, synchronized non-silent audio, a complete decode check and visual review around roundout and contact.

Native AVI frames are mapped to simulator time using the visible HUD clock at one-second intervals; repeated frames recorded after the simulator paused are collapsed. Loopback audio is aligned from the measured capture timestamps. Independent readings of the HUD clock in all 24 exported review frames differ from the trace by at most 0.067 seconds. The original AVI and audio files, frame-clock measurements and export verification records are retained.
