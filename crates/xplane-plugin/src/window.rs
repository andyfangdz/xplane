use std::ffi::c_void;
use std::mem;
use std::ptr::{self, NonNull};

use xplane_sdk_sys::{
    XPLMCreateWindow_t, XPLMCreateWindowEx, XPLMDestroyWindow, XPLMDrawWindow_f,
    XPLMHandleCursor_f, XPLMHandleKey_f, XPLMHandleMouseClick_f, XPLMHandleMouseWheel_f,
    XPLMWindowDecoration, XPLMWindowLayer, xplm_WindowPositionFree, xplm_WindowVR,
};

use crate::xplm::LiveWindow;
use crate::{Bounds, c_string};

#[derive(Copy, Clone)]
pub struct WindowCallbacks {
    pub draw: XPLMDrawWindow_f,
    pub mouse: XPLMHandleMouseClick_f,
    pub key: XPLMHandleKey_f,
    pub cursor: XPLMHandleCursor_f,
    pub wheel: XPLMHandleMouseWheel_f,
    pub right_click: XPLMHandleMouseClick_f,
}

pub struct WindowConfig {
    pub bounds: Bounds,
    pub visible: bool,
    pub callbacks: WindowCallbacks,
    pub decoration: XPLMWindowDecoration,
    pub layer: XPLMWindowLayer,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum WindowPosition {
    Free,
    Vr,
}

/// An owned modern XPLM window.
///
/// The wrapper must be created, used, and dropped on X-Plane's plugin thread.
pub struct Window {
    handle: NonNull<c_void>,
}

impl Window {
    pub fn create(config: WindowConfig) -> Result<Self, String> {
        let mut params = XPLMCreateWindow_t {
            structSize: mem::size_of::<XPLMCreateWindow_t>() as i32,
            left: config.bounds.left,
            top: config.bounds.top,
            right: config.bounds.right,
            bottom: config.bounds.bottom,
            visible: i32::from(config.visible),
            drawWindowFunc: config.callbacks.draw,
            handleMouseClickFunc: config.callbacks.mouse,
            handleKeyFunc: config.callbacks.key,
            handleCursorFunc: config.callbacks.cursor,
            handleMouseWheelFunc: config.callbacks.wheel,
            refcon: ptr::null_mut(),
            decorateAsFloatingWindow: config.decoration,
            layer: config.layer,
            handleRightClickFunc: config.callbacks.right_click,
        };
        // SAFETY: the structure has the SDK-prescribed size, all callbacks
        // have static code lifetime, and the null refcon needs no lifetime.
        let handle = NonNull::new(unsafe { XPLMCreateWindowEx(&mut params) })
            .ok_or_else(|| "XPLMCreateWindowEx failed".to_owned())?;
        Ok(Self { handle })
    }

    /// The window, live while `self` is borrowed.
    fn live(&self) -> LiveWindow<'_> {
        LiveWindow::of_window(self)
    }

    /// The window handle as an integer.
    pub(crate) fn raw(&self) -> usize {
        self.handle.as_ptr() as usize
    }

    pub fn geometry(&self) -> Bounds {
        let [left, top, right, bottom] = self.live().geometry();
        Bounds {
            left,
            top,
            right,
            bottom,
        }
    }

    pub fn set_geometry(&self, bounds: Bounds) {
        self.live()
            .set_geometry(bounds.left, bounds.top, bounds.right, bounds.bottom);
    }

    pub fn set_resizing_limits(
        &self,
        min_width: i32,
        min_height: i32,
        max_width: i32,
        max_height: i32,
    ) {
        self.live()
            .set_resizing_limits(min_width, min_height, max_width, max_height);
    }

    pub fn set_title(&self, title: &str) {
        self.live().set_title(&c_string(title));
    }

    pub fn set_position(&self, position: WindowPosition) {
        let mode = match position {
            WindowPosition::Free => xplm_WindowPositionFree,
            WindowPosition::Vr => xplm_WindowVR,
        };
        self.live().set_positioning_mode(mode, -1);
    }

    pub fn is_visible(&self) -> bool {
        self.live().is_visible() != 0
    }

    pub fn set_visible(&self, visible: bool) {
        self.live().set_visible(i32::from(visible));
    }

    pub fn bring_to_front(&self) {
        self.live().bring_to_front();
    }

    pub fn set_keyboard_focus(&self, focused: bool) {
        // Null is XPLM's documented way to return focus to X-Plane.
        crate::xplm::take_keyboard_focus(focused.then(|| self.live()));
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        // SAFETY: the handle was created by this wrapper and is destroyed once.
        unsafe { XPLMDestroyWindow(self.handle.as_ptr()) };
    }
}
