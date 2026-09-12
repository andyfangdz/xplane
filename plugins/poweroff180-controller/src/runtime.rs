use poweroff180::{
    guidance::clamp,
    protocol::{self, field, Snapshot, NAMES},
    Config, Controller, Phase, Reason,
};
use std::{
    cell::Cell,
    ffi::c_void,
    fs::{self, OpenOptions},
    io::{BufWriter, Write},
    path::PathBuf,
    time::Instant,
};
use uom::si::{
    f32::{Length as Length32, Velocity as Velocity32},
    velocity::{foot_per_minute, knot, meter_per_second},
};
use uom::si::{f64::Length, length::foot, length::meter};
use xplane_airports::GeoPoint;
use xplane_plugin::{
    command_once, current_aircraft_path, plugin_directory, Command, DataRefCache, DebugLogger,
    OwnedDataRef, OwnedFloatArray, PhaseFlightLoop, PluginStateSlot,
};
use xplane_sdk_sys::{
    xplm_CommandBegin, XPLMCommandPhase, XPLMCommandRef, XPLMPluginID, XPLM_MSG_PLANE_LOADED,
};

const LOG: DebugLogger = DebugLogger::new("[XPT Rust]");
// Appended to CSV evidence only; the existing HUD/supervisor snapshot stays v1.
const ATTITUDE_FIELDS: [(&str, &str); 7] = [
    ("attitude_armed", "sr20g6/test_controller/armed"),
    ("attitude_active", "sr20g6/test_controller/active"),
    (
        "attitude_release_reason",
        "sr20g6/test_controller/release_reason",
    ),
    (
        "attitude_frame_dt_s",
        "sim/operation/misc/frame_rate_period",
    ),
    (
        "attitude_override_roll",
        "sim/operation/override/override_joystick_roll",
    ),
    (
        "attitude_override_pitch",
        "sim/operation/override/override_joystick_pitch",
    ),
    (
        "attitude_override_yaw",
        "sim/operation/override/override_joystick_heading",
    ),
];
thread_local! {
    static STATE:PluginStateSlot<Runtime>=const {PluginStateSlot::new()};
    static HEARTBEAT:Cell<Option<Instant>>=const {Cell::new(None)};
}
fn heartbeat_written() {
    HEARTBEAT.with(|value| value.set(Some(Instant::now())));
}
fn heartbeat_age() -> f64 {
    HEARTBEAT.with(|value| value.get().map_or(0.0, |time| time.elapsed().as_secs_f64()))
}
fn with_state<T>(f: impl FnOnce(&mut Runtime) -> T) -> Option<T> {
    STATE.with(|s| s.with_mut(f))
}

