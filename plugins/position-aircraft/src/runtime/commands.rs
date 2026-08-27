use std::ffi::{c_float, c_int, c_void};

use xplane_plugin::{Command, PluginMenu};
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
    PositionPattern,
    PreviousPatternLocation,
    NextPatternLocation,
}

impl CommandAction {
    fn from_refcon(refcon: *mut c_void) -> Option<Self> {
        match Command::identifier_from_refcon(refcon) {
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
            11 => Some(Self::PositionPattern),
            12 => Some(Self::PreviousPatternLocation),
            13 => Some(Self::NextPatternLocation),
            _ => None,
        }
    }
}

pub(in crate::runtime) struct RegisteredCommand {
    command: Command,
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
        CommandAction::PositionPattern => state.position_pattern(),
        CommandAction::PreviousPatternLocation => state.cycle_pattern_location(-1),
        CommandAction::NextPatternLocation => state.cycle_pattern_location(1),
    });
}

extern "C" fn command_handler(
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

pub(super) extern "C" fn flight_loop(
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

extern "C" fn menu_handler(_menu_ref: *mut c_void, _item_ref: *mut c_void) {}

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
        (
            CommandAction::PositionPattern,
            "position_pattern",
            "PositionAircraft Native: Position at the selected traffic-pattern location",
        ),
        (
            CommandAction::PreviousPatternLocation,
            "previous_pattern_location",
            "PositionAircraft Native: Select previous traffic-pattern location",
        ),
        (
            CommandAction::NextPatternLocation,
            "next_pattern_location",
            "PositionAircraft Native: Select next traffic-pattern location",
        ),
    ];
    for (action, short_name, description) in definitions {
        let command = Command::create(
            &format!("PositionAircraftNative/{short_name}"),
            description,
            Some(command_handler),
            true,
            action as usize,
        )?;
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
        ("Position Pattern Location", CommandAction::PositionPattern),
    ];
    for (label, action) in labels {
        if let Some(command) = state
            .commands
            .iter()
            .find(|registered| registered.action == action)
            .map(|registered| &registered.command)
        {
            menu.append_command(label, command)?;
        }
    }
    state.menu = Some(menu);
    Ok(())
}

pub(super) fn unregister(state: &mut PluginState) {
    state.menu = None;
    state.commands.clear();
}
