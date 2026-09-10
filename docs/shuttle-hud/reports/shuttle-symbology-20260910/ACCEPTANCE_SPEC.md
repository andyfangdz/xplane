# HUD symbology acceptance contract

Defined against baseline release 136 before implementation.

## Preserved behavior

- Original aircraft files, derivative ACF, cockpit/combiner geometry, native optical configuration, runway scenery, landing-guidance header, chute physics and validation-pilot control laws remain at the accepted baseline.
- Presentation state does not write flight position, attitude, velocity, forces, AP modes or control inputs.
- The current final approach remains native physics, with the same 195–205 KEAS touchdown acceptance contract. A flown regression must pass before release.

## Reference-driven behavior

Use JSC-23266 Rev B section 2.12 for symbol definitions, source values, gear timing and contact transitions. Use sections 5.3.3 and 5.3.6 for CAPT/OGS/FLARE/FNLFL sequencing. Retain F-SIM's automatic declutter as an explicitly selectable convenience alongside the manual cycle. Use real footage to corroborate horizon retention during airborne declutter 2 and removal after nosewheel contact.

| Card | Required evidence |
|---|---|
| Banked HAC | Body-fixed square flight director, rotating pitch ladder, stable tape locations, guidance and Nz; document geometry-derived early phase |
| Prefinal | PRFNL, five-second FD-to-VV transition, body-parallel vector wings, drift follows ground velocity, ATT REF horizontal cage if implemented |
| Capture and OGS | CAPT/OGS sequence using measured path errors; nominal OGS triangles distinct from guidance diamond |
| Flare preview | Second pair appears around 3,500–3,800 ft and approaches OGS pair continuously; no duplicate pair after merge |
| Preflare/IGS | FLARE, one flare-reference pair, no persistent gear-down text after its five-second interval |
| CSS final flare | FNLFL, speed/radar altitude and VV retained, diamond and gamma triangles absent |
| AUTO final flare | Corresponding native AUTO/control-source state permits guidance cues; validation-pilot activity alone must not falsely claim Shuttle AUTO |
| Gear | Flashing GEAR for up below 300 ft and 300 KEAS, GR while moving, GR-DN on all-three locked for five seconds; clears at WOW |
| Main contact and rebound | Display WOW latched, speed relocates beside boresight; airborne symbols clear; pitch references return; deceleration scale appears; rebound does not revert to airborne format |
| Nose contact and rollout | G-prefixed groundspeed; no VV/altitude/runway/modes/gear; pitch ladder and horizon clear; deceleration and speedbrake remain |
| Declutter | Full, runway off, digital data, boresight-only; distinct post-WOW cycle; predictable return to full format |
| Numeric boundaries | Digital altitude truncation checked at 50, 400, 1,000 ft; tape increments checked at 500, 1,000, 100,000 ft; integer speed and radar suffix |
| Limits and alerts | FOV clipping, VV X, flashing limited diamond, >20-degree speedbrake mismatch, Nz flash when displayed |
| Optical and view regression | Native cockpit center/off-axis view, full-screen behavior, brightness and power, no changed combiner/atlas/ACF |
| Lifecycle | Live disable/re-enable, paused state, replay/time rewind reset, aircraft switch/reload, normal simulator shutdown with cleanup log |

Pure-state tests must cover meaningful boundary and transition cases. Native screenshots must verify the actual compiled renderer, with telemetry showing the inputs and symbol flags. Final evidence must identify observed flight states versus deliberately initialized paused display cards. Retain failed cards and build/configuration hashes.

## Honest limits

This aircraft lacks Shuttle GPC phase flags, complete TAEM/HAC guidance, MLS fault data and authentic braking guidance software. Early-phase estimates and any reconstructed deceleration guide must be described as local equivalents; no fabricated fault/status annunciations. The two physical weight endpoints and their existing limitations remain the performance scope. Exact font photometry and mission-specific software differences cannot be recovered from compressed video.
