//! Aircraft assumptions are data, separate from simulator/SDK behavior.
use crate::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AircraftProfile {
    pub schema_version: u32,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub parked_height_m: f64,
    pub mass_kg: Option<f64>,
    pub tail_number: Option<String>,
    /// Zero means unspecified. It does not infer an engine from the package name.
    pub engine_count: usize,
    pub avionics: Option<AvionicsProfile>,
    #[serde(default)]
    pub engine_indicators: Vec<EngineIndicator>,
    /// Simulator-owned initial values, applied after common parked-state seeds.
    #[serde(default)]
    pub datarefs: BTreeMap<String, ProfileValue>,
    /// Applied after plugin enable, before the three initial lifecycle messages.
    #[serde(default)]
    pub cold: BTreeMap<String, ProfileValue>,
    #[serde(default)]
    pub running: BTreeMap<String, ProfileValue>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AvionicsProfile {
    pub bus_index: usize,
    pub nominal_volts: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EngineIndicator {
    pub source: String,
    pub destination: String,
    pub scale: f64,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlightStart {
    Cold,
    Running,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(
    tag = "type",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum ProfileValue {
    Int(i32),
    Float(f32),
    Double(f64),
    FloatArray(Vec<f32>),
    IntArray(Vec<i32>),
    Bytes(String),
}
impl ProfileValue {
    pub fn types(&self) -> i32 {
        match self {
            Self::Int(_) => INT,
            Self::Float(_) => FLOAT,
            Self::Double(_) => DOUBLE,
            Self::FloatArray(_) => FLOAT_ARRAY,
            Self::IntArray(_) => INT_ARRAY,
            Self::Bytes(_) => DATA,
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        let valid = match self {
            Self::Int(_) => true,
            Self::Float(v) => v.is_finite(),
            Self::Double(v) => v.is_finite(),
            Self::FloatArray(v) => v.len() <= MAX_ELEMENTS && v.iter().all(|v| v.is_finite()),
            Self::IntArray(v) => v.len() <= MAX_ELEMENTS,
            Self::Bytes(v) => v.len() < MAX_ELEMENTS && !v.contains('\0'),
        };
        if valid {
            Ok(())
        } else {
            Err("invalid or oversized profile value".into())
        }
    }
    pub fn json_value(&self) -> serde_json::Value {
        match self {
            Self::Int(v) => serde_json::json!(v),
            Self::Float(v) => serde_json::json!(v),
            Self::Double(v) => serde_json::json!(v),
            Self::FloatArray(v) => serde_json::json!(v),
            Self::IntArray(v) => serde_json::json!(v),
            Self::Bytes(v) => serde_json::json!(v.as_bytes()),
        }
    }
}
impl AircraftProfile {
    pub fn builtin(name: &str) -> Result<Self, String> {
        let text = match name {
            "generic" => include_str!("../profiles/generic.json"),
            "c182" => include_str!("../profiles/c182.json"),
            _ => return Err(format!("unknown built-in aircraft profile: {name}")),
        };
        Self::from_json(text)
    }
    pub fn from_json(text: &str) -> Result<Self, String> {
        let profile: Self =
            serde_json::from_str(text).map_err(|e| format!("invalid aircraft profile: {e}"))?;
        profile.validate()?;
        Ok(profile)
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1 {
            return Err("unsupported aircraft profile schema_version (expected 1)".into());
        }
        if self.name.trim().is_empty() || self.name.contains('\0') {
            return Err("aircraft profile needs a name without NUL bytes".into());
        }
        if !self.parked_height_m.is_finite() || self.parked_height_m < 0. {
            return Err("profile parked_height_m must be finite and nonnegative".into());
        }
        if self.mass_kg.is_some_and(|v| !v.is_finite() || v <= 0.) {
            return Err("profile mass_kg must be finite and positive".into());
        }
        if self.engine_count > 16 {
            return Err("profile engine_count must be in 0..16".into());
        }
        if self
            .tail_number
            .as_ref()
            .is_some_and(|v| v.contains('\0') || v.len() >= MAX_ELEMENTS)
        {
            return Err("invalid profile tail_number".into());
        }
        if self.avionics.as_ref().is_some_and(|a| {
            a.bus_index >= MAX_ELEMENTS || !a.nominal_volts.is_finite() || a.nominal_volts <= 0.
        }) {
            return Err(
                "profile avionics needs a bounded bus_index and positive finite nominal_volts"
                    .into(),
            );
        }
        for mapping in &self.engine_indicators {
            if !mapping.source.starts_with("sim/")
                || !mapping.destination.starts_with("sim/")
                || mapping.source.contains('\0')
                || mapping.destination.contains('\0')
                || !mapping.scale.is_finite()
            {
                return Err("invalid simulator engine indicator mapping".into());
            }
        }
        for (name, value) in self.datarefs.iter().chain(&self.cold).chain(&self.running) {
            if !name.starts_with("sim/") || name.contains('\0') {
                return Err(format!(
                    "profile seeds must name simulator-owned datarefs: {name}"
                ));
            }
            value.validate().map_err(|e| format!("{name}: {e}"))?;
        }
        Ok(())
    }
    pub fn start_values(&self, start: FlightStart) -> &BTreeMap<String, ProfileValue> {
        match start {
            FlightStart::Cold => &self.cold,
            FlightStart::Running => &self.running,
        }
    }
    /// All engine slots use the same declared mapping; no special engine zero.
    pub fn refresh_indicators(&self, store: &mut Store) {
        for mapping in &self.engine_indicators {
            let Some(src) = store.find(&mapping.source) else {
                continue;
            };
            let Some(dst) = store.find(&mapping.destination) else {
                continue;
            };
            let values: Vec<_> = store.entries[src]
                .floats
                .iter()
                .take(self.engine_count)
                .map(|&v| (v as f64 * mapping.scale) as f32)
                .collect();
            for (dest, value) in store.entries[dst].floats.iter_mut().zip(values) {
                *dest = value;
            }
        }
    }
    pub fn avionics_ratio(&self, store: &Store) -> f32 {
        let Some(avionics) = &self.avionics else {
            return 0.;
        };
        let voltage = store
            .find("sim/cockpit2/electrical/bus_volts")
            .and_then(|id| store.entries[id].floats.get(avionics.bus_index))
            .copied()
            .unwrap_or_default();
        (voltage / avionics.nominal_volts).clamp(0., 1.)
    }
}
impl Store {
    pub fn seed_profile_value(&mut self, name: &str, value: &ProfileValue) -> Result<(), String> {
        value.validate()?;
        if let Some(id) = self.find(name) {
            let e = &self.entries[id];
            if e.owned || e.types & value.types() == 0 {
                return Err(format!(
                    "profile type conflicts with dataref catalog: {name}"
                ));
            }
        }
        let e = self.ensure(name, value.types());
        match value {
            ProfileValue::Int(v) => e.scalar = *v as f64,
            ProfileValue::Float(v) => e.scalar = *v as f64,
            ProfileValue::Double(v) => e.scalar = *v,
            ProfileValue::FloatArray(v) => {
                if e.floats.len() < v.len() {
                    e.floats.resize(v.len(), 0.);
                }
                e.floats[..v.len()].copy_from_slice(v);
            }
            ProfileValue::IntArray(v) => {
                if e.ints.len() < v.len() {
                    e.ints.resize(v.len(), 0);
                }
                e.ints[..v.len()].copy_from_slice(v);
            }
            ProfileValue::Bytes(v) => {
                e.bytes = v.as_bytes().to_vec();
                e.bytes.push(0);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn profiles_isolate_aircraft_assumptions() {
        let generic = AircraftProfile::builtin("generic").unwrap();
        assert!(generic.mass_kg.is_none());
        assert!(generic.avionics.is_none());
        assert_eq!(generic.engine_count, 0);
        let c182 = AircraftProfile::builtin("c182").unwrap();
        assert_eq!(c182.mass_kg, Some(1200.));
        assert_eq!(
            c182.start_values(FlightStart::Running)["sim/cockpit2/engine/actuators/ignition_key"]
                .json_value(),
            serde_json::json!([3])
        );
    }
    #[test]
    fn validates_and_updates_multiple_engines_and_selected_bus() {
        let mut profile = AircraftProfile::builtin("c182").unwrap();
        profile.engine_count = 2;
        profile.avionics = Some(AvionicsProfile {
            bus_index: 1,
            nominal_volts: 12.,
        });
        let mut store = Store::default();
        for (name, value) in [
            ("sim/flightmodel/engine/ENGN_tacrad", vec![1., 2.]),
            (
                "sim/cockpit2/engine/indicators/engine_speed_rpm",
                vec![0., 0.],
            ),
            ("sim/cockpit2/electrical/bus_volts", vec![0., 6.]),
        ] {
            store
                .seed_profile_value(name, &ProfileValue::FloatArray(value))
                .unwrap();
        }
        profile.refresh_indicators(&mut store);
        let rpm = &store.entries[store
            .find("sim/cockpit2/engine/indicators/engine_speed_rpm")
            .unwrap()]
        .floats;
        assert!((rpm[1] - 2. * rpm[0]).abs() < 1e-5);
        assert_eq!(profile.avionics_ratio(&store), 0.5);
        profile.avionics.as_mut().unwrap().nominal_volts = 0.;
        assert!(profile.validate().is_err());
        assert!(AircraftProfile::from_json(r#"{"schema_version":99}"#).is_err());
    }
    #[test]
    fn catalog_metadata_and_array_tail_survive_profile_seeding() {
        let mut store = Store::default();
        store.load_catalog("sim/array\tint[4]\tn").unwrap();
        store
            .seed_profile_value("sim/array", &ProfileValue::IntArray(vec![3, 4]))
            .unwrap();
        let e = &store.entries[0];
        assert!(!e.writable);
        assert_eq!(e.ints, [3, 4, 0, 0]);
        assert!(store
            .seed_profile_value("sim/array", &ProfileValue::Float(1.))
            .is_err());
    }
}
