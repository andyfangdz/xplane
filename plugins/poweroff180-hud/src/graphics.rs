//! Safe rendering of the SR20 display commands in X-Plane's borrowed context.
//! The native font atlas and its resource lifetime are owned by `font.rs`.
#![cfg_attr(not(test), forbid(unsafe_code))]
use crate::{
    font::FontAtlas,
    scene::{Color, Draw, Scene},
};
use poweroff180::guidance::{rad, PI};
use xplane_hud::{point, Point, Segment};
use xplane_plugin::opengl::{Attribute, Capability, DrawContext, Matrix, Primitive, Vertices};
use xplane_plugin::{set_graphics_state, set_textured_graphics_state};

#[cfg(test)]
mod tests;

#[derive(Default)]
pub struct Graphics {
    font: FontAtlas,
}
impl Graphics {
    pub fn paint(&mut self, gl: &mut DrawContext, scene: &Scene, size: (i32, i32)) -> bool {
        let (w, h) = size;
        if w <= 0 || h <= 0 {
            return false;
        }
        set_textured_graphics_state();
        if !self.font.ensure(gl) {
            return false;
        }
        set_graphics_state(true);
        gl.attributes(
            &[Attribute::Line, Attribute::Current, Attribute::Scissor],
            |gl| {
                gl.enable(Capability::Scissor, false);
                gl.matrix(Matrix::Modelview, |gl| {
                    gl.translate(0.0, f64::from(h));
                    gl.scale(f64::from(w) / 1920.0, -f64::from(h) / 1080.0);
                    let viewport = gl.viewport();
                    self.commands(gl, &mut scene.commands.iter(), viewport, false)
                })
                .flatten()
            },
        )
        .flatten()
        .is_some()
    }

    // Consume each nested clip inside a scope. Even malformed command lists
    // cannot pop an outer GL stack entry or leave a primitive open.
    fn commands(
        &self,
        gl: &mut DrawContext,
        commands: &mut std::slice::Iter<'_, Draw>,
        viewport: [i32; 4],
        clipped: bool,
    ) -> Option<()> {
        while let Some(command) = commands.next() {
            match command {
                Draw::Line(a, b, c, width) => {
                    gl.color(*c);
                    gl.vertices(Primitive::Quads, |vertices| {
                        stroke(vertices, *a, *b, *width)
                    });
                }
                Draw::Polygon(points, c, fill, width) => {
                    gl.color(*c);
                    gl.vertices(
                        if *fill {
                            Primitive::Polygon
                        } else {
                            Primitive::Quads
                        },
                        |vertices| {
                            if *fill {
                                for p in points {
                                    vertices.vertex(p.x, p.y);
                                }
                            } else if let Some(mut previous) = points.last().copied() {
                                for p in points {
                                    stroke(vertices, previous, *p, *width);
                                    previous = *p;
                                }
                            }
                        },
                    );
                }
                Draw::Circle(center, r, c, width, fill) => {
                    gl.color(*c);
                    gl.vertices(
                        if *fill {
                            Primitive::Polygon
                        } else {
                            Primitive::Quads
                        },
                        |vertices| {
                            let mut previous = point(center.x + r, center.y);
                            for i in 1..=48 {
                                let a = f64::from(i) * 2.0 * PI / 48.0;
                                let p = point(center.x + r * a.cos(), center.y + r * a.sin());
                                if *fill {
                                    vertices.vertex(p.x, p.y);
                                } else {
                                    stroke(vertices, previous, p, *width);
                                }
                                previous = p;
                            }
                        },
                    );
                }
                Draw::Arc(center, r, from, to, c, width) => {
                    gl.color(*c);
                    gl.vertices(Primitive::Quads, |vertices| {
                        let count = (((to - from).abs() / 3.0) as i32).max(2);
                        let mut previous = point(
                            center.x + r * rad(*from).cos(),
                            center.y + r * rad(*from).sin(),
                        );
                        for i in 1..=count {
                            let a = rad(from + (to - from) * f64::from(i) / f64::from(count));
                            let p = point(center.x + r * a.cos(), center.y + r * a.sin());
                            stroke(vertices, previous, p, *width);
                            previous = p;
                        }
                    });
                }
                Draw::Text {
                    at,
                    text,
                    size,
                    color,
                    anchor,
                    rotation,
                } => {
                    if let Some(angle) = rotation {
                        gl.matrix(Matrix::Modelview, |gl| {
                            gl.translate(at.x, at.y);
                            gl.rotate(*angle);
                            self.text(gl, point(0.0, -14.0), text, *size, *color, *anchor);
                        })?;
                    } else {
                        self.text(gl, *at, text, *size, *color, *anchor);
                    }
                }
                Draw::Clip { x, y, w, h } => {
                    gl.attributes(&[Attribute::Scissor], |gl| {
                        gl.enable(Capability::Scissor, true);
                        gl.scissor([
                            viewport[0] + (x * f64::from(viewport[2]) / 1920.0) as i32,
                            viewport[1]
                                + ((1080.0 - y - h) * f64::from(viewport[3]) / 1080.0) as i32,
                            (w * f64::from(viewport[2]) / 1920.0) as i32,
                            (h * f64::from(viewport[3]) / 1080.0) as i32,
                        ]);
                        self.commands(gl, commands, viewport, true)
                    })
                    .flatten()?;
                }
                Draw::Unclip => return clipped.then_some(()),
            }
        }
        (!clipped).then_some(())
    }

    fn text(&self, gl: &mut DrawContext, at: Point, text: &str, size: f64, c: Color, anchor: u8) {
        let index = |ch: u8| {
            if (32..128).contains(&ch) {
                usize::from(ch - 32)
            } else {
                31
            }
        };
        let width: f64 = text
            .bytes()
            .map(|ch| f64::from(self.font.advance(index(ch))) * size / 44.0)
            .sum();
        let scale = size / 44.0;
        let start = at.x - width * f64::from(anchor) * 0.5;
        set_textured_graphics_state();
        self.font.bind(gl);
        for pass in 0..2 {
            let offset = if pass == 0 { 1.2 } else { 0.0 };
            let mut x = start + offset;
            gl.color(if pass == 0 { [0.0, 0.0, 0.0, 0.8] } else { c });
            gl.vertices(Primitive::Quads, |vertices| {
                for ch in text.bytes() {
                    let i = index(ch);
                    let u = (i % 16) as f64 * 64.0 / 1024.0;
                    let v = (i / 16) as f64 * 64.0 / 384.0;
                    let l = x - 2.0 * scale;
                    let t = at.y - 2.0 * scale + offset;
                    let r = l + 64.0 * scale;
                    let b = t + 64.0 * scale;
                    vertices.tex_coord(u, v);
                    vertices.vertex(l, t);
                    vertices.tex_coord(u + 64.0 / 1024.0, v);
                    vertices.vertex(r, t);
                    vertices.tex_coord(u + 64.0 / 1024.0, v + 64.0 / 384.0);
                    vertices.vertex(r, b);
                    vertices.tex_coord(u, v + 64.0 / 384.0);
                    vertices.vertex(l, b);
                    x += f64::from(self.font.advance(i)) * scale;
                }
            });
        }
        set_graphics_state(true);
    }
}
fn stroke(vertices: &mut Vertices, a: Point, b: Point, width: f64) {
    if let Some(points) = (Segment { a, b }).quad(width) {
        for p in points {
            vertices.vertex(p.x, p.y);
        }
    }
}
