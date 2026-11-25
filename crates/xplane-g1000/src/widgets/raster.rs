//! Software readback of the same retained meshes, for offline pixel comparisons.
use crate::panel::Batch;
use egui::epaint::Vertex;
use egui::{Color32, ColorImage, Pos2, Rect, pos2};
use std::sync::OnceLock;

pub(crate) fn paint(size: [u16; 2], batches: &[Batch], pixels: &mut [u8]) {
    let stride = usize::from(size[0]);
    let screen = Rect::from_min_max(Pos2::ZERO, pos2(size[0].into(), size[1].into()));
    for b in batches {
        let mesh = &b.mesh;
        let tex = &b.atlas.image;
        let clip = screen.intersect(b.clip);
        let mut indices = mesh.indices.as_slice();
        while indices.len() >= 3 {
            if let [a, b, c, d, e, f, ..] = *indices
                && c == d
                && b == e
            {
                let corners = [a, b, c, f].map(|i| mesh.vertices[i as usize]);
                if quad(pixels, stride, clip, tex, corners) {
                    indices = &indices[6..];
                    continue;
                }
            }
            let vertices = [indices[0], indices[1], indices[2]].map(|i| mesh.vertices[i as usize]);
            triangle(pixels, stride, clip, tex, vertices);
            indices = &indices[3..];
        }
    }
}

// A texel is owned by the primitive containing its center. The far edge is exclusive.
fn bounds(rect: Rect, clip: Rect) -> [usize; 4] {
    let r = rect.intersect(clip);
    [r.min.x, r.min.y, r.max.x, r.max.y].map(|v| (v - 0.5).ceil() as usize)
}
fn sample(tex: &ColorImage, uv: Pos2) -> Color32 {
    let x = ((uv.x * tex.width() as f32) as usize).min(tex.width() - 1);
    let y = ((uv.y * tex.height() as f32) as usize).min(tex.height() - 1);
    tex.pixels[y * tex.width() + x]
}
struct Transfer {
    linear: [f32; 256],
    encoded: [u8; 4097],
}
fn transfer() -> &'static Transfer {
    static TABLE: OnceLock<Transfer> = OnceLock::new();
    TABLE.get_or_init(|| Transfer {
        linear: std::array::from_fn(|i| {
            let v = i as f32 / 255.;
            if v <= 0.04045 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        }),
        encoded: std::array::from_fn(|i| {
            let v = i as f32 / 4096.;
            (255.
                * if v <= 0.0031308 {
                    12.92 * v
                } else {
                    1.055 * v.powf(1. / 2.4) - 0.055
                })
            .round() as u8
        }),
    })
}

impl Transfer {
    fn blend(&self, dst: u8, src: u8, alpha: u8) -> u8 {
        let d = self.linear[dst as usize];
        self.encoded[((d + (self.linear[src as usize] - d) * (f32::from(alpha) / 255.)) * 4096.)
            .round() as usize]
    }
}

fn shade(dst: &mut [u8; 4], ink: [u8; 4], texel: Color32) {
    let [r, g, b, a] = texel.to_srgba_unmultiplied();
    let mul = |x: u8, y: u8| ((u16::from(x) * u16::from(y) + 127) / 255) as u8;
    let alpha = mul(a, ink[3]);
    if alpha == 0 {
        return;
    }
    let src = [mul(b, ink[2]), mul(g, ink[1]), mul(r, ink[0]), alpha];
    if alpha == 255 || dst[3] == 0 {
        *dst = src;
        return;
    }
    let table = transfer();
    let total = u32::from(alpha) * 255 + u32::from(dst[3]) * u32::from(255 - alpha);
    let weight = ((u32::from(alpha) * 65025 + total / 2) / total) as u8;
    dst[3] = ((total + 127) / 255) as u8;
    for i in 0..3 {
        dst[i] = table.blend(dst[i], src[i], weight);
    }
}

// Text cells, artwork and pixel-snapped gauge bars dominate the page.
fn quad(
    p: &mut [u8],
    stride: usize,
    clip: Rect,
    tex: &ColorImage,
    [a, b, c, d]: [Vertex; 4],
) -> bool {
    if a.pos.y != b.pos.y
        || a.pos.x != c.pos.x
        || d.pos.x != b.pos.x
        || d.pos.y != c.pos.y
        || a.uv.y != b.uv.y
        || a.uv.x != c.uv.x
        || d.uv.x != b.uv.x
        || d.uv.y != c.uv.y
        || [b, c, d].iter().any(|v| v.color != a.color)
        || a.pos.x >= d.pos.x
        || a.pos.y >= d.pos.y
    {
        return false;
    }
    let [l, t, r, bot] = bounds(Rect::from_min_max(a.pos, d.pos), clip);
    if l >= r || t >= bot {
        return true;
    }
    let pixels = p.as_chunks_mut::<4>().0;
    let ink = a.color.to_srgba_unmultiplied();
    if a.uv == d.uv && ink[3] == 255 && sample(tex, a.uv) == Color32::WHITE {
        let color = [ink[2], ink[1], ink[0], 255];
        for y in t..bot {
            pixels[y * stride + l..y * stride + r].fill(color);
        }
    } else {
        let step = (d.uv - a.uv) / (d.pos - a.pos);
        for y in t..bot {
            let v = a.uv.y + (y as f32 + 0.5 - a.pos.y) * step.y;
            for x in l..r {
                let u = a.uv.x + (x as f32 + 0.5 - a.pos.x) * step.x;
                shade(&mut pixels[y * stride + x], ink, sample(tex, pos2(u, v)));
            }
        }
    }
    true
}
fn edge(a: Pos2, b: Pos2, p: Pos2) -> f32 {
    (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x)
}
fn triangle(p: &mut [u8], stride: usize, clip: Rect, tex: &ColorImage, mut v: [Vertex; 3]) {
    let area = edge(v[0].pos, v[1].pos, v[2].pos);
    if area == 0. || !area.is_finite() {
        return;
    }
    if area < 0. {
        v.swap(1, 2);
    }
    let [a, b, c] = v.map(|v| v.pos);
    let [l, t, r, bot] = bounds(Rect::from_min_max(a.min(b).min(c), a.max(b).max(c)), clip);
    let pairs = [(b, c), (c, a), (a, b)];
    let inclusive = pairs.map(|(p, q)| q.y < p.y || (q.y == p.y && q.x > p.x));
    let colors = v.map(|p| p.color.to_array());
    let pixels = p.as_chunks_mut::<4>().0;
    for y in t..bot {
        for x in l..r {
            let pt = pos2(x as f32 + 0.5, y as f32 + 0.5);
            let edges = pairs.map(|(p, q)| edge(p, q, pt));
            if (0..3).any(|i| edges[i] < 0. || (edges[i] == 0. && !inclusive[i])) {
                continue;
            }
            let w = edges.map(|e| e / area.abs());
            let channel = |i: usize| {
                (w[0] * f32::from(colors[0][i])
                    + w[1] * f32::from(colors[1][i])
                    + w[2] * f32::from(colors[2][i]))
                .round() as u8
            };
            let ink =
                Color32::from_rgba_premultiplied(channel(0), channel(1), channel(2), channel(3))
                    .to_srgba_unmultiplied();
            let uv =
                (v[0].uv.to_vec2() * w[0] + v[1].uv.to_vec2() * w[1] + v[2].uv.to_vec2() * w[2])
                    .to_pos2();
            shade(&mut pixels[y * stride + x], ink, sample(tex, uv));
        }
    }
}
