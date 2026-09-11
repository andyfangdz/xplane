use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use uom::si::{
    angle::degree, f64::Angle, f64::Length, length::foot, length::meter, length::nautical_mile,
};

use xplane_airports::{distance, offset, GeoPoint, RunwaySelection};
use xplane_plugin::magnetic_variation;

use crate::pad::{parse_pad, safe_pad_filename, write_pad, PadData};

use super::state::PluginState;

const SETTINGS_FILE: &str = "position-aircraft-rs.prf";

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub(in crate::runtime) enum PanelTab {
    #[default]
    Pad,
    Pattern,
}

impl PanelTab {
    fn key(self) -> &'static str {
        match self {
            Self::Pad => "pad",
            Self::Pattern => "pattern",
        }
    }

    fn from_key(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "pattern" => Self::Pattern,
            _ => Self::Pad,
        }
    }
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub(in crate::runtime) enum PatternDirection {
    #[default]
    Left,
    Right,
}

impl PatternDirection {
    pub(in crate::runtime) fn label(self) -> &'static str {
        match self {
            Self::Left => "Left traffic",
            Self::Right => "Right traffic",
        }
    }

    fn key(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Right => "right",
        }
    }

    fn from_key(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "right" => Self::Right,
            _ => Self::Left,
        }
    }
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub(in crate::runtime) enum PatternLocation {
    #[default]
    OnFinal,
    InterceptFinal,
    Base,
    Downwind,
    Entry,
}

impl PatternLocation {
    pub(in crate::runtime) const ALL: [Self; 5] = [
        Self::OnFinal,
        Self::InterceptFinal,
        Self::Base,
        Self::Downwind,
        Self::Entry,
    ];

