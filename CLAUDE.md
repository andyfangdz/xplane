# X-Plane native plugins

- Aircraft testing and mods use the `test-xplane-aircraft` and
  `develop-xplane-aircraft-mod` skills. Flight-test harness rules are in
  `tools/flight-test-harness/AGENTS.md`.
- Scenery development has its own checkout at `../xplane-scenery`, branch
  `scenery/nj-airports`. Use that checkout's guidance and scenery skill.
  Its ignored data is shared from this checkout's `build/nj-airports`.
- Public C182 code lives in `crates/c182-core`, `crates/c182-ui`,
  `crates/xplane-rpn`, `crates/xplane-numerics`, `crates/xplane-egui`, `tools/c182` and
  `tools/c182-fidelity`. Keep independently authored models, UI and reusable
  tooling here. Recovered source, vendor assets, fixtures and aircraft-specific
  native adapters belong in the private sibling `../xplane-private`.
  Old `build/c182-*` paths are compatibility junctions to archived inputs.
- X-Plane 12 is installed at `D:\X-Plane 12` (Web API port 8086; scenery trials
  use 8091).
- This repository is public on GitHub. Do not push. Keep payware-derived files,
  reference photos, and session paths out of tracked files.
- Use PowerShell 7. Write multi-line Python to a script file instead of nested
  shell quoting. Keep generated products and private inputs under ignored paths.
