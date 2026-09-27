# SR20 power-off 180 in gusty wind (guidance v11)

Guidance v9–v11 flies the TorqueSim SR20 power-off 180 in gusty wind on X-Plane 12.4.4 beta 1. The target envelope is 10 kt sustained with gusts to 20 kt. v11 replaces tuning with the decisions a pilot makes: round out earlier when sinking fast, and in the flare put it down when long or hold it off when short. Steady cards are unchanged: with no gust, the frozen C++ replays of guidance and attitude still match frame for frame.

## Guidance v11 in the 10G20 envelope

**Result.**

- **Precision.** Across ten conditions within 10 kt gusting 20, each at three gust timings, the final source passed every precision limit in [20 of 30 flights](XPT_GUST_V11_RESERVE_20260927/summary.json). All 30 were measurement-valid, with no aborts.
- **Baseline.** v10 passed 16 of the same 30 cards and 25 of 45 in-envelope flights overall, so v11 is 67% against 56%.
- **Landing quality.** Firm landings (over 300 fpm) went from 5 of 45 to 1 of 60. The worst sink fell from 400 to 306 fpm.
- **Worst cases.** Flare pitch rate 2.70°/s, pitch 6.7°, touchdown offset 2.8 ft.
- **Remaining misses.** Touchdown distance (5 long, 4 short), one 306 fpm landing and one touchdown-speed miss (+0.7 kt in a quartering tailwind).
- **Steady wind.** The [steady smoke](XPT_GUST_V11_STEADY_20260927/summary.json) on the final build passed 6/6 (touchdowns 1,032–1,113 ft, sink ≤170 fpm).

| Campaign (same 30 cards) | Passes | Touchdown mean / SD | Sink >300 fpm | Worst sink |
|---|---|---|---|---|
| v10 ([repeat](XPT_GUST_ENERGY6_REPEAT_20260927/summary.json) + [extended](XPT_GUST_EXTENDED_20260927/summary.json)) | 16 | 1,136 / 95 ft | 3 | 400 fpm |
| [v11, reserve 0.15 s earlier](XPT_GUST_V11_ENVELOPE_20260927/summary.json) | 19 | 1,151 / 102 ft | 0 | 290 fpm |
| [v11 final, base 0.1 s later](XPT_GUST_V11_RESERVE_20260927/summary.json) | 20 | 1,106 / 99 ft | 1 | 306 fpm |

| Condition (3 timings) | v10 | v11 final | v11 touchdown ft |
|---|---|---|---|
| Left crosswind 10G18 | 2 | 3 | 1,015–1,106 |
| Headwind 10G20 | 1 | 3 | 1,115–1,137 |
| Quartering left 10G16 | 1 | 3 | 1,010–1,167 |
| Right crosswind 10G18 | 2 | 2 | 1,049–1,269 |
| Right crosswind 6G12 | 2 | 2 | 1,060–1,290 |
| Headwind 7G13 | 2 | 2 | 993–1,111 |
| Tailwind 5G10 | 2 | 2 | 1,054–1,323 |
| Headwind 5G13 | 2 | 1 | 943–1,057 |
| Quartering right 8G16 | 1 | 1 | 886–1,244 |
| Quartering tailwind 6G10 | 1 | 1 | 1,109–1,202 |

`configs/gust-envelope.json` flies the matrix: the phased and extended conditions without 15G23 and 12G18.

### What a pilot would do, and what the data said

A [longitudinal sim](analysis/flaresim/) screened each decision before flight time. It runs the real controller from each recorded flight's state at 150 ft, in that flight's recorded wind:

- **Model.** A point-mass SR20 whose lift model was fitted on the gust traces (R² 0.99), with drag scaled to the measured glide and the fitted attitude-helper response.
- **Validation.**
  - On held-out conditions it reproduced v10 touchdowns within 29 ft (SD, r = 0.94).
  - It scored v10 at 16/30 and v11 at 19/30, exactly as flown.

