//! DA40 colors and softkey artwork.
use crate::widgets::egui;
use crate::widgets::{InstrumentFont, InstrumentPainter, color};
use crate::{FontSize, Layer, ProFont, TextAlign};
use egui::{Color32, ColorImage, Rect, TextureHandle, TextureOptions, pos2, vec2};
use std::ops::Deref;
pub const WHITE: u32 = 0xffffffff;
pub const BLACK: u32 = 0xff000000;
pub const BG: u32 = 0xff202020;
pub const GREY: u32 = 0xffbcbcbc;
pub const GREEN: u32 = 0xff00ff00;
pub const YELLOW: u32 = 0xffffff00;
pub const RED: u32 = 0xffff0000;
pub const CYAN: u32 = 0xff00ffff;

/// Font and optional 1024 × 52 grayscale softkey artwork for a DA40 display.
pub struct Atlas {
    font: InstrumentFont,
    softkeys: Option<TextureHandle>,
}
impl Atlas {
    /// Use the installed font to draw every indication, including the softkey labels.
    pub fn new(ctx: &egui::Context, font: &ProFont) -> Self {
        Self {
            font: InstrumentFont::new(ctx, font),
            softkeys: None,
        }
    }
    /// Supply the normal and LEAN-selected 1024 × 26 rows, stacked top to bottom.
    pub fn with_softkeys(ctx: &egui::Context, font: &ProFont, masks: &[u8]) -> Option<Self> {
        if masks.len() != 1024 * 52 {
            return None;
        }
        let image = ColorImage::new(
            [1024, 52],
            masks.iter().copied().map(Color32::from_gray).collect(),
        );
        let softkeys = ctx.load_texture("DA40 softkeys", image, TextureOptions::NEAREST);
        Some(Self {
            font: InstrumentFont::new(ctx, font),
            softkeys: Some(softkeys),
        })
    }
}
pub struct Canvas<'a> {
    pub base: bool,
    ui: &'a mut egui::Ui,
    instrument: InstrumentPainter,
    atlas: &'a Atlas,
}
impl Deref for Canvas<'_> {
    type Target = InstrumentPainter;
    fn deref(&self) -> &Self::Target {
        &self.instrument
    }
}
impl<'a> Canvas<'a> {
    pub(super) fn layer(&self) -> Layer {
        if self.base {
            Layer::Background
        } else {
            Layer::Indications
        }
    }

    pub fn new(ui: &'a mut egui::Ui, atlas: &'a Atlas, layer: Layer) -> Self {
        let rect = ui.available_rect_before_wrap();
        let instrument = InstrumentPainter {
            painter: ui.painter_at(rect),
            origin: rect.min,
        };
        Self {
            base: layer == Layer::Background,
            ui,
            instrument,
            atlas,
        }
    }
    fn strike(size: i32) -> FontSize {
        match size {
            ..=16 => FontSize::Small,
            17..=22 => FontSize::Medium,
            _ => FontSize::Large,
        }
    }
    pub fn text(&mut self, x: f32, y: f32, s: impl AsRef<str>, size: i32, col: u32, align: i32) {
        let baseline = match Self::strike(size) {
            FontSize::Small => (y - 5.).round() as i32 + 9,
            FontSize::Medium => (y - 7.).round() as i32 + 12,
            FontSize::Large => (y - 8.5).round() as i32 + 16,
        };
        self.baseline(x, baseline, s.as_ref(), size, col, align)
    }
    pub fn baseline(&mut self, x: f32, baseline: i32, s: &str, size: i32, col: u32, align: i32) {
        let alignment = match align {
            0 => TextAlign::Left,
            1 => TextAlign::Center,
            _ => TextAlign::Right,
        };
        // DA40 layout coordinates name the last ink row. The font baseline is its lower edge.
        let anchor = self.point([x, (baseline + 1) as f32]);
        crate::widgets::Readout {
            font: &self.atlas.font,
            text: s,
            size: Self::strike(size),
            color: color(col),
        }
        .at(self.ui, anchor, alignment);
    }
    pub fn keys(&mut self, lean: bool) {
        let Some(texture) = &self.atlas.softkeys else {
            let labels = [
                "ENGINE", "", "", "", "", "LEAN", "", "", "", "DEC FUEL", "INC FUEL", "RST FUEL",
            ];
            self.rect(0., 742., 1024., 26., BG);
            self.rect(0., 742., 1024., 2., GREY);
            for (i, label) in labels.into_iter().enumerate() {
                let x = 4. + 85. * i as f32;
                let selected = i == 0 || (lean && i == 5);
                self.rect(x - 2., 744., 2., 24., GREY);
                if selected {
                    self.rect(x, 744., 83., 23., WHITE);
                }
                if !label.is_empty() {
                    self.baseline(
                        x + 41.5,
                        760,
                        label,
                        16,
                        if selected { BG } else { WHITE },
                        1,
                    );
                }
            }
            self.rect(1022., 744., 2., 24., GREY);
            return;
        };
        let top = if lean { 0.5 } else { 0.0 };
        self.painter.image(
            texture.id(),
            Rect::from_min_size(self.point([0., 742.]), vec2(1024., 26.)),
            Rect::from_min_max(pos2(0., top), pos2(1., top + 0.5)),
            Color32::WHITE,
        );
    }
}
