//! `XPLMDisplay.h`: screen and monitor queries, window calls on live windows,
//! and avionics device calls.

use std::ffi::{CStr, c_void};
use std::marker::PhantomData;

use xplane_sdk_sys as xp;

use crate::CallbackWindow;

/// `XPLMGetScreenSize` as `(width, height)`, through [`crate::screen_size`].
pub fn screen_size() -> (i32, i32) {
    crate::screen_size()
}

/// `XPLMGetScreenSize` as `[width, height]`.
pub fn screen_size_array() -> [i32; 2] {
    let (width, height) = screen_size();
    [width, height]
}

/// `XPLMGetScreenBoundsGlobal` as `[left, top, right, bottom]`, through
/// [`crate::screen_bounds`].
pub fn screen_bounds_global() -> [i32; 4] {
    let bounds = crate::screen_bounds();
    [bounds.left, bounds.top, bounds.right, bounds.bottom]
}

/// `XPLMGetMouseLocationGlobal` as `[x, y]`.
pub fn mouse_location_global() -> [i32; 2] {
    let mut out = [0; 2];
    // SAFETY: both out pointers are distinct elements of the local array `out`.
    unsafe { xp::XPLMGetMouseLocationGlobal(&mut out[0], &mut out[1]) };
    out
}

/// `XPLMGetAllMonitorBoundsGlobal`, calling `each(index, left, top, right,
/// bottom)` for every monitor, synchronously, before it returns.
pub fn all_monitor_bounds_global<F: FnMut(i32, i32, i32, i32, i32)>(mut each: F) {
    unsafe extern "C" fn receive<G: FnMut(i32, i32, i32, i32, i32)>(
        index: i32,
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
        refcon: *mut c_void,
    ) {
        // SAFETY: `refcon` is the `&mut G` passed below, exclusively borrowed for the whole
        // synchronous enumeration and used only from here.
        let each = unsafe { &mut *refcon.cast::<G>() };
        each(index, left, top, right, bottom)
    }
    let refcon = std::ptr::from_mut(&mut each).cast::<c_void>();
    // SAFETY: X-Plane calls `receive::<F>` only during this call, with `refcon`, which points
    // at `each` for the whole call.
    unsafe { xp::XPLMGetAllMonitorBoundsGlobal(Some(receive::<F>), refcon) }
}

/// A window that stays live for `'a`, one a borrowed [`CallbackWindow`] owns.
/// The window calls take it. Only a value from the `unsafe`
/// [`LiveWindow::from_raw`] may be null, and only for
/// [`LiveWindow::take_keyboard_focus`].
#[derive(Clone, Copy, Debug)]
pub struct LiveWindow<'a>(usize, PhantomData<&'a CallbackWindow>);

impl LiveWindow<'static> {
    /// A window that was stored as a raw `usize`.
    ///
    /// # Safety
    /// `raw` is an `XPLMWindowID` that stays live (not destroyed) for every
    /// call made through the returned value, or 0 when the only call made
    /// through it is [`LiveWindow::take_keyboard_focus`] (which documents null
    /// as "X-Plane"); the SDK promises no null check for the other calls.
    pub const unsafe fn from_raw(raw: usize) -> Self {
        LiveWindow(raw, PhantomData)
    }
}

impl<'a> LiveWindow<'a> {
    /// The window `window` owns, live while it is borrowed.
    pub fn of(window: &'a CallbackWindow) -> Self {
        Self(window.handle() as usize, PhantomData)
    }

    pub const fn raw(self) -> usize {
        self.0
    }

    fn ptr(self) -> xp::XPLMWindowID {
        self.0 as xp::XPLMWindowID
    }

    /// `XPLMGetWindowGeometry` as `[left, top, right, bottom]`.
    pub fn geometry(self) -> [i32; 4] {
        let mut out = [0; 4];
        // SAFETY: the window is live for 'a (type note); the four out pointers are distinct
        // elements of the local array `out`.
        unsafe {
            xp::XPLMGetWindowGeometry(
                self.ptr(),
                &mut out[0],
                &mut out[1],
                &mut out[2],
                &mut out[3],
            )
        };
        out
    }

    /// `XPLMSetWindowGeometry(window, left, top, right, bottom)`.
    pub fn set_geometry(self, left: i32, top: i32, right: i32, bottom: i32) {
        // SAFETY: the window is live for 'a (type note); plain values otherwise.
        unsafe { xp::XPLMSetWindowGeometry(self.ptr(), left, top, right, bottom) }
    }

