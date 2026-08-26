#![allow(non_snake_case)]

mod ui;

use std::ffi::{c_char, c_float, c_int, c_void, CStr, CString};
use std::fs;
use std::mem;
use std::path::PathBuf;
use std::ptr;
use std::sync::{Mutex, MutexGuard, OnceLock};

use crate::pad::{
    normalize_heading, parse_pad, safe_pad_filename, write_pad, AutopilotData, Field, Form, PadData,
};
use crate::xplm::*;
use ui::{
    draw_window, handle_cursor, handle_key, handle_mouse, handle_right_click, handle_wheel,
    UiAction, DROPDOWN_ROWS,
};

const WINDOW_WIDTH: i32 = 720;
const WINDOW_HEIGHT: i32 = 650;
const METERS_TO_FEET: f64 = 3.280_839_895_013_1;
const KNOTS_TO_MPS: f64 = 0.514_444_444_444_44;

static STATE: OnceLock<Mutex<Option<PluginState>>> = OnceLock::new();

fn state_lock() -> MutexGuard<'static, Option<PluginState>> {
    STATE
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum CommandAction {
    ToggleWindow = 1,
    CaptureCurrent,
    PositionLoaded,
    QuickSave,
    QuickLoad,
    QuickLoadAndPosition,
    PreviousPad,
    NextPad,
    PreviousPadAndPosition,
    NextPadAndPosition,
}

impl CommandAction {
    fn from_refcon(refcon: *mut c_void) -> Option<Self> {
        match refcon as usize {
            1 => Some(Self::ToggleWindow),
            2 => Some(Self::CaptureCurrent),
            3 => Some(Self::PositionLoaded),
            4 => Some(Self::QuickSave),
            5 => Some(Self::QuickLoad),
            6 => Some(Self::QuickLoadAndPosition),
            7 => Some(Self::PreviousPad),
            8 => Some(Self::NextPad),
            9 => Some(Self::PreviousPadAndPosition),
            10 => Some(Self::NextPadAndPosition),
            _ => None,
        }
    }
}

struct RegisteredCommand {
    command: XPLMCommandRef,
    action: CommandAction,
}

struct DataRefs {
    latitude: XPLMDataRef,
    longitude: XPLMDataRef,
    elevation: XPLMDataRef,
    theta: XPLMDataRef,
    phi: XPLMDataRef,
    psi: XPLMDataRef,
    magvar: XPLMDataRef,
    ias: XPLMDataRef,
    local_x: XPLMDataRef,
    local_y: XPLMDataRef,
    local_z: XPLMDataRef,
    local_vx: XPLMDataRef,
    local_vy: XPLMDataRef,
    local_vz: XPLMDataRef,
    rate_p: XPLMDataRef,
    rate_q: XPLMDataRef,
    rate_r: XPLMDataRef,
    quaternion: XPLMDataRef,
    throttles: XPLMDataRef,
    flaps: XPLMDataRef,
    gear: XPLMDataRef,
    ap_mode: XPLMDataRef,
    ap_altitude: XPLMDataRef,
    ap_vvi: XPLMDataRef,
    ap_heading: XPLMDataRef,
    ap_airspeed: XPLMDataRef,
    ap_state: XPLMDataRef,
    ap_heading_roll_mode: XPLMDataRef,
    vr_enabled: XPLMDataRef,
}

unsafe impl Send for DataRefs {}

