# Shared egui integration

Position Aircraft and the C182 utility windows use `PanelPainter` to submit egui
meshes to SDK 4.4 panel-graphics windows. It batches indexed triangles, converts
premultiplied colors to straight RGBA, patches the font atlas, and submits each
mesh's clip rectangle in the same local coordinates as its vertices. The SDK
maps boxels to the display. The painter reports the pixel density to egui so
fonts are rasterized at the current interface or popped-out window scale.

On Windows, `Surface` also supplies queued pointer, keyboard and wheel events
and manages keyboard focus. Create and drop these resources on the simulator
thread. Call the painter from a panel-graphics window's draw callback. Aircraft
actions run on the model tick.

`cargo test -p xplane-egui` covers input, layout, mesh packing and texture updates.
