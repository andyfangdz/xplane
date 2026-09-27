# Native X-Plane harness report

**2/6 flights passed the landing limits.** Measurement-valid flights: 6. Installation restored: True.

Landing pass/fail is separate from harness operation. Native guidance runs on simulator frames; Python only supervises it. All attempts, including aborted and non-passing flights, remain in this report.

| Trial | Touchdown ft | KIAS | Physical sink fpm | Peak pitch ° | Peak pitch rate °/s | Result |
|---|---:|---:|---:|---:|---:|---|
| cross_left10_gust8-01 | 1178.9 | 70.8 | 185.8 | 3.5 | 2.3 | Pass |
| cross_right10_gust8-01 | 1027.3 | 69.1 | 47.6 | 5.2 | 2.4 | Alignment |
| head10_gust10-01 | 1063.7 | 69.1 | 155.5 | 5.4 | 2.1 | Pass |
| head15_gust8-01 | 794.3 | 69.8 | 157.9 | 4.7 | 2.8 | Touchdown distance; Flare motion |
| head5_gust8-01 | 985.8 | 71.4 | 280.8 | 3.6 | 2.8 | Touchdown distance; Physical sink; Flare motion |
| tail5_gust5-01 | 1231.1 | 71.0 | 193.9 | 2.8 | 1.4 | Touchdown distance; Touchdown speed |

## cross left10 gust8

![Flight-path, speed, pitch and descent overlays](charts/cross_left10_gust8.png)

- Fewer than two traces; repeatability is not established

## cross right10 gust8

![Flight-path, speed, pitch and descent overlays](charts/cross_right10_gust8.png)

- Fewer than two traces; repeatability is not established

## head10 gust10

![Flight-path, speed, pitch and descent overlays](charts/head10_gust10.png)

- Fewer than two traces; repeatability is not established

## head15 gust8

![Flight-path, speed, pitch and descent overlays](charts/head15_gust8.png)

- Fewer than two traces; repeatability is not established

## head5 gust8

![Flight-path, speed, pitch and descent overlays](charts/head5_gust8.png)

- Fewer than two traces; repeatability is not established

## tail5 gust5

![Flight-path, speed, pitch and descent overlays](charts/tail5_gust5.png)

- Fewer than two traces; repeatability is not established

## Evidence

- `resolved-config.json` and each card’s `effective-config.json`: complete settings, with no inactive legacy fields.
- `native-effective.ini`: exact configuration read back from the plugin before flight.
- `trace.csv`: native-frame observations; `supervision.json`: independent polling and deliberate gap probes.
- `source-manifest.json`: harness, native binary, setup adapter and original aircraft lineage.
- `events.jsonl`, `status.json`: structured progress; `restoration.json`: recovery verification.
- `summary.json`: metrics and repeat-divergence timestamps; `charts/*.svg`: standalone vector figures.