1. **Round out earlier when sinking fast.** Every v10 gust flight reached the 35 ft roundout at ~685 fpm, but the flare's profile expects ~530 fpm. The firm ones then met the gust decay (head5_gust8: 850 fpm at 100 ft, 690 at roundout, 398 at touchdown). Below 100 ft, v11 raises the nose 0.4° per ft/s of sink above a profile that eases to 540 fpm at 35 ft, up to 3°. This sits outside the path authority: it stops the descent, it doesn't steer it. In the sim, sink failures went from 4 to 0.
2. **Know where the wheels will touch.** v10 predicted touchdown by extrapolating the unflared sink. That read short at roundout (1,054 ft predicted, 1,269 ft flown) until it was too late. Easing from the present sink to a contact sink cs at constant deceleration takes 2h / (sink + cs), which on the planned profile is exactly its time to go. v11 picks the cs that lands on the aim.
   - **Put it down when long,** at up to 240 fpm, but only once slow enough to land (66.5 kt plus the gust allowance). Planting a fast airplane only converts a distance miss into a speed miss.
   - **Hold it off when short,** descending only as fast as reaches the aim, while above 65 kt.
3. **Plan slightly low.** Holding off can stretch a landing by hundreds of feet. Putting it down gains only ~100 ft, and only once the speed allows. So a short error is recoverable and a long one mostly is not.
   - **Change.** v10 turned base 0.3 s early to keep a reserve for the slip. v11 turns 0.1 s *later* than the mean-wind law (not in a tailwind).
   - **Measured effect.** Each 0.1 s is ~4 ft of energy on final, measured between the two v11 runs. The change moved mean touchdown from 1,151 to 1,106 ft.
4. **Slip into short final.** The slip now fades out between 100 and 70 ft instead of ending by 110 ft.

**Tried and dropped.** These moved speed but not energy, so they made landings longer in the sim:

- A smaller speed margin (½ gust factor instead of 0.7).
- No landing margin in crosswinds, where a decaying gust costs little airspeed.

Inside the last 150 ft, slowing down converts speed into height; only drag and base timing change the energy that reaches the runway.

### Limits

- **Scatter.** The mean is now centered, but touchdown SD is still ~99 ft against a ±100 ft box. Misses repeat by card and gust timing: right crosswind and quartering right at the early timing are long in every run, and the late headwind timings are short.
- **Energy at 150 ft.** With the recorded gusts replaced by mean wind below 150 ft, the sim's scatter grows rather than shrinks. The spread is inherited from the energy state on final, yet no linear function of position, speed or sink at 150–300 ft predicts touchdown (residual SD 90 ft of 92).
- **Flare authority.** Short landings come from gust decay in a flare already near the 2.8°/s motion limit (v11 median 2.44°/s, worst 2.71°/s), so holding off harder is not available.
- **Speed at roundout.** Long landings arrive at roundout ~5 kt above the touchdown ceiling and must float that off before they may land.

## Guidance v10

**Result.**

- **Safety.** Every gust flight completes the maneuver without an abort.
- **Precision.** Across six gust conditions at three gust timings, the final source met every precision limit in 20 of 36 flights, flown in two hash-identical 18-flight runs of 10 each. All 36 were measurement-valid.
  - Worst cases: flare pitch rate 2.74°/s, touchdown offset 2.7 ft, sink 398 fpm.
- **Remaining misses.** Mostly touchdown distance just outside the 1,000–1,200 ft box (13), then four firm landings, two capture overshoots and two touchdown-speed misses.

| Campaign | Flights | Valid | Strict passes |
|---|---|---|---|
| [v10 gust matrix, three timings](XPT_GUST_ENERGY6_REPEAT_20260927/summary.json) | 18 | 18 | 10 |
| [Same source, first run](XPT_GUST_ENERGY6_PHASED_20260926/summary.json) | 18 | 18 | 10 |
| [v10 steady smoke](XPT_GUST_ENERGY_STEADY_20260927/summary.json) | 6 | 6 | 6 |

| Condition (v10, 6 flights each) | Passes | Touchdown ft | Sink fpm |
|---|---|---|---|
| Left crosswind 10G18 | 5 | 994–1,191 | 26–176 |
| Right crosswind 10G18 | 4 | 1,086–1,272 | 56–162 |
| Headwind 5G13 | 4 | 990–1,114 | 149–398 |
| Tailwind 5G10 | 3 | 930–1,304 | 41–151 |
| Headwind 10G20 | 2 | 987–1,173 | 109–386 |
| Headwind 15G23 | 2 | 1,140–1,229 | 183–292 |

