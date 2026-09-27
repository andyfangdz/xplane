# Native X-Plane harness report

**5/18 flights passed the landing limits.** Measurement-valid flights: 18. Installation restored: True.

Landing pass/fail is separate from harness operation. Native guidance runs on simulator frames; Python only supervises it. All attempts, including aborted and non-passing flights, remain in this report.

| Trial | Touchdown ft | KIAS | Physical sink fpm | Peak pitch ° | Peak pitch rate °/s | Result |
|---|---:|---:|---:|---:|---:|---|
| cross_left10_gust8-01 | 1228.9 | 70.3 | 169.8 | 3.8 | 2.5 | Touchdown distance |
| cross_left10_gust8_early-01 | 993.7 | 72.4 | 46.2 | 3.6 | 2.6 | Touchdown distance |
| cross_left10_gust8_late-01 | 1148.8 | 70.8 | 172.7 | 3.4 | 1.9 | Pass |
| cross_right10_gust8-01 | 1117.2 | 69.1 | 119.5 | 4.5 | 2.4 | Pass |
| cross_right10_gust8_early-01 | 1336.9 | 70.8 | 169.0 | 3.1 | 2.6 | Touchdown distance |
| cross_right10_gust8_late-01 | 1116.1 | 71.7 | 137.5 | 3.3 | 3.6 | Flare motion |
| head10_gust10-01 | 1187.1 | 68.2 | 241.6 | 4.4 | 3.1 | Flare motion |
| head10_gust10_early-01 | 960.1 | 74.0 | 209.3 | 3.0 | 2.4 | Touchdown distance |
| head10_gust10_late-01 | 1332.6 | 72.4 | 258.5 | 2.2 | 2.8 | Touchdown distance |
| head15_gust8-01 | 1209.9 | 69.7 | 405.8 | 3.4 | 2.7 | Touchdown distance; Physical sink |
| head15_gust8_early-01 | 1261.7 | 70.5 | 227.2 | 3.5 | 2.9 | Touchdown distance; Alignment; Flare motion |
| head15_gust8_late-01 | 1334.3 | 70.1 | 312.0 | 2.7 | 2.5 | Touchdown distance; Physical sink |
| head5_gust8-01 | 1039.2 | 70.4 | 402.2 | 2.9 | 2.3 | Physical sink |
| head5_gust8_early-01 | 1110.9 | 69.4 | 180.5 | 3.8 | 2.6 | Pass |
| head5_gust8_late-01 | 1141.0 | 70.3 | 191.9 | 3.8 | 2.3 | Pass |
| tail5_gust5-01 | 1154.0 | 64.9 | 139.7 | 6.9 | 2.1 | Pass |
| tail5_gust5_early-01 | 1308.9 | 64.9 | 157.8 | 6.3 | 2.3 | Touchdown distance |
| tail5_gust5_late-01 | 944.8 | 67.6 | 40.3 | 5.6 | 2.5 | Touchdown distance; Touchdown speed |

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
