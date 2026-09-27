# Native X-Plane harness report

**0/0 flights passed the landing limits.** Measurement-valid flights: 0. Installation restored: True.

Landing pass/fail is separate from harness operation. Native guidance runs on simulator frames; Python only supervises it. All attempts, including aborted and non-passing flights, remain in this report.

| Trial | Touchdown ft | KIAS | Physical sink fpm | Peak pitch ° | Peak pitch rate °/s | Result |
|---|---:|---:|---:|---:|---:|---|

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
    "utc": "2026-09-15T02:23:56.0446338Z",
    "stack": "at <ScriptBlock>, V:\\src\\xplane\\tools\\flight-test-harness\\Run-XPlaneTest.ps1: line 175\r\nat <ScriptBlock>, V:\\src\\xplane\\build\\stutter-textures-20260915\\Run-TextureTest.ps1: line 43\r\nat <ScriptBlock>, <No file>: line 2",
    "error": "Worker failed with exit code -1; see worker-error.log"
  }
]
```
