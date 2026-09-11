use crate::GeoPoint;
use uom::si::{angle::degree, angle::radian, f64::Angle, f64::Length, length::meter, ratio::ratio};

/// A fixed-latitude, linear projection for runway and short-leg calculations.
///
/// `distance_per_degree` sets the calibrated geographic scale as a length. The longitude reference latitude is explicit:
/// some calibrated consumers use the origin, others the segment midpoint.
/// Elevation does not participate in the horizontal projection.
/// Unlike `project`, this does not change its scale with each sampled point.
#[derive(Clone, Copy, Debug)]
pub struct LocalProjection {
    origin: GeoPoint,
    distance_per_degree: Length,
    longitude_cos: f64,
}

impl LocalProjection {
    pub fn new(origin: GeoPoint, reference_latitude: Angle, distance_per_degree: Length) -> Self {
        Self {
            origin,
            distance_per_degree,
            longitude_cos: reference_latitude.get::<radian>().cos(),
        }
    }

    /// East and north offsets as physical lengths.
    pub fn project(self, point: GeoPoint) -> (Length, Length) {
        (
            (point.lon - self.origin.lon) * self.distance_per_degree * self.longitude_cos,
            (point.lat - self.origin.lat) * self.distance_per_degree,
        )
    }

    /// Inverse horizontal projection, retaining the origin's elevation.
    pub fn unproject(self, east: Length, north: Length) -> GeoPoint {
        GeoPoint {
            lat: self.origin.lat + (north / self.distance_per_degree).get::<ratio>(),
            lon: self.origin.lon
                + (east / (self.distance_per_degree * self.longitude_cos)).get::<ratio>(),
            elevation: self.origin.elevation,
        }
    }

    pub fn axis_to(self, end: GeoPoint) -> Option<RunwayAxis> {
        let (east, north) = self.project(end);
        RunwayAxis::new(east, north)
    }
}

/// A runway or navigation-leg axis with dimensionally checked distances.
///
/// Positive along-track follows the axis toward its far end; positive cross-track
/// is to the right when looking in that direction.
///
/// ```compile_fail
/// use xplane_airports::RunwayAxis;
/// use uom::si::{f64::Length, f64::Time, length::meter, time::second};
/// RunwayAxis::new(Length::new::<meter>(100.0), Time::new::<second>(20.0));
/// ```
#[derive(Clone, Copy, Debug)]
pub struct RunwayAxis {
    east: f64,
    north: f64,
    length: Length,
    heading: Angle,
}

impl RunwayAxis {
    pub fn new(east: Length, north: Length) -> Option<Self> {
        let length = north.hypot(east);
        if !length.is_finite() || length <= Length::new::<meter>(f64::EPSILON) {
            return None;
        }
        Some(Self {
            east: (east / length).get::<ratio>(),
            north: (north / length).get::<ratio>(),
            length,
            heading: Angle::new::<degree>(east.atan2(north).get::<degree>().rem_euclid(360.0)),
        })
    }

    pub fn length(self) -> Length {
        self.length
    }

    pub fn heading(self) -> Angle {
        self.heading
    }

    pub fn offsets(self, east: Length, north: Length) -> (Length, Length) {
        (
            north * self.north + east * self.east,
            east * self.north - north * self.east,
        )
    }

    pub fn east_north(self, along: Length, cross: Length) -> (Length, Length) {
        (
            along * self.east + cross * self.north,
            along * self.north - cross * self.east,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::{f64::Length, length::foot, length::nautical_mile};

    #[test]
    fn projection_roundtrips_offsets_in_each_consumers_units() {
        let origin = GeoPoint {
            lat: 40.8,
            lon: -74.2,
            elevation: Length::new::<meter>(50.0),
        };
        for scale in [
            Length::new::<nautical_mile>(60.0),
            Length::new::<meter>(111_120.0),
            Length::new::<foot>(100_000.0),
        ] {
            let projection = LocalProjection::new(origin, Angle::new::<degree>(40.9), scale);
            let point = projection.unproject(0.02 * scale, -0.01 * scale);
            let (east, north) = projection.project(point);
            assert!((east - 0.02 * scale).abs() < Length::new::<meter>(1e-8));
            assert!((north + 0.01 * scale).abs() < Length::new::<meter>(1e-8));
            assert_eq!(point.elevation, Length::new::<meter>(50.0));
        }
    }

    #[test]
    fn axis_offsets_are_signed_and_reversible_in_every_quadrant() {
        for (east, north) in [
            (0.0, 100.0),
            (100.0, 0.0),
            (3.0, -4.0),
            (-3.0, -4.0),
            (-3.0, 4.0),
        ] {
            let axis =
                RunwayAxis::new(Length::new::<meter>(east), Length::new::<meter>(north)).unwrap();
            assert!((0.0..360.0).contains(&axis.heading().get::<degree>()));
            let (along, cross) =
                axis.offsets(Length::new::<meter>(east), Length::new::<meter>(north));
            assert!((along - axis.length()).abs() < Length::new::<meter>(1e-12));
            assert!(cross.abs() < Length::new::<meter>(1e-12));
            for offsets in [(-200.0, -20.0), (0.0, 0.0), (200.0, 20.0)] {
                let (e, n) = axis.east_north(
                    Length::new::<meter>(offsets.0),
                    Length::new::<meter>(offsets.1),
                );
                let actual = axis.offsets(e, n);
                assert!(
                    (actual.0 - Length::new::<meter>(offsets.0)).abs()
                        < Length::new::<meter>(1e-12)
                );
                assert!(
                    (actual.1 - Length::new::<meter>(offsets.1)).abs()
                        < Length::new::<meter>(1e-12)
                );
            }
        }
        let northbound =
            RunwayAxis::new(Length::new::<meter>(0.0), Length::new::<meter>(100.0)).unwrap();
        assert_eq!(
            northbound.offsets(Length::new::<meter>(20.0), Length::new::<meter>(-200.0)),
            (Length::new::<meter>(-200.0), Length::new::<meter>(20.0))
        );
        let southbound =
            RunwayAxis::new(Length::new::<meter>(0.0), Length::new::<meter>(-100.0)).unwrap();
        assert_eq!(
            southbound.offsets(Length::new::<meter>(20.0), Length::new::<meter>(-200.0)),
            (Length::new::<meter>(200.0), Length::new::<meter>(-20.0))
        );
    }

    #[test]
    fn axis_rejects_degenerate_or_nonfinite_segments() {
        for (east, north) in [(0.0, 0.0), (f64::NAN, 1.0), (1.0, f64::INFINITY)] {
            assert!(
                RunwayAxis::new(Length::new::<meter>(east), Length::new::<meter>(north)).is_none()
            );
        }
    }
}
