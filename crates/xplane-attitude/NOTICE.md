# Attitude controller provenance

This Rust crate translates the local GPL-3.0-or-later X-Plane adapter of
ArduPilot's fixed-wing controller. The original adapter retains the angle-to-rate,
airspeed scaling, filtered PID and feed-forward, integrator limits, oscillation
slew limiter, and output slew limit from the upstream control path.

- Upstream: https://github.com/ArduPilot/ardupilot
- Pinned commit: `4fe7ad4fab8c1bf4ade7cbc7ae85a73c81d73e05`
- Relevant upstream components: `AP_RollController`, `AP_FW_Controller`, `AC_PID`,
  `Filter/SlewLimiter`, and `ArduPlane/Attitude.cpp`.
- Local reference: SR20 G6 attitude helper v1.9. Its original binary SHA-256 is
  `2114469978b60dca646c014ec528b84a476b356535edf0257df6f1d0ac597f9e`.

The crate, local adapter changes, and `plugins/poweroff180-attitude` are licensed
under [GPL-3.0-or-later](../../licenses/GPL-3.0-or-later.txt). They are separate
from the MIT guidance and display crates. Frozen C++ source under
`tests/reference` is used only to produce replay evidence; Cargo builds and
simulator deployment use Rust exclusively.