**Generalization.** [Six additional conditions](XPT_GUST_EXTENDED_20260927/summary.json) flown by the unchanged v10 source passed 10 of 18. The conditions were headwinds 7G13 and 12G18, quartering 10G16 from the left and 8G16 from the right, right crosswind 6G12 and quartering tailwind 6G10, each at three timings (`configs/gust-matrix-extended.json`). That matches the original six conditions.

**Energy management compared with v9** (36 flights each, same matrix):

| | v9 (no energy management) | v10 |
|---|---|---|
| Strict passes | 19/36 | 20/36 |
| Touchdown SD | 106–108 ft | 90–97 ft |
| Sink >300 fpm | 6 | 4 |
| Touchdown-speed misses | 5 | 2 |

The pass rate is the same within noise; landing quality is better.

## What X-Plane's gusts do

- **Behavior.** [Probe R6](XPT_GUST_PROBE_R6_20260925/) measured gusts that rise from the steady speed toward the peak over 1–2 s and hold for several seconds. The mean is steady + 0.34 × gust, and direction stays constant. True airspeed follows the gust on the same frame; indicated airspeed lags about 1 s.
- **Near-surface decay.** The gust decays to the steady wind in the last ~50 ft, so every gusty flare meets a headwind loss.
- **Repeatability.** For a given load the gust sequence is nearly repeatable, so the matrix shifts its phase with the downwind start distance (12,500, 14,000 and 15,500 ft).
- **Sensitivity.** Height differences of 10–50 ft between timings move the roundout point by up to ~500 ft, because the pre-flare path is shallow. Float after roundout adds its own ~±90 ft.

### v10 design

