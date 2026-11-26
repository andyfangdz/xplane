//! Instrument drawing operations in page coordinates, before egui tessellation.
use crate::{FontSize, FontStrike};
use egui::{Color32, Id, Mesh, Painter, Pos2, Rect, Shape, Stroke, TextureId, Vec2, pos2, vec2};
use std::{ffi::CString, sync::Arc};
use xplane_sdk_sys as sdk;

pub(crate) enum Command {
    Rect(Rect, Color32),
    Rects(Vec<[sdk::XPLMVertex_t; 8]>, Color32),
    Fill(Vec<sdk::XPLMVertex_t>, Color32),
    Stroke(Vec<sdk::XPLMVertex_t>, Stroke),
    Segments(Vec<[sdk::XPLMVertex_t; 2]>, Stroke),
    Label {
        left: f32,
        baseline: f32,
        text: CString,
        size: FontSize,
        color: Color32,
        texture: TextureId,
        extent: Vec2,
    },
}

pub(crate) fn vertex(p: Pos2) -> sdk::XPLMVertex_t {
    sdk::XPLMVertex_t { x: p.x, y: -p.y }
}

impl Command {
    pub(crate) fn rects(rects: impl Iterator<Item = Rect>, color: Color32) -> Self {
        // Collapse the first and last edge of each rectangle to a point. Connecting
        // these collapsed edges adds zero area even between offset columns.
        Self::Rects(
            rects
                .map(|r| {
                    let a = r.left_top();
                    let d = r.right_bottom();
                    [a, a, a, r.left_bottom(), r.right_top(), d, d, d].map(vertex)
                })
                .collect(),
            color,
        )
    }
    pub(crate) fn enable(ctx: &egui::Context) {
        ctx.data_mut(|data| data.insert_temp(Id::new("xplane-g1000 panel painter"), true));
    }
    pub(crate) fn paint(self, painter: &Painter) {
        if painter
            .ctx()
            .data(|d| d.get_temp::<bool>(Id::new("xplane-g1000 panel painter")))
            .unwrap_or(false)
        {
            painter.add(Shape::Callback(egui::PaintCallback {
                rect: painter.clip_rect(),
                callback: Arc::new(self),
            }));
        } else {
            painter.add(self.software_shape());
        }
    }
    pub(crate) fn texture(&self) -> TextureId {
        match self {
            Self::Label { texture, .. } => *texture,
            _ => TextureId::default(),
        }
    }
    pub(crate) fn is_label(&self) -> bool {
        matches!(self, Self::Label { .. })
    }

    /// Used by ordinary egui painters and deterministic software readback.
    pub(crate) fn software_shape(&self) -> Shape {
        let pos = |p: &sdk::XPLMVertex_t| pos2(p.x, -p.y);
        match self {
            Self::Rect(rect, color) => {
                let mut mesh = Mesh::default();
                mesh.add_colored_rect(*rect, *color);
                Shape::mesh(mesh)
            }
            Self::Rects(rects, color) => {
                let mut mesh = Mesh::default();
                for r in rects {
                    mesh.add_colored_rect(Rect::from_min_max(pos(&r[2]), pos(&r[5])), *color);
                }
                Shape::mesh(mesh)
            }
            Self::Fill(points, color) => {
                Shape::convex_polygon(points.iter().map(pos).collect(), *color, Stroke::NONE)
            }
            Self::Stroke(points, stroke) if points.len() == 2 => {
                Shape::line_segment([pos(&points[0]), pos(&points[1])], *stroke)
            }
            Self::Stroke(points, stroke) => Shape::line(points.iter().map(pos).collect(), *stroke),
            Self::Segments(segments, stroke) => Shape::Vec(
                segments
                    .iter()
                    .map(|[a, b]| Shape::line_segment([pos(a), pos(b)], *stroke))
                    .collect(),
            ),
            Self::Label {
                left,
                baseline,
                text,
                size,
                color,
                texture,
                extent,
            } => {
                let mut mesh = Mesh::with_texture(*texture);
                let cell = Vec2::from(size.cell().map(f32::from));
                for (index, ch) in text
                    .to_str()
                    .expect("UTF-8 instrument label")
                    .chars()
                    .enumerate()
                {
                    if let Some(slot @ 1..) = FontStrike::slot(ch) {
                        let uv = vec2((slot % 16) as f32, (slot / 16) as f32) * cell;
                        mesh.add_rect_with_uv(
                            Rect::from_min_size(
                                pos2(
                                    left + index as f32 * cell.x,
                                    baseline - f32::from(size.baseline()),
                                ),
                                cell,
                            ),
                            Rect::from_min_max(
                                (uv / *extent).to_pos2(),
                                ((uv + cell) / *extent).to_pos2(),
                            ),
                            *color,
                        );
                    }
                }
                Shape::mesh(mesh)
            }
        }
    }

