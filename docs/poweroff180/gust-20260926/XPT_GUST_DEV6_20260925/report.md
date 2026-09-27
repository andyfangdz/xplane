# Native X-Plane harness report

**0/3 flights passed the landing limits.** Measurement-valid flights: 3. Installation restored: True.

Landing pass/fail is separate from harness operation. Native guidance runs on simulator frames; Python only supervises it. All attempts, including aborted and non-passing flights, remain in this report.

| Trial | Touchdown ft | KIAS | Physical sink fpm | Peak pitch ° | Peak pitch rate °/s | Result |
|---|---:|---:|---:|---:|---:|---|
| head10_gust10-01 | 1091.0 | 68.9 | 287.4 | 4.3 | 2.9 | Physical sink; Flare motion |
| head10_gust10-02 | 1134.6 | 68.2 | 228.8 | 5.4 | 2.8 | Physical sink |
| head10_gust10-03 | 1139.8 | 68.3 | 223.4 | 5.3 | 2.8 | Physical sink; Flare motion |

## head10 gust10

![Flight-path, speed, pitch and descent overlays](charts/head10_gust10.png)

- ias_kias spread 5.4 at 57.2s exceeds diagnostic threshold 3
- pitch_deg spread 4.4 at 74.6s exceeds diagnostic threshold 2
- physical_sink_fpm spread 306.9 at 74.5s exceeds diagnostic threshold 150

## Evidence

- `resolved-config.json` and each card’s `effective-config.json`: complete settings, with no inactive legacy fields.
- `native-effective.ini`: exact configuration read back from the plugin before flight.
- `trace.csv`: native-frame observations; `supervision.json`: independent polling and deliberate gap probes.
- `source-manifest.json`: harness, native binary, setup adapter and original aircraft lineage.
- `events.jsonl`, `status.json`: structured progress; `restoration.json`: recovery verification.
- `summary.json`: metrics and repeat-divergence timestamps; `charts/*.svg`: standalone vector figures.
