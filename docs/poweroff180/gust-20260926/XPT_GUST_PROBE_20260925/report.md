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
    "stack": "at <ScriptBlock>, V:\\src\\xplane\\tools\\flight-test-harness\\Run-XPlaneTest.ps1: line 175\r\nat <ScriptBlock>, <No file>: line 1\r\nat <ScriptBlock>, <No file>: line 1",
    "utc": "2026-09-26T01:40:59.9621759Z"
  },
  {
    "error": "Could not set paused=False",
    "traceback": "Traceback (most recent call last):\n  File \"V:\\src\\xplane\\tools\\flight-test-harness\\xpt\\cli.py\", line 69, in worker\n    document,rows=adapter.execute()\n                  ~~~~~~~~~~~~~~~^^\n  File \"V:\\src\\xplane\\tools\\flight-test-harness\\xpt\\adapter.py\", line 244, in execute\n    self._prepare_flight()\n    ~~~~~~~~~~~~~~~~~~~~^^\n  File \"V:\\src\\xplane\\tools\\flight-test-harness\\xpt\\adapter.py\", line 56, in _prepare_flight\n    result=super()._prepare_flight()\n  File \"V:\\src\\xplane\\tools\\flight-test-harness\\vendor\\flight_test\\short_field.py\", line 592, in _prepare_flight\n    self._set_pause(False)\n    ~~~~~~~~~~~~~~~^^^^^^^\n  File \"V:\\src\\xplane\\tools\\flight-test-harness\\xpt\\adapter.py\", line 130, in _set_pause\n    result = super()._set_pause(paused)\n  File \"V:\\src\\xplane\\tools\\flight-test-harness\\vendor\\flight_test\\short_field.py\", line 301, in _set_pause\n    raise RuntimeError(f\"Could not set paused={paused}\")\nRuntimeError: Could not set paused=False\n"
  }
]
```