impl DataRefs {
    unsafe fn find() -> Result<Self, String> {
        unsafe fn required(name: &str) -> Result<XPLMDataRef, String> {
            let name_c = CString::new(name).unwrap();
            let data_ref = XPLMFindDataRef(name_c.as_ptr());
            if data_ref.is_null() {
                Err(format!("Missing required dataref: {name}"))
            } else {
                Ok(data_ref)
            }
        }

        Ok(Self {
            latitude: required("sim/flightmodel/position/latitude")?,
            longitude: required("sim/flightmodel/position/longitude")?,
            elevation: required("sim/flightmodel/position/elevation")?,
            theta: required("sim/flightmodel/position/theta")?,
            phi: required("sim/flightmodel/position/phi")?,
            psi: required("sim/flightmodel/position/psi")?,
            magvar: required("sim/flightmodel/position/magnetic_variation")?,
            ias: required("sim/flightmodel/position/indicated_airspeed")?,
            local_x: required("sim/flightmodel/position/local_x")?,
            local_y: required("sim/flightmodel/position/local_y")?,
            local_z: required("sim/flightmodel/position/local_z")?,
            local_vx: required("sim/flightmodel/position/local_vx")?,
            local_vy: required("sim/flightmodel/position/local_vy")?,
            local_vz: required("sim/flightmodel/position/local_vz")?,
            rate_p: required("sim/flightmodel/position/P")?,
            rate_q: required("sim/flightmodel/position/Q")?,
            rate_r: required("sim/flightmodel/position/R")?,
            quaternion: required("sim/flightmodel/position/q")?,
            throttles: required("sim/flightmodel/engine/ENGN_thro")?,
            flaps: required("sim/flightmodel/controls/flaprqst")?,
            gear: required("sim/cockpit/switches/gear_handle_status")?,
            ap_mode: required("sim/cockpit/autopilot/autopilot_mode")?,
            ap_altitude: required("sim/cockpit/autopilot/altitude")?,
            ap_vvi: required("sim/cockpit/autopilot/vertical_velocity")?,
            ap_heading: required("sim/cockpit/autopilot/heading_mag")?,
            ap_airspeed: required("sim/cockpit/autopilot/airspeed")?,
            ap_state: required("sim/cockpit/autopilot/autopilot_state")?,
            ap_heading_roll_mode: required("sim/cockpit/autopilot/heading_roll_mode")?,
            vr_enabled: required("sim/graphics/VR/enabled")?,
        })
    }
}

struct PendingReapply {
    data: PadData,
    wait_frames: i32,
    remaining_frames: i32,
}

struct PluginState {
    window: XPLMWindowID,
    pad_directory: PathBuf,
    pads: Vec<String>,
    selected_index: usize,
    form: Form,
    status: String,
    active_field: Option<Field>,
    hovered_action: Option<UiAction>,
    dropdown_open: bool,
    dropdown_scroll: usize,
    mouse_captured: bool,
    datarefs: DataRefs,
    commands: Vec<RegisteredCommand>,
    menu: XPLMMenuID,
    plugins_menu: XPLMMenuID,
    plugins_menu_item: i32,
    pending: Option<PendingReapply>,
}

unsafe impl Send for PluginState {}

impl PluginState {
    unsafe fn capture_current(&mut self) -> PadData {
        let mut throttle = 0.0_f32;
        XPLMGetDatavf(self.datarefs.throttles, &mut throttle, 0, 1);
        let data = PadData {
            latitude: XPLMGetDatad(self.datarefs.latitude),
            longitude: XPLMGetDatad(self.datarefs.longitude),
            altitude: XPLMGetDatad(self.datarefs.elevation) * METERS_TO_FEET,
            heading: normalize_heading(
                XPLMGetDataf(self.datarefs.psi) as f64 + XPLMGetDataf(self.datarefs.magvar) as f64,
            ),
            pitch: XPLMGetDataf(self.datarefs.theta) as f64,
            roll: XPLMGetDataf(self.datarefs.phi) as f64,
            speed: XPLMGetDataf(self.datarefs.ias) as f64,
            throttle: throttle as f64,
            flaps: XPLMGetDataf(self.datarefs.flaps) as f64,
            gear: XPLMGetDatai(self.datarefs.gear),
            use_ap: self.form.use_ap,
            ap: AutopilotData {
                mode: XPLMGetDatai(self.datarefs.ap_mode),
                altitude: XPLMGetDataf(self.datarefs.ap_altitude) as f64,
                vertical_velocity: XPLMGetDataf(self.datarefs.ap_vvi) as f64,
                heading: XPLMGetDataf(self.datarefs.ap_heading) as f64,
                airspeed: XPLMGetDataf(self.datarefs.ap_airspeed) as f64,
                state: XPLMGetDatai(self.datarefs.ap_state),
                heading_roll_mode: XPLMGetDatai(self.datarefs.ap_heading_roll_mode),
            },
        };
        let save_name = self.form.value(Field::SaveName).to_owned();
        self.form = Form::from_data(&data, &save_name);
        self.status = "Captured current aircraft data".to_owned();
        data
    }

