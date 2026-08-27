use std::ffi::{c_int, c_void};

use crate::pad::{Form, PadData};
use xplane_plugin::{
    screen_bounds, system_path, Bounds, FlightLoop, Window, WindowCallbacks, WindowConfig,
    WindowPosition,
};
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

fn create_window() -> Result<Window, String> {
    let screen = screen_bounds();
    let window = Window::create(WindowConfig {
        bounds: Bounds::new(
            screen.left + 100,
            screen.top - 100,
            screen.left + 100 + WINDOW_WIDTH,
            screen.top - 100 - WINDOW_HEIGHT,
        ),
        visible: false,
        callbacks: WindowCallbacks {
            draw: Some(draw_window),
            mouse: Some(handle_mouse),
            key: Some(handle_key),
            cursor: Some(handle_cursor),
            wheel: Some(handle_wheel),
            right_click: Some(handle_right_click),
        },
        decoration: xplm_WindowDecorationRoundRectangle,
        layer: xplm_WindowLayerFloatingWindows,
    })?;
    window.set_resizing_limits(660, 840, 1000, 1000);
    window.set_title("Position Aircraft - Native Rust");
    Ok(window)
}

pub(crate) fn start() -> bool {
    let datarefs = match DataRefs::find() {
        Ok(datarefs) => datarefs,
        Err(error) => {
            log(&error);
            return false;
        }
    };
    let pad_directory = system_path()
        .join("Resources")
        .join("plugins")
        .join("PositionAircraft");
    let mut initial = PluginState {
        window: None,
        flight_loop: None,
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
            return false;
        }
    };
    let setup_result = with_state_mut(|state| {
        state.window = Some(window);
        let position = if state.datarefs.vr_enabled.get_i32() != 0 {
            WindowPosition::Vr
        } else {
            WindowPosition::Free
        };
        state
            .window
            .as_ref()
            .expect("window was assigned")
            .set_position(position);
        commands::register(state)?;
        commands::create_menu(state)?;
        Ok(())
    })
    .unwrap_or_else(|| Err("Plugin state disappeared during startup".to_owned()));
    if let Err(error) = setup_result {
        log(&error);
        replace_state(None);
        return false;
    }
    let flight_loop = match FlightLoop::register(Some(commands::flight_loop), -1.0) {
        Ok(flight_loop) => flight_loop,
        Err(error) => {
            log(&error);
            replace_state(None);
            return false;
        }
    };
    with_state_mut(|state| state.flight_loop = Some(flight_loop));
    log("0.3.0 loaded (native XPLM window, egui interface)");
    true
}

pub(crate) fn enable() -> bool {
    true
}

pub(crate) fn disable() {}

pub(crate) fn stop() {
    let Some(mut state) = replace_state(None) else {
        return;
    };
    state.flight_loop.take();
    commands::unregister(&mut state);
    state.menu.take();
    if let Some(ui) = state.ui.as_mut() {
        ui.destroy_renderer();
    }
    state.window.take();
    log("unloaded");
}

pub(crate) fn receive_message(from: XPLMPluginID, message: c_int, _parameter: *mut c_void) {
    if from != XPLM_PLUGIN_XPLANE as XPLMPluginID {
        return;
    }
    with_state_mut(|state| {
        let Some(window) = state.window.as_ref() else {
            return;
        };
        if message == XPLM_MSG_ENTERED_VR as c_int {
            window.set_position(WindowPosition::Vr);
        } else if message == XPLM_MSG_EXITING_VR as c_int {
            window.set_position(WindowPosition::Free);
            let screen = screen_bounds();
            window.set_geometry(Bounds::new(
                screen.left + 100,
                screen.top - 100,
                screen.left + 100 + WINDOW_WIDTH,
                screen.top - 100 - WINDOW_HEIGHT,
            ));
        }
    });
}
