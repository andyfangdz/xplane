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
compatibility features or unsafe public entry points.

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