    pub(in crate::runtime) fn label(self) -> &'static str {
        match self {
            Self::OnFinal => "On final",
            Self::InterceptFinal => "Intercept final",
            Self::Base => "Base",
            Self::Downwind => "Downwind",
            Self::Entry => "45° entry",
        }
    }

    pub(in crate::runtime) fn detail(self) -> &'static str {
        match self {
            Self::OnFinal => "Established on runway centerline",
            Self::InterceptFinal => "Forty-five degree intercept to final",
            Self::Base => "Halfway along the base leg",
            Self::Downwind => "Abeam the runway on the downwind leg",
            Self::Entry => "Forty-five degree entry to downwind",
        }
    }

    fn key(self) -> &'static str {
        match self {
            Self::OnFinal => "final",
            Self::InterceptFinal => "intercept",
            Self::Base => "base",
            Self::Downwind => "downwind",
            Self::Entry => "entry",
        }
    }

    fn from_key(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "intercept" => Self::InterceptFinal,
            "base" => Self::Base,
            "downwind" => Self::Downwind,
            "entry" => Self::Entry,
            _ => Self::OnFinal,
        }
    }

    pub(in crate::runtime) fn relative(self, delta: isize) -> Self {
        let index = Self::ALL
            .iter()
            .position(|candidate| *candidate == self)
            .unwrap_or(0);
        Self::ALL[(index as isize + delta).rem_euclid(Self::ALL.len() as isize) as usize]
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(in crate::runtime) struct PatternSettings {
    pub(in crate::runtime) active_tab: PanelTab,
    pub(in crate::runtime) airport_id: String,
    pub(in crate::runtime) runway_id: String,
    pub(in crate::runtime) configuration_pad: String,
    pub(in crate::runtime) location: PatternLocation,
    pub(in crate::runtime) direction: PatternDirection,
    pub(in crate::runtime) approach_angle_deg: f64,
    pub(in crate::runtime) pattern_altitude_agl_ft: f64,
    pub(in crate::runtime) downwind_offset_nm: f64,
    pub(in crate::runtime) base_intercept_nm: f64,
    pub(in crate::runtime) final_distance_nm: f64,
}

impl Default for PatternSettings {
    fn default() -> Self {
        Self {
            active_tab: PanelTab::Pad,
            airport_id: String::new(),
            runway_id: String::new(),
            configuration_pad: "QuickFile.pad".to_owned(),
            location: PatternLocation::OnFinal,
            direction: PatternDirection::Left,
            approach_angle_deg: 3.0,
            pattern_altitude_agl_ft: 1_000.0,
            downwind_offset_nm: 1.0,
            base_intercept_nm: 2.0,
            final_distance_nm: 3.0,
        }
    }
}

impl PatternSettings {
    fn load(path: &Path) -> Self {
        let Ok(text) = fs::read_to_string(path) else {
            return Self::default();
        };
        Self::from_text(&text)
    }

    fn from_text(text: &str) -> Self {
        let values = text
            .lines()
            .filter_map(|line| {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    return None;
                }
                let (key, value) = line.split_once('=')?;
                Some((key.trim().to_ascii_lowercase(), value.trim().to_owned()))
            })
            .collect::<HashMap<_, _>>();
        let mut settings = Self::default();
        if let Some(value) = values.get("active_tab") {
            settings.active_tab = PanelTab::from_key(value);
        }
        if let Some(value) = values.get("airport") {
            settings.airport_id = value.trim().to_ascii_uppercase();
        }
        if let Some(value) = values.get("runway") {
            settings.runway_id = value.trim().to_ascii_uppercase();
        }
        if let Some(value) = values.get("configuration_pad") {
            settings.configuration_pad = value.to_owned();
        }
        if let Some(value) = values.get("location") {
            settings.location = PatternLocation::from_key(value);
        }
        if let Some(value) = values.get("direction") {
            settings.direction = PatternDirection::from_key(value);
        }
        parse_setting(
            &values,
            "approach_angle_deg",
            &mut settings.approach_angle_deg,
        );
        parse_setting(
            &values,
            "pattern_altitude_agl_ft",
            &mut settings.pattern_altitude_agl_ft,
        );
        parse_setting(
            &values,
            "downwind_offset_nm",
            &mut settings.downwind_offset_nm,
        );
        parse_setting(
            &values,
            "base_intercept_nm",
            &mut settings.base_intercept_nm,
        );
        parse_setting(
            &values,
            "final_distance_nm",
            &mut settings.final_distance_nm,
        );
        settings.sanitize();
        settings
    }

    fn sanitize(&mut self) {
        self.approach_angle_deg = self.approach_angle_deg.clamp(1.0, 10.0);
        self.pattern_altitude_agl_ft = self.pattern_altitude_agl_ft.clamp(500.0, 5_000.0);
        self.downwind_offset_nm = self.downwind_offset_nm.clamp(0.2, 10.0);
        self.base_intercept_nm = self.base_intercept_nm.clamp(0.2, 20.0);
        self.final_distance_nm = self.final_distance_nm.clamp(0.2, 30.0);
    }

    fn text(&self) -> String {
        format!(
            "# Position Aircraft pattern tab\n\
active_tab={}\n\
airport={}\n\
runway={}\n\
configuration_pad={}\n\
location={}\n\
direction={}\n\
approach_angle_deg={:.2}\n\
pattern_altitude_agl_ft={:.0}\n\
downwind_offset_nm={:.2}\n\
base_intercept_nm={:.2}\n\
final_distance_nm={:.2}\n",
            self.active_tab.key(),
            self.airport_id,
            self.runway_id,
            self.configuration_pad,
            self.location.key(),
            self.direction.key(),
            self.approach_angle_deg,
            self.pattern_altitude_agl_ft,
            self.downwind_offset_nm,
            self.base_intercept_nm,
            self.final_distance_nm,
        )
    }
}

fn parse_setting(values: &HashMap<String, String>, key: &str, destination: &mut f64) {
    if let Some(value) = values.get(key).and_then(|value| value.parse().ok()) {
        *destination = value;
    }
}

#[derive(Clone, Debug)]
pub(in crate::runtime) struct PatternPlacement {
    pub(in crate::runtime) data: PadData,
    pub(in crate::runtime) runway: RunwaySelection,
    pub(in crate::runtime) true_heading_deg: f64,
    pub(in crate::runtime) altitude_agl_ft: f64,
    pub(in crate::runtime) remaining_path_nm: f64,
}

