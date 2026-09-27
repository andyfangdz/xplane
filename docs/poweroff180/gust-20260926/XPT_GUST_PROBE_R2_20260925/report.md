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
    "error": "Worker failed with exit code 1; see worker-error.log",
    "utc": "2026-09-26T01:54:04.6821703Z",
    "stack": "at <ScriptBlock>, V:\\src\\xplane\\tools\\flight-test-harness\\Run-XPlaneTest.ps1: line 175\r\nat <ScriptBlock>, <No file>: line 1\r\nat <ScriptBlock>, <No file>: line 1"
  },
  {
    "error": "Regional gust readback failed",
    "traceback": "Traceback (most recent call last):\n  File \"V:\\src\\xplane\\tools\\flight-test-harness\\xpt\\cli.py\", line 69, in worker\n    document,rows=adapter.execute()\n                  ~~~~~~~~~~~~~~~^^\n  File \"V:\\src\\xplane\\tools\\flight-test-harness\\xpt\\adapter.py\", line 309, in execute\n    self._apply_gust()\n    ~~~~~~~~~~~~~~~~^^\n  File \"V:\\src\\xplane\\tools\\flight-test-harness\\xpt\\adapter.py\", line 121, in _apply_gust\n    raise RuntimeError('Regional gust readback failed')\nRuntimeError: Regional gust readback failed\n"
  }
]
```
