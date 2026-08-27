use std::ffi::c_void;
use std::mem;
use std::ptr::{self, NonNull};

use xplane_sdk_sys::{
    xplm_WindowPositionFree, xplm_WindowVR, XPLMBringWindowToFront, XPLMCreateWindowEx,
    XPLMCreateWindow_t, XPLMDestroyWindow, XPLMDrawWindow_f, XPLMGetWindowGeometry,
    XPLMGetWindowIsVisible, XPLMHandleCursor_f, XPLMHandleKey_f, XPLMHandleMouseClick_f,
    XPLMHandleMouseWheel_f, XPLMSetWindowGeometry, XPLMSetWindowIsVisible,
    XPLMSetWindowPositioningMode, XPLMSetWindowResizingLimits, XPLMSetWindowTitle,
    XPLMTakeKeyboardFocus, XPLMWindowDecoration, XPLMWindowLayer,
};

use crate::{c_string, Bounds};

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

    pub fn geometry(&self) -> Bounds {
        let mut bounds = Bounds::default();
        // SAFETY: this wrapper owns a live window and all output pointers are valid.
        unsafe {
            XPLMGetWindowGeometry(
                self.handle.as_ptr(),
                &mut bounds.left,
                &mut bounds.top,
                &mut bounds.right,
                &mut bounds.bottom,
            );
        }
        bounds
    }

    pub fn set_geometry(&self, bounds: Bounds) {
        // SAFETY: this wrapper owns a live window.
        unsafe {
            XPLMSetWindowGeometry(
                self.handle.as_ptr(),
                bounds.left,
                bounds.top,
                bounds.right,
                bounds.bottom,
            );
        }
    }

    pub fn set_resizing_limits(
        &self,
        min_width: i32,
        min_height: i32,
        max_width: i32,
        max_height: i32,
    ) {
        // SAFETY: this wrapper owns a live modern window.
        unsafe {
            XPLMSetWindowResizingLimits(
                self.handle.as_ptr(),
                min_width,
                min_height,
                max_width,
                max_height,
            );
        }
    }

    pub fn set_title(&self, title: &str) {
        let title = c_string(title);
        // SAFETY: the window is live and the title is NUL-terminated.
        unsafe { XPLMSetWindowTitle(self.handle.as_ptr(), title.as_ptr()) };
    }

    pub fn set_position(&self, position: WindowPosition) {
        let mode = match position {
            WindowPosition::Free => xplm_WindowPositionFree,
            WindowPosition::Vr => xplm_WindowVR,
        };
        // SAFETY: this wrapper owns a live modern window.
        unsafe { XPLMSetWindowPositioningMode(self.handle.as_ptr(), mode, -1) };
    }

    pub fn is_visible(&self) -> bool {
        // SAFETY: this wrapper owns a live window.
        unsafe { XPLMGetWindowIsVisible(self.handle.as_ptr()) != 0 }
    }

    pub fn set_visible(&self, visible: bool) {
        // SAFETY: this wrapper owns a live window.
        unsafe { XPLMSetWindowIsVisible(self.handle.as_ptr(), i32::from(visible)) };
    }

    pub fn bring_to_front(&self) {
        // SAFETY: this wrapper owns a live window.
        unsafe { XPLMBringWindowToFront(self.handle.as_ptr()) };
    }

    pub fn set_keyboard_focus(&self, focused: bool) {
        // SAFETY: this wrapper owns a live window; null is XPLM's documented
        // way to return focus to X-Plane.
        unsafe {
            XPLMTakeKeyboardFocus(if focused {
                self.handle.as_ptr()
            } else {
                ptr::null_mut()
            });
        }
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        // SAFETY: the handle was created by this wrapper and is destroyed once.
        unsafe { XPLMDestroyWindow(self.handle.as_ptr()) };
    }
}
