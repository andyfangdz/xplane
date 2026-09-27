# Native X-Plane harness report

**6/18 flights passed the landing limits.** Measurement-valid flights: 18. Installation restored: True.

Landing pass/fail is separate from harness operation. Native guidance runs on simulator frames; Python only supervises it. All attempts, including aborted and non-passing flights, remain in this report.

| Trial | Touchdown ft | KIAS | Physical sink fpm | Peak pitch ° | Peak pitch rate °/s | Result |
|---|---:|---:|---:|---:|---:|---|
| cross_left10_gust8-01 | 1255.8 | 71.5 | 146.4 | 3.2 | 2.5 | Touchdown distance |
| cross_left10_gust8_early-01 | 1110.6 | 71.7 | 120.5 | 3.2 | 2.2 | Pass |
| cross_left10_gust8_late-01 | 1240.6 | 70.6 | 144.0 | 3.3 | 2.6 | Touchdown distance |
| cross_right10_gust8-01 | 1144.0 | 70.2 | 130.1 | 3.4 | 2.5 | Pass |
| cross_right10_gust8_early-01 | 1348.3 | 72.2 | 123.4 | 2.8 | 2.6 | Touchdown distance |
| cross_right10_gust8_late-01 | 1164.9 | 71.6 | 127.8 | 3.3 | 2.4 | Pass |
| head10_gust10-01 | 1226.0 | 68.4 | 225.2 | 4.3 | 2.6 | Touchdown distance |
| head10_gust10_early-01 | 1115.7 | 72.7 | 116.0 | 3.5 | 2.3 | Pass |
| head10_gust10_late-01 | 1119.5 | 73.7 | 414.7 | 1.5 | 2.4 | Physical sink |
| head15_gust8-01 | 1235.4 | 65.4 | 268.8 | 6.4 | 2.5 | Touchdown distance |
| head15_gust8_early-01 | 1281.4 | 69.6 | 182.4 | 3.8 | 2.7 | Touchdown distance; Alignment |
| head15_gust8_late-01 | 1100.1 | 72.8 | 348.0 | 2.3 | 2.6 | Physical sink; Touchdown speed |
| head5_gust8-01 | 1028.6 | 72.1 | 412.1 | 2.1 | 2.6 | Physical sink |
| head5_gust8_early-01 | 1244.0 | 70.0 | 176.2 | 3.7 | 2.5 | Touchdown distance |
| head5_gust8_late-01 | 1184.6 | 71.3 | 182.1 | 3.4 | 2.4 | Pass |
| tail5_gust5-01 | 1341.1 | 65.4 | 147.3 | 6.5 | 2.0 | Touchdown distance |
| tail5_gust5_early-01 | 1447.5 | 65.7 | 110.8 | 6.0 | 2.4 | Touchdown distance |
| tail5_gust5_late-01 | 1162.8 | 66.8 | 137.4 | 4.8 | 2.2 | Pass |

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
