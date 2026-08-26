use std::ffi::{c_char, c_int};
use std::mem;
use std::ptr;

use crate::pad::{Form, PadData};
use crate::xplm::*;

use super::commands;
use super::datarefs::DataRefs;
use super::state::{replace_state, with_state_mut, PluginState};
use super::support::{c_string, log, system_path, write_plugin_string};
use super::ui::{
    draw_window, handle_cursor, handle_key, handle_mouse, handle_right_click, handle_wheel,
    EguiIntegration,
};

const WINDOW_WIDTH: i32 = 720;
const WINDOW_HEIGHT: i32 = 880;

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
        commands::register(state)?;
        commands::create_menu(state);
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
    unsafe { XPLMRegisterFlightLoopCallback(Some(commands::flight_loop), -1.0, ptr::null_mut()) };
    log("0.3.0 loaded (XPLM 4.3 native window, egui interface)");
    1
}

pub(crate) fn stop() {
    // SAFETY: this unregisters the exact callback/refcon pair registered by `start`.
    unsafe { XPLMUnregisterFlightLoopCallback(Some(commands::flight_loop), ptr::null_mut()) };
    let Some(mut state) = replace_state(None) else {
        return;
    };
    commands::unregister(&mut state);
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
