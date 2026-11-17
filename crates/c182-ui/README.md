# C182 egui windows

Asset-free Rust views for Controls, Payload & fuel, Maintenance, Preflight,
Engine, Radios, Pause rules, Notes, and notifications. The public crate owns
layout, styling, navigation, scrolling, filtering and interaction state. It
contains no aircraft assets or recovered scripts.

The aircraft supplies typed `view::Page` display snapshots (`page.rs`). A page is
either a plain list of action rows beside its readouts, or a structured page:
titled panels in two columns built from status rows with their repair action,
on/off switches, one-of-several choices, −/+ steppers, level bars, captioned
fields and text, with an optional banner (why a page is locked, and the way
past it) and an optional dialog that must be confirmed or cancelled. Labels,
states and tones all arrive as data.

The `xplane` feature adds the Windows SDK bridge using the public `xplane-egui`
renderer. Actions return through a FIFO; the adapter executes source handlers on
its existing model tick. This preserves held controls, right-button flags,
variant locks and source slider mapping. The wheel only scrolls: it never
changes the control under the pointer, so a value that needs fine adjustment
is given a slider or −/+ buttons by the adapter. Hiding a window clears
pending egui input; the aircraft adapter owns release/drain ordering during
close and tab changes.

Rust callers use `surface::UiSurface` with a typed `view::Page`, input methods
and `next_action()`. The view owns no aircraft state and executes no aircraft
action. The retired C++ bridge, C entry points and header have been removed.

```powershell
cargo test -p c182-ui
cargo check -p c182-ui --features xplane
cargo run -p c182-ui --example preview -- build/c182-ui-preview
python tools/c182/python/render_ui_preview.py build/c182-ui-preview
```

The preview draws one synthetic page per tab, plus a locked page and a dialog,
and rasterizes actual egui meshes, with NumPy and Pillow for the optional PNG
step. It checks layout without a simulator.
X-Plane acceptance still needs mouse/VR input, keyboard focus, clipping, window
placement, and OpenGL drawing checks. Cockpit instrument rendering and installed
third-party avionics remain separate from these utility windows.
