# Native X-Plane harness report

**9/18 flights passed the landing limits.** Measurement-valid flights: 18. Installation restored: True.

Landing pass/fail is separate from harness operation. Native guidance runs on simulator frames; Python only supervises it. All attempts, including aborted and non-passing flights, remain in this report.

| Trial | Touchdown ft | KIAS | Physical sink fpm | Peak pitch ° | Peak pitch rate °/s | Result |
|---|---:|---:|---:|---:|---:|---|
| cross_left10_gust8-01 | 1192.1 | 70.8 | 170.4 | 3.3 | 2.6 | Pass |
| cross_left10_gust8_early-01 | 1000.8 | 72.2 | 35.4 | 3.6 | 2.8 | Pass |
| cross_left10_gust8_late-01 | 1145.9 | 70.4 | 175.4 | 3.3 | 2.3 | Pass |
| cross_right10_gust8-01 | 1126.6 | 69.0 | 92.8 | 4.7 | 2.5 | Pass |
| cross_right10_gust8_early-01 | 1259.5 | 72.0 | 161.9 | 2.6 | 2.6 | Touchdown distance |
| cross_right10_gust8_late-01 | 1117.7 | 71.5 | 132.2 | 3.3 | 2.5 | Pass |
| head10_gust10-01 | 1201.5 | 68.2 | 251.3 | 4.4 | 2.7 | Touchdown distance |
| head10_gust10_early-01 | 988.2 | 73.6 | 147.4 | 3.5 | 2.4 | Touchdown distance |
| head10_gust10_late-01 | 1140.0 | 74.0 | 316.0 | 2.1 | 2.5 | Physical sink |
| head15_gust8-01 | 1211.3 | 66.5 | 289.8 | 5.9 | 2.5 | Touchdown distance |
| head15_gust8_early-01 | 1243.8 | 70.2 | 186.7 | 3.8 | 2.7 | Touchdown distance; Alignment |
| head15_gust8_late-01 | 1185.5 | 72.3 | 202.0 | 3.0 | 2.6 | Pass |
| head5_gust8-01 | 1018.7 | 70.8 | 352.3 | 3.1 | 2.4 | Physical sink |
| head5_gust8_early-01 | 1132.5 | 68.9 | 182.5 | 4.1 | 2.6 | Pass |
| head5_gust8_late-01 | 1059.4 | 71.9 | 221.2 | 3.7 | 2.3 | Pass |
| tail5_gust5-01 | 1154.3 | 65.2 | 134.2 | 6.7 | 2.1 | Pass |
| tail5_gust5_early-01 | 1322.0 | 65.1 | 136.9 | 6.1 | 2.4 | Touchdown distance |
| tail5_gust5_late-01 | 986.3 | 67.1 | 41.1 | 5.8 | 2.5 | Touchdown distance; Touchdown speed |

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
