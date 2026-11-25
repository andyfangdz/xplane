//! SDK 4.4 painter for egui meshes in a panel-graphics window.
use crate::{Geometry, ui_scale};
use egui::{
    Color32, ColorImage, Context, FullOutput, ImageData, RawInput, Rect, TextureId,
    epaint::{ClippedPrimitive, ImageDelta, Mesh, Mesh16, Primitive},
};
use std::{collections::HashMap, ffi::c_void, mem, ptr::NonNull, sync::Arc};
use xplane_plugin::DataRefCache;
use xplane_sdk_sys as sdk;

struct Handle(NonNull<c_void>);
impl Drop for Handle {
    fn drop(&mut self) {
        // The SDK retains submitted draws until rendered, even when their texture is freed here.
        unsafe { sdk::XPLMDestroyTexture(self.0.as_ptr()) }
    }
}

struct Texture {
    // The SDK takes whole images. Retain the source for egui's rectangular font/image patches.
    image: Arc<ColorImage>,
    handle: Option<Handle>,
}

impl Texture {
    fn patch(&mut self, [x, y]: [usize; 2], patch: &ColorImage) -> Result<(), &'static str> {
        let w = self.image.width();
        if patch.width() == 0
            || patch.height() == 0
            || x.checked_add(patch.width()).is_none_or(|r| r > w)
            || y.checked_add(patch.height())
                .is_none_or(|b| b > self.image.height())
        {
            return Err("egui texture patch is outside its image");
        }
        let pixels = &mut Arc::make_mut(&mut self.image).pixels;
        for (row, source) in patch.pixels.chunks_exact(patch.width()).enumerate() {
            let start = (y + row) * w + x;
            pixels[start..start + source.len()].copy_from_slice(source);
        }
        self.handle = None;
        Ok(())
    }

    fn upload(&mut self) -> Result<NonNull<c_void>, &'static str> {
        if let Some(handle) = &self.handle {
            return Ok(handle.0);
        }
        let w = self
            .image
            .width()
            .try_into()
            .map_err(|_| "egui texture too wide")?;
        let h = self
            .image
            .height()
            .try_into()
            .map_err(|_| "egui texture too tall")?;
        let rgba = straight_texels(&self.image);
        // w*h tightly packed RGBA8 texels, top row first. The SDK consumes them before returning.
        let handle = NonNull::new(unsafe { sdk::XPLMCreateTexture(rgba.as_ptr().cast(), w, h) })
            .ok_or("X-Plane could not allocate the egui texture")?;
        self.handle = Some(Handle(handle));
        Ok(handle)
    }
}

// egui also permits additive premultiplied colors (including its faint row fill).
// Straight blending cannot add to an arbitrary background. Use the smallest
// representable opacity that preserves the emitted RGB on a dark background.
fn straight_rgba(color: Color32) -> [u8; 4] {
    let [r, g, b, a] = color.to_array();
    Color32::from_rgba_premultiplied(r, g, b, a.max(r).max(g).max(b)).to_srgba_unmultiplied()
}

// Straight-alpha bilinear filtering interpolates RGB across glyph/image edges.
// Give zero-alpha neighbours the visible edge RGB lost to egui's premultiplication.
fn straight_texels(image: &ColorImage) -> Vec<[u8; 4]> {
    let mut out: Vec<_> = image.pixels.iter().copied().map(straight_rgba).collect();
    if !image.pixels.contains(&Color32::TRANSPARENT) {
        return out;
    }
    let w = image.width();
    let h = image.height();
    for (i, p) in image.pixels.iter().enumerate() {
        if *p == Color32::TRANSPARENT {
            continue;
        }
        let (x, y) = (i % w, i / w);
        let [r, g, b, _] = out[i];
        for row in y.saturating_sub(1)..=(y + 1).min(h - 1) {
            for col in x.saturating_sub(1)..=(x + 1).min(w - 1) {
                let j = row * w + col;
                if image.pixels[j] == Color32::TRANSPARENT {
                    out[j] = [r, g, b, 0];
                }
            }
        }
    }
    out
}

