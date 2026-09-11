use crate::{
    graphics::Graphics,
    scene::Hud,
    values::{Values, NAMES},
};
use poweroff180::{
    protocol::{self, field},
    Config,
};
use std::{collections::HashMap, ffi::c_void, fmt::Write, fs, path::PathBuf};
use xplane_plugin::opengl::DrawContext;
use xplane_plugin::{
    current_aircraft_path, fms_destination, fms_entries, load_fms_plan, plugin_directory,
    screen_size, set_fms_destination, system_path, Command, DataRefCache, DebugLogger,
    DrawCallback, OwnedDataRef, PluginStateSlot,
};
use xplane_sdk_sys::{
    xplm_CommandBegin, xplm_Phase_Window, XPLMCommandPhase, XPLMCommandRef, XPLMDrawingPhase,
    XPLMPluginID, XPLM_MSG_PLANE_LOADED,
};
const LOG: DebugLogger = DebugLogger::new("[XPT Rust HUD]");
thread_local! {static STATE:PluginStateSlot<Runtime>=const {PluginStateSlot::new()};}
fn with_state<T>(f: impl FnOnce(&mut Runtime) -> T) -> Option<T> {
    STATE.with(|s| s.with_mut(f))
}
struct Runtime {
    folder: PathBuf,
    refs: DataRefCache,
    enabled: bool,
    token: i32,
    hud: Hud,
    graphics: Graphics,
    published: HashMap<&'static str, OwnedDataRef>,
    _commands: Vec<Command>,
    _draw: DrawCallback,
}
impl Runtime {
    fn new() -> Result<Self, String> {
        let folder = plugin_directory().ok_or("cannot resolve plugin folder")?;
        let mut published = HashMap::new();
        for (key, value) in [
            ("version", 5),
            ("draw_frames", 0),
            ("font_ready", 0),
            ("navigation_ready", 0),
            ("full_flap_limit_kias", 0),
            ("rust_implementation", 1),
        ] {
            published.insert(
                key,
                OwnedDataRef::integer(&format!("xpt/video_hud/{key}"), value, false, None)?,
            );
        }
        let commands = [
            (
                "xpt/video_hud/load_approach",
                "Load KCDW RNAV22 for HUD while paused",
            ),
            (
                "xpt/video_hud/activate_approach",
                "Activate KOLLI to RW22 for HUD while paused",
            ),
        ]
        .iter()
        .enumerate()
        .map(|(i, (name, desc))| Command::create(name, desc, Some(navigation_command), true, i))
        .collect::<Result<_, _>>()?;
        let draw = DrawCallback::register(Some(draw), xplm_Phase_Window, false, 0)?;
        Ok(Self {
            folder,
            refs: DataRefCache::default(),
            enabled: true,
            token: -1,
            hud: Hud::default(),
            graphics: Graphics::default(),
            published,
            _commands: commands,
            _draw: draw,
        })
    }
    fn value(&self, name: &'static str) -> f64 {
        self.refs.find(name).map_or(f64::NAN, |r| {
            r.scalar().unwrap_or_else(|| r.array_element(0))
        })
    }
    fn set(&self, key: &str, value: i32) {
        if let Some(r) = self.published.get(key) {
            r.set_i32(value);
        }
    }
    fn load_airframe_marks(&mut self) {
        let path = current_aircraft_path();
        self.hud.full_flap_limit = f64::NAN;
        if let Some(path) = &path {
            if let Ok(text) = fs::read_to_string(path) {
                self.hud.full_flap_limit = text
                    .lines()
                    .find_map(|line| {
                        line.strip_prefix("P acf/_Vfem_kts ")
                            .and_then(|v| v.trim().parse().ok())
                    })
                    .unwrap_or(f64::NAN);
            }
        }
        self.set(
            "full_flap_limit_kias",
            if self.hud.full_flap_limit.is_finite() {
                self.hud.full_flap_limit.round() as i32
            } else {
                0
            },
        );
        let readback = format!(
            "aircraft={}\nfull_flap_limit_kias={}\n",
            path.as_ref()
                .map_or_else(String::new, |p| p.display().to_string()),
            self.hud.full_flap_limit
        );
        if let Err(e) = fs::write(self.folder.join("instrument-readback.txt"), readback) {
            LOG.log(&format!("instrument readback failed: {e}"));
        }
    }
    fn draw(&mut self, gl: &mut DrawContext) {
        if !self.enabled {
            return;
        }
        let mut snapshot = [0.0; protocol::LENGTH];
        if self
            .refs
            .find("xpt/snapshot")
            .map_or(0, |r| r.read_f32(&mut snapshot))
            != protocol::LENGTH
            || snapshot[field::CONFIGURED] < 0.5
        {
            return;
        }
        if snapshot[field::RUN_TOKEN] as i32 != self.token
            || f64::from(snapshot[field::SIM_TIME]) < self.hud.last_time - 1.0
        {
            self.token = snapshot[field::RUN_TOKEN] as i32;
            let path = self
                .folder
                .parent()
                .unwrap_or(&self.folder)
                .join("XPTNativeGuidance/effective-card.ini");
            let config = fs::read_to_string(path)
                .map_err(|e| e.to_string())
                .and_then(|text| Config::parse(&text));
            match config {
                Ok(c) => self.hud.new_flight(c),
                Err(e) => {
                    LOG.log(&format!("HUD configuration failed: {e}"));
                    return;
                }
            }
            self.load_airframe_marks();
        }
        let values = Values(NAMES.iter().map(|name| (*name, self.value(name))).collect());
        let scene = self.hud.frame(&snapshot, &values);
        if self.graphics.paint(gl, &scene, screen_size()) {
            self.set("font_ready", 1);
            let counter = &self.published["draw_frames"];
            counter.set_i32(counter.get_i32().saturating_add(1));
        }
    }
    fn navigation(&mut self, id: usize) {
        if self.value("sim/time/paused") != 1.0 {
            return;
        }
        let mut snapshot = [0.0; protocol::LENGTH];
        if let Some(r) = self.refs.find("xpt/snapshot") {
            r.read_f32(&mut snapshot);
        }
        if snapshot[field::NATIVE_RUNNING] > 0.5 {
            return;
        }
        if id == 0 {
            self.hud.nav_ready = false;
            self.set("navigation_ready", 0);
            let cycle = fs::read_to_string(system_path().join("Custom Data/cycle_info.txt"))
                .ok()
                .and_then(|text| {
                    text.lines()
                        .find(|line| line.contains("AIRAC cycle"))
                        .and_then(|line| line.split_once(':'))
                        .and_then(|(_, value)| value.split_whitespace().next())
                        .map(str::to_owned)
                });
            let Some(cycle) =
                cycle.filter(|v| v.len() == 4 && v.chars().all(|c| c.is_ascii_digit()))
            else {
                LOG.log("AIRAC cycle unavailable");
                return;
            };
            let plan=format!("I\n1100 Version\nCYCLE {cycle}\nADEP KCDW\nADES KCDW\nDESRWY RW22\nAPP R22\nNUMENR 2\n1 KCDW ADEP 0.000000 40.875222 -74.281361\n1 KCDW ADES 0.000000 40.875222 -74.281361\n");
            if !load_fms_plan(0, &plan) {
                LOG.log("FMS approach load rejected");
            }
        } else {
            let entries = fms_entries(2);
            for (i, pair) in entries.windows(2).enumerate() {
                if pair[0].identifier == "KOLLI" && pair[1].identifier == "RW22" {
                    let a = &pair[0];
                    let b = &pair[1];
                    self.hud.leg = [
                        f64::from(a.latitude),
                        f64::from(a.longitude),
                        f64::from(b.latitude),
                        f64::from(b.longitude),
                    ];
                    if a.latitude != b.latitude || a.longitude != b.longitude {
                        self.hud.nav_ready = set_fms_destination(2, i as i32 + 1);
                    }
                    break;
                }
            }
            self.set("navigation_ready", i32::from(self.hud.nav_ready));
        }
        let mut out = String::new();
        for plan in 0..4 {
            let entries = fms_entries(plan);
            writeln!(
                &mut out,
                "PLAN {plan} COUNT {} ACTIVE {}",
                entries.len(),
                fms_destination(plan)
            )
            .unwrap();
            for (i, entry) in entries.iter().enumerate() {
                writeln!(
                    &mut out,
                    "{i} {} {:.9} {:.9}",
                    entry.identifier, entry.latitude, entry.longitude
                )
                .unwrap();
            }
        }
        if let Err(e) = fs::write(self.folder.join("navigation-readback.txt"), out) {
            LOG.log(&format!("navigation readback failed: {e}"));
        }
    }
}
extern "C" fn draw(_: XPLMDrawingPhase, _: i32, _: *mut c_void) -> i32 {
    // SAFETY: X-Plane invokes this callback with its compatibility context current.
    // The synchronous draw scope keeps every primitive and stack balanced.
    unsafe { DrawContext::with_current(|gl| with_state(|s| s.draw(gl))) };
    1
}
extern "C" fn navigation_command(
    _: XPLMCommandRef,
    phase: XPLMCommandPhase,
    refcon: *mut c_void,
) -> i32 {
    if phase == xplm_CommandBegin {
        with_state(|s| s.navigation(Command::identifier_from_refcon(refcon)));
    }
    1
}
pub fn start() -> bool {
    match Runtime::new() {
        Ok(state) => {
            STATE.with(|s| s.replace(Some(state)));
            LOG.log("native HUD v5 Rust implementation loaded");
            true
        }
        Err(e) => {
            LOG.log(&e);
            false
        }
    }
}
pub fn stop() {
    STATE.with(|s| s.replace(None));
}
pub fn enable() -> bool {
    with_state(|s| s.enabled = true);
    true
}
pub fn disable() {
    with_state(|s| s.enabled = false);
}
pub fn receive_message(_: XPLMPluginID, message: i32, aircraft: *mut c_void) {
    with_state(|s| {
        s.refs.clear();
        if message == XPLM_MSG_PLANE_LOADED as i32 && aircraft.is_null() {
            s.token = -1;
            s.hud.nav_ready = false;
            s.set("navigation_ready", 0);
        }
    });
}