pub(in crate::runtime) struct PatternState {
    pub(in crate::runtime) settings: PatternSettings,
    pub(in crate::runtime) airport_input: String,
    pub(in crate::runtime) preference_path: PathBuf,
    pub(in crate::runtime) preview: Option<PatternPlacement>,
    pub(in crate::runtime) preview_error: Option<String>,
    pub(in crate::runtime) save_name: String,
    auto_save_name: String,
}

impl PatternState {
    pub(in crate::runtime) fn load(preference_directory: &Path) -> Self {
        let preference_path = preference_directory.join(SETTINGS_FILE);
        let settings = PatternSettings::load(&preference_path);
        Self {
            airport_input: settings.airport_id.clone(),
            settings,
            preference_path,
            preview: None,
            preview_error: None,
            save_name: String::new(),
            auto_save_name: String::new(),
        }
    }

    pub(in crate::runtime) fn save(&self) -> Result<(), String> {
        fs::write(&self.preference_path, self.settings.text()).map_err(|error| {
            format!(
                "Unable to save pattern settings {}: {error}",
                self.preference_path.display()
            )
        })
    }
}

impl PluginState {
    pub(in crate::runtime) fn initialize_pattern(&mut self, current_position: GeoPoint) {
        if self.airports.is_none() {
            self.pattern.preview_error = Some("Airport database is unavailable".to_owned());
            return;
        }
        self.ensure_pattern_configuration();
        let reference_position = self.pattern_reference_position(current_position);
        let saved_airport_exists = self.airports.as_ref().is_some_and(|database| {
            database
                .airport(&self.pattern.settings.airport_id)
                .is_some()
        });
        if !saved_airport_exists {
            let nearest_id = self
                .airports
                .as_ref()
                .and_then(|database| database.nearest_airport(reference_position))
                .map(|airport| airport.id.clone());
            if let Some(id) = nearest_id {
                self.pattern.settings.airport_id = id.clone();
                self.pattern.airport_input = id;
            }
        }
        self.ensure_pattern_runway();
        self.refresh_pattern_preview();
        self.save_pattern_settings();
    }

    pub(in crate::runtime) fn resolve_pattern_airport(&mut self) {
        let id = self.pattern.airport_input.trim().to_ascii_uppercase();
        if id.is_empty() {
            self.status = "Enter an airport identifier".to_owned();
            return;
        }
        let Some(airport) = self
            .airports
            .as_ref()
            .and_then(|database| database.airport(&id))
        else {
            self.status = format!("Airport {id} was not found in active scenery");
            return;
        };
        let airport_name = airport.name.clone();
        self.pattern.settings.airport_id = id.clone();
        self.pattern.airport_input = id.clone();
        self.ensure_pattern_runway();
        self.refresh_pattern_preview();
        self.save_pattern_settings();
        self.status = format!("Selected {id} · {airport_name}");
    }

    pub(in crate::runtime) fn select_nearest_pattern_airport(&mut self) {
        let current_position = GeoPoint {
            lat: self.datarefs.latitude.get_f64(),
            lon: self.datarefs.longitude.get_f64(),
            elevation: Length::new::<meter>(self.datarefs.elevation.get_f64()),
        };
        let position = self.pattern_reference_position(current_position);
        let Some(id) = self
            .airports
            .as_ref()
            .and_then(|database| database.nearest_airport(position))
            .map(|airport| airport.id.clone())
        else {
            self.status = "No nearby airport was found".to_owned();
            return;
        };
        self.pattern.airport_input = id;
        self.resolve_pattern_airport();
    }

    pub(in crate::runtime) fn select_pattern_pad(&mut self, filename: String) {
        self.pattern.settings.configuration_pad = filename.clone();
        self.refresh_pattern_preview();
        self.save_pattern_settings();
        if self.pattern.preview.is_some() {
            self.status = format!("Using aircraft configuration from {filename}");
        }
    }

    pub(in crate::runtime) fn pattern_settings_changed(&mut self) {
        self.pattern.settings.sanitize();
        self.ensure_pattern_runway();
        self.refresh_pattern_preview();
        self.save_pattern_settings();
    }

