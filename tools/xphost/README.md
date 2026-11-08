# xphost

A Windows x64 headless host for testing X-Plane plugins. It loads a real `win.xpl` against a local replacement `XPLM_64.dll`, runs the plugin lifecycle and flight-loop callbacks, and exposes a typed dataref store, commands, scripted checks, and a loopback HTTP API.

The [test-xplane-plugin skill](../../skills/test-xplane-plugin/SKILL.md) guides agents through quick headless reproductions, profile selection, assertions, and the handoff to simulator checks. See [skill setup](../../skills/README.md) to make it discoverable in Codex.

Use it for plugin logic, lifecycle, dataref/command contracts, and transport regression tests. It does not run X-Plane's flight model, renderer, audio engine, scenery, or other plugins. Passing here does not establish flight behavior or visual/audio correctness in X-Plane.

The host and fake SDK are Rust workspace packages. The synthetic C++ plugin is an independent ABI fixture. SDK bindings, libraries, and fixture headers come from the repo's existing `xplane-sdk-sys` dependency. Aircraft packages, vendor binaries, nav databases, and captured aircraft state are not bundled.

**Build on Windows**

Requirements are the repository's pinned Rust 1.99.1 toolchain with the `x86_64-pc-windows-msvc` target, Visual Studio Build Tools with the C++ workload and Windows SDK, and Python 3.10+ for the integration tests. Rust packages use edition 2024. The host itself needs C only for a small structured-exception guard; the independent test fixture uses C++17.

From the repository root:

```powershell
powershell -NoProfile -File tools/xphost/build.ps1
```

The build uses an active x64 MSVC environment or discovers Visual Studio through `vswhere`. `build.cmd` forwards the same parameters. Optional `-Sdk` (or `XPLM_SDK`) overrides fixture headers with an SDK root or its `CHeaders/XPLM` directory; Rust continues using the workspace's SDK bindings. `-DebugBuild` selects a debug build; release is the default. `-Cargo` selects a Cargo executable.

Cargo artifacts go to the workspace target directory; the build script stages runnable artifacts in ignored `tools/xphost/build/`:

- `xphost.exe` and `XPLM_64.dll`, which must remain side by side;
- an original, minimal plugin fixture under `fixture/` for standalone tests.

No X-Plane installation or aircraft package is needed to build or run those tests.

**Run the standalone tests**

```powershell
python tools/xphost/tools/transport_regression.py --out tools/xphost/build/test-results
python tools/xphost/tools/native_regression.py --out tools/xphost/build/native-results
python tools/xphost/tools/profile_regression.py --out tools/xphost/build/profile-results
cargo test -p xphost-core -p xphost
```

The tests exercise the fixture's lifecycle, script assertions, scalar/array/byte datarefs, command propagation, HTTP connections, and graceful shutdown. The `serve --selftest` option targets this bundled test fixture, not arbitrary third-party plugins. They launch and stop only their own host process. Test results and logs remain under the supplied output directory.

The native suite also checks all 116 original XPLM exports, stale and invalid handles, accessor re-entry and self-unregistration, callback rescheduling and destruction, coordinate/weather/nav/FMS contracts, and faults at nine native lifecycle/callback boundaries. These tests use synthetic inputs and no X-Plane installation.

**Rust organization and safety**

- `crates/xphost-core`: typed store, bounded array operations, scheduler, parked-state/derived values, nav parser, and versioned private control messages. It forbids unsafe code. Coordinates reuse `xplane-airports::LocalProjection` and `uom`.
- `crates/xphost-native`: thread-bound DLL ownership, plugin lifecycle, hidden OpenGL context, and native fault guards. `seh.c` is the small Windows SEH boundary; it calls foreign functions directly so a trapped fault does not unwind through a Rust callback trampoline.
- `crates/xphost-sdk`: the `XPLM_64.dll` replacement, with SDK types from `xplane-sdk-sys` and window bounds from `xplane-plugin`. Unsafe code adapts foreign buffers and registered callbacks. Integer handles are checked against owned collections; they are never dereferenced. Callback dispatch snapshots registrations and drops locks before entering plugin code.
- `tools/xphost`: the executable plus a reusable runner library for CLI setup, scripts and HTTP routing/transport. Both forbid unsafe code. HTTP workers queue owned requests; only the simulation thread calls the backend. A timeout cancels work that has not begun.