All gust logic is inert when `wind_gust_kt = 0`. [POWER_OFF_180.md](../../../tools/flight-test-harness/POWER_OFF_180.md#gusty-wind) has the formulas; the [controller](../../../plugins/poweroff180-controller/README.md#gusts) and [attitude helper](../../../plugins/poweroff180-attitude/README.md) READMEs summarize them.

1. **Mean-wind guidance.** Airspeed logic uses |ground velocity + 8 s mean wind|. The final-turn arc uses the measured wind. Safety checks stay raw, and the wind guard accepts the steady-to-peak band ±2 kt.
2. **Speed margins.** 0.7 × gust is added to the final and landing targets, keeping the short-final deceleration as a pre-flare. The landing margin fades to zero in a tailwind.
3. **Energy management.**
   - **Reserve.** Base comes 0.3 s earlier (not in a tailwind).
   - **Flaps.** Half flaps are held on 15G23, which cannot turn earlier, until final shows a surplus.
   - **Path authority.** ±1.3° of path pitch authority, with nose-down authority fading before roundout, and a pre-flare nose-up bias when sink exceeds 560 fpm below 100 ft.
   - **Slip.** On final, a forward slip (up to 8°, upwind wing low) dissipates projected surplus and ends by 110 ft.
   - **Aim.** The aim is 40 ft short of 1,100 ft, correcting the measured mean, and tailwind gusts aim the roundout 30 ft per knot earlier because they float.
   - **Helper change.** The attitude helper gained a sideslip target with blended yaw authority and rudder feed-forward. Achieved sideslip reached ~5–6° for an 8° command.
4. **Rate-commanded flare.**
   - **Model.** The helper answers a command lead as rate = (lead − L₀(h)) / 0.4 s. The holding lead L₀ grows from ~0.2° at 30 ft to ~1.7° at the runway as full flaps and ground effect pitch the nose down (24 flares, R² 0.8–0.9).
   - **Law.** Gust flares command pitch + L₀(h) + 0.4 s × rate, adapt L₀ slowly against a 0.3 s reference response, and feed back the achieved rate. The demand is capped at 2.2°/s and ramped in, and attitude is capped at 7°.
   - **Touchdown control.** The terminal sink rises with the predicted overrun, up to 240 fpm, so long floats settle sooner.

## Development evidence

Every run is retained. Full traces are kept only for the final v11 matrix ([XPT_GUST_V11_RESERVE_20260927](XPT_GUST_V11_RESERVE_20260927/)) and the characterization probe; the rest keep assessments. The v10 traces are in the history of commit 1bc666c. The [analysis scripts](analysis/) take trace globs and were run against the complete harness output.

| Step | Runs | Finding |
|---|---|---|
| Characterization | [probes](XPT_GUST_PROBE_R6_20260925/) | Gust shape, TAS/IAS lag, near-surface decay |
| Readiness race | [probe, R1, R3](XPT_GUST_PROBE_R3_20260925/) | Flights requested before X-Plane's main menu never resumed (3/5). A 20 s API settle gate fixed it; all later sessions resumed |
| First gust design | [validation](XPT_GUST_VALIDATION_20260926/) | 8/12 at standard timing; 2 flare-rate and 2 distance misses |
| Rate-commanded flare | [iterations](XPT_GUST_RATEFLARE1_20260926/) | Integral lead correction oscillated (7.7°/s); a lead without holding term ran away (−6.8°/s); the affine helper model fixed both |
| v9 final | [runs](XPT_GUST_FINAL_PHASED_20260926/), [twin](XPT_GUST_PHASED2_20260926/) | 19/36 across timings |
| Energy management | [iterations](XPT_GUST_ENERGY4_PHASED_20260926/) | A half-flap reserve through base overshot (roundouts 630–1,093 ft). The slip needed more rudder (3° → 5.5°). Aiming the roundout point missed float variance; touchdown-point control and a centered aim fixed it. An overriding pre-flare made landings long and pitchy ([5/18](XPT_GUST_ENERGY7_PHASED_20260926/)) and was reverted |

## Base-leg energy controller: investigated, not built

The plan was to angle the base leg toward the runway when low and away when high. Such a controller needs an estimate of energy relative to the remaining path, and it may only act on one that is unbiased across wind conditions and predicts touchdown within them. The [analysis scripts](analysis/) are retained.

**Estimators tested.** Five families, on 54 v10 flights across 12 conditions, validated leave-one-condition-out:

- **Geometric energy.** Height against planned path times.
- **Measured-sink projections.**
- **Integrated gust drift.** ∫(measured − expected wind): the displacement is only 8–28 ft SD, too small to matter.
- **Air-relative energy height** against a steady-wind drag polar: between-condition bias is low (7 ft), but it does not predict touchdown within a condition (r ≈ 0).
- **Polar-planned surplus.** Its between-condition bias has the wrong sign, e.g. the tailwind reads 46–67 ft short but lands long.

**Result** ([fit12.py](analysis/fit12.py)).

- Within a condition, touchdown scatter stays about 80 ft whatever base-leg height, position or energy terms are added.
- On held-out conditions, every base-state model does worse than doing nothing: 104–301 ft RMS against 119 ft.
- The final-approach controls already absorb base-leg energy errors. The remaining scatter comes from gusts late on final and the float after roundout, which the base leg cannot see.
- An early suggestion that base state explained 45% of the scatter did not survive. The repeats are nearly deterministic, so 36 flights hold about 18 independent gust timings, and the more complete tests found no signal.

**Aim recentering.**

- **Held-out prediction.** Only recentering generalized: held-out RMS went from 119 to 95 ft, with wind-specific terms adding nothing beyond noise.
- **Flown test.** A −75 ft aim over 36 flights ([phased](XPT_GUST_AIM75_PHASED_20260927/summary.json), [extended](XPT_GUST_AIM75_EXTENDED_20260927/summary.json)) moved mean touchdown only 7–12 ft and passed 18/36, against 20/36. The flare and float absorb most of an aim change, so it was reverted.

### v10 limits and open items

- **Touchdown scatter.** It is ~85–97 ft (SD) against a 200 ft box, and is not predictable from base-leg state (see above). What remains are late-final gusts and float variation, which only faster flare-phase control or a model of the gust itself could address.
- **Firm landings.** 5G13 standard (~380–400 fpm) and 10G20 late (~370–390 fpm) are firm in every run. Both are steep, high-energy entries. The one iteration with a stronger, overriding pre-flare also added extra turn lead and scored 5/18, so the pre-flare's effect alone is untested.
- **Slip effectiveness.** Achieved sideslip reached ~5–6° for an 8° command, and the extra sink is modest in this model.
- **15G23 early timing.** A repeatable 18–20 ft capture overshoot remains (extra turn lead was tried in [iteration 7](XPT_GUST_ENERGY7_PHASED_20260926/) alongside the overriding pre-flare and reverted with it).
- **Scope.** Turbulence and shear direction remain untested.
