#![allow(non_snake_case)]

use std::ffi::{c_char, c_float, c_int, c_void};

pub(crate) type XPLMDataRef = *mut c_void;
pub(crate) type XPLMWindowID = *mut c_void;
pub(crate) type XPLMCommandRef = *mut c_void;
pub(crate) type XPLMMenuID = *mut c_void;
pub(crate) type XPLMPluginID = c_int;
pub(crate) type XPLMKeyFlags = c_int;
pub(crate) type XPLMMouseStatus = c_int;
pub(crate) type XPLMCursorStatus = c_int;

pub(crate) type DrawWindowCallback = unsafe extern "C" fn(XPLMWindowID, *mut c_void);
pub(crate) type KeyCallback =
    unsafe extern "C" fn(XPLMWindowID, c_char, XPLMKeyFlags, c_char, *mut c_void, c_int);
pub(crate) type MouseCallback =
    unsafe extern "C" fn(XPLMWindowID, c_int, c_int, XPLMMouseStatus, *mut c_void) -> c_int;
pub(crate) type CursorCallback =
    unsafe extern "C" fn(XPLMWindowID, c_int, c_int, *mut c_void) -> XPLMCursorStatus;
pub(crate) type WheelCallback =
    unsafe extern "C" fn(XPLMWindowID, c_int, c_int, c_int, c_int, *mut c_void) -> c_int;
pub(crate) type CommandCallback = unsafe extern "C" fn(XPLMCommandRef, c_int, *mut c_void) -> c_int;
pub(crate) type FlightLoopCallback =
    unsafe extern "C" fn(c_float, c_float, c_int, *mut c_void) -> c_float;
pub(crate) type MenuCallback = unsafe extern "C" fn(*mut c_void, *mut c_void);

#[repr(C)]
pub(crate) struct XPLMCreateWindowT {
    pub(crate) struct_size: c_int,
    pub(crate) left: c_int,
    pub(crate) top: c_int,
    pub(crate) right: c_int,
    pub(crate) bottom: c_int,
    pub(crate) visible: c_int,
    pub(crate) draw_window_func: Option<DrawWindowCallback>,
    pub(crate) handle_mouse_click_func: Option<MouseCallback>,
    pub(crate) handle_key_func: Option<KeyCallback>,
    pub(crate) handle_cursor_func: Option<CursorCallback>,
    pub(crate) handle_mouse_wheel_func: Option<WheelCallback>,
    pub(crate) refcon: *mut c_void,
    pub(crate) decorate_as_floating_window: c_int,
    pub(crate) layer: c_int,
    pub(crate) handle_right_click_func: Option<MouseCallback>,
}