The executable and SDK DLL must be built together: their private control protocol is version-checked and is not a stable third-party API. The former C++ implementation is replaced, while CLI, script, HTTP and XPLM import contracts remain. Script numbers are now validated rather than silently converted to zero; arrays and requests have explicit size bounds. Simulator fixture writes can override read-only catalog datarefs; HTTP writes still enforce metadata writability. Native plugins remain responsible for honoring SDK pointer/callback contracts; SEH is fault reporting, not memory isolation.

**Run a plugin**

Use a disposable copy of the package under test. A plugin runs ordinary native code in this process and may write package state; xphost is not a sandbox.

```powershell
$hostExe = './tools/xphost/build/xphost.exe'
& $hostExe --pkg C:/Tests/MyAircraft --acf MyAircraft.acf --xpl C:/Tests/MyAircraft/plugins/MyPlugin/64/win.xpl --clock fast --seconds 30 --out ./tools/xphost/out/run
& $hostExe serve --pkg C:/Tests/MyAircraft --acf MyAircraft.acf --xpl C:/Tests/MyAircraft/plugins/MyPlugin/64/win.xpl --port 8199 --out ./tools/xphost/out/server
```

`--pkg` supplies the package root and `--acf` names an existing aircraft file relative to it. The host exposes that path to the plugin; it does not parse or simulate the ACF. `--xpl` selects the plugin explicitly. Automatic discovery is suitable only when the package contains one `plugins/*/64/win.xpl` candidate.

Simulator dataref and command catalogs, and optional navigation data, are external inputs. Use `--datarefs PATH`, `--commands PATH`, and `--navdat PATH` to supply them, or `none` to omit them. These inputs default to `none`. A small built-in simulator state remains available without catalogs; no navigation records are loaded by default. Navigation input is also copied into the fake system tree under the output directory for plugins that read it directly.

**Aircraft profiles**

`--profile generic|c182|FILE.json` selects aircraft assumptions explicitly; package/ACF names do not select a profile automatically. The default `generic` profile supplies no mass, engine count, electrical voltage, or engine indicator mappings, and parks at zero local height. Use `--profile c182` to retain the former 1 m parked height, 1200 kg mass, single engine, 24 V bus-zero ratio and piston ignition defaults. These are legacy test fixture values, not calibrated C182 performance data. Common SDK behavior, clock, weather, and geographic formulas remain independent of profiles.

The built-ins are [generic.json](../../crates/xphost-core/profiles/generic.json) and [c182.json](../../crates/xphost-core/profiles/c182.json), embedded in the binary. Copy one to create another aircraft profile; custom profiles do not require a rebuild:

```powershell
Copy-Item crates/xphost-core/profiles/c182.json ./my-airplane.json
./tools/xphost/build/xphost.exe --pkg C:/Tests/MyAircraft --acf MyAircraft.acf --profile ./my-airplane.json --flight-start running
```

Profile schema version 1 supports:

| Field | Meaning |
|---|---|
| `name`, `description` | Profile identity and provenance. |
| `parked_height_m` | Aircraft's local Y/AGL above the configured field elevation. |
| `mass_kg`, `tail_number` | Optional parked total mass and tail number; `null` leaves them unspecified. |
| `engine_count` | Number of engine slots to derive; zero means unspecified. |
| `avionics` | Optional `{ "bus_index": 0, "nominal_volts": 24.0 }` for the SDK avionics voltage ratio. |
| `engine_indicators` | Float-array mappings with full `source`/`destination` dataref names and a `scale`, applied to every configured engine slot each frame. Source and destination arrays must exist in the catalog or `datarefs`. |
| `datarefs` | Typed simulator-owned seeds, applied after common simulator defaults and before the plugin starts. |
| `cold`, `running` | Typed simulator overrides applied after plugin enable, before initial plane/airport/scenery messages, selected by `--flight-start`. |

For example, a two-engine 12 V fixture can set `engine_count` to `2`, `avionics` to `{ "bus_index": 1, "nominal_volts": 12.0 }`, and put this in `running`:

```json
{
  "sim/cockpit2/engine/actuators/ignition_key": {
    "type": "int_array", "value": [3, 3]
  }
}
```

Seed types are `int`, `float`, `double`, `int_array`, `float_array`, and `bytes` (text). Catalog types and read-only metadata are retained; array seeds replace the supplied prefix while retaining the catalog's remaining slots. These overrides target `sim/` datarefs so they cannot preempt a plugin's own accessor registrations. Unknown profile fields, invalid values and catalog type conflicts are errors. Every run saves the resolved profile as `aircraft-profile.json` beside its log and records its name in `Log.txt`.

