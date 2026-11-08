# C182 egui windows

Asset-free Rust views for Controls, Payload & fuel, Maintenance, Preflight,
Engine, Radios, Pause rules, Notes, and notifications. The public crate owns
layout, styling, navigation, scrolling, filtering and interaction state. It
contains no aircraft assets or recovered scripts.

The private aircraft adapter supplies borrowed display snapshots through
[`include/c182_egui.hpp`](include/c182_egui.hpp). The `xplane` feature adds the
Windows SDK bridge using the public `xplane-egui` renderer. Actions return through
a FIFO; the adapter executes source handlers on its existing model tick. This
preserves held controls, right-button flags, wheel contracts, variant locks and
source slider mapping. Hiding a window clears pending egui input; the aircraft
adapter owns release/drain ordering during close and tab changes.

```powershell
cargo test -p c182-ui
cargo check -p c182-ui --features xplane
cargo run -p c182-ui --example preview -- ../xplane-private/evidence/ui
python tools/c182/python/render_ui_preview.py ../xplane-private/evidence/ui
```

The preview uses synthetic data and rasterizes actual egui meshes, with NumPy
and Pillow for the optional PNG step. It checks layout without a simulator.
X-Plane acceptance still needs mouse/VR input, keyboard focus, clipping, window
placement, and OpenGL drawing checks. Cockpit instrument rendering and installed
third-party avionics remain separate from these utility windows.
