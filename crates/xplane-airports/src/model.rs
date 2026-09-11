use crate::GeoPoint;
use xplane_units::{Angle, Length};

#[derive(Clone, Debug)]
pub struct Airport {
    pub id: String,
    pub name: String,
    pub elevation: Length,
    pub(crate) runway_indices: Vec<usize>,
}

#[derive(Clone, Debug)]
pub struct RunwayEnd {
    pub id: String,
    pub physical: GeoPoint,
    pub threshold: GeoPoint,
    pub heading: Angle,
    pub displaced_threshold: Length,
}

#[derive(Clone, Debug)]
pub(crate) struct Runway {
    pub airport_index: usize,
    pub width: Length,
    pub ends: [RunwayEnd; 2],
}

#[derive(Clone, Debug)]
pub struct RunwaySelection {
    pub airport_id: String,
    pub airport_name: String,
    pub airport_elevation: Length,
    pub width: Length,
    pub length: Length,
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
    pub threshold_elevation: Length,
    pub distance_from_threshold: Length,
    pub centerline_deviation: Length,
    pub centerline_angle: Angle,
}
