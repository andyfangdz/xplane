# Native X-Plane harness report

**6/6 flights passed the landing limits.** Measurement-valid flights: 6. Installation restored: True.

Landing pass/fail is separate from harness operation. Native guidance runs on simulator frames; Python only supervises it. All attempts, including aborted and non-passing flights, remain in this report.

| Trial | Touchdown ft | KIAS | Physical sink fpm | Peak pitch ° | Peak pitch rate °/s | Result |
|---|---:|---:|---:|---:|---:|---|
| calm-01 | 1033.8 | 66.1 | 54.6 | 6.9 | 2.5 | Pass |
| calm-02 | 1080.0 | 66.1 | 43.6 | 6.7 | 2.5 | Pass |
| cross_left10-01 | 1082.2 | 66.2 | 122.1 | 6.2 | 2.1 | Pass |
| cross_left10-02 | 1048.9 | 65.5 | 52.9 | 7.4 | 2.5 | Pass |
| head15-01 | 1057.6 | 64.9 | 146.0 | 7.3 | 2.6 | Pass |
| head15-02 | 1112.8 | 65.3 | 183.6 | 7.1 | 2.5 | Pass |

## calm

![Flight-path, speed, pitch and descent overlays](charts/calm.png)

No repeat-divergence diagnostic threshold was exceeded.

## cross left10

![Flight-path, speed, pitch and descent overlays](charts/cross_left10.png)

- pitch_deg spread 3.6 at 80.9s exceeds diagnostic threshold 2

## head15

![Flight-path, speed, pitch and descent overlays](charts/head15.png)

- pitch_deg spread 5.2 at 80.2s exceeds diagnostic threshold 2
- physical_sink_fpm spread 193.0 at 79.9s exceeds diagnostic threshold 150

## Evidence

- `resolved-config.json` and each card’s `effective-config.json`: complete settings, with no inactive legacy fields.
- `native-effective.ini`: exact configuration read back from the plugin before flight.
- `trace.csv`: native-frame observations; `supervision.json`: independent polling and deliberate gap probes.
- `source-manifest.json`: harness, native binary, setup adapter and original aircraft lineage.
- `events.jsonl`, `status.json`: structured progress; `restoration.json`: recovery verification.
- `summary.json`: metrics and repeat-divergence timestamps; `charts/*.svg`: standalone vector figures.
