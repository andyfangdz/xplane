# c182-core

Independently authored, asset-free Rust components used by the private C182
conversion: lossless persistence encoding, panel timing, deterministic random
streams, calibration and AoA helpers, radio targeting, coordinate math, and the
NAV III standby-battery model. The crate has no SDK, vendor-binary or reference
capture dependency. Public unit tests use synthetic inputs.

`native` contains the small compatibility primitives needed when moving native
code into Rust: explicit floating-point operand selection and runtime arithmetic,
linear interpolation, quaternion rotations and rate conversions, trapezoidal
steps, three-component vector operations, and bounds-checked byte records.
These helpers keep arithmetic grouping and conventions visible to callers.
Aircraft constants, policies, record schemas, adapters and reference fixtures
remain outside the public crate.

On x86-64, `native::fp::{add, sub, mul, div}` use SSE2 at runtime and observe
the caller's MXCSR rounding direction. The existing strict arithmetic helper
uses the same implementation. Other architectures use ordinary Rust arithmetic
without that dynamic-rounding guarantee. Vector, rotation and integration
expressions use ordinary Rust operations with explicit grouping. C math calls
use the platform runtime; transcendental results can vary across platforms.
The host owns the floating-point environment, exception masks and restoration.
Byte records use native endianness and are not a portable file format.

```rust
use c182_core::native::{interpolation, rotation, vector};

assert_eq!(interpolation::linear(&[[0.0, 10.0], [2.0, 30.0]], 0.5), Ok(15.0));
let q = rotation::from_euler_zyx(0.0, 0.0, std::f64::consts::FRAC_PI_2);
let v = vector::transform(rotation::matrix(q), [1.0, 0.0, 0.0]);
assert!(v[0].abs() < 1e-15 && (v[1] - 1.0).abs() < 1e-15);
```

The native compatibility tests use synthetic tables, analytic rotation
identities, integration checks, byte patterns, NaNs and signed zeros. On
x86-64 they exercise all four SSE rounding directions and restore the original
environment. No private checkout is required to run them.

The crate also exports a flat C ABI with its public
[`c182_core.hpp`](include/c182_core.hpp) header for existing native adapters. Hardware/SDK
ownership, UI, rendering, engine implementation and the complete aircraft package
live outside this crate. Numerical defaults are project behavior, not a claim of
flight-model certification or real-world planning accuracy.

C ABI functions use the `c182_*` prefix, independent of the implementation
language. Rebuild native callers with the matching public header when upgrading
from the earlier `c182_rust_*` names; the old symbols are not aliases.

```powershell
cargo test -p c182-core
```

Private differential tests against captured/recovered reference implementations
remain in `xplane-private`; no proprietary fixtures are included here.
