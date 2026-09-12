use std::{collections::HashSet, fmt::Write};
use uom::si::{angle::degree, f64::Angle, f64::Length, length::meter, length::nautical_mile};
use xplane_airports::{GeoPoint, LocalProjection, RunwayAxis};

/// A complete card is validated before it can replace active configuration.
macro_rules! parameters {
    ($($name:ident: ($default:expr, $min:expr, $max:expr),)*) => {
        #[derive(Debug, Clone, Copy, PartialEq)]
        pub struct Config { $(pub $name: f64,)* }
        impl Default for Config {
            fn default() -> Self { Self { $($name: $default,)* } }
        }
        impl Config {
            pub const SCHEMA: &'static [(&'static str, f64, f64, f64)] = &[$((stringify!($name), $default, $min, $max),)*];
            pub fn parse(text: &str) -> Result<Self, String> {
                let mut candidate = Self::default();
                let mut seen = HashSet::new();
                for line in text.lines().filter(|line| !line.is_empty()) {
                    let (key, number) = line.split_once('=').ok_or("missing equals")?;
                    if !seen.insert(key) { return Err(format!("duplicate {key}")); }
                    let value: f64 = number.parse().map_err(|_| format!("invalid numeric value {key}"))?;
                    if !value.is_finite() { return Err(format!("non-finite {key}")); }
                    match key {
                        $(stringify!($name) => {
                            if !($min..=$max).contains(&value) { return Err(format!("range {key}")); }
                            candidate.$name = value;
                        },)*
                        _ => return Err(format!("unknown {key}")),
                    }
                }
                if seen.len() != Self::SCHEMA.len() { return Err("incomplete configuration".into()); }
                candidate.validate()?;
                Ok(candidate)
            }
            pub fn text(&self) -> String {
                let mut text = String::new();
                $(writeln!(&mut text, "{}={}", stringify!($name), self.$name).unwrap();)*
                text
            }
        }
    };
}
include!(concat!(env!("OUT_DIR"), "/parameters.rs"));

impl Config {
    /// Fixed midpoint-latitude projection using 60 nautical miles per degree.
    pub fn runway_projection(&self) -> LocalProjection {
        LocalProjection::new(
            GeoPoint {
                lat: self.threshold_lat,
                lon: self.threshold_lon,
                elevation: Length::new::<meter>(0.0),
            },
            Angle::new::<degree>((self.threshold_lat + self.end_lat) * 0.5),
            Length::new::<nautical_mile>(60.0),
        )
    }

    /// Axis from the configured usable threshold to the opposite end.
    pub fn runway_axis(&self) -> Option<RunwayAxis> {
        self.runway_projection().axis_to(GeoPoint {
            lat: self.end_lat,
            lon: self.end_lon,
            elevation: Length::new::<meter>(0.0),
        })
    }

    fn validate(&self) -> Result<(), String> {
        if self.run_token != self.run_token.floor() {
            return Err("run_token must be an integer".into());
        }
        if self.flare_float_enabled != self.flare_float_enabled.floor()
            || self.flare_float_height_ft >= self.flare_height_ft
            || self.flare_float_contact_height_ft >= self.flare_float_height_ft
            || self.flare_float_sink_fps < self.flare_contact_sink_fps
        {
            return Err("late-flare float correction".into());
        }
        if self.runway_axis().is_none() {
            return Err("empty runway".into());
        }
        if self.capture_blend_start_deg <= self.capture_blend_full_deg {
            return Err("capture blend interval".into());
        }
        if self.deceleration_start_height_ft <= self.deceleration_end_height_ft
            || self.deceleration_end_height_ft < self.flare_height_ft
            || self.landing_entry_kias > self.final_kias
        {
            return Err("short-final deceleration interval".into());
        }
        Ok(())
    }
}
