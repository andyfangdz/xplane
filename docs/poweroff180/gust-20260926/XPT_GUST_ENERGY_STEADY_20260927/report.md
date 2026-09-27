# Native X-Plane harness report

**6/6 flights passed the landing limits.** Measurement-valid flights: 6. Installation restored: True.

Landing pass/fail is separate from harness operation. Native guidance runs on simulator frames; Python only supervises it. All attempts, including aborted and non-passing flights, remain in this report.

| Trial | Touchdown ft | KIAS | Physical sink fpm | Peak pitch ° | Peak pitch rate °/s | Result |
|---|---:|---:|---:|---:|---:|---|
| calm-01 | 1028.9 | 66.1 | 55.3 | 6.9 | 2.5 | Pass |
| calm-02 | 1075.9 | 66.2 | 45.1 | 6.7 | 2.5 | Pass |
| cross_left10-01 | 1080.6 | 66.3 | 118.1 | 6.2 | 2.2 | Pass |
| cross_left10-02 | 1048.2 | 65.4 | 48.6 | 7.4 | 2.5 | Pass |
| head15-01 | 1057.4 | 64.9 | 150.9 | 7.3 | 2.7 | Pass |
| head15-02 | 1113.7 | 65.3 | 170.0 | 7.1 | 2.6 | Pass |

## calm

![Flight-path, speed, pitch and descent overlays](charts/calm.png)

No repeat-divergence diagnostic threshold was exceeded.

## cross left10

![Flight-path, speed, pitch and descent overlays](charts/cross_left10.png)

- pitch_deg spread 2.3 at 78.5s exceeds diagnostic threshold 2

## head15

![Flight-path, speed, pitch and descent overlays](charts/head15.png)

- pitch_deg spread 5.5 at 80.3s exceeds diagnostic threshold 2
- physical_sink_fpm spread 187.0 at 80.0s exceeds diagnostic threshold 150

## Evidence

- `resolved-config.json` and each card’s `effective-config.json`: complete settings, with no inactive legacy fields.
- `native-effective.ini`: exact configuration read back from the plugin before flight.
- `trace.csv`: native-frame observations; `supervision.json`: independent polling and deliberate gap probes.
- `source-manifest.json`: harness, native binary, setup adapter and original aircraft lineage.
- `events.jsonl`, `status.json`: structured progress; `restoration.json`: recovery verification.
- `summary.json`: metrics and repeat-divergence timestamps; `charts/*.svg`: standalone vector figures.
