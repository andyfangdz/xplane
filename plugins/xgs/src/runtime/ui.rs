use std::ffi::CString;

use xplane_plugin::{
    CallbackWindow, WindowHandler, WindowParams, c_string,
    panel_ui::{Font, fill_rect},
    screen_bounds,
    xplm::LiveWindow,
};
use xplane_sdk_sys as xp;

use super::config::ShowDuration;
use super::support::log;
use super::with_state_mut;

const STANDARD_WIDTH: i32 = 185;
const WINDOW_HEIGHT: i32 = 170;
const HEADER_HEIGHT: i32 = 20;
const SIDE_MARGIN: i32 = 10;
const TEXT_LINE_HEIGHT: i32 = 15;
const TITLE: &std::ffi::CStr = c"Landing Speed Rust 3.46.1";

#[derive(Copy, Clone, Debug, PartialEq)]
enum HideTimer {
    Hidden,
    Seconds(f32),
    UntilClosed,
}

#[derive(Copy, Clone, Debug, PartialEq)]
enum Header {
    CloseLeft,
    CloseRight,
    Title,
}

fn header([left, top, right, _]: [i32; 4], x: i32, y: i32) -> Option<Header> {
    if x < left || x >= right || y <= top - HEADER_HEIGHT || y > top {
        None
    } else if x < left + 26 {
        Some(Header::CloseLeft)
    } else if x >= right - 26 {
        Some(Header::CloseRight)
    } else {
        Some(Header::Title)
    }
}

enum Press {
    Move { x_offset: i32, y_offset: i32 },
    Close(Header),
}

struct Panel {
    window: CallbackWindow,
    report: Font,
    title: Font,
}

struct Handler;

impl WindowHandler for Handler {
    fn draw(&self, _: xp::XPLMWindowID) {
        with_state_mut(|state| state.overlay.draw());
    }

    fn click(&self, _: xp::XPLMWindowID, x: i32, y: i32, status: i32) -> i32 {
        with_state_mut(|state| {
            if state.overlay.click(x, y, status) {
                state.hide_overlay();
            }
        });
        1
    }
}

pub(super) struct OverlayWindow {
    panel: Option<Panel>,
    lines: Vec<CString>,
    width: i32,
    timer: HideTimer,
    in_vr: bool,
    press: Option<Press>,
}

impl Default for OverlayWindow {
    fn default() -> Self {
        Self {
            panel: None,
            lines: Vec::new(),
            width: STANDARD_WIDTH,
            timer: HideTimer::Hidden,
            in_vr: false,
            press: None,
        }
    }
}

impl OverlayWindow {
    pub(super) fn is_visible(&self) -> bool {
        self.panel.is_some() && self.timer != HideTimer::Hidden
    }

    pub(super) fn show(
        &mut self,
        lines: Vec<String>,
        window_x: &mut i32,
        window_y: &mut i32,
        duration: ShowDuration,
        in_vr: bool,
    ) {
        if self.panel.is_none() {
            self.panel = Self::create_panel(*window_x, *window_y);
        }
        if self.panel.is_none() {
            log("could not create the landing overlay or load its X-Plane fonts");
            return;
        }
        self.set_lines(lines);
        force_visible(window_x, window_y, self.width);
        self.set_vr(in_vr, window_x, window_y);
        self.set_geometry(*window_x, *window_y);
        let live = LiveWindow::of(&self.panel.as_ref().unwrap().window);
        live.set_visible(1);
        live.bring_to_front();
        self.timer = match duration {
            ShowDuration::Seconds(seconds) => HideTimer::Seconds(seconds),
            ShowDuration::UntilClosed => HideTimer::UntilClosed,
        };
    }

    fn create_panel(left: i32, top: i32) -> Option<Panel> {
        // The stock XGS capture has six-boxel Inconsolata cells, 15-boxel
        // row spacing and a 144-boxel Roboto title. Use those advances even
        // when the SDK's size convention differs from the TTF em size.
        let report =
            Font::load(c"Resources/fonts/Inconsolata.ttf", 12.)?.fit_width(c"00000000", 48.)?;
        let title =
            Font::load(c"Resources/fonts/Roboto-Regular.ttf", 12.)?.fit_width(TITLE, 144.)?;
        let params = WindowParams {
            bounds: [left, top, left + STANDARD_WIDTH, top - WINDOW_HEIGHT],
            visible: false,
            decoration: xp::xplm_WindowDecorationSelfDecorated,
            layer: xp::xplm_WindowLayerFloatingWindows,
        };
        let window = CallbackWindow::create_panel(params, "XGS landing report", Handler)?;
        Some(Panel {
            window,
            report,
            title,
        })
    }

    fn set_lines(&mut self, lines: Vec<String>) {
        let panel = self.panel.as_ref().unwrap();
        self.lines = lines.iter().map(|line| c_string(line)).collect();
        self.width = self
            .lines
            .iter()
            .map(|line| panel.report.measure(line).ceil() as i32 + 2 * SIDE_MARGIN)
            .max()
            .unwrap_or(STANDARD_WIDTH)
            .max(STANDARD_WIDTH);
    }

