# X-Plane Plugin SDK 4.4.0-r2

This workspace crate supplies the XPLM and XPWidgets C headers, Rust declarations
and link libraries for 64-bit plugins. Both aircraft and the headless host use
the same SDK. Building a plugin needs neither a separate SDK download nor
libclang. The simulator supplies the runtime.

The SDK inputs come from Laminar's [4.4.0-r2 tag](https://github.com/X-Plane/xpsdk/releases/tag/xpsdk_4.4.0-r2),
commit `ed88aed734106466436ba9efd9a87332fbd9808e`. `SDK/` contains the
upstream `CHeaders/XPLM`, `CHeaders/Widgets`, Windows import libraries and
[SDK license](SDK/license.txt). Each Mac framework contains the upstream
`Versions/C` universal binary under its unversioned link name. Linux plugins
resolve SDK imports when X-Plane loads them.

To regenerate `src/bindings.rs`, install `bindgen-cli 0.72.1` and LLVM with
libclang and its C headers, then run `python crates/xplane-sdk-sys/generate.py`.
The generator enables the cumulative SDK definitions through `XPLM440`.
Build scripts of direct Cargo dependents can locate the headers via
`DEP_XPLANE_SDK_ROOT`.

Plugins that directly import a new 4.4 function require X-Plane 12.4.4 or later.
Use `XPLMFindSymbol` for optional entry points when supporting older hosts.
