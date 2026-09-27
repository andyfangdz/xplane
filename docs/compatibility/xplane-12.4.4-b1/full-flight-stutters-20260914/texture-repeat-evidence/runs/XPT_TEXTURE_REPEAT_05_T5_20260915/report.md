# Native X-Plane harness report

**1/2 flights passed the landing limits.** Measurement-valid flights: 2. Installation restored: True.

Landing pass/fail is separate from harness operation. Native guidance runs on simulator frames; Python only supervises it. All attempts, including aborted and non-passing flights, remain in this report.

| Trial | Touchdown ft | KIAS | Physical sink fpm | Peak pitch ° | Peak pitch rate °/s | Result |
|---|---:|---:|---:|---:|---:|---|
| calm-01 | 1053.7 | 66.9 | 82.8 | 6.3 | 2.4 | Pass |
| calm-02 | 1085.5 | 67.2 | 112.3 | 5.4 | 2.4 | Touchdown speed |

## calm

![Flight-path, speed, pitch and descent overlays](charts/calm.png)

- pitch_deg spread 3.6 at 82.3s exceeds diagnostic threshold 2

## Evidence

- `resolved-config.json` and each card’s `effective-config.json`: complete settings, with no inactive legacy fields.
- `native-effective.ini`: exact configuration read back from the plugin before flight.
- `trace.csv`: native-frame observations; `supervision.json`: independent polling and deliberate gap probes.
- `source-manifest.json`: harness, native binary, setup adapter and original aircraft lineage.
- `events.jsonl`, `status.json`: structured progress; `restoration.json`: recovery verification.
- `summary.json`: metrics and repeat-divergence timestamps; `charts/*.svg`: standalone vector figures.

## Execution errors

```json
[
  {
    "error": "An X-Plane process is active; recovery will not move files",
    "utc": "2026-09-15T03:09:35.5817357Z"
  },
  {
    "utc": "2026-09-15T03:09:36.9152056Z",
    "error": "Launcher process ended before restoration"
  }
]
```