    fn refresh_pads(&mut self) {
        let old = self.pads.get(self.selected_index).cloned();
        let mut pads = Vec::new();
        match fs::read_dir(&self.pad_directory) {
            Ok(entries) => {
                for entry in entries.flatten() {
                    if !entry
                        .file_type()
                        .map(|kind| kind.is_file())
                        .unwrap_or(false)
                    {
                        continue;
                    }
                    let name = entry.file_name().to_string_lossy().into_owned();
                    if name.to_ascii_lowercase().ends_with(".pad") {
                        pads.push(name);
                    }
                }
                pads.sort_by_key(|name| name.to_ascii_lowercase());
            }
            Err(error) => {
                self.status = format!("Unable to read PAD directory: {error}");
            }
        }
        self.pads = pads;
        self.selected_index = old
            .as_ref()
            .and_then(|old_name| self.pads.iter().position(|name| name == old_name))
            .or_else(|| {
                self.pads
                    .iter()
                    .position(|name| name.eq_ignore_ascii_case("QuickFile.pad"))
            })
            .unwrap_or(0)
            .min(self.pads.len().saturating_sub(1));
        self.dropdown_scroll = self.dropdown_scroll.min(self.dropdown_max_scroll());
        if !self.status.starts_with("Unable") {
            self.status = format!("Found {} PAD files", self.pads.len());
        }
    }

    fn selected_name(&self) -> Option<&str> {
        self.pads.get(self.selected_index).map(String::as_str)
    }

    fn dropdown_max_scroll(&self) -> usize {
        self.pads.len().saturating_sub(DROPDOWN_ROWS)
    }

    fn open_dropdown(&mut self) {
        self.dropdown_open = true;
        self.dropdown_scroll = self
            .selected_index
            .saturating_sub(DROPDOWN_ROWS / 2)
            .min(self.dropdown_max_scroll());
    }

    fn scroll_dropdown(&mut self, delta: isize) {
        let next = (self.dropdown_scroll as isize + delta)
            .clamp(0, self.dropdown_max_scroll() as isize) as usize;
        self.dropdown_scroll = next;
    }

    fn select_pad(&mut self, index: usize) {
        if index < self.pads.len() {
            self.selected_index = index;
            self.status = format!("Selected {}", self.pads[index]);
        }
        self.dropdown_open = false;
    }

    fn load_file(&mut self, filename: &str) -> bool {
        match parse_pad(&self.pad_directory.join(filename)) {
            Ok(data) => {
                let save_name = filename
                    .strip_suffix(".pad")
                    .or_else(|| filename.strip_suffix(".PAD"))
                    .unwrap_or(filename);
                self.form = Form::from_data(&data, save_name);
                self.status = format!("Loaded {filename}");
                true
            }
            Err(error) => {
                self.status = error;
                false
            }
        }
    }

    fn load_selected(&mut self, position: bool) {
        let Some(filename) = self.selected_name().map(str::to_owned) else {
            self.status = "No PAD file is selected".to_owned();
            return;
        };
        if self.load_file(&filename) && position {
            unsafe { self.position_loaded() };
        }
    }

    fn select_relative(&mut self, delta: isize, position: bool) {
        if self.pads.is_empty() {
            self.refresh_pads();
        }
        if self.pads.is_empty() {
            self.status = "No PAD files found".to_owned();
            return;
        }
        self.selected_index =
            (self.selected_index as isize + delta).rem_euclid(self.pads.len() as isize) as usize;
        self.dropdown_open = false;
        self.load_selected(position);
    }

    fn quick_load(&mut self, position: bool) {
        if self.load_file("QuickFile.pad") && position {
            unsafe { self.position_loaded() };
        }
    }

    unsafe fn quick_save(&mut self) {
        let data = self.capture_current();
        match write_pad(&self.pad_directory.join("QuickFile.pad"), &data) {
            Ok(()) => {
                self.refresh_pads();
                self.status = "Quick-saved current aircraft to QuickFile.pad".to_owned();
            }
            Err(error) => self.status = format!("Unable to write QuickFile.pad: {error}"),
        }
    }

    fn save_named(&mut self) {
        let data = match self.form.to_data() {
            Ok(data) => data,
            Err(error) => {
                self.status = error;
                return;
            }
        };
        let Some(filename) = safe_pad_filename(self.form.value(Field::SaveName)) else {
            self.status = "Enter a PAD filename".to_owned();
            return;
        };
        match write_pad(&self.pad_directory.join(&filename), &data) {
            Ok(()) => {
                self.refresh_pads();
                if let Some(index) = self.pads.iter().position(|name| name == &filename) {
                    self.selected_index = index;
                }
                self.status = format!("Saved {filename}");
            }
            Err(error) => self.status = format!("Unable to write {filename}: {error}"),
        }
    }

