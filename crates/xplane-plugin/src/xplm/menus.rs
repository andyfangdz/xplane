//! `XPLMMenus.h`: X-Plane's shared menus and item calls on live menus.

use std::cell::RefCell;
use std::ffi::{CStr, c_void};
use std::marker::PhantomData;

use xplane_sdk_sys as xp;

use super::{AddressSet, CommandRef, Found};
use crate::CallbackMenu;
use crate::registrations::{CommandHandle, MenuHandle};

thread_local! {
    static SEEN_SHARED_MENUS: RefCell<AddressSet> = RefCell::new(AddressSet::default());
    /// Menus the crate's owners ([`PlainMenu`], [`crate::PluginMenu`],
    /// [`CallbackMenu`], the session menus of
    /// [`crate::registrations::Registrations`]) destroyed, until X-Plane hands
    /// the same address out again for a new menu.
    static DESTROYED_MENUS: RefCell<AddressSet> = RefCell::new(AddressSet::default());
}

static SHARED_MENUS: Found = Found::new(&SEEN_SHARED_MENUS);

value_calls! {
    fn sdk_plugins_menu() -> xp::XPLMMenuID = XPLMFindPluginsMenu;
    fn sdk_aircraft_menu() -> xp::XPLMMenuID = XPLMFindAircraftMenu;
}

/// Records that X-Plane just created `menu` (it may reuse a destroyed menu's
/// address). Every owner that calls `XPLMCreateMenu` calls this.
pub(crate) fn menu_created(menu: MenuHandle) {
    DESTROYED_MENUS.with_borrow_mut(|destroyed| destroyed.remove(&(menu.as_raw() as usize)));
}

/// Records that `menu` was just destroyed with `XPLMDestroyMenu`. Every owner
/// that destroys a menu calls this, so a [`crate::MenuItem`] or a submenu
/// that outlives its parent through a copied [`MenuHandle`] makes no call on it.
pub(crate) fn menu_destroyed(menu: MenuHandle) {
    DESTROYED_MENUS.with_borrow_mut(|destroyed| destroyed.insert(menu.as_raw() as usize));
}

/// Whether one of the crate's owners destroyed `menu` (and X-Plane has not
/// created a menu at its address since).
pub(crate) fn is_destroyed(menu: MenuHandle) -> bool {
    DESTROYED_MENUS.with_borrow(|destroyed| destroyed.contains(&(menu.as_raw() as usize)))
}

/// The refusal [`PlainMenu::create_with_handler`] and
/// [`CallbackMenu::create`] make before `XPLMCreateMenu`: whether `parent` is
/// a menu one of the crate's owners destroyed.
pub(crate) fn refuses_parent(parent: Option<MenuHandle>) -> bool {
    parent.is_some_and(is_destroyed)
}

/// X-Plane's own Plugins or Aircraft menu as its lookup returned it, or null.
/// X-Plane never destroys these menus.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct SharedMenu(usize);

impl SharedMenu {
    /// `XPLMFindPluginsMenu()`.
    pub fn plugins() -> Self {
        Self::found(sdk_plugins_menu() as usize)
    }

    /// `XPLMFindAircraftMenu()`.
    pub fn aircraft() -> Self {
        Self::found(sdk_aircraft_menu() as usize)
    }

