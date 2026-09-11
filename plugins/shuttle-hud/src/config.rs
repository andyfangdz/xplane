use crate::math::{deg, rad, Point, View};
use std::{collections::BTreeMap, fs, path::Path};
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
    pub length: f64,
    pub un: f64,
    pub ue: f64,
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
                let n = (end_lat - lat) * 111120.0;
                let e = (end_lon - lon) * 111120.0 * rad((lat + end_lat) * 0.5).cos();
                let length = n.hypot(e);
                if length < 1000.0 {
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
                    length,
                    un: n / length,
                    ue: e / length,
                    heading: (deg(e.atan2(n)) + 360.0) % 360.0,
                })
            })
            .collect()
    }
    pub fn offsets(&self, lat: f64, lon: f64) -> (f64, f64) {
        let n = (lat - self.lat) * 111120.0;
        let e = (lon - self.lon) * 111120.0 * rad(self.lat).cos();
        (
            n * self.un + e * self.ue - self.displaced,
            e * self.un - n * self.ue,
        )
    }
}
#[cfg(test)]
mod tests {
    use super::*;
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
