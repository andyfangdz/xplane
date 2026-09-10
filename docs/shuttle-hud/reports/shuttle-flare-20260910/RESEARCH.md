# Flare and landing-light refinement





The starting point is native HUD release 127 and its recorded light, dry, no-wind landing. This refinement responds to the user's STS-125 video and approximate 50/250, 40/240, 30/230, 20/220, 10/210 callouts, with touchdown requested at 195–205 knots.





## Reference conditions





[NASA's STS-125 mission record](https://www.nasa.gov/mission/sts-125/) gives Atlantis' landing weight as 226,040 lb, whereas the existing test aircraft weighed 184,000 lb. The [STS-125 press kit, mission profile](https://www.nasa.gov/wp-content/uploads/2023/05/331922main-sts125-presskit-050609.pdf) also lists 226,040 lb. This is a material difference when comparing speed/height traces. The flight model, winds, pilot technique and runway slope also differ; the reference video is not a precise aerodynamic calibration dataset.





[The supplied video](https://www.youtube.com/watch?v=JBk6lCikqkQ) was viewed in the browser at approach and flare frames. Its automatic captions were exported and retained, but are noisy. Readable HUD frames corroborate the broad height/speed progression. We did not infer exact telemetry from blurred characters. The displayed R height is measured against terrain below, while guidance uses main-wheel height above the touchdown-zone datum. Both are logged separately from release 129 onward.





## Procedures





Primary manual: [JSC-23266 Rev B, May 2005](https://www.ibiblio.org/apollo/Shuttle/Crew%20Training/Flight%20Procedures%20App%20Land%20Roll.pdf), already retained with hash in `../shuttle-correction-20260910/sources/landing-procedures.pdf`.





- Section 3.5, pages 3-8 to 3-10, figures 3.5-1/2: six bar assemblies of four red PAR-56 lamps, with 15 ft between assembly centers. The nearest assembly is 200 ft left of runway centerline. Bar station 2,200 ft, ball station 1,700 ft. Red lamps are 2 ft and white lamps 15 ft above the centerline elevation at bar station. Three white ball lamps. Lamp centers within an assembly are 10.5 inches apart. Aim is 4 degrees upward. Native scenery uses these dimensions; fixture shapes and brightness are visual approximations.


- Section 4, pages 4-19/21: final flare is sink-dependent, 30–80 ft, nominally about 50 ft. Lightweight touchdown target is 195 KEAS and heavyweight 205 KEAS.


- Page 5-43: inner glide-slope contact may be brief or momentary. The old analysis's minimum five-second dwell was not a manual requirement and has been removed before testing. The transition itself is still checked and measured.


- Page 5-44: final flare starts at about 240 KEAS, reduces sink toward less than 3 ft/s, and calls for small pitch inputs. Avoid abrupt pitch-up in the final few feet. CSS removes guidance diamond/gamma triangles in final flare; AUTO retains them. The released HUD now follows that distinction.


- Page 5-46: nose lowering begins at 185 KEAS; control input builds smoothly over 1–2 seconds toward 1.5–2.5 degrees/sec. The test pilot previously used a two-second-after-contact trigger and a step in commanded rate. It now waits for 185 KEAS and ramps to 2 degrees/sec over 1.5 seconds.





## Implementation and evidence





The new scenery is `Custom Scenery/Shuttle Landing Aids`, placed above existing airports without replacing them. One OBJ origin per runway lies on the centerline at bar station, letting X-Plane sample the correct native terrain elevation. All lamp elevations use that common datum; off-runway mesh slope does not redefine it. The two overlay DSFs were compiled and decoded with [Laminar's DSFTool](https://developer.x-plane.com/tools/xptools/). The [OBJ8 format](https://developer.x-plane.com/article/obj8-file-format-specification/) and existing native `spot_params_bb_day_pm` light definition provide the lamp billboards. No HUD overlay draws these lights.





The retained candidate uses a continuous weight fraction `w = clamp((mass_lb - 184000) / 42040, 0, 1)`. Below 3,000 ft, speedbrake ratio is `0.105 + 0.145*w`; the 500 ft event holds the same setting. The original C2 preflare geometry remains the reference. A C2 quintic ramp between reference heights 400 and 100 ft offsets the tracking target downward by `28 - 8*w` ft. This local correction creates a brief inner-path intercept instead of a long dwell. The final-flare trigger is `clamp(-vertical_speed*4.2, 30 ft, 80 ft)`. The vertical-speed command is 1.5–1.6 m/s downward above 15 ft and tapers toward 0.9144 m/s below 15 ft. These are local control calibrations, not Shuttle GPC code.





Both the display and the test pilot consume this guidance; the released aircraft contains no automatic pilot. The ACF, elevon travel, original gear geometry/stiffness and prior 5× damping calibration remain unchanged from release 127. The airborne tests apply ordinary control inputs. Position/velocity initialization occurs only while paused, and path overrides are released before flight.





The first native screenshots showed the fixtures were absent under the session’s low object-density setting. The overlay now includes `sim/require_object 0/0`, which requires its objects at every density setting according to [Laminar’s DSF format specification](https://developer.x-plane.com/article/dsf-file-format-specification/). After a native scenery reload, the fixtures, their high/low parallax and their daylight/night visibility were confirmed. The earlier empty-scene screenshots are retained as rejected evidence; final cards have `density` in their filenames.





All configurations, raw traces, first-contact values, acceptance results and rejected trials are preserved here. The original 127 report remains historical evidence. Early setup attempts were rejected because asynchronous aircraft loads overlapped; the aircraft never entered an accepted test from the wrong setup. Release-load readiness now waits for actual rendered frames before loading the test aircraft. The startup menu was closed through the simulator UI before flight.
