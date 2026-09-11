# Rust migration evidence

These release records pin the source at migration commit `0445689`; run the
source-byte validator from that checkout. The later
[standard-unit validation](../standard-units-validation.md) has its own source
manifest and flight evidence.

`release/` contains the final three sessions using Rust guidance and attitude
control. The HUD session additionally loads the Rust display for three recorded
flights; the other eleven flights exercise guidance without video capture.
Their combined release set has two flights in each of seven winds. Each session
explicitly isolated `TelemFFB-XPP` and restored it afterward. Source and build
artifact hashes must agree across these sessions.

Each release card also retains `cpp-guidance-replay.json`: the original C++ v7
generator was fed that Rust flight's exact recorded inputs, and all 18 guidance
outputs per frame were compared by their protocol float bytes. The record pins
the input trace, configuration, reference source and generator hashes. All frames
matched. The saved-evidence validator checks those records against the retained
traces and frozen C++ source; rerunning the generator is optional and requires a
C++17 compiler as described in the reference directory.

After compiling that generator, rerun the comparison from the repository root:

```powershell
& .\tools\flight-test-harness\.venv\Scripts\python.exe .\docs\poweroff180\verify_cpp_commands.py --generator '<compiled generator executable>'
```

`diagnostics/` retains the initial Rust-guidance attempt and the subsequent
original-C++ comparison with the external hardware writer still present. Failed
landings stay failed. The cancelled follow-up is an aborted measurement.

`recovery/` contains deliberate preparation, launch, launcher-exit and native
watchdog failures. These are recovery tests, not landing-performance trials.
The final all-Rust watchdog readback is
[native-terminal-safety.json](recovery/XPT_RUST_ALL_WATCHDOG_20260911/cards/calm-01/native-terminal-safety.json).

`hud/` contains three native Rust-HUD screenshots and the associated capture
metadata. Full-resolution PNGs were copied only after their original screenshot
files finished writing. `evidence-sha256.json` covers the retained evidence bytes.

From the repository root, after bootstrapping the harness environment:

```powershell
& .\tools\flight-test-harness\.venv\Scripts\python.exe .\docs\poweroff180\validate_evidence.py
```

This verifies evidence hashes, checks current source bytes against all release
manifests, decompresses and reassesses the traces, and computes paired trajectory
comparisons. It requires 14 passing release flights and two traces for each wind.
Recorded binary hashes are compared across sessions; a fresh clone builds its
own binaries using the workspace and records new hashes for new tests.

No raw AVI, WAV, simulator preference contents or installed aircraft files are
included here. Those originals remain in the named `Output/performance-tests`
directories on the test installation.

The [overview chart](release-overview.png) can be regenerated with
`docs/poweroff180/plot_release.py`. It displays the individual landing results
and the unchanged distance, airspeed and physical-descent limits.
