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
#[must_use = "dropping a menu item immediately removes it"]
pub struct MenuItem {
    parent: MenuHandle,
    token: u64,
}

impl MenuItem {
    /// Append `name` to `parent` with the opaque item reference `item_ref`,
    /// which X-Plane passes back to the parent menu's handler as a pointer.
    /// Returns `None` if X-Plane refuses.
    pub fn append(parent: MenuHandle, name: &CStr, item_ref: usize) -> Option<Self> {
        // SAFETY: the parent is a live SDK menu and the name is NUL-terminated
        // for the call; X-Plane only stores `item_ref`, never dereferences it.
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
        // SAFETY: the parent is a live SDK menu and `index` is this item's
        // current place among this plugin's items in it.
        unsafe { sdk::XPLMRemoveMenuItem(self.parent.as_raw(), index) };
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
