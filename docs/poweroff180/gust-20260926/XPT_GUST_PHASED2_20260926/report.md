# Native X-Plane harness report

**11/18 flights passed the landing limits.** Measurement-valid flights: 18. Installation restored: True.

Landing pass/fail is separate from harness operation. Native guidance runs on simulator frames; Python only supervises it. All attempts, including aborted and non-passing flights, remain in this report.

| Trial | Touchdown ft | KIAS | Physical sink fpm | Peak pitch ° | Peak pitch rate °/s | Result |
|---|---:|---:|---:|---:|---:|---|
| cross_left10_gust8-01 | 1165.4 | 69.7 | 179.1 | 4.1 | 2.4 | Pass |
| cross_left10_gust8_early-01 | 852.5 | 73.3 | 69.9 | 3.3 | 2.7 | Touchdown distance; Touchdown speed |
| cross_left10_gust8_late-01 | 1102.2 | 70.1 | 157.6 | 3.8 | 1.7 | Pass |
| cross_right10_gust8-01 | 1087.0 | 68.3 | 78.1 | 4.9 | 2.4 | Pass |
| cross_right10_gust8_early-01 | 1191.1 | 72.2 | 126.2 | 3.3 | 2.6 | Pass |
| cross_right10_gust8_late-01 | 1089.3 | 70.9 | 105.7 | 3.4 | 2.4 | Pass |
| head10_gust10-01 | 1129.5 | 68.7 | 247.9 | 4.2 | 2.6 | Pass |
| head10_gust10_early-01 | 1136.9 | 70.6 | 47.3 | 4.1 | 2.5 | Pass |
| head10_gust10_late-01 | 1073.6 | 74.0 | 349.1 | 1.8 | 2.6 | Physical sink; Touchdown speed |
| head15_gust8-01 | 1182.8 | 65.6 | 277.1 | 6.2 | 2.6 | Pass |
| head15_gust8_early-01 | 1114.4 | 71.8 | 208.5 | 3.2 | 2.5 | Alignment |
| head15_gust8_late-01 | 1130.5 | 73.6 | 334.0 | 2.1 | 2.7 | Physical sink; Touchdown speed |
| head5_gust8-01 | 947.1 | 71.1 | 380.1 | 2.8 | 2.3 | Touchdown distance; Physical sink |
| head5_gust8_early-01 | 1096.8 | 67.8 | 190.2 | 4.5 | 2.4 | Pass |
| head5_gust8_late-01 | 995.8 | 71.2 | 174.1 | 4.4 | 2.3 | Touchdown distance |
| tail5_gust5-01 | 1182.4 | 64.4 | 144.5 | 7.1 | 2.3 | Pass |
| tail5_gust5_early-01 | 1368.9 | 64.8 | 101.6 | 6.3 | 2.5 | Touchdown distance |
| tail5_gust5_late-01 | 1026.3 | 66.4 | 44.6 | 6.2 | 2.5 | Pass |

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
