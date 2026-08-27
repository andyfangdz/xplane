use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use crate::geo::{bearing, distance, offset, project, GeoPoint};
use crate::model::{Airport, Runway, RunwayEnd, RunwayMatch, RunwaySelection, TouchdownMetrics};

const MAX_APPROACH_HEADING_ERROR_DEG: f64 = 20.0;

#[derive(Default)]
pub struct RunwayDatabase {
    airports: Vec<Airport>,
    airport_lookup: HashMap<String, usize>,
    runways: Vec<Runway>,
    grid: HashMap<(i32, i32), Vec<usize>>,
}

impl RunwayDatabase {
    pub fn load(xplane_root: &Path) -> Result<Self, String> {
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

    pub fn airport_count(&self) -> usize {
        self.airports.len()
    }

    pub fn runway_count(&self) -> usize {
        self.runways.len()
    }

    pub fn airport(&self, id: &str) -> Option<&Airport> {
        let normalized = id.trim().to_ascii_uppercase();
        self.airport_lookup
            .get(&normalized)
            .and_then(|index| self.airports.get(*index))
    }

    pub fn runway_ids(&self, airport_id: &str) -> Vec<String> {
        let Some(airport) = self.airport(airport_id) else {
            return Vec::new();
        };
        let mut ids = airport
            .runway_indices
            .iter()
            .filter_map(|index| self.runways.get(*index))
            .flat_map(|runway| runway.ends.iter().map(|end| end.id.clone()))
            .collect::<Vec<_>>();
        ids.sort_by_key(|id| runway_sort_key(id));
        ids.dedup();
        ids
    }

    pub fn select_runway(&self, airport_id: &str, runway_id: &str) -> Option<RunwaySelection> {
        let airport = self.airport(airport_id)?;
        let wanted = normalize_runway_id(runway_id);
        for &runway_index in &airport.runway_indices {
            let runway = self.runways.get(runway_index)?;
            for end_index in 0..2 {
                if runway.ends[end_index].id == wanted {
                    return Some(RunwaySelection {
                        airport_id: airport.id.clone(),
                        airport_name: airport.name.clone(),
                        airport_elevation_m: airport.elevation_m,
                        width_m: runway.width_m,
                        length_m: distance(runway.ends[0].physical, runway.ends[1].physical),
                        end: runway.ends[end_index].clone(),
                        opposite: runway.ends[1 - end_index].clone(),
                    });
                }
            }
        }
        None
    }

    pub fn nearest_airport(&self, position: GeoPoint) -> Option<&Airport> {
        self.airports
            .iter()
            .filter_map(|airport| {
                let runway = airport
                    .runway_indices
                    .first()
                    .and_then(|index| self.runways.get(*index))?;
                Some((distance(position, runway.ends[0].threshold), airport))
            })
            .min_by(|(left, _), (right, _)| left.total_cmp(right))
            .map(|(_, airport)| airport)
    }

    pub fn find_approach(&self, position: GeoPoint, true_heading_deg: f64) -> Option<RunwayMatch> {
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

    pub fn metrics(
        &self,
        runway_match: RunwayMatch,
        position: GeoPoint,
        true_heading_deg: f64,
    ) -> TouchdownMetrics {
        let runway = &self.runways[runway_match.runway_index];
        let airport = &self.airports[runway.airport_index];
        let end = &runway.ends[runway_match.end_index];
        let (east, north) = project(end.threshold, position);
        let heading_rad = end.heading_deg.to_radians();
        let right_east = heading_rad.cos();
        let right_north = -heading_rad.sin();
        TouchdownMetrics {
            airport: airport.id.clone(),
            runway: end.id.clone(),
            threshold_elevation_m: end.threshold.elevation_m,
            distance_from_threshold_m: east.hypot(north),
            centerline_deviation_m: east * right_east + north * right_north,
            centerline_angle_deg: angular_delta(end.heading_deg, true_heading_deg),
        }
    }

    fn parse_file(
        &mut self,
        path: &Path,
        seen_airports: &mut HashSet<String>,
    ) -> Result<(), String> {
        let file = File::open(path).map_err(|error| format!("{}: {error}", path.display()))?;
        let mut current_airport = None;
        for line in BufReader::new(file).lines() {
            let line = line.map_err(|error| format!("{}: {error}", path.display()))?;
            let fields = line.split_whitespace().collect::<Vec<_>>();
            let Some(code) = fields.first().copied() else {
                continue;
            };
            if matches!(code, "1" | "16" | "17") {
                current_airport = if fields.len() >= 5 {
                    let id = fields[4].to_ascii_uppercase();
                    let accepted = code == "1" && seen_airports.insert(id.clone());
                    if code != "1" {
                        seen_airports.insert(id.clone());
                    }
                    accepted.then(|| {
                        let airport_index = self.airports.len();
                        let airport = Airport {
                            id: id.clone(),
                            name: fields.get(5..).unwrap_or_default().join(" "),
                            elevation_m: fields[1].parse::<f64>().unwrap_or(0.0) * 0.3048,
                            runway_indices: Vec::new(),
                        };
                        self.airport_lookup.insert(id, airport_index);
                        self.airports.push(airport);
                        airport_index
                    })
                } else {
                    None
                };
                continue;
            }
            if code != "100" || fields.len() < 26 {
                continue;
            }
            let Some(airport_index) = current_airport else {
                continue;
            };
            let elevation_m = self.airports[airport_index].elevation_m;
            if let Some(runway) = parse_runway(&fields, airport_index, elevation_m) {
                let runway_index = self.runways.len();
                self.runways.push(runway);
                self.airports[airport_index]
                    .runway_indices
                    .push(runway_index);
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
            let apt = xplane_root
                .join(normalized)
                .join("Earth nav data")
                .join("apt.dat");
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

fn parse_runway(fields: &[&str], airport_index: usize, elevation_m: f64) -> Option<Runway> {
    let width_m = fields[1].parse::<f64>().ok()?;
    let end_a_physical = point(fields[9], fields[10], elevation_m)?;
    let end_b_physical = point(fields[18], fields[19], elevation_m)?;
    let displacement_a = fields[11].parse::<f64>().ok()?;
    let displacement_b = fields[20].parse::<f64>().ok()?;
    let heading_a = bearing(end_a_physical, end_b_physical);
    let heading_b = bearing(end_b_physical, end_a_physical);
    Some(Runway {
        airport_index,
        width_m,
        ends: [
            RunwayEnd {
                id: normalize_runway_id(fields[8]),
                physical: end_a_physical,
                threshold: offset(end_a_physical, heading_a, displacement_a),
                heading_deg: heading_a,
                displaced_threshold_m: displacement_a,
            },
            RunwayEnd {
                id: normalize_runway_id(fields[17]),
                physical: end_b_physical,
                threshold: offset(end_b_physical, heading_b, displacement_b),
                heading_deg: heading_b,
                displaced_threshold_m: displacement_b,
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

fn normalize_runway_id(raw: &str) -> String {
    let value = raw
        .trim()
        .to_ascii_uppercase()
        .strip_prefix("RW")
        .unwrap_or(raw.trim())
        .to_ascii_uppercase();
    let digit_count = value.chars().take_while(char::is_ascii_digit).count();
    let (digits, suffix) = value.split_at(digit_count);
    digits
        .parse::<u8>()
        .map(|number| format!("{number:02}{suffix}"))
        .unwrap_or(value)
}

fn runway_sort_key(id: &str) -> (u8, String) {
    let digits = id.chars().take_while(char::is_ascii_digit).count();
    (
        id[..digits].parse().unwrap_or(u8::MAX),
        id[digits..].to_owned(),
    )
}

fn angular_delta(reference: f64, value: f64) -> f64 {
    (value - reference + 180.0).rem_euclid(360.0) - 180.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_database() -> RunwayDatabase {
        let fields = "100 45.00 1 0 0.25 1 1 0 9 40.000000 -75.010000 100.00 0.00 2 0 0 0 27 40.000000 -74.990000 0.00 0.00 2 0 0 0"
            .split_whitespace()
            .collect::<Vec<_>>();
        let mut database = RunwayDatabase::default();
        database.airports.push(Airport {
            id: "TEST".to_owned(),
            name: "Test Municipal".to_owned(),
            elevation_m: 100.0,
            runway_indices: vec![0],
        });
        database.airport_lookup.insert("TEST".to_owned(), 0);
        database
            .runways
            .push(parse_runway(&fields, 0, 100.0).unwrap());
        database.rebuild_grid();
        database
    }

    #[test]
    fn parses_and_selects_displaced_threshold() {
        let database = sample_database();
        let runway = database.select_runway("test", "RW09").unwrap();
        assert_eq!(runway.end.id, "09");
        assert!((distance(runway.end.physical, runway.end.threshold) - 100.0).abs() < 0.1);
        assert!((runway.end.heading_deg - 90.0).abs() < 0.1);
        assert_eq!(database.runway_ids("TEST"), ["09", "27"]);
    }

    #[test]
    fn matches_aircraft_inside_runway_heading() {
        let database = sample_database();
        let position = GeoPoint {
            lat: 40.0,
            lon: -75.0,
            elevation_m: 110.0,
        };
        let matched = database.find_approach(position, 90.0).unwrap();
        assert_eq!(database.metrics(matched, position, 90.0).runway, "09");
    }

    #[test]
    #[ignore = "scans the local X-Plane scenery installation"]
    fn loads_installed_scenery_database() {
        let root = std::env::var_os("XPLANE_PATH")
            .map(PathBuf::from)
            .or_else(|| {
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .ancestors()
                    .find(|path| path.join("X-Plane.exe").is_file())
                    .map(PathBuf::from)
            })
            .expect("X-Plane installation was not found");
        let database = RunwayDatabase::load(&root).unwrap();
        assert!(database.airport_count() > 1_000);
        let runway = database.select_runway("KBDR", "06").unwrap();
        assert_eq!(runway.end.id, "06");
        assert_eq!(runway.opposite.id, "24");
        assert!(runway.length_m > 1_000.0);
    }
}
