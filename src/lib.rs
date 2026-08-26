#![allow(non_snake_case)]

use std::ffi::{c_char, c_float, c_int, c_void, CStr, CString};
use std::fs;
use std::io;
use std::mem;
use std::path::{Path, PathBuf};
use std::ptr;
use std::sync::{Mutex, MutexGuard, OnceLock};

type XPLMDataRef = *mut c_void;
type XPLMWindowID = *mut c_void;
type XPLMCommandRef = *mut c_void;
type XPLMMenuID = *mut c_void;
type XPLMPluginID = c_int;
type XPLMKeyFlags = c_int;
type XPLMMouseStatus = c_int;
type XPLMCursorStatus = c_int;

type DrawWindowCallback = unsafe extern "C" fn(XPLMWindowID, *mut c_void);
type KeyCallback =
    unsafe extern "C" fn(XPLMWindowID, c_char, XPLMKeyFlags, c_char, *mut c_void, c_int);
type MouseCallback =
    unsafe extern "C" fn(XPLMWindowID, c_int, c_int, XPLMMouseStatus, *mut c_void) -> c_int;
type CursorCallback =
    unsafe extern "C" fn(XPLMWindowID, c_int, c_int, *mut c_void) -> XPLMCursorStatus;
type WheelCallback =
    unsafe extern "C" fn(XPLMWindowID, c_int, c_int, c_int, c_int, *mut c_void) -> c_int;
type CommandCallback = unsafe extern "C" fn(XPLMCommandRef, c_int, *mut c_void) -> c_int;
type FlightLoopCallback = unsafe extern "C" fn(c_float, c_float, c_int, *mut c_void) -> c_float;
type MenuCallback = unsafe extern "C" fn(*mut c_void, *mut c_void);

#[repr(C)]
struct XPLMCreateWindowT {
    struct_size: c_int,
    left: c_int,
    top: c_int,
    right: c_int,
    bottom: c_int,
    visible: c_int,
    draw_window_func: Option<DrawWindowCallback>,
    handle_mouse_click_func: Option<MouseCallback>,
    handle_key_func: Option<KeyCallback>,
    handle_cursor_func: Option<CursorCallback>,
    handle_mouse_wheel_func: Option<WheelCallback>,
    refcon: *mut c_void,
    decorate_as_floating_window: c_int,
    layer: c_int,
    handle_right_click_func: Option<MouseCallback>,
}

#[link(name = "XPLM_64")]
extern "C" {
    fn XPLMDebugString(message: *const c_char);
    fn XPLMGetSystemPath(path: *mut c_char);

    fn XPLMFindDataRef(name: *const c_char) -> XPLMDataRef;
    fn XPLMGetDatai(data_ref: XPLMDataRef) -> c_int;
    fn XPLMSetDatai(data_ref: XPLMDataRef, value: c_int);
    fn XPLMGetDataf(data_ref: XPLMDataRef) -> c_float;
    fn XPLMSetDataf(data_ref: XPLMDataRef, value: c_float);
    fn XPLMGetDatad(data_ref: XPLMDataRef) -> f64;
    fn XPLMSetDatad(data_ref: XPLMDataRef, value: f64);
    fn XPLMGetDatavf(
        data_ref: XPLMDataRef,
        values: *mut c_float,
        offset: c_int,
        max: c_int,
    ) -> c_int;
    fn XPLMSetDatavf(data_ref: XPLMDataRef, values: *const c_float, offset: c_int, count: c_int);

    fn XPLMWorldToLocal(
        latitude: f64,
        longitude: f64,
        altitude_m: f64,
        out_x: *mut f64,
        out_y: *mut f64,
        out_z: *mut f64,
    );

    fn XPLMCreateWindowEx(params: *mut XPLMCreateWindowT) -> XPLMWindowID;
    fn XPLMDestroyWindow(window: XPLMWindowID);
    fn XPLMGetScreenBoundsGlobal(
        left: *mut c_int,
        top: *mut c_int,
        right: *mut c_int,
        bottom: *mut c_int,
    );
    fn XPLMGetWindowGeometry(
        window: XPLMWindowID,
        left: *mut c_int,
        top: *mut c_int,
        right: *mut c_int,
        bottom: *mut c_int,
    );
    fn XPLMSetWindowGeometry(
        window: XPLMWindowID,
        left: c_int,
        top: c_int,
        right: c_int,
        bottom: c_int,
    );
    fn XPLMSetWindowIsVisible(window: XPLMWindowID, visible: c_int);
    fn XPLMGetWindowIsVisible(window: XPLMWindowID) -> c_int;
    fn XPLMBringWindowToFront(window: XPLMWindowID);
    fn XPLMSetWindowPositioningMode(window: XPLMWindowID, mode: c_int, monitor: c_int);
    fn XPLMSetWindowResizingLimits(
        window: XPLMWindowID,
        min_width: c_int,
        min_height: c_int,
        max_width: c_int,
        max_height: c_int,
    );
    fn XPLMSetWindowTitle(window: XPLMWindowID, title: *const c_char);
    fn XPLMTakeKeyboardFocus(window: XPLMWindowID);

    fn XPLMDrawString(
        color: *mut c_float,
        x: c_int,
        y: c_int,
        text: *mut c_char,
        word_wrap_width: *mut c_int,
        font_id: c_int,
    );
    fn XPLMMeasureString(font_id: c_int, text: *const c_char, character_count: c_int) -> c_float;
    fn XPLMSetGraphicsState(
        enable_fog: c_int,
        texture_units: c_int,
        enable_lighting: c_int,
        enable_alpha_testing: c_int,
        enable_alpha_blending: c_int,
        enable_depth_testing: c_int,
        enable_depth_writing: c_int,
    );

    fn XPLMCreateCommand(name: *const c_char, description: *const c_char) -> XPLMCommandRef;
    fn XPLMRegisterCommandHandler(
        command: XPLMCommandRef,
        handler: Option<CommandCallback>,
        before: c_int,
        refcon: *mut c_void,
    );
    fn XPLMUnregisterCommandHandler(
        command: XPLMCommandRef,
        handler: Option<CommandCallback>,
        before: c_int,
        refcon: *mut c_void,
    );

    fn XPLMRegisterFlightLoopCallback(
        callback: Option<FlightLoopCallback>,
        interval: c_float,
        refcon: *mut c_void,
    );
    fn XPLMUnregisterFlightLoopCallback(callback: Option<FlightLoopCallback>, refcon: *mut c_void);

    fn XPLMFindPluginsMenu() -> XPLMMenuID;
    fn XPLMAppendMenuItem(
        menu: XPLMMenuID,
        name: *const c_char,
        item_ref: *mut c_void,
        deprecated_and_ignored: c_int,
    ) -> c_int;
    fn XPLMCreateMenu(
        name: *const c_char,
        parent_menu: XPLMMenuID,
        parent_item: c_int,
        handler: Option<MenuCallback>,
        menu_ref: *mut c_void,
    ) -> XPLMMenuID;
    fn XPLMAppendMenuItemWithCommand(
        menu: XPLMMenuID,
        name: *const c_char,
        command: XPLMCommandRef,
    ) -> c_int;
    fn XPLMDestroyMenu(menu: XPLMMenuID);
    fn XPLMRemoveMenuItem(menu: XPLMMenuID, index: c_int);
}

#[link(name = "OpenGL32")]
extern "system" {
    fn glBegin(mode: u32);
    fn glEnd();
    fn glColor4f(red: f32, green: f32, blue: f32, alpha: f32);
    fn glLineWidth(width: f32);
    fn glVertex2i(x: c_int, y: c_int);
}

