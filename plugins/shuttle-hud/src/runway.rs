//! Scenery-conformal runway rays. Guidance retains its independent landing datum.
use crate::{
    config::{Optics, Runway, RUNWAY_METERS_PER_DEGREE},
    guidance::LandingPath,
    math::Point,
    scene::Segment,
};
pub use xplane_airports::GeoPoint;
use xplane_airports::LocalProjection;
use xplane_units::{degrees, length::meter, meters};

pub type Vec3 = [f64; 3];
pub type Matrix = [f32; 16];

#[derive(Default)]
pub struct RunwaySurface {
    pub edges: Vec<[GeoPoint; 2]>,
    pub aims: Vec<GeoPoint>,
}

impl RunwaySurface {
    /// Match the configured landing threshold and the opposite physical end.
    /// Sample both edges at <=250 m intervals to follow a sloping runway.
    /// A failed terrain sample suppresses the outline until the next refresh.
    pub fn sample(r: &Runway, mut elevation: impl FnMut(f64, f64) -> Option<f64>) -> Option<Self> {
        let length = r.axis.length().get::<meter>() - r.displaced;
        if !(length > 0.0 && r.width > 0.0) {
            return None;
        }
        let point = |along: f64, cross: f64| {
            let t = (along + r.displaced) / r.axis.length().get::<meter>();
            let lat = r.lat + t * (r.end_lat - r.lat);
            let lon = r.lon + t * (r.end_lon - r.lon);
            let projection = LocalProjection::new(
                GeoPoint {
                    lat,
                    lon,
                    elevation: meters(r.elev),
                },
                degrees(lat),
                meters(RUNWAY_METERS_PER_DEGREE),
            );
            let (east, north) = r.axis.east_north(meters(0.0), meters(cross));
            projection.unproject(east, north)
        };
        let mut sample = |along, cross| {
            let point = point(along, cross);
            elevation(point.lat, point.lon)
                .filter(|h| h.is_finite())
                .map(|elevation_m| GeoPoint {
                    elevation: meters(elevation_m),
                    ..point
                })
        };
        let mut result = Self::default();
        let steps = (length / 250.0).ceil() as usize;
        let mut first = None;
        let mut previous = None;
        for i in 0..=steps {
            let along = length * i as f64 / steps as f64;
            let pair = [
                sample(along, -r.width / 2.0)?,
                sample(along, r.width / 2.0)?,
            ];
            if let Some(prev) = previous {
                let prev: [GeoPoint; 2] = prev;
                result
                    .edges
                    .extend([[prev[0], pair[0]], [prev[1], pair[1]]]);
            } else {
                first = Some(pair);
            }
            previous = Some(pair);
        }
        result.edges.push(first?);
        result.edges.push(previous?);
        // Aim markers retain their existing distances ahead of the threshold,
        // but now share the scenery and eye geometry used by the runway box.
        for along in [LandingPath::INNER_AIM, LandingPath::OUTER_AIM] {
            if let Some(point) = sample(along, 0.0) {
                result.aims.push(point);
            }
        }
        Some(result)
    }

    pub fn rays(
        &self,
        world: &Matrix,
        aircraft: &Matrix,
        mut local: impl FnMut(GeoPoint) -> Vec3,
    ) -> RunwayRays {
        let mut ray = |point| body_ray(world, aircraft, local(point));
        RunwayRays {
            edges: self.edges.iter().map(|p| [ray(p[0]), ray(p[1])]).collect(),
            aims: self.aims.iter().map(|p| ray(*p)).collect(),
        }
    }
}

#[derive(Default)]
pub struct RunwayRays {
    pub edges: Vec<[Vec3; 2]>,
    pub aims: Vec<Vec3>,
}

fn transform(m: &Matrix, p: [f64; 4]) -> [f64; 4] {
    std::array::from_fn(|row| (0..4).map(|col| f64::from(m[col * 4 + row]) * p[col]).sum())
}

/// X-Plane's world matrix includes the actual camera origin. Undo only the
/// aircraft matrix's rotation to express that eye-to-ground ray in body axes.
/// Translation in the aircraft matrix refers to the CG, not the pilot's eye.
pub fn body_ray(world: &Matrix, aircraft: &Matrix, local: Vec3) -> Vec3 {
    let eye = transform(world, [local[0], local[1], local[2], 1.0]);
    std::array::from_fn(|col| {
        (0..3)
            .map(|row| f64::from(aircraft[col * 4 + row]) * eye[row])
            .sum()
    })
}

pub fn valid_rotation(matrix: &Matrix) -> bool {
    matrix.iter().all(|v| v.is_finite())
        && (0..3).all(|col| {
            let length: f64 = (0..3)
                .map(|row| f64::from(matrix[col * 4 + row]).powi(2))
                .sum();
            (length - 1.0).abs() < 0.01
        })
        && [(0, 1), (0, 2), (1, 2)].into_iter().all(|(a, b)| {
            let dot: f64 = (0..3)
                .map(|row| f64::from(matrix[a * 4 + row]) * f64::from(matrix[b * 4 + row]))
                .sum();
            dot.abs() < 0.01
        })
}

pub fn project_body(optics: Optics, ray: Vec3) -> Option<Point> {
    if ray.iter().any(|v| !v.is_finite()) || -ray[2] < 0.01 {
        return None;
    }
    let v = optics.body_view(0.0, 0.0, 0.0);
    Some(Point::new(
        v.center.x + v.fx * ray[0] / -ray[2],
        v.center.y - v.fy * ray[1] / -ray[2],
    ))
}

