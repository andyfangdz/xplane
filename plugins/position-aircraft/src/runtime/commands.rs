use std::ffi::{c_float, c_int, c_void};
use std::ptr;

use xplane_sdk_sys::*;

use super::state::{with_state_mut, PluginState};
use super::support::c_string;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(in crate::runtime) enum CommandAction {
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

pub(in crate::runtime) struct RegisteredCommand {
    command: XPLMCommandRef,
    action: CommandAction,
}

fn execute(action: CommandAction) {
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
    if phase == xplm_CommandBegin {
        if let Some(action) = CommandAction::from_refcon(refcon) {
            execute(action);
        }
    }
    1
}

pub(super) unsafe extern "C" fn flight_loop(
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

pub(super) fn register(state: &mut PluginState) -> Result<(), String> {
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

pub(super) fn create_menu(state: &mut PluginState) {
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
            .find(|registered| registered.action == action)
            .map(|registered| registered.command)
        {
            let label = c_string(label);
            // SAFETY: the menu and command handles are live and the label is
            // NUL-terminated for the duration of the call.
            unsafe { XPLMAppendMenuItemWithCommand(state.menu, label.as_ptr(), command) };
        }
    }
}

pub(super) fn unregister(state: &mut PluginState) {
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
}
