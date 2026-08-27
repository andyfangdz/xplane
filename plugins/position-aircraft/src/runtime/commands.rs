use std::ffi::{c_float, c_int, c_void};
use std::ptr::NonNull;

use xplane_plugin::{c_string, PluginMenu};
use xplane_sdk_sys::*;

use super::state::{with_state_mut, PluginState};

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
    command: NonNull<c_void>,
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
        let command =
            NonNull::new(unsafe { XPLMCreateCommand(name.as_ptr(), description.as_ptr()) })
                .ok_or_else(|| format!("Unable to create command {short_name}"))?;
        // SAFETY: `command` is live, the callback has the required ABI, and the
        // integer-valued refcon is decoded without dereferencing it.
        unsafe {
            XPLMRegisterCommandHandler(
                command.as_ptr(),
                Some(command_handler),
                1,
                action as usize as *mut c_void,
            );
        }
        state.commands.push(RegisteredCommand { command, action });
    }
    Ok(())
}

pub(super) fn create_menu(state: &mut PluginState) -> Result<(), String> {
    let menu = PluginMenu::new("Position Aircraft Native", Some(menu_handler))?;
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
            menu.append_command(label, command)?;
        }
    }
    state.menu = Some(menu);
    Ok(())
}

pub(super) fn unregister(state: &mut PluginState) {
    state.menu = None;
    for command in state.commands.drain(..) {
        // SAFETY: each tuple exactly matches a registration retained in state.
        unsafe {
            XPLMUnregisterCommandHandler(
                command.command.as_ptr(),
                Some(command_handler),
                1,
                command.action as usize as *mut c_void,
            );
        }
    }
}
