use poweroff180::{
    guidance::{clamp, rad},
    protocol::{self, Snapshot, NAMES},
    Config, Controller, Phase, Reason,
};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    ffi::c_void,
    fs::{self, OpenOptions},
    io::{BufWriter, Write},
    path::PathBuf,
    time::Instant,
};
use xplane_plugin::{
    command_once, current_aircraft_path, plugin_directory, Command, DataRef, DebugLogger,
    OwnedDataRef, OwnedFloatArray, PhaseFlightLoop, PluginStateSlot,
};
use xplane_sdk_sys::{
    xplm_CommandBegin, XPLMCommandPhase, XPLMCommandRef, XPLMPluginID, XPLM_MSG_PLANE_LOADED,
};

const LOG: DebugLogger = DebugLogger::new("[XPT Rust]");
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
    refs: RefCell<HashMap<&'static str, Option<DataRef>>>,
}
impl Native {
    fn find(&self, name: &'static str) -> Option<DataRef> {
        let mut refs = self.refs.borrow_mut();
        let r = refs.entry(name).or_insert(None);
        if r.is_none() {
            *r = DataRef::find(name);
        }
        *r
    }
    fn get(&self, name: &'static str) -> f64 {
        self.find(name)
            .map_or(0.0, |r| r.scalar().unwrap_or_else(|| r.array_element(0)))
    }
    fn set(&self, name: &'static str, value: f64) {
        if let Some(r) = self.find(name) {
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
    snapshot: OwnedFloatArray<75>,
    error: OwnedDataRef,
    _heartbeat: OwnedDataRef,
    _rust: OwnedDataRef,
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
            out: [0.0; 75],
            prior: [0.0; 75],
            flush_t: 0.0,
            last_flap: -1,
            snapshot,
            error,
            _heartbeat: heartbeat,
            _rust: rust,
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
        self.out[33..].fill(0.0);
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
            || self.native.get(NAMES[28]) != 0.0
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
                writeln!(trace, "{}", protocol::HEADER)?;
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
        self.controller.start(self.native.get(NAMES[0]));
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
        s[48] = c.predicted_cross as f32;
        s[49] = c.cross_accel as f32;
        s[52] = c.phase as u8 as f32;
        s[54] = c.bank as f32;
        s[55] = c.pitch as f32;
        s[56] = c.flap as f32;
        s[57] = c.throttle as f32;
        s[58] = c.lead as f32;
        s[59] = c.desired as f32;
        s[60] = c.accel as f32;
        s[61] = c.wind_ff as f32;
        s[62] = c.pitch_rate as f32;
        s[63] = c.dt as f32;
        s[64] = c.cut_t as f32;
        s[65] = c.roundout_t as f32;
        s[66] = c.reason as u8 as f32;
        s[67] = f32::from(c.running());
        s[68] = f32::from(self.configured);
        s[69] = 7.0;
        s[70] = c.gate_s as f32;
        s[71] = c.c.run_token as i32 as f32;
        s[72] = c.steps as f32;
        s[73] = heartbeat_age() as f32;
        s[74] = c.c.run_token as f32;
        self.snapshot.set(*s);
    }
    fn observe(&mut self) {
        let n = &self.native;
        let s = &mut self.out;
        for (i, name) in NAMES.iter().enumerate() {
            s[i] = n.get(name) as f32;
        }
        // The original protocol applies these two conversions in float precision.
        s[1] *= 3.280_84_f32;
        s[3] *= 1.943_844_4_f32;
        let mut ground = [0; 10];
        if let Some(r) = n.find(NAMES[15]) {
            r.read_i32(&mut ground);
        }
        s[15] = f32::from(ground.iter().any(|g| *g != 0));
        let c = self.controller.c;
        let scale = 60.0 * 6076.12;
        let ls = scale * rad((c.threshold_lat + c.end_lat) * 0.5).cos();
        let north = (c.end_lat - c.threshold_lat) * scale;
        let east = (c.end_lon - c.threshold_lon) * ls;
        let length = north.hypot(east);
        let un = north / length;
        let ue = east / length;
        let north = (n.get("sim/flightmodel/position/latitude") - c.threshold_lat) * scale;
        let east = (n.get("sim/flightmodel/position/longitude") - c.threshold_lon) * ls;
        s[29] = (north * un + east * ue) as f32;
        s[30] = (east * un - north * ue) as f32;
        s[31] = n.get("sim/flightmodel/position/hpath") as f32;
        s[32] = (n.get("sim/flightmodel/position/elevation") * 3.280839895) as f32;
        s[50] = f32::from(NAMES.iter().all(|name| n.find(name).is_some()));
        s[51] = 2.0;
        let was_running = self.controller.running();
        if was_running && heartbeat_age() > c.watchdog_wall_s {
            self.controller.abort(Reason::SupervisorLost);
        }
        if was_running && s[50] == 0.0 {
            self.controller.abort(Reason::MissingDataref);
        }
        if self.controller.running() && !n.paused() {
            if s[28] != 0.0 {
                self.controller.abort(Reason::OverrideConflict);
            }
            if self.controller.cut_t >= 0.0 && s[33] == 0.0 && s[15] != 0.0 && s[1] < 20.0 {
                s[33] = 1.0;
                s[34] = s[0];
                s[35] = s[29];
                s[36] = s[30];
                s[37] = s[2];
                s[38] = s[4];
                s[39] = self.prior[23] * 196.850_39_f32;
                s[40] = s[7];
                s[41] = s[10];
                s[42] = s[24] * 1.943_844_4_f32;
                s[43] = self.prior[29];
                s[44] = self.prior[0];
                s[45] = s[10];
                s[46] = s[1];
            }
            if s[33] != 0.0 {
                s[45] = s[45].max(s[10]);
                s[46] = s[46].max(s[1]);
                if s[15] == 0.0 {
                    s[47] += 1.0;
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
        self.out[53] += 1.0;
        self.publish();
        if was_running && !self.native.paused() {
            let flush = self.out[0] as f64 - self.flush_t >= 1.0 || !self.controller.running();
            if let Some(trace) = &mut self.trace {
                let result = (|| -> std::io::Result<()> {
                    for (i, value) in self.out.iter().enumerate() {
                        if i > 0 {
                            write!(trace, ",")?;
                        }
                        write!(trace, "{value}")?;
                    }
                    writeln!(trace)?;
                    if flush {
                        trace.flush()?;
                    }
                    Ok(())
                })();
                if flush {
                    self.flush_t = self.out[0] as f64;
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
        if self.out[15] == 0.0 && self.out[33] == 0.0 {
            self.prior = self.out;
        }
    }
}
unsafe extern "C" fn command(
    _: XPLMCommandRef,
    phase: XPLMCommandPhase,
    refcon: *mut c_void,
) -> i32 {
    if phase == xplm_CommandBegin {
        with_state(|s| match Command::identifier_from_refcon(refcon) {
            0 => s.configure(),
            1 => s.start_card(),
            _ => s.cancel(),
        });
    }
    1
}
unsafe extern "C" fn observe(_: f32, _: f32, _: i32, _: *mut c_void) -> f32 {
    with_state(Runtime::observe);
    -1.0
}
pub fn start() -> bool {
    match Runtime::new() {
        Ok(state) => {
            STATE.with(|s| s.replace(Some(state)));
            LOG.log("guidance algorithm v7, protocol v1 loaded inert");
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
            s.native.refs.borrow_mut().clear();
            s.publish();
        });
    }
}
