use std::ffi::c_void;
use std::ptr::{self, NonNull};

use xplane_sdk_sys::{
    xpMainWindowStyle_Translucent, xpProperty_MainWindowHasCloseBoxes, xpProperty_MainWindowType,
    xpWidgetClass_MainWindow, XPAddWidgetCallback, XPBringRootWidgetToFront, XPCreateCustomWidget,
    XPCreateWidget, XPDestroyWidget, XPGetWidgetGeometry, XPGetWidgetUnderlyingWindow,
    XPHideWidget, XPLMSetWindowPositioningMode, XPSetWidgetGeometry, XPSetWidgetProperty,
    XPShowWidget, XPWidgetFunc_t, XPWidgetID,
};

use crate::{c_string, Bounds, WindowPosition};

/// An owned translucent root widget and its custom drawing child.
pub struct WidgetWindow {
    root: NonNull<c_void>,
    content: NonNull<c_void>,
}

impl WidgetWindow {
    pub fn create_translucent(
        title: &str,
        bounds: Bounds,
        content_bounds: Bounds,
        callback: XPWidgetFunc_t,
    ) -> Result<Self, String> {
        if callback.is_none() {
            return Err("widget callback cannot be null".to_owned());
        }
        let title = c_string(title);
        let empty = c_string("");
        // SAFETY: descriptors remain live for the calls, the callback has the
        // widget ABI, and the root has no parent.
        let root = NonNull::new(unsafe {
            XPCreateWidget(
                bounds.left,
                bounds.top,
                bounds.right,
                bounds.bottom,
                0,
                title.as_ptr(),
                1,
                ptr::null_mut(),
                xpWidgetClass_MainWindow as i32,
            )
        })
        .ok_or_else(|| "XPCreateWidget failed".to_owned())?;
        // SAFETY: `root` is live and these properties belong to a main window.
        unsafe {
            XPSetWidgetProperty(
                root.as_ptr(),
                xpProperty_MainWindowType,
                xpMainWindowStyle_Translucent as isize,
            );
            XPSetWidgetProperty(root.as_ptr(), xpProperty_MainWindowHasCloseBoxes, 1);
            XPAddWidgetCallback(root.as_ptr(), callback);
        }
        // SAFETY: `root` is a live parent and the descriptor/callback remain
        // valid for the immediate creation call.
        let Some(content) = NonNull::new(unsafe {
            XPCreateCustomWidget(
                content_bounds.left,
                content_bounds.top,
                content_bounds.right,
                content_bounds.bottom,
                1,
                empty.as_ptr(),
                0,
                root.as_ptr(),
                callback,
            )
        }) else {
            // SAFETY: creation failed before ownership escaped; recursively
            // destroy the one live root widget.
            unsafe { XPDestroyWidget(root.as_ptr(), 1) };
            return Err("XPCreateCustomWidget failed".to_owned());
        };
        Ok(Self { root, content })
    }

    pub fn show(&self) {
        // SAFETY: this wrapper owns a live rooted widget hierarchy.
        unsafe {
            XPShowWidget(self.root.as_ptr());
            XPBringRootWidgetToFront(self.root.as_ptr());
        }
    }

    pub fn hide(&self) {
        // SAFETY: this wrapper owns a live root widget.
        unsafe { XPHideWidget(self.root.as_ptr()) };
    }

    pub fn root_geometry(&self) -> Bounds {
        widget_geometry(self.root)
    }

    pub fn content_geometry(&self) -> Bounds {
        widget_geometry(self.content)
    }

    pub fn set_geometry(&self, root: Bounds, content: Bounds) {
        // SAFETY: both widgets are live and owned by this wrapper.
        unsafe {
            XPSetWidgetGeometry(
                self.root.as_ptr(),
                root.left,
                root.top,
                root.right,
                root.bottom,
            );
            XPSetWidgetGeometry(
                self.content.as_ptr(),
                content.left,
                content.top,
                content.right,
                content.bottom,
            );
        }
    }

    pub fn set_position(&self, position: WindowPosition) -> bool {
        // SAFETY: the root is a live modern widget. XPLM owns its underlying window.
        let window = unsafe { XPGetWidgetUnderlyingWindow(self.root.as_ptr()) };
        if window.is_null() {
            return false;
        }
        let mode = match position {
            WindowPosition::Free => xplane_sdk_sys::xplm_WindowPositionFree,
            WindowPosition::Vr => xplane_sdk_sys::xplm_WindowVR,
        };
        // SAFETY: XPLM returned this live underlying window for the owned root.
        unsafe { XPLMSetWindowPositioningMode(window, mode, -1) };
        true
    }

    pub fn is_root(&self, widget: XPWidgetID) -> bool {
        self.root.as_ptr() == widget
    }

    pub fn is_content(&self, widget: XPWidgetID) -> bool {
        self.content.as_ptr() == widget
    }
}

fn widget_geometry(widget: NonNull<c_void>) -> Bounds {
    let mut bounds = Bounds::default();
    // SAFETY: callers pass one of the live widgets owned by `WidgetWindow`.
    unsafe {
        XPGetWidgetGeometry(
            widget.as_ptr(),
            &mut bounds.left,
            &mut bounds.top,
            &mut bounds.right,
            &mut bounds.bottom,
        );
    }
    bounds
}

impl Drop for WidgetWindow {
    fn drop(&mut self) {
        // SAFETY: recursively destroying the root destroys `content` too, and
        // this ownership path runs once.
        unsafe { XPDestroyWidget(self.root.as_ptr(), 1) };
    }
}