    pub(in crate::runtime) fn position_pattern(&mut self) {
        self.refresh_pattern_preview();
        let Some(preview) = self.pattern.preview.as_ref() else {
            self.status = self
                .pattern
                .preview_error
                .clone()
                .unwrap_or_else(|| "Pattern placement is unavailable".to_owned());
            return;
        };
        let data = preview.data.clone();
        let airport = preview.runway.airport_id.clone();
        let runway = preview.runway.end.id.clone();
        let location = self.pattern.settings.location.label();
        self.position_data(data);
        self.status = format!("Positioned {location} · {airport} RWY {runway}");
    }

    pub(in crate::runtime) fn save_pattern_pad(&mut self) {
        self.refresh_pattern_preview();
        let Some(preview) = self.pattern.preview.as_ref() else {
            self.status = self
                .pattern
                .preview_error
                .clone()
                .unwrap_or_else(|| "Pattern placement is unavailable".to_owned());
            return;
        };
        let Some(filename) = safe_pad_filename(&self.pattern.save_name) else {
            self.status = "Enter a PAD filename".to_owned();
            return;
        };
        match write_pad(&self.pad_directory.join(&filename), &preview.data) {
            Ok(()) => {
                self.refresh_pads();
                if let Some(index) = self.pads.iter().position(|name| name == &filename) {
                    self.selected_index = index;
                }
                self.status = format!("Saved pattern placement as {filename}");
            }
            Err(error) => self.status = format!("Unable to write {filename}: {error}"),
        }
    }

    pub(in crate::runtime) fn cycle_pattern_location(&mut self, delta: isize) {
        self.pattern.settings.location = self.pattern.settings.location.relative(delta);
        self.pattern_settings_changed();
        self.status = format!(
            "Pattern location: {}",
            self.pattern.settings.location.label()
        );
    }

    pub(in crate::runtime) fn save_pattern_settings(&mut self) {
        if let Err(error) = self.pattern.save() {
            self.status = error;
        }
    }

    fn ensure_pattern_configuration(&mut self) {
        if self
            .pads
            .iter()
            .any(|name| name == &self.pattern.settings.configuration_pad)
        {
            return;
        }
        self.pattern.settings.configuration_pad = self
            .pads
            .iter()
            .find(|name| name.eq_ignore_ascii_case("QuickFile.pad"))
            .or_else(|| self.pads.first())
            .cloned()
            .unwrap_or_default();
    }

    fn pattern_reference_position(&self, current_position: GeoPoint) -> GeoPoint {
        if usable_position(current_position) {
            return current_position;
        }
        parse_pad(
            &self
                .pad_directory
                .join(&self.pattern.settings.configuration_pad),
        )
        .ok()
        .map(|data| GeoPoint {
            lat: data.latitude,
            lon: data.longitude,
            elevation: Length::new::<foot>(data.altitude),
        })
        .filter(|position| usable_position(*position))
        .unwrap_or(current_position)
    }

    fn ensure_pattern_runway(&mut self) {
        let Some(database) = self.airports.as_ref() else {
            return;
        };
        let runways = database.runway_ids(&self.pattern.settings.airport_id);
        if !runways
            .iter()
            .any(|id| id == &self.pattern.settings.runway_id)
        {
            self.pattern.settings.runway_id = runways.first().cloned().unwrap_or_default();
        }
    }

    fn refresh_pattern_preview(&mut self) {
        let result = (|| {
            let database = self
                .airports
                .as_ref()
                .ok_or_else(|| "Airport database is unavailable".to_owned())?;
            let runway = database
                .select_runway(
                    &self.pattern.settings.airport_id,
                    &self.pattern.settings.runway_id,
                )
                .ok_or_else(|| "Choose a valid airport and runway".to_owned())?;
            if self.pattern.settings.configuration_pad.is_empty() {
                return Err("Choose an aircraft configuration PAD".to_owned());
            }
            let source = parse_pad(
                &self
                    .pad_directory
                    .join(&self.pattern.settings.configuration_pad),
            )?;
            let geometry = placement_geometry(&runway, &self.pattern.settings);
            let variation = magnetic_variation(geometry.point.lat, geometry.point.lon);
            Ok(build_placement(
                runway,
                &self.pattern.settings,
                source,
                geometry,
                variation,
            ))
        })();
        match result {
            Ok(preview) => {
                self.update_auto_save_name(&preview);
                self.pattern.preview = Some(preview);
                self.pattern.preview_error = None;
            }
            Err(error) => {
                self.pattern.preview = None;
                self.pattern.preview_error = Some(error);
            }
        }
    }

