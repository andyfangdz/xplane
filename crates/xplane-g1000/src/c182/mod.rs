//! Cessna 182T NAV III engine, lean and system pages.
pub mod layout;
pub mod strip;
use crate::{
    FontSize, TextAlign,
    widgets::{InstrumentFont, InstrumentPainter, Readout, color},
};
use egui::{Response, Sense, Ui, Widget, vec2};
use layout::{Kind, Rgba};
pub use strip::{Input, Page, Strip, build, lean_softkeys};

/// Device rectangles [left, top, right, bottom) occupied by the strip and LEAN softkeys.
pub const STRIP_BOUNDS: [u16; 4] = [0, 55, 153, 742];
pub const LEAN_KEY_BOUNDS: [[u16; 4]; 2] = [[344, 744, 427, 767], [429, 744, 512, 767]];
pub fn character(c: u8) -> char {
    match c {
        127 => 'Δ',
        // The stock EIS uses the underlined ordinal glyph for temperature units.
        0xb0 => 'º',
        _ => char::from(c),
    }
}

/// Numeric formats used by the NAV III, with a maximum of 15 visible bytes per field.
#[derive(Copy, Clone)]
enum NumberFormat {
    Whole,
    Tenths,
    SignedWhole,
    SignedTenths,
    Hours,
    Zero,
}
impl NumberFormat {
    fn number(self, v: f32) -> Vec<u8> {
        let mut text = match self {
            Self::Whole => format!("{v:.0}"),
            Self::Tenths => format!("{v:.1}"),
            Self::SignedWhole => format!("{v:+.0}"),
            Self::SignedTenths => format!("{v:+.1}"),
            Self::Hours => format!("{v:06.1}"),
            Self::Zero => String::from("0.0"),
        };
        text.truncate(15);
        text.into_bytes()
    }
}
fn argb(c: Rgba) -> u32 {
    u32::from_be_bytes([c.a, c.r, c.g, c.b].map(|v| (255. * v).round() as u8))
}
/// Draw a prepared page or softkey scene at the supplied device-space offset.
pub struct StripWidget<'a> {
    pub scene: &'a Strip,
    pub font: &'a InstrumentFont,
    pub offset: [u16; 2],
}
impl Widget for StripWidget<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        paint_strip(ui, self.scene, Some(self.font), self.offset)
    }
}

/// Paint the geometry alone when the installed instrument font is unavailable.
pub fn paint_strip(
    ui: &mut Ui,
    scene: &Strip,
    font: Option<&InstrumentFont>,
    offset: [u16; 2],
) -> Response {
    let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::hover());
    let draw = InstrumentPainter {
        painter: ui.painter_at(rect),
        origin: rect.min - vec2(offset[0].into(), offset[1].into()),
    };
    for p in &scene.prims {
        let a = [p.x0, 768. - p.y0];
        let b = [p.x1, 768. - p.y1];
        let col = argb(p.color);
        match p.kind {
            Kind::Rect => draw.rect(
                a[0].min(b[0]),
                a[1].min(b[1]),
                (b[0] - a[0]).abs(),
                (b[1] - a[1]).abs(),
                col,
            ),
            Kind::Line => draw.line(a, b, col, p.width),
            Kind::Arc => draw.arc(a, p.r, -p.start, -p.sweep, col, p.width),
            Kind::Tri => draw.tri([a, b, [p.r, 768. - p.start]], col),
            Kind::Label => {}
        }
    }
    let Some(font) = font else { return response };
    for p in &scene.prims {
        if p.kind == Kind::Label && (1..=6).contains(&p.text) {
            Readout {
                font,
                text: &p.text.to_string(),
                size: FontSize::Small,
                color: color(argb(p.color)),
            }
            .at(
                ui,
                draw.point([p.x0.round(), 768. - p.y0.round()]),
                TextAlign::Center,
            );
        }
    }
    for t in &scene.texts {
        let align = match t.align {
            i8::MIN..=-1 => TextAlign::Left,
            0 => TextAlign::Center,
            _ => TextAlign::Right,
        };
        let label: String =
            t.s.iter()
                .copied()
                .take_while(|&b| b != 0)
                .map(character)
                .collect();
        Readout {
            font,
            text: &label,
            size: t.font.size(),
            color: color(argb(t.color)),
        }
        .at(ui, draw.point([t.x, 768. - t.y.round()]), align);
    }
    response
}
