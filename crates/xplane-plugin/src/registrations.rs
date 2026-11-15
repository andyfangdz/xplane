//! Own callback-backed SDK registrations on the plugin thread.

use std::{
    cell::RefCell,
    collections::BTreeSet,
    convert::Infallible,
    ffi::{CStr, c_void},
    ptr::NonNull,
    rc::Rc,
    thread::{self, ThreadId},
};

use xplane_sdk_sys as sdk;

use crate::{
    CallbackMenu, CommandHandler, DataAccess, DataAccessor, FlightLoopError, FlightLoopFault,
    FlightLoopFrame, FlightLoopHandler, ManagedFlightLoop, callbacks::RetiredAccess, copy_array,
};

/// A command that the SDK owns. The registration owns its handler separately.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommandHandle(NonNull<c_void>);

impl CommandHandle {
    pub fn as_raw(self) -> sdk::XPLMCommandRef {
        self.0.as_ptr()
    }

    /// # Safety
    /// A nonnull handle must refer to a live SDK command on the plugin thread.
    pub unsafe fn from_raw(handle: sdk::XPLMCommandRef) -> Option<Self> {
        NonNull::new(handle).map(Self)
    }

    /// A handle no SDK call is made with, for tests of the dispatch code.
    #[cfg(test)]
    pub(crate) fn from_raw_for_test(raw: usize) -> Self {
        Self(NonNull::new(std::ptr::without_provenance_mut(raw)).unwrap())
    }

    /// Look up an existing command. X-Plane never destroys commands, so the
    /// handle stays valid for the process.
    pub fn find(name: &CStr) -> Option<Self> {
        // SAFETY: the name is NUL-terminated and lives through the call.
        NonNull::new(unsafe { sdk::XPLMFindCommand(name.as_ptr()) }).map(Self)
    }

    /// Create a command, or find it if it already exists.
    pub fn create(name: &CStr, description: &CStr) -> Option<Self> {
        // SAFETY: both strings are NUL-terminated and live through the call.
        NonNull::new(unsafe { sdk::XPLMCreateCommand(name.as_ptr(), description.as_ptr()) })
            .map(Self)
    }
}

/// An SDK menu handle. Parent menu items are managed separately by the caller.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MenuHandle(NonNull<c_void>);

impl MenuHandle {
    pub fn as_raw(self) -> sdk::XPLMMenuID {
        self.0.as_ptr()
    }

    /// # Safety
    /// A nonnull handle must refer to a live SDK menu on the plugin thread.
    pub unsafe fn from_raw(handle: sdk::XPLMMenuID) -> Option<Self> {
        NonNull::new(handle).map(Self)
    }

    /// A handle no SDK call is made with, for tests of the refusal paths.
    #[cfg(test)]
    pub(crate) fn from_raw_for_test(raw: usize) -> Self {
        Self(NonNull::new(std::ptr::without_provenance_mut(raw)).unwrap())
    }
}

pub type ByteWriter = Rc<dyn Fn(&[u8], i32)>;

/// Callbacks for a dataref's SDK scalar, array and byte representations.
#[derive(Default)]
pub struct DataCallbacks {
    pub int: Option<Rc<dyn Fn() -> i32>>,
    pub set_int: Option<Rc<dyn Fn(i32)>>,
    pub float: Option<Rc<dyn Fn() -> f32>>,
    pub set_float: Option<Rc<dyn Fn(f32)>>,
    pub floats: Option<Rc<dyn Fn() -> Vec<f32>>>,
    pub bytes: Option<Rc<dyn Fn() -> Vec<u8>>>,
    pub set_bytes: Option<ByteWriter>,
}

struct RegisteredCommand {
    group: u32,
    /// Dropping it unregisters the handler, then releases the callback.
    _handler: CommandHandler,
}

enum RegisteredData {
    /// Registered by [`Registrations::data`] with these callbacks.
    Callbacks(DataAccessor),
    /// Registered by the caller and handed over; dropping it unregisters it.
    Owned { _accessor: DataAccessor },
    /// A `Callbacks` accessor already unregistered; dropping it releases the
    /// callbacks.
    Retired { _callbacks: RetiredAccess },
}

struct RegisteredMenu {
    handle: MenuHandle,
    /// Dropping it destroys the menu, then releases the callback.
    menu: CallbackMenu,
}

struct AfterPhysics(Box<dyn Fn() -> f32>);

impl FlightLoopHandler for AfterPhysics {
    type Error = Infallible;

