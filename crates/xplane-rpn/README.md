# xplane-rpn

An independently authored, embeddable interpreter for the FSX/Prepar3D-style
Reverse Polish Notation (RPN) calculator language used in XML gauges and model
animation/visibility expressions. RPN places operators after their operands:
`9 4 - 2 /` computes `(9 - 4) / 2`.

The crate executes classified instructions through a typed Rust `Host` trait. It runs independently of Prepar3D or Microsoft Flight
Simulator, allowing a host such as an X-Plane plugin to reuse calculator logic
with its own variable store and simulator bindings. It originated in the C182
study but contains no aircraft programs, vendor assets, or simulator dependencies.

## Compatibility and scope

This implements the observed `p3d-v5-oracle-v1` compatibility subset, carried
forward from the study's Lua interpreter. It is not a complete implementation
of every FSX, Prepar3D, or modern MSFS calculator dialect. Supported instructions
include arithmetic and comparisons, conditional jumps, stack operations, 50
registers, number/boolean/byte-string values, and host reads, writes, and events.

Compatibility includes behavior that a generic postfix calculator or direct
translation to another language can change:

| Operation | Retained behavior |
| --- | --- |
| `int`, `flr`, `near` | Truncation, floor, and rounding halfway away from zero, respectively |
| `div` | Signed 32-bit truncation of the quotient |
| Division by zero | Returns positive zero |
| Empty-stack reads and uninitialized registers | Return positive zero |
| Numeric pushes | Normalize NaN to positive zero; retain infinities |
| Consecutive programs | Share the remaining stack and registers until explicitly reset |
| Local writes and external effects | Invoke host callbacks in instruction order |

Scalar math uses the shared `xplane-numerics::fp` boundary; this crate declares
no CRT imports of its own. CRT math is retained for compatibility with the native
host. Evaluation uses
the caller's floating-point environment; the host owns MXCSR setup and
restoration. Supported behavior and synthetic regression checks do not establish
whole-aircraft equivalence or compatibility with untested simulator versions.

## Embedding contract

The primary API is `Vm`, `Value`, `Instruction`, `Program`, `ExecutionContext`
and `Host`. The VM owns its instruction constants and seeded context strings;
returned values and context snapshots remain valid after later operations or
VM destruction. Host errors retain their instruction location. Host panics
unwind through ordinary Rust calls when unwinding is enabled.

```rust
use xplane_rpn::{Instruction, Program, Value, Vm};
let code = [Instruction::push(Value::Number(42.0))];
let mut vm = Vm::new(&code, &[Program { first: 0, count: 1 }]).unwrap();
vm.begin(false);
```

The `source` module parses RPN expression bytes at load time into instructions,
variable operands and source locations. It expands macros, resolves conditional
branches and registers, and optionally lowers stack selection (`?`). The host
supplies numeric-literal conversion so parser arithmetic and rounding can remain
compatible with its source environment. Nonstandard control syntax requires
explicit `source::Options`; the default requires balanced blocks and adjacent
else blocks. Source, macro expansion and block nesting are bounded. No XML or
filesystem loader is included, and parsed instructions need not be serialized.

Hosts can also construct `Instruction` and `Program` values directly.
`Vm::new` checks opcode, register,
jump and program bounds and copies string constants. `begin` resets the stack,
register presence and temporary string storage. `set_context` installs an owned
copy of a stack and sparse registers; `context` returns an independent snapshot.
Booleans retain their type in equality, registers and host effects; numeric
operators reject them. `Nil` represents a missing value: reads/pushes reject it,
register seeds clear presence, and dense stack seeds cannot contain it.

`evaluate` executes one program with an explicit step budget. It returns and
consumes one result, retaining the remaining context for the next program. An
empty result is numeric zero. Dropping the Rust value releases the VM normally.
There are no raw VM handles, C descriptors, callback tables, custom allocators,
compatibility features. Optional trusted AOT execution has an explicit unsafe contract.

The `Host` trait receives global instruction indices, which the host maps to
names, namespaces, units and event identifiers. The host owns unit conversion,
local storage, external inputs, event delivery, scheduling and thread ownership.
Event calls distinguish an absent payload from an explicit numeric zero.

