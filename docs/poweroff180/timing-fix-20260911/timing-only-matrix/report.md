# Native X-Plane harness report

**13/14 flights passed the landing limits.** Measurement-valid flights: 14. Installation restored: True.

Landing pass/fail is separate from harness operation. Native guidance runs on simulator frames; Python only supervises it. All attempts, including aborted and non-passing flights, remain in this report.

| Trial | Touchdown ft | KIAS | Physical sink fpm | Peak pitch ° | Peak pitch rate °/s | Result |
|---|---:|---:|---:|---:|---:|---|
| calm-01 | 1029.8 | 66.1 | 54.9 | 6.9 | 2.5 | Pass |
| calm-02 | 1073.4 | 66.1 | 44.9 | 6.7 | 2.5 | Pass |
| cross_left10-01 | 1080.3 | 64.8 | 55.6 | 7.3 | 2.2 | Pass |
| cross_left10-02 | 1238.6 | 64.4 | 47.8 | 7.3 | 2.2 | Touchdown distance |
| cross_right10-01 | 1130.1 | 64.8 | 20.7 | 7.2 | 2.3 | Pass |
| cross_right10-02 | 1127.1 | 64.8 | 20.2 | 7.3 | 2.3 | Pass |
| head10-01 | 1113.0 | 64.4 | 120.2 | 7.2 | 2.5 | Pass |
| head10-02 | 1120.6 | 64.5 | 130.0 | 7.2 | 2.4 | Pass |
| head15-01 | 1106.2 | 65.0 | 190.1 | 7.1 | 2.2 | Pass |
| head15-02 | 1107.5 | 65.1 | 180.5 | 7.2 | 2.6 | Pass |
| head5-01 | 1111.8 | 65.0 | 58.8 | 7.3 | 2.4 | Pass |
| head5-02 | 1111.7 | 64.9 | 59.2 | 7.3 | 2.4 | Pass |
| tail5-01 | 1090.9 | 65.9 | 33.3 | 6.6 | 2.0 | Pass |
| tail5-02 | 1082.8 | 65.9 | 34.3 | 6.6 | 2.0 | Pass |

## calm

![Flight-path, speed, pitch and descent overlays](charts/calm.png)

No repeat-divergence diagnostic threshold was exceeded.

## cross left10

![Flight-path, speed, pitch and descent overlays](charts/cross_left10.png)

- ias_kias spread 3.4 at 83.4s exceeds diagnostic threshold 3
- pitch_deg spread 4.6 at 82.0s exceeds diagnostic threshold 2

## cross right10

![Flight-path, speed, pitch and descent overlays](charts/cross_right10.png)

No repeat-divergence diagnostic threshold was exceeded.

## head10

![Flight-path, speed, pitch and descent overlays](charts/head10.png)

No repeat-divergence diagnostic threshold was exceeded.

## head15

![Flight-path, speed, pitch and descent overlays](charts/head15.png)

No repeat-divergence diagnostic threshold was exceeded.

## head5

![Flight-path, speed, pitch and descent overlays](charts/head5.png)

No repeat-divergence diagnostic threshold was exceeded.

## tail5

![Flight-path, speed, pitch and descent overlays](charts/tail5.png)

No repeat-divergence diagnostic threshold was exceeded.

## Evidence

- `resolved-config.json` and each card’s `effective-config.json`: complete settings, with no inactive legacy fields.
- `native-effective.ini`: exact configuration read back from the plugin before flight.
- `trace.csv`: native-frame observations; `supervision.json`: independent polling and deliberate gap probes.
- `source-manifest.json`: harness, native binary, setup adapter and original aircraft lineage.
- `events.jsonl`, `status.json`: structured progress; `restoration.json`: recovery verification.
- `summary.json`: metrics and repeat-divergence timestamps; `charts/*.svg`: standalone vector figures.
