use xplane_units::{
    angle::{degree, radian},
    degrees, meters,
    ratio::ratio,
    Angle, Length,
};

#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct GeoPoint {
    /// Geodetic degrees, retained in their source representation so subtracting
    /// nearby coordinates does not introduce a radians conversion round trip.
    pub lat: f64,
    pub lon: f64,
    pub elevation: Length,
}

pub fn project(origin: GeoPoint, point: GeoPoint) -> (Length, Length) {
    let radius = meters(6_371_000.0);
    let mean_lat = ((origin.lat + point.lat) * 0.5).to_radians();
    let east = radius * ((point.lon - origin.lon).to_radians() * mean_lat.cos());
    let north = radius * (point.lat - origin.lat).to_radians();
    (east, north)
}

pub fn distance(from: GeoPoint, to: GeoPoint) -> Length {
    let (east, north) = project(from, to);
    east.hypot(north)
}

pub fn offset(origin: GeoPoint, heading: Angle, distance: Length) -> GeoPoint {
    let radius = meters(6_371_000.0);
    let heading = heading.get::<radian>();
    let north = distance * heading.cos();
    let east = distance * heading.sin();
    let longitude_scale = (radius * origin.lat.to_radians().cos()).max(meters(1.0));
    GeoPoint {
        lat: origin.lat + (north / radius).get::<ratio>().to_degrees(),
        lon: origin.lon + (east / longitude_scale).get::<ratio>().to_degrees(),
        elevation: origin.elevation,
    }
}

pub fn bearing(from: GeoPoint, to: GeoPoint) -> Angle {
    let (east, north) = project(from, to);
    degrees(east.atan2(north).get::<degree>().rem_euclid(360.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn offset_round_trip_preserves_distance_and_bearing() {
        let origin = GeoPoint {
            lat: 41.16,
            lon: -73.13,
            elevation: meters(10.0),
        };
        let destination = offset(origin, degrees(58.0), meters(1_852.0));
        assert!((distance(origin, destination) - meters(1_852.0)).abs() < meters(0.5));
        assert!((bearing(origin, destination) - degrees(58.0)).abs() < degrees(0.05));
    }
}
