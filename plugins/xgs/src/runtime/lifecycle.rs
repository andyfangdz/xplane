use std::ffi::{c_char, c_int, c_void};
use std::ptr;
use std::time::Instant;

use xplane_plugin::{
    current_aircraft_path, enable_feature, plugin_directory, preferences_directory, system_path,
    write_plugin_metadata, PluginMenu, PluginMetadata,
};
use xplane_sdk_sys::*;

use super::config::{Settings, SHOW_DURATIONS};
use super::datarefs::DataRefs;
use super::runway::RunwayDatabase;
use super::support::log;
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
        write_plugin_metadata(
            out_name,
            out_signature,
            out_description,
            PluginMetadata {
                name: "Landing Speed Rust 3.46.1",
                signature: "com.andyfang.xgs-rs",
                description: "Rust recreation of Landing Speed (xgs) 3.46",
            },
        );
    }
    for feature in ["XPLM_USE_NATIVE_PATHS", "XPLM_USE_NATIVE_WIDGET_WINDOWS"] {
        enable_feature(feature);
    }
    let root = system_path();
    let directory = plugin_directory()
        .unwrap_or_else(|| root.join("Resources").join("plugins").join("XgsRust"));
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
        if let Err(error) = create_menu(state) {
            log(&format!("enable failed: {error}"));
            state.datarefs = None;
            state.runways = None;
            return false;
        }
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

fn create_menu(state: &mut PluginState) -> Result<(), String> {
    if state.menu.menu.is_some() {
        return Ok(());
    }
    let menu = PluginMenu::new("Landing Speed Rust", Some(menu_callback))?;
    state.menu.log_index = menu.append_item("Enable Log", MENU_LOG)?;
    state.menu.replay_index = menu.append_item("Show in Replay", MENU_REPLAY)?;
    menu.append_separator();
    state.menu.duration_indices = SHOW_DURATIONS
        .iter()
        .enumerate()
        .map(|(index, (label, _))| menu.append_item(label, MENU_DURATION_BASE + index))
        .collect::<Result<Vec<_>, _>>()?;
    menu.append_separator();
    menu.append_item("Preview Overlay", MENU_PREVIEW)?;
    state.menu.menu = Some(menu);
    update_menu_checks(state);
    Ok(())
}

fn update_menu_checks(state: &PluginState) {
    let Some(menu) = state.menu.menu.as_ref() else {
        return;
    };
    menu.set_checked(state.menu.log_index, state.settings.log_enabled);
    menu.set_checked(state.menu.replay_index, state.settings.show_in_replay);
    for (index, menu_index) in state.menu.duration_indices.iter().enumerate() {
        menu.set_checked(*menu_index, index == state.settings.show_duration_index);
    }
}

fn destroy_menu(state: &mut PluginState) {
    state.menu = super::state::MenuState::default();
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
