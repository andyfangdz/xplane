# xplane-numerics

Data-independent numerical compatibility primitives for native simulation and
Rust migrations. This crate has no aircraft data, X-Plane SDK dependency or
external Rust dependencies. It is MIT licensed and can be tested independently.

- `fp`: first/second operand selection for NaNs and signed zeros, bounds-first
  clamping, truncation, platform C math and runtime scalar arithmetic.
- `interpolation`: validated, clamped linear tables and individual segments.
- `rotation`: Hamilton quaternions, ZYX Euler angles, row-major matrices,
  body/Euler angular rates, normalization and derivative integration.
- `integration`: trapezoidal steps with caller-owned derivative history.
- `vector`: three-component addition, dot products and matrix transforms.
- `storage`: bounds-checked native-endian byte records without pointer casts.
- `stream` (Windows/MSVC): classic-locale numeric and text extraction with native
  prefix consumption, stream flags, errno and floating-point status.

Use ordinary Rust arithmetic and `f32`/`f64` methods by default. Use `fp` when
matching a native contract requires runtime rounding, exception flags, signed-zero
or NaN operand selection, or the platform math implementation. Aircraft models
and the RPN interpreter share this boundary instead of declaring local CRT math
imports. A bounded finite integer check can use Rust's `floor()` directly; that
does not imply arbitrary CRT calls are interchangeable with Rust intrinsics.
`stream` separately retains native parsing, locale and error behavior.

The APIs keep arithmetic grouping and coordinate conventions explicit.
Callers supply aircraft constants, tables, limits, restart rules, singularity
guards and record schemas.

```rust
use xplane_numerics::{interpolation, rotation, vector};

assert_eq!(interpolation::linear(&[[0.0, 10.0], [2.0, 30.0]], 0.5), Ok(15.0));
let q = rotation::from_euler_zyx(0.0, 0.0, std::f64::consts::FRAC_PI_2);
let v = vector::transform(rotation::matrix(q), [1.0, 0.0, 0.0]);
assert!(v[0].abs() < 1e-15 && (v[1] - 1.0).abs() < 1e-15);
```

On x86-64, `fp::{add, sub, mul, div}`, their `_f32` counterparts, and
`fp::{narrow_f32, from_i64}` use SSE2 at runtime and observe the
caller's MXCSR rounding direction. Other architectures use ordinary Rust
arithmetic without that dynamic-rounding guarantee.
`fp::widen_f64` issues CVTSS2SD: finite conversions are exact, while DAZ,
signaling NaN quieting and exception status follow the caller's MXCSR.
`fp::max_second_f32` also issues MAXSS on x86-64, preserving its second-operand
selection, denormal handling and floating-point status behavior.
`fp::{add4_f32, mul4_f32}` issue packed ADDPS/MULPS with the first argument as
destination, including NaN operand selection and status from all four lanes.
Vector, rotation and integration expressions use ordinary Rust operations
with explicit grouping.
C math calls use the platform runtime; transcendental results can vary across
platforms. C math wrappers remain runtime calls even for constant inputs under
link-time optimization; compiler folding through a different math implementation
can produce different bits from the host runtime, especially for binary32 trig.
`fp::{roundf, truncf, floorf, ceilf}` call the binary32 CRT entries directly,
retaining signed-zero behavior without widening to binary64. Their integral
rounding rules are independent of the caller's rounding direction.
The truncation helpers provide native 32-bit and 64-bit integer-indefinite
results for invalid conversions. The host owns the floating-point environment, exception masks and
restoration. Byte records use native endianness and are not a portable file format.

```powershell
cargo test -p xplane-numerics
```

Tests use synthetic tables, analytic rotation identities, integration checks,
byte patterns, NaNs and signed zeros. On x86-64 they exercise all four SSE
rounding directions and restore the original environment. No private checkout
is required.
