//! Own callback-backed SDK registrations on the plugin thread.

use std::{
    cell::RefCell,
    collections::BTreeSet,
    convert::Infallible,
    ffi::{CStr, c_void},
    ptr::{self, NonNull},
    rc::Rc,
    thread::{self, ThreadId},
};

use xplane_sdk_sys as sdk;

use crate::{
    FlightLoopError, FlightLoopFault, FlightLoopFrame, FlightLoopHandler, ManagedFlightLoop,
    guard::guard,
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

type CommandCallback = Rc<dyn Fn(CommandHandle, i32) -> i32>;
type MenuCallback = Rc<dyn Fn(usize)>;

struct RegisteredCommand {
    handle: CommandHandle,
    group: u32,
    before: i32,
    callback: Rc<CommandCallback>,
}

struct RegisteredData {
    handle: NonNull<c_void>,
    _callbacks: Option<Rc<DataCallbacks>>,
}

struct RegisteredMenu {
    handle: MenuHandle,
    _callback: Rc<MenuCallback>,
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
        // SAFETY: both strings are NUL-terminated and live through the call.
        let handle = unsafe { sdk::XPLMCreateCommand(name.as_ptr(), description.as_ptr()) };
        NonNull::new(handle).map(CommandHandle)
    }

    pub fn register_command(
        &self,
        handle: CommandHandle,
        before: bool,
        group: u32,
        callback: impl Fn(CommandHandle, i32) -> i32 + 'static,
    ) {
        self.check_thread();
        let callback: Rc<CommandCallback> = Rc::new(Rc::new(callback));
        let before = i32::from(before);
        // SAFETY: the command came from the SDK. The callback allocation remains
        // live through registration and until its handler is unregistered.
        unsafe {
            sdk::XPLMRegisterCommandHandler(
                handle.as_raw(),
                Some(command),
                before,
                Rc::as_ptr(&callback).cast_mut().cast(),
            );
        }
        self.commands.borrow_mut().push(Some(RegisteredCommand {
            handle,
            group,
            before,
            callback,
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
            if let Some(entry) = retired {
                // SAFETY: this is the tuple registered above. The callback
                // remains live for the call and the slot is already retired.
                unsafe {
                    sdk::XPLMUnregisterCommandHandler(
                        entry.handle.as_raw(),
                        Some(command),
                        entry.before,
                        Rc::as_ptr(&entry.callback).cast_mut().cast(),
                    );
                }
            }
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
        let callbacks = Rc::new(callbacks);
        let context = Rc::as_ptr(&callbacks).cast_mut().cast();
        // SAFETY: callbacks use the stable allocation as their refcon. The
        // name is valid for the call, and successful registration retains it.
        let handle = unsafe {
            sdk::XPLMRegisterDataAccessor(
                name.as_ptr(),
                kind,
                i32::from(writable),
                callbacks.int.as_ref().map(|_| get_int as _),
                (writable && callbacks.set_int.is_some()).then_some(put_int as _),
                callbacks.float.as_ref().map(|_| get_float as _),
                (writable && callbacks.set_float.is_some()).then_some(put_float as _),
                None,
                None,
                None,
                None,
                callbacks.floats.as_ref().map(|_| get_floats as _),
                None,
                callbacks.bytes.as_ref().map(|_| get_bytes as _),
                (writable && callbacks.set_bytes.is_some()).then_some(put_bytes as _),
                context,
                if writable { context } else { ptr::null_mut() },
            )
        };
        let handle = NonNull::new(handle)?;
        self.data.borrow_mut().push(RegisteredData {
            handle,
            _callbacks: Some(callbacks),
        });
        Some(handle)
    }

    /// Take ownership of unregistering an accessor registered by the caller.
    ///
    /// # Safety
    /// `handle` must be a live owned accessor from this SDK. The caller must
    /// keep any existing callback context valid until it is unregistered.
    pub unsafe fn adopt_data(&self, handle: NonNull<c_void>) {
        self.check_thread();
        self.data.borrow_mut().push(RegisteredData {
            handle,
            _callbacks: None,
        });
    }

    pub fn close_data(&self) {
        self.check_thread();
        let data = std::mem::take(&mut *self.data.borrow_mut());
        for entry in &data {
            // SAFETY: each live handle is owned by this session. Keep the
            // callbacks allocated through all unregistration calls.
            unsafe { sdk::XPLMUnregisterDataAccessor(entry.handle.as_ptr()) };
        }
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
        let callback: Rc<MenuCallback> = Rc::new(Rc::new(callback));
        // SAFETY: the parent came from the SDK and the callback lives until
        // this registration is destroyed. The name is valid for this call.
        let handle = unsafe {
            sdk::XPLMCreateMenu(
                name.as_ptr(),
                parent.as_raw(),
                item,
                Some(menu),
                Rc::as_ptr(&callback).cast_mut().cast(),
            )
        };
        let handle = MenuHandle(NonNull::new(handle)?);
        self.menus.borrow_mut().push(RegisteredMenu {
            handle,
            _callback: callback,
        });
        Some(handle)
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
        if let Some(entry) = entry {
            // SAFETY: this session owns the menu and keeps its callback
            // allocated through destruction.
            unsafe { sdk::XPLMDestroyMenu(entry.handle.as_raw()) };
        }
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

unsafe extern "C" fn command(handle: sdk::XPLMCommandRef, phase: i32, context: *mut c_void) -> i32 {
    // SAFETY: registration provides a live, stable CommandCallback. Clone it
    // before dispatch so it survives reentrant unregistration.
    let callback = unsafe { &*context.cast::<CommandCallback>() }.clone();
    // After a fault, let X-Plane and other handlers process the command.
    guard("command handler", 1, || {
        NonNull::new(handle).map_or(0, |handle| callback(CommandHandle(handle), phase))
    })
}

unsafe extern "C" fn menu(context: *mut c_void, item: *mut c_void) {
    // SAFETY: registration provides a live MenuCallback. Clone it so a menu
    // can be destroyed during its own callback.
    let callback = unsafe { &*context.cast::<MenuCallback>() }.clone();
    guard("menu handler", (), || callback(item as usize));
}

unsafe extern "C" fn get_int(context: *mut c_void) -> i32 {
    // SAFETY: only registered with a live DataCallbacks containing this reader.
    let Some(read) = unsafe { &*context.cast::<DataCallbacks>() }.int.clone() else {
        return Default::default();
    };
    guard("dataref read", Default::default(), || read())
}

unsafe extern "C" fn put_int(context: *mut c_void, value: i32) {
    // SAFETY: only registered with a live DataCallbacks containing this writer.
    let Some(write) = unsafe { &*context.cast::<DataCallbacks>() }.set_int.clone() else {
        return;
    };
    guard("dataref write", (), || write(value));
}

unsafe extern "C" fn get_float(context: *mut c_void) -> f32 {
    // SAFETY: only registered with a live DataCallbacks containing this reader.
    let Some(read) = unsafe { &*context.cast::<DataCallbacks>() }.float.clone() else {
        return Default::default();
    };
    guard("dataref read", Default::default(), || read())
}

unsafe extern "C" fn put_float(context: *mut c_void, value: f32) {
    // SAFETY: only registered with a live DataCallbacks containing this writer.
    let Some(write) = unsafe { &*context.cast::<DataCallbacks>() }
        .set_float
        .clone()
    else {
        return;
    };
    guard("dataref write", (), || write(value));
}

unsafe fn read_array<T: Copy>(values: &[T], out: *mut T, offset: i32, count: i32) -> i32 {
    if out.is_null() {
        return i32::try_from(values.len()).unwrap_or(i32::MAX);
    }
    if offset < 0 || offset as usize >= values.len() || count <= 0 {
        return 0;
    }
    let length = (count as usize).min(values.len() - offset as usize);
    // SAFETY: SDK's output has space for count items. The source range is
    // bounded above and comes from separate Rust storage.
    unsafe { ptr::copy_nonoverlapping(values.as_ptr().add(offset as usize), out, length) };
    length as i32
}

unsafe extern "C" fn get_floats(
    context: *mut c_void,
    out: *mut f32,
    offset: i32,
    count: i32,
) -> i32 {
    // SAFETY: only registered with this reader present and SDK-owned output.
    let Some(read) = unsafe { &*context.cast::<DataCallbacks>() }.floats.clone() else {
        return 0;
    };
    guard("dataref read", 0, || {
        // SAFETY: the SDK supplies an output with room for `count` values.
        unsafe { read_array(&read(), out, offset, count) }
    })
}

unsafe extern "C" fn get_bytes(
    context: *mut c_void,
    out: *mut c_void,
    offset: i32,
    count: i32,
) -> i32 {
    // SAFETY: only registered with this reader present and SDK-owned output.
    let Some(read) = unsafe { &*context.cast::<DataCallbacks>() }.bytes.clone() else {
        return 0;
    };
    guard("dataref read", 0, || {
        // SAFETY: the SDK supplies an output with room for `count` bytes.
        unsafe { read_array(&read(), out.cast(), offset, count) }
    })
}

unsafe extern "C" fn put_bytes(context: *mut c_void, input: *mut c_void, offset: i32, count: i32) {
    if input.is_null() || count <= 0 {
        return;
    }
    // SAFETY: this writer was registered with a live DataCallbacks. The SDK
    // lends count input bytes for this call only.
    let Some(write) = unsafe { &*context.cast::<DataCallbacks>() }
        .set_bytes
        .clone()
    else {
        return;
    };
    // SAFETY: the SDK lends `count` readable input bytes for this call only.
    let input = unsafe { std::slice::from_raw_parts(input.cast(), count as usize) };
    guard("dataref write", (), || write(input, offset));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn menu_callback_survives_reentrant_registration_destruction() {
        let registration = Rc::new(RefCell::new(None::<Rc<MenuCallback>>));
        let captured = Rc::downgrade(&registration);
        let seen = Rc::new(Cell::new(None));
        let observed = seen.clone();
        let callback: Rc<MenuCallback> = Rc::new(Rc::new(move |item| {
            let registration = captured.upgrade().unwrap();
            drop(registration.borrow_mut().take().unwrap());
            observed.set(Some(item));
        }));
        let context = Rc::as_ptr(&callback).cast_mut().cast();
        *registration.borrow_mut() = Some(callback);

        // SAFETY: the simulated SDK uses the exact registered callback
        // context. The callback retires its own registration during dispatch.
        unsafe { menu(context, 42usize as *mut c_void) };
        assert!(registration.borrow().is_none());
        assert_eq!(seen.get(), Some(42));
    }
}
