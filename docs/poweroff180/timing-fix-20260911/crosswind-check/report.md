# Native X-Plane harness report

**4/4 flights passed the landing limits.** Measurement-valid flights: 4. Installation restored: True.

Landing pass/fail is separate from harness operation. Native guidance runs on simulator frames; Python only supervises it. All attempts, including aborted and non-passing flights, remain in this report.

| Trial | Touchdown ft | KIAS | Physical sink fpm | Peak pitch ° | Peak pitch rate °/s | Result |
|---|---:|---:|---:|---:|---:|---|
| cross_left10-01 | 1045.6 | 66.3 | 142.9 | 6.3 | 2.1 | Pass |
| cross_left10-02 | 1111.2 | 65.4 | 75.6 | 6.8 | 2.1 | Pass |
| cross_left10-03 | 1061.5 | 65.3 | 68.3 | 7.3 | 2.2 | Pass |
| cross_left10-04 | 1191.7 | 66.8 | 127.6 | 5.6 | 1.7 | Pass |

## cross left10

![Flight-path, speed, pitch and descent overlays](charts/cross_left10.png)

- ias_kias spread 3.4 at 82.5s exceeds diagnostic threshold 3
- pitch_deg spread 3.3 at 80.9s exceeds diagnostic threshold 2
- physical_sink_fpm spread 169.1 at 55.0s exceeds diagnostic threshold 150

## Evidence

- `resolved-config.json` and each card’s `effective-config.json`: complete settings, with no inactive legacy fields.
- `native-effective.ini`: exact configuration read back from the plugin before flight.
- `trace.csv`: native-frame observations; `supervision.json`: independent polling and deliberate gap probes.
- `source-manifest.json`: harness, native binary, setup adapter and original aircraft lineage.
- `events.jsonl`, `status.json`: structured progress; `restoration.json`: recovery verification.
- `summary.json`: metrics and repeat-divergence timestamps; `charts/*.svg`: standalone vector figures.
