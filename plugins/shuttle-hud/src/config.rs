use crate::math::{Point, View};
use std::{collections::BTreeMap, fs, path::Path};
use xplane_airports::{GeoPoint, LocalProjection, RunwayAxis};
use xplane_units::{angle::degree, degrees, length::meter, meters};

/// The accepted Shuttle runway table uses 60 nautical miles per degree.
pub const RUNWAY_METERS_PER_DEGREE: f64 = 111_120.0;
#[derive(Clone, Copy, Debug)]
pub struct Optics {
    pub left: f64,
    pub right: f64,
    pub top: f64,
    pub bottom: f64,
    pub panel_left: f64,
    pub panel_bottom: f64,
    pub panel_size: f64,
}
impl Optics {
    pub fn parse(input: &str) -> Result<Self, String> {
        let values: BTreeMap<_, _> = input
            .lines()
            .filter_map(|line| {
                let mut words = line.split_whitespace();
                Some((words.next()?, words.next()?.parse::<f64>().ok()?))
            })
            .collect();
        let get = |key| {
            values
                .get(key)
                .copied()
                .filter(|v| v.is_finite())
                .ok_or_else(|| format!("missing or invalid optics value {key}"))
        };
        let optics = Self {
            left: get("left")?,
            right: get("right")?,
            top: get("top")?,
            bottom: get("bottom")?,
            panel_left: get("panel_left")?,
            panel_bottom: get("panel_bottom")?,
            panel_size: get("panel_size")?,
        };
        if !(optics.left < 0.0
            && optics.right > 0.0
            && optics.bottom < optics.top
            && optics.panel_left >= 2048.0
            && optics.panel_bottom >= 0.0
            && optics.panel_size >= 64.0
            && optics.panel_left + optics.panel_size <= 4096.0
            && optics.panel_bottom + optics.panel_size <= 2048.0)
        {
            return Err("optics exceed the native atlas/combiner bounds".to_owned());
        }
        Ok(optics)
    }
    pub fn load(directory: &Path) -> Result<Self, String> {
        Self::parse(
            &fs::read_to_string(directory.join("hud-optics.txt")).map_err(|e| e.to_string())?,
        )
    }
    pub fn body_view(self, heading: f64, pitch: f64, roll: f64) -> View {
        let fx = 960.0 / (self.right - self.left);
        let fy = 960.0 / (self.top - self.bottom);
        View {
            fx,
            fy,
            center: Point::new(480.0 - self.left * fx, 100.0 + self.top * fy),
            heading,
            pitch,
            roll,
        }
    }
}
#[derive(Clone, Debug)]
pub struct Runway {
    pub name: String,
    pub lat: f64,
    pub lon: f64,
    pub end_lat: f64,
    pub end_lon: f64,
    pub width: f64,
    pub displaced: f64,
    pub elev: f64,
    pub axis: RunwayAxis,
    /// Guidance heading is retained separately from the outline's vector:
    /// frozen display inputs include an independently rounded heading.
    pub heading: f64,
}
impl Runway {
    pub fn parse_all(text: &str) -> Vec<Self> {
        text.lines()
            .filter_map(|line| {
                let words: Vec<_> = line.split(',').collect();
                if words.len() != 8 {
                    return None;
                }
                let numbers: Vec<f64> = words[1..]
                    .iter()
                    .map(|s| s.parse::<f64>())
                    .collect::<Result<_, _>>()
                    .ok()?;
                if numbers.iter().any(|v| !v.is_finite()) {
                    return None;
                }
                let (lat, lon, end_lat, end_lon, width, displaced, elev) = (
                    numbers[0], numbers[1], numbers[2], numbers[3], numbers[4], numbers[5],
                    numbers[6],
                );
                let axis = LocalProjection::new(
                    GeoPoint {
                        lat,
                        lon,
                        elevation: meters(elev),
                    },
                    degrees((lat + end_lat) * 0.5),
                    meters(RUNWAY_METERS_PER_DEGREE),
                )
                .axis_to(GeoPoint {
                    lat: end_lat,
                    lon: end_lon,
                    elevation: meters(elev),
                })?;
                if axis.length() < meters(1000.0) {
                    return None;
                }
                Some(Self {
                    name: words[0].to_owned(),
                    lat,
                    lon,
                    end_lat,
                    end_lon,
                    width,
                    displaced,
                    elev,
                    axis,
                    heading: axis.heading().get::<degree>(),
                })
            })
            .collect()
    }
    pub fn offsets(&self, lat: f64, lon: f64) -> (f64, f64) {
        let (east, north) = self.projection().project(GeoPoint {
            lat,
            lon,
            elevation: meters(0.0),
        });
        let (along, cross) = self.axis.offsets(east, north);
        (along.get::<meter>() - self.displaced, cross.get::<meter>())
    }

