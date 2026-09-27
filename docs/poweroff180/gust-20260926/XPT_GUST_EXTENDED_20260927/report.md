# Native X-Plane harness report

**10/18 flights passed the landing limits.** Measurement-valid flights: 18. Installation restored: True.

Landing pass/fail is separate from harness operation. Native guidance runs on simulator frames; Python only supervises it. All attempts, including aborted and non-passing flights, remain in this report.

| Trial | Touchdown ft | KIAS | Physical sink fpm | Peak pitch ° | Peak pitch rate °/s | Result |
|---|---:|---:|---:|---:|---:|---|
| cross_right6_gust6-01 | 1095.0 | 70.0 | 120.5 | 3.7 | 2.5 | Pass |
| cross_right6_gust6_early-01 | 1284.7 | 71.3 | 174.5 | 2.9 | 2.5 | Touchdown distance; Touchdown speed |
| cross_right6_gust6_late-01 | 1123.4 | 71.1 | 145.5 | 3.2 | 2.4 | Pass |
| head12_gust6-01 | 1110.1 | 67.8 | 218.3 | 5.6 | 2.4 | Pass |
| head12_gust6_early-01 | 1164.3 | 69.2 | 177.2 | 3.9 | 2.7 | Pass |
| head12_gust6_late-01 | 1171.7 | 70.7 | 215.4 | 3.3 | 2.6 | Pass |
| head7_gust6-01 | 961.3 | 70.2 | 400.5 | 3.1 | 2.6 | Touchdown distance; Alignment; Physical sink |
| head7_gust6_early-01 | 1172.6 | 68.2 | 187.3 | 4.5 | 2.5 | Pass |
| head7_gust6_late-01 | 1168.1 | 69.2 | 184.2 | 4.4 | 2.2 | Pass |
| qleft10_gust6-01 | 1252.4 | 68.9 | 218.9 | 4.2 | 2.3 | Touchdown distance |
| qleft10_gust6_early-01 | 1184.6 | 68.4 | 153.8 | 4.7 | 2.5 | Pass |
| qleft10_gust6_late-01 | 1260.1 | 68.0 | 167.6 | 4.7 | 2.5 | Touchdown distance |
| qright8_gust8-01 | 1149.5 | 71.3 | 164.7 | 4.0 | 2.6 | Pass |
| qright8_gust8_early-01 | 1259.6 | 70.5 | 220.3 | 3.1 | 2.6 | Touchdown distance |
| qright8_gust8_late-01 | 970.6 | 72.1 | 277.3 | 3.1 | 2.4 | Touchdown distance |
| qtail6_gust4-01 | 1205.8 | 67.6 | 149.5 | 4.6 | 1.6 | Touchdown distance |
| qtail6_gust4_early-01 | 1146.3 | 67.4 | 124.1 | 5.0 | 2.4 | Pass |
| qtail6_gust4_late-01 | 1116.9 | 69.0 | 131.9 | 4.0 | 4.1 | Touchdown speed; Flare motion |

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