pub fn project_edge(optics: Optics, mut rays: [Vec3; 2]) -> Option<Segment> {
    if rays.iter().flatten().any(|v| !v.is_finite()) || (rays[0][2] > -0.01 && rays[1][2] > -0.01) {
        return None;
    }
    for k in 0..2 {
        if rays[k][2] > -0.01 {
            let other = rays[1 - k];
            let t = (-0.01 - rays[k][2]) / (other[2] - rays[k][2]);
            rays[k] = std::array::from_fn(|j| rays[k][j] + t * (other[j] - rays[k][j]));
            rays[k][2] = -0.01;
        }
    }
    Some(Segment {
        a: project_body(optics, rays[0])?,
        b: project_body(optics, rays[1])?,
    })
}

pub struct CameraProjection {
    pub aircraft: Matrix,
    pub projection: Matrix,
    pub logical_width: f64,
}
impl CameraProjection {
    /// Reproject a clipped combiner ray with the same matrices as the scenery.
    pub fn point(&self, optics: Optics, point: Point) -> Option<Point> {
        let v = optics.body_view(0.0, 0.0, 0.0);
        let body = [
            (point.x - v.center.x) / v.fx,
            -(point.y - v.center.y) / v.fy,
            -1.0,
            0.0,
        ];
        let camera = transform(&self.aircraft, body);
        let clip = transform(&self.projection, camera);
        if clip.iter().any(|v| !v.is_finite()) || clip[3] <= 0.0 {
            return None;
        }
        Some(Point::new(
            self.logical_width * (0.5 + 0.5 * clip[0] / clip[3])
                - (self.logical_width - 1920.0) / 2.0,
            540.0 * (1.0 - clip[1] / clip[3]),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::rad;

    #[test]
    fn scenery_samples_preserve_width_threshold_end_and_slope() {
        for r in Runway::parse_all(include_str!("../runways.csv")) {
            let surface = RunwaySurface::sample(&r, |lat, lon| Some(700.0 + lat + lon)).unwrap();
            let near = surface.edges[surface.edges.len() - 2];
            let far = surface.edges[surface.edges.len() - 1];
            let center =
                |p: [GeoPoint; 2]| ((p[0].lat + p[1].lat) / 2.0, (p[0].lon + p[1].lon) / 2.0);
            let (lat, lon) = center(far);
            assert!((lat - r.end_lat).abs() < 1e-10 && (lon - r.end_lon).abs() < 1e-10);
            let (lat, lon) = center(near);
            let t = r.displaced / r.axis.length().get::<meter>();
            assert!((lat - (r.lat + t * (r.end_lat - r.lat))).abs() < 1e-10);
            assert!((lon - (r.lon + t * (r.end_lon - r.lon))).abs() < 1e-10);
            for pair in [near, far] {
                let n = (pair[1].lat - pair[0].lat) * 111120.0;
                let e = (pair[1].lon - pair[0].lon) * 111120.0 * rad(center(pair).0).cos();
                assert!((n.hypot(e) - r.width).abs() < 1e-5);
                for p in pair {
                    assert!((p.elevation.get::<meter>() - (700.0 + p.lat + p.lon)).abs() < 1e-10);
                }
            }
        }
        let r = Runway::parse_all(include_str!("../runways.csv")).remove(0);
        assert!(RunwaySurface::sample(&r, |_, _| None).is_none());
    }

    #[test]
    fn runway_ray_projection_matches_independent_world_to_clip_matrix() {
        let optics = Optics::parse(include_str!("../hud-optics.txt")).unwrap();
        // A translated, yawed camera and a separately rotated aircraft.
        let world = [
            0.8, 0.0, 0.6, 0.0, 0.0, 1.0, 0.0, 0.0, -0.6, 0.0, 0.8, 0.0, 20.0, -3.0, 5.0, 1.0,
        ];
        let aircraft = [
            1.0, 0.0, 0.0, 0.0, 0.0, 0.8, 0.6, 0.0, 0.0, -0.6, 0.8, 0.0, 2.0, -5.0, -10.0, 1.0,
        ];
        let projection = [
            1.5, 0.0, 0.0, 0.0, 0.0, 2.4, 0.0, 0.0, 0.12, -0.25, -1.0, -1.0, 0.0, 0.0, -0.2, 0.0,
        ];
        let camera = CameraProjection {
            aircraft,
            projection,
            logical_width: 1920.0,
        };
        for point in [
            [-100.0, 40.0, -1500.0],
            [100.0, -10.0, -500.0],
            [0.0, 0.0, -10000.0],
        ] {
            let ray = body_ray(&world, &aircraft, point);
            let p = camera
                .point(optics, project_body(optics, ray).unwrap())
                .unwrap();
            let eye = transform(&world, [point[0], point[1], point[2], 1.0]);
            let clip = transform(&projection, eye);
            assert!((p.x - 960.0 * (1.0 + clip[0] / clip[3])).abs() < 0.0001);
            assert!((p.y - 540.0 * (1.0 - clip[1] / clip[3])).abs() < 0.0001);
        }
        assert!(project_edge(optics, [[0.0, 0.0, 1.0], [0.0, 0.0, 2.0]]).is_none());
        assert!(project_edge(optics, [[0.0, 0.0, 1.0], [10.0, 0.0, -10.0]]).is_some());
        assert!(!valid_rotation(&[0.0; 16]));
        assert!(valid_rotation(&aircraft));
    }
}