#[link(name = "XPLM_64")]
extern "C" {
    pub(crate) fn XPLMDebugString(message: *const c_char);
    pub(crate) fn XPLMGetSystemPath(path: *mut c_char);

    pub(crate) fn XPLMFindDataRef(name: *const c_char) -> XPLMDataRef;
    pub(crate) fn XPLMGetDatai(data_ref: XPLMDataRef) -> c_int;
    pub(crate) fn XPLMSetDatai(data_ref: XPLMDataRef, value: c_int);
    pub(crate) fn XPLMGetDataf(data_ref: XPLMDataRef) -> c_float;
    pub(crate) fn XPLMSetDataf(data_ref: XPLMDataRef, value: c_float);
    pub(crate) fn XPLMGetDatad(data_ref: XPLMDataRef) -> f64;
    pub(crate) fn XPLMSetDatad(data_ref: XPLMDataRef, value: f64);
    pub(crate) fn XPLMGetDatavf(
        data_ref: XPLMDataRef,
        values: *mut c_float,
        offset: c_int,
        max: c_int,
    ) -> c_int;
    pub(crate) fn XPLMGetDatavi(
        data_ref: XPLMDataRef,
        values: *mut c_int,
        offset: c_int,
        max: c_int,
    ) -> c_int;
    pub(crate) fn XPLMSetDatavf(
        data_ref: XPLMDataRef,
        values: *const c_float,
        offset: c_int,
        count: c_int,
    );

    pub(crate) fn XPLMWorldToLocal(
        latitude: f64,
        longitude: f64,
        altitude_m: f64,
        out_x: *mut f64,
        out_y: *mut f64,
        out_z: *mut f64,
    );

    pub(crate) fn XPLMCreateWindowEx(params: *mut XPLMCreateWindowT) -> XPLMWindowID;
    pub(crate) fn XPLMDestroyWindow(window: XPLMWindowID);
    pub(crate) fn XPLMGetScreenBoundsGlobal(
        left: *mut c_int,
        top: *mut c_int,
        right: *mut c_int,
        bottom: *mut c_int,
    );
    pub(crate) fn XPLMGetWindowGeometry(
        window: XPLMWindowID,
        left: *mut c_int,
        top: *mut c_int,
        right: *mut c_int,
        bottom: *mut c_int,
    );
    pub(crate) fn XPLMSetWindowGeometry(
        window: XPLMWindowID,
        left: c_int,
        top: c_int,
        right: c_int,
        bottom: c_int,
    );
    pub(crate) fn XPLMSetWindowIsVisible(window: XPLMWindowID, visible: c_int);
    pub(crate) fn XPLMGetWindowIsVisible(window: XPLMWindowID) -> c_int;
    pub(crate) fn XPLMBringWindowToFront(window: XPLMWindowID);
    pub(crate) fn XPLMSetWindowPositioningMode(window: XPLMWindowID, mode: c_int, monitor: c_int);
    pub(crate) fn XPLMSetWindowResizingLimits(
        window: XPLMWindowID,
        min_width: c_int,
        min_height: c_int,
        max_width: c_int,
        max_height: c_int,
    );
    pub(crate) fn XPLMSetWindowTitle(window: XPLMWindowID, title: *const c_char);
    pub(crate) fn XPLMTakeKeyboardFocus(window: XPLMWindowID);

    pub(crate) fn XPLMSetGraphicsState(
        enable_fog: c_int,
        texture_units: c_int,
        enable_lighting: c_int,
        enable_alpha_testing: c_int,
        enable_alpha_blending: c_int,
        enable_depth_testing: c_int,
        enable_depth_writing: c_int,
    );

    pub(crate) fn XPLMCreateCommand(
        name: *const c_char,
        description: *const c_char,
    ) -> XPLMCommandRef;
    pub(crate) fn XPLMRegisterCommandHandler(
        command: XPLMCommandRef,
        handler: Option<CommandCallback>,
        before: c_int,
        refcon: *mut c_void,
    );
    pub(crate) fn XPLMUnregisterCommandHandler(
        command: XPLMCommandRef,
        handler: Option<CommandCallback>,
        before: c_int,
        refcon: *mut c_void,
    );

    pub(crate) fn XPLMRegisterFlightLoopCallback(
        callback: Option<FlightLoopCallback>,
        interval: c_float,
        refcon: *mut c_void,
    );
    pub(crate) fn XPLMUnregisterFlightLoopCallback(
        callback: Option<FlightLoopCallback>,
        refcon: *mut c_void,
    );

    pub(crate) fn XPLMFindPluginsMenu() -> XPLMMenuID;
    pub(crate) fn XPLMAppendMenuItem(
        menu: XPLMMenuID,
        name: *const c_char,
        item_ref: *mut c_void,
        deprecated_and_ignored: c_int,
    ) -> c_int;
    pub(crate) fn XPLMCreateMenu(
        name: *const c_char,
        parent_menu: XPLMMenuID,
        parent_item: c_int,
        handler: Option<MenuCallback>,
        menu_ref: *mut c_void,
    ) -> XPLMMenuID;
    pub(crate) fn XPLMAppendMenuItemWithCommand(
        menu: XPLMMenuID,
        name: *const c_char,
        command: XPLMCommandRef,
    ) -> c_int;
    pub(crate) fn XPLMDestroyMenu(menu: XPLMMenuID);
    pub(crate) fn XPLMRemoveMenuItem(menu: XPLMMenuID, index: c_int);
}

#[link(name = "OpenGL32")]
extern "system" {
    pub(crate) fn wglGetProcAddress(name: *const c_char) -> *const c_void;
}

#[link(name = "Kernel32")]
extern "system" {
    pub(crate) fn LoadLibraryA(name: *const u8) -> *mut c_void;
    pub(crate) fn GetProcAddress(module: *mut c_void, name: *const c_char) -> *const c_void;
}

pub(crate) const XPLM_WINDOW_POSITION_FREE: i32 = 0;
pub(crate) const XPLM_WINDOW_VR: i32 = 5;
pub(crate) const XPLM_WINDOW_DECORATION_ROUND_RECTANGLE: i32 = 1;
pub(crate) const XPLM_WINDOW_LAYER_FLOATING: i32 = 1;
pub(crate) const XPLM_MOUSE_DOWN: i32 = 1;
pub(crate) const XPLM_MOUSE_DRAG: i32 = 2;
pub(crate) const XPLM_MOUSE_UP: i32 = 3;
pub(crate) const XPLM_COMMAND_BEGIN: i32 = 0;
pub(crate) const XPLM_DOWN_FLAG: i32 = 8;
pub(crate) const XPLM_UP_FLAG: i32 = 16;
pub(crate) const XPLM_SHIFT_FLAG: i32 = 1;
pub(crate) const XPLM_OPTION_ALT_FLAG: i32 = 2;
pub(crate) const XPLM_CONTROL_FLAG: i32 = 4;
pub(crate) const XPLM_MSG_ENTERED_VR: i32 = 109;
pub(crate) const XPLM_MSG_EXITING_VR: i32 = 110;
pub(crate) const XPLM_PLUGIN_XPLANE: i32 = 0;
pub(crate) const XPLM_CURSOR_DEFAULT: i32 = 0;
pub(crate) const XPLM_CURSOR_IBEAM: i32 = 1;
pub(crate) const XPLM_CURSOR_ARROW: i32 = 2;

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem;

    #[test]
    fn xplm_window_struct_matches_x64_sdk_layout() {
        assert_eq!(mem::size_of::<XPLMCreateWindowT>(), 88);
    }
}