    fn update_auto_save_name(&mut self, preview: &PatternPlacement) {
        let location = match self.pattern.settings.location {
            PatternLocation::OnFinal => format!(
                "{}nm-final",
                compact_number(self.pattern.settings.final_distance_nm)
            ),
            PatternLocation::InterceptFinal => "intercept-final".to_owned(),
            PatternLocation::Base => format!("{}-base", self.pattern.settings.direction.key()),
            PatternLocation::Downwind => {
                format!("{}-downwind", self.pattern.settings.direction.key())
            }
            PatternLocation::Entry => format!("{}-entry", self.pattern.settings.direction.key()),
        };
        let next = format!(
            "{}-RWY{}-{location}",
            preview.runway.airport_id, preview.runway.end.id
        );
        if self.pattern.save_name.trim().is_empty()
            || self.pattern.save_name == self.pattern.auto_save_name
        {
            self.pattern.save_name = next.clone();
        }
        self.pattern.auto_save_name = next;
    }
}

fn usable_position(position: GeoPoint) -> bool {
    position.lat.is_finite()
        && position.lon.is_finite()
        && (-90.0..=90.0).contains(&position.lat)
        && (-180.0..=180.0).contains(&position.lon)
        && (position.lat.abs() > 0.000_1 || position.lon.abs() > 0.000_1)
}

#[derive(Copy, Clone)]
struct PlacementGeometry {
    point: GeoPoint,
    true_heading: Angle,
    remaining_path: Length,
    altitude_profile: AltitudeProfile,
}

#[derive(Copy, Clone)]
enum AltitudeProfile {
    Approach,
    Pattern,
}

fn normalize_angle(angle: Angle) -> Angle {
    Angle::new::<degree>(angle.get::<degree>().rem_euclid(360.0))
}

fn placement_geometry(runway: &RunwaySelection, settings: &PatternSettings) -> PlacementGeometry {
    let heading = runway.end.heading;
    let reciprocal = normalize_angle(heading + Angle::new::<degree>(180.0));
    let side_heading = normalize_angle(
        heading
            + Angle::new::<degree>(match settings.direction {
                PatternDirection::Left => -90.0,
                PatternDirection::Right => 90.0,
            }),
    );
    let toward_centerline = normalize_angle(
        heading
            + Angle::new::<degree>(match settings.direction {
                PatternDirection::Left => 90.0,
                PatternDirection::Right => -90.0,
            }),
    );
    let downwind_offset = Length::new::<nautical_mile>(settings.downwind_offset_nm);
    let base_intercept = Length::new::<nautical_mile>(settings.base_intercept_nm);
    let final_distance = Length::new::<nautical_mile>(settings.final_distance_nm);
    let threshold = runway.end.threshold;
    let base_intersection = offset(threshold, reciprocal, base_intercept);
    let usable_runway_length = distance(threshold, runway.opposite.physical);
    let runway_midpoint = offset(threshold, heading, usable_runway_length * 0.5);
    let downwind_midpoint = offset(runway_midpoint, side_heading, downwind_offset);
    let downwind_path = usable_runway_length * 0.5 + base_intercept * 2.0 + downwind_offset;

    let (point, true_heading, remaining_path) = match settings.location {
        PatternLocation::OnFinal => (
            offset(threshold, reciprocal, final_distance),
            heading,
            final_distance,
        ),
        PatternLocation::InterceptFinal => {
            let behind = offset(base_intersection, reciprocal, downwind_offset);
            let point = offset(behind, side_heading, downwind_offset);
            let intercept_heading = normalize_angle(
                heading
                    + Angle::new::<degree>(match settings.direction {
                        PatternDirection::Left => 45.0,
                        PatternDirection::Right => -45.0,
                    }),
            );
            (
                point,
                intercept_heading,
                base_intercept + downwind_offset * 2.0_f64.sqrt(),
            )
        }
        PatternLocation::Base => (
            offset(base_intersection, side_heading, downwind_offset * 0.5),
            toward_centerline,
            base_intercept + downwind_offset * 0.5,
        ),
        PatternLocation::Downwind => (downwind_midpoint, reciprocal, downwind_path),
        PatternLocation::Entry => {
            let entry_heading = normalize_angle(
                reciprocal
                    + Angle::new::<degree>(match settings.direction {
                        PatternDirection::Left => -45.0,
                        PatternDirection::Right => 45.0,
                    }),
            );
            let entry_leg = downwind_offset * 0.75;
            (
                offset(
                    downwind_midpoint,
                    normalize_angle(entry_heading + Angle::new::<degree>(180.0)),
                    entry_leg,
                ),
                entry_heading,
                entry_leg + downwind_path,
            )
        }
    };
    let altitude_profile = match settings.location {
        PatternLocation::Downwind | PatternLocation::Entry => AltitudeProfile::Pattern,
        PatternLocation::OnFinal | PatternLocation::InterceptFinal | PatternLocation::Base => {
            AltitudeProfile::Approach
        }
    };
    PlacementGeometry {
        point,
        true_heading,
        remaining_path,
        altitude_profile,
    }
}