    fn frame(&self, _: FlightLoopFrame) -> Result<f32, Self::Error> {
        Ok((self.0)())
    }

    fn fault(&self, _: FlightLoopFault<Self::Error>) {}
}

/// A session's command handlers, accessors, menus and after-physics loop.
///
/// Explicit close methods let the caller choose the teardown order. Drop
/// releases the remaining loop, datarefs, commands by group, and menus from
/// newest to oldest. Callback storage stays live through SDK unregistration.
pub struct Registrations {
    owner: ThreadId,
    commands: RefCell<Vec<Option<RegisteredCommand>>>,
    data: RefCell<Vec<RegisteredData>>,
    menus: RefCell<Vec<RegisteredMenu>>,
    after: RefCell<Option<Rc<ManagedFlightLoop<AfterPhysics>>>>,
}

impl Default for Registrations {
    fn default() -> Self {
        Self {
            owner: thread::current().id(),
            commands: RefCell::new(Vec::new()),
            data: RefCell::new(Vec::new()),
            menus: RefCell::new(Vec::new()),
            after: RefCell::new(None),
        }
    }
}

impl Registrations {
    fn check_thread(&self) {
        if self.owner != thread::current().id() {
            // Kept as an abort: SDK registration calls from another thread
            // would corrupt simulator state. The type is not Send, so safe
            // code cannot reach this.
            std::process::abort();
        }
    }

    pub fn create_command(&self, name: &CStr, description: &CStr) -> Option<CommandHandle> {
        self.check_thread();
        CommandHandle::create(name, description)
    }

    pub fn register_command(
        &self,
        handle: CommandHandle,
        before: bool,
        group: u32,
        callback: impl Fn(CommandHandle, i32) -> i32 + 'static,
    ) {
        self.check_thread();
        // A null or foreign command (which X-Plane does not pass) is consumed
        // without calling the callback; after a fault commands pass through.
        let handler =
            CommandHandler::register_with(handle, before, "command handler", 0, Box::new(callback));
        self.commands.borrow_mut().push(Some(RegisteredCommand {
            group,
            _handler: handler,
        }));
    }

    pub fn close_commands(&self, group: u32) {
        self.check_thread();
        let count = self.commands.borrow().len();
        for index in 0..count {
            let retired = {
                let mut commands = self.commands.borrow_mut();
                let slot = &mut commands[index];
                if slot.as_ref().is_some_and(|entry| entry.group == group) {
                    slot.take()
                } else {
                    None
                }
            };
            // The slot is already retired; dropping the entry unregisters the
            // handler and then releases its callback.
            drop(retired);
        }
    }

    pub fn data(
        &self,
        name: &CStr,
        kind: sdk::XPLMDataTypeID,
        writable: bool,
        callbacks: DataCallbacks,
    ) -> Option<NonNull<c_void>> {
        self.check_thread();
        let DataCallbacks {
            int,
            set_int,
            float,
            set_float,
            floats,
            bytes,
            set_bytes,
        } = callbacks;
        let access = DataAccess {
            int: int.map(|read| Box::new(move || read()) as Box<dyn Fn() -> i32>),
            set_int: set_int.map(|write| Box::new(move |value| write(value)) as Box<dyn Fn(i32)>),
            float: float.map(|read| Box::new(move || read()) as Box<dyn Fn() -> f32>),
            set_float: set_float
                .map(|write| Box::new(move |value| write(value)) as Box<dyn Fn(f32)>),
            floats: floats.map(|read| {
                Box::new(move |out: Option<&mut [f32]>, offset| copy_array(&read(), out, offset))
                    as crate::ArrayRead<f32>
            }),
            bytes: bytes.map(|read| {
                Box::new(move |out: Option<&mut [u8]>, offset| copy_array(&read(), out, offset))
                    as crate::ArrayRead<u8>
            }),
            set_bytes: set_bytes.map(|write| {
                Box::new(move |values: &[u8], offset| write(values, offset))
                    as crate::ArrayWrite<u8>
            }),
            ..DataAccess::default()
        };
        let accessor = DataAccessor::register_labelled(
            name,
            kind,
            writable,
            "dataref read",
            "dataref write",
            access,
        )?;
        let handle = NonNull::new(accessor.handle())?;
        self.data
            .borrow_mut()
            .push(RegisteredData::Callbacks(accessor));
        Some(handle)
    }

