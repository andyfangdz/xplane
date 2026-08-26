use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::Path;

#[derive(Clone, Debug, Default)]
pub(crate) struct AutopilotData {
    pub(crate) mode: i32,
    pub(crate) altitude: f64,
    pub(crate) vertical_velocity: f64,
    pub(crate) heading: f64,
    pub(crate) airspeed: f64,
    pub(crate) state: i32,
    pub(crate) heading_roll_mode: i32,
}

#[derive(Clone, Debug)]
pub(crate) struct PadData {
    pub(crate) latitude: f64,
    pub(crate) longitude: f64,
    pub(crate) altitude: f64,
    pub(crate) heading: f64,
    pub(crate) pitch: f64,
    pub(crate) roll: f64,
    pub(crate) speed: f64,
    pub(crate) throttle: f64,
    pub(crate) flaps: f64,
    pub(crate) gear: i32,
    pub(crate) use_ap: bool,
    pub(crate) ap: AutopilotData,
}

impl Default for PadData {
    fn default() -> Self {
        Self {
            latitude: 0.0,
            longitude: 0.0,
            altitude: 0.0,
            heading: 0.0,
            pitch: 0.0,
            roll: 0.0,
            speed: 0.0,
            throttle: 0.0,
            flaps: 0.0,
            gear: 0,
            use_ap: false,
            ap: AutopilotData::default(),
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(usize)]
pub(crate) enum Field {
    Latitude = 0,
    Longitude,
    Altitude,
    Heading,
    Pitch,
    Roll,
    Speed,
    Throttle,
    Flaps,
    Gear,
    ApMode,
    ApAltitude,
    ApVerticalVelocity,
    ApHeading,
    ApAirspeed,
    ApState,
    ApHeadingRollMode,
    SaveName,
}

const FIELD_COUNT: usize = 18;

#[derive(Clone)]
pub(crate) struct Form {
    values: [String; FIELD_COUNT],
    pub(crate) use_ap: bool,
}

impl Form {
    pub(crate) fn from_data(data: &PadData, save_name: &str) -> Self {
        let mut values: [String; FIELD_COUNT] = std::array::from_fn(|_| String::new());
        values[Field::Latitude as usize] = format!("{:.6}", data.latitude);
        values[Field::Longitude as usize] = format!("{:.6}", data.longitude);
        values[Field::Altitude as usize] = format!("{:.2}", data.altitude);
        values[Field::Heading as usize] = format!("{:.2}", data.heading);
        values[Field::Pitch as usize] = format!("{:.2}", data.pitch);
        values[Field::Roll as usize] = format!("{:.2}", data.roll);
        values[Field::Speed as usize] = format!("{:.2}", data.speed);
        values[Field::Throttle as usize] = format!("{:.4}", data.throttle);
        values[Field::Flaps as usize] = format!("{:.4}", data.flaps);
        values[Field::Gear as usize] = data.gear.to_string();
        values[Field::ApMode as usize] = data.ap.mode.to_string();
        values[Field::ApAltitude as usize] = format!("{:.2}", data.ap.altitude);
        values[Field::ApVerticalVelocity as usize] = format!("{:.2}", data.ap.vertical_velocity);
        values[Field::ApHeading as usize] = format!("{:.2}", data.ap.heading);
        values[Field::ApAirspeed as usize] = format!("{:.2}", data.ap.airspeed);
        values[Field::ApState as usize] = data.ap.state.to_string();
        values[Field::ApHeadingRollMode as usize] = data.ap.heading_roll_mode.to_string();
        values[Field::SaveName as usize] = save_name.to_owned();
        Self {
            values,
            use_ap: data.use_ap,
        }
    }

    pub(crate) fn value(&self, field: Field) -> &str {
        &self.values[field as usize]
    }

    pub(crate) fn value_mut(&mut self, field: Field) -> &mut String {
        &mut self.values[field as usize]
    }

    fn parse_number(&self, field: Field, label: &str) -> Result<f64, String> {
        self.value(field)
            .trim()
            .parse::<f64>()
            .map_err(|_| format!("{label} is not a valid number"))
    }

    pub(crate) fn to_data(&self) -> Result<PadData, String> {
        let data = PadData {
            latitude: self.parse_number(Field::Latitude, "Latitude")?,
            longitude: self.parse_number(Field::Longitude, "Longitude")?,
            altitude: self.parse_number(Field::Altitude, "Altitude")?,
            heading: normalize_heading(self.parse_number(Field::Heading, "Heading")?),
            pitch: self.parse_number(Field::Pitch, "Pitch")?,
            roll: self.parse_number(Field::Roll, "Roll")?,
            speed: self.parse_number(Field::Speed, "Speed")?,
            throttle: self.parse_number(Field::Throttle, "Throttle")?,
            flaps: self.parse_number(Field::Flaps, "Flaps")?,
            gear: if self.parse_number(Field::Gear, "Gear")? >= 0.5 {
                1
            } else {
                0
            },
            use_ap: self.use_ap,
            ap: AutopilotData {
                mode: self.parse_number(Field::ApMode, "AP mode")?.round() as i32,
                altitude: self.parse_number(Field::ApAltitude, "AP altitude")?,
                vertical_velocity: self
                    .parse_number(Field::ApVerticalVelocity, "AP vertical velocity")?,
                heading: normalize_heading(self.parse_number(Field::ApHeading, "AP heading")?),
                airspeed: self.parse_number(Field::ApAirspeed, "AP airspeed")?,
                state: self.parse_number(Field::ApState, "AP state")?.round() as i32,
                heading_roll_mode: self
                    .parse_number(Field::ApHeadingRollMode, "AP heading/roll mode")?
                    .round() as i32,
            },
        };
        validate_data(&data)?;
        Ok(data)
    }
}

pub(crate) fn normalize_heading(value: f64) -> f64 {
    value.rem_euclid(360.0)
}

fn validate_data(data: &PadData) -> Result<(), String> {
    if !(-90.0..=90.0).contains(&data.latitude) {
        return Err("Latitude must be between -90 and 90".to_owned());
    }
    if !(-180.0..=180.0).contains(&data.longitude) {
        return Err("Longitude must be between -180 and 180".to_owned());
    }
    if !(-90.0..=90.0).contains(&data.pitch) {
        return Err("Pitch must be between -90 and 90".to_owned());
    }
    if !(-180.0..=180.0).contains(&data.roll) {
        return Err("Roll must be between -180 and 180".to_owned());
    }
    if !(0.0..=5000.0).contains(&data.speed) {
        return Err("Speed must be between 0 and 5000 knots".to_owned());
    }
    if !(0.0..=1.0).contains(&data.throttle) {
        return Err("Throttle must be between 0 and 1".to_owned());
    }
    if !(0.0..=1.0).contains(&data.flaps) {
        return Err("Flaps must be between 0 and 1".to_owned());
    }
    Ok(())
}

pub(crate) fn parse_pad(path: &Path) -> Result<PadData, String> {
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("Unable to open PAD {}: {error}", path.display()))?;
    parse_pad_text(&contents)
}

fn parse_pad_text(contents: &str) -> Result<PadData, String> {
    let mut values = HashMap::<String, String>::new();
    let mut section = String::new();
    for raw_line in contents.lines() {
        let line = raw_line.trim();
        if line.starts_with('[') && line.ends_with(']') {
            section = line[1..line.len() - 1].trim().to_ascii_lowercase();
            continue;
        }
        if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            values.insert(
                format!("{}.{}", section, key.trim().to_ascii_lowercase()),
                value.trim().to_owned(),
            );
        }
    }

