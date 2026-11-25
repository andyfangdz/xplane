use crate::{Geometry, PanelPainter};
use egui::{
    Event, Key, Modifiers, MouseWheelUnit, PointerButton, Pos2, RawInput, Rect, TouchPhase, vec2,
};
use std::{
    mem, ptr,
    thread::{self, ThreadId},
    time::Instant,
};
use xplane_sdk_sys::*;

pub struct Surface {
    pub context: egui::Context,
    painter: PanelPainter,
    events: Vec<Event>,
    modifiers: Modifiers,
    pointer: Pos2,
    captured: [bool; 2],
    focused: bool,
    start: Instant,
    owner: ThreadId,
    pixels_per_point: f32,
}
impl Default for Surface {
    fn default() -> Self {
        Self::new()
    }
}
impl Surface {
    pub fn new() -> Self {
        Self {
            context: egui::Context::default(),
            painter: PanelPainter::default(),
            events: Vec::new(),
            modifiers: Modifiers::NONE,
            pointer: Pos2::ZERO,
            captured: [false; 2],
            focused: false,
            start: Instant::now(),
            owner: thread::current().id(),
            pixels_per_point: 1.,
        }
    }
    /// Device pixels per boxel used for font rasterization in the current frame.
    pub fn pixels_per_point(&self) -> f32 {
        self.pixels_per_point
    }
    pub fn mouse(&mut self, x: f32, y: f32, phase: i32, button: PointerButton) {
        if thread::current().id() != self.owner {
            return;
        }
        self.pointer = Pos2::new(x, y);
        self.events.push(Event::PointerMoved(self.pointer));
        let index = usize::from(button == PointerButton::Secondary);
        if phase == 1 {
            self.captured[index] = true;
            self.events.push(Event::PointerButton {
                pos: self.pointer,
                button,
                pressed: true,
                modifiers: self.modifiers,
            });
        }
        if phase == 3 && self.captured[index] {
            self.captured[index] = false;
            self.events.push(Event::PointerButton {
                pos: self.pointer,
                button,
                pressed: false,
                modifiers: self.modifiers,
            });
        }
    }
    pub fn scroll(&mut self, x: f32, y: f32, clicks: i32) {
        if thread::current().id() != self.owner {
            return;
        }
        self.pointer = Pos2::new(x, y);
        self.events.push(Event::PointerMoved(self.pointer));
        self.events.push(Event::MouseWheel {
            unit: MouseWheelUnit::Line,
            delta: vec2(0., clicks as f32),
            phase: TouchPhase::Move,
            modifiers: self.modifiers,
        });
    }
    pub fn key(&mut self, key: u8, virtual_key: u8, flags: i32, losing: bool) {
        if losing {
            self.events.push(Event::WindowFocused(false));
            self.context.memory_mut(|m| m.stop_text_input());
            self.focused = false;
            return;
        }
        self.modifiers = modifiers_from_flags(flags);
        self.events.push(Event::ModifiersChanged(self.modifiers));
        let pressed = flags & xplm_DownFlag != 0;
        let released = flags & xplm_UpFlag != 0;
        if !pressed && !released {
            return;
        }
        if let Some(key) = map_key(key, virtual_key) {
            self.events.push(Event::Key {
                key,
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
    // `window` is an opaque SDK handle handed back to X-Plane, never dereferenced here.
    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    pub fn hide(&mut self, window: XPLMWindowID) {
        self.events.clear();
        for (index, button) in [PointerButton::Primary, PointerButton::Secondary]
            .into_iter()
            .enumerate()
        {
            if self.captured[index] {
                self.events.push(Event::PointerButton {
                    pos: self.pointer,
                    button,
                    pressed: false,
                    modifiers: self.modifiers,
                });
            }
        }
        self.events.push(Event::PointerGone);
        self.events.push(Event::WindowFocused(false));
        self.captured = [false; 2];
        self.context.memory_mut(|m| m.stop_text_input());
        if self.focused && !window.is_null() && unsafe { XPLMHasKeyboardFocus(window) } != 0 {
            unsafe {
                XPLMTakeKeyboardFocus(ptr::null_mut());
            }
        }
        self.focused = false;
    }
    pub fn wants_keyboard(&self) -> bool {
        self.context.egui_wants_keyboard_input()
    }
    /// Lay out a live window at its current font rasterization density.
    pub fn window_frame(
        &mut self,
        window: XPLMWindowID,
        geometry: Geometry,
        build: impl FnMut(&mut egui::Ui),
    ) -> egui::FullOutput {
        self.pixels_per_point = self.painter.pixels_per_point(window, geometry);
        self.frame(geometry, build)
    }
    pub fn frame(
        &mut self,
        geometry: Geometry,
        mut build: impl FnMut(&mut egui::Ui),
    ) -> egui::FullOutput {
        // The screen is the window in boxels (egui points); the scale tells
        // egui how many device pixels each one covers.
        let mut raw = RawInput {
            screen_rect: Some(Rect::from_min_size(
                Pos2::ZERO,
                vec2(geometry.width() as f32, geometry.height() as f32),
            )),
            time: Some(self.start.elapsed().as_secs_f64()),
            events: mem::take(&mut self.events),
            focused: true,
            ..Default::default()
        };
        PanelPainter::set_input_scale(&mut raw, self.pixels_per_point);
        self.context.run_ui(raw, |ui| build(ui))
    }
    // As in `hide`: the window handle is only passed back to the SDK.
    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    pub fn paint(
        &mut self,
        window: XPLMWindowID,
        geometry: Geometry,
        mut output: egui::FullOutput,
    ) -> Result<(), String> {
        if thread::current().id() != self.owner {
            output.textures_delta.clear();
            return Err("egui requires its owner thread".into());
        }
        let keyboard = self.context.egui_wants_keyboard_input();
        if keyboard != self.focused {
            self.focused = keyboard;
            unsafe {
                if keyboard {
                    XPLMTakeKeyboardFocus(window);
                } else if XPLMHasKeyboardFocus(window) != 0 {
                    XPLMTakeKeyboardFocus(ptr::null_mut());
                }
            }
        }
        self.painter
            .paint(&self.context, geometry, output)
            .map_err(String::from)
    }
}
pub fn modifiers_from_flags(flags: i32) -> Modifiers {
    let ctrl = flags & xplm_ControlFlag != 0;
    Modifiers {
        alt: flags & xplm_OptionAltFlag != 0,
        ctrl,
        shift: flags & xplm_ShiftFlag != 0,
        mac_cmd: false,
        command: ctrl,
    }
}
pub fn map_key(key: u8, v: u8) -> Option<Key> {
    match v {
        8 => Some(Key::Backspace),
        9 => Some(Key::Tab),
        13 => Some(Key::Enter),
        27 => Some(Key::Escape),
        33 => Some(Key::PageUp),
        34 => Some(Key::PageDown),
        35 => Some(Key::End),
        36 => Some(Key::Home),
        37 => Some(Key::ArrowLeft),
        38 => Some(Key::ArrowUp),
        39 => Some(Key::ArrowRight),
        40 => Some(Key::ArrowDown),
        45 => Some(Key::Insert),
        46 | 127 => Some(Key::Delete),
        b'0'..=b'9' | b'A'..=b'Z' => Key::from_name(&(v as char).to_string()),
        _ => Key::from_name(&(key as char).to_string()),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn geometry() -> Geometry {
        Geometry {
            left: 0,
            top: 300,
            right: 400,
            bottom: 0,
        }
    }
    #[test]
    fn input_from_another_thread_is_dropped() {
        let mut s = Surface::new();
        s.owner = thread::spawn(|| thread::current().id()).join().unwrap();
        s.scroll(10., 10., 3);
        s.mouse(10., 10., 1, PointerButton::Primary);
        assert!(s.events.is_empty());
        s.owner = thread::current().id();
        s.scroll(10., 10., 3);
        assert_eq!(s.events.len(), 2);
    }
    #[test]
    fn a_frame_lays_out_in_boxels_at_the_measured_scale() {
        let mut s = Surface::new();
        let mut size = egui::Vec2::ZERO;
        let mut output = s.frame(geometry(), |ui| size = ui.ctx().content_rect().size());
        output.textures_delta.clear();
        assert_eq!(output.pixels_per_point, 1.);
        s.pixels_per_point = 1.5;
        let mut output = s.frame(geometry(), |ui| size = ui.ctx().content_rect().size());
        output.textures_delta.clear();
        assert_eq!(output.pixels_per_point, 1.5);
        assert_eq!(size, vec2(400., 300.));
    }
}