    unsafe fn position_loaded(&mut self) {
        let data = match self.form.to_data() {
            Ok(data) => data,
            Err(error) => {
                self.status = error;
                return;
            }
        };
        let mut x = 0.0;
        let mut y = 0.0;
        let mut z = 0.0;
        XPLMWorldToLocal(
            data.latitude,
            data.longitude,
            data.altitude / METERS_TO_FEET,
            &mut x,
            &mut y,
            &mut z,
        );
        XPLMSetDatad(self.datarefs.local_x, x);
        XPLMSetDatad(self.datarefs.local_y, y);
        XPLMSetDatad(self.datarefs.local_z, z);
        self.apply_attitude_velocity_controls(&data);
        self.pending = Some(PendingReapply {
            data: data.clone(),
            wait_frames: 2,
            remaining_frames: 6,
        });
        self.status = format!(
            "Positioned: {:.5}, {:.5} at {:.0} ft",
            data.latitude, data.longitude, data.altitude
        );
    }

    unsafe fn apply_attitude_velocity_controls(&self, data: &PadData) {
        let true_heading =
            normalize_heading(data.heading - XPLMGetDataf(self.datarefs.magvar) as f64);
        let psi = true_heading.to_radians() * 0.5;
        let theta = data.pitch.to_radians() * 0.5;
        let phi = data.roll.to_radians() * 0.5;
        let (sin_psi, cos_psi) = psi.sin_cos();
        let (sin_theta, cos_theta) = theta.sin_cos();
        let (sin_phi, cos_phi) = phi.sin_cos();
        let q = [
            (cos_psi * cos_theta * cos_phi + sin_psi * sin_theta * sin_phi) as f32,
            (cos_psi * cos_theta * sin_phi - sin_psi * sin_theta * cos_phi) as f32,
            (cos_psi * sin_theta * cos_phi + sin_psi * cos_theta * sin_phi) as f32,
            (-cos_psi * sin_theta * sin_phi + sin_psi * cos_theta * cos_phi) as f32,
        ];
        XPLMSetDatavf(self.datarefs.quaternion, q.as_ptr(), 0, q.len() as i32);

        let speed_mps = data.speed * KNOTS_TO_MPS;
        let heading_rad = true_heading.to_radians();
        let pitch_rad = data.pitch.to_radians();
        let horizontal_speed = speed_mps * pitch_rad.cos();
        XPLMSetDataf(
            self.datarefs.local_vx,
            (horizontal_speed * heading_rad.sin()) as f32,
        );
        XPLMSetDataf(self.datarefs.local_vy, (speed_mps * pitch_rad.sin()) as f32);
        XPLMSetDataf(
            self.datarefs.local_vz,
            (-horizontal_speed * heading_rad.cos()) as f32,
        );
        XPLMSetDataf(self.datarefs.rate_p, 0.0);
        XPLMSetDataf(self.datarefs.rate_q, 0.0);
        XPLMSetDataf(self.datarefs.rate_r, 0.0);

        let throttles = [data.throttle.clamp(0.0, 1.0) as f32; 16];
        XPLMSetDatavf(
            self.datarefs.throttles,
            throttles.as_ptr(),
            0,
            throttles.len() as i32,
        );
        XPLMSetDataf(self.datarefs.flaps, data.flaps.clamp(0.0, 1.0) as f32);
        XPLMSetDatai(self.datarefs.gear, if data.gear != 0 { 1 } else { 0 });

        if data.use_ap {
            XPLMSetDataf(self.datarefs.ap_altitude, data.ap.altitude as f32);
            XPLMSetDataf(self.datarefs.ap_vvi, data.ap.vertical_velocity as f32);
            XPLMSetDataf(
                self.datarefs.ap_heading,
                normalize_heading(data.ap.heading) as f32,
            );
            XPLMSetDataf(self.datarefs.ap_airspeed, data.ap.airspeed as f32);
            XPLMSetDatai(
                self.datarefs.ap_heading_roll_mode,
                data.ap.heading_roll_mode,
            );
            XPLMSetDatai(self.datarefs.ap_state, data.ap.state);
            XPLMSetDatai(self.datarefs.ap_mode, data.ap.mode);
        }
    }

