# Repository import verification

Imported on 10 September 2026 from the completed local Shuttle HUD work.

- The C++ runtime, three headers, three C++ test suites, HUD optics and Lua
  validation control laws match the original source byte for byte.
- The portable Zig/XPSDK430 build passes all three C++ suites with compiler
  warnings treated as errors. The existing four-member Rust workspace still
  resolves correctly with the C++ plugin excluded from Cargo's wildcard.
- A fresh derivative generated in a separate scratch installation matches the
  tested aircraft's ACF, cockpit mesh, transparent HUD-glass mesh, panel atlas,
  optics, runway table and archived release binary byte for byte. The installer
  dry run writes nothing; a second install refuses the existing destination.
  The derivative contains only the glider ACF and no validation controller.
- Regenerating the ball/bar overlay reproduces every distributed scenery file
  byte for byte, including both DSF tiles.
- All 277 local documentation references resolve, all 31 PNG files decode, and all six
  current flight analyses reproduce exactly from the included traces/readbacks.
  The earlier failed rebound checks remain failed.

The original native-tested binary and its build/source snapshot are retained.
The repository rebuild has a different binary hash; it is not represented as
the exact binary flown during the original native tests. No additional simulator
flight was required for this source-preserving import. The live X-Plane aircraft
and complete local development archive remain unchanged.

Build and installer parameters and repository documentation are the import's
functional changes. Original entry points remain in `original-source` for
comparison. Archived experiment scripts still describe their original local
environment and are not a portable flight-test runner.