// Egui's splitter assumes each individual triangle spans at most 65535 vertices.
// A user-provided indexed mesh can space even its three corners farther apart.
fn panel_meshes(mesh: Mesh) -> Vec<Mesh16> {
    const MAX: usize = u16::MAX as usize;
    if mesh.vertices.len() <= MAX
        || mesh
            .indices
            .as_chunks::<3>()
            .0
            .iter()
            .all(|tri| tri[0].max(tri[1]).max(tri[2]) - tri[0].min(tri[1]).min(tri[2]) < MAX as u32)
    {
        return mesh.split_to_u16();
    }
    // MAX is divisible by three, so each chunk ends on a triangle boundary.
    mesh.indices
        .chunks(MAX)
        .map(|indices| Mesh16 {
            indices: (0..indices.len() as u16).collect(),
            vertices: indices.iter().map(|&i| mesh.vertices[i as usize]).collect(),
            texture_id: mesh.texture_id,
        })
        .collect()
}

#[derive(Default)]
struct DrawList {
    vertices: Vec<[f32; 5]>,
    indices: Vec<u16>,
    calls: Vec<sdk::XPLMDrawCall_t>,
}

impl DrawList {
    fn clear(&mut self) {
        self.vertices.clear();
        self.indices.clear();
        self.calls.clear();
    }

    fn push(&mut self, mesh: Mesh16, clip: Rect, tex: NonNull<c_void>) -> Result<(), &'static str> {
        let vtx_offset =
            i32::try_from(self.vertices.len()).map_err(|_| "too many egui vertices")?;
        let idx_offset = i32::try_from(self.indices.len()).map_err(|_| "too many egui indices")?;
        let element_count = i32::try_from(mesh.indices.len()).map_err(|_| "egui mesh too large")?;
        self.calls.push(sdk::XPLMDrawCall_t {
            tex_ref: tex.as_ptr(),
            scissors: [clip.min.x, clip.min.y, clip.max.x, clip.max.y],
            vtx_offset,
            idx_offset,
            element_count,
        });
        let base = self.vertices.len();
        self.vertices.extend(mesh.vertices.iter().map(|v| {
            let packed = u32::from_le_bytes(straight_rgba(v.color));
            [v.pos.x, v.pos.y, v.uv.x, v.uv.y, f32::from_bits(packed)]
        }));
        // The outer ring of egui's antialiasing is transparent black in premultiplied
        // form. In a straight-alpha mesh it needs its adjacent interior color at alpha zero.
        for triangle in mesh.indices.as_chunks::<3>().0 {
            let Some(edge) = triangle
                .iter()
                .map(|&i| mesh.vertices[usize::from(i)].color)
                .find(|c| *c != Color32::TRANSPARENT)
            else {
                continue;
            };
            for &i in triangle {
                if mesh.vertices[usize::from(i)].color == Color32::TRANSPARENT {
                    let [r, g, b, _] = straight_rgba(edge);
                    self.vertices[base + usize::from(i)][4] =
                        f32::from_bits(u32::from_le_bytes([r, g, b, 0]));
                }
            }
        }
        self.indices.extend(mesh.indices);
        Ok(())
    }

    fn draw(&self, bounds: Geometry) -> Result<(), &'static str> {
        if self.calls.is_empty() {
            return Ok(());
        }
        let mesh = sdk::XPLMMesh_t {
            vertex_count: self
                .vertices
                .len()
                .try_into()
                .map_err(|_| "too many egui vertices")?,
            vertices: self.vertices.as_ptr().cast(),
            index_count: self
                .indices
                .len()
                .try_into()
                .map_err(|_| "too many egui indices")?,
            indices: self.indices.as_ptr(),
        };
        let count = self
            .calls
            .len()
            .try_into()
            .map_err(|_| "too many egui draw calls")?;
        // All buffers live through submission. Positions and clips share exactly the
        // inverse of Geometry::local, including for a monitor with a negative origin.
        unsafe {
            sdk::XPLMTransformPush();
            sdk::XPLMTransformTranslate(bounds.left as f32, bounds.top as f32);
            sdk::XPLMTransformScale(1., -1.);
            sdk::XPLMDrawCalls(&mesh, count, self.calls.as_ptr());
            sdk::XPLMTransformPop();
        }
        Ok(())
    }
}