Use `--help` for the current option list. `--clock fast` advances fixed simulation steps as quickly as the host can run; `--clock real` paces them to wall time. `--dt` controls the frame interval and defaults to 1/60 second. Fast clock does not make the plugin see an accelerated simulation-rate setting. The default parked position, heading and magnetic variation are zero; set `--lat`, `--lon`, `--elev`, `--heading` and `--magvar` for your fixture.

Each run writes `Log.txt`, `commands.tsv`, missing-dataref diagnostics, script output, and an `xplane/` system tree beneath `--out`. Choose a fresh output directory when retaining evidence from several runs.

**Script checks**

Pass a text script with `--script FILE`. Each line contains one instruction; `#` starts a comment. Dataref array elements use `name[index]`.

```text
mark start
wait 0.25
set example/value 2
expect example/value 1.99 2.01
cmd example/increment
wait_until example/count >= 1 2
trace samples.tsv 0.1 example/value example/count
wait 1
trace_off
dump final.tsv example/value example/count
```

| Instruction | Purpose |
|---|---|
| `wait seconds` | Advance simulation time. |
| `wait_until ref comparison value timeout` | Wait for a numeric condition with a bound. |
| `set ref value`, `sets ref text` | Write numeric or byte datarefs. |
| `get ref`, `dump file ref...` | Read values to the console or a TSV file. |
| `cmd name`, `cmd_begin name`, `cmd_end name` | Issue command phases. |
| `expect ref minimum maximum` | Record a failed assertion outside the band. |
| `trace file interval ref...`, `trace_off` | Record periodic TSV samples. |
| `mark label`, `echo text` | Add timing or explanatory output. |

For a plugin with a request/response debug channel, provide both `--debug-request DATAREF` and `--debug-result DATAREF` to enable the optional `dbg text` script instruction. No plugin-specific debug namespace is assumed.

Script assertions continue collecting evidence after a failure. Exit codes are 0 for success, 1 for usage/input errors, 2 for caught plugin lifecycle/callback faults, and 3 for script failures. A nonzero exit status must be checked; ordinary plugin log text does not classify a fault. Serve selftests also return failure when their checks fail.

**Loopback HTTP subset**

Serve mode listens on `127.0.0.1` and supports:

- `GET /api/v3/datarefs` and `GET /api/v3/commands`, with name filtering;
- `GET` or `PATCH /api/v3/datarefs/{id}/value`;
- `POST /api/v3/command/{id}/activate` with a finite `duration`;
- `POST /xphost/stop` for graceful host shutdown.

Discover numeric IDs for each run. Byte values use base64 in the API. All dataref value access and commands are marshalled to the simulation thread; metadata uses the store lock. Queued work that times out before execution is cancelled; work already executing may finish. There is no `/flight` endpoint, aircraft reload, or full Web API implementation.

**Model boundaries**

The host supplies flat terrain, ISA-style weather, a parked pose, limited derived simulator values, and optional nav records. It can execute plugin callbacks and retain fake window/menu/object state. Draw callbacks and actual sound are not implemented; a hidden legacy OpenGL context only makes basic incidental GL calls possible. It models one loaded plugin, and support for an SDK function does not imply complete simulator semantics.

Window focus ownership is retained and cleared on destruction; focus callbacks
and keyboard dispatch are not simulated. Camera registration is retained for
lifecycle bookkeeping, but camera callbacks, camera poses and view changes are
not simulated.

Simulator-owned datarefs may be writable in the fake store even when their metadata is read-only in X-Plane. Do not use those writes as evidence that the same operation is supported by the real simulator. Tests should record their initial state and make explicit assertions about the behavior under examination. Command lookup can synthesize unknown names, so an unknown command may succeed as a no-op; assert observable effects rather than using this host to prove a command exists in X-Plane.

Optional PE inspection tools use `pefile`:

```powershell
python -m pip install -r tools/xphost/tools/requirements.txt
python tools/xphost/tools/check_exports.py C:/Tests/MyPlugin/win.xpl tools/xphost/build/XPLM_64.dll tools/xphost/build/exports.txt
```

Import/export coverage only checks symbol availability. Validate callback semantics with the synthetic tests and validate aircraft behavior in X-Plane separately.
