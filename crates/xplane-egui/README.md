# Shared egui integration

X-Plane matrix/viewport conversion, mesh and clipping transforms, OpenGL loading,
and graphics-state restoration extracted from Position Aircraft's existing egui
adapter. Position Aircraft and C182 use the same rendering helpers.

On Windows, `Surface` supplies queued pointer/key/wheel events, owner-thread
painting, lazy `egui_glow` initialization, retained texture updates, and SDK
keyboard focus. Construct and destroy it on the simulator thread; paint only in
an SDK window draw callback with the OpenGL context current. Aircraft behavior
must run outside draw/input callbacks.

`cargo test -p xplane-egui` checks the projection and offset viewport transform.
