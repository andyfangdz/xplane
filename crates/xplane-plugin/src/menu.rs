use std::ffi::c_void;
use std::ptr;

use xplane_sdk_sys::{
    XPLMAppendMenuItem, XPLMAppendMenuItemWithCommand, XPLMAppendMenuSeparator, XPLMCheckMenuItem,
    XPLMCreateMenu, XPLMDestroyMenu, XPLMFindPluginsMenu, XPLMMenuHandler_f, XPLMMenuID,
    xplm_Menu_Checked, xplm_Menu_Unchecked,
};

use crate::{Command, MenuItem, c_string, registrations::MenuHandle};

/// Owns a submenu inserted into X-Plane's Plugins menu.
///
/// The value must be created and dropped on X-Plane's plugin thread.
pub struct PluginMenu {
    menu: XPLMMenuID,
    /// The Plugins menu item that hosts `menu`; it tracks its own index.
    item: Option<MenuItem>,
}

impl PluginMenu {
    pub fn new(title: &str, handler: XPLMMenuHandler_f) -> Result<Self, String> {
        let title = c_string(title);
        // SAFETY: XPLMFindPluginsMenu takes no arguments; a nonnull result is
        // X-Plane's live Plugins menu.
        let parent = unsafe { MenuHandle::from_raw(XPLMFindPluginsMenu()) }
            .ok_or_else(|| "XPLMFindPluginsMenu returned null".to_owned())?;
        let item = MenuItem::append(parent, &title, 0)
            .ok_or_else(|| format!("could not append Plugins menu item {title:?}"))?;
        // SAFETY: X-Plane owns the parent menu and `item` is its live item.
        // `title` is NUL-terminated and the callback has the SDK ABI.
        let menu = unsafe {
            XPLMCreateMenu(
                title.as_ptr(),
                parent.as_raw(),
                item.index(),
                handler,
                ptr::null_mut(),
            )
        };
        if menu.is_null() {
            return Err(format!("could not create Plugins submenu {title:?}"));
        }
        Ok(Self {
            menu,
            item: Some(item),
        })
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
        // SAFETY: `new` created this menu; it is cleared immediately after
        // this one-time destruction.
        unsafe { XPLMDestroyMenu(self.menu) };
        self.menu = ptr::null_mut();
        // Remove the hosting Plugins menu item at its current index.
        self.item = None;
    }
}

impl Drop for PluginMenu {
    fn drop(&mut self) {
        self.destroy();
    }
}
