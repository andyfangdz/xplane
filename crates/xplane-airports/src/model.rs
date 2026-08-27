use crate::GeoPoint;

#[derive(Clone, Debug)]
pub struct Airport {
    pub id: String,
    pub name: String,
    pub elevation_m: f64,
    pub(crate) runway_indices: Vec<usize>,
}

#[derive(Clone, Debug)]
pub struct RunwayEnd {
    pub id: String,
    pub physical: GeoPoint,
    pub threshold: GeoPoint,
    pub heading_deg: f64,
    pub displaced_threshold_m: f64,
}

#[derive(Clone, Debug)]
pub(crate) struct Runway {
    pub airport_index: usize,
    pub width_m: f64,
    pub ends: [RunwayEnd; 2],
}

#[derive(Clone, Debug)]
pub struct RunwaySelection {
    pub airport_id: String,
    pub airport_name: String,
    pub airport_elevation_m: f64,
    pub width_m: f64,
    pub length_m: f64,
    pub end: RunwayEnd,
    pub opposite: RunwayEnd,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct RunwayMatch {
    pub(crate) runway_index: usize,
    pub(crate) end_index: usize,
}

#[derive(Clone, Debug)]
pub struct TouchdownMetrics {
    pub airport: String,
    pub runway: String,
    pub threshold_elevation_m: f64,
    pub distance_from_threshold_m: f64,
    pub centerline_deviation_m: f64,
    pub centerline_angle_deg: f64,
}
