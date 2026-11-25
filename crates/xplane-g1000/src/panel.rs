//! Retain clipped instrument commands, egui meshes and their atlas generations in paint order.
use crate::command::{Command, Face};
use egui::epaint::{ClippedPrimitive, ClippedShape, Mesh16, Primitive};
use egui::{ColorImage, ImageData, Rect, TextureId, TexturesDelta};
use std::{
    cell::{Cell, OnceCell, RefCell},
    collections::HashMap,
    ptr::NonNull,
    rc::{Rc, Weak},
    sync::Arc,
};
use xplane_sdk_sys as sdk;

#[derive(Clone, Copy)]
struct ImageRef {
    atlas: NonNull<std::ffi::c_void>,
    index: i32,
}
struct Handle(NonNull<std::ffi::c_void>);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe { sdk::XPLMDestroyTextureAtlas(self.0.as_ptr()) }
    }
}

pub(crate) struct Atlas {
    pub image: Arc<ColorImage>,
    uploads: Rc<Uploads>,
    ready: Cell<Option<ImageRef>>,
    queued: Cell<bool>,
    failed: Cell<bool>,
}
impl Atlas {
    fn new(image: Arc<ColorImage>, uploads: Rc<Uploads>) -> Rc<Self> {
        Rc::new(Self {
            image,
            uploads,
            ready: Cell::default(),
            queued: Cell::default(),
            failed: Cell::default(),
        })
    }
    fn request(self: &Rc<Self>) -> bool {
        if !self.queued.replace(true) && !self.uploads.queue(self) {
            self.failed.set(true);
        }
        !self.failed.get()
    }
}

// The SDK prohibits creating, baking or destroying an atlas in a panel
// callback. The flight loop owns the handles. Pages retain image generations
// while they use them, including when another page changes a shared texture.
#[derive(Default)]
struct Uploads {
    task: OnceCell<xplane_plugin::CallbackFlightLoop>,
    pending: RefCell<Vec<Weak<Atlas>>>,
    font_requested: Cell<bool>,
    font: OnceCell<Option<Face>>,
    live: RefCell<Vec<(Weak<Atlas>, Handle)>>,
}
impl Uploads {
    fn schedule(self: &Rc<Self>) -> bool {
        if self.task.get().is_none() {
            let weak = Rc::downgrade(self);
            let Some(task) = xplane_plugin::CallbackFlightLoop::create(
                sdk::xplm_FlightLoop_Phase_BeforeFlightModel,
                "G1000 panel textures",
                move |_| weak.upgrade().map_or(0., |uploads| uploads.flush()),
            ) else {
                return false;
            };
            let _ = self.task.set(task);
        }
        self.task.get().unwrap().schedule(-1., true);
        true
    }
    fn queue(self: &Rc<Self>, asset: &Rc<Atlas>) -> bool {
        self.pending.borrow_mut().push(Rc::downgrade(asset));
        self.schedule()
    }
    fn request_font(self: &Rc<Self>) -> bool {
        if !self.font_requested.replace(true) && !self.schedule() {
            let _ = self.font.set(None);
        }
        self.font.get().is_some()
    }
    fn flush(&self) -> f32 {
        if self.font_requested.get() {
            self.font.get_or_init(Face::load);
        }
        let mut live = self.live.borrow_mut();
        live.retain(|(asset, _)| asset.strong_count() > 0);
        for weak in self.pending.take() {
            if let Some(asset) = weak.upgrade() {
                if let Some((handle, image)) = upload(&asset.image) {
                    asset.ready.set(Some(image));
                    live.push((weak, handle));
                } else {
                    asset.failed.set(true);
                }
            }
        }
        if live.is_empty() { 0. } else { -1. }
    }
}
fn upload(image: &ColorImage) -> Option<(Handle, ImageRef)> {
    let w = image.width().try_into().ok()?;
    let h = image.height().try_into().ok()?;
    let rgba: Vec<_> = image
        .pixels
        .iter()
        .map(|p| p.to_srgba_unmultiplied())
        .collect();
    let atlas = NonNull::new(unsafe { sdk::XPLMCreateTextureAtlas() })?;
    let handle = Handle(atlas);
    let index =
        unsafe { sdk::XPLMTextureAtlasAddImage(atlas.as_ptr(), rgba.as_ptr().cast(), w, h) };
    if index < 0 {
        return None;
    }
    unsafe { sdk::XPLMTextureAtlasBake(atlas.as_ptr()) };
    Some((handle, ImageRef { atlas, index }))
}

enum Geometry {
    Mesh(Mesh16),
    Instrument {
        command: Arc<Command>,
        ctx: egui::Context,
        readback: OnceCell<Vec<Mesh16>>,
    },
}
pub(crate) struct Batch {
    pub clip: Rect,
    geometry: Geometry,
    pub atlas: Rc<Atlas>,
    strips: OnceCell<Vec<Strip>>,
}

