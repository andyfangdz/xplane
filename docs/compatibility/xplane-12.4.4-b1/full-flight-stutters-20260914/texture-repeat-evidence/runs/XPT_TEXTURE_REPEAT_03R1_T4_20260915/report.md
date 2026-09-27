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
    "stack": "at <ScriptBlock>, V:\\src\\xplane\\tools\\flight-test-harness\\Run-XPlaneTest.ps1: line 175\r\nat <ScriptBlock>, V:\\src\\xplane\\build\\stutter-textures-20260915\\Run-TextureTest.ps1: line 43\r\nat <ScriptBlock>, <No file>: line 10",
    "utc": "2026-09-15T02:33:46.9168951Z",
    "error": "Worker failed with exit code 1; see worker-error.log"
  },
  {
    "error": "0:06:42.903 E/SYS: MACIBM_alert: Error generating path for aircraft preview.\n0:06:42.903 E/SYS: MACIBM_alert: No liveries for aircraft \n0:06:42.903 E/SYS: MACIBM_alert: Please report this to Laminar Research.\n0:06:42.903 E/SYS: MACIBM_alert: \n0:06:42.903 E/SYS: MACIBM_alert: C:/Users/Administrator/builds/GecRx2m4B/0/x-plane/design/source_code/app/screens/freeflight_config/widgets/aircraft_grid_widget.cpp:565",
    "traceback": "Traceback (most recent call last):\n  File \"V:\\src\\xplane\\tools\\flight-test-harness\\xpt\\cli.py\", line 69, in worker\n    document,rows=adapter.execute()\n                  ~~~~~~~~~~~~~~~^^\n  File \"V:\\src\\xplane\\tools\\flight-test-harness\\xpt\\adapter.py\", line 242, in execute\n    self._prepare_flight()\n    ~~~~~~~~~~~~~~~~~~~~^^\n  File \"V:\\src\\xplane\\tools\\flight-test-harness\\xpt\\adapter.py\", line 56, in _prepare_flight\n    result=super()._prepare_flight()\n  File \"V:\\src\\xplane\\tools\\flight-test-harness\\vendor\\flight_test\\short_field.py\", line 592, in _prepare_flight\n    self._set_pause(False)\n    ~~~~~~~~~~~~~~~^^^^^^^\n  File \"V:\\src\\xplane\\tools\\flight-test-harness\\xpt\\adapter.py\", line 126, in _set_pause\n    check_startup(self.resolved['xplane_root'])\n    ~~~~~~~~~~~~~^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^\n  File \"V:\\src\\xplane\\tools\\flight-test-harness\\xpt\\readiness.py\", line 24, in check_startup\n    if state['fatal']:raise SimulatorStartupError('\\n'.join(state['fatal']))\n                      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^\nxpt.readiness.SimulatorStartupError: 0:06:42.903 E/SYS: MACIBM_alert: Error generating path for aircraft preview.\n0:06:42.903 E/SYS: MACIBM_alert: No liveries for aircraft \n0:06:42.903 E/SYS: MACIBM_alert: Please report this to Laminar Research.\n0:06:42.903 E/SYS: MACIBM_alert: \n0:06:42.903 E/SYS: MACIBM_alert: C:/Users/Administrator/builds/GecRx2m4B/0/x-plane/design/source_code/app/screens/freeflight_config/widgets/aircraft_grid_widget.cpp:565\n"
  }
]
```
