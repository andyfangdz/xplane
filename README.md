# Position Aircraft Native

Native Windows x64 X-Plane 12 plugin written in Rust. It reads and writes the
original Sandy Barbour `Resources/plugins/PositionAircraft/*.pad` files.

The plugin intentionally uses an XPLM 4.3 modern, decorated floating window
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
- X-Plane Plugin SDK 4.3

## Build and install

Point the build at the SDK directory containing `Libraries/Win/XPLM_64.lib`:

```powershell
$env:XPLM_SDK_PATH = "C:\path\to\XPSDK430\SDK"
.\build.ps1 -BuildOnly
```

The release DLL is written to
`target/release/position_aircraft_native.dll`. To build, test, and install it
in one command:

```powershell
.\build.ps1 -XPlanePath "D:\X-Plane 12"
```

The installed plugin is
`Resources/plugins/PositionAircraftNative/64/win.xpl`. Restart X-Plane after
replacing the binary.

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

- Solid blue and amber controls are actions; amber marks actions that move or
  save the aircraft immediately.
- Near-black outlined controls are editable fields. Hovering highlights the
  outline, and the focused field shows a cyan edge and text cursor.
- Click the current PAD filename to open the library dropdown. The dropdown
  shows the selected file, supports mouse-wheel scrolling anywhere over the
  plugin window while open, and includes a scrollbar with arrow and page
  controls. Load and Load + position remain separate deliberate actions.
- The status indicator at the bottom is green for normal results and red for
  errors.

## License

MIT