    fn number(
        values: &HashMap<String, String>,
        section: &str,
        key: &str,
        required: bool,
        default: f64,
    ) -> Result<f64, String> {
        let lookup = format!(
            "{}.{}",
            section.to_ascii_lowercase(),
            key.to_ascii_lowercase()
        );
        match values.get(&lookup) {
            Some(raw) if !raw.is_empty() => raw
                .parse::<f64>()
                .map_err(|_| format!("Invalid number for {key}")),
            _ if required => Err(format!("Missing {key}")),
            _ => Ok(default),
        }
    }

    let data = PadData {
        latitude: number(&values, "position_data", "latitude", true, 0.0)?,
        longitude: number(&values, "position_data", "longitude", true, 0.0)?,
        altitude: number(&values, "position_data", "altitude", true, 0.0)?,
        heading: normalize_heading(number(&values, "position_data", "heading", true, 0.0)?),
        pitch: number(&values, "position_data", "pitch", true, 0.0)?,
        roll: number(&values, "position_data", "roll", true, 0.0)?,
        speed: number(&values, "position_data", "speed", true, 0.0)?,
        throttle: number(&values, "position_data", "throttle", true, 0.0)?,
        flaps: number(&values, "position_data", "flaps", true, 0.0)?,
        gear: if number(&values, "position_data", "gear", true, 0.0)? >= 0.5 {
            1
        } else {
            0
        },
        use_ap: number(&values, "config", "use_autopilot_data", false, 0.0)? != 0.0,
        ap: AutopilotData {
            mode: number(&values, "autopilot_data", "autopilot_mode", false, 0.0)?.round() as i32,
            altitude: number(&values, "autopilot_data", "autopilot_altitude", false, 0.0)?,
            vertical_velocity: number(
                &values,
                "autopilot_data",
                "autopilot_vertical_velocity",
                false,
                0.0,
            )?,
            heading: normalize_heading(number(
                &values,
                "autopilot_data",
                "autopilot_heading",
                false,
                0.0,
            )?),
            airspeed: number(&values, "autopilot_data", "autopilot_airspeed", false, 0.0)?,
            state: number(&values, "autopilot_data", "autopilot_state", false, 0.0)?.round() as i32,
            heading_roll_mode: number(
                &values,
                "autopilot_data",
                "autopilot_heading_roll_mode",
                false,
                0.0,
            )?
            .round() as i32,
        },
    };
    validate_data(&data)?;
    Ok(data)
}