/// One painter per egui context. Use on the simulator thread, inside the window's draw callback.
#[derive(Default)]
pub struct PanelPainter {
    datarefs: DataRefCache,
    textures: HashMap<TextureId, Texture>,
    draw_list: DrawList,
}

impl PanelPainter {
    /// Device pixels per layout boxel, used only for tessellation and atlas resolution.
    /// X-Plane applies the window transform and interface scaling itself.
    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    pub fn pixels_per_point(&self, window: sdk::XPLMWindowID, bounds: Geometry) -> f32 {
        if !window.is_null()
            && bounds.width() > 0
            && unsafe { sdk::XPLMWindowIsPoppedOut(window) } != 0
        {
            let (mut l, mut r) = (0, 0);
            unsafe {
                sdk::XPLMGetWindowGeometryOS(
                    window,
                    &mut l,
                    std::ptr::null_mut(),
                    &mut r,
                    std::ptr::null_mut(),
                )
            };
            if r > l {
                return ui_scale((r - l) as f32 / bounds.width() as f32);
            }
        }
        ui_scale(
            self.datarefs
                .find("sim/graphics/misc/user_interface_scale")
                .map_or(1., |r| r.get_f32()),
        )
    }

    pub fn set_input_scale(input: &mut RawInput, pixels_per_point: f32) {
        input
            .viewports
            .entry(input.viewport_id)
            .or_default()
            .native_pixels_per_point = Some(pixels_per_point);
    }

