use std::ffi::{c_int, CString};
use std::ptr;

use xplane_plugin::c_string;
use xplane_sdk_sys::*;

use super::config::ShowDuration;
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
    root: XPWidgetID,
    custom: XPWidgetID,
    lines: Vec<CString>,
    width: i32,
    timer: HideTimer,
    widget_in_vr: bool,
}

impl Default for OverlayWindow {
    fn default() -> Self {
        Self {
            root: ptr::null_mut(),
            custom: ptr::null_mut(),
            lines: Vec::new(),
            width: STANDARD_WIDTH,
            timer: HideTimer::Hidden,
            widget_in_vr: false,
        }
    }
}

impl OverlayWindow {
    pub(super) fn is_visible(&self) -> bool {
        !self.root.is_null() && self.timer != HideTimer::Hidden
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
        self.width = lines
            .iter()
            .map(|line| {
                let line = c_string(line);
                // SAFETY: the string pointer is live for this call and its byte length is valid.
                unsafe {
                    XPLMMeasureString(xplmFont_Basic, line.as_ptr(), line.as_bytes().len() as i32)
                }
                .ceil() as i32
                    + 2 * SIDE_MARGIN
            })
            .max()
            .unwrap_or(STANDARD_WIDTH)
            .max(STANDARD_WIDTH);
        force_visible(window_x, window_y, self.width);
        self.ensure_created(*window_x, *window_y);
        self.set_geometry(*window_x, *window_y);
        self.set_vr(in_vr, window_x, window_y);
        // SAFETY: `root` is a live widget created by this instance.
        unsafe {
            XPShowWidget(self.root);
            XPBringRootWidgetToFront(self.root);
        }
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
        let width = lines
            .iter()
            .map(|line| {
                let line = c_string(line);
                // SAFETY: the string pointer is live for this call.
                unsafe {
                    XPLMMeasureString(xplmFont_Basic, line.as_ptr(), line.as_bytes().len() as i32)
                }
                .ceil() as i32
                    + 2 * SIDE_MARGIN
            })
            .max()
            .unwrap_or(STANDARD_WIDTH)
            .max(STANDARD_WIDTH);
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
        if self.root.is_null() {
            self.timer = HideTimer::Hidden;
            return;
        }
        if !self.widget_in_vr {
            // SAFETY: `root` is a live widget and the two output pointers are writable.
            unsafe {
                XPGetWidgetGeometry(
                    self.root,
                    window_x,
                    window_y,
                    ptr::null_mut(),
                    ptr::null_mut(),
                )
            };
        }
        // SAFETY: `root` is a live widget.
        unsafe { XPHideWidget(self.root) };
        self.timer = HideTimer::Hidden;
    }

    pub(super) fn set_vr(&mut self, in_vr: bool, window_x: &mut i32, window_y: &mut i32) {
        if self.root.is_null() || self.widget_in_vr == in_vr {
            return;
        }
        // SAFETY: `root` is a live modern widget and the returned window belongs to it.
        let window = unsafe { XPGetWidgetUnderlyingWindow(self.root) };
        if window.is_null() {
            return;
        }
        // SAFETY: `window` is the live underlying XPLM window.
        unsafe {
            XPLMSetWindowPositioningMode(
                window,
                if in_vr {
                    xplm_WindowVR
                } else {
                    xplm_WindowPositionFree
                },
                -1,
            )
        };
        self.widget_in_vr = in_vr;
        if !in_vr {
            force_visible(window_x, window_y, self.width);
            self.set_geometry(*window_x, *window_y);
        }
    }

    pub(super) fn destroy(&mut self, window_x: &mut i32, window_y: &mut i32) {
        if self.root.is_null() {
            return;
        }
        self.hide(window_x, window_y);
        // SAFETY: `root` is live; passing 1 recursively destroys `custom` too.
        unsafe { XPDestroyWidget(self.root, 1) };
        self.root = ptr::null_mut();
        self.custom = ptr::null_mut();
        self.lines.clear();
    }

