#![allow(non_snake_case)]

mod ui;

use std::cell::RefCell;
use std::ffi::{c_char, c_float, c_int, c_void, CStr, CString};
use std::fs;
use std::mem;
use std::path::PathBuf;
use std::ptr;

use crate::pad::{
    normalize_heading, parse_pad, safe_pad_filename, write_pad, AutopilotData, Field, Form, PadData,
};
use crate::xplm::*;
use ui::{
    draw_window, handle_cursor, handle_key, handle_mouse, handle_right_click, handle_wheel,
    EguiIntegration,
};

const WINDOW_WIDTH: i32 = 720;
const WINDOW_HEIGHT: i32 = 880;
const METERS_TO_FEET: f64 = 3.280_839_895_013_1;
const KNOTS_TO_MPS: f64 = 0.514_444_444_444_44;

thread_local! {
    /// XPLM invokes this plugin's lifecycle, flight-loop, command, and window
    /// callbacks on its plugin thread. Keeping state thread-local makes that
    /// affinity explicit and avoids claiming that XPLM/GL handles are `Send`.
    static STATE: RefCell<Option<PluginState>> = const { RefCell::new(None) };
}

fn with_state_mut<R>(f: impl FnOnce(&mut PluginState) -> R) -> Option<R> {
    STATE.with(|slot| slot.borrow_mut().as_mut().map(f))
}

