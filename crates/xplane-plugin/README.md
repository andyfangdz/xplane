# xplane-plugin

Shared infrastructure for the native plugins in this workspace. The crate
keeps common XPLM operations behind small Rust APIs while leaving each
plugin's behavior, callbacks, and rendering architecture independent.

It currently provides:

- typed scalar, array, and string dataref access, plus lazy lookup caching that
  retries missing providers and can be cleared on reload;
- owned command, flight-loop, modern-window, widget-window, and Plugins-menu
  registration and cleanup;
- drawing, coordinate conversion, plugin metadata, debug logging, feature, and
  path helpers;
- a shared adapter for X-Plane's five required plugin entry points, including
  lifecycle support for aircraft and plugins with reentrant callbacks; and
- thread-local plugin-state storage for SDK and graphics handles.

`lifecycle::Lifecycle` tracks active and suspended instances on its creating
thread. A `lifecycle::Host` creates and destroys each instance, handles SDK
messages and supplies its own handle and error types. When an instance is
disabled or replaced during one of its callbacks, the lifecycle suspends it and
waits for the callback to return before destroying it. The host can schedule a
later drain while an instance is still busy in an SDK callback outside
lifecycle message dispatch.

`ManagedFlightLoop` owns a before or after physics SDK registration and a
`FlightLoopHandler`. The loop begins dormant. The handler receives frame timing,
returns the next interval, and receives errors with the original payload. A stop
requested during a callback takes precedence over its returned interval. The
owner can close the registration after the callback returns.
`SimulationTimingSampler` reads standard X-Plane timing datarefs for providers
that need simulator time alongside the SDK callback timing.

`registrations::Registrations` owns command groups, datarefs, menus and an after
physics loop. The provider supplies handlers and chooses registration and teardown
order. Registration trampolines retain callbacks through each invocation so a
handler can release its registration during reentry.

The optional Windows `opengl` feature provides a borrowed `DrawContext` for both
HUDs. Its single unsafe entry point belongs at the native draw callback boundary;
the compatibility context must remain current until drawing returns. Safe methods
scope attributes, matrices, and primitives with private guards, reject full GL
stacks without popping, and restore state on early return or unwind. The context
cannot escape the callback or cross threads. A primitive exposes only vertices
and texture coordinates, preventing nested primitives and matrix changes through
the drawing API. XPLM graphics-state selection remains separate.

`cargo test -p xplane-plugin --features opengl` checks pixel output, stack limits,
reentrancy, missing contexts, and unwind cleanup using a real Windows OpenGL
context attached to an invisible window. Compile-fail checks cover context lifetimes
and borrowing during primitives. No simulator or visible window is needed.

The crate is intentionally local to this workspace and does not attempt to
wrap the entire X-Plane SDK. Add an abstraction only after at least two plugins
need the same lifecycle and safety rules.
