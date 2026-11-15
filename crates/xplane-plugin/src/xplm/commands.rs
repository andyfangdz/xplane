//! `XPLMUtilities.h`, commands: lookup, creation and execution.

use std::cell::RefCell;
use std::ffi::CStr;

use xplane_sdk_sys as xp;

use super::{AddressSet, Found};
use crate::registrations::CommandHandle;

thread_local! {
    static SEEN_COMMANDS: RefCell<AddressSet> = RefCell::new(AddressSet::default());
}

static COMMANDS: Found = Found::new(&SEEN_COMMANDS);

/// A command handle from `XPLMFindCommand` or `XPLMCreateCommand`, or the
/// null handle.
///
/// [`CommandHandle`] is the never-null form the crate's registrations take;
/// [`CommandRef::handle`] converts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct CommandRef(usize);

impl CommandRef {
    pub const NULL: Self = Self(0);

    /// `XPLMFindCommand(name)` (through [`CommandHandle::find`]); the null
    /// handle when X-Plane has no such command.
    pub fn find(name: &CStr) -> Self {
        Self::found(CommandHandle::find(name))
    }

    /// `XPLMCreateCommand(name, description)` (through
    /// [`CommandHandle::create`]): the new or existing command, or null.
    pub fn create(name: &CStr, description: &CStr) -> Self {
        Self::found(CommandHandle::create(name, description))
    }

    fn found(handle: Option<CommandHandle>) -> Self {
        let raw = handle.map_or(0, |handle| handle.as_raw() as usize);
        COMMANDS.insert(raw);
        Self(raw)
    }

    /// The command as a [`CommandHandle`]; `None` for null.
    pub fn handle(self) -> Option<CommandHandle> {
        // SAFETY: a non-null value came from XPLMFindCommand or XPLMCreateCommand (the only
        // constructors), and X-Plane never destroys commands.
        unsafe { CommandHandle::from_raw(self.ptr()) }
    }

    /// The handle that was stored as [`CommandRef::raw`].
    ///
    /// # Panics
    /// If `raw` is neither 0 nor a handle [`CommandRef::find`] or
    /// [`CommandRef::create`] returned in this process.
    pub fn from_raw(raw: usize) -> Self {
        assert!(
            COMMANDS.contains(raw),
            "not an XPLM command handle: {raw:#x}"
        );
        Self(raw)
    }

    pub const fn raw(self) -> usize {
        self.0
    }

    pub const fn is_null(self) -> bool {
        self.0 == 0
    }

    fn ptr(self) -> xp::XPLMCommandRef {
        self.0 as xp::XPLMCommandRef
    }

    /// The handle for a call the SDK does not document as null-safe.
    ///
    /// # Panics
    /// If the handle is null.
    pub(super) fn non_null(self) -> xp::XPLMCommandRef {
        assert!(!self.is_null(), "XPLM command call with the null handle");
        self.ptr()
    }

    /// `XPLMCommandOnce`.
    ///
    /// # Panics
    /// If the handle is null; check the lookup first.
    pub fn once(self) {
        let Some(command) = self.handle() else {
            panic!("XPLM command call with the null handle")
        };
        once_handle(command)
    }

    /// `XPLMCommandBegin`.
    ///
    /// # Panics
    /// If the handle is null.
    pub fn begin(self) {
        let command = self.non_null();
        // SAFETY: a non-null handle is a never-destroyed command (module note); no pointers are
        // passed.
        unsafe { xp::XPLMCommandBegin(command) }
    }

    /// `XPLMCommandEnd`.
    ///
    /// # Panics
    /// If the handle is null.
    pub fn end(self) {
        let command = self.non_null();
        // SAFETY: a non-null handle is a never-destroyed command (module note); no pointers are
        // passed.
        unsafe { xp::XPLMCommandEnd(command) }
    }
}

/// `XPLMCommandOnce` on a command the crate already holds as a
/// [`CommandHandle`], without registering it as a found handle.
pub(crate) fn once_handle(command: CommandHandle) {
    // SAFETY: a `CommandHandle` is a live, never-destroyed SDK command (its `from_raw`
    // contract, or a successful lookup); no pointers are passed.
    unsafe { xp::XPLMCommandOnce(command.as_raw()) }
}
