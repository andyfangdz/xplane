use std::ffi::c_void;

use xplane_sdk_sys::XPLMMenuHandler_f;

use crate::xplm::{LiveMenu, PlainMenu, SharedMenu, menus};
use crate::{Command, MenuItem, c_string};

/// A menu handler (`XPLMMenuHandler_f`) that is a safe function:
/// `(menu refcon, item refcon)`. Being safe, it must be sound for any
/// argument values; X-Plane calls it on the plugin thread.
pub type MenuHandlerFn = extern "C" fn(*mut c_void, *mut c_void);

/// Owns a submenu inserted into X-Plane's Plugins menu.
///
/// The value must be created and dropped on X-Plane's plugin thread.
pub struct PluginMenu {
    /// The submenu; `None` once destroyed.
    menu: Option<PlainMenu>,
    /// The Plugins menu item that hosts `menu`; it tracks its own index.
    item: Option<MenuItem>,
}

impl PluginMenu {
    /// Appends `title` to X-Plane's Plugins menu and creates a submenu under
    /// it with `handler` (`None`: no handler). X-Plane calls the handler with
    /// a null menu refcon and, as the item refcon, the `identifier` given to
    /// [`PluginMenu::append_item`] cast to a pointer.
    pub fn new(title: &str, handler: Option<MenuHandlerFn>) -> Result<Self, String> {
        let handler: XPLMMenuHandler_f = handler.map(|handler| handler as _);
        let title = c_string(title);
        let parent = SharedMenu::plugins()
            .handle()
            .ok_or_else(|| "XPLMFindPluginsMenu returned null".to_owned())?;
        let item = MenuItem::append(parent, &title, 0)
            .ok_or_else(|| format!("could not append Plugins menu item {title:?}"))?;
        // SAFETY: the handler is null or a safe function, which is sound for any arguments: the
        // null refcon and any item reference included.
        let menu =
            unsafe { PlainMenu::create_with_handler(&title, Some(parent), item.index(), handler) }
                .ok_or_else(|| format!("could not create Plugins submenu {title:?}"))?;
        Ok(Self {
            menu: Some(menu),
            item: Some(item),
        })
    }

    /// The submenu, or `None` after [`PluginMenu::destroy`]: the append calls
    /// then pass X-Plane the null menu as they always did, which the SDK
    /// documents as failing the add.
    fn target(&self) -> Option<LiveMenu<'_>> {
        self.menu.as_ref().map(LiveMenu::of_plain)
    }

    pub fn append_item(&self, label: &str, identifier: usize) -> Result<i32, String> {
        let label = c_string(label);
        let index = menus::append_item_to(self.target(), &label, identifier);
        (index >= 0)
            .then_some(index)
            .ok_or_else(|| format!("could not append menu item {label:?}"))
    }

    pub fn append_command(&self, label: &str, command: &Command) -> Result<i32, String> {
        let label = c_string(label);
        let index = menus::append_command_to(self.target(), &label, command.command_handle());
        (index >= 0)
            .then_some(index)
            .ok_or_else(|| format!("could not append command menu item {label:?}"))
    }

    pub fn append_separator(&self) {
        menus::append_separator_to(self.target());
    }

    /// # Panics
    /// After [`PluginMenu::destroy`]: this used to pass X-Plane the null menu,
    /// which `XPLMMenus.h` does not document `XPLMCheckMenuItem` as accepting.
    pub fn set_checked(&self, index: i32, checked: bool) {
        self.target()
            .expect("PluginMenu used after destroy")
            .check_item(index, checked);
    }

    pub fn destroy(&mut self) {
        if self.menu.is_none() {
            return;
        }
        // Destroy the submenu once, then remove the hosting Plugins menu item
        // at its current index.
        self.menu = None;
        self.item = None;
    }
}

impl Drop for PluginMenu {
    fn drop(&mut self) {
        self.destroy();
    }
}