    /// Guidance uses the physical end's latitude, independently of the
    /// midpoint latitude used to calculate the runway direction.
    pub fn projection(&self) -> LocalProjection {
        LocalProjection::new(
            GeoPoint {
                lat: self.lat,
                lon: self.lon,
                elevation: meters(self.elev),
            },
            degrees(self.lat),
            meters(RUNWAY_METERS_PER_DEGREE),
        )
    }

    /// Ground location relative to the displaced landing threshold, in metres.
    pub fn point(&self, along: f64, cross: f64) -> GeoPoint {
        let (east, north) = self
            .axis
            .east_north(meters(along + self.displaced), meters(cross));
        self.projection().unproject(east, north)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::{deg, rad};

    #[test]
    fn shared_geometry_preserves_shuttle_scale_and_displaced_datum() {
        for r in Runway::parse_all(include_str!("../runways.csv")) {
            // Frozen pre-refactor formulas: direction uses midpoint latitude,
            // but aircraft offsets and the terrain datum use physical-end latitude.
            let north = (r.end_lat - r.lat) * 111120.0;
            let east = (r.end_lon - r.lon) * 111120.0 * rad((r.lat + r.end_lat) * 0.5).cos();
            let length = north.hypot(east);
            let un = north / length;
            let ue = east / length;
            assert!((r.axis.length().get::<meter>() - length).abs() < 1e-10);
            assert!((r.heading - ((deg(east.atan2(north)) + 360.0) % 360.0)).abs() < 1e-12);
            for (lat, lon) in [
                (r.lat, r.lon),
                (r.end_lat, r.end_lon),
                (r.lat - 0.1, r.lon + 0.1),
                (r.lat + 0.1, r.lon - 0.1),
            ] {
                let n = (lat - r.lat) * 111120.0;
                let e = (lon - r.lon) * 111120.0 * rad(r.lat).cos();
                let expected = (n * un + e * ue - r.displaced, e * un - n * ue);
                let actual = r.offsets(lat, lon);
                assert!((actual.0 - expected.0).abs() < 1e-8);
                assert!((actual.1 - expected.1).abs() < 1e-8);
            }
            let distance = r.displaced + 2500.0 / crate::math::FT;
            let terrain = r.point(2500.0 / crate::math::FT, 0.0);
            assert!((terrain.lat - (r.lat + distance * un / 111120.0)).abs() < 1e-12);
            assert!(
                (terrain.lon - (r.lon + distance * ue / (111120.0 * rad(r.lat).cos()))).abs()
                    < 1e-12
            );
            let threshold = r.point(0.0, 0.0);
            let offsets = r.offsets(threshold.lat, threshold.lon);
            assert!(offsets.0.abs() < 1e-8 && offsets.1.abs() < 1e-8);
        }
    }

    #[test]
    fn reject_invalid_optics_before_registering() {
        let source = include_str!("../hud-optics.txt");
        assert!(Optics::parse(source).is_ok());
        assert!(Optics::parse("").is_err());
        assert!(Optics::parse(&source.replace("panel_size 640", "panel_size 9000")).is_err());
        assert!(Optics::parse(&format!("{source}\nleft NaN")).is_err());
        assert_eq!(Runway::parse_all(include_str!("../runways.csv")).len(), 4);
    }
}
