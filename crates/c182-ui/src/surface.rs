//! Typed X-Plane rendering and input resource for management and notice pages.
use crate::{
    theme,
    view::{Action, Page, ViewState},
};
use std::collections::VecDeque;
use xplane_egui::{Geometry, Surface};
use xplane_sdk_sys::{XPLMDebugString, XPLMWindowID};

pub struct UiSurface {
    surface: Surface,
    view: ViewState,
    actions: VecDeque<Action>,
    last_error: String,
    last_window: XPLMWindowID,
}
impl Default for UiSurface {
    fn default() -> Self {
        Self::new()
    }
}
impl UiSurface {
    pub fn new() -> Self {
        let surface = Surface::new();
        theme::apply(&surface.context);
        Self {
            surface,
            view: ViewState::default(),
            actions: VecDeque::new(),
            last_error: String::new(),
            last_window: std::ptr::null_mut(),
        }
    }
    /// Returns 0 for empty geometry, 1 for a paint failure, and 2 on success.
    /// Actions remain queued in FIFO order even when painting fails.
    pub fn draw(&mut self, window: XPLMWindowID, geometry: Geometry, page: &Page) -> i32 {
        if geometry.right <= geometry.left || geometry.top <= geometry.bottom {
            return 0;
        }
        self.last_window = window;
        let saved = self.view.clone();
        let mut final_view = saved.clone();
        let output = self.surface.frame(geometry, |ui| {
            let mut next = saved.clone();
            next.show(ui, page);
            final_view = next;
        });
        self.view = final_view;
        self.actions.extend(self.view.take_actions());
        if let Err(error) = self.surface.paint(window, geometry, output) {
            if error != self.last_error {
                self.last_error = error.clone();
                let line = xplane_plugin::c_string(&format!("C182 egui: {error}\n"));
                unsafe {
                    XPLMDebugString(line.as_ptr());
                }
            }
            return 1;
        }
        self.last_error.clear();
        2
    }
    pub fn mouse(&mut self, x: f32, y: f32, phase: i32, button: i32) {
        self.surface.mouse(
            x,
            y,
            phase,
            if button == 1 {
                egui::PointerButton::Secondary
            } else {
                egui::PointerButton::Primary
            },
        );
    }
    pub fn wheel(&mut self, x: f32, y: f32, clicks: i32) {
        self.view.wheel_clicks = (self.view.wheel_clicks + clicks).clamp(-32, 32);
        self.surface.scroll(x, y, clicks);
    }
    pub fn key(&mut self, key: u8, virtual_key: u8, flags: i32, losing: bool) {
        self.surface.key(key, virtual_key, flags, losing);
    }
    pub fn hide(&mut self, window: XPLMWindowID) {
        self.view.release_all();
        self.view.take_actions();
        self.actions.clear();
        self.surface.hide(window);
    }
    pub fn wants_keyboard(&self) -> bool {
        self.surface.wants_keyboard()
    }
    pub fn next_action(&mut self) -> Option<Action> {
        self.actions.pop_front()
    }
}
impl Drop for UiSurface {
    fn drop(&mut self) {
        self.surface.hide(self.last_window);
    }
}