#[derive(Default)]
pub(crate) struct Textures(HashMap<TextureId, Rc<Atlas>>, Rc<Uploads>);
impl Textures {
    pub fn prepare(
        &mut self,
        ctx: &egui::Context,
        items: Vec<ClippedPrimitive>,
        mut delta: TexturesDelta,
    ) -> Option<Vec<Batch>> {
        for (id, patches) in std::mem::take(&mut delta.set) {
            for patch in patches {
                let ImageData::Color(image) = patch.image;
                let image = if let Some([x, y]) = patch.pos {
                    let mut previous = self.0.get(&id)?.image.as_ref().clone();
                    let width = previous.width();
                    if x.checked_add(image.width())? > width
                        || y.checked_add(image.height())? > previous.height()
                    {
                        return None;
                    }
                    for (row, pixels) in image.pixels.chunks_exact(image.width()).enumerate() {
                        let start = (y + row) * width + x;
                        previous.pixels[start..start + pixels.len()].copy_from_slice(pixels);
                    }
                    Arc::new(previous)
                } else {
                    image
                };
                self.0.insert(id, Atlas::new(image, self.1.clone()));
            }
        }
        let batches = (|| {
            let mut batches = Vec::new();
            for item in items {
                match item.primitive {
                    Primitive::Callback(cb) => {
                        let command = cb.callback.downcast::<Command>().ok()?;
                        let atlas = self.0.get(&command.texture())?.clone();
                        batches.push(Batch {
                            clip: item.clip_rect,
                            atlas,
                            strips: OnceCell::new(),
                            geometry: Geometry::Instrument {
                                command,
                                ctx: ctx.clone(),
                                readback: OnceCell::new(),
                            },
                        });
                    }
                    Primitive::Mesh(mesh) => {
                        let atlas = self.0.get(&mesh.texture_id)?;
                        if !mesh.is_valid() {
                            return None;
                        }
                        for mesh in mesh.split_to_u16() {
                            if !mesh.indices.is_empty() {
                                batches.push(Batch {
                                    geometry: Geometry::Mesh(mesh),
                                    clip: item.clip_rect,
                                    atlas: atlas.clone(),
                                    strips: OnceCell::new(),
                                });
                            }
                        }
                    }
                }
            }
            Some(batches)
        })();
        for id in std::mem::take(&mut delta.free) {
            self.0.remove(&id);
        }
        batches
    }
}

enum Strip {
    Solid(Vec<sdk::XPLMVertexColor_t>),
    Textured(u32, Vec<sdk::XPLMTextureVertex_t>),
    Raster,
}

