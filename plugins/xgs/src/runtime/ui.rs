use std::ffi::{c_int, CString};

use xplane_plugin::{
    c_string, draw_string, measure_string, screen_bounds, Bounds, WidgetWindow, WindowPosition,
};
use xplane_sdk_sys::{
    xpMessage_CloseButtonPushed, xpMsg_Draw, xplmFont_Basic, XPWidgetID, XPWidgetMessage,
};

use super::config::ShowDuration;
use super::support::log;
use super::{with_state_mut, PluginState};

const STANDARD_WIDTH: i32 = 185;
const WINDOW_HEIGHT: i32 = 170;
const SIDE_MARGIN: i32 = 10;
const TEXT_LINE_HEIGHT: i32 = 15;

#[derive(Copy, Clone, Debug, PartialEq)]
enum HideTimer {
    Hidden,
    Seconds(f32),
    UntilClosed,
}

pub(super) struct OverlayWindow {
    window: Option<WidgetWindow>,
    lines: Vec<CString>,
    width: i32,
    timer: HideTimer,
    widget_in_vr: bool,
}

impl Default for OverlayWindow {
    fn default() -> Self {
        Self {
            window: None,
            lines: Vec::new(),
            width: STANDARD_WIDTH,
            timer: HideTimer::Hidden,
            widget_in_vr: false,
        }
    }
}

impl OverlayWindow {
    pub(super) fn is_visible(&self) -> bool {
        self.window.is_some() && self.timer != HideTimer::Hidden
    }

    pub(super) fn show(
        &mut self,
        lines: Vec<String>,
        window_x: &mut i32,
        window_y: &mut i32,
        duration: ShowDuration,
        in_vr: bool,
    ) {
        self.lines = lines.iter().map(|line| c_string(line)).collect();
        self.width = measured_width(&self.lines);
        force_visible(window_x, window_y, self.width);
        if !self.ensure_created(*window_x, *window_y) {
            self.timer = HideTimer::Hidden;
            return;
        }
        self.set_geometry(*window_x, *window_y);
        self.set_vr(in_vr, window_x, window_y);
        self.window.as_ref().expect("window was created").show();
        self.timer = match duration {
            ShowDuration::Seconds(seconds) => HideTimer::Seconds(seconds),
            ShowDuration::UntilClosed => HideTimer::UntilClosed,
        };
    }

    pub(super) fn update_lines(&mut self, lines: Vec<String>, window_x: i32, window_y: i32) {
        if !self.is_visible() {
            return;
        }
        self.lines = lines.iter().map(|line| c_string(line)).collect();
        let width = measured_width(&self.lines);
        if width != self.width {
            self.width = width;
            self.set_geometry(window_x, window_y);
        }
    }

    pub(super) fn tick(&mut self, elapsed: f32, teleported: bool) -> bool {
        if teleported && matches!(self.timer, HideTimer::Seconds(_)) {
            self.timer = HideTimer::Hidden;
            return true;
        }
        if let HideTimer::Seconds(seconds) = &mut self.timer {
            *seconds -= elapsed;
            if *seconds <= 0.0 {
                self.timer = HideTimer::Hidden;
                return true;
            }
        }
        false
    }

    pub(super) fn hide(&mut self, window_x: &mut i32, window_y: &mut i32) {
        let Some(window) = self.window.as_ref() else {
            self.timer = HideTimer::Hidden;
            return;
        };
        if !self.widget_in_vr {
            let geometry = window.root_geometry();
            *window_x = geometry.left;
            *window_y = geometry.top;
        }
        window.hide();
        self.timer = HideTimer::Hidden;
    }

    pub(super) fn set_vr(&mut self, in_vr: bool, window_x: &mut i32, window_y: &mut i32) {
        let Some(window) = self.window.as_ref() else {
            return;
        };
        if self.widget_in_vr == in_vr {
            return;
        }
        if !window.set_position(if in_vr {
            WindowPosition::Vr
        } else {
            WindowPosition::Free
        }) {
            return;
        }
        self.widget_in_vr = in_vr;
        if !in_vr {
            force_visible(window_x, window_y, self.width);
            self.set_geometry(*window_x, *window_y);
        }
    }

