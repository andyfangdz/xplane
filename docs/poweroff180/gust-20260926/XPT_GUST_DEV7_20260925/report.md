# Native X-Plane harness report

**0/3 flights passed the landing limits.** Measurement-valid flights: 3. Installation restored: True.

Landing pass/fail is separate from harness operation. Native guidance runs on simulator frames; Python only supervises it. All attempts, including aborted and non-passing flights, remain in this report.

| Trial | Touchdown ft | KIAS | Physical sink fpm | Peak pitch ° | Peak pitch rate °/s | Result |
|---|---:|---:|---:|---:|---:|---|
| head10_gust10-01 | 999.3 | 74.1 | 131.3 | 3.0 | 2.2 | Touchdown distance |
| head10_gust10-02 | 1159.2 | 70.9 | 239.1 | 3.7 | 2.7 | Physical sink |
| head10_gust10-03 | 1148.4 | 70.9 | 244.2 | 3.7 | 2.8 | Physical sink; Flare motion |

## head10 gust10

![Flight-path, speed, pitch and descent overlays](charts/head10_gust10.png)

- ias_kias spread 6.5 at 57.4s exceeds diagnostic threshold 3
- pitch_deg spread 2.4 at 72.7s exceeds diagnostic threshold 2
- physical_sink_fpm spread 283.3 at 71.6s exceeds diagnostic threshold 150

## Evidence

- `resolved-config.json` and each card’s `effective-config.json`: complete settings, with no inactive legacy fields.
- `native-effective.ini`: exact configuration read back from the plugin before flight.
- `trace.csv`: native-frame observations; `supervision.json`: independent polling and deliberate gap probes.
- `source-manifest.json`: harness, native binary, setup adapter and original aircraft lineage.
- `events.jsonl`, `status.json`: structured progress; `restoration.json`: recovery verification.
- `summary.json`: metrics and repeat-divergence timestamps; `charts/*.svg`: standalone vector figures.
