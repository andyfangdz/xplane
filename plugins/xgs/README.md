# XGS Rust

An idiomatic Rust reimplementation of hotbso's XGS Landing Speed 3.46
plugin. It preserves the configurable touchdown rating, landing metrics,
translucent result window, VR positioning, landing log, and selectable
automatic hide time.

This crate is based on the behavior and algorithms of the GPL-2.0-licensed
upstream project: <https://github.com/hotbso/xgs/tree/V3.46>.

The Rust plugin has its own X-Plane plugin signature so it can be installed
beside the legacy binary for comparison. On first run it imports the legacy
`Output/preferences/xgs.prf` settings when `xgs-rs.prf` does not yet exist.
After comparison, disable the legacy `xgs/64/win.xpl` to avoid duplicate
touchdown overlays.

This crate is licensed under GPL-2.0-only; see [LICENSE](LICENSE). The rest of
the workspace retains the license stated by each package.
