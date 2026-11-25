//! SDK 4.4 panel resources and command recording. Pointers are confined to
//! each synchronous ABI call; captures contain only owned host data.
use crate::{
    panel::{Avionics, Drawing, Screen, intersection},
    state::*,
};
use ab_glyph::{Font, FontArc, ScaleFont};
use serde_json::{Value, json};
use std::{
    ffi::{c_char, c_void},
    hash::Hasher,
    mem::offset_of,
    ptr::null_mut,
};
use xphost_core::MAX_ELEMENTS;
use xplane_sdk_sys::*;

/// # Safety
/// The caller supplies the SDK record through its declared structSize.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn XPLMRegisterAvionicsCallbacksEx(
    p: *mut XPLMCustomizeAvionics_t,
) -> XPLMAvionicsID {
    if p.is_null() {
        return null_mut();
    }
    let size = unsafe { (*p).structSize };
    let content = offset_of!(XPLMCustomizeAvionics_t, contentType) as i32;
    if size < content {
        return null_mut();
    }
    let mut s = state();
    let id = s.handle();
    s.popups.insert(id, false);
    // The legacy struct ends immediately before contentType.
    if size >= content + 4 && unsafe { (*p).contentType } == xplm_WindowContentTypePanelGraphics {
        let a = unsafe {
            Avionics {
                screen: Screen::Builtin {
                    device: (*p).deviceId,
                    before: (*p).drawCallbackBefore,
                    after: (*p).drawCallbackAfter,
                },
                refcon: (*p).refcon as usize,
                name: (*p).deviceId.to_string(),
            }
        };
        s.avionics.insert(id, a);
    }
    handle(id)
}
/// # Safety
/// The caller supplies the SDK record and its NUL-terminated device ID.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn XPLMCreateAvionicsEx(p: *mut XPLMCreateAvionics_t) -> XPLMAvionicsID {
    if p.is_null() {
        return null_mut();
    }
    let size = unsafe { (*p).structSize };
    let content = offset_of!(XPLMCreateAvionics_t, contentType) as i32;
    if size < content {
        return null_mut();
    }
    let mut s = state();
    let id = s.handle();
    s.popups.insert(id, false);
    if size >= content + 4 && unsafe { (*p).contentType } == xplm_WindowContentTypePanelGraphics {
        // Read fields individually: later SDK revisions append fields here.
        let a = unsafe {
            Avionics {
                screen: Screen::Custom {
                    draw: (*p).drawCallback,
                    on_demand: (*p).drawOnDemand != 0,
                    dirty: true,
                },
                refcon: (*p).refcon as usize,
                name: string((*p).deviceID),
            }
        };
        s.avionics.insert(id, a);
    }
    handle(id)
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMDestroyAvionics(id: XPLMAvionicsID) {
    let mut s = state();
    s.popups.remove(&index(id));
    s.avionics.remove(&index(id));
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMUnregisterAvionicsCallbacks(id: XPLMAvionicsID) {
    XPLMDestroyAvionics(id);
}

// SDK buffers stay live until the call returns. Reject malformed dimensions
// before constructing a slice (even an empty Rust slice needs a valid pointer).
unsafe fn slice<'a, T>(p: *const T, n: usize, max: usize) -> Option<&'a [T]> {
    if n == 0 {
        Some(&[])
    } else if p.is_null() || n > max {
        None
    } else {
        Some(unsafe { std::slice::from_raw_parts(p, n) })
    }
}
unsafe fn image(p: *const u8, w: i32, h: i32) -> Option<Value> {
    if w <= 0 || h <= 0 {
        return None;
    }
    let n = (w as usize).checked_mul(h as usize)?.checked_mul(4)?;
    let bytes = unsafe { slice(p, n, 256 * 1024 * 1024) }?;
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    hash.write(bytes);
    Some(json!({"size":[w,h],"first_rgba":&bytes[..4],"hash":format!("{:016x}",hash.finish())}))
}
/// # Safety
/// The SDK caller supplies width * height RGBA8 pixels.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn XPLMCreateTexture(
    data: *const u8,
    width: i32,
    height: i32,
) -> *mut c_void {
    let Some(info) = (unsafe { image(data, width, height) }) else {
        return null_mut();
    };
    let mut s = state();
    let id = s.handle();
    s.panel.textures.insert(id, info);
    handle(id)
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMDestroyTexture(texture: *mut c_void) {
    state().panel.textures.remove(&index(texture));
}
/// # Safety
/// The mesh buffers and draw list must span the element counts supplied by the SDK caller.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn XPLMDrawCalls(
    mesh: *const XPLMMesh_t,
    count: i32,
    calls: *const XPLMDrawCall_t,
) {
    let mut s = state();
    let panel = &mut s.panel;
    let op = "mesh";
    if mesh.is_null() {
        panel.error(op, "missing mesh");
        return;
    }
    let mesh = unsafe { &*mesh };
    let capacity = MAX_ELEMENTS * 5;
    let Some(vertices) = usize::try_from(mesh.vertex_count)
        .ok()
        .and_then(|n| n.checked_mul(5))
        .and_then(|n| unsafe { slice(mesh.vertices, n, capacity) })
    else {
        panel.error(op, "invalid vertex buffer");
        return;
    };
    let Some(indices) = (unsafe { slice(mesh.indices, mesh.index_count as usize, MAX_ELEMENTS) })
    else {
        panel.error(op, "invalid index buffer");
        return;
    };
    let Some(calls) = (unsafe { slice(calls, count as usize, MAX_ELEMENTS) }) else {
        panel.error(op, "invalid draw list");
        return;
    };
    for c in calls {
        let Some(texture) = panel.textures.get(&index(c.tex_ref)).cloned() else {
            panel.error(op, "unknown texture");
            continue;
        };
        panel.draw(op, |d| {
            if d.transform.rotated {
                return Err("scissor under rotation");
            }
            if !c.scissors.iter().all(|v| v.is_finite()) {
                return Err("invalid scissor");
            }
            let first_index = c.idx_offset as usize;
            let len = c.element_count as usize;
            let run = first_index.checked_add(len)
                .and_then(|end| indices.get(first_index..end))
                .ok_or("index range exceeds buffer")?;
            if !len.is_multiple_of(3) {
                return Err("incomplete triangles");
            }
            let mut bounds = [f32::INFINITY, f32::INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY];
            let mut uv_bounds = bounds;
            let mut first = Value::Null;
            for &index in run {
                let index = (i64::from(index) + i64::from(c.vtx_offset)) as usize;
                let v = index.checked_mul(5)
                    .and_then(|i| vertices.get(i..))
                    .and_then(|v| v.get(..5))
                    .ok_or("vertex index outside buffer")?;
                if !v[..4].iter().all(|f| f.is_finite()) {
                    return Err("nonfinite mesh position or UV");
                }
                let [x, y] = d.transform.point([v[0], v[1]]);
                uv_bounds = [uv_bounds[0].min(v[2]), uv_bounds[1].min(v[3]),
                             uv_bounds[2].max(v[2]), uv_bounds[3].max(v[3])];
                bounds = [bounds[0].min(x), bounds[1].min(y), bounds[2].max(x), bounds[3].max(y)];
                if first.is_null() {
                    first = json!({"xy": [x, y], "uv": [v[2], v[3]], "rgba": v[4].to_bits().to_le_bytes()});
                }
            }
            let rect = d.transform.rect(c.scissors);
            let clip = d.clip.map_or(rect, |outer| intersection(rect, outer));
            if len > 0 && clip[0] < clip[2] && clip[1] < clip[3] {
                d.record(op, json!({
                    "texture": texture, "indices": len, "bounds": bounds, "uv_bounds": uv_bounds, "scissor": clip,
                    "first_vertex": first
                }));
            }
            Ok(())
        });
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMTransformPush() {
    state().panel.draw("transform push", |d| {
        d.transforms.push(d.transform);
        Ok(())
    });
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMTransformPop() {
    state().panel.draw("transform pop", |d| {
        d.transform = d.transforms.pop().ok_or("empty stack")?;
        Ok(())
    });
}
fn transform(op: &str, call: impl FnOnce(&mut super::panel::Transform)) {
    state().panel.draw(op, |d| {
        if d.transforms.is_empty() {
            return Err("transform outside push/pop");
        }
        call(&mut d.transform);
        Ok(())
    });
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMTransformTranslate(x: f32, y: f32) {
    transform("translate", |t| t.translate(x, y));
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMTransformRotate(x: f32, y: f32, angle: f32) {
    transform("rotate", |t| {
        t.translate(x, y);
        t.rotate(angle);
        t.translate(-x, -y)
    });
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMTransformScale(x: f32, y: f32) {
    if x == 0. || y == 0. {
        state().panel.error("scale", "zero factor");
        return;
    }
    transform("scale", |t| t.scale(x, y));
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMScissorPush() {
    state().panel.draw("scissor push", |d| {
        d.scissors.push(d.clip);
        Ok(())
    });
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMScissorPop() {
    state().panel.draw("scissor pop", |d| {
        d.clip = d.scissors.pop().ok_or("empty stack")?;
        Ok(())
    });
}
fn scissor(r: [i32; 4], intersect: bool) {
    state().panel.draw("scissor", |d| {
        if d.transform.rotated {
            return Err("scissor under rotation");
        }
        let rect = d.transform.rect(r.map(|i| i as f32));
        d.clip = Some(if intersect {
            d.clip.map_or(rect, |outer| intersection(rect, outer))
        } else {
            rect
        });
        Ok(())
    });
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMScissorSet(l: i32, t: i32, r: i32, b: i32) {
    scissor([l, t, r, b], false);
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMScissorIntersect(l: i32, t: i32, r: i32, b: i32) {
    scissor([l, t, r, b], true);
}

unsafe fn vertices(op: &str, color: u32, width: f32, p: *const XPLMVertex_t, n: i32, min: usize) {
    let pts = unsafe { slice(p, n as usize, MAX_ELEMENTS) };
    state().panel.draw(op, |d| {
        let pts = pts
            .filter(|pts| pts.len() >= min)
            .ok_or("invalid vertex buffer")?;
        let mapped: Vec<_> = pts.iter().map(|p| d.transform.point([p.x, p.y])).collect();
        d.record(op, json!({"color":color,"width":width,"points":mapped}));
        Ok(())
    });
}
/// # Safety
/// The SDK caller supplies count vertices.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn XPLMPolygon(color: u32, p: *const XPLMVertex_t, n: i32) {
    unsafe {
        vertices("polygon", color, 0., p, n, 3);
    }
}
/// # Safety
/// The SDK caller supplies count vertices.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn XPLMLineStripWithWidth(
    color: u32,
    width: f32,
    p: *const XPLMVertex_t,
    n: i32,
) {
    unsafe {
        vertices("line_strip", color, width, p, n, 2);
    }
}
/// # Safety
/// The SDK caller supplies count vertices.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn XPLMLinesWithWidth(
    color: u32,
    width: f32,
    p: *const XPLMVertex_t,
    n: i32,
) {
    unsafe {
        vertices("lines", color, width, p, n, 2);
    }
}
/// # Safety
/// The SDK caller supplies count vertices for a single-color quad strip.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn XPLMQuadstrip(color: u32, p: *const XPLMVertex_t, n: i32) {
    unsafe {
        vertices("quadstrip", color, 0., p, n, 4);
    }
}
/// # Safety
/// The SDK caller supplies count color vertices.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn XPLMQuadstripc(p: *const XPLMVertexColor_t, n: i32) {
    let pts = unsafe { slice(p, n as usize, MAX_ELEMENTS) };
    state().panel.draw("quadstrip", |d| {
        let pts = pts.ok_or("invalid vertex buffer")?;
        let mapped: Vec<_> = pts
            .iter()
            .map(|p| json!([d.transform.point([p.x, p.y]), p.color]))
            .collect();
        d.record("quadstrip", json!({"points":mapped}));
        Ok(())
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn XPLMCreateFont(_charset: XPLMCharSet_t) -> XPLMFontHandle {
    let mut s = state();
    let id = s.handle();
    s.panel_fonts.insert(id, Vec::new());
    handle(id)
}
/// # Safety
/// The SDK supplies a NUL-terminated path to an installed face.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn XPLMFontAddFace(font: XPLMFontHandle, path: *const c_char) -> i32 {
    let path = state().asset_path(&unsafe { string(path) });
    let Ok(bytes) = std::fs::read(path) else {
        return 0;
    };
    let Ok(face) = FontArc::try_from_vec(bytes) else {
        return 0;
    };
    let mut s = state();
    let Some(font) = s.panel_fonts.get_mut(&index(font)) else {
        return 0;
    };
    font.push(face);
    1
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMDestroyFont(font: XPLMFontHandle) {
    state().panel_fonts.remove(&index(font));
}
fn measure(faces: &[FontArc], size: f32, text: &str) -> f32 {
    let mut width = 0.;
    let mut previous = None;
    for ch in text.chars() {
        let face = faces
            .iter()
            .position(|f| f.glyph_id(ch).0 != 0)
            .unwrap_or(0);
        let Some(f) = faces.get(face) else { break };
        let f = f.as_scaled(size);
        let glyph = f.glyph_id(ch);
        if let Some((last, id)) = previous
            && last == face
        {
            width += f.kern(id, glyph);
        }
        width += f.h_advance(glyph);
        previous = Some((face, glyph));
    }
    width
}
/// # Safety
/// The SDK supplies a NUL-terminated UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn XPLMFontMeasureString(
    font: XPLMFontHandle,
    size: f32,
    text: *const c_char,
) -> f32 {
    let text = unsafe { string(text) };
    state()
        .panel_fonts
        .get(&index(font))
        .map_or(0., |f| measure(f, size, &text))
}
/// # Safety
/// The SDK supplies a writable metrics record carrying its size.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn XPLMFontGetMetrics(
    font: XPLMFontHandle,
    size: f32,
    out: *mut XPLMFontMetrics_t,
) {
    if out.is_null() || unsafe { (*out).structSize } < size_of::<XPLMFontMetrics_t>() as i32 {
        return;
    }
    let s = state();
    let metrics = s
        .panel_fonts
        .get(&index(font))
        .and_then(|f| f.first())
        .map(|f| {
            let scale = f.as_scaled(size);
            (
                scale.height() + scale.line_gap(),
                scale.ascent(),
                -scale.descent(),
            )
        })
        .unwrap_or_default();
    unsafe {
        ((*out).lineHeight, (*out).lineAscent, (*out).lineDescent) = metrics;
    }
}
enum TextLayout {
    Proportional,
    Fixed(i32),
    Wrapped(i32),
}
fn text_run(
    font: XPLMFontHandle,
    color: u32,
    size: f32,
    xy: [f32; 2],
    text: String,
    justify: i32,
    layout: TextLayout,
) {
    let mut s = state();
    let Some(faces) = s.panel_fonts.get(&index(font)) else {
        s.panel.error("text", "unknown font");
        return;
    };
    if faces.is_empty() {
        return;
    }
    let width = measure(faces, size, &text);
    let (spacing, wrap) = match layout {
        TextLayout::Proportional => (None, None),
        TextLayout::Fixed(spacing) => (Some(spacing), None),
        TextLayout::Wrapped(width) => (None, Some(width)),
    };
    s.panel.draw("text", |d| {
        d.record(
            "text",
            json!({
                "text": text, "size": size, "color": color, "anchor": d.transform.point(xy),
                "advance": width, "justify": justify, "spacing": spacing, "wrap": wrap
            }),
        );
        Ok(())
    });
}
/// # Safety
/// The SDK supplies a NUL-terminated text run.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn XPLMFontDrawString(
    font: XPLMFontHandle,
    color: u32,
    size: f32,
    x: f32,
    y: f32,
    text: *const c_char,
    justify: i32,
) {
    text_run(
        font,
        color,
        size,
        [x, y],
        unsafe { string(text) },
        justify,
        TextLayout::Proportional,
    );
}
/// # Safety
/// The SDK supplies a NUL-terminated text run.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn XPLMFontDrawStringFixedSpacing(
    font: XPLMFontHandle,
    color: u32,
    size: f32,
    x: f32,
    y: f32,
    text: *const c_char,
    spacing: i32,
    justify: i32,
) {
    text_run(
        font,
        color,
        size,
        [x, y],
        unsafe { string(text) },
        justify,
        TextLayout::Fixed(spacing),
    );
}
/// # Safety
/// The SDK supplies a NUL-terminated text run.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn XPLMFontDrawStringWordWrapped(
    font: XPLMFontHandle,
    color: u32,
    size: f32,
    x: f32,
    y: f32,
    text: *const c_char,
    width: i32,
    justify: i32,
) {
    text_run(
        font,
        color,
        size,
        [x, y],
        unsafe { string(text) },
        justify,
        TextLayout::Wrapped(width),
    );
}

#[unsafe(no_mangle)]
pub extern "C" fn XPLMCreateTextureAtlas() -> XPLMTextureAtlasRef {
    let mut s = state();
    let id = s.handle();
    s.panel_atlases.insert(id, (Vec::new(), false));
    handle(id)
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMDestroyTextureAtlas(atlas: XPLMTextureAtlasRef) {
    state().panel_atlases.remove(&index(atlas));
}
/// # Safety
/// The caller supplies w*h RGBA8 pixels.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn XPLMTextureAtlasAddImage(
    atlas: XPLMTextureAtlasRef,
    p: *const u8,
    w: i32,
    h: i32,
) -> i32 {
    unsafe { XPLMTextureAtlasAddImageSet(atlas, p, w, h, 1, 1) }
}
/// # Safety
/// The caller supplies w*h RGBA8 pixels, subdivided into a grid in row-major order.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn XPLMTextureAtlasAddImageSet(
    atlas: XPLMTextureAtlasRef,
    p: *const u8,
    w: i32,
    h: i32,
    columns: i32,
    rows: i32,
) -> i32 {
    let mut s = state();
    let Some((images, false)) = s.panel_atlases.get_mut(&index(atlas)) else {
        return -1;
    };
    if columns <= 0 || rows <= 0 || w % columns != 0 || h % rows != 0 || w < columns || h < rows {
        return -1;
    }
    let Some(count) = columns
        .checked_mul(rows)
        .filter(|&n| n as usize <= MAX_ELEMENTS)
    else {
        return -1;
    };
    let Some(info) = (unsafe { image(p, w, h) }) else {
        return -1;
    };
    let first = images.len() as i32;
    for n in 0..count {
        images.push(json!({"size":[w/columns,h/rows],"cell":[n%columns,n/columns],"source_hash":info["hash"]}));
    }
    first
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMTextureAtlasBake(atlas: XPLMTextureAtlasRef) {
    if let Some((_, baked)) = state().panel_atlases.get_mut(&index(atlas)) {
        *baked = true;
    }
}
fn atlas_draw(
    op: &str,
    atlas: XPLMTextureAtlasRef,
    idx: i32,
    color: u32,
    call: impl FnOnce(&Drawing) -> Value,
) {
    let mut s = state();
    let Some((images, true)) = s.panel_atlases.get(&index(atlas)) else {
        s.panel.error(op, "unknown or unbaked atlas");
        return;
    };
    let Some(info) = images.get(idx as usize).cloned() else {
        s.panel.error(op, "unknown image");
        return;
    };
    s.panel.draw(op, |d| {
        d.record(
            op,
            json!({"index":idx,"image":info,"color":color,"geometry":call(d)}),
        );
        Ok(())
    });
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMTextureAtlasDrawAt(
    atlas: XPLMTextureAtlasRef,
    idx: i32,
    color: u32,
    x: f32,
    y: f32,
) {
    atlas_draw("atlas_at", atlas, idx, color, |d| {
        json!(d.transform.point([x, y]))
    });
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMTextureAtlasDrawIn(
    atlas: XPLMTextureAtlasRef,
    idx: i32,
    color: u32,
    l: f32,
    t: f32,
    r: f32,
    b: f32,
) {
    atlas_draw("atlas_in", atlas, idx, color, |_| json!([l, t, r, b]));
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMTextureAtlasDrawStretched(
    atlas: XPLMTextureAtlasRef,
    idx: i32,
    color: u32,
    l: f32,
    t: f32,
    r: f32,
    b: f32,
) {
    atlas_draw("atlas_9slice", atlas, idx, color, |_| json!([l, t, r, b]));
}
/// # Safety
/// The caller supplies n texture vertices.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn XPLMTextureAtlasDrawMesh(
    atlas: XPLMTextureAtlasRef,
    idx: i32,
    color: u32,
    p: *const XPLMTextureVertex_t,
    n: i32,
) {
    let Some(pts) = (unsafe { slice(p, n as usize, MAX_ELEMENTS) }) else {
        state().panel.error("atlas_mesh", "invalid vertices");
        return;
    };
    atlas_draw("atlas_mesh", atlas, idx, color, |d| {
        Value::Array(
            pts.iter()
                .map(|p| json!([d.transform.point([p.x, p.y]), [p.s, p.t]]))
                .collect(),
        )
    });
}
