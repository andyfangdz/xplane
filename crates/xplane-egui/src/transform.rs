use egui::{epaint::Primitive, Pos2, Rect};
use xplane_plugin::Bounds;

#[derive(Clone, Copy, Debug)]
pub struct Geometry {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}
impl Geometry {
    pub fn width(self) -> i32 {
        self.right - self.left
    }
    pub fn height(self) -> i32 {
        self.top - self.bottom
    }
    pub fn local(self, x: i32, y: i32) -> Pos2 {
        Pos2::new((x - self.left) as f32, (self.top - y) as f32)
    }
}
impl From<Bounds> for Geometry {
    fn from(b: Bounds) -> Self {
        Self {
            left: b.left,
            top: b.top,
            right: b.right,
            bottom: b.bottom,
        }
    }
}
pub struct RenderTransform {
    pub modelview: [f32; 16],
    pub projection: [f32; 16],
    pub viewport: [i32; 4],
    pub window: Geometry,
    pub render_size: [u32; 2],
}
impl RenderTransform {
    pub fn new(
        modelview: [f32; 16],
        projection: [f32; 16],
        viewport: [i32; 4],
        window: Geometry,
    ) -> Option<Self> {
        if viewport[0] < 0
            || viewport[1] < 0
            || viewport[2] <= 0
            || viewport[3] <= 0
            || !modelview
                .iter()
                .chain(projection.iter())
                .all(|v| v.is_finite())
        {
            return None;
        }
        let size = [
            viewport[0].checked_add(viewport[2])? as u32,
            viewport[1].checked_add(viewport[3])? as u32,
        ];
        Some(Self {
            modelview,
            projection,
            viewport,
            window,
            render_size: size,
        })
    }
    pub fn point(&self, p: Pos2) -> Pos2 {
        let model = [
            self.window.left as f32 + p.x,
            self.window.top as f32 - p.y,
            0.0,
            1.0,
        ];
        let eye = multiply_matrix_vector(&self.modelview, model);
        let clip = multiply_matrix_vector(&self.projection, eye);
        if clip[3].abs() < f32::EPSILON {
            return p;
        }
        let x = self.viewport[0] as f32 + (clip[0] / clip[3] + 1.0) * self.viewport[2] as f32 * 0.5;
        let y = self.viewport[1] as f32 + (clip[1] / clip[3] + 1.0) * self.viewport[3] as f32 * 0.5;
        Pos2::new(x, self.render_size[1] as f32 - y)
    }
    pub fn rect(&self, rect: Rect) -> Rect {
        let points = [
            self.point(rect.left_top()),
            self.point(rect.right_top()),
            self.point(rect.right_bottom()),
            self.point(rect.left_bottom()),
        ];
        let mut min = points[0];
        let mut max = min;
        for p in &points[1..] {
            min.x = min.x.min(p.x);
            min.y = min.y.min(p.y);
            max.x = max.x.max(p.x);
            max.y = max.y.max(p.y);
        }
        Rect::from_min_max(min, max)
    }
}
pub fn multiply_matrix_vector(matrix: &[f32; 16], vector: [f32; 4]) -> [f32; 4] {
    std::array::from_fn(|row| {
        matrix[row] * vector[0]
            + matrix[4 + row] * vector[1]
            + matrix[8 + row] * vector[2]
            + matrix[12 + row] * vector[3]
    })
}
pub fn transform_primitives(
    primitives: &mut [egui::ClippedPrimitive],
    transform: &RenderTransform,
) {
    for primitive in primitives {
        primitive.clip_rect = transform.rect(primitive.clip_rect);
        if let Primitive::Mesh(mesh) = &mut primitive.primitive {
            for vertex in &mut mesh.vertices {
                vertex.pos = transform.point(vertex.pos);
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    const IDENTITY: [f32; 16] = [
        1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.,
    ];
    #[test]
    fn column_major_projection_respects_window_and_offset_viewport() {
        let p = [
            0.002, 0., 0., 0., 0., 0.002, 0., 0., 0., 0., 1., 0., -1., -1., 0., 1.,
        ];
        let t = RenderTransform::new(
            IDENTITY,
            p,
            [20, 30, 1000, 1000],
            Geometry {
                left: 100,
                top: 900,
                right: 500,
                bottom: 500,
            },
        )
        .unwrap();
        let actual = t.point(Pos2::new(20., 30.));
        assert!((actual.x - 140.).abs() < 0.001 && (actual.y - 130.).abs() < 0.001);
        assert!(RenderTransform::new(IDENTITY, p, [0, 0, 0, 1000], t.window).is_none());
    }
}
