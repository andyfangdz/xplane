//! Submit the HUD's 1920 x 1080 design coordinates to the native window painter.
use crate::{
    font::FontAtlas,
    scene::{Color, Draw, Scene},
};
use poweroff180::guidance::{PI, rad};
use xplane_hud::{Point, Segment, point};
use xplane_plugin::Bounds;
use xplane_sdk_sys as sdk;

#[cfg(test)]
mod tests;

pub struct Graphics {
    font: FontAtlas,
}

fn color(c: Color) -> u32 {
    u32::from_le_bytes(c.map(|v| (255. * v.clamp(0., 1.)).round() as u8))
}

fn polygon(points: &[Point], tint: u32) {
    if points.len() >= 3 {
        let mut vertices: Vec<_> = points
            .iter()
            .map(|p| sdk::XPLMVertex_t {
                x: p.x as f32,
                y: -p.y as f32,
            })
            .collect();
        // Mirrored flight-director bars and tape pointers can have opposite
        // winding. Submit all convex fills counter-clockwise in panel space.
        if vertices
            .iter()
            .zip(vertices.iter().cycle().skip(1))
            .take(vertices.len())
            .map(|(a, b)| a.x * b.y - a.y * b.x)
            .sum::<f32>()
            < 0.
        {
            vertices.reverse();
        }
        // Called only by the native window's draw handler. X-Plane copies the points.
        unsafe { sdk::XPLMPolygon(tint, vertices.as_ptr(), vertices.len() as i32) }
    }
}
fn stroke(a: Point, b: Point, tint: u32, width: f64) {
    if let Some(quad) = (Segment { a, b }).quad(width) {
        polygon(&quad, tint);
    }
}
fn outline(points: &[Point], tint: u32, width: f64, closed: bool) {
    if closed && let (Some(a), Some(b)) = (points.last(), points.first()) {
        stroke(*a, *b, tint, width);
    }
    for pair in points.windows(2) {
        stroke(pair[0], pair[1], tint, width);
    }
}

// Intersect in design coordinates. A single SDK scissor level suffices at any nesting depth.
type Clip = [i32; 4]; // left, top, right, bottom in SDK (y-up) coordinates
fn intersect(a: Clip, b: Clip) -> Clip {
    let left = a[0].max(b[0]);
    let top = a[1].min(b[1]);
    [left, top, a[2].min(b[2]).max(left), a[3].max(b[3]).min(top)]
}
const FIELD: Clip = [0, 0, 1920, -1080];

fn draw_clipped(commands: &[Draw], mut paint: impl FnMut(&Draw, Clip)) -> bool {
    let mut clips = vec![FIELD];
    for command in commands {
        match command {
            Draw::Clip { x, y, w, h } => {
                let clip = intersect(
                    *clips.last().unwrap(),
                    [
                        x.floor() as i32,
                        (-y).ceil() as i32,
                        (x + w).ceil() as i32,
                        (-y - h).floor() as i32,
                    ],
                );
                clips.push(clip);
            }
            Draw::Unclip if clips.len() > 1 => {
                clips.pop();
            }
            Draw::Unclip => {
                return false;
            }
            _ => {
                paint(command, *clips.last().unwrap());
            }
        }
    }
    clips.len() == 1
}

impl Graphics {
    pub fn new() -> Option<Self> {
        Some(Self {
            font: FontAtlas::new()?,
        })
    }
    pub fn paint(&self, scene: &Scene, bounds: Bounds) -> bool {
        if bounds.width() <= 0 || bounds.height() <= 0 {
            return false;
        }
        unsafe {
            sdk::XPLMTransformPush();
            sdk::XPLMTransformTranslate(bounds.left as f32, bounds.top as f32);
            sdk::XPLMTransformScale(
                bounds.width() as f32 / 1920.,
                bounds.height() as f32 / 1080.,
            );
            sdk::XPLMScissorPush();
        }
        let mut active = FIELD;
        let valid = draw_clipped(&scene.commands, |command, clip| {
            if clip != active {
                let [l, t, r, b] = clip;
                // Recover the window's clip before intersecting: a following draw
                // may return to a larger parent clip or the complete field.
                unsafe {
                    sdk::XPLMScissorPop();
                    sdk::XPLMScissorPush();
                    sdk::XPLMScissorIntersect(l, t, r, b);
                }
                active = clip;
            }
            self.command(command);
        });
        unsafe {
            sdk::XPLMScissorPop();
            sdk::XPLMTransformPop();
        }
        valid
    }
    fn command(&self, command: &Draw) {
        match command {
            Draw::Line(a, b, c, w) => stroke(*a, *b, color(*c), *w),
            Draw::Polygon(points, c, fill, w) => {
                if *fill {
                    polygon(points, color(*c));
                } else {
                    outline(points, color(*c), *w, true);
                }
            }
            Draw::Circle(center, r, c, w, fill) => {
                let points: Vec<_> = (0..48)
                    .map(|i| {
                        let a = f64::from(i) * 2. * PI / 48.;
                        point(center.x + r * a.cos(), center.y + r * a.sin())
                    })
                    .collect();
                if *fill {
                    polygon(&points, color(*c));
                } else {
                    outline(&points, color(*c), *w, true);
                }
            }
            Draw::Arc(center, r, from, to, c, w) => {
                let count = (((to - from).abs() / 3.) as i32).max(2);
                let points: Vec<_> = (0..=count)
                    .map(|i| {
                        let a = rad(from + (to - from) * f64::from(i) / f64::from(count));
                        point(center.x + r * a.cos(), center.y + r * a.sin())
                    })
                    .collect();
                outline(&points, color(*c), *w, false);
            }
            Draw::Text {
                at,
                text,
                size,
                color: c,
                anchor,
                rotation,
            } => {
                if let Some(angle) = rotation {
                    unsafe {
                        sdk::XPLMTransformPush();
                        sdk::XPLMTransformTranslate(at.x as f32, -at.y as f32);
                        sdk::XPLMTransformRotate(0., 0., -*angle as f32);
                    }
                    self.text(point(0., -14.), text, *size, *c, *anchor);
                    unsafe { sdk::XPLMTransformPop() };
                } else {
                    self.text(*at, text, *size, *c, *anchor);
                }
            }
            Draw::Clip { .. } | Draw::Unclip => unreachable!("handled by the clip stack"),
        }
    }
    fn text(&self, at: Point, text: &str, size: f64, tint: Color, anchor: u8) {
        let glyph = |ch: u8| {
            if (32..128).contains(&ch) {
                usize::from(ch - 32)
            } else {
                31
            }
        };
        let scale = size / 44.;
        let width = text
            .bytes()
            .map(|v| self.font.advance(glyph(v)) * scale)
            .sum::<f64>();
        let start = at.x - width * f64::from(anchor) * 0.5;
        for (offset, tint) in [(1.2, color([0., 0., 0., 0.8])), (0., color(tint))] {
            let mut x = start + offset;
            for ch in text.bytes() {
                let i = glyph(ch);
                self.font.draw_glyph(
                    i,
                    tint,
                    x - 2. * scale,
                    at.y - 2. * scale + offset,
                    64. * scale,
                );
                x += self.font.advance(i) * scale;
            }
        }
    }
}