    pub(crate) fn draw(&self, font: Option<&Face>) {
        let color = |c: Color32| u32::from_le_bytes(c.to_srgba_unmultiplied());
        match self {
            Self::Rect(r, c) => {
                let pts = [
                    r.left_top(),
                    r.left_bottom(),
                    r.right_bottom(),
                    r.right_top(),
                ]
                .map(vertex);
                unsafe { sdk::XPLMPolygon(color(*c), pts.as_ptr(), 4) };
            }
            Self::Fill(points, c) => {
                unsafe { sdk::XPLMPolygon(color(*c), points.as_ptr(), points.len() as i32) };
            }
            Self::Rects(rects, c) => {
                for batch in rects.chunks(i32::MAX as usize / 8) {
                    let points = batch.as_flattened();
                    // Each complete rectangle supplies four pairs, live for the drawing call.
                    unsafe { sdk::XPLMQuadstrip(color(*c), points.as_ptr(), points.len() as i32) };
                }
            }
            Self::Stroke(points, stroke) => {
                unsafe {
                    sdk::XPLMLineStripWithWidth(
                        color(stroke.color),
                        stroke.width,
                        points.as_ptr(),
                        points.len() as i32,
                    )
                };
            }
            Self::Segments(segments, stroke) => {
                // SAFETY: inside the panel callback/recording, each borrowed pair supplies
                // two adjacent SDK vertices and remains live through the synchronous call.
                for batch in segments.chunks(i32::MAX as usize / 2) {
                    let points = batch.as_flattened();
                    unsafe {
                        sdk::XPLMLinesWithWidth(
                            color(stroke.color),
                            stroke.width,
                            points.as_ptr(),
                            points.len() as i32,
                        )
                    };
                }
            }
            Self::Label {
                left,
                baseline,
                text,
                size,
                color: c,
                ..
            } => {
                let face = font.expect("native font is ready");
                let advance = i32::from(size.cell()[0]);
                unsafe {
                    sdk::XPLMFontDrawStringFixedSpacing(
                        face.handle.as_ptr(),
                        color(*c),
                        2. * advance as f32,
                        // The SDK places visible ink one texel left of the stock ProFont run.
                        *left + 1.,
                        -*baseline,
                        text.as_ptr(),
                        advance,
                        sdk::xplm_JustLeft,
                    )
                };
            }
        }
    }
}
pub(crate) struct Face {
    handle: std::ptr::NonNull<std::ffi::c_void>,
}
impl Face {
    pub(crate) fn load() -> Option<Self> {
        let handle =
            std::ptr::NonNull::new(unsafe { sdk::XPLMCreateFont(sdk::xplm_CharSetUnicode) })?;
        let face = Self { handle };
        // The panel SDK takes a simulator-relative path with forward slashes.
        if unsafe {
            sdk::XPLMFontAddFace(
                handle.as_ptr(),
                c"Resources/fonts/ProFontWindows.ttf".as_ptr(),
            )
        } == 0
        {
            return None;
        }
        Some(face)
    }
}
impl Drop for Face {
    fn drop(&mut self) {
        unsafe { sdk::XPLMDestroyFont(self.handle.as_ptr()) }
    }
}