    pub(super) fn destroy(&mut self, window_x: &mut i32, window_y: &mut i32) {
        if self.window.is_none() {
            return;
        }
        self.hide(window_x, window_y);
        self.window.take();
        self.lines.clear();
    }

    pub(super) fn draw(&self) {
        let Some(window) = self.window.as_ref() else {
            return;
        };
        let geometry = window.content_geometry();
        for (index, line) in self.lines.iter().enumerate() {
            draw_string(
                [1.0, 1.0, 1.0],
                geometry.left,
                geometry.top - (index as i32 + 1) * TEXT_LINE_HEIGHT,
                line.as_c_str(),
                xplmFont_Basic,
            );
        }
    }

    pub(super) fn is_root(&self, widget: XPWidgetID) -> bool {
        self.window
            .as_ref()
            .is_some_and(|window| window.is_root(widget))
    }

    pub(super) fn is_content(&self, widget: XPWidgetID) -> bool {
        self.window
            .as_ref()
            .is_some_and(|window| window.is_content(widget))
    }

    fn ensure_created(&mut self, left: i32, top: i32) -> bool {
        if self.window.is_some() {
            return true;
        }
        match WidgetWindow::create_translucent(
            "Landing Speed Rust 3.46.1",
            Bounds::new(left, top, left + self.width, top - WINDOW_HEIGHT),
            Bounds::new(
                left + SIDE_MARGIN,
                top - 20,
                left + self.width - SIDE_MARGIN,
                top - WINDOW_HEIGHT,
            ),
            Some(widget_callback),
        ) {
            Ok(window) => {
                self.window = Some(window);
                true
            }
            Err(error) => {
                log(&format!("overlay window creation failed: {error}"));
                false
            }
        }
    }

    fn set_geometry(&self, left: i32, top: i32) {
        let Some(window) = self.window.as_ref() else {
            return;
        };
        window.set_geometry(
            Bounds::new(left, top, left + self.width, top - WINDOW_HEIGHT),
            Bounds::new(
                left + SIDE_MARGIN,
                top - 20,
                left + self.width - SIDE_MARGIN,
                top - WINDOW_HEIGHT,
            ),
        );
    }
}

fn measured_width(lines: &[CString]) -> i32 {
    lines
        .iter()
        .map(|line| measure_string(xplmFont_Basic, line.as_c_str()).ceil() as i32 + 2 * SIDE_MARGIN)
        .max()
        .unwrap_or(STANDARD_WIDTH)
        .max(STANDARD_WIDTH)
}

fn force_visible(window_x: &mut i32, window_y: &mut i32, width: i32) {
    let screen = screen_bounds();
    *window_x = (*window_x).clamp(screen.left, (screen.right - width).max(screen.left));
    *window_y = (*window_y).clamp((screen.bottom + WINDOW_HEIGHT).min(screen.top), screen.top);
}

extern "C" fn widget_callback(
    message: XPWidgetMessage,
    widget: XPWidgetID,
    _parameter1: isize,
    _parameter2: isize,
) -> c_int {
    if message == xpMessage_CloseButtonPushed {
        let mut handled = false;
        with_state_mut(|state: &mut PluginState| {
            if state.is_overlay_root(widget) {
                state.hide_overlay();
                handled = true;
            }
        });
        return c_int::from(handled);
    }
    if message == xpMsg_Draw {
        let mut handled = false;
        with_state_mut(|state: &mut PluginState| {
            if state.is_overlay_content(widget) {
                state.draw_overlay();
                handled = true;
            }
        });
        return c_int::from(handled);
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timed_overlay_expires() {
        let mut window = OverlayWindow {
            timer: HideTimer::Seconds(10.0),
            ..OverlayWindow::default()
        };
        assert!(!window.tick(9.9, false));
        assert!(window.tick(0.2, false));
        assert_eq!(window.timer, HideTimer::Hidden);
    }

    #[test]
    fn until_closed_does_not_expire() {
        let mut window = OverlayWindow {
            timer: HideTimer::UntilClosed,
            ..OverlayWindow::default()
        };
        assert!(!window.tick(1000.0, false));
    }
}
