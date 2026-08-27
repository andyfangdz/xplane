use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use super::support::angular_delta;

const EARTH_RADIUS_M: f64 = 6_371_000.0;
const MAX_APPROACH_HEADING_ERROR_DEG: f64 = 20.0;

#[derive(Copy, Clone, Debug, Default)]
pub(super) struct GeoPoint {
    pub(super) lat: f64,
    pub(super) lon: f64,
    pub(super) elevation_m: f64,
}

#[derive(Clone, Debug)]
struct RunwayEnd {
    id: String,
    physical: GeoPoint,
    threshold: GeoPoint,
    heading_deg: f64,
}

#[derive(Clone, Debug)]
struct Runway {
    airport: String,
    width_m: f64,
    ends: [RunwayEnd; 2],
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(super) struct RunwayMatch {
    runway_index: usize,
    end_index: usize,
}

#[derive(Clone, Debug)]
pub(super) struct TouchdownMetrics {
    pub(super) airport: String,
    pub(super) runway: String,
    pub(super) threshold_elevation_m: f64,
    pub(super) distance_from_threshold_m: f64,
    pub(super) centerline_deviation_m: f64,
    pub(super) centerline_angle_deg: f64,
}

#[derive(Default)]
pub(super) struct RunwayDatabase {
    runways: Vec<Runway>,
    grid: HashMap<(i32, i32), Vec<usize>>,
}

impl RunwayDatabase {
    pub(super) fn load(xplane_root: &Path) -> Result<Self, String> {
        let mut database = Self::default();
        let mut seen_airports = HashSet::new();
        let paths = apt_paths(xplane_root);
        if paths.is_empty() {
            return Err("no apt.dat files were found".to_owned());
        }
        for path in paths {
            database.parse_file(&path, &mut seen_airports)?;
        }
        database.rebuild_grid();
        Ok(database)
    }

    pub(super) fn runway_count(&self) -> usize {
        self.runways.len()
    }

    fn parse_file(
        &mut self,
        path: &Path,
        seen_airports: &mut HashSet<String>,
    ) -> Result<(), String> {
        let file = File::open(path).map_err(|error| format!("{}: {error}", path.display()))?;
        let mut current_airport: Option<(String, f64, bool)> = None;
        for line in BufReader::new(file).lines() {
            let line = line.map_err(|error| format!("{}: {error}", path.display()))?;
            let fields: Vec<_> = line.split_whitespace().collect();
            let Some(code) = fields.first().copied() else {
                continue;
            };
            if matches!(code, "1" | "16" | "17") {
                if fields.len() >= 5 {
                    let id = fields[4].to_owned();
                    let elevation_m = fields[1].parse::<f64>().unwrap_or(0.0) * 0.3048;
                    let accepted = code == "1" && seen_airports.insert(id.clone());
                    if code != "1" {
                        seen_airports.insert(id.clone());
                    }
                    current_airport = Some((id, elevation_m, accepted));
                }
                continue;
            }
            if code != "100" || fields.len() < 26 {
                continue;
            }
            let Some((airport, elevation_m, true)) = current_airport.as_ref() else {
                continue;
            };
            if let Some(runway) = parse_runway(fields.as_slice(), airport, *elevation_m) {
                self.runways.push(runway);
            }
        }
        Ok(())
    }

    fn rebuild_grid(&mut self) {
        self.grid.clear();
        for (index, runway) in self.runways.iter().enumerate() {
            for end in &runway.ends {
                self.grid
                    .entry(grid_key(end.physical))
                    .or_default()
                    .push(index);
            }
        }
        for indices in self.grid.values_mut() {
            indices.sort_unstable();
            indices.dedup();
        }
    }

    pub(super) fn find_approach(
        &self,
        position: GeoPoint,
        true_heading_deg: f64,
    ) -> Option<RunwayMatch> {
        let (lat_cell, lon_cell) = grid_key(position);
        let mut visited = HashSet::new();
        let mut best: Option<(f64, RunwayMatch)> = None;
        for lat in (lat_cell - 1)..=(lat_cell + 1) {
            for lon in (lon_cell - 1)..=(lon_cell + 1) {
                let Some(indices) = self.grid.get(&(lat, lon)) else {
                    continue;
                };
                for &runway_index in indices {
                    if !visited.insert(runway_index) {
                        continue;
                    }
                    let runway = &self.runways[runway_index];
                    if !inside_runway(runway, position) {
                        continue;
                    }
                    for end_index in 0..2 {
                        let end = &runway.ends[end_index];
                        let heading_error = angular_delta(true_heading_deg, end.heading_deg).abs();
                        if heading_error <= MAX_APPROACH_HEADING_ERROR_DEG
                            && best.is_none_or(|(error, _)| heading_error < error)
                        {
                            best = Some((
                                heading_error,
                                RunwayMatch {
                                    runway_index,
                                    end_index,
                                },
                            ));
                        }
                    }
                }
            }
        }
        best.map(|(_, runway_match)| runway_match)
    }

