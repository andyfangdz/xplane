use crate::{
    config::{Optics, Runway},
    graphics,
    guidance::{chute_area_ratio, GuidanceInput, LandingGuidance, LandingPath},
    math::{deg, eas, matches, rad, wrap, Point, View, FT},
    presentation::{digital_height, HudInput, HudPresentation},
    runway::{valid_rotation, CameraProjection, RunwayRays, RunwaySurface},
    scene::{self, Frame},
};
use std::{
    cell::RefCell,
    collections::HashMap,
    ffi::{c_int, c_void},
    fs,
    path::{Path, PathBuf},
};
use xplane_plugin::{
    command_once, current_aircraft_path, plugin_directory, screen_size, world_to_local, Command,
    DataRef, DebugLogger, DrawCallback, FlightLoop, OwnedDataRef, PluginMenu, PluginStateSlot,
    TerrainProbe,
};
use xplane_sdk_sys::{
    xplm_CommandBegin, xplm_Phase_Gauges, xplm_Phase_Window, XPLMCommandPhase, XPLMCommandRef,
    XPLMDrawingPhase, XPLMPluginID, XPLM_MSG_PLANE_LOADED, XPLM_MSG_SCENERY_LOADED,
};

const VERSION: i32 = 144;
const LOG: DebugLogger = DebugLogger::new("[ShuttleHUD]");
thread_local! {static STATE:PluginStateSlot<Runtime>=const {PluginStateSlot::new()};}
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
        let reference = refs.entry(name).or_insert(None);
        if reference.is_none() {
            *reference = DataRef::find(name);
        }
        *reference
    }
    fn val(&self, name: &'static str, fallback: f64) -> f64 {
        self.find(name)
            .and_then(DataRef::scalar)
            .filter(|x| x.is_finite())
            .unwrap_or(fallback)
    }
    fn arr(&self, name: &'static str, index: i32) -> f64 {
        self.find(name).map_or(0.0, |r| r.array_element(index))
    }
    fn set(&self, name: &'static str, value: f32) {
        if let Some(r) = self.find(name).filter(|r| r.writable()) {
            r.set_f32(value);
        }
    }
    fn ready(&self) -> bool {
        [
            "sim/flightmodel/position/latitude",
            "sim/flightmodel/position/longitude",
            "sim/flightmodel/position/elevation",
            "sim/flightmodel/position/local_vx",
            "sim/flightmodel/position/local_vy",
            "sim/flightmodel/position/local_vz",
            "sim/time/total_flight_time_sec",
            "sim/time/paused",
            "sim/time/is_in_replay",
            "sim/flightmodel2/gear/on_ground",
            "sim/aircraft/specialcontrols/acf_chute_area",
            "sim/cockpit/switches/parachute_on",
        ]
        .iter()
        .all(|name| self.find(name).is_some())
    }
}
struct Runtime {
    native: Native,
    published: HashMap<&'static str, OwnedDataRef>,
    expected: String,
    optics: Optics,
    runways: Vec<Runway>,
    runway_index: usize,
    runway_surface: RunwaySurface,
    matched: bool,
    plugin_enabled: bool,
    display: HudPresentation,
    guidance: LandingGuidance,
    radar_valid: bool,
    radar_ground: f64,
    next_radar: f64,
    chute_owned: bool,
    chute_base: f64,
    chute_start: f64,
    old_view: Option<(f32, f32)>,
    draw_callbacks: Vec<DrawCallback>,
    flight_loop: Option<FlightLoop>,
    commands: Vec<Command>,
    menu: Option<PluginMenu>,
    probe: Option<TerrainProbe>,
}
fn normalized(path: &Path) -> String {
    path.components()
        .collect::<PathBuf>()
        .to_string_lossy()
        .replace('\\', "/")
}
impl Runtime {
    fn new(folder: PathBuf) -> Result<Self, String> {
        let optics = Optics::load(&folder)?;
        let runways = Runway::parse_all(
            &fs::read_to_string(folder.join("runways.csv")).map_err(|e| e.to_string())?,
        );
        if runways.is_empty() {
            return Err("no valid Shuttle runway definitions".to_owned());
        }
        let aircraft = folder
            .parent()
            .and_then(Path::parent)
            .ok_or("invalid aircraft-local plugin path")?;
        let mut state = Self {
            native: Native::default(),
            published: HashMap::new(),
            expected: normalized(&aircraft.join("Orbiter_Glider.acf")),
            optics,
            runways,
            runway_index: 0,
            runway_surface: RunwaySurface::default(),
            matched: false,
            plugin_enabled: false,
            display: HudPresentation::default(),
            guidance: LandingGuidance::default(),
            radar_valid: false,
            radar_ground: 0.0,
            next_radar: 0.0,
            chute_owned: false,
            chute_base: 0.0,
            chute_start: -1.0,
            old_view: None,
            draw_callbacks: Vec::new(),
            flight_loop: None,
            commands: Vec::new(),
            menu: None,
            probe: None,
        };
        for (name, initial, writable) in [
            ("version", VERSION, false),
            ("enabled", 1, true),
            ("aircraft_match", 0, false),
            ("active", 0, false),
            ("draw_frames", 0, false),
            ("declutter_mode", -1, true),
            ("declutter_level", 0, false),
            ("runway_index", 0, false),
            ("velocity_limited", 0, false),
            ("guidance_limited", 0, false),
            ("main_wow", 0, false),
            ("nose_wow", 0, false),
            ("cockpit_active", 0, false),
            ("cockpit_draw_frames", 0, false),
            ("renderer", 2, false),
            ("landing_systems_enabled", 1, true),
            ("chute_reefing_active", 0, false),
            ("plugin_enabled", 0, false),
            ("guidance_phase", 1, false),
            ("att_ref_caged", 0, true),
            ("display_phase", 0, false),
            ("display_main_wow", 0, false),
            ("display_nose_wow", 0, false),
            ("gear_cue", 0, false),
            ("display_flags", 0, false),
            ("control_auto", 0, false),
            ("rust_implementation", 1, false),
            ("runway_projection_valid", 0, false),
        ] {
            state.published.insert(
                name,
                OwnedDataRef::integer(
                    &format!("fsim_hud/{name}"),
                    initial,
                    writable,
                    if writable {
                        Some(controls_changed)
                    } else {
                        None
                    },
                )?,
            );
        }
        for (name, initial, writable) in [
            ("main_wheel_height_ft", 0.0, false),
            ("radar_height_ft", 0.0, false),
            ("equivalent_airspeed_kt", 0.0, false),
            ("gamma_command_deg", 0.0, false),
            ("speedbrake_command_ratio", 0.0, false),
            ("brightness", 1.0, true),
            ("chute_area_ratio", 1.0, false),
            ("runway_ground_elevation_m", 0.0, false),
            ("vertical_velocity_mps", 0.0, false),
            ("groundspeed_mps", 0.0, false),
            ("ground_track_deg", 0.0, false),
            ("height_command_m", 0.0, false),
            ("gear_command", 0.0, false),
            ("along_m", 0.0, false),
            ("cross_m", 0.0, false),
            ("velocity_vector_blend", 0.0, false),
            ("display_altitude_ft", 0.0, false),
            ("deceleration_g", 0.0, false),
            ("deceleration_command_g", 0.0, false),
        ] {
            state.published.insert(
                name,
                OwnedDataRef::float(
                    &format!("fsim_hud/{name}"),
                    initial,
                    writable,
                    if writable {
                        Some(controls_changed)
                    } else {
                        None
                    },
                )?,
            );
        }
        state.probe = Some(TerrainProbe::new()?);
        state.check_match();
        state.choose_runway();
        state.terrain();
        for (id, (name, description)) in COMMANDS.iter().enumerate() {
            state.commands.push(if id == 4 {
                Command::intercept(name, Some(command_callback), true, id)?
            } else {
                Command::create(name, description, Some(command_callback), true, id)?
            });
        }
        let menu = PluginMenu::new("Shuttle HUD", None)?;
        for id in [0, 1, 2, 3, 5, 6, 7] {
            menu.append_command(COMMANDS[id].1, &state.commands[id])?;
        }
        state.menu = Some(menu);
        state.draw_callbacks.push(DrawCallback::register(
            Some(draw_callback),
            xplm_Phase_Window,
            false,
            0,
        )?);
        state.draw_callbacks.push(DrawCallback::register(
            Some(draw_callback),
            xplm_Phase_Gauges,
            false,
            0,
        )?);
        state.flight_loop = Some(FlightLoop::register(Some(flight_callback), -1.0)?);
        Ok(state)
    }
    fn i(&self, name: &str) -> i32 {
        self.published.get(name).map_or(0, OwnedDataRef::get_i32)
    }
    fn f(&self, name: &str) -> f64 {
        self.published
            .get(name)
            .map_or(0.0, |r| f64::from(r.get_f32()))
    }
    fn put_i(&self, name: &str, value: i32) {
        if let Some(r) = self.published.get(name) {
            r.set_i32(value);
        }
    }
    fn put_f(&self, name: &str, value: f64) {
        if let Some(r) = self.published.get(name) {
            r.set_f32(value as f32);
        }
    }
    fn check_match(&mut self) {
        self.matched =
            current_aircraft_path().is_some_and(|p| matches(&normalized(&p), &self.expected));
        self.put_i("aircraft_match", i32::from(self.matched));
    }
    fn controls_changed(&mut self) {
        self.put_i("enabled", i32::from(self.i("enabled") != 0));
        self.put_i("declutter_mode", self.i("declutter_mode").clamp(-1, 3));
        self.put_i("att_ref_caged", i32::from(self.i("att_ref_caged") != 0));
        self.put_f("brightness", self.f("brightness").clamp(0.05, 1.0));
        if self.i("enabled") == 0 {
            self.put_i("runway_projection_valid", 0);
            self.restore_view();
        }
    }
    fn restore_view(&mut self) {
        if let Some((shift, fov)) = self.old_view.take() {
            self.native
                .set("sim/graphics/view/field_of_view_vertical_ratio", shift);
            self.native.set("sim/graphics/view/field_of_view_deg", fov);
        }
    }
    fn cockpit_view(&mut self) {
        self.check_match();
        if !self.matched || !self.plugin_enabled || self.i("enabled") == 0 {
            return;
        }
        self.restore_view();
        self.choose_runway();
        self.terrain();
        command_once("sim/view/3d_cockpit_cmnd_look");
    }
    fn fullscreen_view(&mut self) {
        self.check_match();
        if !self.matched || !self.plugin_enabled || self.i("enabled") == 0 {
            return;
        }
        self.choose_runway();
        self.terrain();
        if self.old_view.is_none() {
            self.old_view = Some((
                self.native
                    .val("sim/graphics/view/field_of_view_vertical_ratio", 0.0)
                    as f32,
                self.native.val("sim/graphics/view/field_of_view_deg", 65.0) as f32,
            ));
        }
        command_once("sim/view/forward_with_nothing");
        self.native.set("sim/graphics/view/field_of_view_deg", 65.0);
        self.native
            .set("sim/graphics/view/field_of_view_vertical_ratio", -0.5);
    }
    fn kinematics(&self) {
        if !self.matched {
            return;
        }
        let lat = self.native.val("sim/flightmodel/position/latitude", 0.0);
        let lon = self.native.val("sim/flightmodel/position/longitude", 0.0);
        let alt = self.native.val("sim/flightmodel/position/elevation", 0.0);
        let p = world_to_local(lat, lon, alt);
        let up = world_to_local(lat, lon, alt + 100.0);
        let north = world_to_local(lat + 0.001, lon, alt);
        let mut u = [up.0 - p.0, up.1 - p.1, up.2 - p.2];
        let mut n = [north.0 - p.0, north.1 - p.1, north.2 - p.2];
        fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
            a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
        }
        fn norm(a: &mut [f64; 3]) -> bool {
            let d = dot(*a, *a).sqrt();
            if d < 1e-6 {
                return false;
            }
            for x in a {
                *x /= d;
            }
            true
        }
        if !norm(&mut u) {
            return;
        }
        let vertical = dot(n, u);
        for k in 0..3 {
            n[k] -= vertical * u[k];
        }
        if !norm(&mut n) {
            return;
        }
        let e = [
            n[1] * u[2] - n[2] * u[1],
            n[2] * u[0] - n[0] * u[2],
            n[0] * u[1] - n[1] * u[0],
        ];
        let velocity = [
            self.native.val("sim/flightmodel/position/local_vx", 0.0),
            self.native.val("sim/flightmodel/position/local_vy", 0.0),
            self.native.val("sim/flightmodel/position/local_vz", 0.0),
        ];
        let north = dot(velocity, n);
        let east = dot(velocity, e);
        self.put_f("vertical_velocity_mps", dot(velocity, u));
        self.put_f("groundspeed_mps", north.hypot(east));
        self.put_f("ground_track_deg", (deg(east.atan2(north)) + 360.0) % 360.0);
    }
    fn choose_runway(&mut self) {
        self.kinematics();
        let track = if self.f("groundspeed_mps") > 5.0 {
            self.f("ground_track_deg")
        } else {
            self.native.val("sim/flightmodel/position/psi", 0.0)
        };
        let mut best = f64::INFINITY;
        for (index, r) in self.runways.iter().enumerate() {
            let n = (self.native.val("sim/flightmodel/position/latitude", 0.0) - r.lat) * 111120.0;
            let e = (self.native.val("sim/flightmodel/position/longitude", 0.0) - r.lon)
                * 111120.0
                * rad(r.lat).cos();
            let score = n.hypot(e) + 100.0 * wrap(track - r.heading).abs();
            if score < best {
                best = score;
                self.runway_index = index;
            }
        }
        self.put_i("runway_index", self.runway_index as i32);
    }
    fn terrain(&mut self) {
        let Some(probe) = self.probe.as_ref() else {
            return;
        };
        let r = &mut self.runways[self.runway_index];
        let distance = r.displaced + 2500.0 / FT;
        if let Some(alt) = probe.elevation(
            r.lat + distance * r.un / 111120.0,
            r.lon + distance * r.ue / (111120.0 * rad(r.lat).cos()),
            r.elev + 2000.0,
        ) {
            r.elev = alt;
            LOG.log(&format!("{} touchdown-zone datum {alt:.3} m MSL", r.name));
            self.put_f("runway_ground_elevation_m", alt);
        }
        self.refresh_runway_surface();
    }
    fn refresh_runway_surface(&mut self) {
        let Some(probe) = self.probe.as_ref() else {
            return;
        };
        let r = &self.runways[self.runway_index];
        self.runway_surface =
            RunwaySurface::sample(r, |lat, lon| probe.elevation(lat, lon, r.elev + 2000.0))
                .unwrap_or_default();
    }
    fn radar(&mut self) {
        self.radar_valid = false;
        if !self.matched {
            return;
        }
        if let Some(probe) = self.probe.as_ref() {
            if let Some(alt) = probe.elevation(
                self.native.val("sim/flightmodel/position/latitude", 0.0),
                self.native.val("sim/flightmodel/position/longitude", 0.0),
                self.native.val("sim/flightmodel/position/elevation", 0.0) + 1000.0,
            ) {
                self.radar_ground = alt;
                self.radar_valid = true;
            }
        }
    }
    fn wheel_offset(&self) -> f64 {
        let y = self.native.arr("sim/aircraft/parts/acf_gear_ynodef", 1)
            - self.native.arr("sim/aircraft/parts/acf_gear_leglen", 1)
            - self.native.arr("sim/flightmodel2/gear/tire_radius_mtrs", 1);
        let z = self.native.arr("sim/aircraft/parts/acf_gear_znodef", 1);
        let pitch = rad(self.native.val("sim/flightmodel/position/theta", 0.0));
        let roll = rad(self.native.val("sim/flightmodel/position/phi", 0.0));
        y * pitch.cos() * roll.cos() - z * pitch.sin()
    }
    fn guidance(&mut self) {
        if !self.matched {
            return;
        }
        let r = &self.runways[self.runway_index];
        let (along, cross) = r.offsets(
            self.native.val("sim/flightmodel/position/latitude", 0.0),
            self.native.val("sim/flightmodel/position/longitude", 0.0),
        );
        self.put_f("along_m", along);
        self.put_f("cross_m", cross);
        let main = self.native.arr("sim/flightmodel2/gear/on_ground", 1) != 0.0
            || self.native.arr("sim/flightmodel2/gear/on_ground", 2) != 0.0;
        let height = if main {
            0.0
        } else {
            (self.native.val("sim/flightmodel/position/elevation", 0.0) - r.elev
                + self.wheel_offset())
            .max(0.0)
        };
        let i = GuidanceInput {
            time: self.native.val("sim/time/total_flight_time_sec", 0.0),
            along: self.f("along_m"),
            height,
            groundspeed: self.f("groundspeed_mps"),
            vy: self.f("vertical_velocity_mps"),
            eas: eas(
                self.native
                    .val("sim/flightmodel/position/true_airspeed", 0.0),
                self.native.val("sim/weather/rho", 1.225),
            ),
            mass_lb: self.native.val("sim/flightmodel/weight/m_total", 0.0) * 2.204622622,
            main_wow: main,
        };
        self.put_i("main_wow", i32::from(main));
        self.put_i(
            "nose_wow",
            i32::from(self.native.arr("sim/flightmodel2/gear/on_ground", 0) != 0.0),
        );
        self.put_f("main_wheel_height_ft", height * FT);
        self.put_f("equivalent_airspeed_kt", i.eas);
        if self.display.last_time >= 0.0
            && ((height * FT - self.display.last_height).abs() > 500.0
                || (self.f("along_m") - self.display.last_along).abs() > 500.0)
        {
            self.guidance.reset();
            self.display.reset();
        }
        self.guidance.update(i);
        self.put_f("gamma_command_deg", self.guidance.gamma);
        self.put_f("speedbrake_command_ratio", self.guidance.speedbrake);
        self.put_f("height_command_m", self.guidance.target_height);
        self.put_f("gear_command", self.guidance.gear);
        self.put_i("guidance_phase", self.guidance.phase);
    }
    fn release_chute(&mut self) {
        if self.chute_owned && self.matched {
            self.native.set(
                "sim/aircraft/specialcontrols/acf_chute_area",
                self.chute_base as f32,
            );
        }
        self.chute_owned = false;
        self.put_i("chute_reefing_active", 0);
        self.put_f("chute_area_ratio", 1.0);
    }
    fn chute(&mut self) {
        if !self.matched
            || !self.plugin_enabled
            || self.i("landing_systems_enabled") == 0
            || self.native.val("sim/time/is_in_replay", 0.0) != 0.0
            || self.native.val("sim/time/paused", 0.0) != 0.0
        {
            self.release_chute();
            return;
        }
        if self.native.val("sim/cockpit/switches/parachute_on", 0.0) == 0.0 {
            self.release_chute();
            self.chute_start = -1.0;
            return;
        }
        let now = self.native.val("sim/time/total_flight_time_sec", 0.0);
        if self.chute_start < 0.0 {
            if self.i("main_wow") == 0 {
                return;
            }
            self.chute_start = now;
        }
        if now < self.chute_start {
            self.release_chute();
            self.chute_start = -1.0;
            return;
        }
        if !self.chute_owned {
            self.chute_base = self
                .native
                .val("sim/aircraft/specialcontrols/acf_chute_area", 0.0);
            if self.chute_base <= 0.0 {
                return;
            }
            self.chute_owned = true;
        }
        self.put_f("chute_area_ratio", chute_area_ratio(now - self.chute_start));
        self.put_i(
            "chute_reefing_active",
            i32::from(self.f("chute_area_ratio") < 1.0),
        );
        self.native.set(
            "sim/aircraft/specialcontrols/acf_chute_area",
            (self.chute_base * self.f("chute_area_ratio")) as f32,
        );
    }
    fn presentation(&mut self) {
        if !self.matched || !self.plugin_enabled {
            return;
        }
        let r = &self.runways[self.runway_index];
        self.put_f(
            "radar_height_ft",
            (self.native.val("sim/flightmodel/position/elevation", 0.0) - self.radar_ground
                + self.wheel_offset())
            .max(0.0)
                * FT,
        );
        let height = self.f("main_wheel_height_ft");
        let along = self.f("along_m");
        let i = HudInput {
            time: self.native.val("sim/time/total_flight_time_sec", 0.0),
            height_ft: height,
            along,
            eas: self.f("equivalent_airspeed_kt"),
            groundspeed: self.f("groundspeed_mps"),
            heading_error: wrap(self.f("ground_track_deg") - r.heading),
            cross_ft: self.f("cross_m") * FT,
            path_error_ft: height - LandingPath::at(along).height * FT,
            gamma_error: deg(f64::from(
                (self.f("vertical_velocity_mps") as f32)
                    .atan2((self.f("groundspeed_mps") as f32).max(1.0)),
            )) + 20.0,
            bank: self.native.val("sim/flightmodel/position/phi", 0.0),
            stop_distance: r.length - r.displaced - along - 1000.0 / FT,
            gear: std::array::from_fn(|k| {
                self.native
                    .arr("sim/flightmodel2/gear/deploy_ratio", k as i32)
            }),
            main: self.i("main_wow") != 0,
            nose: self.i("nose_wow") != 0,
            final_flare: self.guidance.final_flare,
            replay: self.native.val("sim/time/is_in_replay", 0.0) != 0.0,
            automatic: self.native.val("sim/cockpit2/autopilot/servos_on", 0.0) != 0.0
                && self.native.val("sim/cockpit/autopilot/autopilot_mode", 0.0) == 2.0,
        };
        let visible = self
            .native
            .val("sim/weather/visibility_reported_m", 50000.0)
            > along.hypot(self.f("cross_m"));
        self.display.update(i);
        let level = self
            .display
            .level(self.i("declutter_mode"), height, visible);
        self.put_i("declutter_level", level);
        self.put_i("display_phase", self.display.phase as i32);
        self.put_i("display_main_wow", i32::from(self.display.main));
        self.put_i("display_nose_wow", i32::from(self.display.nose));
        self.put_i("gear_cue", self.display.gear_cue);
        self.put_i("control_auto", i32::from(self.display.automatic));
        self.put_f("velocity_vector_blend", self.display.fade / 5.0);
        self.put_f(
            "display_altitude_ft",
            f64::from(digital_height(
                if self.radar_valid && self.f("radar_height_ft") < 5000.0 {
                    self.f("radar_height_ft")
                } else {
                    height
                },
            )),
        );
        self.put_f("deceleration_g", self.display.deceleration);
        self.put_f("deceleration_command_g", self.display.required_deceleration);
        self.put_i("display_flags", self.display.flags(level));
    }
    fn tick(&mut self) {
        self.check_match();
        self.controls_changed();
        if !self.native.ready() {
            self.release_chute();
            self.put_i("active", 0);
            return;
        }
        self.kinematics();
        self.guidance();
        self.chute();
        let now = self.native.val("sim/time/total_running_time_sec", 0.0);
        if now >= self.next_radar {
            self.radar();
            self.next_radar = now + 0.2;
        }
        self.presentation();
        if self.old_view.is_some()
            && (!self.matched
                || self.i("enabled") == 0
                || !self.plugin_enabled
                || self.native.val("sim/graphics/view/view_type", 0.0) as i32 != 1024)
        {
            self.restore_view();
        }
    }
    fn draw(&mut self, panel: bool) {
        let view_type = self.native.val("sim/graphics/view/view_type", 0.0) as i32;
        let powered = self.native.val("sim/cockpit2/switches/HUD_on", 0.0) != 0.0
            && self
                .native
                .val("sim/cockpit2/electrical/HUD_brightness_ratio", 0.0)
                > 0.0;
        let enabled = self.plugin_enabled && self.i("enabled") != 0 && powered;
        if !enabled || !self.matched {
            self.put_i("runway_projection_valid", 0);
        }
        if panel {
            if !self.matched {
                return;
            }
            let render_type = self.native.val("sim/graphics/view/panel_render_type", 0.0) as i32;
            if render_type == 0 {
                return;
            }
            graphics::clear_panel(self.optics);
            if render_type != 2 {
                return;
            }
            if !enabled {
                self.put_i("cockpit_active", 0);
                return;
            }
        } else {
            let cockpit =
                enabled && self.matched && self.i("cockpit_draw_frames") > 0 && view_type == 1026;
            self.put_i("cockpit_active", i32::from(cockpit));
            self.put_i("active", i32::from(cockpit));
            if view_type != 1024 || self.native.val("sim/graphics/VR/enabled", 0.0) != 0.0 {
                return;
            }
        }
        if !enabled || !self.matched {
            return;
        }
        let size = screen_size();
        if size.0 <= 0 || size.1 <= 0 {
            return;
        }
        let scale = f64::from(size.1) / 1080.0;
        let logical_w = f64::from(size.0) / scale;
        let shift = (logical_w - 1920.0) / 2.0;
        let r = &self.runways[self.runway_index];
        let altitude = self.native.val("sim/flightmodel/position/elevation", 0.0);
        let (along, cross) = r.offsets(
            self.native.val("sim/flightmodel/position/latitude", 0.0),
            self.native.val("sim/flightmodel/position/longitude", 0.0),
        );
        let pitch = self.native.val("sim/flightmodel/position/theta", 0.0);
        let roll = self.native.val("sim/flightmodel/position/phi", 0.0);
        let heading = self.native.val("sim/flightmodel/position/psi", 0.0);
        let mut matrix = [0.0_f32; 16];
        if let Some(reference) = self.native.find("sim/graphics/view/projection_matrix_3d") {
            reference.read_f32(&mut matrix);
        }
        let mut world = [0.0_f32; 16];
        let mut aircraft = [0.0_f32; 16];
        if let Some(reference) = self.native.find("sim/graphics/view/world_matrix") {
            reference.read_f32(&mut world);
        }
        if let Some(reference) = self.native.find("sim/graphics/view/acf_matrix") {
            reference.read_f32(&mut aircraft);
        }
        let runway_valid = valid_rotation(&world)
            && valid_rotation(&aircraft)
            && matrix.iter().all(|v| v.is_finite())
            && matrix[0] > 0.01
            && matrix[5] > 0.01
            && !self.runway_surface.edges.is_empty();
        self.put_i("runway_projection_valid", i32::from(runway_valid));
        let runway_rays = if runway_valid {
            self.runway_surface.rays(&world, &aircraft, |p| {
                let local = world_to_local(p.lat, p.lon, p.elevation);
                [local.0, local.1, local.2]
            })
        } else {
            RunwayRays::default()
        };
        let runway_camera = CameraProjection {
            aircraft,
            projection: matrix,
            logical_width: logical_w,
        };
        let fx = (if matrix[0] > 0.01 {
            f64::from(matrix[0])
        } else {
            1.0 / rad(self.native.val("sim/graphics/view/field_of_view_deg", 65.0) / 2.0).tan()
        }) * logical_w
            / 2.0;
        let camera = View {
            heading: self.native.val("sim/graphics/view/view_heading", heading),
            pitch: self.native.val("sim/graphics/view/view_pitch", pitch),
            roll: self.native.val("sim/graphics/view/view_roll", roll),
            fx,
            fy: if matrix[5] > 0.01 {
                f64::from(matrix[5] * 540.0)
            } else {
                fx
            },
            center: Point::new(
                logical_w * (0.5 - 0.5 * f64::from(matrix[8])) - shift,
                f64::from(540.0 * (1.0 + matrix[9])),
            ),
        };
        let scene = scene::build(&Frame {
            optics: self.optics,
            runway: r,
            runway_rays: Some(&runway_rays),
            display: &self.display,
            panel,
            level: self.i("declutter_level"),
            heading,
            pitch,
            roll,
            along,
            cross,
            altitude,
            height_ft: self.f("main_wheel_height_ft"),
            radar_height_ft: self.f("radar_height_ft"),
            use_radar: self.radar_valid && self.f("radar_height_ft") < 5000.0,
            equivalent: self.f("equivalent_airspeed_kt"),
            groundspeed: self.f("groundspeed_mps"),
            ground_track: self.f("ground_track_deg"),
            vertical_velocity: self.f("vertical_velocity_mps"),
            command_gamma: self.f("gamma_command_deg"),
            command_height: self.f("height_command_m"),
            command_speedbrake: self.f("speedbrake_command_ratio"),
            actual_speedbrake: self.native.val(
                "sim/flightmodel2/controls/speedbrake_ratio",
                self.native
                    .val("sim/cockpit2/controls/speedbrake_ratio", 0.0),
            ),
            horizontal_cage: self.i("att_ref_caged") != 0,
            time: self.native.val("sim/time/total_running_time_sec", 0.0),
            nz: self.native.val("sim/flightmodel/forces/g_nrml", 1.0),
        });
        self.put_i("velocity_limited", i32::from(scene.velocity_limited));
        self.put_i("guidance_limited", i32::from(scene.guidance_limited));
        graphics::paint(
            &scene,
            self.optics,
            camera,
            runway_valid.then_some(&runway_camera),
            panel,
            size,
            self.f("brightness") as f32,
        );
        if panel {
            self.put_i(
                "cockpit_draw_frames",
                self.i("cockpit_draw_frames").saturating_add(1),
            );
            self.put_i("cockpit_active", i32::from(view_type == 1026));
            self.put_i("active", i32::from(view_type == 1026));
        } else {
            self.put_i("active", 1);
        }
        self.put_i("draw_frames", self.i("draw_frames").saturating_add(1));
    }
    fn command(&mut self, id: usize) -> i32 {
        self.check_match();
        if !self.matched {
            return 1;
        }
        match id {
            0 | 4 => self.cockpit_view(),
            1 => {
                self.put_i("enabled", i32::from(self.i("enabled") == 0));
                if self.i("enabled") == 0 {
                    self.restore_view();
                }
            }
            2 => {
                if self.display.main {
                    self.display.ground_declutter = (self.display.ground_declutter + 1) % 3;
                } else {
                    self.put_i(
                        "declutter_mode",
                        if self.i("declutter_mode") < 0 {
                            0
                        } else {
                            (self.i("declutter_mode") + 1) % 4
                        },
                    );
                }
            }
            3 => {
                self.runway_index = (self.runway_index + 1) % self.runways.len();
                self.put_i("runway_index", self.runway_index as i32);
                self.terrain();
            }
            5 => self.fullscreen_view(),
            6 => {
                self.put_i("declutter_mode", -1);
                self.display.ground_declutter = 0;
            }
            7 => self.put_i("att_ref_caged", i32::from(self.i("att_ref_caged") == 0)),
            _ => {}
        }
        if id == 4 && self.i("enabled") != 0 {
            0
        } else {
            1
        }
    }
    fn disable(&mut self) {
        self.release_chute();
        self.plugin_enabled = false;
        self.put_i("plugin_enabled", 0);
        self.put_i("active", 0);
        self.put_i("cockpit_active", 0);
        self.put_i("runway_projection_valid", 0);
        self.restore_view();
    }
}
impl Drop for Runtime {
    fn drop(&mut self) {
        self.disable();
        self.draw_callbacks.clear();
        self.flight_loop.take();
        self.menu.take();
        self.commands.clear();
        self.published.clear();
        self.probe.take();
        LOG.log("cleanup complete; native panel callback released.");
    }
}
const COMMANDS: [(&str, &str); 8] = [
    ("fsim_hud/view", "Show shuttle cockpit HUD"),
    ("fsim_hud/toggle", "Toggle shuttle HUD symbology"),
    ("fsim_hud/declutter_cycle", "Cycle manual HUD declutter"),
    ("fsim_hud/next_runway", "Select next shuttle runway"),
    ("sim/view/forward_with_hud", ""),
    ("fsim_hud/fullscreen", "Show full-screen shuttle HUD"),
    ("fsim_hud/declutter_auto", "Use F-SIM automatic declutter"),
    ("fsim_hud/att_ref", "Toggle ATT REF horizontal cage"),
];
fn controls_changed() {
    with_state(Runtime::controls_changed);
}
extern "C" fn command_callback(
    _: XPLMCommandRef,
    phase: XPLMCommandPhase,
    token: *mut c_void,
) -> i32 {
    if phase != xplm_CommandBegin {
        return 1;
    }
    with_state(|s| s.command(Command::identifier_from_refcon(token))).unwrap_or(1)
}
extern "C" fn draw_callback(phase: XPLMDrawingPhase, _: c_int, _: *mut c_void) -> i32 {
    with_state(|s| s.draw(phase == xplm_Phase_Gauges));
    1
}
extern "C" fn flight_callback(_: f32, _: f32, _: c_int, _: *mut c_void) -> f32 {
    with_state(Runtime::tick);
    -1.0
}
pub fn start() -> bool {
    xplane_plugin::enable_feature("XPLM_USE_NATIVE_PATHS");
    let result = plugin_directory()
        .ok_or_else(|| "plugin directory unavailable".to_owned())
        .and_then(Runtime::new);
    match result {
        Ok(runtime) => {
            STATE.with(|s| s.replace(Some(runtime)));
            LOG.log(&format!("v{VERSION} Rust registered; native optics, shared guidance and staged drag chute. No position or force overrides."));
            true
        }
        Err(error) => {
            LOG.log(&format!("startup failed: {error}"));
            false
        }
    }
}
pub fn enable() -> bool {
    with_state(|s| {
        s.plugin_enabled = true;
        s.put_i("plugin_enabled", 1);
        true
    })
    .unwrap_or(false)
}
pub fn disable() {
    with_state(Runtime::disable);
}
pub fn stop() {
    let old = STATE.with(|s| s.replace(None));
    drop(old);
    LOG.log("clean stop; presentation settings restored.");
}
pub fn receive_message(_: XPLMPluginID, message: c_int, parameter: *mut c_void) {
    if message as u32 == XPLM_MSG_SCENERY_LOADED {
        with_state(Runtime::refresh_runway_surface);
    }
    if message as u32 == XPLM_MSG_PLANE_LOADED && parameter.is_null() {
        with_state(|s| {
            s.native.refs.borrow_mut().clear();
            s.display.reset();
            s.guidance.reset();
            s.next_radar = 0.0;
            s.check_match();
            s.choose_runway();
            s.terrain();
        });
    }
}