    unsafe fn toggle_window(&mut self) {
        if self.window.is_null() {
            return;
        }
        if XPLMGetWindowIsVisible(self.window) != 0 {
            XPLMSetWindowIsVisible(self.window, 0);
            self.active_field = None;
            self.hovered_action = None;
            self.dropdown_open = false;
            XPLMTakeKeyboardFocus(ptr::null_mut());
        } else {
            XPLMSetWindowIsVisible(self.window, 1);
            XPLMBringWindowToFront(self.window);
        }
    }
}

fn c_string(value: &str) -> CString {
    CString::new(value.replace('\0', " ")).unwrap()
}

unsafe fn log(message: &str) {
    let message = c_string(&format!("PositionAircraftNative: {message}\n"));
    XPLMDebugString(message.as_ptr());
}

unsafe fn write_plugin_string(destination: *mut c_char, value: &str) {
    if destination.is_null() {
        return;
    }
    let bytes = value.as_bytes();
    ptr::copy_nonoverlapping(bytes.as_ptr(), destination.cast::<u8>(), bytes.len());
    *destination.add(bytes.len()) = 0;
}

unsafe fn system_path() -> PathBuf {
    let mut buffer = [0_i8; 1024];
    XPLMGetSystemPath(buffer.as_mut_ptr());
    let path = CStr::from_ptr(buffer.as_ptr())
        .to_string_lossy()
        .into_owned();
    PathBuf::from(path)
}

unsafe fn execute_command(action: CommandAction) {
    let mut guard = state_lock();
    let Some(state) = guard.as_mut() else { return };
    match action {
        CommandAction::ToggleWindow => state.toggle_window(),
        CommandAction::CaptureCurrent => {
            state.capture_current();
        }
        CommandAction::PositionLoaded => state.position_loaded(),
        CommandAction::QuickSave => state.quick_save(),
        CommandAction::QuickLoad => state.quick_load(false),
        CommandAction::QuickLoadAndPosition => state.quick_load(true),
        CommandAction::PreviousPad => state.select_relative(-1, false),
        CommandAction::NextPad => state.select_relative(1, false),
        CommandAction::PreviousPadAndPosition => state.select_relative(-1, true),
        CommandAction::NextPadAndPosition => state.select_relative(1, true),
    }
}

unsafe extern "C" fn command_handler(
    _command: XPLMCommandRef,
    phase: c_int,
    refcon: *mut c_void,
) -> c_int {
    if phase == XPLM_COMMAND_BEGIN {
        if let Some(action) = CommandAction::from_refcon(refcon) {
            execute_command(action);
        }
    }
    1
}

unsafe extern "C" fn flight_loop(
    _elapsed_since_last_call: c_float,
    _elapsed_since_last_loop: c_float,
    _counter: c_int,
    _refcon: *mut c_void,
) -> c_float {
    let mut guard = state_lock();
    let Some(state) = guard.as_mut() else {
        return -1.0;
    };
    let Some(mut pending) = state.pending.take() else {
        return -1.0;
    };
    if pending.wait_frames > 0 {
        pending.wait_frames -= 1;
        state.pending = Some(pending);
        return -1.0;
    }
    state.apply_attitude_velocity_controls(&pending.data);
    pending.remaining_frames -= 1;
    if pending.remaining_frames > 0 {
        state.pending = Some(pending);
    }
    -1.0
}

unsafe extern "C" fn menu_handler(_menu_ref: *mut c_void, _item_ref: *mut c_void) {}

