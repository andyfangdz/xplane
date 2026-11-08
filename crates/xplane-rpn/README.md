# xplane-rpn

An independently authored XML-style RPN interpreter with a C callback interface
for simulator locals and external variables. It supports number/string values,
register context, bounded execution, and exact callback failure locations.

This is the existing Rust interpreter formerly embedded in the C182 study. It
is reusable without any aircraft, vendor gauge, or compiled expression bundle.
Programs and aircraft-specific host bindings are separate inputs. CRT math is
retained for compatibility with the native host's floating-point environment.

```powershell
cargo test -p xplane-rpn
```
