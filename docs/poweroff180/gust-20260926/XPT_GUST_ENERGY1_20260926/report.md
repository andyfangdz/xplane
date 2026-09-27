# Native X-Plane harness report

**0/12 flights passed the landing limits.** Measurement-valid flights: 12. Installation restored: True.

Landing pass/fail is separate from harness operation. Native guidance runs on simulator frames; Python only supervises it. All attempts, including aborted and non-passing flights, remain in this report.

| Trial | Touchdown ft | KIAS | Physical sink fpm | Peak pitch ° | Peak pitch rate °/s | Result |
|---|---:|---:|---:|---:|---:|---|
| cross_left10_gust8-01 | 1769.6 | 69.7 | 113.9 | 4.2 | 2.6 | Touchdown distance; Alignment |
| cross_left10_gust8-02 | 1875.3 | 68.8 | 137.0 | 4.2 | 2.4 | Touchdown distance; Alignment |
| cross_right10_gust8-01 | 1373.0 | 69.9 | 133.8 | 3.6 | 2.4 | Touchdown distance; Alignment |
| cross_right10_gust8-02 | 1372.2 | 69.8 | 134.1 | 3.6 | 2.4 | Touchdown distance; Alignment |
| head10_gust10-01 | 1383.3 | 68.5 | 240.0 | 4.0 | 2.6 | Touchdown distance |
| head10_gust10-02 | 1308.1 | 72.1 | 448.8 | 1.8 | 2.7 | Touchdown distance; Physical sink |
| head15_gust8-01 | 1240.2 | 65.7 | 308.5 | 6.0 | 2.5 | Touchdown distance; Physical sink |
| head15_gust8-02 | 1238.5 | 65.4 | 196.9 | 6.6 | 2.5 | Touchdown distance |
| head5_gust8-01 | 1416.5 | 73.2 | 126.3 | 3.4 | 2.6 | Touchdown distance; Alignment; Touchdown speed |
| head5_gust8-02 | 1413.1 | 73.2 | 114.7 | 3.4 | 2.6 | Touchdown distance; Alignment; Touchdown speed |
| tail5_gust5-01 | 1642.6 | 65.8 | 51.2 | 7.0 | 2.3 | Touchdown distance; Alignment |
| tail5_gust5-02 | 1636.7 | 65.8 | 51.5 | 7.0 | 2.3 | Touchdown distance; Alignment |

## cross left10 gust8

![Flight-path, speed, pitch and descent overlays](charts/cross_left10_gust8.png)

- pitch_deg spread 3.0 at 80.4s exceeds diagnostic threshold 2
- physical_sink_fpm spread 189.2 at 80.4s exceeds diagnostic threshold 150

## cross right10 gust8

![Flight-path, speed, pitch and descent overlays](charts/cross_right10_gust8.png)

No repeat-divergence diagnostic threshold was exceeded.

## head10 gust10

![Flight-path, speed, pitch and descent overlays](charts/head10_gust10.png)

- ias_kias spread 5.4 at 57.4s exceeds diagnostic threshold 3
- pitch_deg spread 2.6 at 76.2s exceeds diagnostic threshold 2
- physical_sink_fpm spread 321.3 at 72.8s exceeds diagnostic threshold 150

## head15 gust8

![Flight-path, speed, pitch and descent overlays](charts/head15_gust8.png)

- pitch_deg spread 2.8 at 77.9s exceeds diagnostic threshold 2

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