const WINDOW_WIDTH: i32 = 720;
const WINDOW_HEIGHT: i32 = 650;
const XPLM_WINDOW_POSITION_FREE: i32 = 0;
const XPLM_WINDOW_VR: i32 = 5;
const XPLM_WINDOW_DECORATION_ROUND_RECTANGLE: i32 = 1;
const XPLM_WINDOW_LAYER_FLOATING: i32 = 1;
const XPLM_MOUSE_DOWN: i32 = 1;
const XPLM_MOUSE_DRAG: i32 = 2;
const XPLM_MOUSE_UP: i32 = 3;
const XPLM_COMMAND_BEGIN: i32 = 0;
const XPLM_DOWN_FLAG: i32 = 8;
const XPLM_MSG_ENTERED_VR: i32 = 109;
const XPLM_MSG_EXITING_VR: i32 = 110;
const XPLM_PLUGIN_XPLANE: i32 = 0;
const XPLM_CURSOR_DEFAULT: i32 = 0;
const XPLM_CURSOR_ARROW: i32 = 2;
const GL_LINES: u32 = 0x0001;
const GL_LINE_LOOP: u32 = 0x0002;
const GL_QUADS: u32 = 0x0007;
const METERS_TO_FEET: f64 = 3.280_839_895_013_1;
const KNOTS_TO_MPS: f64 = 0.514_444_444_444_44;

const UI_MARGIN: i32 = 14;
const UI_GAP: i32 = 8;
const ACTION_Y: i32 = 44;
const PAD_Y: i32 = 112;
const POSITION_Y: i32 = 190;
const AP_TOGGLE_Y: i32 = 374;
const AP_FIELDS_Y: i32 = 416;
const SAVE_Y: i32 = 568;
const DROPDOWN_ROWS: usize = 8;
const DROPDOWN_ROW_HEIGHT: i32 = 28;
const DROPDOWN_SCROLLBAR_WIDTH: i32 = 22;

const COLOR_CANVAS: [f32; 4] = [0.025, 0.040, 0.055, 0.96];
const COLOR_PANEL: [f32; 4] = [0.055, 0.080, 0.105, 0.98];
const COLOR_FIELD: [f32; 4] = [0.018, 0.030, 0.040, 0.98];
const COLOR_FIELD_HOVER: [f32; 4] = [0.055, 0.115, 0.145, 0.98];
const COLOR_BUTTON: [f32; 4] = [0.075, 0.235, 0.325, 0.98];
const COLOR_BUTTON_HOVER: [f32; 4] = [0.105, 0.385, 0.510, 0.98];
const COLOR_PRIMARY: [f32; 4] = [0.780, 0.390, 0.065, 0.98];
const COLOR_PRIMARY_HOVER: [f32; 4] = [1.000, 0.570, 0.100, 0.98];
const COLOR_SELECTED: [f32; 4] = [0.060, 0.290, 0.390, 0.98];
const COLOR_BORDER: [f32; 4] = [0.210, 0.390, 0.485, 1.0];
const COLOR_FOCUS: [f32; 4] = [0.120, 0.820, 1.000, 1.0];
const COLOR_TEXT: [f32; 3] = [0.920, 0.960, 0.980];
const COLOR_MUTED: [f32; 3] = [0.520, 0.670, 0.740];
const COLOR_AMBER_TEXT: [f32; 3] = [1.000, 0.770, 0.350];

static STATE: OnceLock<Mutex<Option<PluginState>>> = OnceLock::new();

fn state_lock() -> MutexGuard<'static, Option<PluginState>> {
    STATE
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[derive(Clone, Debug, Default)]
struct AutopilotData {
    mode: i32,
    altitude: f64,
    vertical_velocity: f64,
    heading: f64,
    airspeed: f64,
    state: i32,
    heading_roll_mode: i32,
}

#[derive(Clone, Debug)]
struct PadData {
    latitude: f64,
    longitude: f64,
    altitude: f64,
    heading: f64,
    pitch: f64,
    roll: f64,
    speed: f64,
    throttle: f64,
    flaps: f64,
    gear: i32,
    use_ap: bool,
    ap: AutopilotData,
}

