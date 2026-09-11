use poweroff180::{
    guidance::{deg, rad},
    Config, Controller,
};
use uom::si::{f64::Length, length::meter};
use xplane_airports::GeoPoint;

#[test]
fn shared_geometry_uses_nautical_miles_and_midpoint_latitude() {
    let original = Config::default();
    for config in [
        original,
        Config {
            threshold_lat: original.end_lat,
            threshold_lon: original.end_lon,
            end_lat: original.threshold_lat,
            end_lon: original.threshold_lon,
            ..original
        },
        Config {
            end_lat: original.threshold_lat,
            ..original
        },
        Config {
            end_lon: original.threshold_lon,
            ..original
        },
    ] {
        let projection = config.runway_projection();
        let axis = config.runway_axis().unwrap();
        // Independent 1852 m/NM oracle; project with midpoint latitude.
        let heading_north = (config.end_lat - config.threshold_lat) * 60.0 * 1852.0;
        let heading_east = (config.end_lon - config.threshold_lon)
            * 60.0
            * 1852.0
            * rad((config.threshold_lat + config.end_lat) * 0.5).cos();
        let expected_heading = deg(heading_east.atan2(heading_north)).rem_euclid(360.0);
        assert!((Controller::new(config).heading - expected_heading).abs() < 0.0001);

        let scale = 60.0 * 1852.0;
        let longitude_scale = scale * rad((config.threshold_lat + config.end_lat) * 0.5).cos();
        let north = (config.end_lat - config.threshold_lat) * scale;
        let east = (config.end_lon - config.threshold_lon) * longitude_scale;
        let length = north.hypot(east);
        for delta_lat in [-0.1, -0.02, 0.0, 0.02, 0.1] {
            for delta_lon in [-0.1, -0.02, 0.0, 0.02, 0.1] {
                let point = GeoPoint {
                    lat: config.threshold_lat + delta_lat,
                    lon: config.threshold_lon + delta_lon,
                    elevation: Length::new::<meter>(0.0),
                };
                let n = (point.lat - config.threshold_lat) * scale;
                let e = (point.lon - config.threshold_lon) * longitude_scale;
                let expected = (
                    n * (north / length) + e * (east / length),
                    e * (north / length) - n * (east / length),
                );
                let (e, n) = projection.project(point);
                let actual = axis.offsets(e, n);
                assert!((actual.0.get::<meter>() - expected.0).abs() < 0.001);
                assert!((actual.1.get::<meter>() - expected.1).abs() < 0.001);
            }
        }
    }
}
