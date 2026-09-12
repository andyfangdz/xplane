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
    "utc": "2026-09-12T01:44:16.6055454Z"
  },
  {
    "error": "Simulator API did not become ready",
    "traceback": "Traceback (most recent call last):\n  File \"V:\\src\\xplane\\tools\\flight-test-harness\\vendor\\flight_test\\api.py\", line 62, in request\n    connection.request(method, f\"{self.base_path}{path}\", body=encoded, headers=headers)\n    ~~~~~~~~~~~~~~~~~~^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^\n  File \"C:\\Users\\andyf\\scoop\\apps\\python\\3.13.3\\Lib\\http\\client.py\", line 1338, in request\n    self._send_request(method, url, body, headers, encode_chunked)\n    ~~~~~~~~~~~~~~~~~~^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^\n  File \"C:\\Users\\andyf\\scoop\\apps\\python\\3.13.3\\Lib\\http\\client.py\", line 1384, in _send_request\n    self.endheaders(body, encode_chunked=encode_chunked)\n    ~~~~~~~~~~~~~~~^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^\n  File \"C:\\Users\\andyf\\scoop\\apps\\python\\3.13.3\\Lib\\http\\client.py\", line 1333, in endheaders\n    self._send_output(message_body, encode_chunked=encode_chunked)\n    ~~~~~~~~~~~~~~~~~^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^\n  File \"C:\\Users\\andyf\\scoop\\apps\\python\\3.13.3\\Lib\\http\\client.py\", line 1093, in _send_output\n    self.send(msg)\n    ~~~~~~~~~^^^^^\n  File \"C:\\Users\\andyf\\scoop\\apps\\python\\3.13.3\\Lib\\http\\client.py\", line 1037, in send\n    self.connect()\n    ~~~~~~~~~~~~^^\n  File \"C:\\Users\\andyf\\scoop\\apps\\python\\3.13.3\\Lib\\http\\client.py\", line 1003, in connect\n    self.sock = self._create_connection(\n                ~~~~~~~~~~~~~~~~~~~~~~~^\n        (self.host,self.port), self.timeout, self.source_address)\n        ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^\n  File \"C:\\Users\\andyf\\scoop\\apps\\python\\3.13.3\\Lib\\socket.py\", line 864, in create_connection\n    raise exceptions[0]\n  File \"C:\\Users\\andyf\\scoop\\apps\\python\\3.13.3\\Lib\\socket.py\", line 849, in create_connection\n    sock.connect(sa)\n    ~~~~~~~~~~~~^^^^\nConnectionRefusedError: [WinError 10061] No connection could be made because the target machine actively refused it\n\nThe above exception was the direct cause of the following exception:\n\nTraceback (most recent call last):\n  File \"V:\\src\\xplane\\tools\\flight-test-harness\\xpt\\cli.py\", line 48, in worker\n    try:api.request('GET','/datarefs?limit=1');break\n        ~~~~~~~~~~~^^^^^^^^^^^^^^^^^^^^^^^^^^^\n  File \"V:\\src\\xplane\\tools\\flight-test-harness\\vendor\\flight_test\\api.py\", line 76, in request\n    raise XPlaneConnectionError(f\"{method} {path} failed: {error}\") from error\nflight_test.api.XPlaneConnectionError: GET /datarefs?limit=1 failed: [WinError 10061] No connection could be made because the target machine actively refused it\n\nDuring handling of the above exception, another exception occurred:\n\nTraceback (most recent call last):\n  File \"V:\\src\\xplane\\tools\\flight-test-harness\\xpt\\cli.py\", line 50, in worker\n    if time.monotonic()>deadline:raise TimeoutError('Simulator API did not become ready')\n                                 ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^\nTimeoutError: Simulator API did not become ready\n"
  }
]
```
