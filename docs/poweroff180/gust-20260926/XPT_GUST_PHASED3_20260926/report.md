# Native X-Plane harness report

**7/18 flights passed the landing limits.** Measurement-valid flights: 18. Installation restored: True.

Landing pass/fail is separate from harness operation. Native guidance runs on simulator frames; Python only supervises it. All attempts, including aborted and non-passing flights, remain in this report.

| Trial | Touchdown ft | KIAS | Physical sink fpm | Peak pitch ° | Peak pitch rate °/s | Result |
|---|---:|---:|---:|---:|---:|---|
| cross_left10_gust8-01 | 1162.7 | 69.8 | 179.1 | 4.0 | 2.4 | Pass |
| cross_left10_gust8_early-01 | 862.4 | 73.3 | 65.7 | 3.4 | 3.0 | Touchdown distance; Touchdown speed; Flare motion |
| cross_left10_gust8_late-01 | 1105.5 | 70.2 | 164.2 | 3.7 | 2.1 | Pass |
| cross_right10_gust8-01 | 979.5 | 70.0 | 36.9 | 4.3 | 2.4 | Touchdown distance |
| cross_right10_gust8_early-01 | 1274.8 | 70.8 | 125.3 | 3.3 | 2.5 | Touchdown distance |
| cross_right10_gust8_late-01 | 1090.4 | 71.0 | 113.7 | 3.4 | 2.4 | Pass |
| head10_gust10-01 | 1122.0 | 68.9 | 236.2 | 4.2 | 2.6 | Pass |
| head10_gust10_early-01 | 875.4 | 74.3 | 303.6 | 2.2 | 2.4 | Touchdown distance; Physical sink; Touchdown speed |
| head10_gust10_late-01 | 1078.4 | 74.2 | 322.9 | 1.9 | 2.6 | Physical sink; Touchdown speed |
| head15_gust8-01 | 1164.2 | 66.3 | 239.7 | 6.3 | 2.6 | Pass |
| head15_gust8_early-01 | 1195.8 | 70.3 | 184.0 | 3.8 | 2.6 | Alignment |
| head15_gust8_late-01 | 1128.8 | 73.6 | 338.5 | 2.1 | 2.6 | Physical sink; Touchdown speed |
| head5_gust8-01 | 946.5 | 71.2 | 372.8 | 2.8 | 2.4 | Touchdown distance; Physical sink |
| head5_gust8_early-01 | 1072.3 | 68.5 | 140.1 | 4.5 | 2.5 | Pass |
| head5_gust8_late-01 | 993.2 | 71.2 | 176.2 | 4.4 | 2.3 | Touchdown distance |
| tail5_gust5-01 | 1156.5 | 64.5 | 134.0 | 7.1 | 2.2 | Pass |
| tail5_gust5_early-01 | 1315.1 | 65.1 | 120.7 | 6.4 | 2.5 | Touchdown distance |
| tail5_gust5_late-01 | 880.8 | 68.5 | 59.2 | 5.0 | 2.5 | Touchdown distance; Touchdown speed |

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
