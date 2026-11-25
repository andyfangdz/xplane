use std::ffi::{c_char, c_int, c_void};
use std::mem;
use std::time::Instant;
use xplane_egui::{Geometry as WindowGeometry, PanelPainter, map_key, modifiers_from_flags};

use egui::{
    Event, Modifiers, MouseWheelUnit, PointerButton, Pos2, RawInput, Rect, TouchPhase, vec2,
};

use crate::runtime::support::log;
use crate::runtime::{CommandAction, PluginState, with_state_mut};
use xplane_sdk_sys::*;

use super::theme;
use super::view::{self, Action, HitCursor, HitRegion};

pub(in crate::runtime) struct EguiIntegration {
    context: egui::Context,
    painter: PanelPainter,
    events: Vec<Event>,
    modifiers: Modifiers,
    pointer_pos: Pos2,
    hit_regions: Vec<HitRegion>,
    popup_open: bool,
    mouse_captured: bool,
    keyboard_focused: bool,
    started_at: Instant,
    renderer_error: Option<String>,
}

impl EguiIntegration {
    pub(in crate::runtime) fn new() -> Self {
        let context = egui::Context::default();
        theme::apply(&context);
        Self {
            context,
            painter: PanelPainter::default(),
            events: Vec::new(),
            modifiers: Modifiers::NONE,
            pointer_pos: Pos2::ZERO,
            hit_regions: Vec::new(),
            popup_open: false,
            mouse_captured: false,
            keyboard_focused: false,
            started_at: Instant::now(),
            renderer_error: None,
        }
    }

    pub(in crate::runtime) fn hide(&mut self) {
        self.events.push(Event::PointerGone);
        self.events.push(Event::WindowFocused(false));
        self.context.memory_mut(|memory| memory.stop_text_input());
        self.hit_regions.clear();
        self.popup_open = false;
        self.mouse_captured = false;
        self.keyboard_focused = false;
    }

    fn hit_cursor(&self, position: Pos2) -> Option<HitCursor> {
        self.hit_regions
            .iter()
            .rev()
            .find(|region| region.rect.contains(position))
            .map(|region| region.cursor)
    }

    fn pointer_moved(&mut self, position: Pos2) {
        self.pointer_pos = position;
        self.events.push(Event::PointerMoved(position));
    }

    fn begin_pointer(&mut self, position: Pos2) -> bool {
        self.pointer_moved(position);
        let handled = self.hit_cursor(position).is_some()
            || self.popup_open
            || self.context.egui_is_using_pointer();
        if handled {
            self.events.push(Event::PointerButton {
                pos: position,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: self.modifiers,
            });
            self.mouse_captured = true;
        }
        handled
    }

    fn continue_pointer(&mut self, position: Pos2, released: bool) -> bool {
        if !self.mouse_captured {
            return false;
        }
        self.pointer_moved(position);
        if released {
            self.events.push(Event::PointerButton {
                pos: position,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: self.modifiers,
            });
            self.mouse_captured = false;
        }
        true
    }

    fn scroll(&mut self, position: Pos2, clicks: i32) -> bool {
        self.pointer_moved(position);
        if self.hit_cursor(position).is_none() && !self.popup_open {
            return false;
        }
        self.events.push(Event::MouseWheel {
            unit: MouseWheelUnit::Line,
            delta: vec2(0.0, clicks as f32),
            phase: TouchPhase::Move,
            modifiers: self.modifiers,
        });
        true
    }

    fn key_event(&mut self, key: u8, virtual_key: u8, flags: i32) {
        self.modifiers = modifiers_from_flags(flags);
        self.events.push(Event::ModifiersChanged(self.modifiers));
        let pressed = flags & xplm_DownFlag != 0;
        let released = flags & xplm_UpFlag != 0;
        if !pressed && !released {
            return;
        }
        if let Some(mapped) = map_key(key, virtual_key) {
            self.events.push(Event::Key {
                key: mapped,
                physical_key: None,
                pressed,
                repeat: false,
                modifiers: self.modifiers,
            });
        }
        if pressed && !self.modifiers.ctrl && !self.modifiers.alt && (32..=126).contains(&key) {
            self.events.push(Event::Text((key as char).to_string()));
        }
    }

