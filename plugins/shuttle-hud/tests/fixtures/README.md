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
Integer state and topology must match exactly. Floating-point tolerance is
`1e-8 + abs(expected) * 2e-12` in each quantity's native units.

Treat changes to expected results as behavior changes: review them against the
landing equations, display requirements and native simulator evidence before
updating the data and hashes. Test runs never rewrite the baseline. Scenery
projection uses separate matrix/geometry unit tests and native alignment checks;
the scene fixtures exercise the configured fallback geometry.
