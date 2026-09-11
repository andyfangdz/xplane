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
the protocol sample and checks every output on every frame, with an absolute
tolerance of 1e-8. The observed maximum difference on Windows is 3.19e-12.

The seven fixtures contain 88,586 frames from the accepted
`XPT_LOWER_FLARE_O1_20260909` campaign. `../fixtures/manifest.json` records source
trace hashes and fixture hashes. No original flight trace is needed to run
`cargo test -p poweroff180`.
