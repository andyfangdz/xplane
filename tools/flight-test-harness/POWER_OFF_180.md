# SR20 power-off 180 guidance v7

This temporary native test controller addresses the fast touchdowns and high roundout in the earlier recordings. `../../crates/poweroff180/src/guidance.rs` owns guidance; `../../crates/poweroff180/parameters.csv` owns parameters. Aircraft physics and the approved native HUD v5 are unchanged.

## Aircraft and entry

TorqueSim SR20, KCDW runway 22, full fuel requested through the add-on and 200 lb in each front seat. The v3 adapter verifies payload and stable full-tank readings, then records the measured starting mass and binds the unchanged 3 lb native mass-drift guard to that value. Validated starting masses were 2944.0–2945.0 lb. This replaces the assumption that every full-fuel load weighs exactly 2,942.5 lb.

Entry remains 100 KIAS at 1,000 ft AGL, approximately 1 NM downwind. Power is cut abeam the 1,000 ft runway point and remains idle. The turn target is 90 KIAS and the established final target is 75 KIAS. Full flaps are retained.

## Short-final speed and turn timing

Let H be configured headwind and C configured wind from the right, in knots. Negative H is tailwind. The final speed target changes smoothly between a wind-adjusted start height and 35 ft AGL:

```text
tail = max(0, -H)
strong_headwind = max(0, H - 10)
speed_margin = 0.3*strong_headwind
start_height = max(35+20, 120-4*strong_headwind)
landing_target = max(65, 70 - 0.4*tail - 0.07*abs(C))
f = clamp((start_height-height)/(start_height-35), 0, 1)
target_KIAS = 75 + speed_margin + (landing_target-75)*f*f*(3-2*f)
delay_seconds = max(0, 8.5 + (-0.445)*H + (-0.015)*strong_headwind + 0.02*tail + 0.223*C + (-0.034)*abs(C))
```

The tested 5 kt tailwind therefore uses a 68 KIAS late-final target. The speed change applies on established final, retaining the turn speed law and ordinary 1.25 degree/s pre-roundout pitch-command slew limit.

## Final-turn geometry

Throughout the final turn, the controller solves the bank required for the remaining circular arc to end on the centerline. It includes crosswind drift and predicts position and heading 0.7 seconds ahead to account for the aircraft's roll response. Its estimated mean speed over the remaining turn combines 40% of current IAS and 60% of the final target minus 1.5 kt, converted using the measured TAS/IAS ratio.

For predicted cross-runway position `Y`, estimated true speed `V`, predicted heading relative to the runway `b`, wind-corrected final heading `e`, and wind-from-right component `C` in feet per second:

```text
N = V*V*(cos(e) - cos(b)) + C*V*max(0, e-b)
turn_bank = degrees(atan2(max(0, N), 32.174*max(10, Y)))
```

Angles inside the formula are radians. The turn command is limited to 0–25°. This changes the radius in response to position and speed differences before they grow into a late alignment correction. The radius command blends into the following centerline feedback between 20° and 5° of remaining heading error.

## Centerline capture

The controller estimates lateral acceleration from native-frame cross-runway velocity with a 0.35 s low-pass filter. With position `y`, velocity `v`, acceleration `a` and lookahead `T = 0.7 s`, it uses:

```text
predicted_position = y + v*T + 0.5*a*T*T
predicted_velocity = v + a*T
bank = degrees(atan2(-0.16*predicted_position - 0.68*predicted_velocity, 32.174)) + 1.5
```

Distances are feet, time seconds, and bank degrees. The centerline bank command is limited to ±20°, reducing to ±7° below 50 ft AGL. The final-phase transition at 3° retains the same lateral law, avoiding a command jump.

## Approach position and lower roundout

Path feedback operates on final between 180 ft and the 35 ft roundout. It projects the intersection with roundout height, then applies a pitch bias limited to ±0.8 degrees:

```text
travel_ft = 680 - 11.7*H - 0.28*H*H - 0.9*C
along_speed = max(80, groundspeed_fps*cos(track-runway_heading))
sink = clamp(-vertical_velocity_fps, 10, 20)
projected_roundout_x = x + (height-35)*along_speed/sink
pitch_bias = clamp(-0.008*(projected_roundout_x-(1100-travel_ft)), -0.8, 0.8)
```

Here H and C are measured wind components. Travel is an empirical approximation for this aircraft and loading. It targets roundout position; it is not an explicitly selected fixed ground aimpoint. The earlier analysis did not establish that aimpoint placement alone caused the fast landings.

Roundout starts at 35 ft and preserves pitch-command continuity. With physical vertical velocity vy and its filtered acceleration a in feet/s units:

```text
projected_height = max(0, height + 0.8*vy)
desired_vy = -sqrt(0.6^2 + 2*1.3*projected_height)
strong_headwind = max(0, configured_H - 10)
base_rate = max(0.3, 2.2 - 0.015*abs(configured_C))
upward_rate_limit = base_rate + min(0.6, 0.05*strong_headwind + max(0, wind_rate_feedforward))
pitch_command_limit = min(12, 8.9 + 0.08*strong_headwind)
measured_pitch_rate = low_pass(d(pitch)/dt, 0.06 seconds)
pitch_rate_command = max(-0.8, limited_rate_command - 1.5*max(0, measured_pitch_rate-2.4))
```

