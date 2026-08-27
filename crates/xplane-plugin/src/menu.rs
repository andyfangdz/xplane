use std::ffi::c_void;
use std::ptr;

use xplane_sdk_sys::{
    xplm_Menu_Checked, xplm_Menu_Unchecked, XPLMAppendMenuItem, XPLMAppendMenuItemWithCommand,
    XPLMAppendMenuSeparator, XPLMCheckMenuItem, XPLMCreateMenu, XPLMDestroyMenu,
    XPLMFindPluginsMenu, XPLMMenuHandler_f, XPLMMenuID, XPLMRemoveMenuItem,
};

use crate::{c_string, Command};

/// Owns a submenu inserted into X-Plane's Plugins menu.
///
/// The value must be created and dropped on X-Plane's plugin thread.
pub struct PluginMenu {
    parent: XPLMMenuID,
    menu: XPLMMenuID,
    parent_item: i32,
}

impl PluginMenu {
    pub fn new(title: &str, handler: XPLMMenuHandler_f) -> Result<Self, String> {
        let title = c_string(title);
        // SAFETY: X-Plane owns the parent menu. `title` is NUL-terminated and
        // the callback has the SDK ABI.
        unsafe {
            let parent = XPLMFindPluginsMenu();
            if parent.is_null() {
                return Err("XPLMFindPluginsMenu returned null".to_owned());
            }
            let parent_item = XPLMAppendMenuItem(parent, title.as_ptr(), ptr::null_mut(), 0);
            if parent_item < 0 {
                return Err(format!("could not append Plugins menu item {title:?}"));
            }
            let menu = XPLMCreateMenu(
                title.as_ptr(),
                parent,
                parent_item,
                handler,
                ptr::null_mut(),
            );
            if menu.is_null() {
                XPLMRemoveMenuItem(parent, parent_item);
                return Err(format!("could not create Plugins submenu {title:?}"));
            }
            Ok(Self {
                parent,
                menu,
                parent_item,
            })
        }
    }

    pub fn append_item(&self, label: &str, identifier: usize) -> Result<i32, String> {
        let label = c_string(label);
        // SAFETY: this wrapper owns a live menu. The integer token is only
        // round-tripped through XPLM and is never dereferenced.
        let index =
            unsafe { XPLMAppendMenuItem(self.menu, label.as_ptr(), identifier as *mut c_void, 0) };
        (index >= 0)
            .then_some(index)
            .ok_or_else(|| format!("could not append menu item {label:?}"))
    }

    pub fn append_command(&self, label: &str, command: &Command) -> Result<i32, String> {
        let label = c_string(label);
        // SAFETY: this menu and `command` are live XPLM handles.
        let index =
            unsafe { XPLMAppendMenuItemWithCommand(self.menu, label.as_ptr(), command.handle()) };
        (index >= 0)
            .then_some(index)
            .ok_or_else(|| format!("could not append command menu item {label:?}"))
    }

    pub fn append_separator(&self) {
        // SAFETY: this wrapper owns a live menu.
        unsafe { XPLMAppendMenuSeparator(self.menu) }
    }

    pub fn set_checked(&self, index: i32, checked: bool) {
        // SAFETY: callers retain item indices returned by this menu.
        unsafe {
            XPLMCheckMenuItem(
                self.menu,
                index,
                if checked {
                    xplm_Menu_Checked
                } else {
                    xplm_Menu_Unchecked
                },
            )
        }
    }

    pub fn destroy(&mut self) {
        if self.menu.is_null() {
            return;
        }
        // SAFETY: these handles and the parent index were created together by
        // `new` and are cleared immediately after this one-time destruction.
        unsafe {
            XPLMDestroyMenu(self.menu);
            XPLMRemoveMenuItem(self.parent, self.parent_item);
        }
        self.parent = ptr::null_mut();
        self.menu = ptr::null_mut();
        self.parent_item = -1;
    }
}

impl Drop for PluginMenu {
    fn drop(&mut self) {
        self.destroy();
    }
}
