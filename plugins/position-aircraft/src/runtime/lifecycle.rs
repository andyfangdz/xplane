use std::ffi::{c_char, c_int};
use std::mem;
use std::ptr;

use crate::pad::{Form, PadData};
use xplane_plugin::{c_string, system_path, write_plugin_metadata, PluginMetadata};
use xplane_sdk_sys::*;

use super::commands;
use super::datarefs::DataRefs;
use super::state::{replace_state, with_state_mut, PluginState};
use super::support::log;
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
    let mut params = XPLMCreateWindow_t {
        structSize: mem::size_of::<XPLMCreateWindow_t>() as i32,
        left: screen_left + 100,
        top: screen_top - 100,
        right: screen_left + 100 + WINDOW_WIDTH,
        bottom: screen_top - 100 - WINDOW_HEIGHT,
        visible: 0,
        drawWindowFunc: Some(draw_window),
        handleMouseClickFunc: Some(handle_mouse),
        handleKeyFunc: Some(handle_key),
        handleCursorFunc: Some(handle_cursor),
        handleMouseWheelFunc: Some(handle_wheel),
        refcon: ptr::null_mut(),
        decorateAsFloatingWindow: xplm_WindowDecorationRoundRectangle,
        layer: xplm_WindowLayerFloatingWindows,
        handleRightClickFunc: Some(handle_right_click),
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
        write_plugin_metadata(
            out_name,
            out_signature,
            out_description,
            PluginMetadata {
                name: "Position Aircraft Native",
                signature: "com.openai.position-aircraft-native-rust",
                description: "Native VR and joystick PositionAircraft replacement written in Rust",
            },
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
        menu: None,
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
                XPLMSetWindowPositioningMode(window, xplm_WindowVR, -1);
            } else {
                XPLMSetWindowPositioningMode(window, xplm_WindowPositionFree, -1);
            }
        }
        commands::register(state)?;
        commands::create_menu(state)?;
        Ok(())
    })
    .unwrap_or_else(|| Err("Plugin state disappeared during startup".to_owned()));
    if let Err(error) = setup_result {
        log(&error);
        with_state_mut(commands::unregister);
        // SAFETY: `window` is the live handle created immediately above.
        unsafe { XPLMDestroyWindow(window) };
        replace_state(None);
        return 0;
    }
    // SAFETY: the callback has the XPLM ABI and uses no refcon.
    unsafe { XPLMRegisterFlightLoopCallback(Some(commands::flight_loop), -1.0, ptr::null_mut()) };
    log("0.3.0 loaded (native XPLM window, egui interface)");
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
    if from != XPLM_PLUGIN_XPLANE as XPLMPluginID {
        return;
    }
    with_state_mut(|state| {
        if state.window.is_null() {
            return;
        }
        // SAFETY: the retained window handle is live, and the screen-bound
        // outputs point to valid locals.
        unsafe {
            if message == XPLM_MSG_ENTERED_VR as c_int {
                XPLMSetWindowPositioningMode(state.window, xplm_WindowVR, -1);
            } else if message == XPLM_MSG_EXITING_VR as c_int {
                XPLMSetWindowPositioningMode(state.window, xplm_WindowPositionFree, -1);
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
