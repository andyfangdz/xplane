# Shuttle HUD regression fixtures

These frozen project results cover 737 landing-path samples (including segment
joins), 3,053 heavy/light model frames, and 24 cockpit/full-screen scenes with
10,163 vector segments. The baseline was established for release 142 and
verified by the Rust test suite for releases 143 and 144. The recorded numbers
are unchanged; [PROVENANCE.json](PROVENANCE.json) records their SHA-256 hashes.

Run `cargo test -p shuttle-hud-rs --test regression` from the workspace with the
X-Plane SDK runtime libraries on PATH, or use the root build script. The tests
use the project's [Rust modules](../../src) directly. They require no fixture
generator or additional compiler.

`path-baseline.csv` records along-runway distance, height, slope, curvature and
segment. `model-baseline.csv` contains the reset flag, 15 flight inputs and 19
guidance/presentation results. `scene-inputs.csv` supplies 31 state inputs;
`scene-baseline.csv` records case/layer/segment indices, endpoints, visibility
and clipped endpoints. See [regression.rs](../regression.rs) for field mappings.
Flight state, gear, latches, cues, event timestamps, and scene topology/visibility
must match exactly. Floating-point tolerances reflect the physical output:
1 mm for path/target/flare height, 0.001 degree for guidance angles, 0.00001 for
control ratios, 0.001 m/s² for deceleration, and 0.01 pixel for scene endpoints.
Path slope and curvature limits are 0.000001 and 0.00000001 per metre. Continuous
presentation timers allow 0.000001 second.

A frozen path sample within 0.000001 m of a segment join may belong to either
adjacent segment after unit conversion rounding. Its height, slope and curvature
must still pass; separate assertions check the intended side of each current
join. No such exception applies to flight phases or event latches.

Treat changes to expected results as behavior changes: review them against the
landing equations, display requirements and native simulator evidence before
updating the data and hashes. Test runs never rewrite the baseline. Scenery
projection uses separate matrix/geometry unit tests and native alignment checks;
the scene fixtures exercise the configured fallback geometry.
