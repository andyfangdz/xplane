# Position Aircraft Native

Native Windows x64 X-Plane 12 replacement for Sandy Barbour's
[Position Aircraft plugin](https://web.archive.org/web/20130908120408/http://www.xpluginsdk.org/position_aircraft.htm),
written in Rust with an [egui](https://github.com/emilk/egui) interface. It reads and writes the original
`Resources/plugins/PositionAircraft/*.pad` files.

The plugin intentionally uses a modern, decorated XPLM floating window
and leaves unhandled mouse/controller events to X-Plane. This is also a test of
X-Plane 12.4.3's native spatial VR window manipulation, which FlyWithLua's
always-consumed window input can interfere with.

## Screenshots

![Position Aircraft Native main window](docs/screenshots/main-window.png)

![PAD library dropdown and scrollbar](docs/screenshots/pad-dropdown.png)

## Requirements

- Windows x64 and X-Plane 12
- Stable Rust with the MSVC target
- Visual Studio C++ Build Tools

## Build and install

```powershell
.\build.ps1 -BuildOnly
```

The generated XPLM bindings and Windows import libraries come from the
`xplane-sdk-sys` crate, so a separate SDK download is not required.

The release DLL is written to
`target/release/position_aircraft_native.dll`. To build, test, and install it
in one command:

```powershell
.\build.ps1 -XPlanePath "D:\X-Plane 12"
```

The installed plugin is
`Resources/plugins/PositionAircraftNative/64/win.xpl`. Restart X-Plane after
replacing the binary.

## Source layout

- The root `Cargo.toml` defines a workspace whose plugin members live under
  `plugins/`.
- `plugins/position-aircraft/src/lib.rs` exposes only the five X-Plane plugin
  ABI entry points.
- `plugins/position-aircraft/src/runtime/` owns datarefs, simulator state,
  commands/menus, lifecycle/window setup, FFI helpers, and the egui adapter.
- `plugins/position-aircraft/src/pad.rs` owns the original PAD format,
  validation, and form conversion.
- `xplane-sdk-sys` supplies generated XPLM declarations; `windows-sys`
  supplies the WGL and Windows loader declarations.

To add another plugin, create a Cargo package under `plugins/` and add its
artifact/install mapping to `build.ps1`. Cargo discovers the workspace member
through the root `plugins/*` pattern.

## Safety boundaries

- Plugin state is thread-local, matching XPLM's plugin-thread callback model;
  XPLM and OpenGL handles are never declared `Send`.
- Datarefs are exposed to the runtime through a private safe wrapper. Raw SDK
  calls and buffer pointers stay at the FFI boundary.
- The crate denies unsafe operations inside unsafe functions unless they are
  placed in an explicit, documented `unsafe` block.

## Commands

All commands are assignable in X-Plane's keyboard/joystick settings under
`PositionAircraftNative/`:

- `toggle_window`
- `capture_current`
- `position_loaded`
- `quick_save`
- `quick_load`
- `quick_load_and_position`
- `previous_pad`
- `next_pad`
- `previous_pad_and_position`
- `next_pad_and_position`

The FlyWithLua implementation is not removed or disabled by installation.

## Window controls

- Blue and amber controls are actions; amber marks actions that immediately
  move the aircraft. Dark outlined controls are editable fields.
- Click the current PAD filename to open egui's bounded-height library
  dropdown. It supports wheel scrolling and a standard scrollbar; Load and
  Load + position remain separate deliberate actions.
- Hover, focus, text selection, keyboard navigation, clipping, and control
  styling are provided by egui rather than a custom widget implementation.
- The status indicator at the bottom is green for normal results and red for
  errors.

## License

MIT
