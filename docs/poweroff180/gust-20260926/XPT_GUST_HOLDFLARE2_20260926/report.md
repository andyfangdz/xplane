# Native X-Plane harness report

**1/3 flights passed the landing limits.** Measurement-valid flights: 3. Installation restored: True.

Landing pass/fail is separate from harness operation. Native guidance runs on simulator frames; Python only supervises it. All attempts, including aborted and non-passing flights, remain in this report.

| Trial | Touchdown ft | KIAS | Physical sink fpm | Peak pitch ° | Peak pitch rate °/s | Result |
|---|---:|---:|---:|---:|---:|---|
| head10_gust10-01 | 1177.6 | 66.9 | 247.5 | 5.1 | 2.9 | Flare motion |
| head15_gust8-01 | 1175.1 | 66.2 | 292.6 | 6.0 | 2.7 | Pass |
| head5_gust8-01 | 989.6 | 70.8 | 214.2 | 4.1 | 2.6 | Touchdown distance |

## head10 gust10

![Flight-path, speed, pitch and descent overlays](charts/head10_gust10.png)

- Fewer than two traces; repeatability is not established

## head15 gust8

![Flight-path, speed, pitch and descent overlays](charts/head15_gust8.png)

- Fewer than two traces; repeatability is not established

## head5 gust8

![Flight-path, speed, pitch and descent overlays](charts/head5_gust8.png)

- Fewer than two traces; repeatability is not established

## Evidence

- `resolved-config.json` and each card’s `effective-config.json`: complete settings, with no inactive legacy fields.
- `native-effective.ini`: exact configuration read back from the plugin before flight.
- `trace.csv`: native-frame observations; `supervision.json`: independent polling and deliberate gap probes.
- `source-manifest.json`: harness, native binary, setup adapter and original aircraft lineage.
- `events.jsonl`, `status.json`: structured progress; `restoration.json`: recovery verification.
- `summary.json`: metrics and repeat-divergence timestamps; `charts/*.svg`: standalone vector figures.
