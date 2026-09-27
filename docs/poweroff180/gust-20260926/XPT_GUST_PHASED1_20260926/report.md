# Native X-Plane harness report

**7/12 flights passed the landing limits.** Measurement-valid flights: 12. Installation restored: True.

Landing pass/fail is separate from harness operation. Native guidance runs on simulator frames; Python only supervises it. All attempts, including aborted and non-passing flights, remain in this report.

| Trial | Touchdown ft | KIAS | Physical sink fpm | Peak pitch ° | Peak pitch rate °/s | Result |
|---|---:|---:|---:|---:|---:|---|
| cross_left10_gust8-01 | 1150.6 | 69.7 | 178.6 | 3.9 | 2.6 | Pass |
| cross_left10_gust8_late-01 | 1106.6 | 70.8 | 148.5 | 3.4 | 2.2 | Pass |
| cross_right10_gust8-01 | 1086.5 | 68.1 | 83.1 | 5.0 | 2.6 | Pass |
| cross_right10_gust8_late-01 | 1121.7 | 70.4 | 131.9 | 3.4 | 2.4 | Pass |
| head10_gust10-01 | 1183.5 | 66.5 | 220.8 | 6.2 | 2.8 | Flare motion |
| head10_gust10_late-01 | 1143.4 | 74.5 | 104.4 | 3.4 | 2.8 | Touchdown speed |
| head15_gust8-01 | 1182.2 | 65.8 | 313.0 | 5.9 | 2.4 | Physical sink |
| head15_gust8_late-01 | 1283.9 | 70.5 | 210.3 | 3.3 | 2.7 | Touchdown distance |
| head5_gust8-01 | 1001.0 | 70.4 | 203.2 | 4.5 | 2.5 | Pass |
| head5_gust8_late-01 | 1139.5 | 68.9 | 149.9 | 4.7 | 2.5 | Pass |
| tail5_gust5-01 | 1192.9 | 64.8 | 139.4 | 7.0 | 2.1 | Pass |
| tail5_gust5_late-01 | 949.9 | 67.7 | 36.6 | 5.6 | 2.5 | Touchdown distance; Touchdown speed |

## cross left10 gust8

![Flight-path, speed, pitch and descent overlays](charts/cross_left10_gust8.png)

- Fewer than two traces; repeatability is not established

## cross left10 gust8 late

![Flight-path, speed, pitch and descent overlays](charts/cross_left10_gust8_late.png)

- Fewer than two traces; repeatability is not established

## cross right10 gust8

![Flight-path, speed, pitch and descent overlays](charts/cross_right10_gust8.png)

- Fewer than two traces; repeatability is not established

## cross right10 gust8 late

![Flight-path, speed, pitch and descent overlays](charts/cross_right10_gust8_late.png)

- Fewer than two traces; repeatability is not established

## head10 gust10

![Flight-path, speed, pitch and descent overlays](charts/head10_gust10.png)

- Fewer than two traces; repeatability is not established

## head10 gust10 late

![Flight-path, speed, pitch and descent overlays](charts/head10_gust10_late.png)

- Fewer than two traces; repeatability is not established

## head15 gust8

![Flight-path, speed, pitch and descent overlays](charts/head15_gust8.png)

- Fewer than two traces; repeatability is not established

## head15 gust8 late

![Flight-path, speed, pitch and descent overlays](charts/head15_gust8_late.png)

- Fewer than two traces; repeatability is not established

## head5 gust8

![Flight-path, speed, pitch and descent overlays](charts/head5_gust8.png)

- Fewer than two traces; repeatability is not established

## head5 gust8 late

![Flight-path, speed, pitch and descent overlays](charts/head5_gust8_late.png)

- Fewer than two traces; repeatability is not established

## tail5 gust5

![Flight-path, speed, pitch and descent overlays](charts/tail5_gust5.png)

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
