use std::fs;
use std::path::{Path, PathBuf};

use super::support::log;

const METERS_PER_SECOND_TO_FPM: f32 = 196.850;

#[derive(Copy, Clone, Debug, PartialEq)]
pub(super) enum ShowDuration {
    Seconds(f32),
    UntilClosed,
}

pub(super) const SHOW_DURATIONS: [(&str, ShowDuration); 5] = [
    (" 5 seconds", ShowDuration::Seconds(5.0)),
    ("10 seconds", ShowDuration::Seconds(10.0)),
    ("30 seconds", ShowDuration::Seconds(30.0)),
    ("60 seconds", ShowDuration::Seconds(60.0)),
    ("Until closed", ShowDuration::UntilClosed),
];

#[derive(Clone, Debug)]
pub(super) struct Settings {
    pub(super) window_x: i32,
    pub(super) window_y: i32,
    pub(super) log_enabled: bool,
    pub(super) show_duration_index: usize,
    pub(super) show_in_replay: bool,
    path: PathBuf,
}

impl Settings {
    pub(super) fn load(preferences_directory: &Path) -> Self {
        let path = preferences_directory.join("xgs-rs.prf");
        let legacy_path = preferences_directory.join("xgs.prf");
        let source = if path.is_file() {
            Some((path.clone(), false))
        } else if legacy_path.is_file() {
            Some((legacy_path, true))
        } else {
            None
        };
        let mut settings = Self {
            window_x: 20,
            window_y: 600,
            log_enabled: false,
            show_duration_index: 3,
            show_in_replay: false,
            path,
        };
        if let Some((source_path, imported)) = source {
            match fs::read_to_string(&source_path) {
                Ok(text) => {
                    let values: Vec<_> = text.split_whitespace().collect();
                    if values.len() >= 5 {
                        settings.window_x = values[0].parse().unwrap_or(settings.window_x);
                        settings.window_y = values[1].parse().unwrap_or(settings.window_y);
                        settings.log_enabled = values[2].parse::<i32>().unwrap_or(0) != 0;
                        settings.show_duration_index = values[3]
                            .parse::<usize>()
                            .unwrap_or(settings.show_duration_index)
                            .min(SHOW_DURATIONS.len() - 1);
                        settings.show_in_replay = values[4].parse::<i32>().unwrap_or(0) != 0;
                        if imported {
                            log(&format!(
                                "imported legacy settings from {}",
                                source_path.display()
                            ));
                        }
                    }
                }
                Err(error) => log(&format!(
                    "could not read {}: {error}",
                    source_path.display()
                )),
            }
        }
        settings
    }

    pub(super) fn duration(&self) -> ShowDuration {
        SHOW_DURATIONS[self.show_duration_index].1
    }

