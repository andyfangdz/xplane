# Shuttle approach and HUD correction — acceptance contract

Research and validation started 10 September 2026. Host: X-Plane 12.4.3, Windows, Vulkan/Zink. Aircraft: the existing aircraft-local Space Shuttle F-SIM HUD derivative; unmodified source aircraft retained separately. Baseline version 117 and its file hashes are preserved in this directory.

## Landing behavior

Use the late Shuttle program nominal, lightweight, dry runway case: 184,000 lb, no wind, Edwards 22L. Distances are measured from the displaced landing threshold, with positive distance down the runway. Use equivalent airspeed for Shuttle energy criteria. Indicated, equivalent and ground speeds are distinct measurements.

The correction must demonstrate the following through actual X-Plane physics and controls:

| Event | Reference | Acceptance for the simulated demonstration |
|---|---|---|
| Outer glideslope | 20 degrees below 222,000 lb; 18 degrees for heavyweight | Lightweight descent near 20 degrees; no arbitrary shallow approach |
| Outer approach energy | 300 KEAS | Stabilized within 10 KEAS before the 3,000 ft speedbrake retraction |
| Preflare | Starts about 2,000 ft; approximately 1.3 g; circular pullup followed by an exponential transition | Begins near 2,000 ft and rounds into the 1.5-degree inner path without the previous prolonged level segment |
| Speedbrakes | Energy control above 3,000 ft; retract setting computed at 3,000 ft, adjusted at 500 ft, then held to touchdown | Separate the outer speed regulation from the subsequent energy-to-touchdown schedule |
| Gear | 300 +/- 100 ft; down and locked at least five seconds before touchdown | Deploy at approximately 300 ft and verify actual deployment and time margin |
| Inner path | 1.5 degrees, intercept 1,000 ft downrange; brief stable interval before flare | Follow this geometry; measure its duration and descent rate |
| Final flare | Trigger depends on sink rate, bounded by 30–80 ft; targets about 3 ft/s descent at touchdown | Start inside that altitude band and land with a small downward velocity |
| Main gear touchdown | Nominal light vehicle target 195 KEAS at 2,500 ft downrange | 190–205 KEAS; 1,500–3,500 ft downrange; sink no greater than 5 ft/s; centerline error under 10 m |
| Rollout | Full speedbrake at main gear touchdown; light vehicle chute promptly after touchdown, derotate about two seconds later, not below 175 KEAS; chute jettison 60 KGS | Demonstrate event order, controlled nose lowering, and stop on runway |

These are demonstration tolerances, not a claim of Shuttle flight qualification. The handbook's off-nominal 185 KEAS option for vehicles below 200,000 lb requires a low-energy case; it is not the default for this nominal demonstration.

Primary sources: NASA [JSC-23266, Approach, Landing and Rollout Flight Procedures Handbook, Revision B, May 2005](https://www.ibiblio.org/apollo/Shuttle/Crew%20Training/Flight%20Procedures%20App%20Land%20Roll.pdf), sections 4.4–4.7 and 4.11, Appendix B.3; [USA005512, Entry, TAEM, and Approach/Landing Guidance Workbook, January 2006](https://www.ibiblio.org/apollo/Shuttle/Crew%20Training/Entry,%20TAEM%20and%20Approach%20Workbook.pdf), sections 4.2–4.5. Local copies, extraction and hashes are in `sources`.

## HUD optics

The previous version attached a custom avionics texture to the glass. Its symbology was readable, but this did not provide optical collimation. The replacement must use a projection whose angular position stays aligned with the distant scene when the eye translates. It must be clipped by the actual combiner opening and preserve correct angular scaling with field of view.

The preferred implementation is X-Plane's native 3-D HUD: emissive panel image, `ATTR_hud_glass` mesh, and calibrated panel rectangle and angular bounds. Laminar documents that this compositor provides the distant projection and VR eye handling. The stock F-14 passed a live check in the same graphics session, establishing that the host renderer supports the feature.

Acceptance views: centered eye; eye translated left/right and up/down; eye translated forward; camera rotation; multiple fields of view; external camera; power off/on; brightness; day/night. The decisive check is that a conformal symbol remains at the same outside direction while the physical glass moves across it. A texture that merely remains attached to the glass fails this check. VR support must be distinguished from an actual headset test.

Primary sources: [X-Plane 3-D HUD](https://developer.x-plane.com/article/x-plane-3-d-hud/), [Plugin Guidance for OpenGL Drawing](https://developer.x-plane.com/article/plugin-guidance-for-opengl-drawing/), [OBJ8 specification](https://developer.x-plane.com/article/obj8-file-format-specification/), and [USA006082 Star Tracker/HUD/COAS Workbook](https://www.ibiblio.org/apollo/Shuttle/Crew%20Training/S-TRK%20HUD%20COAS%20Workbook.pdf), section 7.3, which describes the Shuttle image focused at infinity.

## Implementation boundaries

The HUD and flown demonstration must read the same guidance definition. Aircraft motion during the recorded flight must arise from native aerodynamics responding to controls; do not command airborne position, attitude or velocities. Initial paused placement is permitted and its override must be released before flight. Log event timing, EAS, height, flightpath angle, sink rate, normal acceleration, gear state, speedbrake and touchdown location. Preserve both successful and failed test evidence. Validate plugin unload and restore temporary test settings before delivery.
