# Native X-Plane harness report

**6/6 flights passed the landing limits.** Measurement-valid flights: 6. Installation restored: True.

Landing pass/fail is separate from harness operation. Native guidance runs on simulator frames; Python only supervises it. All attempts, including aborted and non-passing flights, remain in this report.

| Trial | Touchdown ft | KIAS | Physical sink fpm | Peak pitch ° | Peak pitch rate °/s | Result |
|---|---:|---:|---:|---:|---:|---|
| calm-01 | 1032.1 | 66.1 | 54.4 | 6.9 | 2.5 | Pass |
| calm-02 | 1074.8 | 66.1 | 44.8 | 6.7 | 2.5 | Pass |
| cross_left10-01 | 1078.2 | 66.2 | 120.2 | 6.3 | 2.1 | Pass |
| cross_left10-02 | 1045.2 | 65.5 | 52.8 | 7.4 | 2.5 | Pass |
| head15-01 | 1056.9 | 64.9 | 145.4 | 7.3 | 2.6 | Pass |
| head15-02 | 1113.0 | 65.3 | 174.7 | 7.1 | 2.6 | Pass |

## calm

![Flight-path, speed, pitch and descent overlays](charts/calm.png)

No repeat-divergence diagnostic threshold was exceeded.

## cross left10

![Flight-path, speed, pitch and descent overlays](charts/cross_left10.png)

- pitch_deg spread 3.5 at 81.0s exceeds diagnostic threshold 2

## head15

![Flight-path, speed, pitch and descent overlays](charts/head15.png)

- pitch_deg spread 5.3 at 80.2s exceeds diagnostic threshold 2
- physical_sink_fpm spread 188.8 at 79.9s exceeds diagnostic threshold 150

## Evidence

- `resolved-config.json` and each card’s `effective-config.json`: complete settings, with no inactive legacy fields.
- `native-effective.ini`: exact configuration read back from the plugin before flight.
- `trace.csv`: native-frame observations; `supervision.json`: independent polling and deliberate gap probes.
- `source-manifest.json`: harness, native binary, setup adapter and original aircraft lineage.
- `events.jsonl`, `status.json`: structured progress; `restoration.json`: recovery verification.
- `summary.json`: metrics and repeat-divergence timestamps; `charts/*.svg`: standalone vector figures.