    fn lose_keyboard_focus(&mut self) {
        self.events.push(Event::WindowFocused(false));
        self.keyboard_focused = false;
    }

    fn cursor_status(&self, position: Pos2) -> XPLMCursorStatus {
        match self.hit_cursor(position) {
            Some(HitCursor::Text | HitCursor::Arrow) => xplm_CursorArrow,
            None if self.popup_open => xplm_CursorArrow,
            None => xplm_CursorDefault,
        }
    }

    fn draw(&mut self, window_id: XPLMWindowID, state: &mut PluginState) {
        let Some(window) = state.window.as_ref() else {
            return;
        };
        let geometry = WindowGeometry::from(window.geometry());
        if geometry.width() <= 0 || geometry.height() <= 0 {
            return;
        }

        let mut input = RawInput {
            screen_rect: Some(Rect::from_min_size(
                Pos2::ZERO,
                vec2(geometry.width() as f32, geometry.height() as f32),
            )),
            time: Some(self.started_at.elapsed().as_secs_f64()),
            events: mem::take(&mut self.events),
            focused: true,
            ..Default::default()
        };

        let scale = self.painter.pixels_per_point(window_id, geometry);
        PanelPainter::set_input_scale(&mut input, scale);
        let context = self.context.clone();
        let mut view_output = None;
        let full_output = context.run_ui(input, |ui| {
            view_output = Some(view::show(ui, state));
        });
        let view_output = view_output.expect("egui UI closure did not run");
        self.hit_regions = view_output.hit_regions;
        self.popup_open = context.any_popup_open();
        self.update_keyboard_focus(state, context.egui_wants_keyboard_input());
        for action in view_output.actions {
            apply_action(state, action);
        }

        match self.painter.paint(&context, geometry, full_output) {
            Ok(()) => self.renderer_error = None,
            Err(error) if self.renderer_error.as_deref() != Some(error) => {
                self.renderer_error = Some(error.to_owned());
                state.status = format!("UI renderer unavailable: {error}");
                log(&state.status);
            }
            Err(_) => {}
        }
    }

    fn update_keyboard_focus(&mut self, state: &PluginState, wants_keyboard: bool) {
        if wants_keyboard == self.keyboard_focused {
            return;
        }
        self.keyboard_focused = wants_keyboard;
        if let Some(window) = state.window.as_ref() {
            window.set_keyboard_focus(wants_keyboard);
        }
    }
}

fn apply_action(state: &mut PluginState, action: Action) {
    match action {
        Action::Command(CommandAction::CaptureCurrent) => {
            state.capture_current();
        }
        Action::Command(CommandAction::PositionLoaded) => state.position_loaded(),
        Action::Command(CommandAction::QuickSave) => state.quick_save(),
        Action::Command(CommandAction::QuickLoadAndPosition) => state.quick_load(true),
        Action::Command(CommandAction::PreviousPad) => state.select_relative(-1, false),
        Action::Command(CommandAction::NextPad) => state.select_relative(1, false),
        Action::Command(CommandAction::ToggleWindow) => state.toggle_window(),
        Action::Command(CommandAction::QuickLoad) => state.quick_load(false),
        Action::Command(CommandAction::PreviousPadAndPosition) => state.select_relative(-1, true),
        Action::Command(CommandAction::NextPadAndPosition) => state.select_relative(1, true),
        Action::Command(CommandAction::PositionPattern) => state.position_pattern(),
        Action::Command(CommandAction::PreviousPatternLocation) => state.cycle_pattern_location(-1),
        Action::Command(CommandAction::NextPatternLocation) => state.cycle_pattern_location(1),
        Action::LoadSelected(position) => state.load_selected(position),
        Action::Refresh => state.refresh_pads(),
        Action::SelectPad(index) => state.select_pad(index),
        Action::SaveNamed => state.save_named(),
        Action::ResolvePatternAirport => state.resolve_pattern_airport(),
        Action::NearestPatternAirport => state.select_nearest_pattern_airport(),
        Action::SelectPatternPad(filename) => state.select_pattern_pad(filename),
        Action::PatternSettingsChanged => state.pattern_settings_changed(),
        Action::PositionPattern => state.position_pattern(),
        Action::SavePatternPad => state.save_pattern_pad(),
    }
}

