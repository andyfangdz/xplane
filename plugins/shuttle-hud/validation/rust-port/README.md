# Rust 143 native validation archive

These local orchestration scripts record the release-143 native validation,
including the corrected chute-area assertion in `reload.py`. They depend on the
original local flight-test harness, screenshot helper, scenery snapshot and
request templates under the X-Plane `Output` directory. They are archived for
inspection, not installed in the aircraft or presented as a portable runner.

Only the separate Shuttle Rust Trial aircraft contained `ValidationController.lua`
as `Final_Demo.lua`. Its control laws were unchanged; only the output directory
was changed. That trial was retired outside the Aircraft scan path after normal
shutdown. The two flown runs were 302 and 303; 301 was setup only.

Portable comparisons are the Rust tests under `../../tests`. The report's
`analyze_trial.py` operates on its sibling traces/results without X-Plane.
Native evidence and setup failures are documented in the
[native validation report](../../../../docs/shuttle-hud/rust-port/README.md).

`check_exports.py` validates the current project's [API contract](../api-contract.json)
and is independent of the archived session setup. For a native read-only check:

```powershell
python check_exports.py --harness "D:/X-Plane 12/Support/flight-test-harness/vendor" --port 8144 --output "D:/X-Plane 12/Output/shuttle-api-check.json"
```

Use `--catalog` with a saved JSON object containing `datarefs` and `commands`
maps to validate offline. The current contract includes release 144's runway
projection diagnostic; a release-143 catalog correctly fails that requirement.