#[derive(Default)]
struct Native {
    refs: DataRefCache,
}
impl Native {
    fn get(&self, name: &'static str) -> f64 {
        self.refs
            .find(name)
            .map_or(0.0, |r| r.scalar().unwrap_or_else(|| r.array_element(0)))
    }
    fn set(&self, name: &'static str, value: f64) {
        if let Some(r) = self.refs.find(name) {
            r.set_scalar(value);
        }
    }
    fn paused(&self) -> bool {
        self.get("sim/time/paused") == 1.0
    }
}
fn aircraft_allowed() -> bool {
    current_aircraft_path().is_some_and(|p| {
        p.to_string_lossy()
            .replace('\\', "/")
            .to_lowercase()
            .ends_with("/aircraft/x-aviation/torquesim sr20/sr20.acf")
    })
}
struct Runtime {
    native: Native,
    directory: PathBuf,
    controller: Controller,
    trace: Option<BufWriter<fs::File>>,
    configured: bool,
    out: Snapshot,
    prior: Snapshot,
    flush_t: f64,
    last_flap: i32,
    snapshot: OwnedFloatArray<{ protocol::LENGTH }>,
    error: OwnedDataRef,
    _heartbeat: OwnedDataRef,
    _rust: OwnedDataRef,
    _patch: OwnedDataRef,
    _commands: Vec<Command>,
    _flight_loop: PhaseFlightLoop,
}
impl Runtime {
    fn new() -> Result<Self, String> {
        let directory = plugin_directory().ok_or("cannot resolve plugin directory")?;
        let snapshot = OwnedFloatArray::new("xpt/snapshot")?;
        let heartbeat = OwnedDataRef::integer("xpt/heartbeat", 0, true, Some(heartbeat_written))?;
        let error = OwnedDataRef::integer("xpt/configuration_error", 0, false, None)?;
        let rust = OwnedDataRef::integer("xpt/rust_implementation", 1, false, None)?;
        let commands = [
            (
                "xpt/configure",
                "Read and validate the complete staged test card while paused",
            ),
            ("xpt/start", "Start the configured test while paused"),
            ("xpt/abort", "Abort, pause and release test authority"),
        ]
        .iter()
        .enumerate()
        .map(|(id, (name, description))| {
            Command::create(name, description, Some(command), true, id)
        })
        .collect::<Result<Vec<_>, _>>()?;
        let flight_loop = PhaseFlightLoop::after_physics(Some(observe))?;
        heartbeat_written();
        Ok(Self {
            native: Native::default(),
            directory,
            controller: Controller::default(),
            trace: None,
            configured: false,
            out: [0.0; protocol::LENGTH],
            prior: [0.0; protocol::LENGTH],
            flush_t: 0.0,
            last_flap: -1,
            snapshot,
            error,
            _heartbeat: heartbeat,
            _rust: rust,
            _patch: OwnedDataRef::integer("xpt/version_patch", 3, false, None)?,
            _commands: commands,
            _flight_loop: flight_loop,
        })
    }
    fn release(&self) {
        self.native
            .set("sim/cockpit2/engine/actuators/throttle_ratio_all", 0.0);
        self.native.set("sr20g6/test_controller/armed", 0.0);
        self.native
            .set("sim/cockpit2/controls/left_brake_ratio", 0.0);
        self.native
            .set("sim/cockpit2/controls/right_brake_ratio", 0.0);
        command_once("sim/operation/pause_on");
    }
    fn close_trace(&mut self) {
        if let Some(mut trace) = self.trace.take() {
            if let Err(e) = trace.flush() {
                LOG.log(&format!("trace flush failed: {e}"));
                self.controller.abort(Reason::TraceError);
            }
        }
    }
    fn configure(&mut self) {
        if self.controller.running() || !self.native.paused() || !aircraft_allowed() {
            self.error.set_i32(1);
            return;
        }
        self.close_trace();
        let parsed = fs::read_to_string(self.directory.join("active-card.ini"))
            .map_err(|e| e.to_string())
            .and_then(|text| Config::parse(&text));
        let c = match parsed {
            Ok(c) => c,
            Err(e) => {
                self.error.set_i32(1);
                self.configured = false;
                self.controller.abort(Reason::InvalidConfig);
                LOG.log(&format!("configuration rejected: {e}"));
                self.publish();
                return;
            }
        };
        self.controller.reset(c);
        self.configured = true;
        self.error.set_i32(0);
        self.out[field::CONTACT_LATCHED..].fill(0.0);
        if let Err(e) = fs::write(self.directory.join("effective-card.ini"), c.text()) {
            self.configured = false;
            self.error.set_i32(1);
            self.controller.abort(Reason::TraceError);
            LOG.log(&format!("configuration readback failed: {e}"));
        }
        self.publish();
    }
    fn start_card(&mut self) {
        if !self.configured
            || self.controller.phase != Phase::Ready
            || !self.native.paused()
            || self.native.get("sr20g6/test_controller/armed") != 1.0
            || self.native.get(NAMES[field::OVERRIDE_PATH]) != 0.0
            || !aircraft_allowed()
        {
            return;
        }
        let path = self
            .directory
            .join(format!("trace-{}.csv", self.controller.c.run_token as i32));
        let result = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .and_then(|file| {
                let mut trace = BufWriter::new(file);
                write!(trace, "{}", protocol::HEADER)?;
                for (name, _) in ATTITUDE_FIELDS {
                    write!(trace, ",{name}")?;
                }
                writeln!(trace, ",flare_float_active,flare_predicted_touchdown_ft")?;
                Ok(trace)
            });
        match result {
            Ok(trace) => self.trace = Some(trace),
            Err(e) => {
                self.controller.abort(Reason::TraceError);
                LOG.log(&format!("cannot create trace: {e}"));
                self.publish();
                return;
            }
        }
        heartbeat_written();
        self.last_flap = -1;
        self.flush_t = 0.0;
        self.controller
            .start(self.native.get(NAMES[field::SIM_TIME]));
        self.publish();
    }
    fn cancel(&mut self) {
        if self.controller.running() {
            self.controller.abort(Reason::Cancelled);
            self.release();
        }
        self.close_trace();
        self.publish();
    }
    fn publish(&mut self) {
        let c = &self.controller;
        let s = &mut self.out;
        s[field::PREDICTED_CROSS_FT] = c.predicted_cross as f32;
        s[field::CROSS_ACCEL_FPS2] = c.cross_accel as f32;
        s[field::PHASE_ID] = c.phase as u8 as f32;
        s[field::BANK_COMMAND] = c.bank as f32;
        s[field::PITCH_COMMAND] = c.pitch as f32;
        s[field::FLAP_COMMAND] = c.flap as f32;
        s[field::THROTTLE_COMMAND] = c.throttle as f32;
        s[field::TURN_LEAD_FT] = c.lead as f32;
        s[field::DESIRED_VERTICAL_FPS] = c.desired as f32;
        s[field::VERTICAL_ACCEL_FPS2] = c.accel as f32;
        s[field::WIND_PITCH_RATE_FF] = c.wind_ff as f32;
        s[field::ROUNDOUT_PITCH_RATE_COMMAND] = c.pitch_rate as f32;
        s[field::CONTROL_DT_S] = c.dt as f32;
        s[field::CUT_SIM_TIME] = c.cut_t as f32;
        s[field::ROUNDOUT_SIM_TIME] = c.roundout_t as f32;
        s[field::REASON_ID] = c.reason as u8 as f32;
        s[field::NATIVE_RUNNING] = f32::from(c.running());
        s[field::CONFIGURED] = f32::from(self.configured);
        s[field::NATIVE_VERSION] = 8.0;
        s[field::ENTRY_GATE_S] = c.gate_s as f32;
        s[field::CONFIG_TOKEN] = c.c.run_token as i32 as f32;
        s[field::NATIVE_STEPS] = c.steps as f32;
        s[field::HEARTBEAT_AGE_S] = heartbeat_age() as f32;
        s[field::RUN_TOKEN] = c.c.run_token as f32;
        self.snapshot.set(*s);
    }
    fn observe(&mut self) {
        let n = &self.native;
        let attitude = ATTITUDE_FIELDS.map(|(_, name)| n.get(name));
        let s = &mut self.out;
        for (i, name) in NAMES.iter().enumerate() {
            s[i] = n.get(name) as f32;
        }
        // Convert SDK values to the documented f32 snapshot units.
        s[field::AGL_FT] = Length32::new::<meter>(s[field::AGL_FT]).get::<foot>();
        s[field::GROUNDSPEED_KT] =
            Velocity32::new::<meter_per_second>(s[field::GROUNDSPEED_KT]).get::<knot>();
        let mut ground = [0; 10];
        if let Some(r) = n.refs.find(NAMES[field::GROUND_ANY]) {
            r.read_i32(&mut ground);
        }
        s[field::GROUND_ANY] = f32::from(ground.iter().any(|g| *g != 0));
        let c = self.controller.c;
        let (east, north) = c.runway_projection().project(GeoPoint {
            lat: n.get("sim/flightmodel/position/latitude"),
            lon: n.get("sim/flightmodel/position/longitude"),
            elevation: Length::new::<meter>(0.0),
        });
        let (along, cross) = c.runway_axis().map_or(
            (Length::new::<foot>(f64::NAN), Length::new::<foot>(f64::NAN)),
            |axis| axis.offsets(east, north),
        );
        s[field::RUNWAY_ALONG_FT] = along.get::<foot>() as f32;
        s[field::RUNWAY_CROSS_FT] = cross.get::<foot>() as f32;
        s[field::GROUND_TRACK_TRUE_DEG] = n.get("sim/flightmodel/position/hpath") as f32;
        s[field::ELEVATION_MSL_FT] =
            Length::new::<meter>(n.get("sim/flightmodel/position/elevation")).get::<foot>() as f32;
        s[field::TELEMETRY_READY] = f32::from(NAMES.iter().all(|name| n.refs.find(name).is_some()));
        s[field::TELEMETRY_VERSION] = 2.0;
        let was_running = self.controller.running();
        if was_running && heartbeat_age() > c.watchdog_wall_s {
            self.controller.abort(Reason::SupervisorLost);
        }
        if was_running && s[field::TELEMETRY_READY] == 0.0 {
            self.controller.abort(Reason::MissingDataref);
        }
        // The before-physics attitude loop must own all axes on every flying
        // frame. A transient reset cannot be hidden by a later re-engagement.
        if self.controller.running() && !n.paused() {
            if ATTITUDE_FIELDS
                .iter()
                .any(|(_, name)| n.refs.find(name).is_none())
            {
                self.controller.abort(Reason::MissingDataref);
            } else if attitude[0] != 1.0
                || attitude[1] != 1.0
                || attitude[2] != 0.0
                || attitude[4..].iter().any(|value| *value != 1.0)
            {
                self.controller.abort(Reason::AttitudeLost);
            }
        }
        if self.controller.running() && !n.paused() {
            if s[field::OVERRIDE_PATH] != 0.0 {
                self.controller.abort(Reason::OverrideConflict);
            }
            if self.controller.cut_t >= 0.0
                && s[field::CONTACT_LATCHED] == 0.0
                && s[field::GROUND_ANY] != 0.0
                && s[field::AGL_FT] < 20.0
            {
                s[field::CONTACT_LATCHED] = 1.0;
                s[field::FIRST_SIM_TIME] = s[field::SIM_TIME];
                s[field::FIRST_ALONG_FT] = s[field::RUNWAY_ALONG_FT];
                s[field::FIRST_CROSS_FT] = s[field::RUNWAY_CROSS_FT];
                s[field::FIRST_KIAS] = s[field::IAS_KIAS];
                s[field::FIRST_INDICATED_FPM] = s[field::VVI_FPM];
                s[field::FIRST_PHYSICAL_FPM] =
                    Velocity32::new::<meter_per_second>(self.prior[field::VERTICAL_SPEED_MPS])
                        .get::<foot_per_minute>();
                s[field::FIRST_PITCH_DEG] = s[field::PITCH_DEG];
                s[field::FIRST_NORMAL_G] = s[field::NORMAL_G];
                s[field::FIRST_LOCAL_WIND_KT] =
                    Velocity32::new::<meter_per_second>(s[field::WIND_SPEED_MPS]).get::<knot>();
                s[field::LAST_AIRBORNE_ALONG_FT] = self.prior[field::RUNWAY_ALONG_FT];
                s[field::LAST_AIRBORNE_SIM_TIME] = self.prior[field::SIM_TIME];
                s[field::POST_CONTACT_MAX_G] = s[field::NORMAL_G];
                s[field::POST_CONTACT_MAX_AGL_FT] = s[field::AGL_FT];
            }
            if s[field::CONTACT_LATCHED] != 0.0 {
                s[field::POST_CONTACT_MAX_G] = s[field::POST_CONTACT_MAX_G].max(s[field::NORMAL_G]);
                s[field::POST_CONTACT_MAX_AGL_FT] =
                    s[field::POST_CONTACT_MAX_AGL_FT].max(s[field::AGL_FT]);
                if s[field::GROUND_ANY] == 0.0 {
                    s[field::POST_CONTACT_AIR_FRAMES] += 1.0;
                }
            }
            let sample = protocol::sample(s);
            self.controller.step(sample);
            let control = &self.controller;
            if control.running() {
                n.set("sr20g6/test_controller/target_bank_deg", control.bank);
                n.set("sr20g6/test_controller/target_pitch_deg", control.pitch);
                n.set(
                    "sim/cockpit2/engine/actuators/throttle_ratio_all",
                    control.throttle,
                );
                let detent = if control.flap < 0.25 {
                    0
                } else if control.flap < 0.75 {
                    1
                } else {
                    2
                };
                if detent != self.last_flap {
                    n.set("sim/cockpit2/controls/flap_ratio", control.flap);
                    n.set("afm/sr/switches/flaps", f64::from(detent));
                    self.last_flap = detent;
                }
                if control.phase == Phase::Rollout {
                    n.set(
                        "sr20g6/test_controller/ground_target_heading_deg",
                        control.heading
                            + clamp(
                                -0.35 * sample.y - 0.7 * sample.cross_velocity(control.heading),
                                -5.0,
                                5.0,
                            ),
                    );
                    n.set("sim/cockpit2/controls/left_brake_ratio", 0.2);
                    n.set("sim/cockpit2/controls/right_brake_ratio", 0.2);
                }
            }
        }
        self.out[field::SEQUENCE] += 1.0;
        self.publish();
        if was_running && !self.native.paused() {
            let flush = self.out[field::SIM_TIME] as f64 - self.flush_t >= 1.0
                || !self.controller.running();
            if let Some(trace) = &mut self.trace {
                let result = (|| -> std::io::Result<()> {
                    for (i, value) in self.out.iter().enumerate() {
                        if i > 0 {
                            write!(trace, ",")?;
                        }
                        write!(trace, "{value}")?;
                    }
                    for value in attitude {
                        write!(trace, ",{value}")?;
                    }
                    writeln!(
                        trace,
                        ",{},{:.6}",
                        u8::from(self.controller.float_guard_active),
                        self.controller.predicted_touchdown_ft
                    )?;
                    if flush {
                        trace.flush()?;
                    }
                    Ok(())
                })();
                if flush {
                    self.flush_t = self.out[field::SIM_TIME] as f64;
                }
                if let Err(e) = result {
                    self.controller.abort(Reason::TraceError);
                    LOG.log(&format!("trace write failed: {e}"));
                }
            }
        }
        if was_running && !self.controller.running() {
            self.release();
            self.close_trace();
            self.publish();
        }
        if self.out[field::GROUND_ANY] == 0.0 && self.out[field::CONTACT_LATCHED] == 0.0 {
            self.prior = self.out;
        }
    }
}
extern "C" fn command(_: XPLMCommandRef, phase: XPLMCommandPhase, refcon: *mut c_void) -> i32 {
    if phase == xplm_CommandBegin {
        with_state(|s| match Command::identifier_from_refcon(refcon) {
            0 => s.configure(),
            1 => s.start_card(),
            _ => s.cancel(),
        });
    }
    1
}
extern "C" fn observe(_: f32, _: f32, _: i32, _: *mut c_void) -> f32 {
    with_state(Runtime::observe);
    -1.0
}
pub fn start() -> bool {
    match Runtime::new() {
        Ok(state) => {
            STATE.with(|s| s.replace(Some(state)));
            LOG.log("guidance algorithm v8, runtime 0.8.3, protocol v1 loaded inert; attitude and predictive flare guards enabled");
            true
        }
        Err(e) => {
            LOG.log(&e);
            false
        }
    }
}
pub fn stop() {
    disable();
    STATE.with(|s| s.replace(None));
}
pub fn enable() -> bool {
    true
}
pub fn disable() {
    with_state(Runtime::cancel);
}
pub fn receive_message(_: XPLMPluginID, message: i32, aircraft: *mut c_void) {
    if message == XPLM_MSG_PLANE_LOADED as i32 && aircraft.is_null() {
        with_state(|s| {
            s.cancel();
            s.configured = false;
            s.native.refs.clear();
            s.publish();
        });
    }
}