Earlier host effects are not rolled back on failure, and context after an error
or panic is unspecified. A host needing transactions must stage effects and
restore or discard the context. The implementation retains private arena-backed
values for stable string storage; only owned Rust values cross the public API.
Platform CRT math calls are contained in `xplane-numerics` for numerical compatibility.

## Tests

Tests use synthetic programs and need no aircraft installation or private
fixtures. They cover numeric compatibility, context continuation/reset, ordered
effects and event payloads, callback fault locations, string ownership, descriptor
validation, and bounded execution.

```powershell
cargo test -p xplane-rpn
cargo test -p xplane-rpn --release
```

## Ahead-of-time Rust generation

`aot::generate` emits Rust basic blocks for instruction shapes. Compile that
source with the embedding application's normal Rust build, then bind its shape
with `Vm::bind_aot`. An opcode or argument mismatch returns no binding; keep
`Vm::evaluate` as the fallback before execution starts. Literals, byte strings
and variable operands stay in the VM's current source tables. They are never
baked into generated code or borrowed from an old VM.

`Vm::evaluate_aot` accepts a binding belonging to that exact VM and a generated
body. The checked `AotFrame` retains the instruction counter, source indices,
budget failures, register/stack limits, string ownership and host callback
points. It rejects wrong instructions, incorrect control flow and premature
completion. Compiled blocks use constant opcode specializations of the same
operation implementation as the interpreter, allowing rustc to eliminate the
opcode dispatch within each block. Conditional and backward jumps use block
control flow; loops remain budget limited. Arithmetic uses the shared runtime
FP helpers to preserve operation ordering and dynamic rounding.

`Vm::evaluate_aot_prevalidated` is the trusted generated-code path. It checks
VM identity once and selects the interpreter before execution for backward
branches or a budget shorter than the source instruction count. Its unsafe
contract requires the exact generated body for the bound shape. Custom bodies
should use the checked API. Runtime faults never trigger a retry.

The fast generator lowers supported contiguous arithmetic to scalar Rust
locals. A pure integer guard checks live stack/literal types and sufficient
existing stack capacity before entering each region. Non-numeric values or
insufficient capacity use a shared interpreter loop over that region, preserving
operation order without duplicating cold stack/type checks in native code. Constants remain
live VM data; intermediates retain source-order runtime arithmetic and NaN
normalization, including FP status effects for discarded values. Registers,
callbacks, branches, CRT operations and other unsupported instructions use the
shared tagged-value implementation. No update rate changes are involved.

Do not retry the interpreter after a compiled execution faults: its earlier
host effects must occur only once. Fallback is a load/binding decision. An
application may also require a source hash or compatibility profile before
binding. The API does not enable executable-memory allocation or a JIT.

The generated synthetic fixture is compiled by `tests/aot.rs`, which compares
native blocks with the interpreter across types, callbacks, failures, budgets,
branches, loops, register contexts and FP modes. Regenerate that fixture with
`cargo run -p xplane-rpn --example generate_aot_fixture`. The `aot_bench` example
measures native blocks against the interpreter with an old/new/new/old sequence
and verifies that every execution retains its external read. These synthetic
checks do not establish whole-aircraft equivalence or simulator CPU time.

`Vm::evaluate_numeric` is a second trusted backend for larger calculators. It
requires an empty stack, numeric or absent registers, numeric constants, a forward-only CFG
with a proved maximum depth of 64, and a full source-length budget. Other inputs
select the interpreter before any callback. Numeric register seeds and successful
numeric continuations retain their original values and presence. Generated stack-height information
removes tagged stack traffic and repeated underflow/type checks while numeric
registers, host operations, comparisons and branches retain their source order.
CASE analysis includes every valid resulting depth; its count is live VM data.

An external string or boolean read materializes the numeric prefix, accepts that
returned value once, and resumes the interpreter strictly after that read with
the original source position and step counter. A host or runtime fault flushes
the prefix and propagates directly. Host panics unwind normally; as with the VM,
context after panic is unspecified. Small calculators can remain on the ordinary
VM: this backend has entry/materialization costs and needs workload measurement.
