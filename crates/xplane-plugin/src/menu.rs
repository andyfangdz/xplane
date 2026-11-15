use xplane_sdk_sys::XPLMMenuHandler_f;

use crate::xplm::{LiveMenu, PlainMenu, SharedMenu, menus};
use crate::{Command, MenuItem, c_string};

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
    pub fn new(title: &str, handler: XPLMMenuHandler_f) -> Result<Self, String> {
        let title = c_string(title);
        let parent = SharedMenu::plugins()
            .handle()
            .ok_or_else(|| "XPLMFindPluginsMenu returned null".to_owned())?;
        let item = MenuItem::append(parent, &title, 0)
            .ok_or_else(|| format!("could not append Plugins menu item {title:?}"))?;
        // SAFETY: not enforced here (residue, kept for source compatibility): this safe function
        // has always taken a raw `XPLMMenuHandler_f`, which X-Plane calls with a null refcon and
        // the item references appended through this menu while it lives; the plugins pass
        // handlers written for exactly that call.
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
