# xphost

A Windows x64 headless host for testing X-Plane plugins. It loads a real `win.xpl` against a local replacement `XPLM_64.dll`, runs the plugin lifecycle and flight-loop callbacks, and exposes a typed dataref store, commands, scripted checks, and a loopback HTTP API.

Use it for plugin logic, lifecycle, dataref/command contracts, and transport regression tests. It does not run X-Plane's flight model, renderer, audio engine, scenery, or other plugins. Passing here does not establish flight behavior or visual/audio correctness in X-Plane.

The source and synthetic test plugin in this directory are self-contained. Supply the X-Plane SDK headers separately; SDK headers, aircraft packages, vendor binaries, nav databases, and captured aircraft state are not bundled.

**Build on Windows**

Requirements are an x64 MSVC C++17 toolchain with the Windows SDK, external X-Plane SDK headers exposing XPLM410, and Python 3.10+ for the tests. Visual Studio Build Tools with the C++ workload is sufficient.

From the repository root:

```powershell
powershell -NoProfile -File tools/xphost/build.ps1 -Sdk C:/SDK/XPSDK
```

`-Sdk` accepts either the SDK root or its `CHeaders/XPLM` directory. Alternatively set `XPLM_SDK`. The build uses an active x64 MSVC environment or discovers Visual Studio through `vswhere`. `build.cmd` forwards the same parameters to the PowerShell entry point.

Output stays in ignored `tools/xphost/build/`:

- `xphost.exe` and `XPLM_64.dll`, which must remain side by side;
- an original, minimal plugin fixture under `fixture/` for standalone tests.

No X-Plane installation or aircraft package is needed to build or run those tests.

**Run the standalone tests**

```powershell
python tools/xphost/tools/transport_regression.py --out tools/xphost/build/test-results
```

The tests exercise the fixture's lifecycle, script assertions, scalar/array/byte datarefs, command propagation, HTTP connections, and graceful shutdown. The `serve --selftest` option targets this bundled test fixture, not arbitrary third-party plugins. They launch and stop only their own host process. Test results and logs remain under the supplied output directory.

**Run a plugin**

Use a disposable copy of the package under test. A plugin runs ordinary native code in this process and may write package state; xphost is not a sandbox.

```powershell
$hostExe = './tools/xphost/build/xphost.exe'
& $hostExe --pkg C:/Tests/MyAircraft --acf MyAircraft.acf --xpl C:/Tests/MyAircraft/plugins/MyPlugin/64/win.xpl --clock fast --seconds 30 --out ./tools/xphost/out/run
& $hostExe serve --pkg C:/Tests/MyAircraft --acf MyAircraft.acf --xpl C:/Tests/MyAircraft/plugins/MyPlugin/64/win.xpl --port 8199 --out ./tools/xphost/out/server
```

`--pkg` supplies the package root and `--acf` names an existing aircraft file relative to it. The host exposes that path to the plugin; it does not parse or simulate the ACF. `--xpl` selects the plugin explicitly. Automatic discovery is suitable only when the package contains one `plugins/*/64/win.xpl` candidate.

Simulator dataref and command catalogs, and optional navigation data, are external inputs. Use `--datarefs PATH`, `--commands PATH`, and `--navdat PATH` to supply them, or `none` to omit them. These inputs default to `none`. A small built-in simulator state remains available without catalogs; no navigation records are loaded by default. Navigation input is also copied into the fake system tree under the output directory for plugins that read it directly.

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

Simulator-owned datarefs may be writable in the fake store even when their metadata is read-only in X-Plane. Do not use those writes as evidence that the same operation is supported by the real simulator. Tests should record their initial state and make explicit assertions about the behavior under examination. Command lookup can synthesize unknown names, so an unknown command may succeed as a no-op; assert observable effects rather than using this host to prove a command exists in X-Plane.

Optional PE inspection tools use `pefile`:

```powershell
python -m pip install -r tools/xphost/tools/requirements.txt
python tools/xphost/tools/check_exports.py C:/Tests/MyPlugin/win.xpl tools/xphost/build/XPLM_64.dll tools/xphost/build/exports.txt
```

Import/export coverage only checks symbol availability. Validate callback semantics with the synthetic tests and validate aircraft behavior in X-Plane separately.
