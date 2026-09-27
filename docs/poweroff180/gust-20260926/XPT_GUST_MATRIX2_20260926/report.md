# Native X-Plane harness report

**4/12 flights passed the landing limits.** Measurement-valid flights: 12. Installation restored: True.

Landing pass/fail is separate from harness operation. Native guidance runs on simulator frames; Python only supervises it. All attempts, including aborted and non-passing flights, remain in this report.

| Trial | Touchdown ft | KIAS | Physical sink fpm | Peak pitch ° | Peak pitch rate °/s | Result |
|---|---:|---:|---:|---:|---:|---|
| cross_left10_gust8-01 | 1168.4 | 70.8 | 184.7 | 3.5 | 2.3 | Pass |
| cross_left10_gust8-02 | 1135.3 | 70.5 | 111.6 | 4.1 | 1.8 | Pass |
| cross_right10_gust8-01 | 1026.1 | 69.1 | 47.6 | 5.2 | 2.4 | Alignment |
| cross_right10_gust8-02 | 1021.3 | 69.1 | 48.4 | 5.2 | 2.4 | Alignment |
| head10_gust10-01 | 1065.6 | 69.2 | 172.1 | 5.2 | 2.0 | Pass |
| head10_gust10-02 | 1156.2 | 67.5 | 240.2 | 4.8 | 2.8 | Pass |
| head15_gust8-01 | 919.5 | 63.9 | 268.7 | 6.8 | 2.6 | Touchdown distance |
| head15_gust8-02 | 869.3 | 64.6 | 283.3 | 6.6 | 2.5 | Touchdown distance |
| head5_gust8-01 | 985.3 | 71.5 | 288.7 | 3.4 | 2.8 | Touchdown distance |
| head5_gust8-02 | 983.0 | 71.3 | 281.3 | 3.5 | 2.7 | Touchdown distance |
| tail5_gust5-01 | 1409.3 | 66.8 | 95.7 | 5.9 | 2.1 | Touchdown distance |
| tail5_gust5-02 | 1407.6 | 66.6 | 102.1 | 6.1 | 2.0 | Touchdown distance |

## cross left10 gust8

![Flight-path, speed, pitch and descent overlays](charts/cross_left10_gust8.png)

No repeat-divergence diagnostic threshold was exceeded.

## cross right10 gust8

![Flight-path, speed, pitch and descent overlays](charts/cross_right10_gust8.png)

No repeat-divergence diagnostic threshold was exceeded.

## head10 gust10

![Flight-path, speed, pitch and descent overlays](charts/head10_gust10.png)

- ias_kias spread 6.6 at 57.4s exceeds diagnostic threshold 3
- pitch_deg spread 4.2 at 74.6s exceeds diagnostic threshold 2
- physical_sink_fpm spread 237.7 at 74.4s exceeds diagnostic threshold 150

## head15 gust8

![Flight-path, speed, pitch and descent overlays](charts/head15_gust8.png)

- pitch_deg spread 6.6 at 77.3s exceeds diagnostic threshold 2
- physical_sink_fpm spread 321.0 at 77.1s exceeds diagnostic threshold 150

## head5 gust8

![Flight-path, speed, pitch and descent overlays](charts/head5_gust8.png)

No repeat-divergence diagnostic threshold was exceeded.

## tail5 gust5

![Flight-path, speed, pitch and descent overlays](charts/tail5_gust5.png)

No repeat-divergence diagnostic threshold was exceeded.

## Evidence

- `resolved-config.json` and each card’s `effective-config.json`: complete settings, with no inactive legacy fields.
- `native-effective.ini`: exact configuration read back from the plugin before flight.
- `trace.csv`: native-frame observations; `supervision.json`: independent polling and deliberate gap probes.
- `source-manifest.json`: harness, native binary, setup adapter and original aircraft lineage.
- `events.jsonl`, `status.json`: structured progress; `restoration.json`: recovery verification.
- `summary.json`: metrics and repeat-divergence timestamps; `charts/*.svg`: standalone vector figures.
