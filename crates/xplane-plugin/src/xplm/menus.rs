//! `XPLMMenus.h`: X-Plane's shared menus and item calls on live menus.

use std::cell::RefCell;
use std::ffi::{CStr, c_void};
use std::marker::PhantomData;

use xplane_sdk_sys as xp;

use super::{AddressSet, CommandRef, Found};
use crate::CallbackMenu;
use crate::registrations::MenuHandle;

thread_local! {
    static SEEN_SHARED_MENUS: RefCell<AddressSet> = RefCell::new(AddressSet::default());
}

static SHARED_MENUS: Found = Found::new(&SEEN_SHARED_MENUS);

/// X-Plane's own Plugins or Aircraft menu as its lookup returned it, or null.
/// X-Plane never destroys these menus.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct SharedMenu(usize);

impl SharedMenu {
    /// `XPLMFindPluginsMenu()`.
    pub fn plugins() -> Self {
        // SAFETY: no arguments; the result is X-Plane's own menu or null.
        let raw = unsafe { xp::XPLMFindPluginsMenu() } as usize;
        SHARED_MENUS.insert(raw);
        Self(raw)
    }

    /// `XPLMFindAircraftMenu()`.
    pub fn aircraft() -> Self {
        // SAFETY: no arguments; the result is X-Plane's own menu or null.
        let raw = unsafe { xp::XPLMFindAircraftMenu() } as usize;
        SHARED_MENUS.insert(raw);
        Self(raw)
    }

    /// The shared menu that was stored as [`SharedMenu::raw`], or `None` when
    /// `raw` is not 0 and not a menu [`SharedMenu::plugins`] or
    /// [`SharedMenu::aircraft`] returned in this process.
    pub fn from_raw(raw: usize) -> Option<Self> {
        SHARED_MENUS.contains(raw).then_some(Self(raw))
    }

    pub const fn raw(self) -> usize {
        self.0
    }

    pub const fn is_null(self) -> bool {
        self.0 == 0
    }

    /// The menu as a [`MenuHandle`]; `None` for null.
    pub fn handle(self) -> Option<MenuHandle> {
        // SAFETY: a non-null value is X-Plane's own Plugins or Aircraft menu (the only
        // constructors are their lookups), which X-Plane never destroys.
        unsafe { MenuHandle::from_raw(self.0 as xp::XPLMMenuID) }
    }

    /// The menu for item calls, live for the process; `None` for null, which
    /// the SDK does not document the item calls as accepting.
    ///
    /// X-Plane numbers this plugin's items in a shared menu from 0, and a
    /// [`crate::MenuItem`] computes its index from the `MenuItem`s alone, so
    /// do not append through this value to a menu that holds `MenuItem`s.
    pub const fn live(self) -> Option<LiveMenu<'static>> {
        if self.0 == 0 {
            None
        } else {
            Some(LiveMenu(self.0, PhantomData))
        }
    }
}

/// An owned menu with no handler: `XPLMCreateMenu` with a null handler and
/// refcon, destroyed once on drop. Its items are typically commands
/// ([`LiveMenu::append_item_with_command`]); for a handler use
/// [`CallbackMenu`].
#[must_use = "dropping a menu immediately destroys it"]
pub struct PlainMenu {
    handle: MenuHandle,
}

impl PlainMenu {
    /// `XPLMCreateMenu(name, parent or null, item, null, null)`: a submenu
    /// under `parent` item `item`, or in the menu bar when `parent` is `None`.
    /// Returns `None` if X-Plane refuses.
    pub fn create(name: &CStr, parent: Option<MenuHandle>, item: i32) -> Option<Self> {
        // SAFETY: the name is NUL-terminated for the call and X-Plane copies it; the parent is a
        // live SDK menu (a `MenuHandle`) or null; with no handler, the null refcon is never used.
        let handle = unsafe {
            xp::XPLMCreateMenu(
                name.as_ptr(),
                parent.map_or(std::ptr::null_mut(), MenuHandle::as_raw),
                item,
                None,
                std::ptr::null_mut(),
            )
        };
        // SAFETY: a non-null result is the live menu just created, destroyed only by Drop.
        let handle = unsafe { MenuHandle::from_raw(handle) }?;
        Some(Self { handle })
    }

    /// The SDK handle. It is valid while `self` lives.
    pub fn handle(&self) -> MenuHandle {
        self.handle
    }
}

impl Drop for PlainMenu {
    fn drop(&mut self) {
        // SAFETY: this value created the menu and destroys it once.
        unsafe { xp::XPLMDestroyMenu(self.handle.as_raw()) };
    }
}

/// A menu that stays live for `'a`, never null: X-Plane's own Plugins or
/// Aircraft menu, or a menu a borrowed [`CallbackMenu`] or [`PlainMenu`]
/// owns. The item calls take it.
///
/// Item indices are passed through; X-Plane logs an out-of-range one. The
/// append calls here are not counted by [`crate::MenuItem`], which assumes
/// every item of this plugin in its parent menu is a `MenuItem`: keep raw
/// items and `MenuItem`s in different menus.
#[derive(Clone, Copy, Debug)]
pub struct LiveMenu<'a>(usize, PhantomData<&'a CallbackMenu>);

impl LiveMenu<'static> {
    /// A menu that was stored as a raw `usize`.
    ///
    /// # Safety
    /// `raw` is a non-null `XPLMMenuID` that stays live (not destroyed) for
    /// every call made through the returned value.
    pub const unsafe fn from_raw(raw: usize) -> Self {
        LiveMenu(raw, PhantomData)
    }
}