    /// Records a menu one of the lookups returned: X-Plane's own menu or null.
    fn found(raw: usize) -> Self {
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
    /// Returns `None` if X-Plane refuses, or, without a call, if one of the
    /// crate's menu owners has already destroyed `parent`.
    ///
    /// The menu is destroyed exactly once, on drop, even when its parent or
    /// parent item went first: see the open gap on [`crate::MenuItem`] for
    /// what that relies on.
    pub fn create(name: &CStr, parent: Option<MenuHandle>, item: i32) -> Option<Self> {
        // SAFETY: with no handler, X-Plane never calls back into this plugin for the menu.
        unsafe { Self::create_with_handler(name, parent, item, None) }
    }

    /// `XPLMCreateMenu(name, parent or null, item, handler, null)`, destroyed
    /// on drop; `None` as [`PlainMenu::create`].
    ///
    /// # Safety
    /// X-Plane may call `handler` on its main thread with a null refcon and an
    /// item reference of the menu's items, for as long as the menu lives; that
    /// call must be sound.
    pub(crate) unsafe fn create_with_handler(
        name: &CStr,
        parent: Option<MenuHandle>,
        item: i32,
        handler: xp::XPLMMenuHandler_f,
    ) -> Option<Self> {
        if refuses_parent(parent) {
            return None;
        }
        // SAFETY: the name is NUL-terminated for the call and X-Plane copies it; the parent is
        // null or a live SDK menu (a `MenuHandle`, not destroyed by the crate's owners: checked);
        // the handler's calls with the null refcon are the caller's contract.
        let handle = unsafe {
            xp::XPLMCreateMenu(
                name.as_ptr(),
                parent.map_or(std::ptr::null_mut(), MenuHandle::as_raw),
                item,
                handler,
                std::ptr::null_mut(),
            )
        };
        // SAFETY: a non-null result is the live menu just created, destroyed only by Drop.
        let handle = unsafe { MenuHandle::from_raw(handle) }?;
        menu_created(handle);
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
        menu_destroyed(self.handle);
    }
}

/// The menu an append call passes: a live menu, or null. `XPLMMenus.h` says
/// of the null Aircraft menu that "any attempts to add menu items to it will
/// fail", so an append to null is a defined call that adds nothing.
fn append_target(menu: Option<LiveMenu<'_>>) -> xp::XPLMMenuID {
    menu.map_or(std::ptr::null_mut(), LiveMenu::ptr)
}

/// `XPLMAppendMenuItem(menu or null, name, item_ref, 0)`; `item_ref` is an
/// opaque tag X-Plane hands back to the menu's handler.
pub(crate) fn append_item_to(menu: Option<LiveMenu<'_>>, name: &CStr, item_ref: usize) -> i32 {
    // SAFETY: the menu is live for the borrow or null, to which X-Plane fails the add
    // (`append_target`); `name` is NUL-terminated for the call and X-Plane copies it; `item_ref`
    // is stored, never dereferenced by X-Plane.
    unsafe {
        xp::XPLMAppendMenuItem(
            append_target(menu),
            name.as_ptr(),
            item_ref as *mut c_void,
            0,
        )
    }
}

/// `XPLMAppendMenuItemWithCommand(menu or null, name, command)`.
pub(crate) fn append_command_to(
    menu: Option<LiveMenu<'_>>,
    name: &CStr,
    command: CommandHandle,
) -> i32 {
    // SAFETY: the menu is live for the borrow or null, to which X-Plane fails the add
    // (`append_target`); `name` is NUL-terminated for the call and X-Plane copies it; a
    // `CommandHandle` is a live, never-destroyed SDK command.
    unsafe {
        xp::XPLMAppendMenuItemWithCommand(append_target(menu), name.as_ptr(), command.as_raw())
    }
}

/// `XPLMAppendMenuSeparator(menu or null)`.
pub(crate) fn append_separator_to(menu: Option<LiveMenu<'_>>) {
    // SAFETY: the menu is live for the borrow or null, to which X-Plane fails the add
    // (`append_target`); no other argument.
    unsafe { xp::XPLMAppendMenuSeparator(append_target(menu)) }
}

/// Methods of [`LiveMenu`] for the class of menu calls that pass the menu and
/// plain numbers and nothing else: the menu is live for `'a` (type note); the
/// item index is passed through unchecked. `XPLMMenus.h` does not say what an
/// out-of-range index does; X-Plane has been observed to log one, which is
/// observed behaviour, not a documented guarantee (the
/// `xplm` module note lists it as trusted). Members: `XPLMCheckMenuItem`,
/// `XPLMEnableMenuItem`.
macro_rules! menu_calls {
    ($($(#[$attr:meta])* pub fn $name:ident(self $(, $arg:ident: $ty:ty)* $(,)?) = $sdk:ident;)+) => {$(
        $(#[$attr])*
        pub fn $name(self $(, $arg: $ty)*) {
            // SAFETY: a menu call (macro note): the menu is live for 'a; plain numbers otherwise.
            unsafe { xp::$sdk(self.ptr() $(, $arg)*) }
        }
    )+};
}

/// A menu that stays live for `'a`, never null: X-Plane's own Plugins or
/// Aircraft menu, or a menu a borrowed [`CallbackMenu`] or [`PlainMenu`]
/// owns. The item calls take it.
///
/// Item indices are passed through unchecked; X-Plane has been observed to
/// log an out-of-range one (`XPLMMenus.h` does not document it). The
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

    menu_calls! {
        /// `XPLMCheckMenuItem(menu, index, state)` with a raw `XPLMMenuCheck` value.
        pub fn check_item_state(self, index: i32, state: xp::XPLMMenuCheck) = XPLMCheckMenuItem;
        /// `XPLMEnableMenuItem(menu, index, enabled)`.
        pub fn enable_item(self, index: i32, enabled: i32) = XPLMEnableMenuItem;
    }

    /// `XPLMAppendMenuSeparator(menu)`.
    pub fn append_separator(self) {
        append_separator_to(Some(self))
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
        append_item_to(Some(self), name, item_ref)
    }

    /// `XPLMAppendMenuItemWithCommand(menu, name, command)`.
    ///
    /// # Panics
    /// If `command` is null.
    pub fn append_item_with_command(self, name: &CStr, command: CommandRef) -> i32 {
        let Some(command) = command.handle() else {
            panic!("XPLM command call with the null handle")
        };
        self.append_item_with_command_handle(name, command)
    }

    /// `XPLMAppendMenuItemWithCommand(menu, name, command)` for a command the
    /// crate holds as a [`CommandHandle`].
    pub fn append_item_with_command_handle(self, name: &CStr, command: CommandHandle) -> i32 {
        append_command_to(Some(self), name, command)
    }

    /// `XPLMRemoveMenuItem(menu, index)`. For X-Plane's shared menus prefer an
    /// owned [`crate::MenuItem`], which tracks its current index.
    ///
    /// # Safety
    /// As [`LiveMenu::clear_all_items`], for the item at `index`: it does not
    /// host a submenu that is still alive, and in a shared menu no
    /// [`crate::MenuItem`] of this plugin is alive in it (removing an item
    /// moves the later ones up, so a `MenuItem` would remove the wrong one).
    ///
    /// The safe `Drop` of [`crate::MenuItem`] makes this same call without
    /// that check: that inconsistency is the open gap documented on
    /// [`crate::MenuItem`], not a guarantee that the call is sound there.
    pub unsafe fn remove_item(self, index: i32) {
        // SAFETY: the menu is live for 'a (type note); the caller guarantees the item does not
        // host a live submenu.
        unsafe { xp::XPLMRemoveMenuItem(self.ptr(), index) }
    }
}