    fn set_texture(&mut self, id: TextureId, patch: ImageDelta) -> Result<(), &'static str> {
        let ImageData::Color(image) = patch.image;
        if image.pixels.is_empty()
            || image.width().checked_mul(image.height()) != Some(image.pixels.len())
        {
            return Err("invalid egui texture dimensions");
        }
        if let Some(pos) = patch.pos {
            self.textures
                .get_mut(&id)
                .ok_or("egui updated an unknown texture")?
                .patch(pos, &image)
        } else {
            self.textures.insert(
                id,
                Texture {
                    image,
                    handle: None,
                },
            );
            Ok(())
        }
    }

    /// Apply managed texture updates before the draw and retire freed handles after submission.
    pub fn paint(
        &mut self,
        context: &Context,
        bounds: Geometry,
        mut output: FullOutput,
    ) -> Result<(), &'static str> {
        let mut result = Ok(());
        for (id, patches) in mem::take(&mut output.textures_delta.set) {
            for patch in patches {
                if let Err(error) = self.set_texture(id, patch) {
                    result = Err(error);
                }
            }
        }
        let result = result.and_then(|()| {
            let primitives = context.tessellate(output.shapes, output.pixels_per_point);
            self.build(bounds, primitives)?;
            self.draw_list.draw(bounds)
        });
        for id in output.textures_delta.free.drain() {
            self.textures.remove(&id);
        }
        result
    }

    fn build(
        &mut self,
        bounds: Geometry,
        primitives: Vec<ClippedPrimitive>,
    ) -> Result<(), &'static str> {
        self.draw_list.clear();
        let surface = Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(bounds.width() as f32, bounds.height() as f32),
        );
        for item in primitives {
            let clip = item.clip_rect.intersect(surface);
            if !clip.is_positive() {
                continue;
            }
            let Primitive::Mesh(mesh) = item.primitive else {
                return Err("unsupported custom egui paint callback");
            };
            if mesh.indices.is_empty() {
                continue;
            }
            if !mesh.is_valid() || mesh.indices.len() % 3 != 0 {
                return Err("invalid egui triangle mesh");
            }
            let texture = self
                .textures
                .get_mut(&mesh.texture_id)
                .ok_or("egui mesh references an unknown texture")?
                .upload()?;
            for part in panel_meshes(mesh) {
                self.draw_list.push(part, clip, texture)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Pos2, epaint::Vertex};

    #[test]
    fn straight_rgba_and_atlas_patches_preserve_rows_and_filtered_edges() {
        let mut texture = Texture {
            image: Arc::new(ColorImage::filled([3, 3], Color32::TRANSPARENT)),
            handle: None,
        };
        let red = Color32::from_rgba_premultiplied(96, 0, 0, 128);
        let patch = ColorImage::new([1, 2], vec![red, Color32::BLUE]);
        texture.patch([1, 1], &patch).unwrap();
        assert_eq!(texture.image.pixels[4], red);
        assert_eq!(texture.image.pixels[7], Color32::BLUE);
        let texels = straight_texels(&texture.image);
        assert_eq!(texels[0], [191, 0, 0, 0]);
        assert_eq!(texels[4], [191, 0, 0, 128]);
        assert_eq!(texels[7], [0, 0, 255, 255]);
        assert_eq!(
            straight_rgba(Color32::from_additive_luminance(5)),
            [255, 255, 255, 5]
        );
        assert!(texture.patch([2, 2], &patch).is_err());
        assert_eq!(texels, straight_texels(&texture.image));
    }

    #[test]
    fn batched_meshes_keep_local_clips_indices_and_straight_feather_colors() {
        let clip = Rect::from_min_max(egui::pos2(12., 22.), egui::pos2(52., 67.));
        let mut list = DrawList::default();
        let handle = NonNull::dangling();
        let mesh = || Mesh16 {
            texture_id: TextureId::default(),
            vertices: [
                Color32::RED,
                Color32::from_black_alpha(96),
                Color32::TRANSPARENT,
            ]
            .map(|color| Vertex {
                pos: egui::pos2(13.5, 25.),
                uv: egui::pos2(0.25, 0.5),
                color,
            })
            .into(),
            indices: vec![0, 1, 2],
        };
        list.push(mesh(), clip, handle).unwrap();
        list.push(mesh(), clip, handle).unwrap();
        assert_eq!(list.calls[1].vtx_offset, 3);
        assert_eq!(list.calls[1].idx_offset, 3);
        assert_eq!(list.calls[1].element_count, 3);
        assert_eq!(list.calls[1].scissors, [12., 22., 52., 67.]);
        assert_eq!(&list.vertices[0][..4], &[13.5, 25., 0.25, 0.5]);
        assert_eq!(list.vertices[2][4].to_bits().to_le_bytes(), [255, 0, 0, 0]);
        assert_eq!(list.indices, [0, 1, 2, 0, 1, 2]);
        // Pointer coordinates use the inverse of the SDK's translate/vertical flip.
        let bounds = Geometry {
            left: -330,
            top: 500,
            right: 60,
            bottom: 350,
        };
        assert_eq!(bounds.local(-318, 478), clip.min);
    }

    #[test]
    fn valid_sparse_32_bit_triangle_is_packed_for_sdk_16_bit_indices() {
        let mut mesh = Mesh::default();
        mesh.vertices.resize(
            70001,
            Vertex {
                pos: Pos2::ZERO,
                uv: Pos2::ZERO,
                color: Color32::WHITE,
            },
        );
        mesh.vertices[70000].pos = egui::pos2(28., 36.);
        mesh.indices = vec![70000, 20, 1];
        let parts = panel_meshes(mesh);
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].indices, [0, 1, 2]);
        assert_eq!(parts[0].vertices[0].pos, egui::pos2(28., 36.));
    }
}