Vertical-speed error, acceleration error and measured wind loss set pitch-command rate within these limits. Measured-rate damping follows command saturation so it remains effective under maximum demand. See the source for the complete law. Command limits and actual measured pitch/rate are separate. The maximum configured rate is 2.8 degrees/s in the tested winds. The physical motion gates remain 7.5 degrees pitch and 2.8 degrees/s pitch rate.

## Reproduce and assess

With X-Plane closed, run `scripts/Test-Harness.ps1`, then `Run-XPlaneTest.ps1 -Config configs/poweroff180-lower-flare.json`. Run the seven-wind card again in a fresh process for a repeat. Add `-RecordVideo` with the three-video card for native HUD and synchronized audio evidence. Original installation settings are restored after each session.

Acceptance requires touchdown at 1,000–1,200 ft, offset ≤8 ft, short-final offset ≤15 ft, physical sink ≤200 fpm, touchdown speed 63–67 KIAS, roundout ≤35 ft, pitch ≤7.5 degrees, pitch rate ≤2.8 degrees/s and post-contact height ≤2.5 ft. The 60 KIAS airborne abort remains. Every attempt is retained.

These are measured simulator settings for the tested SR20, loading and wind cases. [The release report](../../docs/poweroff180/lower-flare-report.md) links all evidence, source hashes and the historical landing references.

## Gusty wind

Guidance v9 added gust handling, v10 energy management and v11 the pilot decisions below: rounding out earlier when sinking fast, and in the flare putting it down when long or holding it off when short. A card may add `wind_gust_kt`, sent to X-Plane as `gust_increase_in_knots` on every wind layer and read back while paused. Steady cards are unchanged. The [controller README](../../plugins/poweroff180-controller/README.md#gusts) describes the measured gust behavior and the mean-wind guidance. With G the gust increase and w = clamp(1 + cos(wind offset), 0, 1):

```text
mean_wind = steady + 0.35*G                    (turn timing)
final_target   = 75 + 0.7*G + (existing strong-headwind term)
landing_target = existing landing target + min(0.7*G*w, 0.7*G)
delay_seconds  = existing law at mean_wind - 0.28*0.7*G*w
if delay_seconds < 0: turn at the cut; hold half flaps until final shows
    a 150 ft projected surplus, or 150 ft AGL, or 600 ft from the threshold
delay_seconds += 0.1*w  (v11 plans slightly low; v10 turned 0.3*w earlier as a reserve)

energy management on final (E = projected roundout - aim, feet;
aim = 1100 - 40 - flare travel, travel += 30*G*max(0, -cos(offset))):
path pitch bias = clamp(-0.008*E, -down, 1.3) + transition
    down fades from 1.3 at 100 ft to 0.4 at 35 ft
transition (below 100 ft) = min(0.4 * max(0, sink - profile(h)), 3) deg,
    profile(h) = sqrt(9^2 + 2*0.8*(h - 35)) ft/s   (540 fpm at 35 ft)
slip (full flaps, before roundout) = min(0.03*(E - 50), 8) deg,
    upwind wing low, faded out between 100 and 70 ft, <= 3 deg/s

flare touchdown decision (T = (aim - x) / along-runway groundspeed,
aim = 1100 - 40; easing at constant deceleration to a contact sink cs
takes 2h/(sink + cs), which on the planned profile is its time to go):
needed = 2h/T - sink
put it down: if IAS <= 66.5 + landing margin, cs = clamp(needed, 0.6, 4) ft/s
hold it off: if needed < 0.6 and IAS > 65, desired sink <= max(2h/T - 0.6, 0.5)

gust flare (h = height ft, q = achieved pitch rate, all rates deg/s):
demand     = steady flare law, velocity gain 0.7, sink profile 1.1 ft/s^2,
             capped at 2.2 and ramped in over the first 0.8 s
reference  = demand lagged 0.3 s
correction += 0.5 * (reference - q) * dt, limited to +/-0.8 deg
holding    = 1.65 * clamp((33 - h)/33, 0, 1) + correction
rate       = demand - 0.8*(q - reference) - 1.5*max(0, q - 2.4),
             limited so pitch stays <= 7 deg
command    = pitch + holding + 0.4*rate, slewed at <= 2.5 deg/s
```

Gust cards keep every steady acceptance limit except two: physical sink ≤300 fpm (`maximum_gust_sink_fpm`; X-Plane decays the gust in the last ~50 ft, so a power-off flare always meets a headwind loss), and the touchdown-speed ceiling rises by 0.7·G·w. The wind is valid from steady − 2 kt to steady + G + 2 kt.

`configs/gust-matrix.json` flies the six gust cases. X-Plane replays nearly the same gust sequence for a given load, so repeats alone understate variation: `configs/gust-matrix-phased.json` flies each case at three downwind start distances (`start_along_ft` 12,500, 14,000 and 15,500), which shifts the gust phase at the landing.
