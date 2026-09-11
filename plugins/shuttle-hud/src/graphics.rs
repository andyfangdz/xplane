//! The plugin's only OpenGL boundary. Called exclusively by X-Plane's draw
//! callback, with a current compatibility context supplied by its GL bridge.
#![forbid(unsafe_code)]
use crate::{
    config::Optics,
    math::View,
    runway::CameraProjection,
    scene::{Scene, Segment},
};
use xplane_plugin::opengl::{Attribute, Capability, DrawContext, Matrix, Primitive};

pub fn clear_panel(gl: &mut DrawContext, optics: Optics) {
    xplane_plugin::set_graphics_state(false);
    gl.attributes(&[Attribute::Enable], |gl| {
        gl.enable(Capability::Scissor, false);
        gl.enable(Capability::Stencil, false);
        gl.enable(Capability::CullFace, false);
        gl.disable_clip_planes();
        gl.color([0.0, 0.0, 0.0, 0.0]);
        let (l, b, s) = (optics.panel_left, optics.panel_bottom, optics.panel_size);
        gl.vertices(Primitive::Quads, |vertices| {
            vertices.vertex(l, b);
            vertices.vertex(l + s, b);
            vertices.vertex(l + s, b + s);
            vertices.vertex(l, b + s);
        });
    });
}

#[allow(clippy::too_many_arguments)] // Calibrated view and display inputs stay explicit.
pub fn paint(
    gl: &mut DrawContext,
    scene: &Scene,
    optics: Optics,
    camera: View,
    runway_camera: Option<&CameraProjection>,
    panel: bool,
    size: (i32, i32),
    brightness: f32,
) {
    let (w, h) = size;
    let scale = f64::from(h) / 1080.0;
    xplane_plugin::set_graphics_state(true);
    gl.attributes(
        &[Attribute::Current, Attribute::Scissor, Attribute::Enable],
        |gl| {
            gl.enable(Capability::CullFace, false);
            gl.disable_clip_planes();
            gl.enable(Capability::AlphaTest, false);
            gl.enable(Capability::Stencil, false);
            gl.matrix(Matrix::Projection, |gl| {
                if !panel {
                    gl.load_identity();
                    gl.ortho(f64::from(w), f64::from(h));
                }
                gl.matrix(Matrix::Modelview, |gl| {
                    if panel {
                        gl.enable(Capability::Scissor, false);
                        gl.translate(optics.panel_left, optics.panel_bottom + optics.panel_size);
                        gl.scale(optics.panel_size / 960.0, -optics.panel_size / 960.0);
                        gl.translate(-480.0, -100.0);
                    } else {
                        gl.load_identity();
                        gl.translate((f64::from(w) - 1920.0 * scale) / 2.0, f64::from(h));
                        gl.scale(scale, -scale);
                        gl.enable(Capability::Scissor, true);
                    }
                    let viewport = gl.viewport();
                    if !panel {
                        gl.scissor(viewport);
                    }
                    if panel {
                        gl.enable(Capability::Blend, false);
                    }
                    for (group, segments, conformal) in std::iter::once((0, &scene.conformal, true))
                        .chain(
                            scene
                                .layers
                                .iter()
                                .enumerate()
                                .map(|(group, segments)| (group, segments, false)),
                        )
                    {
                        for pass in 0..2 {
                            if panel && pass == 0 {
                                continue;
                            }
                            if pass != 0 {
                                gl.color([
                                    if panel { 0.25 * brightness } else { 0.74 },
                                    if panel { brightness } else { 0.84 },
                                    if panel { 0.40 * brightness } else { 0.64 },
                                    brightness,
                                ]);
                            } else {
                                gl.color([0.025, 0.045, 0.015, brightness * 0.32]);
                            }
                            gl.vertices(Primitive::Quads, |vertices| {
                                for &segment in segments {
                                    let Some(mut segment) = segment.clipped(
                                        482.0,
                                        102.0,
                                        1438.0,
                                        if group == 0 { 835.0 } else { 1005.0 },
                                    ) else {
                                        continue;
                                    };
                                    if !panel {
                                        if conformal {
                                            let Some((a, b)) = runway_camera.and_then(|camera| {
                                                camera
                                                    .point(optics, segment.a)
                                                    .zip(camera.point(optics, segment.b))
                                            }) else {
                                                continue;
                                            };
                                            segment = Segment { a, b };
                                        } else {
                                            let a = scene.body.camera_point(camera, segment.a);
                                            let b = scene.body.camera_point(camera, segment.b);
                                            if a.limited || b.limited {
                                                continue;
                                            }
                                            segment = Segment { a: a.p, b: b.p };
                                        }
                                    }
                                    if let Some(points) = segment.quad(if pass != 0 {
                                        if panel {
                                            4.6
                                        } else {
                                            2.1
                                        }
                                    } else {
                                        3.2
                                    }) {
                                        for p in points {
                                            vertices.vertex(p.x, p.y);
                                        }
                                    }
                                }
                            });
                        }
                    }
                });
            });
        },
    );
}