fn build_placement(
    runway: RunwaySelection,
    settings: &PatternSettings,
    mut data: PadData,
    mut geometry: PlacementGeometry,
    variation_deg: f64,
) -> PatternPlacement {
    let altitude_agl = match geometry.altitude_profile {
        AltitudeProfile::Approach => {
            geometry.remaining_path * settings.approach_angle_deg.to_radians().tan()
        }
        AltitudeProfile::Pattern => Length::new::<foot>(settings.pattern_altitude_agl_ft),
    };
    geometry.point.elevation = runway.airport_elevation + altitude_agl;
    data.latitude = geometry.point.lat;
    data.longitude = geometry.point.lon;
    data.altitude = geometry.point.elevation.get::<foot>();
    data.heading = normalize_angle(geometry.true_heading + Angle::new::<degree>(variation_deg))
        .get::<degree>();
    PatternPlacement {
        data,
        runway,
        true_heading_deg: geometry.true_heading.get::<degree>(),
        altitude_agl_ft: altitude_agl.get::<foot>(),
        remaining_path_nm: geometry.remaining_path.get::<nautical_mile>(),
    }
}

fn compact_number(value: f64) -> String {
    if value.fract().abs() < 0.01 {
        format!("{value:.0}")
    } else {
        format!("{value:.1}").replace('.', "_")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use xplane_airports::{distance, project, RunwayEnd};

    fn runway(displacement_m: f64) -> RunwaySelection {
        let physical = GeoPoint {
            lat: 40.0,
            lon: -75.0,
            elevation: Length::new::<meter>(30.0),
        };
        let threshold = offset(
            physical,
            Angle::new::<degree>(90.0),
            Length::new::<meter>(displacement_m),
        );
        RunwaySelection {
            airport_id: "TEST".to_owned(),
            airport_name: "Test Municipal".to_owned(),
            airport_elevation: Length::new::<meter>(30.0),
            width: Length::new::<meter>(45.0),
            length: Length::new::<meter>(2_000.0),
            end: RunwayEnd {
                id: "09".to_owned(),
                physical,
                threshold,
                heading: Angle::new::<degree>(90.0),
                displaced_threshold: Length::new::<meter>(displacement_m),
            },
            opposite: RunwayEnd {
                id: "27".to_owned(),
                physical: offset(
                    physical,
                    Angle::new::<degree>(90.0),
                    Length::new::<meter>(2_000.0),
                ),
                threshold: offset(
                    physical,
                    Angle::new::<degree>(90.0),
                    Length::new::<meter>(2_000.0),
                ),
                heading: Angle::new::<degree>(270.0),
                displaced_threshold: Length::new::<meter>(0.0),
            },
        }
    }

    #[test]
    fn final_is_measured_from_displaced_threshold() {
        let runway = runway(300.0);
        let settings = PatternSettings::default();
        let geometry = placement_geometry(&runway, &settings);
        assert!(
            (distance(runway.end.threshold, geometry.point) - Length::new::<nautical_mile>(3.0))
                .abs()
                < Length::new::<meter>(0.5)
        );
        assert!(
            (distance(runway.end.physical, geometry.point)
                - (Length::new::<nautical_mile>(3.0) - Length::new::<meter>(300.0)))
            .abs()
                < Length::new::<meter>(0.5)
        );
    }

    #[test]
    fn left_and_right_base_positions_mirror_centerline() {
        let runway = runway(0.0);
        let mut settings = PatternSettings {
            location: PatternLocation::Base,
            ..PatternSettings::default()
        };
        let left = placement_geometry(&runway, &settings);
        settings.direction = PatternDirection::Right;
        let right = placement_geometry(&runway, &settings);
        let (left_east, left_north) = project(runway.end.threshold, left.point);
        let (right_east, right_north) = project(runway.end.threshold, right.point);
        // The equirectangular projection uses each point's mean latitude, so
        // mirrored geodesic offsets can differ by a fraction of a metre.
        assert!((left_east - right_east).abs() < Length::new::<meter>(2.0));
        assert!((left_north + right_north).abs() < Length::new::<meter>(2.0));
        assert_eq!(left.true_heading.get::<degree>(), 180.0);
        assert_eq!(right.true_heading.get::<degree>(), 0.0);
    }

    #[test]
    fn preferences_round_trip() {
        let expected = PatternSettings {
            active_tab: PanelTab::Pattern,
            airport_id: "KBDR".to_owned(),
            runway_id: "06".to_owned(),
            configuration_pad: "Approach.pad".to_owned(),
            location: PatternLocation::Downwind,
            direction: PatternDirection::Right,
            approach_angle_deg: 3.2,
            pattern_altitude_agl_ft: 1_200.0,
            downwind_offset_nm: 1.1,
            base_intercept_nm: 2.4,
            final_distance_nm: 4.0,
        };
        assert_eq!(PatternSettings::from_text(&expected.text()), expected);
    }

    #[test]
    fn downwind_is_abeam_usable_runway_midpoint() {
        let runway = runway(300.0);
        let mut settings = PatternSettings {
            location: PatternLocation::Downwind,
            direction: PatternDirection::Right,
            downwind_offset_nm: 1.0,
            base_intercept_nm: 1.0,
            final_distance_nm: 1.0,
            ..PatternSettings::default()
        };
        let first = placement_geometry(&runway, &settings);
        let usable_length_m = distance(runway.end.threshold, runway.opposite.physical);
        let midpoint = offset(
            runway.end.threshold,
            runway.end.heading,
            usable_length_m * 0.5,
        );
        let (east_m, north_m) = project(midpoint, first.point);

        assert!(east_m.abs() < Length::new::<meter>(2.0));
        assert!((north_m + Length::new::<nautical_mile>(1.0)).abs() < Length::new::<meter>(2.0));
        assert_eq!(first.true_heading.get::<degree>(), 270.0);

        settings.base_intercept_nm = 8.0;
        settings.final_distance_nm = 12.0;
        let changed_lengths = placement_geometry(&runway, &settings);
        assert!(distance(first.point, changed_lengths.point) < Length::new::<meter>(0.1));
    }

    #[test]
    fn downwind_and_entry_use_pattern_altitude() {
        let runway = runway(300.0);
        let mut settings = PatternSettings {
            location: PatternLocation::Downwind,
            approach_angle_deg: 9.0,
            pattern_altitude_agl_ft: 1_200.0,
            ..PatternSettings::default()
        };

        for location in [PatternLocation::Downwind, PatternLocation::Entry] {
            settings.location = location;
            let geometry = placement_geometry(&runway, &settings);
            let placement =
                build_placement(runway.clone(), &settings, PadData::default(), geometry, 0.0);
            assert!((placement.altitude_agl_ft - 1_200.0).abs() < 0.01);
            assert!(
                (placement.data.altitude - (Length::new::<meter>(30.0).get::<foot>() + 1_200.0))
                    .abs()
                    < 0.01
            );
        }
    }

    #[test]
    fn older_preferences_default_pattern_altitude() {
        let settings = PatternSettings::from_text(
            "active_tab=pattern\nlocation=downwind\ndownwind_offset_nm=1.0\n",
        );
        assert_eq!(settings.pattern_altitude_agl_ft, 1_000.0);
    }
}
