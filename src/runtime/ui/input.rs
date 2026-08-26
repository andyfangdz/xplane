use super::layout::*;
use super::*;

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

pub(in crate::runtime) unsafe extern "C" fn handle_mouse(
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

pub(in crate::runtime) unsafe extern "C" fn handle_right_click(
    _window: XPLMWindowID,
    _x: c_int,
    _y: c_int,
    _mouse_status: XPLMMouseStatus,
    _refcon: *mut c_void,
) -> c_int {
    0
}

pub(in crate::runtime) unsafe extern "C" fn handle_cursor(
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

pub(in crate::runtime) unsafe extern "C" fn handle_wheel(
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

pub(in crate::runtime) unsafe extern "C" fn handle_key(
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
