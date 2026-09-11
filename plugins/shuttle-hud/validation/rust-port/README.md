# Rust 143 native validation archive

These are the exact local orchestration scripts used for the Rust migration,
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
[migration report](../../../../docs/shuttle-hud/rust-port/README.md).
