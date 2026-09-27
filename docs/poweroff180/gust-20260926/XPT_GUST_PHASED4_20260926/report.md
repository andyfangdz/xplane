# Native X-Plane harness report

**8/18 flights passed the landing limits.** Measurement-valid flights: 18. Installation restored: True.

Landing pass/fail is separate from harness operation. Native guidance runs on simulator frames; Python only supervises it. All attempts, including aborted and non-passing flights, remain in this report.

| Trial | Touchdown ft | KIAS | Physical sink fpm | Peak pitch ° | Peak pitch rate °/s | Result |
|---|---:|---:|---:|---:|---:|---|
| cross_left10_gust8-01 | 1157.8 | 69.7 | 182.9 | 4.0 | 2.5 | Pass |
| cross_left10_gust8_early-01 | 816.5 | 73.6 | 37.0 | 3.0 | 3.1 | Touchdown distance; Touchdown speed; Flare motion |
| cross_left10_gust8_late-01 | 1111.4 | 70.5 | 168.7 | 3.6 | 2.0 | Pass |
| cross_right10_gust8-01 | 1086.3 | 68.2 | 102.2 | 4.8 | 2.4 | Pass |
| cross_right10_gust8_early-01 | 1231.1 | 71.6 | 126.8 | 3.4 | 2.5 | Touchdown distance |
| cross_right10_gust8_late-01 | 1109.7 | 70.6 | 131.7 | 3.3 | 2.4 | Pass |
| head10_gust10-01 | 1136.4 | 68.5 | 244.0 | 4.3 | 2.6 | Pass |
| head10_gust10_early-01 | 994.3 | 73.1 | 50.8 | 4.0 | 2.5 | Touchdown distance |
| head10_gust10_late-01 | 1164.8 | 74.5 | 181.4 | 2.3 | 2.6 | Touchdown speed |
| head15_gust8-01 | 1165.5 | 66.3 | 273.8 | 6.2 | 2.6 | Pass |
| head15_gust8_early-01 | 1224.8 | 69.3 | 182.9 | 4.4 | 2.9 | Touchdown distance; Alignment; Flare motion |
| head15_gust8_late-01 | 1277.1 | 71.1 | 222.9 | 3.4 | 2.6 | Touchdown distance |
| head5_gust8-01 | 978.5 | 70.6 | 333.0 | 3.3 | 5.2 | Touchdown distance; Physical sink; Flare motion |
| head5_gust8_early-01 | 1098.1 | 67.7 | 190.7 | 4.6 | 2.5 | Pass |
| head5_gust8_late-01 | 974.6 | 71.8 | 281.7 | 3.2 | 5.4 | Touchdown distance; Flare motion |
| tail5_gust5-01 | 1176.8 | 64.7 | 133.3 | 7.1 | 2.2 | Pass |
| tail5_gust5_early-01 | 1304.1 | 65.9 | 106.0 | 6.1 | 2.6 | Touchdown distance |
| tail5_gust5_late-01 | 851.3 | 68.9 | 49.5 | 5.0 | 2.4 | Touchdown distance; Touchdown speed |

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