fn replace_state(state: Option<PluginState>) -> Option<PluginState> {
    STATE.with(|slot| mem::replace(&mut *slot.borrow_mut(), state))
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

#[derive(Copy, Clone)]
struct DataRef(XPLMDataRef);

impl DataRef {
    fn required(name: &str) -> Result<Self, String> {
        let name_c = CString::new(name).unwrap();
        // SAFETY: `name_c` is NUL-terminated and lives for the duration of the
        // call. A non-null XPLM dataref remains owned by X-Plane.
        let data_ref = unsafe { XPLMFindDataRef(name_c.as_ptr()) };
        if data_ref.is_null() {
            Err(format!("Missing required dataref: {name}"))
        } else {
            Ok(Self(data_ref))
        }
    }

    fn get_i32(self) -> i32 {
        // SAFETY: `DataRef` can only be constructed from a successful XPLM lookup.
        unsafe { XPLMGetDatai(self.0) }
    }

    fn get_f32(self) -> f32 {
        // SAFETY: `DataRef` can only be constructed from a successful XPLM lookup.
        unsafe { XPLMGetDataf(self.0) }
    }

    fn get_f64(self) -> f64 {
        // SAFETY: `DataRef` can only be constructed from a successful XPLM lookup.
        unsafe { XPLMGetDatad(self.0) }
    }

    fn read_f32(self, values: &mut [f32]) -> usize {
        let count = i32::try_from(values.len()).expect("XPLM array length exceeds i32::MAX");
        // SAFETY: the slice supplies a valid writable buffer of the advertised length.
        let read = unsafe { XPLMGetDatavf(self.0, values.as_mut_ptr(), 0, count) };
        usize::try_from(read).unwrap_or(0).min(values.len())
    }

    fn read_i32(self, values: &mut [i32]) -> usize {
        let count = i32::try_from(values.len()).expect("XPLM array length exceeds i32::MAX");
        // SAFETY: the slice supplies a valid writable buffer of the advertised length.
        let read = unsafe { XPLMGetDatavi(self.0, values.as_mut_ptr(), 0, count) };
        usize::try_from(read).unwrap_or(0).min(values.len())
    }

    fn set_i32(self, value: i32) {
        // SAFETY: `DataRef` can only be constructed from a successful XPLM lookup.
        unsafe { XPLMSetDatai(self.0, value) }
    }

    fn set_f32(self, value: f32) {
        // SAFETY: `DataRef` can only be constructed from a successful XPLM lookup.
        unsafe { XPLMSetDataf(self.0, value) }
    }

    fn set_f64(self, value: f64) {
        // SAFETY: `DataRef` can only be constructed from a successful XPLM lookup.
        unsafe { XPLMSetDatad(self.0, value) }
    }

    fn write_f32(self, values: &[f32]) {
        let count = i32::try_from(values.len()).expect("XPLM array length exceeds i32::MAX");
        // SAFETY: the slice supplies a valid readable buffer of the advertised length.
        unsafe { XPLMSetDatavf(self.0, values.as_ptr(), 0, count) }
    }
}

struct DataRefs {
    latitude: DataRef,
    longitude: DataRef,
    elevation: DataRef,
    theta: DataRef,
    phi: DataRef,
    psi: DataRef,
    magvar: DataRef,
    ias: DataRef,
    local_x: DataRef,
    local_y: DataRef,
    local_z: DataRef,
    local_vx: DataRef,
    local_vy: DataRef,
    local_vz: DataRef,
    rate_p: DataRef,
    rate_q: DataRef,
    rate_r: DataRef,
    quaternion: DataRef,
    throttles: DataRef,
    flaps: DataRef,
    gear: DataRef,
    ap_mode: DataRef,
    ap_altitude: DataRef,
    ap_vvi: DataRef,
    ap_heading: DataRef,
    ap_airspeed: DataRef,
    ap_state: DataRef,
    ap_heading_roll_mode: DataRef,
    vr_enabled: DataRef,
    projection_matrix: DataRef,
    modelview_matrix: DataRef,
    viewport: DataRef,
}

impl DataRefs {
    fn find() -> Result<Self, String> {
        Ok(Self {
            latitude: DataRef::required("sim/flightmodel/position/latitude")?,
            longitude: DataRef::required("sim/flightmodel/position/longitude")?,
            elevation: DataRef::required("sim/flightmodel/position/elevation")?,
            theta: DataRef::required("sim/flightmodel/position/theta")?,
            phi: DataRef::required("sim/flightmodel/position/phi")?,
            psi: DataRef::required("sim/flightmodel/position/psi")?,
            magvar: DataRef::required("sim/flightmodel/position/magnetic_variation")?,
            ias: DataRef::required("sim/flightmodel/position/indicated_airspeed")?,
            local_x: DataRef::required("sim/flightmodel/position/local_x")?,
            local_y: DataRef::required("sim/flightmodel/position/local_y")?,
            local_z: DataRef::required("sim/flightmodel/position/local_z")?,
            local_vx: DataRef::required("sim/flightmodel/position/local_vx")?,
            local_vy: DataRef::required("sim/flightmodel/position/local_vy")?,
            local_vz: DataRef::required("sim/flightmodel/position/local_vz")?,
            rate_p: DataRef::required("sim/flightmodel/position/P")?,
            rate_q: DataRef::required("sim/flightmodel/position/Q")?,
            rate_r: DataRef::required("sim/flightmodel/position/R")?,
            quaternion: DataRef::required("sim/flightmodel/position/q")?,
            throttles: DataRef::required("sim/flightmodel/engine/ENGN_thro")?,
            flaps: DataRef::required("sim/flightmodel/controls/flaprqst")?,
            gear: DataRef::required("sim/cockpit/switches/gear_handle_status")?,
            ap_mode: DataRef::required("sim/cockpit/autopilot/autopilot_mode")?,
            ap_altitude: DataRef::required("sim/cockpit/autopilot/altitude")?,
            ap_vvi: DataRef::required("sim/cockpit/autopilot/vertical_velocity")?,
            ap_heading: DataRef::required("sim/cockpit/autopilot/heading_mag")?,
            ap_airspeed: DataRef::required("sim/cockpit/autopilot/airspeed")?,
            ap_state: DataRef::required("sim/cockpit/autopilot/autopilot_state")?,
            ap_heading_roll_mode: DataRef::required("sim/cockpit/autopilot/heading_roll_mode")?,
            vr_enabled: DataRef::required("sim/graphics/VR/enabled")?,
            projection_matrix: DataRef::required("sim/graphics/view/projection_matrix")?,
            modelview_matrix: DataRef::required("sim/graphics/view/modelview_matrix")?,
            viewport: DataRef::required("sim/graphics/view/viewport")?,
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
    ui: Option<EguiIntegration>,
    datarefs: DataRefs,
    commands: Vec<RegisteredCommand>,
    menu: XPLMMenuID,
    plugins_menu: XPLMMenuID,
    plugins_menu_item: i32,
    pending: Option<PendingReapply>,
}

impl PluginState {
    fn capture_current(&mut self) -> PadData {
        let mut throttle = 0.0_f32;
        self.datarefs
            .throttles
            .read_f32(std::slice::from_mut(&mut throttle));
        let data = PadData {
            latitude: self.datarefs.latitude.get_f64(),
            longitude: self.datarefs.longitude.get_f64(),
            altitude: self.datarefs.elevation.get_f64() * METERS_TO_FEET,
            heading: normalize_heading(
                self.datarefs.psi.get_f32() as f64 + self.datarefs.magvar.get_f32() as f64,
            ),
            pitch: self.datarefs.theta.get_f32() as f64,
            roll: self.datarefs.phi.get_f32() as f64,
            speed: self.datarefs.ias.get_f32() as f64,
            throttle: throttle as f64,
            flaps: self.datarefs.flaps.get_f32() as f64,
            gear: self.datarefs.gear.get_i32(),
            use_ap: self.form.use_ap,
            ap: AutopilotData {
                mode: self.datarefs.ap_mode.get_i32(),
                altitude: self.datarefs.ap_altitude.get_f32() as f64,
                vertical_velocity: self.datarefs.ap_vvi.get_f32() as f64,
                heading: self.datarefs.ap_heading.get_f32() as f64,
                airspeed: self.datarefs.ap_airspeed.get_f32() as f64,
                state: self.datarefs.ap_state.get_i32(),
                heading_roll_mode: self.datarefs.ap_heading_roll_mode.get_i32(),
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
        if !self.status.starts_with("Unable") {
            self.status = format!("Found {} PAD files", self.pads.len());
        }
    }

    fn selected_name(&self) -> Option<&str> {
        self.pads.get(self.selected_index).map(String::as_str)
    }

    fn select_pad(&mut self, index: usize) {
        if index < self.pads.len() {
            self.selected_index = index;
            self.status = format!("Selected {}", self.pads[index]);
        }
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
            self.position_loaded();
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
        self.load_selected(position);
    }

    fn quick_load(&mut self, position: bool) {
        if self.load_file("QuickFile.pad") && position {
            self.position_loaded();
        }
    }

    fn quick_save(&mut self) {
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

    fn position_loaded(&mut self) {
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
        // SAFETY: all output pointers refer to live local variables and X-Plane
        // owns the coordinate conversion routine for the active scenery.
        unsafe {
            XPLMWorldToLocal(
                data.latitude,
                data.longitude,
                data.altitude / METERS_TO_FEET,
                &mut x,
                &mut y,
                &mut z,
            );
        }
        self.datarefs.local_x.set_f64(x);
        self.datarefs.local_y.set_f64(y);
        self.datarefs.local_z.set_f64(z);
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

    fn apply_attitude_velocity_controls(&self, data: &PadData) {
        let true_heading = normalize_heading(data.heading - self.datarefs.magvar.get_f32() as f64);
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
        self.datarefs.quaternion.write_f32(&q);

        let speed_mps = data.speed * KNOTS_TO_MPS;
        let heading_rad = true_heading.to_radians();
        let pitch_rad = data.pitch.to_radians();
        let horizontal_speed = speed_mps * pitch_rad.cos();
        self.datarefs
            .local_vx
            .set_f32((horizontal_speed * heading_rad.sin()) as f32);
        self.datarefs
            .local_vy
            .set_f32((speed_mps * pitch_rad.sin()) as f32);
        self.datarefs
            .local_vz
            .set_f32((-horizontal_speed * heading_rad.cos()) as f32);
        self.datarefs.rate_p.set_f32(0.0);
        self.datarefs.rate_q.set_f32(0.0);
        self.datarefs.rate_r.set_f32(0.0);

        let throttles = [data.throttle.clamp(0.0, 1.0) as f32; 16];
        self.datarefs.throttles.write_f32(&throttles);
        self.datarefs
            .flaps
            .set_f32(data.flaps.clamp(0.0, 1.0) as f32);
        self.datarefs
            .gear
            .set_i32(if data.gear != 0 { 1 } else { 0 });

        if data.use_ap {
            self.datarefs.ap_altitude.set_f32(data.ap.altitude as f32);
            self.datarefs
                .ap_vvi
                .set_f32(data.ap.vertical_velocity as f32);
            self.datarefs
                .ap_heading
                .set_f32(normalize_heading(data.ap.heading) as f32);
            self.datarefs.ap_airspeed.set_f32(data.ap.airspeed as f32);
            self.datarefs
                .ap_heading_roll_mode
                .set_i32(data.ap.heading_roll_mode);
            self.datarefs.ap_state.set_i32(data.ap.state);
            self.datarefs.ap_mode.set_i32(data.ap.mode);
        }
    }

    fn toggle_window(&mut self) {
        if self.window.is_null() {
            return;
        }
        // SAFETY: `self.window` is either null or the live handle created and
        // retained by this plugin until `stop`.
        unsafe {
            if XPLMGetWindowIsVisible(self.window) != 0 {
                XPLMSetWindowIsVisible(self.window, 0);
                if let Some(ui) = self.ui.as_mut() {
                    ui.hide();
                }
                XPLMTakeKeyboardFocus(ptr::null_mut());
            } else {
                XPLMSetWindowIsVisible(self.window, 1);
                XPLMBringWindowToFront(self.window);
            }
        }
    }
}

fn c_string(value: &str) -> CString {
    CString::new(value.replace('\0', " ")).unwrap()
}

fn log(message: &str) {
    let message = c_string(&format!("PositionAircraftNative: {message}\n"));
    // SAFETY: `message` is a live NUL-terminated string for the duration of the call.
    unsafe { XPLMDebugString(message.as_ptr()) }
}

unsafe fn write_plugin_string(destination: *mut c_char, value: &str) {
    if destination.is_null() {
        return;
    }
    let bytes = value.as_bytes();
    // SAFETY: the caller guarantees an SDK-sized writable output buffer. All
    // values passed here are short plugin metadata constants.
    unsafe {
        ptr::copy_nonoverlapping(bytes.as_ptr(), destination.cast::<u8>(), bytes.len());
        *destination.add(bytes.len()) = 0;
    }
}

fn system_path() -> PathBuf {
    let mut buffer = [0_i8; 1024];
    // SAFETY: the SDK accepts this fixed writable buffer and guarantees a
    // NUL-terminated path on return.
    let path = unsafe {
        XPLMGetSystemPath(buffer.as_mut_ptr());
        CStr::from_ptr(buffer.as_ptr())
            .to_string_lossy()
            .into_owned()
    };
    PathBuf::from(path)
}

fn execute_command(action: CommandAction) {
    with_state_mut(|state| match action {
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
    });
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
    with_state_mut(|state| {
        let Some(mut pending) = state.pending.take() else {
            return;
        };
        if pending.wait_frames > 0 {
            pending.wait_frames -= 1;
            state.pending = Some(pending);
            return;
        }
        state.apply_attitude_velocity_controls(&pending.data);
        pending.remaining_frames -= 1;
        if pending.remaining_frames > 0 {
            state.pending = Some(pending);
        }
    });
    -1.0
}

unsafe extern "C" fn menu_handler(_menu_ref: *mut c_void, _item_ref: *mut c_void) {}

fn create_window() -> Result<XPLMWindowID, String> {
    let mut screen_left = 0;
    let mut screen_top = 0;
    let mut screen_right = 0;
    let mut screen_bottom = 0;
    // SAFETY: all output pointers refer to live local variables.
    unsafe {
        XPLMGetScreenBoundsGlobal(
            &mut screen_left,
            &mut screen_top,
            &mut screen_right,
            &mut screen_bottom,
        );
    }
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
    // SAFETY: `params` has the SDK-prescribed size, live callback pointers, and
    // a null refcon. X-Plane copies the structure during this call.
    let window = unsafe { XPLMCreateWindowEx(&mut params) };
    if window.is_null() {
        return Err("XPLMCreateWindowEx failed".to_owned());
    }
    // SAFETY: `window` was just returned by XPLM and checked for null.
    unsafe { XPLMSetWindowResizingLimits(window, 660, 840, 1000, 1000) };
    let title = c_string("Position Aircraft - Native Rust");
    // SAFETY: the window handle is live and `title` is NUL-terminated.
    unsafe { XPLMSetWindowTitle(window, title.as_ptr()) };
    Ok(window)
}

fn register_commands(state: &mut PluginState) -> Result<(), String> {
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
        // SAFETY: both strings are NUL-terminated and live for the call.
        let command = unsafe { XPLMCreateCommand(name.as_ptr(), description.as_ptr()) };
        if command.is_null() {
            return Err(format!("Unable to create command {short_name}"));
        }
        // SAFETY: `command` is live, the callback has the required ABI, and the
        // integer-valued refcon is decoded without dereferencing it.
        unsafe {
            XPLMRegisterCommandHandler(
                command,
                Some(command_handler),
                1,
                action as usize as *mut c_void,
            );
        }
        state.commands.push(RegisteredCommand { command, action });
    }
    Ok(())
}

fn create_menu(state: &mut PluginState) {
    let menu_name = c_string("Position Aircraft Native");
    // SAFETY: menu handles are owned by X-Plane, the label is NUL-terminated,
    // and callbacks/refcons satisfy the XPLM ABI.
    unsafe {
        state.plugins_menu = XPLMFindPluginsMenu();
        state.plugins_menu_item =
            XPLMAppendMenuItem(state.plugins_menu, menu_name.as_ptr(), ptr::null_mut(), 0);
        state.menu = XPLMCreateMenu(
            menu_name.as_ptr(),
            state.plugins_menu,
            state.plugins_menu_item,
            Some(menu_handler),
            ptr::null_mut(),
        );
    }
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
            // SAFETY: the menu and command handles are live and the label is
            // NUL-terminated for the duration of the call.
            unsafe { XPLMAppendMenuItemWithCommand(state.menu, label.as_ptr(), command) };
        }
    }
}

pub(crate) unsafe fn start(
    out_name: *mut c_char,
    out_signature: *mut c_char,
    out_description: *mut c_char,
) -> c_int {
    // SAFETY: upheld by `XPluginStart`, which receives these buffers directly
    // from X-Plane's plugin manager.
    unsafe {
        write_plugin_string(out_name, "Position Aircraft Native");
        write_plugin_string(out_signature, "com.openai.position-aircraft-native-rust");
        write_plugin_string(
            out_description,
            "Native VR and joystick PositionAircraft replacement written in Rust",
        );
    }

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
        ui: Some(EguiIntegration::new()),
        datarefs,
        commands: Vec::new(),
        menu: ptr::null_mut(),
        plugins_menu: ptr::null_mut(),
        plugins_menu_item: -1,
        pending: None,
    };
    initial.refresh_pads();
    initial.capture_current();
    replace_state(Some(initial));

    let window = match create_window() {
        Ok(window) => window,
        Err(error) => {
            log(&error);
            replace_state(None);
            return 0;
        }
    };
    let setup_result = with_state_mut(|state| {
        state.window = window;
        // SAFETY: the dataref and window were both obtained from XPLM during
        // this startup sequence and remain live.
        unsafe {
            if state.datarefs.vr_enabled.get_i32() != 0 {
                XPLMSetWindowPositioningMode(window, XPLM_WINDOW_VR, -1);
            } else {
                XPLMSetWindowPositioningMode(window, XPLM_WINDOW_POSITION_FREE, -1);
            }
        }
        register_commands(state)?;
        create_menu(state);
        Ok(())
    })
    .unwrap_or_else(|| Err("Plugin state disappeared during startup".to_owned()));
    if let Err(error) = setup_result {
        log(&error);
        // SAFETY: `window` is the live handle created immediately above.
        unsafe { XPLMDestroyWindow(window) };
        replace_state(None);
        return 0;
    }
    // SAFETY: the callback has the XPLM ABI and uses no refcon.
    unsafe { XPLMRegisterFlightLoopCallback(Some(flight_loop), -1.0, ptr::null_mut()) };
    log("0.3.0 loaded (XPLM 4.3 native window, egui interface)");
    1
}

pub(crate) fn stop() {
    // SAFETY: this unregisters the exact callback/refcon pair registered by `start`.
    unsafe { XPLMUnregisterFlightLoopCallback(Some(flight_loop), ptr::null_mut()) };
    let Some(mut state) = replace_state(None) else {
        return;
    };
    for command in state.commands.drain(..) {
        // SAFETY: each tuple exactly matches a registration retained in state.
        unsafe {
            XPLMUnregisterCommandHandler(
                command.command,
                Some(command_handler),
                1,
                command.action as usize as *mut c_void,
            );
        }
    }
    if !state.menu.is_null() {
        // SAFETY: this menu was created by this plugin and has not been destroyed.
        unsafe { XPLMDestroyMenu(state.menu) };
    }
    if !state.plugins_menu.is_null() && state.plugins_menu_item >= 0 {
        // SAFETY: the parent menu and retained item index came from XPLM.
        unsafe { XPLMRemoveMenuItem(state.plugins_menu, state.plugins_menu_item) };
    }
    if !state.window.is_null() {
        if let Some(ui) = state.ui.as_mut() {
            ui.destroy_renderer();
        }
        // SAFETY: this window was created by this plugin and is destroyed once.
        unsafe { XPLMDestroyWindow(state.window) };
    }
    log("unloaded");
}

pub(crate) fn receive_message(from: XPLMPluginID, message: c_int) {
    if from != XPLM_PLUGIN_XPLANE {
        return;
    }
    with_state_mut(|state| {
        if state.window.is_null() {
            return;
        }
        // SAFETY: the retained window handle is live, and the screen-bound
        // outputs point to valid locals.
        unsafe {
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
    });
}
