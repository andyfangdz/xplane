# Original validation orchestration

These scripts are preserved from the local symbology experiment. They retain
their original paths, temporary-aircraft assumptions and references to the
separately installed flight-test harness. They can change simulator state and
session scenery settings; they are development records, not installation tools.

Use the portable build and `install_native.py` in `plugins/shuttle-hud` for the
released aircraft. `ValidationController.lua` is a test pilot and is never copied
by that installer. The report's `analyze_trial.py` works directly on the included
traces without connecting to X-Plane.
