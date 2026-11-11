# c182-core

Independently authored, asset-free Rust components used by the private C182
conversion: the project's calibration curves (`calibration`: airspeed
indication from the Cessna 182T POH, fuel flow, thrust and stall lift) and the
G1000 NAV III standby-battery model (`standby`), written from the 182T POH; the
source aircraft has no standby-pack model. The crate has no SDK, vendor-binary
or reference-capture dependency; the private checkout's differential tests
compare both models with the archived reference.

Recovered source-aircraft behaviour (panel cadence, the state-log layout,
fixed-point positions, the utility random streams, AoA indicator bands and radio
power targets) is private and lives with the aircraft integration in
`xplane-private`.

Numerical compatibility helpers live in the separate public
[`xplane-numerics`](../xplane-numerics/README.md) crate. This crate uses its strict
floating-point operations. Callers needing numerical helpers depend on
`xplane-numerics` directly. Its synthetic tests run without aircraft data or the
private checkout.

Hardware/SDK ownership, UI, rendering, engine implementation and the complete
aircraft package live outside this crate. Numerical defaults are project
behavior, not a claim of flight-model certification or real-world planning
accuracy.

```powershell
cargo test -p c182-core
```

Private differential tests against captured/recovered reference implementations
remain in `xplane-private`; no proprietary fixtures are included here.
