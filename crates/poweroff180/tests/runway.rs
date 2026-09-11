use poweroff180::{
    guidance::{deg, rad},
    Config, Controller,
};
use xplane_airports::GeoPoint;
use xplane_units::{length::foot, meters};

#[test]
fn shared_geometry_preserves_v7_feet_and_midpoint_latitude() {
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
        // Preserve the two original operation orders: the pure v7 controller
        // computed heading, while the SDK adapter separately projected feet.
        let heading_north = (config.end_lat - config.threshold_lat) * 60.0 * 6076.12;
        let heading_east = (config.end_lon - config.threshold_lon)
            * 60.0
            * 6076.12
            * rad((config.threshold_lat + config.end_lat) * 0.5).cos();
        let expected_heading = deg(heading_east.atan2(heading_north)).rem_euclid(360.0);
        assert!((Controller::new(config).heading - expected_heading).abs() < 1e-12);

        let scale = 60.0 * 6076.12;
        let longitude_scale = scale * rad((config.threshold_lat + config.end_lat) * 0.5).cos();
        let north = (config.end_lat - config.threshold_lat) * scale;
        let east = (config.end_lon - config.threshold_lon) * longitude_scale;
        let length = north.hypot(east);
        for delta_lat in [-0.1, -0.02, 0.0, 0.02, 0.1] {
            for delta_lon in [-0.1, -0.02, 0.0, 0.02, 0.1] {
                let point = GeoPoint {
                    lat: config.threshold_lat + delta_lat,
                    lon: config.threshold_lon + delta_lon,
                    elevation: meters(0.0),
                };
                let n = (point.lat - config.threshold_lat) * scale;
                let e = (point.lon - config.threshold_lon) * longitude_scale;
                let expected = (
                    n * (north / length) + e * (east / length),
                    e * (north / length) - n * (east / length),
                );
                let (e, n) = projection.project(point);
                let actual = axis.offsets(e, n);
                assert!((actual.0.get::<foot>() - expected.0).abs() < 1e-8);
                assert!((actual.1.get::<foot>() - expected.1).abs() < 1e-8);
            }
        }
    }
}
