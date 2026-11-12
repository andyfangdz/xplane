# C182 build and packaging tools

Public Rust orchestration for a separately configured local aircraft workspace.
The public repository contains the independently authored core models, RPN VM,
egui UI and renderer, these tools, and reference-measurement tools. It does not contain recovered
aircraft-specific source, vendor assets, binary/capture fixtures, or live state.

```powershell
$env:C182_WORKSPACE = (Resolve-Path ../xplane-private).Path
cargo run -p c182-tools --release -- status
cargo run -p c182-tools --release -- build
cargo run -p c182-tools --release -- test
cargo run -p c182-tools --release -- package
cargo run -p c182-tools --release -- verify
```

The configured workspace supplies `package.toml`, ignored `c182.local.toml`, the
native source/build manifest, maintained visual assembly hooks, and private local
inputs. Relative configuration paths are resolved against that workspace. Every
Cargo and Python child receives absolute `C182_PYTHON` (`paths.python`) and
`C182_LEGACY_STUDY` (`paths.legacy_study`), so build scripts use the configured
venv and study rather than `python` from `PATH`; `status` prints both. Build
and package outputs stay under its `out/` directory. Normal assembly preserves
existing output-package state; `package --fresh-state` resets simulation state
while retaining GPS setup/bezel choices.

`install [--root PATH]` is the only command that writes the simulator. It
refuses while an X-Plane process runs, and unless `out/packages` verifies, holds
the current `out/cargo` build and was packaged from the build inputs both
checkouts hold now (commits that touch only other paths do not matter). It
warns when no acceptance record covers the plugin. It backs up every file it
will replace to
`out/install-backups/<utc>/`, copies the whole package into the aircraft root
(`[install] root` in `c182.local.toml`, or `--root`), verifies every installed
file against the package manifest, and writes `installed.json` (plugin and
package-manifest SHA-256, both input digests and HEADs, UTC) into each aircraft
folder and the root's entry in `out/installed.json`. A failed copy restores the
backup. Simulator `source-state` folders are live aircraft state and are never
written. `rollback [--root PATH]` restores the newest backup of that root not
yet rolled back and records it; `installed [--root PATH]` is read-only and
reports drift between the aircraft folders, their records and `out/packages`.

`clean [--days N] [--cargo] [--apply]` lists, and with `--apply` deletes, leaked
oracle test temp folders, `out/staging/previous-*` sets beyond the newest two,
top-level folders that hold no file, and marked headless case copies, stray
Cargo target folders or abandoned staging assemblies older than N days
(default 3). `--cargo` adds `out/cargo/debug`, the development build cache. It
never enters `out/packages`, the rest of `out/cargo`, `out/acceptance` or
`out/install-backups`, leaves loose files directly under `out/` alone, keeps any
tree holding an acceptance `manifest.json`, and removes links without following
them.

Asset-free Rust tests work directly in this public repository:

```powershell
cargo test -p c182-core -p xplane-rpn -p xplane-gauge-xml -p xplane-plugin -p c182-ui -p xplane-egui
python -m unittest discover -s tools/c182-fidelity -v
```

The configured `test` command additionally runs private Rust contracts and
comparisons with archived C++ fixtures. The Python helpers provide path resolution
and invoke the visual assembly hooks. The shared `xplane-plugin` crate supplies
SDK entry points, lifecycle, flight-loop dispatch and registration ownership.
The private provider supplies the aircraft, callback policy and package contents.
