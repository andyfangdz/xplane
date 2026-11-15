//! Items this plugin appends to menus it shares, such as X-Plane's Plugins
//! and Aircraft menus.
//!
//! X-Plane numbers a plugin's own items in a shared menu from 0, and
//! `XPLMRemoveMenuItem` moves the later items up. An index recorded at append
//! time therefore goes stale as soon as an earlier item is removed: removing
//! by it takes out the wrong item, or for the last one makes X-Plane log
//! "XPLMRemoveMenuItem was passed an out of range item index". A [`MenuItem`]
//! records its place among this plugin's items in its parent and removes
//! itself at its current index, so items can go in any order.

use std::{
    cell::{Cell, RefCell},
    ffi::{CStr, c_int, c_void},
};

use xplane_sdk_sys as sdk;

use crate::registrations::MenuHandle;

thread_local! {
    /// This plugin's live items in shared menus, oldest first, as
    /// (parent, token) pairs.
    static ITEMS: RefCell<Vec<(usize, u64)>> = const { RefCell::new(Vec::new()) };
    static NEXT: Cell<u64> = const { Cell::new(0) };
}

/// An item appended to a shared menu. Drop removes it at its current index.
///
/// Every item this plugin appends to `parent` must be a `MenuItem`, or the
/// indices it computes will not match X-Plane's. Create and drop it on
/// X-Plane's plugin thread.
///
/// # Open gap: removal and destroy order
///
/// This is the one statement of this gap; [`crate::xplm::PlainMenu::create`],
/// [`crate::CallbackMenu::create`] and [`crate::xplm::LiveMenu::remove_item`]
/// refer to it.
///
/// Enforced: no `XPLMCreateMenu`, `XPLMAppendMenuItem` or
/// `XPLMRemoveMenuItem` call is made with a parent menu that one of the
/// crate's owners ([`crate::xplm::PlainMenu`], [`crate::CallbackMenu`],
/// [`crate::PluginMenu`], the session menus of
/// [`crate::registrations::Registrations`]) has destroyed (those calls return
/// `None` or make no call), and each owned menu is destroyed exactly once, on
/// drop.
///
/// Not enforced, and left undefined by `XPLMMenus.h`: what X-Plane does when
/// an item that hosts a live submenu is removed (this `Drop` makes that call,
/// as [`crate::xplm::LiveMenu::remove_item`] does, which is `unsafe` for this
/// reason), and when a parent menu is destroyed while a submenu under it
/// lives (whether X-Plane destroys the submenu with it, and what the
/// submenu's own later `XPLMDestroyMenu` then does). The crate does not track
/// submenus to skip a removal or a destroy, so existing code keeps making
/// exactly the calls it makes now. Dropping an item or a parent before a
/// submenu under it therefore relies on behaviour the header does not
/// define. This is an open gap, not a guarantee: drop submenus before the
/// items that host them and before their parents.
#[must_use = "dropping a menu item immediately removes it"]
pub struct MenuItem {
    parent: MenuHandle,
    token: u64,
}

impl MenuItem {
    /// Append `name` to `parent` with the opaque item reference `item_ref`,
    /// which X-Plane passes back to the parent menu's handler as a pointer.
    /// Returns `None` if X-Plane refuses, or, without a call, if one of the
    /// crate's menu owners has already destroyed `parent`.
    pub fn append(parent: MenuHandle, name: &CStr, item_ref: usize) -> Option<Self> {
        if crate::xplm::menus::is_destroyed(parent) {
            return None;
        }
        // SAFETY: the parent is a live SDK menu (a `MenuHandle` the crate's owners have not
        // destroyed: checked) and the name is NUL-terminated for the call; X-Plane only stores
        // `item_ref`, never dereferences it.
        let index = unsafe {
            sdk::XPLMAppendMenuItem(parent.as_raw(), name.as_ptr(), item_ref as *mut c_void, 0)
        };
        if index < 0 {
            return None;
        }
        let token = NEXT.with(|next| {
            let token = next.get();
            next.set(token + 1);
            token
        });
        ITEMS.with_borrow_mut(|items| items.push((parent.as_raw() as usize, token)));
        Some(Self { parent, token })
    }

    /// The menu this item is in.
    pub fn parent(&self) -> MenuHandle {
        self.parent
    }

    /// The item's current index in its parent.
    pub fn index(&self) -> c_int {
        position(self.parent.as_raw() as usize, self.token)
    }
}

impl Drop for MenuItem {
    fn drop(&mut self) {
        let index = self.index();
        // A parent one of the crate's owners destroyed took its items with it:
        // removing from it would pass X-Plane a dead menu, so make no call.
        if !crate::xplm::menus::is_destroyed(self.parent) {
            // SAFETY: the parent is a live SDK menu (not destroyed by the crate's owners:
            // checked) and `index` is this item's current place among this plugin's items in it.
            // NOT covered: whether the item hosts a live submenu, which `XPLMMenus.h` leaves
            // undefined (open gap on `MenuItem`).
            unsafe { sdk::XPLMRemoveMenuItem(self.parent.as_raw(), index) };
        }
        ITEMS.with_borrow_mut(|items| items.retain(|&(_, token)| token != self.token));
    }
}

/// The number of this plugin's items in `parent` that precede `token`.
fn position(parent: usize, token: u64) -> c_int {
    ITEMS.with_borrow(|items| {
        items
            .iter()
            .take_while(|&&(_, t)| t != token)
            .filter(|&&(p, _)| p == parent)
            .count() as c_int
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positions_close_up_as_earlier_items_go() {
        // Two parents with interleaved items, as the Plugins and Aircraft
        // menus get them from several owners.
        ITEMS.with_borrow_mut(|items| {
            items.clear();
            items.extend([(1, 10), (2, 11), (1, 12), (1, 13), (2, 14)]);
        });
        assert_eq!(
            [position(1, 10), position(1, 12), position(1, 13)],
            [0, 1, 2]
        );
        assert_eq!([position(2, 11), position(2, 14)], [0, 1]);
        // Removing the first item of parent 1 moves its later items up; the
        // stale index 2 would now be out of range.
        ITEMS.with_borrow_mut(|items| items.retain(|&(_, t)| t != 10));
        assert_eq!([position(1, 12), position(1, 13)], [0, 1]);
        assert_eq!([position(2, 11), position(2, 14)], [0, 1]);
    }
}