    /// Release an accessor the caller registered together with this
    /// session's datarefs.
    pub fn adopt_data(&self, accessor: DataAccessor) {
        self.check_thread();
        self.data.borrow_mut().push(RegisteredData::Owned {
            _accessor: accessor,
        });
    }

    pub fn close_data(&self) {
        self.check_thread();
        let data = std::mem::take(&mut *self.data.borrow_mut());
        // Unregister this session's own accessors first, in order, keeping
        // their callbacks allocated until every entry has been unregistered.
        let data: Vec<_> = data
            .into_iter()
            .map(|entry| match entry {
                RegisteredData::Callbacks(accessor) => RegisteredData::Retired {
                    _callbacks: accessor.retire(),
                },
                other => other,
            })
            .collect();
        // Owned accessors unregister themselves here, in order with the
        // release of the retired callbacks.
        drop(data);
    }

    /// Create a submenu under an existing parent item.
    pub fn menu(
        &self,
        name: &CStr,
        parent: MenuHandle,
        item: i32,
        callback: impl Fn(usize) + 'static,
    ) -> Option<MenuHandle> {
        self.check_thread();
        // Borrow the list before creating the menu: inside `with_menu` this
        // panics before the SDK call rather than after it.
        let mut menus = self.menus.borrow_mut();
        let menu = CallbackMenu::create(name, Some(parent), item, "menu handler", callback)?;
        let handle = menu.handle();
        menus.push(RegisteredMenu { handle, menu });
        Some(handle)
    }

    /// Calls `f` with the menu `handle` for item calls, if it is a menu this
    /// session created and has not closed; `None`, without calling `f`,
    /// otherwise. The session's menus stay borrowed while `f` runs, so the
    /// menu cannot be closed under it: creating or closing a menu of this
    /// session from inside `f` panics.
    pub fn with_menu<T>(
        &self,
        handle: MenuHandle,
        f: impl FnOnce(crate::xplm::LiveMenu<'_>) -> T,
    ) -> Option<T> {
        self.check_thread();
        let menus = self.menus.borrow();
        let entry = menus.iter().find(|entry| entry.handle == handle)?;
        Some(f(crate::xplm::LiveMenu::of(&entry.menu)))
    }

    pub fn close_menu(&self, handle: MenuHandle) {
        self.check_thread();
        let entry = {
            let mut menus = self.menus.borrow_mut();
            menus
                .iter()
                .position(|entry| entry.handle == handle)
                .map(|index| menus.remove(index))
        };
        // Dropping the entry destroys the menu, then releases its callback.
        drop(entry);
    }

    pub fn has_after_physics(&self) -> bool {
        self.after.borrow().is_some()
    }

    /// Create and schedule this session's after-physics callback.
    pub fn start_after_physics(
        &self,
        callback: impl Fn() -> f32 + 'static,
    ) -> Option<NonNull<c_void>> {
        self.check_thread();
        if self.has_after_physics() {
            return None;
        }
        let after =
            Rc::new(ManagedFlightLoop::after_physics(AfterPhysics(Box::new(callback))).ok()?);
        let handle = NonNull::new(after.handle())?;
        *self.after.borrow_mut() = Some(after.clone());
        if after.start().is_err() {
            self.after.borrow_mut().take();
            return None;
        }
        Some(handle)
    }

    pub fn after_physics_executing(&self) -> bool {
        self.after
            .borrow()
            .as_ref()
            .is_some_and(|after| after.executing())
    }

    pub fn suspend_after_physics(&self) -> Result<(), FlightLoopError> {
        self.check_thread();
        let after = self.after.borrow().clone();
        if let Some(after) = after {
            after.stop()?;
        }
        Ok(())
    }

    pub fn close_after_physics(&self) -> Result<(), FlightLoopError> {
        self.check_thread();
        let after = self.after.borrow().clone();
        if let Some(after) = after {
            after.close()?;
            self.after.borrow_mut().take();
        }
        Ok(())
    }
}

impl Drop for Registrations {
    fn drop(&mut self) {
        // If the loop is executing, closing fails and the loop's own Drop
        // leaks its registration and reports a fault instead.
        let _ = self.close_after_physics();
        self.close_data();
        let groups: BTreeSet<_> = self
            .commands
            .get_mut()
            .iter()
            .flatten()
            .map(|entry| entry.group)
            .collect();
        for group in groups {
            self.close_commands(group);
        }
        while let Some(handle) = self.menus.get_mut().last().map(|entry| entry.handle) {
            self.close_menu(handle);
        }
    }
}
