# Working on this harness

- Start with README.md. The entry point is Run-XPlaneTest.ps1; never reproduce its installation moves in ad hoc scripts.
- ../../crates/poweroff180/parameters.csv is the parameter authority. Cargo and xpt/config.py consume it directly. The frozen C++ files under the Rust replay tests are reference evidence, not active code.
- Flight guidance belongs in ../../crates/poweroff180/src/guidance.rs. Python may configure, start, cancel, supervise and analyze; it must not become a second airborne guidance writer.
- Keep the native watchdog, exact-PID checks, atomic scenery moves, protected-installation guard, and verified recovery intact.
- Use scripts/Test-Harness.ps1 before actual flights. Verify changes to recovery with the explicit failure probes described in README.md.
- Preserve every flight and error. A landing miss is not a runner failure, and a successful runner is not proof of a successful landing.
- Native algorithm changes require actual simulator validation. Do not claim repeatability from one favorable flight.
- Source adapters currently support TorqueSim SR20 at the documented loading. Add a versioned adapter and its readiness/readback checks before claiming another aircraft is supported.