unsafe fn create_window() -> Result<XPLMWindowID, String> {
    let mut screen_left = 0;
    let mut screen_top = 0;
    let mut screen_right = 0;
    let mut screen_bottom = 0;
    XPLMGetScreenBoundsGlobal(
        &mut screen_left,
        &mut screen_top,
        &mut screen_right,
        &mut screen_bottom,
    );
    let mut params = XPLMCreateWindowT {
        struct_size: mem::size_of::<XPLMCreateWindowT>() as i32,
        left: screen_left + 100,
        top: screen_top - 100,
        right: screen_left + 100 + WINDOW_WIDTH,
        bottom: screen_top - 100 - WINDOW_HEIGHT,
        visible: 0,
        draw_window_func: Some(draw_window),
        handle_mouse_click_func: Some(handle_mouse),
        handle_key_func: Some(handle_key),
        handle_cursor_func: Some(handle_cursor),
        handle_mouse_wheel_func: Some(handle_wheel),
        refcon: ptr::null_mut(),
        decorate_as_floating_window: XPLM_WINDOW_DECORATION_ROUND_RECTANGLE,
        layer: XPLM_WINDOW_LAYER_FLOATING,
        handle_right_click_func: Some(handle_right_click),
    };
    let window = XPLMCreateWindowEx(&mut params);
    if window.is_null() {
        return Err("XPLMCreateWindowEx failed".to_owned());
    }
    XPLMSetWindowResizingLimits(window, 660, 650, 1000, 900);
    let title = c_string("Position Aircraft - Native Rust");
    XPLMSetWindowTitle(window, title.as_ptr());
    Ok(window)
}

unsafe fn register_commands(state: &mut PluginState) -> Result<(), String> {
    let definitions = [
        (
            CommandAction::ToggleWindow,
            "toggle_window",
            "PositionAircraft Native: Toggle VR/2D panel",
        ),
        (
            CommandAction::CaptureCurrent,
            "capture_current",
            "PositionAircraft Native: Capture current aircraft data",
        ),
        (
            CommandAction::PositionLoaded,
            "position_loaded",
            "PositionAircraft Native: Position using loaded/edited data",
        ),
        (
            CommandAction::QuickSave,
            "quick_save",
            "PositionAircraft Native: Quick-save current aircraft",
        ),
        (
            CommandAction::QuickLoad,
            "quick_load",
            "PositionAircraft Native: Load QuickFile.pad without positioning",
        ),
        (
            CommandAction::QuickLoadAndPosition,
            "quick_load_and_position",
            "PositionAircraft Native: Load QuickFile.pad and position",
        ),
        (
            CommandAction::PreviousPad,
            "previous_pad",
            "PositionAircraft Native: Select and load previous PAD",
        ),
        (
            CommandAction::NextPad,
            "next_pad",
            "PositionAircraft Native: Select and load next PAD",
        ),
        (
            CommandAction::PreviousPadAndPosition,
            "previous_pad_and_position",
            "PositionAircraft Native: Load previous PAD and position",
        ),
        (
            CommandAction::NextPadAndPosition,
            "next_pad_and_position",
            "PositionAircraft Native: Load next PAD and position",
        ),
    ];
    for (action, short_name, description) in definitions {
        let name = c_string(&format!("PositionAircraftNative/{short_name}"));
        let description = c_string(description);
        let command = XPLMCreateCommand(name.as_ptr(), description.as_ptr());
        if command.is_null() {
            return Err(format!("Unable to create command {short_name}"));
        }
        XPLMRegisterCommandHandler(
            command,
            Some(command_handler),
            1,
            action as usize as *mut c_void,
        );
        state.commands.push(RegisteredCommand { command, action });
    }
    Ok(())
}

unsafe fn create_menu(state: &mut PluginState) {
    state.plugins_menu = XPLMFindPluginsMenu();
    let menu_name = c_string("Position Aircraft Native");
    state.plugins_menu_item =
        XPLMAppendMenuItem(state.plugins_menu, menu_name.as_ptr(), ptr::null_mut(), 0);
    state.menu = XPLMCreateMenu(
        menu_name.as_ptr(),
        state.plugins_menu,
        state.plugins_menu_item,
        Some(menu_handler),
        ptr::null_mut(),
    );
    if state.menu.is_null() {
        return;
    }
    let labels = [
        ("Toggle Window", CommandAction::ToggleWindow),
        ("Capture Current", CommandAction::CaptureCurrent),
        ("Position Loaded", CommandAction::PositionLoaded),
        ("Quick Save", CommandAction::QuickSave),
        ("Quick Load + Position", CommandAction::QuickLoadAndPosition),
    ];
    for (label, action) in labels {
        if let Some(command) = state
            .commands
            .iter()
            .find(|registered| registered.action as usize == action as usize)
            .map(|registered| registered.command)
        {
            let label = c_string(label);
            XPLMAppendMenuItemWithCommand(state.menu, label.as_ptr(), command);
        }
    }
}