    pub(super) fn update_lines(&mut self, lines: Vec<String>) {
        if !self.is_visible() {
            return;
        }
        let old_width = self.width;
        self.set_lines(lines);
        if self.width != old_width {
            let [mut x, mut y, _, _] =
                LiveWindow::of(&self.panel.as_ref().unwrap().window).geometry();
            force_visible(&mut x, &mut y, self.width);
            self.set_geometry(x, y);
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
        if let Some(panel) = &self.panel {
            let live = LiveWindow::of(&panel.window);
            if !self.in_vr {
                [*window_x, *window_y, _, _] = live.geometry();
            }
            live.set_visible(0);
        }
        self.press = None;
        self.timer = HideTimer::Hidden;
    }

    pub(super) fn set_vr(&mut self, in_vr: bool, window_x: &mut i32, window_y: &mut i32) {
        let Some(panel) = &self.panel else { return };
        if self.in_vr == in_vr {
            return;
        }
        let live = LiveWindow::of(&panel.window);
        if in_vr {
            [*window_x, *window_y, _, _] = live.geometry();
            live.set_positioning_mode(xp::xplm_WindowVR, -1);
        } else {
            live.set_positioning_mode(xp::xplm_WindowPositionFree, -1);
            force_visible(window_x, window_y, self.width);
        }
        self.press = None;
        self.in_vr = in_vr;
        self.set_geometry(*window_x, *window_y);
    }

    pub(super) fn destroy(&mut self, window_x: &mut i32, window_y: &mut i32) {
        self.hide(window_x, window_y);
        self.panel.take();
        self.in_vr = false;
        self.lines.clear();
    }

    fn draw(&self) {
        let Some(panel) = &self.panel else { return };
        let bounds @ [left, top, right, _] = LiveWindow::of(&panel.window).geometry();
        fill_rect(bounds, [0, 0, 0, 128]);
        let mid = (left + (right - left) / 2) as f32;
        panel.title.draw(
            [mid, (top - 14) as f32],
            TITLE,
            [210, 210, 210, 255],
            xp::xplm_JustCenter,
        );
        // The reference widget has a close cross at both ends of the title bar.
        for x in [left + 15, right - 15] {
            let v = |dx, dy| xp::XPLMVertex_t {
                x: (x + dx) as f32,
                y: (top - 10 + dy) as f32,
            };
            let vertices = [v(-4, -4), v(4, 4), v(-4, 4), v(4, -4)];
            // SAFETY: the native drawing callback supplies two line segments.
            unsafe { xp::XPLMLinesWithWidth(0xffdddddd, 2., vertices.as_ptr(), 4) };
        }
        for (index, line) in self.lines.iter().enumerate() {
            let baseline = top - HEADER_HEIGHT - (index as i32 + 1) * TEXT_LINE_HEIGHT;
            panel.report.draw(
                [(left + SIDE_MARGIN) as f32, baseline as f32],
                line,
                [255; 4],
                xp::xplm_JustLeft,
            );
        }
    }

    /// Returns true on release over the same close box that received mouse-down.
    fn click(&mut self, x: i32, y: i32, status: i32) -> bool {
        let Some(panel) = &self.panel else {
            return false;
        };
        let live = LiveWindow::of(&panel.window);
        let bounds @ [left, top, _, _] = live.geometry();
        match status {
            xp::xplm_MouseDown => {
                live.bring_to_front();
                self.press = match header(bounds, x, y) {
                    Some(Header::Title) if !self.in_vr => Some(Press::Move {
                        x_offset: x - left,
                        y_offset: y - top,
                    }),
                    Some(h @ (Header::CloseLeft | Header::CloseRight)) => Some(Press::Close(h)),
                    _ => None,
                };
            }
            xp::xplm_MouseDrag | xp::xplm_MouseUp => {
                if let Some(Press::Move { x_offset, y_offset }) = self.press {
                    let (mut l, mut t) = (x - x_offset, y - y_offset);
                    force_visible(&mut l, &mut t, self.width);
                    self.set_geometry(l, t);
                }
                if status == xp::xplm_MouseUp {
                    return matches!(self.press.take(), Some(Press::Close(h)) if Some(h) == header(bounds, x, y));
                }
            }
            _ => (),
        }
        false
    }

    fn set_geometry(&self, left: i32, top: i32) {
        let Some(panel) = &self.panel else { return };
        let live = LiveWindow::of(&panel.window);
        if self.in_vr {
            live.set_geometry_vr(self.width, WINDOW_HEIGHT);
        } else {
            live.set_geometry(left, top, left + self.width, top - WINDOW_HEIGHT);
        }
    }
}

fn force_visible(window_x: &mut i32, window_y: &mut i32, width: i32) {
    let screen = screen_bounds();
    *window_x = (*window_x).clamp(screen.left, (screen.right - width).max(screen.left));
    *window_y = (*window_y).clamp((screen.bottom + WINDOW_HEIGHT).min(screen.top), screen.top);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_close_boxes_and_drag_strip_follow_window_on_an_offset_monitor() {
        let bounds = [-575, 540, -300, 370];
        assert_eq!(header(bounds, -560, 530), Some(Header::CloseLeft));
        assert_eq!(header(bounds, -315, 530), Some(Header::CloseRight));
        assert_eq!(header(bounds, -400, 530), Some(Header::Title));
        assert_eq!(header(bounds, -400, 515), None);
    }

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
