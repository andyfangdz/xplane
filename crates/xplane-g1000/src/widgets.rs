//! Fixed-texel egui widgets for stock G1000 avionics callbacks.
use crate::command::{Command, vertex};
use crate::{FontSize, FontStrike, Layer, Page, ProFont, TextAlign};
pub use egui;
use egui::{
    Color32, ColorImage, Context, Painter, Pos2, Rect, Stroke, TextureHandle, TextureOptions, Vec2,
    pos2, vec2,
};
use std::time::Instant;
mod radial;
pub(crate) mod raster;
pub use radial::{DialRay, DialTicks, RadialGauge};
mod linear;
pub use linear::{BarGauge, ColumnStack, GaugePointer, LinearScale, TemperatureColumn};

/// Shares the egui context and managed textures across the aircraft's page surfaces.
/// Coordinates are device texels at 1:1 regardless of window size or desktop DPI.
pub struct Renderer {
    context: Context,
    textures: crate::panel::Textures,
}

impl Default for Renderer {
    fn default() -> Self {
        let context = Context::default();
        Command::enable(&context);
        Self {
            context,
            textures: Default::default(),
        }
    }
}

impl Renderer {
    pub fn context(&self) -> &Context {
        &self.context
    }

    /// Run custom egui widgets on the page's cached background and indication layers.
    /// Retains instrument commands, ordinary egui meshes and their managed textures.
    pub fn update(
        &mut self,
        page: &mut Page,
        mut widgets: impl FnMut(&mut egui::Ui, Layer),
    ) -> bool {
        let size = page.size;
        let rect = Rect::from_min_size(Pos2::ZERO, vec2(size[0].into(), size[1].into()));
        let background = color(page.background_color);
        page.update_layers(Instant::now(), |layer| {
            let input = egui::RawInput {
                screen_rect: Some(rect),
                ..Default::default()
            };
            let output = self.context.run_ui(input, |ui| {
                if layer == Layer::Background && background.a() != 0 {
                    Command::Rect(rect, background).paint(ui.painter());
                }
                widgets(ui, layer)
            });
            let primitives = self
                .context
                .tessellate(output.shapes, output.pixels_per_point);
            self.textures
                .prepare(&self.context, primitives, output.textures_delta)
        })
    }
}

/// The three calibrated ProFont strikes as egui textures. Create once per context.
pub struct InstrumentFont([TextureHandle; 3]);
impl InstrumentFont {
    pub fn new(ctx: &Context, font: &ProFont) -> Self {
        Self(FontSize::ALL.map(|size| {
            let [w, h] = size.cell().map(usize::from);
            let mut image = ColorImage::filled(
                [16 * w, FontStrike::COUNT.div_ceil(16) * h],
                Color32::TRANSPARENT,
            );
            for (slot, ch) in FontStrike::characters().enumerate() {
                let mask = font.strike(size).glyph(ch).unwrap();
                for (row, src) in mask.chunks_exact(w).enumerate() {
                    let start = (slot / 16 * h + row) * image.width() + slot % 16 * w;
                    for (out, &a) in image.pixels[start..start + w].iter_mut().zip(src) {
                        *out = Color32::from_white_alpha(a);
                    }
                }
            }
            ctx.load_texture(
                format!("G1000 ProFont {size:?}"),
                image,
                TextureOptions::NEAREST,
            )
        }))
    }

    pub fn text(
        &self,
        painter: &Painter,
        anchor: Pos2,
        size: FontSize,
        align: TextAlign,
        text: &str,
        color: Color32,
    ) {
        let texture = &self.0[size as usize];
        let cell = Vec2::from(size.cell().map(f32::from));
        let extent = texture.size_vec2();
        let left = align
            .left(anchor.x, cell.x * text.chars().count() as f32)
            .round();
        let supported: String = text
            .chars()
            .map(|ch| {
                if FontStrike::slot(ch).is_some() {
                    ch
                } else {
                    ' '
                }
            })
            .collect();
        Command::Label {
            left,
            baseline: anchor.y.round(),
            text: std::ffi::CString::new(supported).unwrap(),
            size,
            color,
            texture: texture.id(),
            extent,
        }
        .paint(painter);
    }
}

