# Native X-Plane harness report

**8/18 flights passed the landing limits.** Measurement-valid flights: 18. Installation restored: True.

Landing pass/fail is separate from harness operation. Native guidance runs on simulator frames; Python only supervises it. All attempts, including aborted and non-passing flights, remain in this report.

| Trial | Touchdown ft | KIAS | Physical sink fpm | Peak pitch ° | Peak pitch rate °/s | Result |
|---|---:|---:|---:|---:|---:|---|
| cross_left10_gust8-01 | 1165.2 | 69.7 | 178.7 | 4.0 | 2.4 | Pass |
| cross_left10_gust8_early-01 | 857.1 | 73.3 | 67.8 | 3.3 | 2.7 | Touchdown distance; Touchdown speed |
| cross_left10_gust8_late-01 | 1109.2 | 70.1 | 166.7 | 3.7 | 1.8 | Pass |
| cross_right10_gust8-01 | 1083.2 | 68.3 | 82.1 | 4.9 | 2.4 | Pass |
| cross_right10_gust8_early-01 | 1205.2 | 72.1 | 125.1 | 3.3 | 2.6 | Touchdown distance |
| cross_right10_gust8_late-01 | 1089.7 | 71.0 | 106.0 | 3.4 | 2.4 | Pass |
| head10_gust10-01 | 1140.1 | 68.3 | 243.7 | 4.4 | 2.6 | Pass |
| head10_gust10_early-01 | 1115.7 | 71.0 | 37.1 | 4.0 | 2.5 | Pass |
| head10_gust10_late-01 | 1071.7 | 74.0 | 351.2 | 1.8 | 2.6 | Physical sink |
| head15_gust8-01 | 1183.7 | 65.7 | 281.0 | 6.2 | 2.6 | Pass |
| head15_gust8_early-01 | 1112.8 | 72.0 | 213.2 | 3.1 | 2.6 | Alignment |
| head15_gust8_late-01 | 1132.7 | 73.7 | 321.6 | 2.2 | 2.7 | Physical sink; Touchdown speed |
| head5_gust8-01 | 945.5 | 71.2 | 378.9 | 2.8 | 2.4 | Touchdown distance; Physical sink |
| head5_gust8_early-01 | 1099.1 | 67.8 | 185.9 | 4.5 | 2.4 | Pass |
| head5_gust8_late-01 | 994.7 | 71.3 | 177.0 | 4.4 | 2.3 | Touchdown distance |
| tail5_gust5-01 | 1204.0 | 64.5 | 146.8 | 7.0 | 2.2 | Touchdown distance |
| tail5_gust5_early-01 | 1357.8 | 65.2 | 98.0 | 6.1 | 2.5 | Touchdown distance |
| tail5_gust5_late-01 | 998.0 | 66.8 | 41.1 | 6.0 | 2.5 | Touchdown distance |

## cross left10 gust8

![Flight-path, speed, pitch and descent overlays](charts/cross_left10_gust8.png)

- Fewer than two traces; repeatability is not established

## cross left10 gust8 early

![Flight-path, speed, pitch and descent overlays](charts/cross_left10_gust8_early.png)

- Fewer than two traces; repeatability is not established

## cross left10 gust8 late

![Flight-path, speed, pitch and descent overlays](charts/cross_left10_gust8_late.png)

- Fewer than two traces; repeatability is not established

## cross right10 gust8

![Flight-path, speed, pitch and descent overlays](charts/cross_right10_gust8.png)

- Fewer than two traces; repeatability is not established

## cross right10 gust8 early

![Flight-path, speed, pitch and descent overlays](charts/cross_right10_gust8_early.png)

- Fewer than two traces; repeatability is not established

## cross right10 gust8 late

![Flight-path, speed, pitch and descent overlays](charts/cross_right10_gust8_late.png)

- Fewer than two traces; repeatability is not established

## head10 gust10

![Flight-path, speed, pitch and descent overlays](charts/head10_gust10.png)

- Fewer than two traces; repeatability is not established

## head10 gust10 early

![Flight-path, speed, pitch and descent overlays](charts/head10_gust10_early.png)

- Fewer than two traces; repeatability is not established

## head10 gust10 late

![Flight-path, speed, pitch and descent overlays](charts/head10_gust10_late.png)

- Fewer than two traces; repeatability is not established

## head15 gust8

![Flight-path, speed, pitch and descent overlays](charts/head15_gust8.png)

- Fewer than two traces; repeatability is not established

## head15 gust8 early

![Flight-path, speed, pitch and descent overlays](charts/head15_gust8_early.png)

- Fewer than two traces; repeatability is not established

## head15 gust8 late

![Flight-path, speed, pitch and descent overlays](charts/head15_gust8_late.png)

- Fewer than two traces; repeatability is not established

## head5 gust8

![Flight-path, speed, pitch and descent overlays](charts/head5_gust8.png)

- Fewer than two traces; repeatability is not established

## head5 gust8 early

![Flight-path, speed, pitch and descent overlays](charts/head5_gust8_early.png)

- Fewer than two traces; repeatability is not established

## head5 gust8 late

![Flight-path, speed, pitch and descent overlays](charts/head5_gust8_late.png)

- Fewer than two traces; repeatability is not established

## tail5 gust5

![Flight-path, speed, pitch and descent overlays](charts/tail5_gust5.png)

- Fewer than two traces; repeatability is not established

## tail5 gust5 early

![Flight-path, speed, pitch and descent overlays](charts/tail5_gust5_early.png)

- Fewer than two traces; repeatability is not established

## tail5 gust5 late

![Flight-path, speed, pitch and descent overlays](charts/tail5_gust5_late.png)

- Fewer than two traces; repeatability is not established

## Evidence

- `resolved-config.json` and each card’s `effective-config.json`: complete settings, with no inactive legacy fields.
- `native-effective.ini`: exact configuration read back from the plugin before flight.
- `trace.csv`: native-frame observations; `supervision.json`: independent polling and deliberate gap probes.
- `source-manifest.json`: harness, native binary, setup adapter and original aircraft lineage.
- `events.jsonl`, `status.json`: structured progress; `restoration.json`: recovery verification.
- `summary.json`: metrics and repeat-divergence timestamps; `charts/*.svg`: standalone vector figures.
