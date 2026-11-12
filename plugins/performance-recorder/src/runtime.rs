use std::{
    ffi::c_void,
    fs::{self, OpenOptions},
    io::{BufWriter, Write},
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};
use windows_sys::Win32::{
    System::{
        Performance::{QueryPerformanceCounter, QueryPerformanceFrequency},
        Threading::GetCurrentProcessId,
    },
    UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId},
};
use xplane_plugin::{
    Command, DataRefCache, DebugLogger, FrameBuffer, OwnedFloatArray, PhaseFlightLoop,
    PluginStateSlot, plugin_directory,
};
use xplane_sdk_sys::{
    XPLM_MSG_PLANE_LOADED, XPLMCommandPhase, XPLMCommandRef, XPLMPluginID, xplm_CommandBegin,
};

const LOG: DebugLogger = DebugLogger::new("[XPT Perf]");
const MAX_FRAMES: usize = 180_000;
const HEAD: &str = "sim/graphics/view/pilots_head_psi";
const PITCH: &str = "sim/graphics/view/pilots_head_the";
thread_local! {static STATE:PluginStateSlot<Runtime>=const{PluginStateSlot::new()};}
fn with_state(f: impl FnOnce(&mut Runtime)) {
    STATE.with(|s| {
        s.with_mut(f);
    });
}
fn qpc() -> i64 {
    let mut v = 0;
    unsafe { QueryPerformanceCounter(&mut v) };
    v
}
fn frequency() -> i64 {
    let mut v = 0;
    unsafe { QueryPerformanceFrequency(&mut v) };
    v
}
fn foreground() -> bool {
    let mut pid = 0;
    unsafe {
        GetWindowThreadProcessId(GetForegroundWindow(), &mut pid);
        pid == GetCurrentProcessId()
    }
}
#[derive(Clone, Copy, Debug)]
struct Config {
    token: u32,
    warmup: f64,
    seconds: f64,
    record: bool,
    sweep: bool,
    amplitude: f64,
    period: f64,
    expected_vr: i32,
    require_foreground: bool,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            token: 0,
            warmup: 0.,
            seconds: 300.,
            record: true,
            sweep: false,
            amplitude: 60.,
            period: 20.,
            expected_vr: -1,
            require_foreground: false,
        }
    }
}
impl Config {
    fn parse(s: &str) -> Result<Self, String> {
        let mut c = Self::default();
        let mut seen = std::collections::HashSet::new();
        for line in s.lines().filter(|l| !l.trim().is_empty()) {
            let (k, v) = line.split_once('=').ok_or("invalid config line")?;
            if !seen.insert(k) {
                return Err(format!("duplicate {k}"));
            }
            let n: f64 = v.parse().map_err(|_| format!("invalid {k}"))?;
            if !n.is_finite() {
                return Err("nonfinite value".into());
            }
            match k {
                "token" if n.fract() == 0. && (1. ..=1_000_000.).contains(&n) => c.token = n as u32,
                "warmup_seconds" if (0. ..=120.).contains(&n) => c.warmup = n,
                "duration_seconds" if (5. ..=300.).contains(&n) => c.seconds = n,
                "record" if n == 0. || n == 1. => c.record = n == 1.,
                "sweep" if n == 0. || n == 1. => c.sweep = n == 1.,
                "amplitude_degrees" if (0. ..=90.).contains(&n) => c.amplitude = n,
                "period_seconds" if (5. ..=120.).contains(&n) => c.period = n,
                "expected_vr" if [-1., 0., 1.].contains(&n) => c.expected_vr = n as i32,
                "require_foreground" if n == 0. || n == 1. => c.require_foreground = n == 1.,
                _ => return Err(format!("unknown or out-of-range {k}")),
            }
        }
        if c.token == 0 {
            return Err("token required".into());
        }
        if c.sweep && c.expected_vr != 0 {
            return Err("camera sweep requires explicit 2D mode".into());
        }
        Ok(c)
    }
}
#[derive(Clone, Copy)]
struct Frame {
    qpc: i64,
    dt: f64,
    sim: f64,
    gpu: f64,
    render: f64,
    head: f64,
    pitch: f64,
    lat: f64,
    lon: f64,
    elevation: f64,
    speed: f64,
    paused: i32,
    vr: i32,
    foreground: i32,
    marks: u32,
    cost_ticks: i64,
}
#[derive(Clone, Copy)]
struct Mark {
    qpc: i64,
    sequence: u32,
}
#[derive(Default)]
struct HitchLatch {
    unpaused_start: Option<i64>,
    sequence: u32,
    values: [f32; 7],
}
impl HitchLatch {
    fn observe(&mut self, now: i64, freq: i64, paused: bool, frame: u32, dt: f64) {
        if paused {
            self.unpaused_start = None;
            return;
        }
        let first = *self.unpaused_start.get_or_insert(now);
        if freq <= 0 || now - first < 5 * freq || dt <= 0.050 {
            return;
        }
        self.sequence += 1;
        let ticks = now as u64;
        // Four 16-bit pieces preserve exact QPC ticks through float datarefs.
        self.values = [
            self.sequence as f32,
            frame as f32,
            (dt * 1000.) as f32,
            (ticks & 0xffff) as f32,
            ((ticks >> 16) & 0xffff) as f32,
            ((ticks >> 32) & 0xffff) as f32,
            ((ticks >> 48) & 0xffff) as f32,
        ];
    }
}
struct Runtime {
    dir: PathBuf,
    refs: DataRefCache,
    config: Config,
    state: i32,
    freq: i64,
    start: i64,
    measure_start: i64,
    end: i64,
    previous: i64,
    epoch_unix_ns: u128,
    epoch_qpc: i64,
    frames: FrameBuffer<Frame>,
    marks: FrameBuffer<Mark>,
    count: u32,
    bad: u32,
    overflow: bool,
    original_head: f64,
    original_pitch: f64,
    original_position: [f64; 3],
    max_cost: i64,
    configured: bool,
    heartbeat: u32,
    snapshot: OwnedFloatArray<16>,
    hitch_snapshot: OwnedFloatArray<8>,
    hitch: HitchLatch,
    _commands: Vec<Command>,
    _loop: PhaseFlightLoop,
}
impl Runtime {
    fn get(&self, n: &'static str) -> f64 {
        self.refs
            .find(n)
            .and_then(|r| r.scalar())
            .unwrap_or(f64::NAN)
    }
    fn set(&self, n: &'static str, v: f64) {
        if let Some(r) = self.refs.find(n) {
            r.set_scalar(v);
        }
    }
    fn new() -> Result<Self, String> {
        let commands = [
            ("xpt/perf/configure", "Configure the performance recorder"),
            ("xpt/perf/start", "Start performance recording"),
            ("xpt/perf/stop", "Stop and export performance recording"),
            ("xpt/perf/mark", "Mark the stutter I just felt"),
        ]
        .iter()
        .enumerate()
        .map(|(i, (n, d))| Command::create(n, d, Some(command), true, i))
        .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            dir: plugin_directory().ok_or("plugin directory unavailable")?,
            refs: DataRefCache::default(),
            config: Config::default(),
            state: 0,
            freq: frequency(),
            start: 0,
            measure_start: 0,
            end: 0,
            previous: 0,
            epoch_unix_ns: 0,
            epoch_qpc: 0,
            frames: FrameBuffer::new(0),
            marks: FrameBuffer::new(0),
            count: 0,
            bad: 0,
            overflow: false,
            original_head: 0.,
            original_pitch: 0.,
            original_position: [0.; 3],
            max_cost: 0,
            configured: false,
            heartbeat: 0,
            snapshot: OwnedFloatArray::new("xpt/perf/snapshot")?,
            hitch_snapshot: OwnedFloatArray::new("xpt/perf/hitch")?,
            hitch: HitchLatch::default(),
            _commands: commands,
            _loop: PhaseFlightLoop::after_physics(Some(observe))?,
        })
    }
    fn configure(&mut self) {
        if self.state == 1 || self.state == 2 {
            return;
        }
        self.configured = false;
        match fs::read_to_string(self.dir.join("performance.ini"))
            .map_err(|e| e.to_string())
            .and_then(|s| Config::parse(&s))
        {
            Ok(c) => {
                self.config = c;
                self.configured = true;
                self.state = 0;
            }
            Err(e) => {
                self.state = 4;
                LOG.log(&e);
            }
        }
    }
    fn begin(&mut self) {
        if self.state == 1 || self.state == 2 {
            return;
        }
        if self.state == 4 && !self.configured {
            LOG.log("correct the failed configuration before starting");
            return;
        }
        if !self.configured {
            self.config = Config {
                token: (SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs()
                    % 1_000_000) as u32
                    + 1,
                ..Config::default()
            };
        }
        let c = self.config;
        for n in [
            HEAD,
            PITCH,
            "sim/time/total_running_time_sec",
            "sim/time/gpu_time_per_frame_sec_approx",
            "sim/time/framerate_period",
            "sim/flightmodel/position/latitude",
            "sim/flightmodel/position/longitude",
            "sim/flightmodel/position/elevation",
            "sim/flightmodel/position/local_x",
            "sim/flightmodel/position/local_y",
            "sim/flightmodel/position/local_z",
        ] {
            self.get(n);
        }
        let vr = self.get("sim/graphics/VR/enabled") as i32;
        let valid = self.freq > 0
            && (!c.require_foreground || foreground())
            && (c.expected_vr < 0 || c.expected_vr == vr)
            && (!c.sweep
                || (self.get("sim/time/paused") == 1.
                    && self.get("sim/flightmodel/position/groundspeed") < 0.5
                    && vr == 0))
            && [
                HEAD,
                PITCH,
                "sim/time/total_running_time_sec",
                "sim/graphics/VR/enabled",
                "sim/time/paused",
                "sim/flightmodel/position/groundspeed",
            ]
            .iter()
            .all(|n| self.get(n).is_finite());
        if !valid {
            self.state = 4;
            LOG.log("start gate failed (focus, mode, parked state or required dataref)");
            return;
        }
        if self
            .dir
            .join(format!("performance-{}.json", c.token))
            .exists()
        {
            self.state = 4;
            LOG.log("token output exists");
            return;
        }
        self.frames = FrameBuffer::new(if c.record { MAX_FRAMES } else { 0 });
        self.marks = FrameBuffer::new(1024);
        self.original_head = self.get(HEAD);
        self.original_pitch = self.get(PITCH);
        self.original_position = [
            self.get("sim/flightmodel/position/local_x"),
            self.get("sim/flightmodel/position/local_y"),
            self.get("sim/flightmodel/position/local_z"),
        ];
        self.count = 0;
        self.bad = 0;
        self.overflow = false;
        self.previous = 0;
        self.measure_start = 0;
        self.end = 0;
        self.max_cost = 0;
        self.hitch = HitchLatch::default();
        self.epoch_qpc = qpc();
        self.epoch_unix_ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        self.start = qpc();
        self.state = 1;
    }
    fn mark(&mut self) {
        if self.state == 1 || self.state == 2 {
            let mark = Mark {
                qpc: qpc(),
                sequence: self.marks.len() as u32 + 1,
            };
            if self.marks.push(mark).is_err() {
                self.overflow = true;
                self.bad |= 16;
            }
        }
    }
    fn finish(&mut self, aborted: bool) {
        if self.state != 1 && self.state != 2 {
            return;
        }
        self.end = qpc();
        self.state = if aborted { 4 } else { 3 };
        if self.config.sweep {
            self.set(HEAD, self.original_head);
            self.set(PITCH, self.original_pitch);
        }
        // All formatting and disk I/O are outside the measured window.
        if let Err(e) = self.export() {
            self.state = 4;
            LOG.log(&format!("export failed: {e}"));
        }
    }
    fn export(&self) -> std::io::Result<()> {
        let stem = self.dir.join(format!("performance-{}", self.config.token));
        let mut f = BufWriter::new(
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(stem.with_extension("csv"))?,
        );
        writeln!(
            f,
            "qpc,wall_frame_dt_s,sim_time,gpu_time_s_approx,render_period_s,head_psi,head_the,latitude,longitude,elevation_m,groundspeed_mps,paused,vr,foreground,mark_sequence,sample_collection_ticks"
        )?;
        for r in self.frames.frames() {
            writeln!(
                f,
                "{},{:.9},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
                r.qpc,
                r.dt,
                r.sim,
                r.gpu,
                r.render,
                r.head,
                r.pitch,
                r.lat,
                r.lon,
                r.elevation,
                r.speed,
                r.paused,
                r.vr,
                r.foreground,
                r.marks,
                r.cost_ticks
            )?;
        }
        f.flush()?;
        let markers = self
            .marks
            .frames()
            .iter()
            .map(|m| format!("{{\"qpc\":{},\"sequence\":{}}}", m.qpc, m.sequence))
            .collect::<Vec<_>>()
            .join(",");
        let c = self.config;
        fs::write(
            stem.with_extension("json"),
            format!(
                "{{\"schema_version\":1,\"token\":{},\"qpc_frequency\":{},\"epoch_qpc\":{},\"epoch_unix_ns\":{},\"start_qpc\":{},\"measure_start_qpc\":{},\"end_qpc\":{},\"frames_seen\":{},\"frames_stored\":{},\"record\":{},\"sweep\":{},\"invalid_flags\":{},\"overflow\":{},\"state\":{},\"sample_collection_max_us\":{},\"markers\":[{}]}}\n",
                c.token,
                self.freq,
                self.epoch_qpc,
                self.epoch_unix_ns,
                self.start,
                self.measure_start,
                self.end,
                self.count,
                self.frames.len(),
                c.record,
                c.sweep,
                self.bad,
                self.overflow,
                self.state,
                self.max_cost as f64 * 1e6 / self.freq as f64,
                markers
            ),
        )
    }
    fn observe(&mut self) {
        self.heartbeat = (self.heartbeat + 1) % 16_000_000;
        let now = qpc();
        let fg = foreground();
        let vr = self.get("sim/graphics/VR/enabled") as i32;
        let paused = self.get("sim/time/paused") as i32;
        if self.state == 1 || self.state == 2 {
            let elapsed = (now - self.start) as f64 / self.freq as f64;
            if elapsed >= self.config.warmup + self.config.seconds {
                self.finish(false);
            } else {
                if self.config.sweep {
                    let phase = std::f64::consts::TAU * elapsed / self.config.period;
                    self.set(
                        HEAD,
                        self.original_head + self.config.amplitude * phase.sin(),
                    );
                    self.set(PITCH, self.original_pitch);
                }
                if elapsed >= self.config.warmup {
                    if self.state == 1 {
                        self.state = 2;
                        self.measure_start = now;
                        self.previous = 0;
                    }
                    self.count += 1;
                    if self.config.require_foreground && !fg {
                        self.bad |= 1;
                    }
                    if self.config.expected_vr >= 0 && vr != self.config.expected_vr {
                        self.bad |= 2;
                    }
                    if self.config.sweep
                        && (paused != 1 || self.get("sim/flightmodel/position/groundspeed") >= 0.5)
                    {
                        self.bad |= 4;
                    }
                    if self.config.sweep
                        && [
                            "sim/flightmodel/position/local_x",
                            "sim/flightmodel/position/local_y",
                            "sim/flightmodel/position/local_z",
                        ]
                        .iter()
                        .enumerate()
                        .any(|(i, n)| (self.get(n) - self.original_position[i]).abs() > 0.1)
                    {
                        self.bad |= 8;
                    }
                    if self.config.record {
                        let frame = Frame {
                            qpc: now,
                            dt: if self.previous == 0 {
                                0.
                            } else {
                                (now - self.previous) as f64 / self.freq as f64
                            },
                            sim: self.get("sim/time/total_running_time_sec"),
                            gpu: self.get("sim/time/gpu_time_per_frame_sec_approx"),
                            render: self.get("sim/time/framerate_period"),
                            head: self.get(HEAD),
                            pitch: self.get(PITCH),
                            lat: self.get("sim/flightmodel/position/latitude"),
                            lon: self.get("sim/flightmodel/position/longitude"),
                            elevation: self.get("sim/flightmodel/position/elevation"),
                            speed: self.get("sim/flightmodel/position/groundspeed"),
                            paused,
                            vr,
                            foreground: i32::from(fg),
                            marks: self.marks.len() as u32,
                            cost_ticks: qpc() - now,
                        };
                        self.max_cost = self.max_cost.max(frame.cost_ticks);
                        self.hitch
                            .observe(now, self.freq, paused != 0, self.count, frame.dt);
                        if self.frames.push(frame).is_err() {
                            self.overflow = true;
                            self.bad |= 16;
                            self.finish(true);
                        }
                    }
                    self.previous = now;
                }
            }
        }
        self.snapshot.set([
            self.state as f32,
            self.count as f32,
            if self.start == 0 {
                0.
            } else {
                (now - self.start) as f32 / self.freq as f32
            },
            self.config.token as f32,
            self.bad as f32,
            self.marks.len() as f32,
            i32::from(fg) as f32,
            vr as f32,
            paused as f32,
            self.get("sim/flightmodel/position/groundspeed") as f32,
            self.frames.len() as f32,
            self.get(HEAD) as f32,
            1.,
            self.max_cost as f32 * 1e6 / self.freq as f32,
            self.get("sim/graphics/scenery/async_scenery_load_in_progress") as f32,
            self.heartbeat as f32,
        ]);
        let mut hitch = [0.; 8];
        hitch[0] = self.config.token as f32;
        hitch[1..].copy_from_slice(&self.hitch.values);
        self.hitch_snapshot.set(hitch);
    }
}
extern "C" fn command(_: XPLMCommandRef, phase: XPLMCommandPhase, refcon: *mut c_void) -> i32 {
    if phase == xplm_CommandBegin {
        with_state(|s| match Command::identifier_from_refcon(refcon) {
            0 => s.configure(),
            1 => s.begin(),
            2 => s.finish(false),
            _ => s.mark(),
        });
    }
    1
}
extern "C" fn observe(_: f32, _: f32, _: i32, _: *mut c_void) -> f32 {
    with_state(Runtime::observe);
    -1.
}
pub fn start() -> bool {
    match Runtime::new() {
        Ok(s) => {
            STATE.with(|v| v.replace(Some(s)));
            LOG.log("0.1.1 loaded; inert until started, QPC timing, bounded memory recording, latched hitch readback");
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
    STATE.with(|v| v.replace(None));
}
pub fn enable() -> bool {
    true
}
pub fn disable() {
    with_state(|s| s.finish(true));
}
pub fn receive_message(_: XPLMPluginID, message: i32, param: *mut c_void) {
    if message == XPLM_MSG_PLANE_LOADED as i32 && param.is_null() {
        with_state(|s| {
            s.finish(true);
            s.refs.clear();
            s.configured = false;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hitch_latch_excludes_pause_and_activation_and_preserves_qpc() {
        let mut h = HitchLatch::default();
        let f = 10_000_000;
        let base = 6_123_456_789_012;
        h.observe(base, f, false, 1, 0.2);
        h.observe(base + 4 * f, f, false, 2, 0.2);
        h.observe(base + 5 * f, f, false, 3, 0.050);
        assert_eq!(h.sequence, 0);
        let tick = base + 6 * f;
        h.observe(tick, f, false, 4, 0.051);
        let decoded = (h.values[3] as u64)
            | ((h.values[4] as u64) << 16)
            | ((h.values[5] as u64) << 32)
            | ((h.values[6] as u64) << 48);
        assert_eq!(decoded, tick as u64);
        assert_eq!(h.values[1], 4.);
        assert_eq!(h.sequence, 1);
        h.observe(tick + f, f, true, 5, 0.3);
        h.observe(tick + 2 * f, f, false, 6, 0.3);
        assert_eq!(h.sequence, 1);
    }
    #[test]
    fn rejects_unbounded_or_ambiguous_capture() {
        for s in [
            "token=1\nduration_seconds=NaN",
            "token=1\nrecord=2",
            "token=1\nsweep=1",
            "token=1\ntoken=2",
            "token=1\nunknown=1",
        ] {
            assert!(Config::parse(s).is_err(), "{s}");
        }
    }
    #[test]
    fn explicit_2d_sweep_is_valid() {
        let c = Config::parse("token=1\nsweep=1\nexpected_vr=0\nduration_seconds=30\nrecord=0")
            .unwrap();
        assert!(c.sweep);
        assert!(!c.record);
    }
    #[test]
    fn performance_clock_advances() {
        let f = frequency();
        let a = qpc();
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert!(f > 0);
        assert!(qpc() > a);
    }
}
