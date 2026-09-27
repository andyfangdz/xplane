# Native X-Plane harness report

**10/18 flights passed the landing limits.** Measurement-valid flights: 18. Installation restored: True.

Landing pass/fail is separate from harness operation. Native guidance runs on simulator frames; Python only supervises it. All attempts, including aborted and non-passing flights, remain in this report.

| Trial | Touchdown ft | KIAS | Physical sink fpm | Peak pitch ° | Peak pitch rate °/s | Result |
|---|---:|---:|---:|---:|---:|---|
| cross_left10_gust8-01 | 1191.3 | 71.5 | 173.5 | 2.9 | 2.3 | Pass |
| cross_left10_gust8_early-01 | 999.7 | 72.3 | 86.2 | 3.6 | 2.6 | Touchdown distance |
| cross_left10_gust8_late-01 | 1135.9 | 70.8 | 172.1 | 3.2 | 2.2 | Pass |
| cross_right10_gust8-01 | 1093.4 | 69.6 | 47.2 | 4.7 | 2.5 | Pass |
| cross_right10_gust8_early-01 | 1257.1 | 72.0 | 179.2 | 2.4 | 2.6 | Touchdown distance |
| cross_right10_gust8_late-01 | 1067.5 | 71.9 | 127.3 | 3.2 | 2.5 | Pass |
| head10_gust10-01 | 1136.2 | 70.0 | 215.5 | 4.1 | 2.7 | Pass |
| head10_gust10_early-01 | 995.5 | 73.5 | 97.8 | 3.8 | 2.3 | Touchdown distance |
| head10_gust10_late-01 | 1104.1 | 73.4 | 380.7 | 1.8 | 2.4 | Physical sink |
| head15_gust8-01 | 1178.5 | 67.8 | 282.7 | 5.1 | 2.6 | Pass |
| head15_gust8_early-01 | 1214.7 | 71.2 | 207.3 | 2.9 | 2.7 | Touchdown distance; Alignment |
| head15_gust8_late-01 | 1150.5 | 72.8 | 197.4 | 3.1 | 2.5 | Touchdown speed |
| head5_gust8-01 | 974.9 | 71.4 | 405.7 | 2.4 | 6.4 | Touchdown distance; Physical sink; Flare motion |
| head5_gust8_early-01 | 1098.2 | 69.7 | 172.9 | 3.6 | 2.6 | Pass |
| head5_gust8_late-01 | 1108.7 | 70.9 | 190.0 | 3.6 | 2.2 | Pass |
| tail5_gust5-01 | 1139.5 | 65.9 | 126.2 | 6.2 | 2.3 | Pass |
| tail5_gust5_early-01 | 1283.8 | 65.4 | 158.1 | 6.0 | 2.3 | Touchdown distance |
| tail5_gust5_late-01 | 1026.0 | 66.5 | 44.6 | 6.2 | 2.5 | Pass |

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
