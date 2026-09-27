# Native X-Plane harness report

**8/12 flights passed the landing limits.** Measurement-valid flights: 12. Installation restored: True.

Landing pass/fail is separate from harness operation. Native guidance runs on simulator frames; Python only supervises it. All attempts, including aborted and non-passing flights, remain in this report.

| Trial | Touchdown ft | KIAS | Physical sink fpm | Peak pitch ° | Peak pitch rate °/s | Result |
|---|---:|---:|---:|---:|---:|---|
| cross_left10_gust8-01 | 1160.1 | 70.7 | 184.2 | 3.6 | 2.2 | Pass |
| cross_left10_gust8-02 | 1126.3 | 69.8 | 72.5 | 4.6 | 1.9 | Pass |
| cross_right10_gust8-01 | 1025.1 | 69.0 | 43.0 | 5.3 | 2.4 | Pass |
| cross_right10_gust8-02 | 1016.4 | 69.1 | 47.2 | 5.2 | 2.4 | Pass |
| head10_gust10-01 | 1059.3 | 69.3 | 128.1 | 5.4 | 2.2 | Pass |
| head10_gust10-02 | 1142.5 | 68.1 | 221.6 | 5.6 | 2.9 | Flare motion |
| head15_gust8-01 | 1162.2 | 66.7 | 278.8 | 6.0 | 2.7 | Pass |
| head15_gust8-02 | 1124.3 | 67.1 | 283.4 | 5.3 | 3.0 | Flare motion |
| head5_gust8-01 | 985.9 | 71.4 | 287.4 | 3.4 | 2.8 | Touchdown distance |
| head5_gust8-02 | 979.2 | 71.4 | 293.1 | 3.4 | 2.8 | Touchdown distance |
| tail5_gust5-01 | 1192.4 | 65.2 | 101.3 | 6.9 | 1.9 | Pass |
| tail5_gust5-02 | 1140.7 | 64.8 | 111.1 | 7.1 | 2.2 | Pass |

## cross left10 gust8

![Flight-path, speed, pitch and descent overlays](charts/cross_left10_gust8.png)

No repeat-divergence diagnostic threshold was exceeded.

## cross right10 gust8

![Flight-path, speed, pitch and descent overlays](charts/cross_right10_gust8.png)

No repeat-divergence diagnostic threshold was exceeded.

## head10 gust10

![Flight-path, speed, pitch and descent overlays](charts/head10_gust10.png)

- ias_kias spread 6.5 at 57.5s exceeds diagnostic threshold 3
- pitch_deg spread 4.8 at 74.6s exceeds diagnostic threshold 2
- physical_sink_fpm spread 228.8 at 74.3s exceeds diagnostic threshold 150

## head15 gust8

![Flight-path, speed, pitch and descent overlays](charts/head15_gust8.png)

- pitch_deg spread 5.1 at 77.2s exceeds diagnostic threshold 2
- physical_sink_fpm spread 345.1 at 77.1s exceeds diagnostic threshold 150

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