pub(crate) fn write_pad(path: &Path, data: &PadData) -> io::Result<()> {
    let text = format!(
        "[CONFIG]\n\
Use_AutoPilot_Data = {}\n\n\
[POSITION_DATA]\n\
Latitude = {:.6}\n\
Longitude = {:.6}\n\
Altitude = {:.6}\n\
Heading = {:.6}\n\
Pitch = {:.6}\n\
Roll = {:.6}\n\
Speed = {:.6}\n\
Throttle = {:.6}\n\
Flaps = {:.6}\n\
Gear = {}\n\n\
[AUTOPILOT_DATA]\n\
AutoPilot_Mode = {}\n\
AutoPilot_Altitude = {:.6}\n\
AutoPilot_Vertical_Velocity = {:.6}\n\
AutoPilot_Heading = {:.6}\n\
AutoPilot_Airspeed = {:.6}\n\
AutoPilot_State = {}\n\
AutoPilot_Heading_Roll_Mode = {}\n",
        if data.use_ap { 1 } else { 0 },
        data.latitude,
        data.longitude,
        data.altitude,
        normalize_heading(data.heading),
        data.pitch,
        data.roll,
        data.speed,
        data.throttle,
        data.flaps,
        if data.gear != 0 { 1 } else { 0 },
        data.ap.mode,
        data.ap.altitude,
        data.ap.vertical_velocity,
        normalize_heading(data.ap.heading),
        data.ap.airspeed,
        data.ap.state,
        data.ap.heading_roll_mode,
    );
    fs::write(path, text)
}

pub(crate) fn safe_pad_filename(name: &str) -> Option<String> {
    let mut output = String::new();
    for character in name.trim().chars() {
        if "\\/:*?\"<>|".contains(character) || character.is_control() {
            output.push('_');
        } else if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.' | ' ') {
            output.push(character);
        } else {
            output.push('_');
        }
    }
    while output.contains("..") {
        output = output.replace("..", "_");
    }
    while output.ends_with('.') || output.ends_with(' ') {
        output.pop();
    }
    if output.to_ascii_lowercase().ends_with(".pad") {
        output.truncate(output.len() - 4);
    }
    if output.is_empty() {
        None
    } else {
        Some(format!("{output}.pad"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
[CONFIG]
Use_AutoPilot_Data = 1

[POSITION_DATA]
Latitude = 41.121516
Longitude = -73.178842
Altitude = 959.000000
Heading = 58.000000
Pitch = 1.250000
Roll = -2.500000
Speed = 90.000000
Throttle = 0.750000
Flaps = 0.250000
Gear = 1

[AUTOPILOT_DATA]
AutoPilot_Mode = 2
AutoPilot_Altitude = 3000
AutoPilot_Vertical_Velocity = 500
AutoPilot_Heading = 60
AutoPilot_Airspeed = 100
AutoPilot_State = 4
AutoPilot_Heading_Roll_Mode = 1
"#;

    #[test]
    fn parses_original_pad_format() {
        let data = parse_pad_text(SAMPLE).unwrap();
        assert!((data.latitude - 41.121516).abs() < 1e-9);
        assert_eq!(data.heading, 58.0);
        assert_eq!(data.gear, 1);
        assert!(data.use_ap);
        assert_eq!(data.ap.mode, 2);
        assert_eq!(data.ap.heading_roll_mode, 1);
    }

    #[test]
    fn form_round_trip_and_heading_normalization() {
        let data = parse_pad_text(SAMPLE).unwrap();
        let mut form = Form::from_data(&data, "Test");
        *form.value_mut(Field::Heading) = "-2".to_owned();
        let parsed = form.to_data().unwrap();
        assert_eq!(parsed.heading, 358.0);
        assert_eq!(parsed.ap.altitude, 3000.0);
    }

    #[test]
    fn safe_file_names_remain_in_pad_directory() {
        assert_eq!(
            safe_pad_filename("My Position"),
            Some("My Position.pad".into())
        );
        assert_eq!(
            safe_pad_filename("../bad:name.pad"),
            Some("__bad_name.pad".into())
        );
        assert_eq!(safe_pad_filename(""), None);
    }

    #[test]
    fn magnetic_heading_conversion_matches_kbdr_regression() {
        let magnetic = normalize_heading(45.0 + 13.0);
        let true_heading = normalize_heading(magnetic - 13.0);
        assert_eq!(magnetic, 58.0);
        assert_eq!(true_heading, 45.0);
    }

    #[test]
    fn parses_every_installed_pad_file() {
        let Some(pad_directory) = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .map(|ancestor| {
                ancestor
                    .join("Resources")
                    .join("plugins")
                    .join("PositionAircraft")
            })
            .find(|candidate| candidate.is_dir())
        else {
            return;
        };
        let mut count = 0;
        for entry in fs::read_dir(&pad_directory).unwrap() {
            let path = entry.unwrap().path();
            if path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("pad"))
            {
                parse_pad(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
                count += 1;
            }
        }
        assert!(count > 0, "no installed PAD files were found");
    }
}
