// SPDX-License-Identifier: GPL-3.0-or-later
use std::ffi::c_void;
use xplane_attitude::flight::{
    float_writable, FlightController, Release, Sample, DOUBLE_NAMES, FLOAT_NAMES, INT_NAMES,
};
use xplane_plugin::{
    current_aircraft_path, DataRef, DebugLogger, OwnedDataRef, PhaseFlightLoop, PluginStateSlot,
};
use xplane_sdk_sys::{XPLMPluginID, XPLM_MSG_PLANE_LOADED};

const LOG: DebugLogger = DebugLogger::new("[XPT Rust attitude]");
thread_local! { static STATE:PluginStateSlot<Runtime> = const {PluginStateSlot::new()}; }
fn with_state<T>(f: impl FnOnce(&mut Runtime) -> T) -> Option<T> {
    STATE.with(|s| s.with_mut(f))
}
fn input_written() {
    with_state(Runtime::inputs);
}
fn armed_written() {
    with_state(|s| s.integer_input(4));
}
fn mode_written() {
    with_state(|s| s.integer_input(7));
}

const SAMPLE_NAMES: [&str; 18] = [
    "sim/flightmodel/position/phi",
    "sim/flightmodel/position/theta",
    "sim/flightmodel/position/psi",
    "sim/flightmodel/position/Prad",
    "sim/flightmodel/position/Qrad",
    "sim/flightmodel/position/Rrad",
    "sim/flightmodel/position/beta",
    "sim/flightmodel/position/indicated_airspeed",
    "sim/flightmodel/position/true_airspeed",
    "sim/operation/misc/frame_rate_period",
    "sim/time/paused",
    "sim/time/is_in_replay",
    "sim/flightmodel2/gear/on_ground",
    "sim/flightmodel/position/vh_ind_fpm",
    "sim/flightmodel/position/latitude",
    "sim/flightmodel/position/longitude",
    "sim/flightmodel/position/elevation",
    "sim/cockpit2/engine/actuators/throttle_ratio_all",
];
fn required<const N: usize>(names: [&str; N]) -> Result<[DataRef; N], String> {
    names
        .into_iter()
        .map(DataRef::required)
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| "dataref count mismatch".into())
}
struct Runtime {
    controller: FlightController,
    ints: Vec<OwnedDataRef>,
    floats: Vec<OwnedDataRef>,
    doubles: Vec<OwnedDataRef>,
    sample: [DataRef; 18],
    overrides: [DataRef; 3],
    axes: [DataRef; 3],
    applied_ownership: [bool; 3],
    _rust: OwnedDataRef,
    _loop: PhaseFlightLoop,
}
impl Runtime {
    fn new() -> Result<Self, String> {
        let controller = FlightController::default();
        let ints = INT_NAMES
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let writable = i == 4 || i == 7;
                OwnedDataRef::integer(
                    &format!("sr20g6/test_controller/{name}"),
                    controller.ints[i],
                    writable,
                    match i {
                        4 => Some(armed_written),
                        7 => Some(mode_written),
                        _ => None,
                    },
                )
            })
            .collect::<Result<_, _>>()?;
        let floats = FLOAT_NAMES
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let writable = float_writable(i);
                OwnedDataRef::float(
                    &format!("sr20g6/test_controller/{name}"),
                    controller.floats[i],
                    writable,
                    if writable { Some(input_written) } else { None },
                )
            })
            .collect::<Result<_, _>>()?;
        let doubles = DOUBLE_NAMES
            .iter()
            .map(|name| {
                OwnedDataRef::double(&format!("sr20g6/test_controller/{name}"), 0.0, false, None)
            })
            .collect::<Result<_, _>>()?;
        let sample = required(SAMPLE_NAMES)?;
        let overrides = required([
            "sim/operation/override/override_joystick_roll",
            "sim/operation/override/override_joystick_pitch",
            "sim/operation/override/override_joystick_heading",
        ])?;
        let axes = required([
            "sim/joystick/yoke_roll_ratio",
            "sim/joystick/yoke_pitch_ratio",
            "sim/joystick/yoke_heading_ratio",
        ])?;
        let rust =
            OwnedDataRef::integer("sr20g6/test_controller/rust_implementation", 1, false, None)?;
        let flight_loop = PhaseFlightLoop::before_physics(Some(observe))?;
        let mut s = Self {
            controller,
            ints,
            floats,
            doubles,
            sample,
            overrides,
            axes,
            applied_ownership: [false; 3],
            _rust: rust,
            _loop: flight_loop,
        };
        s.refresh_match();
        s.publish();
        Ok(s)
    }
    fn refresh_match(&mut self) {
        let matched = current_aircraft_path().is_some_and(|path| {
            let path = path.to_string_lossy().replace('\\', "/").to_lowercase();
            path.ends_with("sr20_g6_custom_fm.acf") || path.ends_with("torquesim sr20/sr20.acf")
        });
        self.controller.ints[3] = i32::from(matched);
        if !matched {
            self.controller.ints[4] = 0;
        }
    }
    fn inputs(&mut self) {
        // Each SDK write is reflected immediately, including disarm and clamps.
        // Storage callbacks never borrow this state for readback.
        for (i, r) in self.floats.iter().enumerate() {
            if float_writable(i) {
                self.controller.write_float(i, r.get_f32());
            }
        }
        self.release_axes();
        self.publish();
    }
    fn integer_input(&mut self, slot: usize) {
        self.controller.write_int(slot, self.ints[slot].get_i32());
        self.release_axes();
        self.publish();
    }
    fn release_axes(&mut self) {
        for i in 0..3 {
            if self.applied_ownership[i] && !self.controller.owns[i] {
                self.axes[i].set_f32(0.0);
                self.overrides[i].set_i32(0);
                self.applied_ownership[i] = false;
            }
        }
    }
    fn release(&mut self, reason: Release) {
        self.controller.release(reason);
        self.release_axes();
        self.publish();
    }
    fn publish(&self) {
        for (r, v) in self.ints.iter().zip(self.controller.ints) {
            r.set_i32(v);
        }
        for (r, v) in self.floats.iter().zip(self.controller.floats) {
            r.set_f32(v);
        }
        for (r, v) in self.doubles.iter().zip(self.controller.doubles) {
            r.set_f64(v);
        }
    }
    fn observe(&mut self) {
        let r = &self.sample;
        let mut ground = [0; 10];
        r[12].read_i32(&mut ground);
        let sample = Sample {
            bank: r[0].get_f32(),
            pitch: r[1].get_f32(),
            heading: r[2].get_f32(),
            p_rad: r[3].get_f32(),
            q_rad: r[4].get_f32(),
            r_rad: r[5].get_f32(),
            beta: r[6].get_f32(),
            ias: r[7].get_f32(),
            tas: r[8].get_f32(),
            dt: r[9].get_f32(),
            paused: r[10].get_i32() != 0,
            replay: r[11].get_i32() != 0,
            ground_mask: i32::from(ground[0] != 0)
                | (i32::from(ground[1] != 0) * 2)
                | (i32::from(ground[2] != 0) * 4),
            vvi: r[13].get_f32(),
            latitude: r[14].get_f64(),
            longitude: r[15].get_f64(),
            elevation: r[16].get_f64(),
            throttle: r[17].get_f32(),
            overrides: self.overrides.map(|r| r.get_i32() != 0),
        };
        self.controller.step(sample);
        self.release_axes();
        for i in 0..3 {
            if self.controller.owns[i] && !self.applied_ownership[i] {
                self.overrides[i].set_i32(1);
                if self.overrides[i].get_i32() == 0 {
                    self.release(Release::Conflict);
                    return;
                }
                self.applied_ownership[i] = true;
            }
        }
        if self.controller.ints[5] != 0 {
            self.axes[0].set_f32(self.controller.floats[12]);
            if self.controller.ints[7] == 2 || self.controller.ints[7] == 3 {
                self.axes[1].set_f32(self.controller.floats[13]);
                self.axes[2].set_f32(self.controller.floats[14]);
            }
        }
        self.publish();
    }
}
extern "C" fn observe(_: f32, _: f32, _: i32, _: *mut c_void) -> f32 {
    with_state(Runtime::observe);
    -1.0
}
pub fn start() -> bool {
    match Runtime::new() {
        Ok(s) => {
            STATE.with(|state| state.replace(Some(s)));
            LOG.log("v1.9 behavior; Rust implementation loaded inert");
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
    with_state(|s| {
        s.controller.ints[2] = 1;
        s.refresh_match();
        s.publish();
    });
    true
}
pub fn disable() {
    with_state(|s| {
        s.controller.ints[2] = 0;
        s.controller.ints[4] = 0;
        s.release(Release::Disabled);
    });
}
pub fn receive_message(_: XPLMPluginID, message: i32, aircraft: *mut c_void) {
    if message == XPLM_MSG_PLANE_LOADED as i32 && aircraft.is_null() {
        with_state(|s| {
            s.controller.ints[4] = 0;
            s.controller.ints[9] = 0;
            s.controller.ints[10] = 0;
            s.controller.reset_contact();
            s.release(Release::Disarmed);
            s.refresh_match();
            s.publish();
        });
    }
}