impl<'a> LiveMenu<'a> {
    /// The menu `menu` owns, live while it is borrowed.
    pub fn of(menu: &'a CallbackMenu) -> Self {
        Self(menu.handle().as_raw() as usize, PhantomData)
    }

    /// The menu `menu` owns, live while it is borrowed.
    pub fn of_plain(menu: &'a PlainMenu) -> Self {
        Self(menu.handle().as_raw() as usize, PhantomData)
    }

    /// A menu the crate keeps live for `'a` by other means.
    ///
    /// # Safety
    /// `menu` is not destroyed during `'a`.
    pub(crate) unsafe fn borrowed(menu: MenuHandle) -> Self {
        Self(menu.as_raw() as usize, PhantomData)
    }

    pub const fn raw(self) -> usize {
        self.0
    }

    fn ptr(self) -> xp::XPLMMenuID {
        self.0 as xp::XPLMMenuID
    }

    /// `XPLMCheckMenuItem(menu, index, checked ? xplm_Menu_Checked : xplm_Menu_Unchecked)`.
    pub fn check_item(self, index: i32, checked: bool) {
        let state = if checked {
            xp::xplm_Menu_Checked
        } else {
            xp::xplm_Menu_Unchecked
        };
        self.check_item_state(index, state)
    }

    /// `XPLMCheckMenuItem(menu, index, state)` with a raw `XPLMMenuCheck` value.
    pub fn check_item_state(self, index: i32, state: xp::XPLMMenuCheck) {
        // SAFETY: the menu is live for 'a (type note); plain values otherwise.
        unsafe { xp::XPLMCheckMenuItem(self.ptr(), index, state) }
    }

    /// `XPLMEnableMenuItem(menu, index, enabled)`.
    pub fn enable_item(self, index: i32, enabled: i32) {
        // SAFETY: the menu is live for 'a (type note); plain values otherwise.
        unsafe { xp::XPLMEnableMenuItem(self.ptr(), index, enabled) }
    }

    /// `XPLMSetMenuItemName(menu, index, name, 0)`.
    pub fn set_item_name(self, index: i32, name: &CStr) {
        self.set_item_name_english(index, name, 0)
    }

    /// `XPLMSetMenuItemName(menu, index, name, force_english)`.
    pub fn set_item_name_english(self, index: i32, name: &CStr, force_english: i32) {
        // SAFETY: the menu is live for 'a; `name` is NUL-terminated and outlives the call, and
        // X-Plane copies it.
        unsafe { xp::XPLMSetMenuItemName(self.ptr(), index, name.as_ptr(), force_english) }
    }

    /// `XPLMClearAllMenuItems(menu)`.
    ///
    /// # Safety
    /// No item of the menu hosts a submenu that is still alive: the SDK does
    /// not say whether removing an item destroys the submenu attached to it,
    /// and if it does, the submenu's owner (a [`CallbackMenu`]) would later
    /// destroy it a second time. Destroy such submenus first. In a shared
    /// menu this removes every item of this plugin, so no [`crate::MenuItem`]
    /// may be alive in it either.
    pub unsafe fn clear_all_items(self) {
        // SAFETY: the menu is live for 'a (type note); the caller guarantees no removed item
        // hosts a live submenu.
        unsafe { xp::XPLMClearAllMenuItems(self.ptr()) }
    }

    /// `XPLMAppendMenuItem(menu, name, item_ref, 0)`; `item_ref` is an opaque
    /// tag X-Plane hands back to the menu's handler.
    pub fn append_item(self, name: &CStr, item_ref: usize) -> i32 {
        // SAFETY: the menu is live for 'a; `name` is NUL-terminated for the call and X-Plane
        // copies it; `item_ref` is stored, never dereferenced by X-Plane.
        unsafe { xp::XPLMAppendMenuItem(self.ptr(), name.as_ptr(), item_ref as *mut c_void, 0) }
    }

    /// `XPLMAppendMenuItemWithCommand(menu, name, command)`.
    ///
    /// # Panics
    /// If `command` is null.
    pub fn append_item_with_command(self, name: &CStr, command: CommandRef) -> i32 {
        let command = command.non_null();
        // SAFETY: the menu is live; `name` is NUL-terminated for the call and X-Plane copies it;
        // a non-null command is never destroyed (module note).
        unsafe { xp::XPLMAppendMenuItemWithCommand(self.ptr(), name.as_ptr(), command) }
    }

    /// `XPLMAppendMenuSeparator(menu)`.
    pub fn append_separator(self) {
        // SAFETY: the menu is live for 'a (type note); no other arguments.
        unsafe { xp::XPLMAppendMenuSeparator(self.ptr()) }
    }

    /// `XPLMRemoveMenuItem(menu, index)`. For X-Plane's shared menus prefer an
    /// owned [`crate::MenuItem`], which tracks its current index.
    ///
    /// # Safety
    /// As [`LiveMenu::clear_all_items`], for the item at `index`: it does not
    /// host a submenu that is still alive, and in a shared menu no
    /// [`crate::MenuItem`] of this plugin is alive in it (removing an item
    /// moves the later ones up, so a `MenuItem` would remove the wrong one).
    pub unsafe fn remove_item(self, index: i32) {
        // SAFETY: the menu is live for 'a (type note); the caller guarantees the item does not
        // host a live submenu.
        unsafe { xp::XPLMRemoveMenuItem(self.ptr(), index) }
    }
}
