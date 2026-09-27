# Native X-Plane harness report

**8/18 flights passed the landing limits.** Measurement-valid flights: 18. Installation restored: True.

Landing pass/fail is separate from harness operation. Native guidance runs on simulator frames; Python only supervises it. All attempts, including aborted and non-passing flights, remain in this report.

| Trial | Touchdown ft | KIAS | Physical sink fpm | Peak pitch ° | Peak pitch rate °/s | Result |
|---|---:|---:|---:|---:|---:|---|
| cross_right6_gust6-01 | 1101.9 | 70.2 | 66.0 | 4.1 | 2.5 | Pass |
| cross_right6_gust6_early-01 | 1281.5 | 71.4 | 190.6 | 2.6 | 2.5 | Touchdown distance; Touchdown speed |
| cross_right6_gust6_late-01 | 1107.7 | 71.4 | 146.4 | 3.1 | 2.4 | Touchdown speed |
| head12_gust6-01 | 1101.0 | 67.9 | 206.1 | 5.5 | 2.4 | Pass |
| head12_gust6_early-01 | 1141.6 | 69.8 | 159.7 | 4.1 | 2.8 | Pass |
| head12_gust6_late-01 | 1122.3 | 71.4 | 121.6 | 3.8 | 2.6 | Touchdown speed |
| head7_gust6-01 | 948.6 | 70.4 | 405.2 | 2.9 | 2.6 | Touchdown distance; Alignment; Physical sink |
| head7_gust6_early-01 | 1155.4 | 68.8 | 179.2 | 4.1 | 2.5 | Pass |
| head7_gust6_late-01 | 1167.3 | 69.1 | 192.1 | 4.2 | 2.2 | Pass |
| qleft10_gust6-01 | 1255.8 | 68.9 | 232.1 | 4.0 | 2.3 | Touchdown distance |
| qleft10_gust6_early-01 | 1164.0 | 68.5 | 146.0 | 4.6 | 2.6 | Pass |
| qleft10_gust6_late-01 | 1239.5 | 68.4 | 181.9 | 4.3 | 1.9 | Touchdown distance |
| qright8_gust8-01 | 1155.4 | 71.1 | 174.2 | 3.6 | 2.6 | Pass |
| qright8_gust8_early-01 | 1239.4 | 70.7 | 236.1 | 2.9 | 2.6 | Touchdown distance |
| qright8_gust8_late-01 | 956.4 | 72.2 | 262.4 | 3.2 | 2.4 | Touchdown distance |
| qtail6_gust4-01 | 1191.9 | 67.9 | 151.8 | 4.5 | 2.1 | Touchdown speed |
| qtail6_gust4_early-01 | 1133.2 | 67.6 | 125.6 | 4.9 | 2.3 | Pass |
| qtail6_gust4_late-01 | 1113.5 | 69.0 | 81.9 | 4.3 | 2.4 | Touchdown speed |

## cross right6 gust6

![Flight-path, speed, pitch and descent overlays](charts/cross_right6_gust6.png)

- Fewer than two traces; repeatability is not established

## cross right6 gust6 early

![Flight-path, speed, pitch and descent overlays](charts/cross_right6_gust6_early.png)

- Fewer than two traces; repeatability is not established

## cross right6 gust6 late

![Flight-path, speed, pitch and descent overlays](charts/cross_right6_gust6_late.png)

- Fewer than two traces; repeatability is not established

## head12 gust6

![Flight-path, speed, pitch and descent overlays](charts/head12_gust6.png)

- Fewer than two traces; repeatability is not established

## head12 gust6 early

![Flight-path, speed, pitch and descent overlays](charts/head12_gust6_early.png)

- Fewer than two traces; repeatability is not established

## head12 gust6 late

![Flight-path, speed, pitch and descent overlays](charts/head12_gust6_late.png)

- Fewer than two traces; repeatability is not established

## head7 gust6

![Flight-path, speed, pitch and descent overlays](charts/head7_gust6.png)

- Fewer than two traces; repeatability is not established

## head7 gust6 early

![Flight-path, speed, pitch and descent overlays](charts/head7_gust6_early.png)

- Fewer than two traces; repeatability is not established

## head7 gust6 late

![Flight-path, speed, pitch and descent overlays](charts/head7_gust6_late.png)

- Fewer than two traces; repeatability is not established

## qleft10 gust6

![Flight-path, speed, pitch and descent overlays](charts/qleft10_gust6.png)

- Fewer than two traces; repeatability is not established

## qleft10 gust6 early

![Flight-path, speed, pitch and descent overlays](charts/qleft10_gust6_early.png)

- Fewer than two traces; repeatability is not established

## qleft10 gust6 late

![Flight-path, speed, pitch and descent overlays](charts/qleft10_gust6_late.png)

- Fewer than two traces; repeatability is not established

## qright8 gust8

![Flight-path, speed, pitch and descent overlays](charts/qright8_gust8.png)

- Fewer than two traces; repeatability is not established

## qright8 gust8 early

![Flight-path, speed, pitch and descent overlays](charts/qright8_gust8_early.png)

- Fewer than two traces; repeatability is not established

## qright8 gust8 late

![Flight-path, speed, pitch and descent overlays](charts/qright8_gust8_late.png)

- Fewer than two traces; repeatability is not established

## qtail6 gust4

![Flight-path, speed, pitch and descent overlays](charts/qtail6_gust4.png)

- Fewer than two traces; repeatability is not established

## qtail6 gust4 early

![Flight-path, speed, pitch and descent overlays](charts/qtail6_gust4_early.png)

- Fewer than two traces; repeatability is not established

## qtail6 gust4 late

![Flight-path, speed, pitch and descent overlays](charts/qtail6_gust4_late.png)

- Fewer than two traces; repeatability is not established

## Evidence

- `resolved-config.json` and each card’s `effective-config.json`: complete settings, with no inactive legacy fields.
- `native-effective.ini`: exact configuration read back from the plugin before flight.
- `trace.csv`: native-frame observations; `supervision.json`: independent polling and deliberate gap probes.
- `source-manifest.json`: harness, native binary, setup adapter and original aircraft lineage.
- `events.jsonl`, `status.json`: structured progress; `restoration.json`: recovery verification.
- `summary.json`: metrics and repeat-divergence timestamps; `charts/*.svg`: standalone vector figures.