pub(crate) unsafe fn start(
    out_name: *mut c_char,
    out_signature: *mut c_char,
    out_description: *mut c_char,
) -> c_int {
    write_plugin_string(out_name, "Position Aircraft Native");
    write_plugin_string(out_signature, "com.openai.position-aircraft-native-rust");
    write_plugin_string(
        out_description,
        "Native VR and joystick PositionAircraft replacement written in Rust",
    );

    let datarefs = match DataRefs::find() {
        Ok(datarefs) => datarefs,
        Err(error) => {
            log(&error);
            return 0;
        }
    };
    let pad_directory = system_path()
        .join("Resources")
        .join("plugins")
        .join("PositionAircraft");
    let mut initial = PluginState {
        window: ptr::null_mut(),
        pad_directory,
        pads: Vec::new(),
        selected_index: 0,
        form: Form::from_data(&PadData::default(), "MyPosition"),
        status: "Ready".to_owned(),
        active_field: None,
        hovered_action: None,
        dropdown_open: false,
        dropdown_scroll: 0,
        mouse_captured: false,
        datarefs,
        commands: Vec::new(),
        menu: ptr::null_mut(),
        plugins_menu: ptr::null_mut(),
        plugins_menu_item: -1,
        pending: None,
    };
    initial.refresh_pads();
    initial.capture_current();
    *state_lock() = Some(initial);

    let window = match create_window() {
        Ok(window) => window,
        Err(error) => {
            log(&error);
            *state_lock() = None;
            return 0;
        }
    };
    {
        let mut guard = state_lock();
        let state = guard.as_mut().unwrap();
        state.window = window;
        if XPLMGetDatai(state.datarefs.vr_enabled) != 0 {
            XPLMSetWindowPositioningMode(window, XPLM_WINDOW_VR, -1);
        } else {
            XPLMSetWindowPositioningMode(window, XPLM_WINDOW_POSITION_FREE, -1);
        }
        if let Err(error) = register_commands(state) {
            log(&error);
            XPLMDestroyWindow(window);
            *guard = None;
            return 0;
        }
        create_menu(state);
    }
    XPLMRegisterFlightLoopCallback(Some(flight_loop), -1.0, ptr::null_mut());
    log("0.2.0 loaded (XPLM 4.3 native window, interactive UI)");
    1
}

pub(crate) unsafe fn stop() {
    XPLMUnregisterFlightLoopCallback(Some(flight_loop), ptr::null_mut());
    let mut guard = state_lock();
    let Some(mut state) = guard.take() else {
        return;
    };
    for command in state.commands.drain(..) {
        XPLMUnregisterCommandHandler(
            command.command,
            Some(command_handler),
            1,
            command.action as usize as *mut c_void,
        );
    }
    if !state.menu.is_null() {
        XPLMDestroyMenu(state.menu);
    }
    if !state.plugins_menu.is_null() && state.plugins_menu_item >= 0 {
        XPLMRemoveMenuItem(state.plugins_menu, state.plugins_menu_item);
    }
    if !state.window.is_null() {
        XPLMDestroyWindow(state.window);
    }
    log("unloaded");
}

pub(crate) unsafe fn receive_message(from: XPLMPluginID, message: c_int) {
    if from != XPLM_PLUGIN_XPLANE {
        return;
    }
    let mut guard = state_lock();
    let Some(state) = guard.as_mut() else { return };
    if state.window.is_null() {
        return;
    }
    if message == XPLM_MSG_ENTERED_VR {
        XPLMSetWindowPositioningMode(state.window, XPLM_WINDOW_VR, -1);
    } else if message == XPLM_MSG_EXITING_VR {
        XPLMSetWindowPositioningMode(state.window, XPLM_WINDOW_POSITION_FREE, -1);
        let mut screen_left = 0;
        let mut screen_top = 0;
        let mut screen_right = 0;
        let mut screen_bottom = 0;
        XPLMGetScreenBoundsGlobal(
            &mut screen_left,
            &mut screen_top,
            &mut screen_right,
            &mut screen_bottom,
        );
        XPLMSetWindowGeometry(
            state.window,
            screen_left + 100,
            screen_top - 100,
            screen_left + 100 + WINDOW_WIDTH,
            screen_top - 100 - WINDOW_HEIGHT,
        );
    }
}