    pub(super) fn save(&self) {
        let text = format!(
            "{} {} {} {} {}",
            self.window_x,
            self.window_y,
            i32::from(self.log_enabled),
            self.show_duration_index,
            i32::from(self.show_in_replay)
        );
        if let Err(error) = fs::write(&self.path, text) {
            log(&format!("could not write {}: {error}", self.path.display()));
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Rating {
    pub(super) limit_mps: f32,
    pub(super) text: String,
}

#[derive(Clone, Debug)]
pub(super) struct RatingScale(Vec<Rating>);

impl Default for RatingScale {
    fn default() -> Self {
        Self(vec![
            Rating {
                limit_mps: 0.5,
                text: "excellent landing".to_owned(),
            },
            Rating {
                limit_mps: 1.0,
                text: "good landing".to_owned(),
            },
            Rating {
                limit_mps: 1.5,
                text: "acceptable landing".to_owned(),
            },
            Rating {
                limit_mps: 2.0,
                text: "hard landing".to_owned(),
            },
            Rating {
                limit_mps: 2.5,
                text: "you are fired".to_owned(),
            },
            Rating {
                limit_mps: 3.0,
                text: "anybody survived?".to_owned(),
            },
            Rating {
                limit_mps: f32::INFINITY,
                text: "R.I.P.".to_owned(),
            },
        ])
    }
}

impl RatingScale {
    pub(super) fn text_for(&self, vertical_speed_mps: f32) -> &str {
        let speed = vertical_speed_mps.abs();
        self.0
            .iter()
            .find(|rating| speed <= rating.limit_mps)
            .or_else(|| self.0.last())
            .map(|rating| rating.text.as_str())
            .unwrap_or("landing")
    }

    fn parse(text: &str) -> Result<Self, String> {
        let mut lines = text
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'));
        if lines.next() != Some("V30") {
            return Err("rating file does not start with V30".to_owned());
        }
        let mut ratings = Vec::new();
        for line in lines.take(10) {
            let mut fields = line.splitn(3, ';');
            let meters_per_second = fields.next().unwrap_or_default().trim();
            let feet_per_minute = fields
                .next()
                .ok_or_else(|| format!("invalid rating line: {line}"))?
                .trim();
            let description = fields
                .next()
                .ok_or_else(|| format!("invalid rating line: {line}"))?
                .trim();
            if description.is_empty() {
                return Err(format!("rating text is empty: {line}"));
            }
            let limit_mps = if !meters_per_second.is_empty() {
                meters_per_second
                    .parse::<f32>()
                    .map_err(|_| format!("invalid m/s rating: {line}"))?
                    .abs()
            } else if !feet_per_minute.is_empty() {
                feet_per_minute
                    .parse::<f32>()
                    .map_err(|_| format!("invalid fpm rating: {line}"))?
                    .abs()
                    / METERS_PER_SECOND_TO_FPM
            } else {
                f32::INFINITY
            };
            ratings.push(Rating {
                limit_mps,
                text: description.to_owned(),
            });
            if limit_mps.is_infinite() {
                break;
            }
        }
        if ratings
            .last()
            .is_none_or(|rating| rating.limit_mps.is_finite())
        {
            return Err("rating file needs a final unlimited entry".to_owned());
        }
        if ratings
            .windows(2)
            .any(|pair| pair[0].limit_mps > pair[1].limit_mps)
        {
            return Err("rating limits must be in ascending order".to_owned());
        }
        Ok(Self(ratings))
    }

    fn load(path: &Path) -> Option<Self> {
        if !path.is_file() {
            return None;
        }
        match fs::read_to_string(path)
            .map_err(|error| error.to_string())
            .and_then(|text| Self::parse(&text))
        {
            Ok(scale) => {
                log(&format!("loaded rating configuration {}", path.display()));
                Some(scale)
            }
            Err(error) => {
                log(&format!(
                    "invalid rating configuration {}: {error}",
                    path.display()
                ));
                None
            }
        }
    }

    pub(super) fn for_aircraft(
        aircraft_directory: Option<&Path>,
        plugin_directory: &Path,
        icao: &str,
    ) -> Self {
        if let Some(directory) = aircraft_directory {
            if let Some(scale) = Self::load(&directory.join("xgs_rating.cfg")) {
                return scale;
            }
        }
        let mapping_path = plugin_directory.join("acf_mapping.cfg");
        if let Ok(mapping) = fs::read_to_string(&mapping_path) {
            for line in mapping.lines().map(str::trim) {
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                let mut fields = line.split_whitespace();
                if fields.next() == Some(icao) {
                    if let Some(file_name) = fields.next() {
                        if let Some(scale) = Self::load(&plugin_directory.join(file_name)) {
                            return scale;
                        }
                    }
                }
            }
        }
        Self::load(&plugin_directory.join("std_xgs_rating.cfg")).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_mps_and_fpm_ratings() {
        let scale = RatingScale::parse("V30\n0.5;;soft\n;240;firm\n;;severe\n").unwrap();
        assert_eq!(scale.text_for(-0.4), "soft");
        assert_eq!(scale.text_for(-0.8), "firm");
        assert_eq!(scale.text_for(-2.0), "severe");
    }

    #[test]
    fn rejects_missing_terminal_rating() {
        assert!(RatingScale::parse("V30\n0.5;;soft\n").is_err());
    }
}
