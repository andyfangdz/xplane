# xplane-rpn

An independently authored, embeddable interpreter for the FSX/Prepar3D-style
Reverse Polish Notation (RPN) calculator language used in XML gauges and model
animation/visibility expressions. RPN places operators after their operands:
`9 4 - 2 /` computes `(9 - 4) / 2`.

The crate executes classified instruction descriptors through a C-compatible
callback interface. It runs independently of Prepar3D or Microsoft Flight
Simulator, allowing a host such as an X-Plane plugin to reuse calculator logic
with its own variable store and simulator bindings. It originated in the C182
study but contains no aircraft programs, vendor assets, or simulator dependencies.

## Compatibility and scope

This implements the observed `p3d-v5-oracle-v1` compatibility subset, carried
forward from the study's Lua interpreter. It is not a complete implementation
of every FSX, Prepar3D, or modern MSFS calculator dialect. Supported instructions
include arithmetic and comparisons, conditional jumps, stack operations, 50
registers, number/byte-string values, and host reads, writes, and events.

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

CRT math is retained for compatibility with the native host. Evaluation uses
the caller's floating-point environment; the host owns MXCSR setup and
restoration. Supported behavior and synthetic regression checks do not establish
whole-aircraft equivalence or compatibility with untested simulator versions.

## Embedding contract

The host compiles/parses source into `RpnInstr` and `RpnProgram` descriptors;
this crate does not parse XML or RPN source text. `rpn_vm_new` validates descriptor
fields and program bounds and copies string constants into VM-owned storage.

Use `rpn_begin` to reset the stack, register presence, and temporary string
storage. Optional `rpn_context_push` and `rpn_context_register` calls seed an
existing context. `rpn_eval` executes a selected program with an explicit step
budget, returns and consumes one result, and retains the remaining context for
the next program. An empty result is numeric zero. `rpn_vm_free` releases the VM.

Every evaluation requires all four `RpnHost` callbacks. Callbacks receive the
global instruction index, which the host maps to variable names, namespaces,
units, and event identifiers. The host owns unit conversion, local storage,
external inputs, event delivery, scheduling, and thread ownership. Key-event
callbacks distinguish an absent payload from an explicit numeric zero.

Failures report a status and instruction location. Earlier host effects are not
rolled back, and the VM context after a failed evaluation is unspecified; a host
requiring transactions must stage effects and restore or discard that context.
The exported functions are unsafe: callers must uphold pointer validity,
exclusive VM access, and documented borrowed-string lifetimes. Inspection
pointers must not be retained across operations that mutate the VM.

The default build uses the standard allocator. `--no-default-features` selects
the CRT allocator/panic integration for native embedding. Current compatibility
targets the Windows x64 CRT/host environment; other platforms require validation.

## Tests

Tests use synthetic programs and need no aircraft installation or private
fixtures. They cover numeric compatibility, context continuation/reset, ordered
effects and event payloads, callback fault locations, string ownership, descriptor
validation, and bounded execution.

```powershell
cargo test -p xplane-rpn
cargo test -p xplane-rpn --release
```
