# Native X-Plane harness report

**10/18 flights passed the landing limits.** Measurement-valid flights: 18. Installation restored: True.

Landing pass/fail is separate from harness operation. Native guidance runs on simulator frames; Python only supervises it. All attempts, including aborted and non-passing flights, remain in this report.

| Trial | Touchdown ft | KIAS | Physical sink fpm | Peak pitch ° | Peak pitch rate °/s | Result |
|---|---:|---:|---:|---:|---:|---|
| cross_left10_gust8-01 | 1183.7 | 71.0 | 168.7 | 3.3 | 2.5 | Pass |
| cross_left10_gust8_early-01 | 1001.2 | 72.1 | 25.8 | 3.6 | 2.7 | Pass |
| cross_left10_gust8_late-01 | 1137.5 | 70.5 | 175.8 | 3.3 | 2.4 | Pass |
| cross_right10_gust8-01 | 1123.6 | 69.1 | 61.6 | 4.8 | 2.4 | Pass |
| cross_right10_gust8_early-01 | 1271.6 | 71.8 | 160.4 | 2.6 | 2.6 | Touchdown distance |
| cross_right10_gust8_late-01 | 1088.8 | 71.8 | 126.7 | 3.3 | 2.5 | Pass |
| head10_gust10-01 | 1173.0 | 68.9 | 221.9 | 4.3 | 2.7 | Pass |
| head10_gust10_early-01 | 986.7 | 73.8 | 127.3 | 3.6 | 2.4 | Touchdown distance |
| head10_gust10_late-01 | 1119.7 | 73.6 | 374.1 | 1.7 | 2.4 | Physical sink |
| head15_gust8-01 | 1201.0 | 66.7 | 291.9 | 5.6 | 2.6 | Touchdown distance |
| head15_gust8_early-01 | 1228.7 | 70.9 | 207.7 | 3.4 | 2.7 | Touchdown distance; Alignment |
| head15_gust8_late-01 | 1177.1 | 72.5 | 183.3 | 3.1 | 2.6 | Pass |
| head5_gust8-01 | 996.6 | 71.3 | 381.2 | 2.7 | 2.3 | Touchdown distance; Physical sink |
| head5_gust8_early-01 | 1113.6 | 69.4 | 181.9 | 3.9 | 2.6 | Pass |
| head5_gust8_late-01 | 1034.6 | 72.0 | 219.5 | 3.7 | 2.3 | Pass |
| tail5_gust5-01 | 1132.5 | 65.3 | 127.5 | 6.7 | 2.1 | Pass |
| tail5_gust5_early-01 | 1280.1 | 65.7 | 137.7 | 5.8 | 2.4 | Touchdown distance |
| tail5_gust5_late-01 | 929.7 | 67.9 | 40.6 | 5.5 | 2.4 | Touchdown distance; Touchdown speed |

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
