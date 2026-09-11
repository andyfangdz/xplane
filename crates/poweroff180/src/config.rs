use std::{collections::HashSet, fmt::Write};

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
    fn validate(&self) -> Result<(), String> {
        if self.run_token != self.run_token.floor() {
            return Err("run_token must be an integer".into());
        }
        if self.threshold_lat == self.end_lat && self.threshold_lon == self.end_lon {
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
