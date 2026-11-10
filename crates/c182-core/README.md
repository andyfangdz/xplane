# c182-core

Independently authored, asset-free Rust components used by the private C182
conversion: lossless persistence encoding, panel timing, deterministic random
streams, calibration and AoA helpers, radio targeting, coordinate math, and the
NAV III standby-battery model. The crate has no SDK, vendor-binary or reference
capture dependency. Public unit tests use synthetic inputs.

Numerical compatibility helpers live in the separate public
[`xplane-numerics`](../xplane-numerics/README.md) crate. This crate uses its strict
floating-point operations. Callers needing numerical helpers depend on
`xplane-numerics` directly. Its synthetic tests run without aircraft data or the
private checkout.

Callers use the typed Rust modules directly. The retired flat C entry points,
header and static-library target have been removed. Hardware/SDK ownership, UI,
rendering, engine implementation and the complete aircraft package live outside
this crate. Numerical defaults are project behavior, not a claim of
flight-model certification or real-world planning accuracy.

```powershell
cargo test -p c182-core
```

Private differential tests against captured/recovered reference implementations
remain in `xplane-private`; no proprietary fixtures are included here.