impl Default for PadData {
    fn default() -> Self {
        Self {
            latitude: 0.0,
            longitude: 0.0,
            altitude: 0.0,
            heading: 0.0,
            pitch: 0.0,
            roll: 0.0,
            speed: 0.0,
            throttle: 0.0,
            flaps: 0.0,
            gear: 0,
            use_ap: false,
            ap: AutopilotData::default(),
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(usize)]
enum Field {
    Latitude = 0,
    Longitude,
    Altitude,
    Heading,
    Pitch,
    Roll,
    Speed,
    Throttle,
    Flaps,
    Gear,
    ApMode,
    ApAltitude,
    ApVerticalVelocity,
    ApHeading,
    ApAirspeed,
    ApState,
    ApHeadingRollMode,
    SaveName,
}

const FIELD_COUNT: usize = 18;

#[derive(Clone)]
struct Form {
    values: [String; FIELD_COUNT],
    use_ap: bool,
}

impl Form {
    fn from_data(data: &PadData, save_name: &str) -> Self {
        let mut values: [String; FIELD_COUNT] = std::array::from_fn(|_| String::new());
        values[Field::Latitude as usize] = format!("{:.6}", data.latitude);
        values[Field::Longitude as usize] = format!("{:.6}", data.longitude);
        values[Field::Altitude as usize] = format!("{:.2}", data.altitude);
        values[Field::Heading as usize] = format!("{:.2}", data.heading);
        values[Field::Pitch as usize] = format!("{:.2}", data.pitch);
        values[Field::Roll as usize] = format!("{:.2}", data.roll);
        values[Field::Speed as usize] = format!("{:.2}", data.speed);
        values[Field::Throttle as usize] = format!("{:.4}", data.throttle);
        values[Field::Flaps as usize] = format!("{:.4}", data.flaps);
        values[Field::Gear as usize] = data.gear.to_string();
        values[Field::ApMode as usize] = data.ap.mode.to_string();
        values[Field::ApAltitude as usize] = format!("{:.2}", data.ap.altitude);
        values[Field::ApVerticalVelocity as usize] = format!("{:.2}", data.ap.vertical_velocity);
        values[Field::ApHeading as usize] = format!("{:.2}", data.ap.heading);
        values[Field::ApAirspeed as usize] = format!("{:.2}", data.ap.airspeed);
        values[Field::ApState as usize] = data.ap.state.to_string();
        values[Field::ApHeadingRollMode as usize] = data.ap.heading_roll_mode.to_string();
        values[Field::SaveName as usize] = save_name.to_owned();
        Self {
            values,
            use_ap: data.use_ap,
        }
    }

    fn value(&self, field: Field) -> &str {
        &self.values[field as usize]
    }

    fn value_mut(&mut self, field: Field) -> &mut String {
        &mut self.values[field as usize]
    }

    fn parse_number(&self, field: Field, label: &str) -> Result<f64, String> {
        self.value(field)
            .trim()
            .parse::<f64>()
            .map_err(|_| format!("{label} is not a valid number"))
    }

    fn to_data(&self) -> Result<PadData, String> {
        let data = PadData {
            latitude: self.parse_number(Field::Latitude, "Latitude")?,
            longitude: self.parse_number(Field::Longitude, "Longitude")?,
            altitude: self.parse_number(Field::Altitude, "Altitude")?,
            heading: normalize_heading(self.parse_number(Field::Heading, "Heading")?),
            pitch: self.parse_number(Field::Pitch, "Pitch")?,
            roll: self.parse_number(Field::Roll, "Roll")?,
            speed: self.parse_number(Field::Speed, "Speed")?,
            throttle: self.parse_number(Field::Throttle, "Throttle")?,
            flaps: self.parse_number(Field::Flaps, "Flaps")?,
            gear: if self.parse_number(Field::Gear, "Gear")? >= 0.5 {
                1
            } else {
                0
            },
            use_ap: self.use_ap,
            ap: AutopilotData {
                mode: self.parse_number(Field::ApMode, "AP mode")?.round() as i32,
                altitude: self.parse_number(Field::ApAltitude, "AP altitude")?,
                vertical_velocity: self
                    .parse_number(Field::ApVerticalVelocity, "AP vertical velocity")?,
                heading: normalize_heading(self.parse_number(Field::ApHeading, "AP heading")?),
                airspeed: self.parse_number(Field::ApAirspeed, "AP airspeed")?,
                state: self.parse_number(Field::ApState, "AP state")?.round() as i32,
                heading_roll_mode: self
                    .parse_number(Field::ApHeadingRollMode, "AP heading/roll mode")?
                    .round() as i32,
            },
        };
        validate_data(&data)?;
        Ok(data)
    }
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

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum UiAction {
    Command(CommandAction),
    LoadSelected(bool),
    ToggleAp,
    SaveNamed,
    Refresh,
    ToggleDropdown,
    CloseDropdown,
    ScrollDropdown(isize),
    SelectPad(usize),
    Edit(Field),
}

#[derive(Copy, Clone)]
struct Rect {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

impl Rect {
    fn contains(self, x: i32, y: i32) -> bool {
        x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height
    }
}

#[derive(Copy, Clone)]
struct PadLayout {
    previous: Rect,
    selector: Rect,
    next: Rect,
    refresh: Rect,
    load: Rect,
    load_and_position: Rect,
}

fn command_button_rects(width: i32) -> [(Rect, CommandAction); 4] {
    let button_width = (width - UI_MARGIN * 2 - UI_GAP * 3) / 4;
    let actions = [
        CommandAction::CaptureCurrent,
        CommandAction::PositionLoaded,
        CommandAction::QuickSave,
        CommandAction::QuickLoadAndPosition,
    ];
    std::array::from_fn(|index| {
        (
            Rect {
                x: UI_MARGIN + index as i32 * (button_width + UI_GAP),
                y: ACTION_Y,
                width: button_width,
                height: 40,
            },
            actions[index],
        )
    })
}

fn pad_layout(width: i32) -> PadLayout {
    let previous_width = 42;
    let next_width = 42;
    let refresh_width = 68;
    let load_width = 72;
    let load_and_position_width = 126;
    let selector_width = width
        - UI_MARGIN * 2
        - UI_GAP * 5
        - previous_width
        - next_width
        - refresh_width
        - load_width
        - load_and_position_width;
    let mut x = UI_MARGIN;
    let mut next_rect = |rect_width| {
        let rect = Rect {
            x,
            y: PAD_Y,
            width: rect_width,
            height: 36,
        };
        x += rect_width + UI_GAP;
        rect
    };
    PadLayout {
        previous: next_rect(previous_width),
        selector: next_rect(selector_width),
        next: next_rect(next_width),
        refresh: next_rect(refresh_width),
        load: next_rect(load_width),
        load_and_position: next_rect(load_and_position_width),
    }
}

fn dropdown_list_rect(selector: Rect, row_count: usize) -> Rect {
    Rect {
        x: selector.x,
        y: PAD_Y + selector.height + 2,
        width: selector.width,
        height: row_count.max(1) as i32 * DROPDOWN_ROW_HEIGHT,
    }
}

fn dropdown_scrollbar_rects(list: Rect) -> (Rect, Rect, Rect) {
    let column_x = list.x + list.width - DROPDOWN_SCROLLBAR_WIDTH;
    let up = Rect {
        x: column_x,
        y: list.y,
        width: DROPDOWN_SCROLLBAR_WIDTH,
        height: DROPDOWN_ROW_HEIGHT,
    };
    let down = Rect {
        x: column_x,
        y: list.y + list.height - DROPDOWN_ROW_HEIGHT,
        width: DROPDOWN_SCROLLBAR_WIDTH,
        height: DROPDOWN_ROW_HEIGHT,
    };
    let track = Rect {
        x: column_x,
        y: up.y + up.height,
        width: DROPDOWN_SCROLLBAR_WIDTH,
        height: (list.height - up.height - down.height).max(0),
    };
    (up, track, down)
}

fn dropdown_thumb_rect(track: Rect, scroll: usize, total_rows: usize) -> Rect {
    if track.height <= 0 {
        return track;
    }
    let max_scroll = total_rows.saturating_sub(DROPDOWN_ROWS);
    let thumb_height = if total_rows <= DROPDOWN_ROWS {
        track.height
    } else {
        (track.height * DROPDOWN_ROWS as i32 / total_rows as i32).clamp(18, track.height)
    };
    let travel = track.height - thumb_height;
    let offset = if max_scroll == 0 {
        0
    } else {
        travel * scroll.min(max_scroll) as i32 / max_scroll as i32
    };
    Rect {
        x: track.x + 5,
        y: track.y + offset,
        width: track.width - 10,
        height: thumb_height,
    }
}

fn field_rect(width: i32, top: i32, index: usize) -> Rect {
    let column_width = (width - UI_MARGIN * 2 - UI_GAP) / 2;
    Rect {
        x: UI_MARGIN + (index as i32 % 2) * (column_width + UI_GAP),
        y: top + (index as i32 / 2) * 34,
        width: column_width,
        height: 30,
    }
}

fn position_fields() -> [(Field, &'static str); 10] {
    [
        (Field::Latitude, "Latitude"),
        (Field::Longitude, "Longitude"),
        (Field::Altitude, "Altitude / ft"),
        (Field::Heading, "Heading / mag"),
        (Field::Pitch, "Pitch / deg"),
        (Field::Roll, "Roll / deg"),
        (Field::Speed, "Speed / KIAS"),
        (Field::Throttle, "Throttle / 0..1"),
        (Field::Flaps, "Flaps / 0..1"),
        (Field::Gear, "Gear / 0 or 1"),
    ]
}

fn autopilot_fields() -> [(Field, &'static str); 7] {
    [
        (Field::ApMode, "AP mode"),
        (Field::ApState, "AP state flags"),
        (Field::ApAltitude, "AP altitude / ft"),
        (Field::ApVerticalVelocity, "AP vertical / fpm"),
        (Field::ApHeading, "AP heading / mag"),
        (Field::ApAirspeed, "AP airspeed / kt"),
        (Field::ApHeadingRollMode, "AP bank limit mode"),
    ]
}

fn save_layout(width: i32) -> (Rect, Rect) {
    let button_width = 120;
    (
        Rect {
            x: UI_MARGIN,
            y: SAVE_Y,
            width: width - UI_MARGIN * 2 - UI_GAP - button_width,
            height: 36,
        },
        Rect {
            x: width - UI_MARGIN - button_width,
            y: SAVE_Y,
            width: button_width,
            height: 36,
        },
    )
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

    fn hit_test(&self, local_x: i32, local_y: i32, width: i32) -> Option<UiAction> {
        let pad = pad_layout(width);
        if self.dropdown_open {
            if pad.selector.contains(local_x, local_y) {
                return Some(UiAction::ToggleDropdown);
            }
            let row_top = PAD_Y + pad.selector.height + 2;
            let visible_rows = self
                .pads
                .len()
                .saturating_sub(self.dropdown_scroll)
                .min(DROPDOWN_ROWS);
            let list = dropdown_list_rect(pad.selector, visible_rows);
            if self.dropdown_max_scroll() > 0 {
                let (up, track, down) = dropdown_scrollbar_rects(list);
                if up.contains(local_x, local_y) {
                    return Some(UiAction::ScrollDropdown(-1));
                }
                if down.contains(local_x, local_y) {
                    return Some(UiAction::ScrollDropdown(1));
                }
                if track.contains(local_x, local_y) {
                    let thumb = dropdown_thumb_rect(track, self.dropdown_scroll, self.pads.len());
                    let page = DROPDOWN_ROWS.saturating_sub(1) as isize;
                    return Some(if local_y < thumb.y {
                        UiAction::ScrollDropdown(-page)
                    } else if local_y >= thumb.y + thumb.height {
                        UiAction::ScrollDropdown(page)
                    } else {
                        UiAction::ScrollDropdown(0)
                    });
                }
            }
            for row in 0..visible_rows {
                let rect = Rect {
                    x: pad.selector.x,
                    y: row_top + row as i32 * DROPDOWN_ROW_HEIGHT,
                    width: pad.selector.width - DROPDOWN_SCROLLBAR_WIDTH,
                    height: DROPDOWN_ROW_HEIGHT,
                };
                if rect.contains(local_x, local_y) {
                    return Some(UiAction::SelectPad(self.dropdown_scroll + row));
                }
            }
            return Some(UiAction::CloseDropdown);
        }

        for (rect, action) in command_button_rects(width) {
            if rect.contains(local_x, local_y) {
                return Some(UiAction::Command(action));
            }
        }

        let file_actions = [
            (pad.previous, UiAction::Command(CommandAction::PreviousPad)),
            (pad.selector, UiAction::ToggleDropdown),
            (pad.next, UiAction::Command(CommandAction::NextPad)),
            (pad.refresh, UiAction::Refresh),
            (pad.load, UiAction::LoadSelected(false)),
            (pad.load_and_position, UiAction::LoadSelected(true)),
        ];
        for (rect, action) in file_actions {
            if rect.contains(local_x, local_y) {
                return Some(action);
            }
        }

        for (index, (field, _)) in position_fields().into_iter().enumerate() {
            let rect = field_rect(width, POSITION_Y, index);
            if rect.contains(local_x, local_y) {
                return Some(UiAction::Edit(field));
            }
        }

        let ap_toggle = Rect {
            x: UI_MARGIN,
            y: AP_TOGGLE_Y,
            width: width - UI_MARGIN * 2,
            height: 32,
        };
        if ap_toggle.contains(local_x, local_y) {
            return Some(UiAction::ToggleAp);
        }

        for (index, (field, _)) in autopilot_fields().into_iter().enumerate() {
            let rect = field_rect(width, AP_FIELDS_Y, index);
            if rect.contains(local_x, local_y) {
                return Some(UiAction::Edit(field));
            }
        }

        let (save_field, save_button) = save_layout(width);
        if save_field.contains(local_x, local_y) {
            return Some(UiAction::Edit(Field::SaveName));
        }
        if save_button.contains(local_x, local_y) {
            return Some(UiAction::SaveNamed);
        }
        None
    }
}

fn normalize_heading(value: f64) -> f64 {
    value.rem_euclid(360.0)
}

fn validate_data(data: &PadData) -> Result<(), String> {
    if !(-90.0..=90.0).contains(&data.latitude) {
        return Err("Latitude must be between -90 and 90".to_owned());
    }
    if !(-180.0..=180.0).contains(&data.longitude) {
        return Err("Longitude must be between -180 and 180".to_owned());
    }
    if !(-90.0..=90.0).contains(&data.pitch) {
        return Err("Pitch must be between -90 and 90".to_owned());
    }
    if !(-180.0..=180.0).contains(&data.roll) {
        return Err("Roll must be between -180 and 180".to_owned());
    }
    if !(0.0..=5000.0).contains(&data.speed) {
        return Err("Speed must be between 0 and 5000 knots".to_owned());
    }
    if !(0.0..=1.0).contains(&data.throttle) {
        return Err("Throttle must be between 0 and 1".to_owned());
    }
    if !(0.0..=1.0).contains(&data.flaps) {
        return Err("Flaps must be between 0 and 1".to_owned());
    }
    Ok(())
}

fn parse_pad(path: &Path) -> Result<PadData, String> {
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("Unable to open PAD {}: {error}", path.display()))?;
    parse_pad_text(&contents)
}

fn parse_pad_text(contents: &str) -> Result<PadData, String> {
    use std::collections::HashMap;
    let mut values = HashMap::<String, String>::new();
    let mut section = String::new();
    for raw_line in contents.lines() {
        let line = raw_line.trim();
        if line.starts_with('[') && line.ends_with(']') {
            section = line[1..line.len() - 1].trim().to_ascii_lowercase();
            continue;
        }
        if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            values.insert(
                format!("{}.{}", section, key.trim().to_ascii_lowercase()),
                value.trim().to_owned(),
            );
        }
    }

    fn number(
        values: &std::collections::HashMap<String, String>,
        section: &str,
        key: &str,
        required: bool,
        default: f64,
    ) -> Result<f64, String> {
        let lookup = format!(
            "{}.{}",
            section.to_ascii_lowercase(),
            key.to_ascii_lowercase()
        );
        match values.get(&lookup) {
            Some(raw) if !raw.is_empty() => raw
                .parse::<f64>()
                .map_err(|_| format!("Invalid number for {key}")),
            _ if required => Err(format!("Missing {key}")),
            _ => Ok(default),
        }
    }

    let data = PadData {
        latitude: number(&values, "position_data", "latitude", true, 0.0)?,
        longitude: number(&values, "position_data", "longitude", true, 0.0)?,
        altitude: number(&values, "position_data", "altitude", true, 0.0)?,
        heading: normalize_heading(number(&values, "position_data", "heading", true, 0.0)?),
        pitch: number(&values, "position_data", "pitch", true, 0.0)?,
        roll: number(&values, "position_data", "roll", true, 0.0)?,
        speed: number(&values, "position_data", "speed", true, 0.0)?,
        throttle: number(&values, "position_data", "throttle", true, 0.0)?,
        flaps: number(&values, "position_data", "flaps", true, 0.0)?,
        gear: if number(&values, "position_data", "gear", true, 0.0)? >= 0.5 {
            1
        } else {
            0
        },
        use_ap: number(&values, "config", "use_autopilot_data", false, 0.0)? != 0.0,
        ap: AutopilotData {
            mode: number(&values, "autopilot_data", "autopilot_mode", false, 0.0)?.round() as i32,
            altitude: number(&values, "autopilot_data", "autopilot_altitude", false, 0.0)?,
            vertical_velocity: number(
                &values,
                "autopilot_data",
                "autopilot_vertical_velocity",
                false,
                0.0,
            )?,
            heading: normalize_heading(number(
                &values,
                "autopilot_data",
                "autopilot_heading",
                false,
                0.0,
            )?),
            airspeed: number(&values, "autopilot_data", "autopilot_airspeed", false, 0.0)?,
            state: number(&values, "autopilot_data", "autopilot_state", false, 0.0)?.round() as i32,
            heading_roll_mode: number(
                &values,
                "autopilot_data",
                "autopilot_heading_roll_mode",
                false,
                0.0,
            )?
            .round() as i32,
        },
    };
    validate_data(&data)?;
    Ok(data)
}

fn write_pad(path: &Path, data: &PadData) -> io::Result<()> {
    let text = format!(
        "[CONFIG]\n\
Use_AutoPilot_Data = {}\n\n\
[POSITION_DATA]\n\
Latitude = {:.6}\n\
Longitude = {:.6}\n\
Altitude = {:.6}\n\
Heading = {:.6}\n\
Pitch = {:.6}\n\
Roll = {:.6}\n\
Speed = {:.6}\n\
Throttle = {:.6}\n\
Flaps = {:.6}\n\
Gear = {}\n\n\
[AUTOPILOT_DATA]\n\
AutoPilot_Mode = {}\n\
AutoPilot_Altitude = {:.6}\n\
AutoPilot_Vertical_Velocity = {:.6}\n\
AutoPilot_Heading = {:.6}\n\
AutoPilot_Airspeed = {:.6}\n\
AutoPilot_State = {}\n\
AutoPilot_Heading_Roll_Mode = {}\n",
        if data.use_ap { 1 } else { 0 },
        data.latitude,
        data.longitude,
        data.altitude,
        normalize_heading(data.heading),
        data.pitch,
        data.roll,
        data.speed,
        data.throttle,
        data.flaps,
        if data.gear != 0 { 1 } else { 0 },
        data.ap.mode,
        data.ap.altitude,
        data.ap.vertical_velocity,
        normalize_heading(data.ap.heading),
        data.ap.airspeed,
        data.ap.state,
        data.ap.heading_roll_mode,
    );
    fs::write(path, text)
}

fn safe_pad_filename(name: &str) -> Option<String> {
    let mut output = String::new();
    for character in name.trim().chars() {
        if "\\/:*?\"<>|".contains(character) || character.is_control() {
            output.push('_');
        } else if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.' | ' ') {
            output.push(character);
        } else {
            output.push('_');
        }
    }
    while output.contains("..") {
        output = output.replace("..", "_");
    }
    while output.ends_with('.') || output.ends_with(' ') {
        output.pop();
    }
    if output.to_ascii_lowercase().ends_with(".pad") {
        output.truncate(output.len() - 4);
    }
    if output.is_empty() {
        None
    } else {
        Some(format!("{output}.pad"))
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

unsafe fn draw_text(x: i32, y: i32, text: &str, color: [f32; 3]) {
    let mut color = color;
    let mut text = c_string(text).into_bytes_with_nul();
    XPLMDrawString(
        color.as_mut_ptr(),
        x,
        y,
        text.as_mut_ptr().cast(),
        ptr::null_mut(),
        0,
    );
}

fn global_rect(left: i32, top: i32, rect: Rect) -> (i32, i32, i32, i32) {
    let box_left = left + rect.x;
    let box_top = top - rect.y;
    (
        box_left,
        box_top,
        box_left + rect.width,
        box_top - rect.height,
    )
}

unsafe fn prepare_flat_drawing() {
    XPLMSetGraphicsState(0, 0, 0, 0, 1, 0, 0);
}

unsafe fn draw_rect(left: i32, top: i32, rect: Rect, color: [f32; 4]) {
    let (box_left, box_top, box_right, box_bottom) = global_rect(left, top, rect);
    prepare_flat_drawing();
    glColor4f(color[0], color[1], color[2], color[3]);
    glBegin(GL_QUADS);
    glVertex2i(box_left, box_top);
    glVertex2i(box_right, box_top);
    glVertex2i(box_right, box_bottom);
    glVertex2i(box_left, box_bottom);
    glEnd();
}

unsafe fn draw_outline(left: i32, top: i32, rect: Rect, color: [f32; 4], width: f32) {
    let (box_left, box_top, box_right, box_bottom) = global_rect(left, top, rect);
    prepare_flat_drawing();
    glLineWidth(width);
    glColor4f(color[0], color[1], color[2], color[3]);
    glBegin(GL_LINE_LOOP);
    glVertex2i(box_left, box_top);
    glVertex2i(box_right, box_top);
    glVertex2i(box_right, box_bottom);
    glVertex2i(box_left, box_bottom);
    glEnd();
    glLineWidth(1.0);
}

unsafe fn draw_rule(x1: i32, y1: i32, x2: i32, y2: i32, color: [f32; 4]) {
    prepare_flat_drawing();
    glColor4f(color[0], color[1], color[2], color[3]);
    glBegin(GL_LINES);
    glVertex2i(x1, y1);
    glVertex2i(x2, y2);
    glEnd();
}

unsafe fn text_width(text: &str) -> f32 {
    let text = c_string(text);
    XPLMMeasureString(0, text.as_ptr(), text.as_bytes().len() as i32)
}

unsafe fn truncate_text(text: &str, max_width: f32) -> String {
    if text_width(text) <= max_width {
        return text.to_owned();
    }
    let mut characters: Vec<char> = text.chars().collect();
    while !characters.is_empty() {
        characters.pop();
        let candidate = format!("{}...", characters.iter().collect::<String>());
        if text_width(&candidate) <= max_width {
            return candidate;
        }
    }
    "...".to_owned()
}

unsafe fn draw_centered_text(left: i32, top: i32, rect: Rect, label: &str, color: [f32; 3]) {
    let label = truncate_text(label, (rect.width - 12).max(12) as f32);
    let width = text_width(&label) as i32;
    draw_text(
        left + rect.x + (rect.width - width) / 2,
        top - rect.y - rect.height / 2 - 5,
        &label,
        color,
    );
}

#[derive(Copy, Clone)]
enum ButtonTone {
    Standard,
    Primary,
    Quiet,
}

struct ButtonSpec<'a> {
    rect: Rect,
    label: &'a str,
    action: UiAction,
    tone: ButtonTone,
    selected: bool,
}

unsafe fn draw_button(state: &PluginState, left: i32, top: i32, spec: ButtonSpec<'_>) {
    let ButtonSpec {
        rect,
        label,
        action,
        tone,
        selected,
    } = spec;
    let hovered = state.hovered_action == Some(action);
    let fill = if selected {
        COLOR_SELECTED
    } else {
        match (tone, hovered) {
            (ButtonTone::Primary, true) => COLOR_PRIMARY_HOVER,
            (ButtonTone::Primary, false) => COLOR_PRIMARY,
            (ButtonTone::Standard, true) => COLOR_BUTTON_HOVER,
            (ButtonTone::Standard, false) => COLOR_BUTTON,
            (ButtonTone::Quiet, true) => COLOR_FIELD_HOVER,
            (ButtonTone::Quiet, false) => COLOR_PANEL,
        }
    };
    draw_rect(left, top, rect, fill);
    draw_outline(
        left,
        top,
        rect,
        if hovered || selected {
            COLOR_FOCUS
        } else {
            COLOR_BORDER
        },
        if hovered { 2.0 } else { 1.0 },
    );
    if matches!(tone, ButtonTone::Primary) {
        draw_rect(
            left,
            top,
            Rect {
                x: rect.x,
                y: rect.y,
                width: 4,
                height: rect.height,
            },
            [
                COLOR_AMBER_TEXT[0],
                COLOR_AMBER_TEXT[1],
                COLOR_AMBER_TEXT[2],
                1.0,
            ],
        );
    }
    draw_centered_text(left, top, rect, label, COLOR_TEXT);
}

unsafe fn draw_field(
    state: &PluginState,
    left: i32,
    top: i32,
    rect: Rect,
    field: Field,
    label: &str,
    emphasized: bool,
) {
    let action = UiAction::Edit(field);
    let hovered = state.hovered_action == Some(action);
    let active = state.active_field == Some(field);
    draw_rect(
        left,
        top,
        rect,
        if hovered || active {
            COLOR_FIELD_HOVER
        } else {
            COLOR_FIELD
        },
    );
    draw_outline(
        left,
        top,
        rect,
        if active || hovered {
            COLOR_FOCUS
        } else {
            COLOR_BORDER
        },
        if active { 2.0 } else { 1.0 },
    );
    draw_rect(
        left,
        top,
        Rect {
            x: rect.x,
            y: rect.y,
            width: if active { 4 } else { 2 },
            height: rect.height,
        },
        if active { COLOR_FOCUS } else { COLOR_BORDER },
    );
    let label_color = if emphasized {
        COLOR_MUTED
    } else {
        [0.38, 0.48, 0.53]
    };
    draw_text(
        left + rect.x + 9,
        top - rect.y - rect.height / 2 - 5,
        label,
        label_color,
    );
    let raw_value = if active {
        format!("{} |", state.form.value(field))
    } else {
        state.form.value(field).to_owned()
    };
    let label_width = text_width(label) as i32;
    let available = (rect.width - label_width - 30).max(30) as f32;
    let value = truncate_text(&raw_value, available);
    let value_width = text_width(&value) as i32;
    draw_text(
        left + rect.x + rect.width - value_width - 9,
        top - rect.y - rect.height / 2 - 5,
        &value,
        if active {
            [0.55, 0.92, 1.0]
        } else {
            COLOR_TEXT
        },
    );
}

unsafe fn draw_section_label(left: i32, top: i32, width: i32, y: i32, label: &str) {
    draw_text(left + UI_MARGIN, top - y, label, COLOR_AMBER_TEXT);
    let label_width = text_width(label) as i32;
    draw_rule(
        left + UI_MARGIN + label_width + 10,
        top - y + 4,
        left + width - UI_MARGIN,
        top - y + 4,
        COLOR_BORDER,
    );
}

unsafe fn draw_pad_selector(state: &PluginState, left: i32, top: i32, rect: Rect) {
    let hovered = state.hovered_action == Some(UiAction::ToggleDropdown);
    draw_rect(
        left,
        top,
        rect,
        if hovered || state.dropdown_open {
            COLOR_FIELD_HOVER
        } else {
            COLOR_FIELD
        },
    );
    draw_outline(
        left,
        top,
        rect,
        if hovered || state.dropdown_open {
            COLOR_FOCUS
        } else {
            COLOR_BORDER
        },
        if hovered || state.dropdown_open {
            2.0
        } else {
            1.0
        },
    );
    let count = if state.pads.is_empty() {
        "0 / 0".to_owned()
    } else {
        format!("{} / {}", state.selected_index + 1, state.pads.len())
    };
    let count_width = text_width(&count) as i32;
    let name = truncate_text(
        state.selected_name().unwrap_or("No PAD files"),
        (rect.width - count_width - 54).max(40) as f32,
    );
    draw_text(
        left + rect.x + 10,
        top - rect.y - rect.height / 2 - 5,
        &name,
        COLOR_TEXT,
    );
    draw_text(
        left + rect.x + rect.width - count_width - 28,
        top - rect.y - rect.height / 2 - 5,
        &count,
        COLOR_MUTED,
    );
    draw_text(
        left + rect.x + rect.width - 16,
        top - rect.y - rect.height / 2 - 5,
        if state.dropdown_open { "^" } else { "v" },
        COLOR_AMBER_TEXT,
    );
}

unsafe fn draw_dropdown(state: &PluginState, left: i32, top: i32, selector: Rect) {
    if !state.dropdown_open {
        return;
    }
    let visible_rows = state
        .pads
        .len()
        .saturating_sub(state.dropdown_scroll)
        .min(DROPDOWN_ROWS);
    let row_count = visible_rows.max(1);
    let list = dropdown_list_rect(selector, row_count);
    let row_top = list.y;
    draw_rect(
        left,
        top,
        Rect {
            x: list.x + 4,
            y: list.y + 4,
            width: list.width,
            height: list.height,
        },
        [0.0, 0.0, 0.0, 0.45],
    );
    draw_rect(left, top, list, COLOR_PANEL);
    draw_outline(left, top, list, COLOR_FOCUS, 2.0);

    if visible_rows == 0 {
        draw_centered_text(left, top, list, "No PAD files found", COLOR_MUTED);
        return;
    }
    for row in 0..visible_rows {
        let index = state.dropdown_scroll + row;
        let row_rect = Rect {
            x: selector.x,
            y: row_top + row as i32 * DROPDOWN_ROW_HEIGHT,
            width: selector.width - DROPDOWN_SCROLLBAR_WIDTH,
            height: DROPDOWN_ROW_HEIGHT,
        };
        let action = UiAction::SelectPad(index);
        let hovered = state.hovered_action == Some(action);
        let selected = state.selected_index == index;
        if hovered || selected {
            draw_rect(
                left,
                top,
                row_rect,
                if hovered {
                    COLOR_BUTTON_HOVER
                } else {
                    COLOR_SELECTED
                },
            );
        }
        if row > 0 {
            let (_, row_global_top, _, _) = global_rect(left, top, row_rect);
            draw_rule(
                left + row_rect.x + 8,
                row_global_top,
                left + row_rect.x + row_rect.width - 8,
                row_global_top,
                [COLOR_BORDER[0], COLOR_BORDER[1], COLOR_BORDER[2], 0.55],
            );
        }
        let number = format!("{:02}", index + 1);
        draw_text(
            left + row_rect.x + 9,
            top - row_rect.y - row_rect.height / 2 - 5,
            &number,
            if selected {
                COLOR_AMBER_TEXT
            } else {
                COLOR_MUTED
            },
        );
        let name = truncate_text(&state.pads[index], (row_rect.width - 50) as f32);
        draw_text(
            left + row_rect.x + 37,
            top - row_rect.y - row_rect.height / 2 - 5,
            &name,
            COLOR_TEXT,
        );
    }

    let (up, track, down) = dropdown_scrollbar_rects(list);
    let max_scroll = state.dropdown_max_scroll();
    let can_scroll = max_scroll > 0;
    for (rect, label, action, enabled) in [
        (
            up,
            "^",
            UiAction::ScrollDropdown(-1),
            can_scroll && state.dropdown_scroll > 0,
        ),
        (
            down,
            "v",
            UiAction::ScrollDropdown(1),
            can_scroll && state.dropdown_scroll < max_scroll,
        ),
    ] {
        let hovered = state.hovered_action == Some(action);
        draw_rect(
            left,
            top,
            rect,
            if hovered && enabled {
                COLOR_BUTTON_HOVER
            } else {
                COLOR_FIELD
            },
        );
        draw_outline(
            left,
            top,
            rect,
            if hovered && enabled {
                COLOR_FOCUS
            } else {
                COLOR_BORDER
            },
            if hovered && enabled { 2.0 } else { 1.0 },
        );
        draw_centered_text(
            left,
            top,
            rect,
            label,
            if enabled { COLOR_TEXT } else { COLOR_MUTED },
        );
    }
    draw_rect(left, top, track, COLOR_FIELD);
    draw_outline(left, top, track, COLOR_BORDER, 1.0);
    let thumb = dropdown_thumb_rect(track, state.dropdown_scroll, state.pads.len());
    draw_rect(
        left,
        top,
        thumb,
        if can_scroll {
            COLOR_FOCUS
        } else {
            [COLOR_MUTED[0], COLOR_MUTED[1], COLOR_MUTED[2], 0.45]
        },
    );
}

unsafe extern "C" fn draw_window(window: XPLMWindowID, _refcon: *mut c_void) {
    let guard = state_lock();
    let Some(state) = guard.as_ref() else { return };
    let mut left = 0;
    let mut top = 0;
    let mut right = 0;
    let mut bottom = 0;
    XPLMGetWindowGeometry(window, &mut left, &mut top, &mut right, &mut bottom);
    let width = right - left;
    let height = top - bottom;

    draw_rect(
        left,
        top,
        Rect {
            x: 0,
            y: 0,
            width,
            height,
        },
        COLOR_CANVAS,
    );
    draw_text(
        left + UI_MARGIN,
        top - 23,
        "POSITION AIRCRAFT",
        COLOR_AMBER_TEXT,
    );
    let mode = "NATIVE  /  VR  /  PAD";
    draw_text(
        right - UI_MARGIN - text_width(mode) as i32,
        top - 23,
        mode,
        COLOR_MUTED,
    );
    draw_rule(
        left + UI_MARGIN,
        top - 32,
        right - UI_MARGIN,
        top - 32,
        COLOR_BORDER,
    );

    let button_labels = [
        "Capture current",
        "Position aircraft",
        "Quick save",
        "Quick load + position",
    ];
    for (index, (rect, action)) in command_button_rects(width).into_iter().enumerate() {
        draw_button(
            state,
            left,
            top,
            ButtonSpec {
                rect,
                label: button_labels[index],
                action: UiAction::Command(action),
                tone: if matches!(
                    action,
                    CommandAction::PositionLoaded | CommandAction::QuickLoadAndPosition
                ) {
                    ButtonTone::Primary
                } else {
                    ButtonTone::Standard
                },
                selected: false,
            },
        );
    }

    draw_section_label(left, top, width, 103, "PAD LIBRARY");
    let pad = pad_layout(width);
    draw_button(
        state,
        left,
        top,
        ButtonSpec {
            rect: pad.previous,
            label: "<",
            action: UiAction::Command(CommandAction::PreviousPad),
            tone: ButtonTone::Quiet,
            selected: false,
        },
    );
    draw_pad_selector(state, left, top, pad.selector);
    draw_button(
        state,
        left,
        top,
        ButtonSpec {
            rect: pad.next,
            label: ">",
            action: UiAction::Command(CommandAction::NextPad),
            tone: ButtonTone::Quiet,
            selected: false,
        },
    );
    draw_button(
        state,
        left,
        top,
        ButtonSpec {
            rect: pad.refresh,
            label: "Refresh",
            action: UiAction::Refresh,
            tone: ButtonTone::Quiet,
            selected: false,
        },
    );
    draw_button(
        state,
        left,
        top,
        ButtonSpec {
            rect: pad.load,
            label: "Load",
            action: UiAction::LoadSelected(false),
            tone: ButtonTone::Standard,
            selected: false,
        },
    );
    draw_button(
        state,
        left,
        top,
        ButtonSpec {
            rect: pad.load_and_position,
            label: "Load + position",
            action: UiAction::LoadSelected(true),
            tone: ButtonTone::Primary,
            selected: false,
        },
    );

    draw_section_label(left, top, width, 181, "AIRCRAFT STATE");
    for (index, (field, label)) in position_fields().into_iter().enumerate() {
        draw_field(
            state,
            left,
            top,
            field_rect(width, POSITION_Y, index),
            field,
            label,
            true,
        );
    }

    let ap_rect = Rect {
        x: UI_MARGIN,
        y: AP_TOGGLE_Y,
        width: width - UI_MARGIN * 2,
        height: 32,
    };
    draw_button(
        state,
        left,
        top,
        ButtonSpec {
            rect: ap_rect,
            label: if state.form.use_ap {
                "AUTOPILOT DATA    [ APPLIED ON POSITION ]"
            } else {
                "AUTOPILOT DATA    [ NOT APPLIED ]"
            },
            action: UiAction::ToggleAp,
            tone: ButtonTone::Quiet,
            selected: state.form.use_ap,
        },
    );
    for (index, (field, label)) in autopilot_fields().into_iter().enumerate() {
        draw_field(
            state,
            left,
            top,
            field_rect(width, AP_FIELDS_Y, index),
            field,
            label,
            state.form.use_ap,
        );
    }

    let (save_field, save_button) = save_layout(width);
    draw_field(
        state,
        left,
        top,
        save_field,
        Field::SaveName,
        "Save as",
        true,
    );
    draw_button(
        state,
        left,
        top,
        ButtonSpec {
            rect: save_button,
            label: "Save PAD",
            action: UiAction::SaveNamed,
            tone: ButtonTone::Primary,
            selected: false,
        },
    );

    let status_is_error = ["Unable", "No ", "Enter ", "Invalid"]
        .iter()
        .any(|prefix| state.status.starts_with(prefix));
    draw_rect(
        left,
        top,
        Rect {
            x: UI_MARGIN,
            y: 618,
            width: 5,
            height: 16,
        },
        if status_is_error {
            [0.95, 0.22, 0.16, 1.0]
        } else {
            [0.16, 0.78, 0.52, 1.0]
        },
    );
    let status = truncate_text(&state.status, (width - UI_MARGIN * 2 - 20) as f32);
    draw_text(
        left + UI_MARGIN + 13,
        top - 629,
        &status,
        if status_is_error {
            [1.0, 0.60, 0.52]
        } else {
            [0.68, 0.92, 0.80]
        },
    );

    draw_dropdown(state, left, top, pad.selector);
}

unsafe fn execute_ui_action(action: UiAction) {
    if !matches!(action, UiAction::Edit(_)) {
        let mut guard = state_lock();
        if let Some(state) = guard.as_mut() {
            state.active_field = None;
            if !matches!(
                action,
                UiAction::ToggleDropdown | UiAction::ScrollDropdown(_) | UiAction::SelectPad(_)
            ) {
                state.dropdown_open = false;
            }
            XPLMTakeKeyboardFocus(ptr::null_mut());
        }
    }
    match action {
        UiAction::Command(command) => execute_command(command),
        UiAction::LoadSelected(position) => {
            let mut guard = state_lock();
            if let Some(state) = guard.as_mut() {
                state.load_selected(position);
            }
        }
        UiAction::ToggleAp => {
            let mut guard = state_lock();
            if let Some(state) = guard.as_mut() {
                state.form.use_ap = !state.form.use_ap;
            }
        }
        UiAction::SaveNamed => {
            let mut guard = state_lock();
            if let Some(state) = guard.as_mut() {
                state.save_named();
            }
        }
        UiAction::Refresh => {
            let mut guard = state_lock();
            if let Some(state) = guard.as_mut() {
                state.refresh_pads();
            }
        }
        UiAction::ToggleDropdown => {
            let mut guard = state_lock();
            if let Some(state) = guard.as_mut() {
                if state.dropdown_open {
                    state.dropdown_open = false;
                } else {
                    if state.pads.is_empty() {
                        state.refresh_pads();
                    }
                    state.open_dropdown();
                }
            }
        }
        UiAction::CloseDropdown => {
            let mut guard = state_lock();
            if let Some(state) = guard.as_mut() {
                state.dropdown_open = false;
                state.hovered_action = None;
            }
        }
        UiAction::ScrollDropdown(delta) => {
            let mut guard = state_lock();
            if let Some(state) = guard.as_mut() {
                state.scroll_dropdown(delta);
            }
        }
        UiAction::SelectPad(index) => {
            let mut guard = state_lock();
            if let Some(state) = guard.as_mut() {
                state.select_pad(index);
            }
        }
        UiAction::Edit(field) => {
            let mut guard = state_lock();
            if let Some(state) = guard.as_mut() {
                state.dropdown_open = false;
                state.active_field = Some(field);
                XPLMTakeKeyboardFocus(state.window);
                state.status = format!("Editing {}; press Enter when done", field_label(field));
            }
        }
    }
}

unsafe extern "C" fn handle_mouse(
    window: XPLMWindowID,
    x: c_int,
    y: c_int,
    mouse_status: XPLMMouseStatus,
    _refcon: *mut c_void,
) -> c_int {
    if mouse_status == XPLM_MOUSE_DOWN {
        let action = {
            let mut guard = state_lock();
            let Some(state) = guard.as_mut() else {
                return 0;
            };
            let mut left = 0;
            let mut top = 0;
            let mut right = 0;
            let mut bottom = 0;
            XPLMGetWindowGeometry(window, &mut left, &mut top, &mut right, &mut bottom);
            let action = state.hit_test(x - left, top - y, right - left);
            state.mouse_captured = action.is_some();
            action
        };
        if let Some(action) = action {
            execute_ui_action(action);
            1
        } else {
            // Important for X-Plane 12.4.3 VR: unlike FlyWithLua, do not
            // swallow controller input that our UI did not actually handle.
            0
        }
    } else if mouse_status == XPLM_MOUSE_DRAG || mouse_status == XPLM_MOUSE_UP {
        let mut guard = state_lock();
        let Some(state) = guard.as_mut() else {
            return 0;
        };
        let captured = state.mouse_captured;
        if mouse_status == XPLM_MOUSE_UP {
            state.mouse_captured = false;
        }
        if captured {
            1
        } else {
            0
        }
    } else {
        0
    }
}

unsafe extern "C" fn handle_right_click(
    _window: XPLMWindowID,
    _x: c_int,
    _y: c_int,
    _mouse_status: XPLMMouseStatus,
    _refcon: *mut c_void,
) -> c_int {
    0
}

unsafe extern "C" fn handle_cursor(
    window: XPLMWindowID,
    x: c_int,
    y: c_int,
    _refcon: *mut c_void,
) -> XPLMCursorStatus {
    let mut left = 0;
    let mut top = 0;
    let mut right = 0;
    let mut bottom = 0;
    XPLMGetWindowGeometry(window, &mut left, &mut top, &mut right, &mut bottom);
    let mut guard = state_lock();
    let Some(state) = guard.as_mut() else {
        return XPLM_CURSOR_DEFAULT;
    };
    let action = state.hit_test(x - left, top - y, right - left);
    state.hovered_action = action.filter(|action| *action != UiAction::CloseDropdown);
    if state.hovered_action.is_some() {
        XPLM_CURSOR_ARROW
    } else {
        XPLM_CURSOR_DEFAULT
    }
}

unsafe extern "C" fn handle_wheel(
    window: XPLMWindowID,
    x: c_int,
    y: c_int,
    wheel: c_int,
    clicks: c_int,
    _refcon: *mut c_void,
) -> c_int {
    if wheel != 0 || clicks == 0 {
        return 0;
    }
    let mut left = 0;
    let mut top = 0;
    let mut right = 0;
    let mut bottom = 0;
    XPLMGetWindowGeometry(window, &mut left, &mut top, &mut right, &mut bottom);
    let local_x = x - left;
    let local_y = top - y;
    let mut guard = state_lock();
    let Some(state) = guard.as_mut() else {
        return 0;
    };
    if !state.dropdown_open {
        return 0;
    }
    state.scroll_dropdown(-(clicks as isize));
    state.hovered_action = state
        .hit_test(local_x, local_y, right - left)
        .filter(|action| *action != UiAction::CloseDropdown);
    1
}

fn field_label(field: Field) -> &'static str {
    match field {
        Field::Latitude => "Latitude",
        Field::Longitude => "Longitude",
        Field::Altitude => "Altitude",
        Field::Heading => "Heading",
        Field::Pitch => "Pitch",
        Field::Roll => "Roll",
        Field::Speed => "Speed",
        Field::Throttle => "Throttle",
        Field::Flaps => "Flaps",
        Field::Gear => "Gear",
        Field::ApMode => "AP mode",
        Field::ApAltitude => "AP altitude",
        Field::ApVerticalVelocity => "AP vertical velocity",
        Field::ApHeading => "AP heading",
        Field::ApAirspeed => "AP airspeed",
        Field::ApState => "AP state",
        Field::ApHeadingRollMode => "AP bank limit mode",
        Field::SaveName => "Save filename",
    }
}

unsafe extern "C" fn handle_key(
    _window: XPLMWindowID,
    key: c_char,
    flags: XPLMKeyFlags,
    virtual_key: c_char,
    _refcon: *mut c_void,
    losing_focus: c_int,
) {
    let mut guard = state_lock();
    let Some(state) = guard.as_mut() else { return };
    if losing_focus != 0 {
        state.active_field = None;
        return;
    }
    if flags & XPLM_DOWN_FLAG == 0 {
        return;
    }
    let Some(field) = state.active_field else {
        return;
    };
    let key_byte = key as u8;
    let virtual_byte = virtual_key as u8;
    if key_byte == 13 || virtual_byte == 13 {
        state.active_field = None;
        XPLMTakeKeyboardFocus(ptr::null_mut());
        state.status = format!("Finished editing {}", field_label(field));
        return;
    }
    if key_byte == 27 || virtual_byte == 27 {
        state.active_field = None;
        XPLMTakeKeyboardFocus(ptr::null_mut());
        return;
    }
    let value = state.form.value_mut(field);
    if key_byte == 8 || virtual_byte == 8 || virtual_byte == 127 {
        value.pop();
        return;
    }
    if value.len() >= 63 || !(32..=126).contains(&key_byte) {
        return;
    }
    let character = key_byte as char;
    if field == Field::SaveName || "0123456789+-.eE".contains(character) {
        value.push(character);
    }
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

unsafe fn plugin_start(
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

unsafe fn plugin_stop() {
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

#[no_mangle]
/// X-Plane plugin entry point.
///
/// # Safety
/// X-Plane must pass writable SDK-sized output buffers and load the plugin on
/// its normal plugin-management thread.
pub unsafe extern "C" fn XPluginStart(
    out_name: *mut c_char,
    out_signature: *mut c_char,
    out_description: *mut c_char,
) -> c_int {
    plugin_start(out_name, out_signature, out_description)
}

#[no_mangle]
/// X-Plane plugin shutdown entry point.
///
/// # Safety
/// X-Plane must invoke this only after a successful `XPluginStart` and after
/// it has stopped dispatching callbacks to this plugin.
pub unsafe extern "C" fn XPluginStop() {
    plugin_stop();
}

#[no_mangle]
/// X-Plane plugin enable entry point.
///
/// # Safety
/// This must be called by X-Plane's plugin manager.
pub unsafe extern "C" fn XPluginEnable() -> c_int {
    1
}

#[no_mangle]
/// X-Plane plugin disable entry point.
///
/// # Safety
/// This must be called by X-Plane's plugin manager.
pub unsafe extern "C" fn XPluginDisable() {}

#[no_mangle]
/// Receives X-Plane broadcast messages.
///
/// # Safety
/// The message and sender values must follow the XPLM ABI and the callback
/// must be made on X-Plane's plugin thread.
pub unsafe extern "C" fn XPluginReceiveMessage(
    from: XPLMPluginID,
    message: c_int,
    _parameter: *mut c_void,
) {
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

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
[CONFIG]
Use_AutoPilot_Data = 1

[POSITION_DATA]
Latitude = 41.121516
Longitude = -73.178842
Altitude = 959.000000
Heading = 58.000000
Pitch = 1.250000
Roll = -2.500000
Speed = 90.000000
Throttle = 0.750000
Flaps = 0.250000
Gear = 1

[AUTOPILOT_DATA]
AutoPilot_Mode = 2
AutoPilot_Altitude = 3000
AutoPilot_Vertical_Velocity = 500
AutoPilot_Heading = 60
AutoPilot_Airspeed = 100
AutoPilot_State = 4
AutoPilot_Heading_Roll_Mode = 1
"#;

    #[test]
    fn parses_original_pad_format() {
        let data = parse_pad_text(SAMPLE).unwrap();
        assert!((data.latitude - 41.121516).abs() < 1e-9);
        assert_eq!(data.heading, 58.0);
        assert_eq!(data.gear, 1);
        assert!(data.use_ap);
        assert_eq!(data.ap.mode, 2);
        assert_eq!(data.ap.heading_roll_mode, 1);
    }

    #[test]
    fn form_round_trip_and_heading_normalization() {
        let data = parse_pad_text(SAMPLE).unwrap();
        let mut form = Form::from_data(&data, "Test");
        *form.value_mut(Field::Heading) = "-2".to_owned();
        let parsed = form.to_data().unwrap();
        assert_eq!(parsed.heading, 358.0);
        assert_eq!(parsed.ap.altitude, 3000.0);
    }

    #[test]
    fn safe_file_names_remain_in_pad_directory() {
        assert_eq!(
            safe_pad_filename("My Position"),
            Some("My Position.pad".into())
        );
        assert_eq!(
            safe_pad_filename("../bad:name.pad"),
            Some("__bad_name.pad".into())
        );
        assert_eq!(safe_pad_filename(""), None);
    }

    #[test]
    fn magnetic_heading_conversion_matches_kbdr_regression() {
        let magnetic = normalize_heading(45.0 + 13.0);
        let true_heading = normalize_heading(magnetic - 13.0);
        assert_eq!(magnetic, 58.0);
        assert_eq!(true_heading, 45.0);
    }

    #[test]
    fn xplm_window_struct_matches_x64_sdk_layout() {
        assert_eq!(mem::size_of::<XPLMCreateWindowT>(), 88);
    }

    #[test]
    fn minimum_width_control_layout_does_not_overlap() {
        let width = 660;
        let pad = pad_layout(width);
        let controls = [
            pad.previous,
            pad.selector,
            pad.next,
            pad.refresh,
            pad.load,
            pad.load_and_position,
        ];
        assert!(controls.iter().all(|rect| rect.width > 0));
        for pair in controls.windows(2) {
            assert!(pair[0].x + pair[0].width <= pair[1].x);
        }
        assert!(pad.load_and_position.x + pad.load_and_position.width <= width - UI_MARGIN);

        let action_buttons = command_button_rects(width);
        for pair in action_buttons.windows(2) {
            assert!(pair[0].0.x + pair[0].0.width <= pair[1].0.x);
        }
    }

    #[test]
    fn dropdown_and_fields_fit_the_minimum_window_height() {
        let dropdown_bottom = PAD_Y + 36 + 2 + DROPDOWN_ROWS as i32 * DROPDOWN_ROW_HEIGHT;
        assert!(dropdown_bottom <= AP_TOGGLE_Y);
        assert!(field_rect(660, AP_FIELDS_Y, autopilot_fields().len() - 1).y + 30 < SAVE_Y);
        let (save_field, _) = save_layout(660);
        assert!(save_field.y + save_field.height < WINDOW_HEIGHT);
    }

    #[test]
    fn dropdown_scrollbar_thumb_tracks_the_visible_page() {
        let selector = pad_layout(660).selector;
        let list = dropdown_list_rect(selector, DROPDOWN_ROWS);
        let (up, track, down) = dropdown_scrollbar_rects(list);
        assert_eq!(up.y + up.height, track.y);
        assert_eq!(track.y + track.height, down.y);
        assert_eq!(down.y + down.height, list.y + list.height);

        let first = dropdown_thumb_rect(track, 0, 46);
        let middle = dropdown_thumb_rect(track, 19, 46);
        let last = dropdown_thumb_rect(track, 38, 46);
        assert_eq!(first.y, track.y);
        assert!(middle.y > first.y);
        assert_eq!(last.y + last.height, track.y + track.height);
    }

    #[test]
    fn parses_every_installed_pad_file() {
        let pad_directory = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("..")
            .join("Resources")
            .join("plugins")
            .join("PositionAircraft");
        let mut count = 0;
        for entry in fs::read_dir(&pad_directory).unwrap() {
            let path = entry.unwrap().path();
            if path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("pad"))
            {
                parse_pad(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
                count += 1;
            }
        }
        assert!(count > 0, "no installed PAD files were found");
    }
}
