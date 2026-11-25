# xplane-g1000

Build a custom engine strip, annunciator or full page on the built-in pilot PFD, copilot PFD or MFD. The renderer currently targets X-Plane 12 on Windows. It uses GDI+ for antialiased shapes and an OpenGL framebuffer blit for native-resolution placement.

Register an overlay with `Display::overlay` and keep its registration for as long as the plugin is enabled. On an active page, call `Page::update` to paint the background and live indications, then use the callback's `DisplayTarget::blit` to copy the desired regions. Coordinates are texels measured from the upper left of the 1024 × 768 display. Blits are opaque rectangles. Areas outside them retain the stock instruments.

The background is painted once. Live indications start with a fresh copy of that background at the refresh interval you choose. Use `Page::invalidate` on a layout or theme change. `Page::pixels` exposes the resulting top-down BGRA frame for offline tests. Supply colors as `0xAARRGGBB`. Text accepts caller-supplied, fixed-cell grayscale coverage masks. `GlyphGrid` packs multiple font sizes into a texture atlas. `TextAlign` positions both fixed-width text and runs with narrower punctuation on exact texels. `Display::softkeys` enumerates the twelve commands for each unit. Aircraft plugins own their artwork, datarefs, power checks, page selection and command bindings.

The [example MFD plugin](examples/mfd_page.rs) toggles a custom page with an assignable command. Build it with `cargo build -p xplane-g1000 --example mfd_page --release`. To try it, copy the resulting `target/release/examples/mfd_page.dll` to `Resources/plugins/G1000PageExample/64/win.xpl` and bind `examples/g1000/toggle_page` in X-Plane. Drop the registration and page resources on the simulator thread during plugin disable.

Use the crate from a sibling checkout with `xplane-g1000 = { path = "../xplane/crates/xplane-g1000" }` (adjust the path relative to your manifest). See Laminar's [avionics drawing callback documentation](https://developer.x-plane.com/sdk/XPLMAvionicsCallback_f/) for the underlying callback contract.
