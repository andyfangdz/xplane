//! Native SDK 4.4 drawing for the collimated cockpit texture and desktop overlay.
use crate::{
    config::Optics,
    math::View,
    runway::CameraProjection,
    scene::{FixedMark, Scene, Segment},
};
use xplane_plugin::{Bounds, panel_ui::RetainedDrawing};
use xplane_sdk_sys as sdk;

fn color(c: [f32; 4]) -> u32 {
    u32::from_le_bytes(c.map(|v| (v.clamp(0., 1.) * 255.).round() as u8))
}
pub fn clear_device() {
    // The collimator samples the emissive (night) panel texture. Zero radiance
    // clears the previous marks, including on the first frame after power-off.
    let quad =
        [(0., 0.), (1024., 0.), (1024., 768.), (0., 768.)].map(|(x, y)| sdk::XPLMVertex_t { x, y });
    unsafe {
        sdk::XPLMPolygon(color([0., 0., 0., 1.]), quad.as_ptr(), 4);
    }
}
struct Placement {
    left: f64,
    top: f64,
    scale: f64,
}
impl Placement {
    fn new(window: Option<Bounds>, optics: Optics) -> Self {
        if let Some(bounds) = window {
            let scale = f64::from(bounds.height()) / 1080.;
            Self {
                left: f64::from(bounds.left) + (f64::from(bounds.width()) - 1920. * scale) / 2.,
                top: f64::from(bounds.top),
                scale,
            }
        } else {
            let scale = optics.panel_size / 960.;
            // The G1000 MFD instrument's lower-left coincides with the HUD
            // source rectangle's lower-left in install_native.py.
            Self {
                left: -480. * scale,
                top: optics.panel_size + 100. * scale,
                scale,
            }
        }
    }
    fn vertex(&self, p: crate::math::Point) -> sdk::XPLMVertex_t {
        sdk::XPLMVertex_t {
            x: (self.left + p.x * self.scale) as f32,
            y: (self.top - p.y * self.scale) as f32,
        }
    }
}
// The immutable optics come from this aircraft's calibration at plug-in start.
// The three cockpit scale drawings are replaced if their emitted tint changes.
#[derive(Default)]
pub struct CockpitMarks {
    tint: Option<u32>,
    drawings: [Option<RetainedDrawing<()>>; 3],
}
impl CockpitMarks {
    fn paint(&mut self, mark: FixedMark, tint: u32, mut paint: impl FnMut()) {
        if self.tint != Some(tint) {
            self.drawings = Default::default();
            self.tint = Some(tint);
        }
        let slot = &mut self.drawings[mark as usize];
        if slot.is_none() {
            // Only copied quad-strip vertices are recorded. No borrowed SDK resources.
            *slot = unsafe { RetainedDrawing::record((), |_| paint()) };
        }
        if let Some(drawing) = slot {
            drawing.draw();
        } else {
            paint();
        }
    }
}

pub fn paint(
    marks: &mut CockpitMarks,
    scene: &Scene,
    optics: Optics,
    camera: View,
    runway_camera: Option<&CameraProjection>,
    window: Option<Bounds>,
    brightness: f32,
) {
    let panel = window.is_none();
    let placement = Placement::new(window, optics);
    let mut strip = Vec::new();
    for (group, segments, conformal) in std::iter::once((0, &scene.conformal, true)).chain(
        scene
            .layers
            .iter()
            .enumerate()
            .map(|(group, segments)| (group, segments, false)),
    ) {
        for pass in usize::from(panel)..2 {
            let tint = color(if pass == 0 {
                [0.025, 0.045, 0.015, brightness * 0.32]
            } else if panel {
                [0.25 * brightness, brightness, 0.40 * brightness, 1.]
            } else {
                [0.74, 0.84, 0.64, brightness]
            });
            let mut draw_segments = |segments: &[Segment]| {
                strip.clear();
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
                        if panel { 4.6 } else { 2.1 }
                    } else {
                        3.2
                    }) {
                        // Convert perimeter order to a strip; duplicate endpoints to
                        // prevent joins between disconnected strokes.
                        let [a, b, c, d] = points.map(|p| placement.vertex(p));
                        if let Some(&last) = strip.last() {
                            strip.extend([last, d]);
                        }
                        // Keep the first triangle counter-clockwise in panel (y-up) space.
                        strip.extend([d, a, c, b]);
                    }
                }
                if !strip.is_empty() {
                    unsafe {
                        sdk::XPLMQuadstrip(tint, strip.as_ptr(), strip.len() as i32);
                    }
                }
            };
            if panel && group == 1 {
                let mut from = 0;
                for (mark, range) in &scene.fixed {
                    draw_segments(&segments[from..range.start]);
                    marks.paint(*mark, tint, || draw_segments(&segments[range.clone()]));
                    from = range.end;
                }
                draw_segments(&segments[from..]);
            } else {
                // Even the body-fixed marks move under a desktop view change.
                draw_segments(segments);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn collimator_rectangle_and_offset_desktop_use_separate_origins() {
        let optics = Optics::parse(include_str!("../hud-optics.txt")).unwrap();
        let panel = Placement::new(None, optics);
        let [tl, br] = [(480., 100.), (1440., 1060.)]
            .map(|(x, y)| panel.vertex(crate::math::Point::new(x, y)));
        assert_eq!([tl.x, tl.y, br.x, br.y], [0., 640., 640., 0.]);
        let overlay = Placement::new(Some(Bounds::new(-1920, 1200, 0, 120)), optics);
        let center = overlay.vertex(crate::math::Point::new(960., 540.));
        assert_eq!([center.x, center.y], [-960., 660.]);
    }
}
