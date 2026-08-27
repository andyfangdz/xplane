use std::ffi::{c_char, c_int, c_void, CStr};
use std::path::PathBuf;
use std::ptr;
use std::time::Instant;

use xplane_sdk_sys::*;

use super::config::{Settings, SHOW_DURATIONS};
use super::datarefs::DataRefs;
use super::runway::RunwayDatabase;
use super::support::{
    c_string, log, plugin_directory, preferences_directory, system_path, write_plugin_string,
};
use super::{replace_state, with_state_mut, PluginState};

const MENU_LOG: usize = 1;
const MENU_REPLAY: usize = 2;
const MENU_DURATION_BASE: usize = 10;
const MENU_PREVIEW: usize = 100;

pub(crate) unsafe fn start(
    out_name: *mut c_char,
    out_signature: *mut c_char,
    out_description: *mut c_char,
) -> c_int {
    // SAFETY: X-Plane supplied its standard writable metadata buffers.
    unsafe {
        write_plugin_string(out_name, "Landing Speed Rust 3.46.1");
        write_plugin_string(out_signature, "com.andyfang.xgs-rs");
        write_plugin_string(
            out_description,
            "Rust recreation of Landing Speed (xgs) 3.46",
        );
    }
    for feature in ["XPLM_USE_NATIVE_PATHS", "XPLM_USE_NATIVE_WIDGET_WINDOWS"] {
        let feature = c_string(feature);
        // SAFETY: the feature name is a live NUL-terminated string.
        unsafe { XPLMEnableFeature(feature.as_ptr(), 1) };
    }
    let root = system_path();
    let directory = plugin_directory();
    let settings = Settings::load(&preferences_directory());
    replace_state(Some(PluginState::new(root, directory, settings)));
    log("startup 3.46.1 (compatible with xgs 3.46)");
    1
}

pub(crate) fn enable() -> bool {
    let mut should_register = false;
    let enabled = with_state_mut(|state| {
        if state.enabled {
            return true;
        }
        let datarefs = match DataRefs::find() {
            Ok(datarefs) => datarefs,
            Err(error) => {
                log(&format!("enable failed: {error}"));
                return false;
            }
        };
        state.datarefs = Some(datarefs);
        let started = Instant::now();
        match RunwayDatabase::load(&state.xplane_root) {
            Ok(database) => {
                log(&format!(
                    "loaded {} runways in {:.1}s",
                    database.runway_count(),
                    started.elapsed().as_secs_f32()
                ));
                state.runways = Some(database);
            }
            Err(error) => log(&format!("runway database unavailable: {error}")),
        }
        create_menu(state);
        state.enabled = true;
        state.aircraft_loaded(current_aircraft_path());
        should_register = true;
        log("enabled");
        true
    })
    .unwrap_or(false);
    if should_register {
        // SAFETY: the callback has the XPLM ABI and remains available for the plugin lifetime.
        unsafe {
            XPLMRegisterFlightLoopCallback(Some(flight_loop_callback), 0.05, ptr::null_mut())
        };
    }
    enabled
}

pub(crate) fn disable() {
    let was_enabled = with_state_mut(|state| state.enabled).unwrap_or(false);
    if was_enabled {
        // SAFETY: this exactly matches the callback registration performed in `enable`.
        unsafe { XPLMUnregisterFlightLoopCallback(Some(flight_loop_callback), ptr::null_mut()) };
    }
    with_state_mut(|state| {
        if !state.enabled {
            return;
        }
        destroy_menu(state);
        state.shutdown_ui();
        state.datarefs = None;
        state.runways = None;
        state.enabled = false;
        log("disabled");
    });
}

pub(crate) fn stop() {
    disable();
    replace_state(None);
    log("stopped");
}

pub(crate) fn receive_message(_from: XPLMPluginID, message: c_int, parameter: *mut c_void) {
    match message as u32 {
        XPLM_MSG_PLANE_LOADED if parameter.is_null() => {
            with_state_mut(|state| state.aircraft_loaded(current_aircraft_path()));
        }
        XPLM_MSG_ENTERED_VR => {
            with_state_mut(|state| state.set_vr(true));
        }
        XPLM_MSG_EXITING_VR => {
            with_state_mut(|state| state.set_vr(false));
        }
        XPLM_MSG_WILL_WRITE_PREFS => {
            with_state_mut(|state| state.settings.save());
        }
        _ => {}
    }
}

unsafe extern "C" fn flight_loop_callback(
    elapsed_since_last_call: f32,
    _elapsed_since_last_flight_loop: f32,
    _counter: c_int,
    _reference: *mut c_void,
) -> f32 {
    with_state_mut(|state| state.flight_loop(elapsed_since_last_call)).unwrap_or(2.0)
}