// Six vertices encode one triangle in an even-length triangle/quad strip.
// The duplicated endpoints also join consecutive triangles without filling
// the gap between them.
fn append<T: Copy>(strip: &mut Vec<T>, [a, b, c]: [T; 3]) {
    strip.extend_from_slice(&[a, a, a, b, c, c]);
}
impl Batch {
    pub(crate) fn meshes(&self) -> &[Mesh16] {
        match &self.geometry {
            Geometry::Mesh(mesh) => std::slice::from_ref(mesh),
            Geometry::Instrument {
                command,
                ctx,
                readback,
            } => readback.get_or_init(|| {
                let shape = ClippedShape {
                    clip_rect: self.clip,
                    shape: command.software_shape(),
                };
                ctx.tessellate(vec![shape], 1.)
                    .into_iter()
                    .flat_map(|item| match item.primitive {
                        Primitive::Mesh(mesh) => mesh.split_to_u16(),
                        Primitive::Callback(_) => {
                            unreachable!("software instrument shapes contain no callbacks")
                        }
                    })
                    .collect()
            }),
        }
    }
    fn command(&self) -> Option<&Command> {
        match &self.geometry {
            Geometry::Instrument { command, .. } => Some(command),
            _ => None,
        }
    }
    fn native(&self) -> Option<&Command> {
        self.command()
            .filter(|c| !c.is_label() || self.atlas.uploads.font.get().is_some_and(Option::is_some))
    }
    fn strips(&self) -> &[Strip] {
        self.strips.get_or_init(|| {
            let mut strips = Vec::new();
            for mesh in self.meshes() {
                for triangle in mesh.indices.as_chunks::<3>().0 {
                    let mut v = triangle.map(|i| mesh.vertices[usize::from(i)]);
                    // Native panel coordinates point up. Keep the real triangle
                    // counter-clockwise, regardless of egui's input winding.
                    let a = v[1].pos - v[0].pos;
                    let b = v[2].pos - v[0].pos;
                    if a.x * b.y - a.y * b.x > 0. {
                        v.swap(1, 2);
                    }
                    if v.iter().all(|p| p.uv == egui::epaint::WHITE_UV)
                        && self.atlas.image.pixels.first() == Some(&egui::Color32::WHITE)
                    {
                        if !matches!(strips.last(), Some(Strip::Solid(_))) {
                            strips.push(Strip::Solid(Vec::new()));
                        }
                        let Some(Strip::Solid(points)) = strips.last_mut() else {
                            unreachable!()
                        };
                        append(
                            points,
                            v.map(|p| sdk::XPLMVertexColor_t {
                                x: p.pos.x,
                                y: -p.pos.y,
                                color: u32::from_le_bytes(p.color.to_srgba_unmultiplied()),
                            }),
                        );
                    } else if v.iter().all(|p| {
                        p.color == v[0].color
                            && (0.0..=1.0).contains(&p.uv.x)
                            && (0.0..=1.0).contains(&p.uv.y)
                    }) {
                        let tint = u32::from_le_bytes(v[0].color.to_srgba_unmultiplied());
                        if !matches!(strips.last(), Some(Strip::Textured(last, _)) if *last == tint)
                        {
                            strips.push(Strip::Textured(tint, Vec::new()));
                        }
                        let Some(Strip::Textured(_, points)) = strips.last_mut() else {
                            unreachable!()
                        };
                        append(
                            points,
                            v.map(|p| sdk::XPLMTextureVertex_t {
                                x: p.pos.x,
                                y: -p.pos.y,
                                s: p.uv.x,
                                // The raw top-down image retains row zero at t=0 in the atlas mesh.
                                t: p.uv.y,
                            }),
                        );
                    } else {
                        // The native textured mesh has one tint per draw. Preserve
                        // textured vertex gradients and out-of-bounds UVs
                        // through the same rasterizer used by Page::pixels.
                        return vec![Strip::Raster];
                    }
                }
            }
            strips
        })
    }
}

fn clipped(rect: Rect, origin: [f32; 2], draw: impl FnOnce()) {
    struct Pop;
    impl Drop for Pop {
        fn drop(&mut self) {
            unsafe {
                sdk::XPLMTransformPop();
                sdk::XPLMScissorPop();
            }
        }
    }
    unsafe {
        sdk::XPLMScissorPush();
        sdk::XPLMScissorIntersect(
            (origin[0] + rect.min.x).round() as i32,
            (origin[1] - rect.min.y).round() as i32,
            (origin[0] + rect.max.x).round() as i32,
            (origin[1] - rect.max.y).round() as i32,
        );
        sdk::XPLMTransformPush();
        sdk::XPLMTransformTranslate(origin[0], origin[1]);
    }
    let _pop = Pop;
    draw();
}