    fn ensure_created(&mut self, left: i32, top: i32) {
        if !self.root.is_null() {
            return;
        }
        let title = c_string("Landing Speed Rust 3.46.1");
        let empty = c_string("");
        // SAFETY: descriptors are live during creation and geometry is valid.
        unsafe {
            self.root = XPCreateWidget(
                left,
                top,
                left + self.width,
                top - WINDOW_HEIGHT,
                0,
                title.as_ptr(),
                1,
                ptr::null_mut(),
                xpWidgetClass_MainWindow as i32,
            );
            XPSetWidgetProperty(
                self.root,
                xpProperty_MainWindowType,
                xpMainWindowStyle_Translucent as isize,
            );
            XPSetWidgetProperty(self.root, xpProperty_MainWindowHasCloseBoxes, 1);
            XPAddWidgetCallback(self.root, Some(widget_callback));
            self.custom = XPCreateCustomWidget(
                left + SIDE_MARGIN,
                top - 20,
                left + self.width - SIDE_MARGIN,
                top - WINDOW_HEIGHT,
                1,
                empty.as_ptr(),
                0,
                self.root,
                Some(widget_callback),
            );
        }
    }

    fn set_geometry(&self, left: i32, top: i32) {
        if self.root.is_null() {
            return;
        }
        // SAFETY: both widgets are live and the geometry follows X-Plane's global convention.
        unsafe {
            XPSetWidgetGeometry(self.root, left, top, left + self.width, top - WINDOW_HEIGHT);
            XPSetWidgetGeometry(
                self.custom,
                left + SIDE_MARGIN,
                top - 20,
                left + self.width - SIDE_MARGIN,
                top - WINDOW_HEIGHT,
            );
        }
    }

    pub(super) fn draw(&self) {
        if self.custom.is_null() {
            return;
        }
        let mut left = 0;
        let mut top = 0;
        // SAFETY: `custom` is live and output pointers are writable.
        unsafe {
            XPGetWidgetGeometry(
                self.custom,
                &mut left,
                &mut top,
                ptr::null_mut(),
                ptr::null_mut(),
            )
        };
        let mut color = [1.0_f32, 1.0, 1.0];
        for (index, line) in self.lines.iter().enumerate() {
            // SAFETY: line and color pointers are live for this immediate draw call.
            unsafe {
                XPLMDrawString(
                    color.as_mut_ptr(),
                    left,
                    top - (index as i32 + 1) * TEXT_LINE_HEIGHT,
                    line.as_ptr(),
                    ptr::null_mut(),
                    xplmFont_Basic,
                )
            };
        }
    }
}

fn force_visible(window_x: &mut i32, window_y: &mut i32, width: i32) {
    let (mut left, mut top, mut right, mut bottom) = (0, 0, 0, 0);
    // SAFETY: all four output pointers are writable.
    unsafe { XPLMGetScreenBoundsGlobal(&mut left, &mut top, &mut right, &mut bottom) };
    *window_x = (*window_x).clamp(left, (right - width).max(left));
    *window_y = (*window_y).clamp((bottom + WINDOW_HEIGHT).min(top), top);
}

unsafe extern "C" fn widget_callback(
    message: XPWidgetMessage,
    widget: XPWidgetID,
    _parameter1: isize,
    _parameter2: isize,
) -> c_int {
    if message == xpMessage_CloseButtonPushed {
        let mut handled = false;
        with_state_mut(|state: &mut PluginState| {
            if widget == state.overlay_root() {
                state.hide_overlay();
                handled = true;
            }
        });
        return c_int::from(handled);
    }
    if message == xpMsg_Draw {
        let mut handled = false;
        with_state_mut(|state: &mut PluginState| {
            if widget == state.overlay_custom() {
                state.draw_overlay();
                handled = true;
            }
        });
        return c_int::from(handled);
    }
    0
}

impl OverlayWindow {
    pub(super) fn root(&self) -> XPWidgetID {
        self.root
    }

    pub(super) fn custom(&self) -> XPWidgetID {
        self.custom
    }
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