fn current_aircraft_path() -> Option<PathBuf> {
    let mut file_name = [0_i8; 256];
    let mut path = [0_i8; 2048];
    // SAFETY: both SDK output buffers are writable and exceed the documented minimum sizes.
    unsafe { XPLMGetNthAircraftModel(0, file_name.as_mut_ptr(), path.as_mut_ptr()) };
    // SAFETY: the SDK writes a NUL-terminated path.
    let path = unsafe { CStr::from_ptr(path.as_ptr()) }.to_string_lossy();
    (!path.is_empty()).then(|| PathBuf::from(path.as_ref()))
}

fn create_menu(state: &mut PluginState) {
    if !state.menu.menu.is_null() {
        return;
    }
    let title = c_string("Landing Speed Rust");
    // SAFETY: the Plugins menu is owned by X-Plane; all strings live for each immediate call.
    unsafe {
        let plugins_menu = XPLMFindPluginsMenu();
        state.menu.parent_index =
            XPLMAppendMenuItem(plugins_menu, title.as_ptr(), ptr::null_mut(), 0);
        state.menu.menu = XPLMCreateMenu(
            title.as_ptr(),
            plugins_menu,
            state.menu.parent_index,
            Some(menu_callback),
            ptr::null_mut(),
        );
        state.menu.log_index = append_menu_item(state.menu.menu, "Enable Log", MENU_LOG);
        state.menu.replay_index = append_menu_item(state.menu.menu, "Show in Replay", MENU_REPLAY);
        XPLMAppendMenuSeparator(state.menu.menu);
        state.menu.duration_indices = SHOW_DURATIONS
            .iter()
            .enumerate()
            .map(|(index, (label, _))| {
                append_menu_item(state.menu.menu, label, MENU_DURATION_BASE + index)
            })
            .collect();
        XPLMAppendMenuSeparator(state.menu.menu);
        append_menu_item(state.menu.menu, "Preview Overlay", MENU_PREVIEW);
    }
    update_menu_checks(state);
}

unsafe fn append_menu_item(menu: XPLMMenuID, label: &str, identifier: usize) -> i32 {
    let label = c_string(label);
    // SAFETY: menu is live, label is NUL-terminated, and the integer token is never dereferenced.
    unsafe { XPLMAppendMenuItem(menu, label.as_ptr(), identifier as *mut c_void, 0) }
}

fn update_menu_checks(state: &PluginState) {
    if state.menu.menu.is_null() {
        return;
    }
    // SAFETY: all item indices were returned by this live menu.
    unsafe {
        XPLMCheckMenuItem(
            state.menu.menu,
            state.menu.log_index,
            if state.settings.log_enabled {
                xplm_Menu_Checked
            } else {
                xplm_Menu_Unchecked
            },
        );
        XPLMCheckMenuItem(
            state.menu.menu,
            state.menu.replay_index,
            if state.settings.show_in_replay {
                xplm_Menu_Checked
            } else {
                xplm_Menu_Unchecked
            },
        );
        for (index, menu_index) in state.menu.duration_indices.iter().enumerate() {
            XPLMCheckMenuItem(
                state.menu.menu,
                *menu_index,
                if index == state.settings.show_duration_index {
                    xplm_Menu_Checked
                } else {
                    xplm_Menu_Unchecked
                },
            );
        }
    }
}

fn destroy_menu(state: &mut PluginState) {
    if state.menu.menu.is_null() {
        return;
    }
    // SAFETY: menu and parent item are live and were created by this plugin.
    unsafe {
        XPLMDestroyMenu(state.menu.menu);
        XPLMRemoveMenuItem(XPLMFindPluginsMenu(), state.menu.parent_index);
    }
    state.menu = super::state::MenuState {
        parent_index: -1,
        log_index: -1,
        replay_index: -1,
        ..super::state::MenuState::default()
    };
}

unsafe extern "C" fn menu_callback(_menu_reference: *mut c_void, item_reference: *mut c_void) {
    let identifier = item_reference as usize;
    with_state_mut(|state| {
        match identifier {
            MENU_LOG => state.settings.log_enabled = !state.settings.log_enabled,
            MENU_REPLAY => state.settings.show_in_replay = !state.settings.show_in_replay,
            MENU_PREVIEW => state.preview_overlay(),
            identifier
                if (MENU_DURATION_BASE..MENU_DURATION_BASE + SHOW_DURATIONS.len())
                    .contains(&identifier) =>
            {
                state.settings.show_duration_index = identifier - MENU_DURATION_BASE;
            }
            _ => return,
        }
        update_menu_checks(state);
        state.settings.save();
    });
}
