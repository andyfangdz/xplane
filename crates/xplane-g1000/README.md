# xplane-g1000

Build a custom engine strip, annunciator or full page on the built-in pilot PFD, copilot PFD or MFD. The renderer currently targets X-Plane 12 on Windows. A page can use custom egui widgets or the GDI+ canvas. Both paths present native-resolution frames through the same OpenGL framebuffer blit.

Register an overlay with `Display::overlay` and keep its registration for as long as the plugin is enabled. On an active page, call `Page::update` to paint the background and live indications, then use the callback's `DisplayTarget::blit` to copy the desired regions. Coordinates are texels measured from the upper left of the 1024 × 768 display. Blits are opaque rectangles. Areas outside them retain the stock instruments.

The background is painted once. Live indications start with a fresh copy of that background at the refresh interval you choose. Use `Page::invalidate` on a layout or theme change. `Page::pixels` exposes the resulting top-down BGRA frame for offline tests. Supply colors as `0xAARRGGBB`. Load the simulator's installed `Resources/fonts/ProFontWindows.ttf` once with `ProFont::load(&xplane_plugin::system_path())`. Each strike draws from a typographic baseline in device texels. Small labels, medium readouts and large dial values advance by 8, 11 and 14 texels per character, including punctuation. The font file stays in the simulator installation. Custom artwork can also supply fixed-cell grayscale coverage masks. `GlyphGrid` packs multiple font sizes into a texture atlas. `TextAlign` positions both fixed-width text and runs with narrower punctuation on exact texels. `Display::softkeys` enumerates the twelve commands for each unit. Aircraft plugins own their artwork, datarefs, power checks, page selection and command bindings.

Enable the `egui` Cargo feature to draw widgets. Keep a `widgets::Renderer` for the aircraft and create an `InstrumentFont` on its context from the installed ProFont. Each point is one display texel. Use `ui.painter_at(rect)` to clip an instrument to its allocated area. The compositor supports egui triangle meshes, managed textures with nearest sampling, and opaque instrument backgrounds. X-Plane softkey and bezel commands continue to drive page state.

```rust
# #[cfg(all(windows, feature = "egui"))]
# fn paint(renderer: &mut xplane_g1000::widgets::Renderer, page: &mut xplane_g1000::Page, font: &xplane_g1000::widgets::InstrumentFont, rpm: u32) {
use xplane_g1000::{FontSize, Layer, TextAlign, widgets::egui};
renderer.update(page, |ui, layer| {
    if layer == Layer::Indications {
        xplane_g1000::widgets::Readout {
            font, text: &rpm.to_string(), size: FontSize::Large, color: egui::Color32::WHITE,
        }.at(ui, egui::pos2(135.0, 72.0), TextAlign::Right);
    }
});
# }
```

## Aircraft pages

Enable the `da40` or `c182` Cargo feature to use that aircraft's widgets on Windows. Each feature includes `egui`. The aircraft adapter supplies indications and handles power and bezel commands.

The DA40 XLS widget draws the full engine page or the compact reversionary strip. Create `da40::Atlas::new(renderer.context(), &font)` once using X-Plane's installed ProFont. It draws the softkeys itself. `Atlas::with_softkeys` also accepts two caller-supplied grayscale footer strips.

```rust
# #[cfg(all(windows, feature = "da40"))]
# fn draw(renderer: &mut xplane_g1000::widgets::Renderer, surface: &mut xplane_g1000::Page, atlas: &xplane_g1000::da40::Atlas, sample: &xplane_g1000::da40::Sample, lean: &xplane_g1000::da40::Lean) {
use xplane_g1000::da40::EngineDisplay;
renderer.update(surface, |ui, layer| {
    ui.add(EngineDisplay { atlas, sample, lean, full: true, layer });
});
# }
```

For the C182T NAV III, build an ENGINE, LEAN or SYSTEM scene from the current indications. Allocate a 153 × 687 surface for the strip. Its top-left corner is at (0, 55) on the native display. Use `c182::STRIP_BOUNDS` and `c182::LEAN_KEY_BOUNDS` to place the strip and its optional softkey overlays.

```rust
# #[cfg(all(windows, feature = "c182"))]
# fn draw(renderer: &mut xplane_g1000::widgets::Renderer, surface: &mut xplane_g1000::Page, font: &xplane_g1000::widgets::InstrumentFont, input: &xplane_g1000::c182::Input) {
use xplane_g1000::{Layer, c182};
let mut scene = c182::Strip::default();
c182::build(c182::Page::Lean, input, &mut scene);
renderer.update(surface, |ui, layer| {
    if layer == Layer::Indications {
        ui.add(c182::StripWidget { scene: &scene, font, offset: [0, 55] });
    }
});
# }
```

The [example MFD plugin](examples/mfd_page.rs) toggles a custom page with an assignable command. Build it with `cargo build -p xplane-g1000 --example mfd_page --release`. To try it, copy the resulting `target/release/examples/mfd_page.dll` to `Resources/plugins/G1000PageExample/64/win.xpl` and bind `examples/g1000/toggle_page` in X-Plane. Drop the registration and page resources on the simulator thread during plugin disable.

Use the crate from a sibling checkout with `xplane-g1000 = { path = "../xplane/crates/xplane-g1000" }` (adjust the path relative to your manifest). See Laminar's [avionics drawing callback documentation](https://developer.x-plane.com/sdk/XPLMAvionicsCallback_f/) for the underlying callback contract.
