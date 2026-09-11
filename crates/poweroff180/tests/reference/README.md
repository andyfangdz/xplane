# Frozen v7 reference

`guidance.hpp` and `parameters.generated.hpp` are the accepted September 9,
2026 C++ controller. They are retained solely to explain and regenerate the
cross-language replay fixtures. Production builds use Rust and never compile
or load these files.

Compile `generate.cpp` with a C++17 compiler, then invoke:

```text
generate <native-effective.ini> <trace.csv> <output.bin>
```

Compress the binary with gzip using a zero timestamp. Each 216-byte,
little-endian record contains the 18 f32 snapshot fields selected in the
generator, followed by 18 f64 controller outputs. The Rust test reconstructs
the protocol sample and checks every output on every frame. The per-field
tolerances in [replay.rs](../replay.rs) allow 0.001 degree for guidance angles,
0.01 foot for predicted distances, 0.001 ft/s or ft/s² for speed/acceleration,
and 0.00001 for control ratios. Phases, reasons, event timestamps and step counts
must match exactly. Each run prints the maximum error for every output.

The fixtures retain their original rounded conversions. Production code uses
standard `uom` units; the fixtures are not rewritten to match its rounding.

The seven fixtures contain 88,586 frames from the accepted
`XPT_LOWER_FLARE_O1_20260909` campaign. `../fixtures/manifest.json` records source
trace hashes and fixture hashes. No original flight trace is needed to run
`cargo test -p poweroff180`.