    /// `XPLMSetWindowIsVisible(window, visible)`.
    pub fn set_visible(self, visible: i32) {
        // SAFETY: the window is live for 'a (type note); plain value otherwise.
        unsafe { xp::XPLMSetWindowIsVisible(self.ptr(), visible) }
    }

    /// `XPLMGetWindowIsVisible(window)`.
    pub fn is_visible(self) -> i32 {
        // SAFETY: the window is live for 'a (type note); no other arguments.
        unsafe { xp::XPLMGetWindowIsVisible(self.ptr()) }
    }

    /// `XPLMBringWindowToFront(window)`.
    pub fn bring_to_front(self) {
        // SAFETY: the window is live for 'a (type note); no other arguments.
        unsafe { xp::XPLMBringWindowToFront(self.ptr()) }
    }

    /// `XPLMSetWindowPositioningMode(window, mode, monitor)`.
    pub fn set_positioning_mode(self, mode: i32, monitor: i32) {
        // SAFETY: the window is live for 'a (type note); plain values otherwise.
        unsafe { xp::XPLMSetWindowPositioningMode(self.ptr(), mode, monitor) }
    }

    /// `XPLMSetWindowGeometryVR(window, width, height)`.
    pub fn set_geometry_vr(self, width: i32, height: i32) {
        // SAFETY: the window is live for 'a (type note); plain values otherwise.
        unsafe { xp::XPLMSetWindowGeometryVR(self.ptr(), width, height) }
    }

    /// `XPLMTakeKeyboardFocus(window)`; the null window gives focus back to X-Plane.
    pub fn take_keyboard_focus(self) {
        // SAFETY: the window is null (documented: X-Plane) or live for 'a.
        unsafe { xp::XPLMTakeKeyboardFocus(self.ptr()) }
    }

    /// `XPLMHasKeyboardFocus(window)`.
    pub fn has_keyboard_focus(self) -> i32 {
        // SAFETY: the window is live for 'a (type note); no other arguments.
        unsafe { xp::XPLMHasKeyboardFocus(self.ptr()) }
    }

    /// `XPLMSetWindowTitle(window, title)`.
    pub fn set_title(self, title: &CStr) {
        // SAFETY: the window is live for 'a (type note); `title` is NUL-terminated for the call
        // and X-Plane copies it.
        unsafe { xp::XPLMSetWindowTitle(self.ptr(), title.as_ptr()) }
    }

    /// `XPLMSetWindowResizingLimits(window, min_w, min_h, max_w, max_h)`.
    pub fn set_resizing_limits(self, min_w: i32, min_h: i32, max_w: i32, max_h: i32) {
        // SAFETY: the window is live for 'a (type note); plain values otherwise.
        unsafe { xp::XPLMSetWindowResizingLimits(self.ptr(), min_w, min_h, max_w, max_h) }
    }

    /// `XPLMWindowIsInVR(window)`.
    pub fn is_in_vr(self) -> i32 {
        // SAFETY: the window is live for 'a (type note); no other arguments.
        unsafe { xp::XPLMWindowIsInVR(self.ptr()) }
    }

    /// `XPLMWindowIsPoppedOut(window)`.
    pub fn is_popped_out(self) -> i32 {
        // SAFETY: the window is live for 'a (type note); no other arguments.
        unsafe { xp::XPLMWindowIsPoppedOut(self.ptr()) }
    }
}

/// `XPLMSetAvionicsPopupVisible(device, visible)`.
///
/// # Safety
/// `device` is a live `XPLMAvionicsID` (not yet destroyed).
pub unsafe fn set_avionics_popup_visible(device: usize, visible: i32) {
    // SAFETY: the caller guarantees the device is live; plain value otherwise.
    unsafe { xp::XPLMSetAvionicsPopupVisible(device as xp::XPLMAvionicsID, visible) }
}

/// `XPLMGetAvionicsBusVoltsRatio(device)`.
///
/// # Safety
/// `device` is a live `XPLMAvionicsID` (not yet destroyed).
pub unsafe fn avionics_bus_volts_ratio(device: usize) -> f32 {
    // SAFETY: the caller guarantees the device is live; no other arguments.
    unsafe { xp::XPLMGetAvionicsBusVoltsRatio(device as xp::XPLMAvionicsID) }
}