pub(crate) fn draw(page: &crate::Page, crop: Rect, origin: [f32; 2]) -> bool {
    let visible = || {
        page.background
            .iter()
            .chain(&page.indications)
            .filter_map(|batch| {
                let rect = crop.intersect(batch.clip);
                rect.is_positive().then_some((batch, rect))
            })
    };
    // Resolve the font outside drawing. A failed face load uses the calibrated glyph atlas.
    let mut waiting = false;
    for (batch, _) in visible() {
        if batch.command().is_some_and(Command::is_label) {
            waiting |= !batch.atlas.uploads.request_font();
        }
    }
    if waiting {
        return true;
    }
    if let Some((batch, _)) =
        visible().find(|(b, _)| b.native().is_none() && matches!(b.strips(), [Strip::Raster]))
    {
        let image = page.fallback.get_or_init(|| {
            let pixels = page
                .pixels()
                .unwrap()
                .as_chunks::<4>()
                .0
                .iter()
                .map(|&[b, g, r, a]| egui::Color32::from_rgba_unmultiplied(r, g, b, a))
                .collect();
            Atlas::new(
                Arc::new(ColorImage::new(page.size.map(usize::from), pixels)),
                batch.atlas.uploads.clone(),
            )
        });
        if !image.request() {
            return false;
        }
        if let Some(image) = image.ready.get() {
            clipped(crop, origin, || unsafe {
                sdk::XPLMTextureAtlasDrawAt(image.atlas.as_ptr(), image.index, u32::MAX, 0., 0.)
            });
        }
        return true;
    }
    let mut pending = false;
    for (batch, _) in visible() {
        if batch.native().is_none()
            && batch
                .strips()
                .iter()
                .any(|s| matches!(s, Strip::Textured(..)))
        {
            if !batch.atlas.request() {
                return false;
            }
            pending |= batch.atlas.ready.get().is_none();
        }
    }
    if pending {
        return true;
    }
    for (batch, rect) in visible() {
        clipped(rect, origin, || {
            if let Some(command) = batch.native() {
                command.draw(batch.atlas.uploads.font.get().and_then(Option::as_ref));
                return;
            }
            for strip in batch.strips() {
                match strip {
                    Strip::Solid(points) => unsafe {
                        sdk::XPLMQuadstripc(points.as_ptr(), points.len() as i32)
                    },
                    Strip::Textured(tint, points) => {
                        let image = batch.atlas.ready.get().unwrap();
                        unsafe {
                            sdk::XPLMTextureAtlasDrawMesh(
                                image.atlas.as_ptr(),
                                image.index,
                                *tint,
                                points.as_ptr(),
                                points.len() as i32,
                            )
                        }
                    }
                    Strip::Raster => unreachable!(),
                }
            }
        });
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::epaint::Vertex;
    use egui::{Color32, Pos2, pos2};

    #[test]
    fn instrument_commands_keep_clip_and_order_among_egui_meshes() {
        use crate::widgets::{InstrumentPainter, Renderer};
        use crate::{FontSize, Layer, Page};

        let mut renderer = Renderer::default();
        let mut page = Page::new([120, 64], 0, std::time::Duration::ZERO).unwrap();
        let clip = Rect::from_min_max(pos2(4., 6.), pos2(112., 52.));
        assert!(renderer.update(&mut page, |ui, layer| {
            if layer != Layer::Indications {
                return;
            }
            let draw = InstrumentPainter {
                painter: ui.painter_at(clip),
                origin: Pos2::ZERO,
            };
            draw.rect(5., 8., 30., 20., 0xff202020);
            draw.painter.rect_filled(clip, 0., Color32::DARK_BLUE);
            draw.line([8., 12.], [16., 20.], u32::MAX, 2.);
            draw.tri([[10., 25.], [15., 20.], [20., 25.]], u32::MAX);
            Command::Label {
                left: 25.,
                baseline: 30.,
                text: c"RPM".to_owned(),
                size: FontSize::Small,
                color: Color32::WHITE,
                texture: TextureId::default(),
                extent: egui::vec2(128., 128.),
            }
            .paint(&draw.painter);
        }));
        let [rect, mesh, line, triangle, label] = page.indications.as_slice() else {
            panic!("expected four instrument commands interleaved with one ordinary egui mesh");
        };
        assert!(matches!(rect.command(), Some(Command::Rect(..))));
        assert!(matches!(mesh.geometry, Geometry::Mesh(_)));
        assert!(
            matches!(line.command(), Some(Command::Stroke(p, s)) if p.len() == 2 && s.width == 2.)
        );
        assert!(matches!(triangle.command(), Some(Command::Fill(p, _)) if p.len() == 3));
        assert!(
            matches!(label.command(), Some(Command::Label { text, .. }) if text.as_bytes() == b"RPM")
        );
        for batch in &page.indications {
            assert_eq!(batch.clip, clip);
            if let Geometry::Instrument { readback, .. } = &batch.geometry {
                assert!(readback.get().is_none());
            }
        }
        assert!(page.pixels().is_some());
        assert!(
            matches!(&label.geometry, Geometry::Instrument { readback, .. } if readback.get().is_some())
        );
    }

    #[test]
    fn textured_strip_keeps_the_top_down_rows_and_only_connects_degenerate_triangles() {
        let asset = Atlas::new(
            Arc::new(ColorImage::filled([4, 8], Color32::WHITE)),
            Rc::default(),
        );
        let vertices = [[1., 2.], [3., 2.], [1., 4.], [3., 4.]].map(|[x, y]| Vertex {
            pos: pos2(x, y),
            uv: pos2(x / 4., y / 8.),
            color: Color32::WHITE,
        });
        let batch = Batch {
            clip: Rect::EVERYTHING,
            geometry: Geometry::Mesh(Mesh16 {
                vertices: vertices.into(),
                indices: vec![0, 1, 2, 2, 1, 3],
                texture_id: TextureId::default(),
            }),
            atlas: asset,
            strips: OnceCell::new(),
        };
        let [Strip::Textured(_, strip)] = batch.strips() else {
            panic!("expected one textured strip")
        };
        let triangles = strip.windows(3).filter(|tri| {
            let [a, b, c] = [tri[0], tri[1], tri[2]].map(|p| Pos2::new(p.x, p.y));
            (b.x - a.x) * (c.y - a.y) != (b.y - a.y) * (c.x - a.x)
        });
        assert_eq!(triangles.count(), 2);
        for p in strip {
            assert_eq!(p.t, -p.y / 8.);
            assert_eq!(p.s, p.x / 4.);
        }
    }
}
