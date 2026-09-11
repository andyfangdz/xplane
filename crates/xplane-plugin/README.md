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
- a shared adapter for X-Plane's five required plugin entry points; and
- thread-local plugin-state storage for SDK and graphics handles.

The optional Windows `opengl` feature provides `AttributeGuard` and `MatrixGuard`
for compatibility-context drawing. Both HUDs use these to restore their selected
attribute groups, matrix stacks, and previous matrix mode. Construction is unsafe:
the context must remain current on the same thread through drop and nested stack
operations must remain balanced. These guards do not manage XPLM's state cache or
choose a renderer's transforms, fonts, blending, or clipping policy.

The crate is intentionally local to this workspace and does not attempt to
wrap the entire X-Plane SDK. Add an abstraction only after at least two plugins
need the same lifecycle and safety rules.
