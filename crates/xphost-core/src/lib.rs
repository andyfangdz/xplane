//! Simulator-independent state and host protocol. No native pointers or SDK calls.
#![forbid(unsafe_code)]

mod model;
mod profile;
mod store;
pub use model::*;
pub use profile::*;
pub use store::*;
pub const PROTOCOL_VERSION: u32 = 2;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub profile: AircraftProfile,
    pub out_dir: String,
    pub system_path: String,
    pub plugin_path: String,
    pub acf_path: String,
    pub acf_name: String,
    pub datarefs: Option<String>,
    pub commands: Option<String>,
    pub lat: f64,
    pub lon: f64,
    pub elev: f64,
    pub heading: f64,
    pub magvar: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Request {
    Init(Box<Config>),
    Info {
        name: String,
        signature: String,
        description: String,
    },
    Enabled(bool),
    FlightStart(FlightStart),
    Step(f64),
    Time,
    Refs,
    Ref(usize),
    Commands,
    CommandInfo(usize),
    Read {
        name: String,
    },
    Write {
        name: String,
        value: Value,
        offset: usize,
    },
    Command {
        name: String,
        action: i32,
    },
    Nav(String),
    Stats,
    Misses,
    Log(String),
}

/// Implementations keep native calls on their owning simulation thread.
pub trait Backend {
    fn request(&self, request: Request) -> Result<Value, String>;
    fn time(&self) -> f64 {
        self.request(Request::Time)
            .ok()
            .and_then(|v| v.as_f64())
            .unwrap_or_default()
    }
    fn step(&self, dt: f64) -> Result<(), String> {
        self.request(Request::Step(dt)).map(|_| ())
    }
    fn read(&self, name: &str) -> Result<Value, String> {
        self.request(Request::Read { name: name.into() })
    }
    fn write(&self, name: &str, value: Value, offset: usize) -> Result<(), String> {
        self.request(Request::Write {
            name: name.into(),
            value,
            offset,
        })
        .map(|_| ())
    }
    fn command(&self, name: &str, action: i32) -> Result<(), String> {
        self.request(Request::Command {
            name: name.into(),
            action,
        })
        .map(|_| ())
    }
}