pub(in crate::runtime) extern "C" fn draw_window(window: XPLMWindowID, _refcon: *mut c_void) {
    with_state_mut(|state| {
        let Some(mut ui) = state.ui.take() else {
            return;
        };
        ui.draw(window, state);
        state.ui = Some(ui);
    });
}

pub(in crate::runtime) extern "C" fn handle_mouse(
    _window: XPLMWindowID,
    x: c_int,
    y: c_int,
    mouse_status: XPLMMouseStatus,
    _refcon: *mut c_void,
) -> c_int {
    with_state_mut(|state| {
        let Some(window) = state.window.as_ref() else {
            return 0;
        };
        let position = WindowGeometry::from(window.geometry()).local(x, y);
        let Some(ui) = state.ui.as_mut() else {
            return 0;
        };
        let handled = if mouse_status == xplm_MouseDown {
            ui.begin_pointer(position)
        } else if mouse_status == xplm_MouseDrag {
            ui.continue_pointer(position, false)
        } else if mouse_status == xplm_MouseUp {
            ui.continue_pointer(position, true)
        } else {
            false
        };
        i32::from(handled)
    })
    .unwrap_or(0)
}

pub(in crate::runtime) extern "C" fn handle_right_click(
    _window: XPLMWindowID,
    _x: c_int,
    _y: c_int,
    _mouse_status: XPLMMouseStatus,
    _refcon: *mut c_void,
) -> c_int {
    0
}

pub(in crate::runtime) extern "C" fn handle_cursor(
    _window: XPLMWindowID,
    x: c_int,
    y: c_int,
    _refcon: *mut c_void,
) -> XPLMCursorStatus {
    with_state_mut(|state| {
        let Some(window) = state.window.as_ref() else {
            return xplm_CursorDefault;
        };
        let position = WindowGeometry::from(window.geometry()).local(x, y);
        let Some(ui) = state.ui.as_mut() else {
            return xplm_CursorDefault;
        };
        ui.pointer_moved(position);
        ui.cursor_status(position)
    })
    .unwrap_or(xplm_CursorDefault)
}

pub(in crate::runtime) extern "C" fn handle_wheel(
    _window: XPLMWindowID,
    x: c_int,
    y: c_int,
    wheel: c_int,
    clicks: c_int,
    _refcon: *mut c_void,
) -> c_int {
    if wheel != 0 || clicks == 0 {
        return 0;
    }
    with_state_mut(|state| {
        let Some(window) = state.window.as_ref() else {
            return 0;
        };
        let position = WindowGeometry::from(window.geometry()).local(x, y);
        state
            .ui
            .as_mut()
            .map(|ui| i32::from(ui.scroll(position, clicks)))
            .unwrap_or(0)
    })
    .unwrap_or(0)
}

pub(in crate::runtime) extern "C" fn handle_key(
    _window: XPLMWindowID,
    key: c_char,
    flags: XPLMKeyFlags,
    virtual_key: c_char,
    _refcon: *mut c_void,
    losing_focus: c_int,
) {
    with_state_mut(|state| {
        let Some(ui) = state.ui.as_mut() else {
            return;
        };
        if losing_focus != 0 {
            ui.lose_keyboard_focus();
        } else {
            ui.key_event(key as u8, virtual_key as u8, flags);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::map_key;
    use egui::Key;

    #[test]
    fn control_shortcuts_use_the_virtual_key() {
        // XPLM can report an ASCII control character in `key` while retaining
        // the actual letter in `virtual_key`.
        assert_eq!(map_key(1, b'A'), Some(Key::A));
        assert_eq!(map_key(b'k', b'K'), Some(Key::K));
    }
}
