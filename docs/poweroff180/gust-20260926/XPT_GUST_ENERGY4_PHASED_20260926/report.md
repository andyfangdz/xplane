# Native X-Plane harness report

**9/18 flights passed the landing limits.** Measurement-valid flights: 18. Installation restored: True.

Landing pass/fail is separate from harness operation. Native guidance runs on simulator frames; Python only supervises it. All attempts, including aborted and non-passing flights, remain in this report.

| Trial | Touchdown ft | KIAS | Physical sink fpm | Peak pitch ° | Peak pitch rate °/s | Result |
|---|---:|---:|---:|---:|---:|---|
| cross_left10_gust8-01 | 1196.0 | 71.1 | 157.9 | 3.3 | 2.5 | Pass |
| cross_left10_gust8_early-01 | 990.0 | 72.1 | 44.0 | 3.7 | 2.8 | Touchdown distance |
| cross_left10_gust8_late-01 | 1183.3 | 70.2 | 169.3 | 3.4 | 2.0 | Pass |
| cross_right10_gust8-01 | 1109.2 | 69.3 | 99.9 | 4.5 | 2.4 | Pass |
| cross_right10_gust8_early-01 | 1207.9 | 73.1 | 144.7 | 2.7 | 2.6 | Touchdown distance; Touchdown speed |
| cross_right10_gust8_late-01 | 1092.2 | 71.8 | 133.0 | 3.2 | 2.4 | Pass |
| head10_gust10-01 | 1186.3 | 68.7 | 243.7 | 4.0 | 2.7 | Pass |
| head10_gust10_early-01 | 1075.9 | 72.3 | 81.1 | 3.8 | 2.3 | Pass |
| head10_gust10_late-01 | 1092.6 | 73.8 | 396.3 | 1.5 | 2.5 | Physical sink |
| head15_gust8-01 | 1222.3 | 66.1 | 305.3 | 5.8 | 2.5 | Touchdown distance; Physical sink |
| head15_gust8_early-01 | 1234.0 | 70.6 | 177.1 | 3.7 | 2.8 | Touchdown distance; Alignment |
| head15_gust8_late-01 | 1095.1 | 72.8 | 356.2 | 2.2 | 2.6 | Physical sink; Touchdown speed |
| head5_gust8-01 | 973.7 | 71.5 | 416.7 | 2.3 | 2.6 | Touchdown distance; Physical sink |
| head5_gust8_early-01 | 1139.0 | 68.9 | 184.8 | 4.2 | 2.5 | Pass |
| head5_gust8_late-01 | 1101.5 | 71.2 | 166.2 | 3.7 | 2.3 | Pass |
| tail5_gust5-01 | 1248.4 | 65.5 | 145.1 | 6.4 | 2.1 | Touchdown distance |
| tail5_gust5_early-01 | 1395.1 | 65.4 | 143.0 | 6.2 | 2.4 | Touchdown distance |
| tail5_gust5_late-01 | 1107.7 | 66.6 | 44.7 | 6.0 | 1.8 | Pass |

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