    pub(super) fn metrics(
        &self,
        runway_match: RunwayMatch,
        position: GeoPoint,
        true_heading_deg: f64,
    ) -> TouchdownMetrics {
        let runway = &self.runways[runway_match.runway_index];
        let end = &runway.ends[runway_match.end_index];
        let (east, north) = project(end.threshold, position);
        let heading_rad = end.heading_deg.to_radians();
        let right_east = heading_rad.cos();
        let right_north = -heading_rad.sin();
        let distance = east.hypot(north);
        let deviation = east * right_east + north * right_north;
        TouchdownMetrics {
            airport: runway.airport.clone(),
            runway: end.id.clone(),
            threshold_elevation_m: end.threshold.elevation_m,
            distance_from_threshold_m: distance,
            centerline_deviation_m: deviation,
            centerline_angle_deg: angular_delta(end.heading_deg, true_heading_deg),
        }
    }
}

fn apt_paths(xplane_root: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let custom = xplane_root.join("Custom Scenery");
    let ini = custom.join("scenery_packs.ini");
    if let Ok(text) = std::fs::read_to_string(ini) {
        for line in text.lines() {
            let Some(relative) = line.trim().strip_prefix("SCENERY_PACK ") else {
                continue;
            };
            let normalized = relative.trim().replace('/', std::path::MAIN_SEPARATOR_STR);
            let base = xplane_root.join(normalized);
            let apt = base.join("Earth nav data").join("apt.dat");
            if apt.is_file() {
                paths.push(apt);
            }
        }
    }
    let global = xplane_root
        .join("Global Scenery")
        .join("Global Airports")
        .join("Earth nav data")
        .join("apt.dat");
    if global.is_file() {
        paths.push(global);
    }
    paths
}

fn parse_runway(fields: &[&str], airport: &str, elevation_m: f64) -> Option<Runway> {
    let width_m = fields[1].parse::<f64>().ok()?;
    let end_a_physical = point(fields[9], fields[10], elevation_m)?;
    let end_b_physical = point(fields[18], fields[19], elevation_m)?;
    let displacement_a = fields[11].parse::<f64>().ok()?;
    let displacement_b = fields[20].parse::<f64>().ok()?;
    let heading_a = bearing(end_a_physical, end_b_physical);
    let heading_b = bearing(end_b_physical, end_a_physical);
    let threshold_a = offset(end_a_physical, heading_a, displacement_a);
    let threshold_b = offset(end_b_physical, heading_b, displacement_b);
    Some(Runway {
        airport: airport.to_owned(),
        width_m,
        ends: [
            RunwayEnd {
                id: fields[8].to_owned(),
                physical: end_a_physical,
                threshold: threshold_a,
                heading_deg: heading_a,
            },
            RunwayEnd {
                id: fields[17].to_owned(),
                physical: end_b_physical,
                threshold: threshold_b,
                heading_deg: heading_b,
            },
        ],
    })
}

fn point(lat: &str, lon: &str, elevation_m: f64) -> Option<GeoPoint> {
    Some(GeoPoint {
        lat: lat.parse().ok()?,
        lon: lon.parse().ok()?,
        elevation_m,
    })
}

fn grid_key(point: GeoPoint) -> (i32, i32) {
    (point.lat.floor() as i32, point.lon.floor() as i32)
}

fn project(origin: GeoPoint, point: GeoPoint) -> (f64, f64) {
    let mean_lat = ((origin.lat + point.lat) * 0.5).to_radians();
    let east = (point.lon - origin.lon).to_radians() * mean_lat.cos() * EARTH_RADIUS_M;
    let north = (point.lat - origin.lat).to_radians() * EARTH_RADIUS_M;
    (east, north)
}

fn offset(origin: GeoPoint, heading_deg: f64, distance_m: f64) -> GeoPoint {
    let heading = heading_deg.to_radians();
    let north = distance_m * heading.cos();
    let east = distance_m * heading.sin();
    GeoPoint {
        lat: origin.lat + (north / EARTH_RADIUS_M).to_degrees(),
        lon: origin.lon + (east / (EARTH_RADIUS_M * origin.lat.to_radians().cos())).to_degrees(),
        elevation_m: origin.elevation_m,
    }
}

fn bearing(from: GeoPoint, to: GeoPoint) -> f64 {
    let (east, north) = project(from, to);
    east.atan2(north).to_degrees().rem_euclid(360.0)
}

fn inside_runway(runway: &Runway, position: GeoPoint) -> bool {
    let (end_east, end_north) = project(runway.ends[0].physical, runway.ends[1].physical);
    let length = end_east.hypot(end_north);
    if length <= f64::EPSILON {
        return false;
    }
    let (east, north) = project(runway.ends[0].physical, position);
    let along = (east * end_east + north * end_north) / length;
    let cross = (east * end_north - north * end_east).abs() / length;
    along >= -10.0 && along <= length + 10.0 && cross <= runway.width_m * 0.5 + 10.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_displaced_threshold() {
        let fields: Vec<_> = "100 45.00 1 0 0.25 1 1 0 09 40.000000 -75.010000 100.00 0.00 2 0 0 0 27 40.000000 -74.990000 0.00 0.00 2 0 0 0"
            .split_whitespace()
            .collect();
        let runway = parse_runway(&fields, "TEST", 100.0).unwrap();
        let displaced = project(runway.ends[0].physical, runway.ends[0].threshold);
        assert!((displaced.0.hypot(displaced.1) - 100.0).abs() < 0.1);
        assert!((runway.ends[0].heading_deg - 90.0).abs() < 0.1);
    }

    #[test]
    fn matches_aircraft_inside_runway_heading() {
        let fields: Vec<_> = "100 45.00 1 0 0.25 1 1 0 09 40.000000 -75.010000 0.00 0.00 2 0 0 0 27 40.000000 -74.990000 0.00 0.00 2 0 0 0"
            .split_whitespace()
            .collect();
        let mut database = RunwayDatabase::default();
        database
            .runways
            .push(parse_runway(&fields, "TEST", 100.0).unwrap());
        database.rebuild_grid();
        let position = GeoPoint {
            lat: 40.0,
            lon: -75.0,
            elevation_m: 110.0,
        };
        let matched = database.find_approach(position, 90.0).unwrap();
        assert_eq!(database.metrics(matched, position, 90.0).runway, "09");
    }
}
