const EARTH_RADIUS_M: f64 = 6_371_000.0;

#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct GeoPoint {
    pub lat: f64,
    pub lon: f64,
    pub elevation_m: f64,
}

pub fn project(origin: GeoPoint, point: GeoPoint) -> (f64, f64) {
    let mean_lat = ((origin.lat + point.lat) * 0.5).to_radians();
    let east = (point.lon - origin.lon).to_radians() * mean_lat.cos() * EARTH_RADIUS_M;
    let north = (point.lat - origin.lat).to_radians() * EARTH_RADIUS_M;
    (east, north)
}

pub fn distance(from: GeoPoint, to: GeoPoint) -> f64 {
    let (east, north) = project(from, to);
    east.hypot(north)
}

pub fn offset(origin: GeoPoint, heading_deg: f64, distance_m: f64) -> GeoPoint {
    let heading = heading_deg.to_radians();
    let north = distance_m * heading.cos();
    let east = distance_m * heading.sin();
    let longitude_scale = (EARTH_RADIUS_M * origin.lat.to_radians().cos()).max(1.0);
    GeoPoint {
        lat: origin.lat + (north / EARTH_RADIUS_M).to_degrees(),
        lon: origin.lon + (east / longitude_scale).to_degrees(),
        elevation_m: origin.elevation_m,
    }
}

pub fn bearing(from: GeoPoint, to: GeoPoint) -> f64 {
    let (east, north) = project(from, to);
    east.atan2(north).to_degrees().rem_euclid(360.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offset_round_trip_preserves_distance_and_bearing() {
        let origin = GeoPoint {
            lat: 41.16,
            lon: -73.13,
            elevation_m: 10.0,
        };
        let destination = offset(origin, 58.0, 1_852.0);
        assert!((distance(origin, destination) - 1_852.0).abs() < 0.5);
        assert!((bearing(origin, destination) - 58.0).abs() < 0.05);
    }
}
