use std::{
    collections::HashMap,
    ffi::c_void,
    fs::File,
    io::Write,
    sync::{Mutex, MutexGuard, OnceLock},
};
use xphost_core::*;
use xplane_plugin::Bounds;
use xplane_sdk_sys::*;

#[derive(Clone, Copy, Default)]
pub struct Accessor {
    pub gi: XPLMGetDatai_f,
    pub si: XPLMSetDatai_f,
    pub gf: XPLMGetDataf_f,
    pub sf: XPLMSetDataf_f,
    pub gd: XPLMGetDatad_f,
    pub sd: XPLMSetDatad_f,
    pub gvi: XPLMGetDatavi_f,
    pub svi: XPLMSetDatavi_f,
    pub gvf: XPLMGetDatavf_f,
    pub svf: XPLMSetDatavf_f,
    pub gb: XPLMGetDatab_f,
    pub sb: XPLMSetDatab_f,
    pub read: usize,
    pub write: usize,
}
#[derive(Clone, Copy)]
pub struct Handler {
    pub callback: XPLMCommandCallback_f,
    pub before: bool,
    pub refcon: usize,
}
pub struct Command {
    pub name: String,
    pub description: String,
    pub handlers: Vec<Handler>,
    pub down: bool,
}
pub struct FlightLoop {
    pub callback: XPLMFlightLoop_f,
    pub refcon: usize,
    pub schedule: Schedule,
}
#[derive(Default)]
pub struct Window {
    pub bounds: Bounds,
    pub visible: bool,
    pub title: String,
}
#[derive(Default)]
pub struct Menu {
    pub items: Vec<String>,
    pub dead: bool,
}
#[derive(Default)]
pub struct State {
    pub store: Store,
    pub accessors: HashMap<usize, Accessor>,
    pub commands: Vec<Command>,
    pub loops: Vec<FlightLoop>,
    pub config: Option<Config>,
    pub frame: u64,
    pub now: f64,
    pub dt: f64,
    pub faulted: bool,
    pub enabled: bool,
    pub name: String,
    pub signature: String,
    pub description: String,
    pub message: Option<xphost_native::guard::Message>,
    pub log: Option<File>,
    pub command_log: Option<File>,
    pub sim_reads: u64,
    pub sim_writes: u64,
    pub plugin_reads: u64,
    pub plugin_writes: u64,
    pub commands_fired: u64,
    pub loops_run: u64,
    pub nav: Vec<Nav>,
    pub plans: [Plan; 6],
    pub windows: HashMap<usize, Window>,
    pub menus: HashMap<usize, Menu>,
    pub popups: HashMap<usize, bool>,
    pub next_handle: usize,
    pub next_texture: i32,
    pub draws: Vec<(usize, i32, i32, usize)>,
    pub reset_pending: bool,
    pub last_local: Option<[f64; 3]>,
}
pub fn state() -> MutexGuard<'static, State> {
    static STATE: OnceLock<Mutex<State>> = OnceLock::new();
    STATE
        .get_or_init(|| {
            Mutex::new(State {
                dt: 1. / 60.,
                next_handle: 100,
                next_texture: 1000,
                ..State::default()
            })
        })
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}
impl State {
    pub fn log_raw(&mut self, text: &str) {
        if let Some(f) = &mut self.log {
            let _ = f.write_all(text.as_bytes());
            let _ = f.flush();
        }
    }
    pub fn command(&mut self, name: &str, description: &str) -> usize {
        if let Some(i) = self.commands.iter().position(|c| c.name == name) {
            if self.commands[i].description.is_empty() {
                self.commands[i].description = description.into();
            }
            return i;
        }
        let i = self.commands.len();
        self.commands.push(Command {
            name: name.into(),
            description: description.into(),
            handlers: Vec::new(),
            down: false,
        });
        i
    }
    pub fn handle(&mut self) -> usize {
        self.next_handle += 1;
        self.next_handle
    }
    pub fn local_frame(&self) -> LocalFrame {
        self.config
            .as_ref()
            .map(|c| LocalFrame::new(c.lat, c.lon, c.elev))
            .unwrap_or_else(|| LocalFrame::new(0., 0., 0.))
    }
    pub fn magvar(&self) -> f32 {
        self.config
            .as_ref()
            .map(|c| c.magvar as f32)
            .unwrap_or_default()
    }
}
pub fn fault(code: u32) {
    if code != 0 {
        let mut s = state();
        s.faulted = true;
        s.log_raw(&format!("xphost: plugin fault (SEH 0x{code:08x})\n"));
    }
}
pub fn handle(id: usize) -> *mut c_void {
    (id + 1) as *mut c_void
}
pub fn index(ptr: *mut c_void) -> usize {
    (ptr as usize).wrapping_sub(1)
}

/// C string and buffer helpers are used only at documented SDK boundaries.
/// # Safety
/// Non-null `p` points to a live NUL-terminated SDK string for this call.
pub unsafe fn string(p: *const i8) -> String {
    if p.is_null() {
        String::new()
    } else {
        // SAFETY: supplied by the calling SDK function's contract.
        unsafe { std::ffi::CStr::from_ptr(p) }
            .to_string_lossy()
            .into_owned()
    }
}
/// # Safety
/// A non-null output points to at least `cap` writable SDK bytes.
pub unsafe fn put(p: *mut i8, text: &str, cap: usize) {
    if p.is_null() || cap == 0 {
        return;
    }
    let n = text.len().min(cap - 1);
    // SAFETY: output capacity is an SDK-defined buffer size; source is live.
    unsafe {
        std::ptr::copy_nonoverlapping(text.as_ptr(), p.cast(), n);
        *p.add(n) = 0;
    }
}
/// # Safety
/// A non-null output is aligned and valid for a single T.
pub unsafe fn output<T: Copy>(p: *mut T, value: T) {
    if !p.is_null() {
        // SAFETY: the calling SDK contract provides one writable T.
        unsafe {
            p.write(value);
        }
    }
}
