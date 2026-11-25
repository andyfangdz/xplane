# xplane-g1000

Build egui pages and overlays for the built-in G1000 pilot PFD, copilot PFD and MFD in X-Plane 12.4.4 or later on Windows. One egui point corresponds to one texel on the 1024 × 768 display.

Register a callback with `Display::overlay` and keep the registration while the plugin is enabled. Keep one `widgets::Renderer` for the aircraft. Call `Renderer::update` with a `Page` to paint its background and live indications, then `DisplayTarget::blit` to draw them. Instrument labels reach the SDK 4.4 panel API as whole strings. Needles, tick marks and gauge arcs use native line strips. Filled instrument shapes use native polygons. Images and other egui meshes retain their place among those commands and share the widget's clip rectangle. Positions are measured from the upper left. A transparent page lets the stock instrument show through.

The renderer caches the background and replaces the live indications at the page's refresh interval. Call `Page::invalidate` after a layout or theme change. For offline tests, `Page::pixels` rasterizes the same drawing order into a top-down BGRA frame using calibrated glyph masks.

Load X-Plane's installed `Resources/fonts/ProFontWindows.ttf` with `ProFont::load(&xplane_plugin::system_path())`, then create an `InstrumentFont` on the renderer's context. Readouts use a typographic baseline. The SDK draws them at 16, 22 and 28 pixels with fixed advances of 8, 11 and 14 texels. The renderer loads the SDK font face and bakes image atlases on the simulator thread between draws. Use `ui.painter_at(rect)` to clip a widget to its instrument. Aircraft plugins handle power, page selection and the bezel commands enumerated by `Display::softkeys`.

```rust
# #[cfg(windows)]
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

Enable the `da40` or `c182` Cargo feature to use that aircraft's widgets on Windows. The aircraft adapter supplies indications and handles power and bezel commands.

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

The [example MFD plugin](examples/mfd_page.rs) toggles a custom page with an assignable command. Build it with `cargo build -p xplane-g1000 --example mfd_page --release`. To try it, copy the resulting `target/release/examples/mfd_page.dll` to `Resources/plugins/G1000PageExample/64/win.xpl` and bind `examples/g1000/toggle_page` in X-Plane. Drop the registrations, renderer, and pages on the simulator thread during plugin disable, outside a panel draw callback.

Use the crate from a sibling checkout with `xplane-g1000 = { path = "../xplane/crates/xplane-g1000" }` (adjust the path relative to your manifest). See Laminar's [avionics drawing callback documentation](https://developer.x-plane.com/sdk/XPLMAvionicsCallback_f/) for the underlying callback contract.
