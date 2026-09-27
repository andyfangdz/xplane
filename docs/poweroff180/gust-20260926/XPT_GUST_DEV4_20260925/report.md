# Native X-Plane harness report

**0/2 flights passed the landing limits.** Measurement-valid flights: 2. Installation restored: True.

Landing pass/fail is separate from harness operation. Native guidance runs on simulator frames; Python only supervises it. All attempts, including aborted and non-passing flights, remain in this report.

| Trial | Touchdown ft | KIAS | Physical sink fpm | Peak pitch ° | Peak pitch rate °/s | Result |
|---|---:|---:|---:|---:|---:|---|
| head10_gust10-01 | 1088.9 | 64.5 | 313.5 | 6.5 | 2.5 | Physical sink |
| head10_gust10-02 | 1061.9 | 65.8 | 399.6 | 5.5 | 3.0 | Physical sink; Flare motion |

## head10 gust10

![Flight-path, speed, pitch and descent overlays](charts/head10_gust10.png)

- ias_kias spread 5.7 at 57.4s exceeds diagnostic threshold 3
- pitch_deg spread 6.3 at 76.7s exceeds diagnostic threshold 2
- physical_sink_fpm spread 424.2 at 76.5s exceeds diagnostic threshold 150

## Evidence

- `resolved-config.json` and each card’s `effective-config.json`: complete settings, with no inactive legacy fields.
- `native-effective.ini`: exact configuration read back from the plugin before flight.
- `trace.csv`: native-frame observations; `supervision.json`: independent polling and deliberate gap probes.
- `source-manifest.json`: harness, native binary, setup adapter and original aircraft lineage.
- `events.jsonl`, `status.json`: structured progress; `restoration.json`: recovery verification.
- `summary.json`: metrics and repeat-divergence timestamps; `charts/*.svg`: standalone vector figures.
