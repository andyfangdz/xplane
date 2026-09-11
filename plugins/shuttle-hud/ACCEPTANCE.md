# Shuttle HUD acceptance — Rust release 144

Release 144 corrects the scenery projection, configured runway width and runway length, including the 542 m displaced threshold at KEDW 22L. Native cockpit/full-screen checks cover both runway directions at Edwards and Kennedy, close/offset/banked/off-axis views, declutter, power/enable transitions, view restoration and a short native glide. The [alignment report](../../docs/shuttle-hud/runway-alignment/README.md) records the exact accepted binary and installation audit. All 39 workspace tests pass, with one existing local-scenery database test ignored; plugin Clippy and the release build pass. Flight guidance and aircraft geometry are unchanged.

## Rust 143 migration and landing baseline

Release 143 ports the complete release-142 native HUD to Rust and the shared
repository SDK infrastructure. It preserves the landing equations, original
vector lettering, symbology, optics, aircraft geometry and control laws.

The Rust workspace has 37 passing tests and one existing ignored local-scenery database
test. Clippy with warnings denied and the release build pass. C++ comparison
fixtures cover 3,053 recorded frames, the path joins and all segments/clipping in
24 representative display scenes. All 45 existing dataref names/types/writability
and all eight command names remain compatible.

Native display cards, chute ownership, SDK disable/re-enable, mismatch/reload,
full-screen/cockpit restoration and final installed-aircraft checks passed.
The dedicated simulator process exited normally; the test aircraft was retired.

| Flight / build | Mass (lb) | Touchdown KEAS | Along runway (m) | Sink (ft/s) | Max native AGL (m) | Result |
|---|---:|---:|---:|---:|---:|---|
| 302 / Rust 143 | 226040 | 200.851 | 804.234 | 2.305 | 0.680006 | 9/9 — pass |
| 303 / Rust 143 | 184000 | 197.796 | 655.129 | 1.143 | 0.755262 | 8/9 — rebound limit missed |

The light run remains a failure against the fixed 0.750 m native-AGL rebound
limit; C++ 142's comparable run reached 0.755323 m. No physics, validation pilot,
threshold or landing calibration was changed to obtain a migration pass.
Other established limits and display approximations remain those documented in
the plugin README and release-142 report. Native-aircraft AGL is not wheel clearance.

Read the [migration report](../../docs/shuttle-hud/rust-port/README.md),
[native gallery](../../docs/shuttle-hud/rust-port/GALLERY.md) and
[predeclared contract](../../docs/shuttle-hud/rust-port/ACCEPTANCE.md) for the
evidence, remaining limitations, setup failures and restoration procedure.
The [release-142 report](../../docs/shuttle-hud/reports/shuttle-symbology-20260910/README.md)
preserves the earlier six-flight ledger and symbology decisions.
