# xplane-plugin

Shared infrastructure for the native plugins in this workspace. The crate
keeps common XPLM operations behind small Rust APIs while leaving each
plugin's behavior, callbacks, and rendering architecture independent.

It currently provides:

- typed scalar, array, and string dataref access;
- owned command, flight-loop, modern-window, widget-window, and Plugins-menu
  registration and cleanup;
- drawing, coordinate conversion, plugin metadata, debug logging, feature, and
  path helpers;
- a shared adapter for X-Plane's five required plugin entry points; and
- thread-local plugin-state storage for SDK and graphics handles.

The crate is intentionally local to this workspace and does not attempt to
wrap the entire X-Plane SDK. Add an abstraction only after at least two plugins
need the same lifecycle and safety rules.