/// A calibrated, fixed-cell text widget for numeric indications, units, and labels.
pub struct Readout<'a> {
    pub font: &'a InstrumentFont,
    pub text: &'a str,
    pub size: FontSize,
    pub color: Color32,
}
impl Readout<'_> {
    fn extent(&self) -> Vec2 {
        let [w, h] = self.size.cell().map(f32::from);
        vec2(w * self.text.chars().count() as f32, h)
    }
    /// Place a readout at a measured display baseline without moving the layout cursor.
    pub fn at(self, ui: &mut egui::Ui, baseline: Pos2, align: TextAlign) -> egui::Response {
        let extent = self.extent();
        let left = align.left(baseline.x, extent.x).round();
        let top = baseline.y.round() - f32::from(self.size.baseline());
        ui.place(Rect::from_min_size(pos2(left, top), extent), self)
    }
}
impl egui::Widget for Readout<'_> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let (rect, response) = ui.allocate_exact_size(self.extent(), egui::Sense::hover());
        let baseline = rect.min + vec2(0., f32::from(self.size.baseline()));
        self.font.text(
            &ui.painter_at(rect),
            baseline,
            self.size,
            TextAlign::Left,
            self.text,
            self.color,
        );
        response
    }
}

/// Drawing primitives in local, top-down instrument coordinates. Use the origin of
/// the rectangle allocated by the enclosing egui widget.
pub struct InstrumentPainter {
    pub painter: Painter,
    pub origin: Pos2,
}
pub fn color(argb: u32) -> Color32 {
    let [a, r, g, b] = argb.to_be_bytes();
    Color32::from_rgba_unmultiplied(r, g, b, a)
}
impl InstrumentPainter {
    pub fn point(&self, p: [f32; 2]) -> Pos2 {
        self.origin + Vec2::from(p)
    }
    fn snapped_rect(&self, [x, y, w, h]: [f32; 4]) -> Option<Rect> {
        (w > 0. && h > 0.).then(|| {
            Rect::from_min_max(
                self.point([x.round(), y.round()]),
                self.point([(x + w).round(), (y + h).round()]),
            )
        })
    }
    pub fn rect(&self, x: f32, y: f32, w: f32, h: f32, col: u32) {
        if let Some(rect) = self.snapped_rect([x, y, w, h]) {
            Command::Rect(rect, color(col)).paint(&self.painter);
        }
    }
    /// Paint consecutive, same-color rectangles as one command, preserving the gaps.
    pub fn rects(&self, bounds: impl IntoIterator<Item = [f32; 4]>, col: u32) {
        let mut rects = bounds.into_iter().filter_map(|r| self.snapped_rect(r));
        let Some(first) = rects.next() else { return };
        let command = match rects.next() {
            None => Command::Rect(first, color(col)),
            Some(second) => Command::rects([first, second].into_iter().chain(rects), color(col)),
        };
        command.paint(&self.painter);
    }
    /// Independent strokes of the same width and color, in painter-local coordinates.
    pub fn segments(&self, lines: impl IntoIterator<Item = [[f32; 2]; 2]>, col: u32, width: f32) {
        let mut lines = lines.into_iter();
        let Some(first) = lines.next() else { return };
        let Some(second) = lines.next() else {
            self.line(first[0], first[1], col, width);
            return;
        };
        let points = [first, second]
            .into_iter()
            .chain(lines)
            .map(|pair| pair.map(|p| vertex(self.point(p))))
            .collect();
        Command::Segments(points, Stroke::new(width, color(col))).paint(&self.painter);
    }
    pub fn line(&self, a: [f32; 2], b: [f32; 2], col: u32, width: f32) {
        Command::Stroke(
            vec![vertex(self.point(a)), vertex(self.point(b))],
            Stroke::new(width, color(col)),
        )
        .paint(&self.painter);
    }
    pub fn tri(&self, pts: [[f32; 2]; 3], col: u32) {
        Command::Fill(pts.map(|p| vertex(self.point(p))).to_vec(), color(col)).paint(&self.painter);
    }
    pub fn arc(&self, center: [f32; 2], r: f32, start: f32, sweep: f32, col: u32, width: f32) {
        if r <= 0. || sweep == 0. {
            return;
        }
        let center = self.point(center);
        let steps = (r * sweep.to_radians().abs() / 2.).ceil().clamp(2., 720.) as usize;
        let points = (0..=steps)
            .map(|i| {
                let (s, c) = (start + sweep * i as f32 / steps as f32)
                    .to_radians()
                    .sin_cos();
                vertex(center + vec2(c, s) * r)
            })
            .collect();
        Command::Stroke(points, Stroke::new(width, color(col))).paint(&self.painter);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn cached_background_retains_its_texture_and_alpha() {
        let mut renderer = Renderer::default();
        let mut page = Page::new([2, 1], 0, Duration::ZERO).unwrap();
        let mut texture = renderer.context().load_texture(
            "overlay",
            ColorImage::filled([1, 1], Color32::from_white_alpha(128)),
            TextureOptions::NEAREST,
        );
        for update in [false, true] {
            if update {
                texture.set(
                    ColorImage::filled([1, 1], Color32::RED),
                    TextureOptions::NEAREST,
                );
            }
            assert!(renderer.update(&mut page, |ui, layer| {
                let x = if layer == Layer::Background { 0. } else { 1. };
                ui.painter().image(
                    texture.id(),
                    Rect::from_min_size(pos2(x, 0.), vec2(1., 1.)),
                    Rect::from_min_max(Pos2::ZERO, pos2(1., 1.)),
                    Color32::WHITE,
                );
            }));
            let texels = page.pixels().unwrap().as_chunks::<4>().0;
            assert_eq!(texels[0], [255, 255, 255, 128]);
            assert_eq!(texels[1], if update { [0, 0, 255, 255] } else { texels[0] });
            assert!(!page.blit([0, 0, 2, 1], [0, 0]));
        }
    }

    #[test]
    fn native_texels_clip_and_repaint_with_partial_texture_updates() {
        let mut renderer = Renderer::default();
        let mut page = Page::new([6, 4], 0xff202020, Duration::ZERO).unwrap();
        let mut image = renderer.context().load_texture(
            "test",
            ColorImage::new([2, 1], vec![Color32::RED, Color32::WHITE]),
            TextureOptions::NEAREST,
        );
        for (foreground, expected_bgra) in [
            (Color32::WHITE, [255; 4]),
            (Color32::GREEN, [0, 255, 0, 255]),
            // Half-covered white over blue in linear light.
            (Color32::from_white_alpha(128), [255, 188, 188, 255]),
        ] {
            image.set_partial(
                [1, 0],
                ColorImage::filled([1, 1], foreground),
                TextureOptions::NEAREST,
            );
            assert!(renderer.update(&mut page, |ui, layer| {
                let painter = ui
                    .painter()
                    .with_clip_rect(Rect::from_min_max(pos2(1., 1.), pos2(4., 3.)));
                let draw = InstrumentPainter {
                    painter,
                    origin: Pos2::ZERO,
                };
                match layer {
                    Layer::Background => draw.rect(-3., 0., 9., 4., 0xff0000ff),
                    Layer::Indications => {
                        draw.painter.image(
                            image.id(),
                            Rect::from_min_size(pos2(2., 1.), vec2(2., 1.)),
                            Rect::from_min_max(Pos2::ZERO, pos2(1., 1.)),
                            Color32::WHITE,
                        );
                    }
                }
            }));
            let pixels = page.pixels().unwrap().as_chunks::<4>().0;
            assert_eq!(pixels[7], [255, 0, 0, 255]);
            assert_eq!(pixels[8], [0, 0, 255, 255]);
            assert_eq!(pixels[9], expected_bgra);
            for i in [0, 4, 6, 10, 18] {
                assert_eq!(pixels[i], [32, 32, 32, 255]);
            }
        }
    }
}
