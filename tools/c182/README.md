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
while retaining GPS setup/bezel choices. No command installs into X-Plane.

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
