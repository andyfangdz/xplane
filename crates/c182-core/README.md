# c182-core

Independently authored, asset-free Rust components used by the private C182
conversion: lossless persistence encoding, panel timing, deterministic random
streams, calibration and AoA helpers, radio targeting, coordinate math, and the
NAV III standby-battery model. The crate has no SDK, vendor-binary or reference
capture dependency. Public unit tests use synthetic inputs.

The crate also exports a flat C ABI with its public
[`c182_core.hpp`](include/c182_core.hpp) header for existing native adapters. Hardware/SDK
ownership, UI, rendering, engine implementation and the complete aircraft package
live outside this crate. Numerical defaults are project behavior, not a claim of
flight-model certification or real-world planning accuracy.

```powershell
cargo test -p c182-core
```

Private differential tests against captured/recovered reference implementations
remain in `xplane-private`; no proprietary fixtures are included here.
