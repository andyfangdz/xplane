# Native X-Plane harness report

**10/18 flights passed the landing limits.** Measurement-valid flights: 18. Installation restored: True.

Landing pass/fail is separate from harness operation. Native guidance runs on simulator frames; Python only supervises it. All attempts, including aborted and non-passing flights, remain in this report.

| Trial | Touchdown ft | KIAS | Physical sink fpm | Peak pitch ° | Peak pitch rate °/s | Result |
|---|---:|---:|---:|---:|---:|---|
| cross_left10_gust8-01 | 1191.1 | 71.1 | 163.4 | 3.3 | 2.5 | Pass |
| cross_left10_gust8_early-01 | 994.3 | 72.2 | 56.1 | 3.7 | 2.7 | Touchdown distance |
| cross_left10_gust8_late-01 | 1152.6 | 70.6 | 171.1 | 3.3 | 2.2 | Pass |
| cross_right10_gust8-01 | 1131.0 | 69.1 | 56.1 | 4.9 | 2.5 | Pass |
| cross_right10_gust8_early-01 | 1269.1 | 71.8 | 162.1 | 2.6 | 2.6 | Touchdown distance |
| cross_right10_gust8_late-01 | 1085.6 | 71.7 | 126.9 | 3.3 | 2.5 | Pass |
| head10_gust10-01 | 1169.2 | 69.0 | 216.1 | 4.4 | 2.6 | Pass |
| head10_gust10_early-01 | 999.3 | 73.1 | 109.0 | 4.0 | 2.3 | Touchdown distance |
| head10_gust10_late-01 | 1116.2 | 73.2 | 385.7 | 1.8 | 2.4 | Physical sink |
| head15_gust8-01 | 1196.1 | 66.8 | 284.8 | 5.6 | 2.6 | Pass |
| head15_gust8_early-01 | 1227.1 | 70.9 | 201.0 | 3.2 | 2.7 | Touchdown distance; Alignment |
| head15_gust8_late-01 | 1140.2 | 72.7 | 236.5 | 3.1 | 2.5 | Touchdown speed |
| head5_gust8-01 | 989.7 | 71.2 | 397.6 | 2.6 | 2.7 | Touchdown distance; Physical sink |
| head5_gust8_early-01 | 1112.6 | 69.5 | 178.0 | 3.8 | 2.6 | Pass |
| head5_gust8_late-01 | 1078.0 | 71.4 | 149.1 | 4.3 | 2.2 | Pass |
| tail5_gust5-01 | 1128.9 | 65.3 | 126.0 | 6.7 | 2.1 | Pass |
| tail5_gust5_early-01 | 1304.5 | 65.2 | 150.7 | 6.1 | 2.4 | Touchdown distance |
| tail5_gust5_late-01 | 1008.1 | 66.7 | 43.0 | 6.1 | 2.5 | Pass |

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
