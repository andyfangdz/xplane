//! Typed, owned callback registrations.
//!
//! Each value owns one SDK registration and the Rust callback X-Plane reaches
//! through its refcon. The trampoline leases the callback (takes a strong
//! reference) for the duration of the call, so a callback may drop its own
//! registration, and runs it inside [`crate::guard`]: a panic is contained,
//! and after a fault the trampoline returns its pass-through default without
//! running plugin code. Drop unregisters the exact tuple that was registered.
//! The values are not `Send`; create, use and drop them on X-Plane's plugin
//! thread.
//!
//! # Refcons
//!
//! A refcon is never turned back into a reference by a pointer cast. Each one
//! is the address of an `Rc` allocation entered in a per-thread registry of
//! live refcons ([`Refcon`]), and a trampoline gets its state back with
//! [`lease`], which looks the address up, checks the type and takes a strong
//! reference. So the trampolines are safe functions: an address that is not
//! a live refcon of the expected type (which X-Plane never passes) only makes
//! the trampoline return its fallback. The registry's weak reference keeps the
//! allocation's address reserved until the entry is removed, so an address is
//! never reused while it is registered.

use std::{
    any::Any,
    cell::{Cell, RefCell},
    collections::HashMap,
    ffi::{CStr, c_char, c_int, c_void},
    mem,
    ptr::{self, NonNull},
    rc::{Rc, Weak},
};

use xplane_sdk_sys as sdk;

use crate::{
    FlightLoopFrame, PhaseFlightLoop,
    guard::{guard, guard_read},
    registrations::{CommandHandle, MenuHandle},
};

thread_local! {
    /// Live refcons of this thread: address to the allocation behind it.
    static LIVE: RefCell<HashMap<usize, Weak<dyn Any>>> = RefCell::new(HashMap::new());
}

/// The registry entry for one refcon; dropping it removes the entry.
pub(crate) struct Refcon {
    key: usize,
}

impl Refcon {
    /// Enter the allocation of `target` as a refcon.
    pub(crate) fn new<T: Any>(target: &Weak<T>) -> Self {
        let key = Weak::as_ptr(target).addr();
        let entry: Weak<dyn Any> = target.clone();
        LIVE.with_borrow_mut(|live| {
            let previous = live.insert(key, entry);
            debug_assert!(previous.is_none(), "refcon registered twice");
        });
        Self { key }
    }

    /// The address to hand X-Plane as the refcon.
    pub(crate) fn as_ptr(&self) -> *mut c_void {
        ptr::without_provenance_mut(self.key)
    }
}

impl Drop for Refcon {
    fn drop(&mut self) {
        // The registry is gone only while this thread is exiting, and then no
        // trampoline runs on it again.
        let _ = LIVE.try_with(|live| {
            if let Ok(mut live) = live.try_borrow_mut() {
                live.remove(&self.key);
            }
        });
    }
}

/// The live `T` registered at `refcon`, with a strong reference for the
/// duration of one SDK call (so a callback may drop its own owner), or
/// `None` when `refcon` is not a live refcon of type `T` on this thread.
pub(crate) fn lease<T: Any>(refcon: *mut c_void) -> Option<Rc<T>> {
    let entry = LIVE
        .try_with(|live| live.try_borrow().ok()?.get(&refcon.addr()).cloned())
        .ok()
        .flatten()?;
    entry.upgrade()?.downcast::<T>().ok()
}

/// A callback allocation whose address is an SDK refcon.
struct Slot<T> {
    label: &'static str,
    value: T,
}

/// A slot entered as a refcon. Owners keep it after their SDK registration
/// field, so the registration is released first.
struct Registered<T: 'static> {
    slot: Rc<Slot<T>>,
    refcon: Refcon,
}

impl<T: 'static> Registered<T> {
    fn new(label: &'static str, value: T) -> Self {
        let slot = Rc::new(Slot { label, value });
        let refcon = Refcon::new(&Rc::downgrade(&slot));
        Self { slot, refcon }
    }

    fn refcon(&self) -> *mut c_void {
        self.refcon.as_ptr()
    }
}

impl<T: 'static> std::ops::Deref for Registered<T> {
    type Target = Slot<T>;

    fn deref(&self) -> &Slot<T> {
        &self.slot
    }
}

/// A handle an SDK create or register call returned, released exactly once
/// on drop by the SDK call that undoes it. Not `Clone`.
struct SdkOwned {
    handle: NonNull<c_void>,
    release: unsafe extern "C" fn(*mut c_void),
}

impl SdkOwned {
    /// Own `handle`, or `None` when it is null (the SDK refused).
    ///
    /// # Safety
    /// `handle` is the result of the SDK call that `release` undoes, made
    /// just now, and nothing else releases it.
    unsafe fn new(handle: *mut c_void, release: unsafe extern "C" fn(*mut c_void)) -> Option<Self> {
        Some(Self {
            handle: NonNull::new(handle)?,
            release,
        })
    }

    fn as_ptr(&self) -> *mut c_void {
        self.handle.as_ptr()
    }
}

impl Drop for SdkOwned {
    fn drop(&mut self) {
        // SAFETY: by `new`'s contract `release` undoes the call that returned
        // the handle, and this value releases it once.
        unsafe { (self.release)(self.handle.as_ptr()) };
    }
}

/// Runs `call` on the slot registered at `context` inside [`guard`] under the
/// slot's label; `fallback` after a fault or for an unknown refcon.
fn enter<T: 'static, R>(context: *mut c_void, fallback: R, call: impl FnOnce(&Slot<T>) -> R) -> R {
    match lease::<Slot<T>>(context) {
        Some(slot) => guard(slot.label, fallback, || call(&slot)),
        None => fallback,
    }
}

type CommandFn = Box<dyn Fn(CommandHandle, sdk::XPLMCommandPhase) -> c_int>;

struct CommandSlot {
    command: CommandHandle,
    /// Returned without calling the callback when X-Plane passes a command
    /// other than the registered one (it passes the registered one).
    unknown_command: c_int,
    callback: CommandFn,
}

/// An owned command handler registration.
#[must_use = "dropping a handler immediately unregisters it"]
pub struct CommandHandler {
    before: c_int,
    slot: Registered<CommandSlot>,
}

impl CommandHandler {
    /// Register `callback` for `command`. The callback returns 1 to let
    /// X-Plane and later handlers process the command, or 0 to consume it.
    /// After a plugin fault the handler passes every command through.
    pub fn register(
        command: CommandHandle,
        before: bool,
        label: &'static str,
        callback: impl Fn(CommandHandle, sdk::XPLMCommandPhase) -> c_int + 'static,
    ) -> Self {
        Self::register_with(command, before, label, 1, Box::new(callback))
    }

    /// [`CommandHandler::register`] that returns `unknown_command` when the
    /// callback gets a null or foreign command handle.
    pub(crate) fn register_with(
        command: CommandHandle,
        before: bool,
        label: &'static str,
        unknown_command: c_int,
        callback: CommandFn,
    ) -> Self {
        let slot = Registered::new(
            label,
            CommandSlot {
                command,
                unknown_command,
                callback,
            },
        );
        let before = c_int::from(before);
        // SAFETY: `command` is a live SDK command (a `CommandHandle`). The
        // trampoline is a safe function, and Drop unregisters this tuple.
        unsafe {
            sdk::XPLMRegisterCommandHandler(
                command.as_raw(),
                Some(command_trampoline),
                before,
                slot.refcon(),
            );
        }
        Self { before, slot }
    }

    pub fn command(&self) -> CommandHandle {
        self.slot.value.command
    }
}

impl Drop for CommandHandler {
    fn drop(&mut self) {
        // SAFETY: this is the tuple registered in `register_with`, for a
        // never-destroyed command.
        unsafe {
            sdk::XPLMUnregisterCommandHandler(
                self.command().as_raw(),
                Some(command_trampoline),
                self.before,
                self.slot.refcon(),
            );
        }
    }
}

extern "C" fn command_trampoline(
    command: sdk::XPLMCommandRef,
    phase: sdk::XPLMCommandPhase,
    context: *mut c_void,
) -> c_int {
    enter(context, 1, |slot: &Slot<CommandSlot>| {
        let entry = &slot.value;
        // X-Plane passes the command the handler was registered for, so the
        // registered handle stands for it.
        if !command.is_null() && command == entry.command.as_raw() {
            (entry.callback)(entry.command, phase)
        } else {
            entry.unknown_command
        }
    })
}

/// Reads `values[offset..]` into an SDK array output, or reports the array
/// length when the SDK asks for it with a null output.
pub fn copy_array<T: Copy>(values: &[T], out: Option<&mut [T]>, offset: c_int) -> c_int {
    let Some(out) = out else {
        return c_int::try_from(values.len()).unwrap_or(c_int::MAX);
    };
    let Ok(offset) = usize::try_from(offset) else {
        return 0;
    };
    if offset >= values.len() || out.is_empty() {
        return 0;
    }
    let length = out.len().min(values.len() - offset);
    out[..length].copy_from_slice(&values[offset..offset + length]);
    length as c_int
}

/// An array reader: `None` asks for the array length; otherwise fill the
/// output starting at the element offset and return the count written.
pub type ArrayRead<T> = Box<dyn Fn(Option<&mut [T]>, c_int) -> c_int>;
/// An array writer: the values and their element offset.
pub type ArrayWrite<T> = Box<dyn Fn(&[T], c_int)>;

/// Callbacks for an owned dataref. Only the representations that are present
/// are registered; setters are registered only for writable datarefs.
#[derive(Default)]
pub struct DataAccess {
    pub int: Option<Box<dyn Fn() -> c_int>>,
    pub set_int: Option<Box<dyn Fn(c_int)>>,
    pub float: Option<Box<dyn Fn() -> f32>>,
    pub set_float: Option<Box<dyn Fn(f32)>>,
    pub double: Option<Box<dyn Fn() -> f64>>,
    pub set_double: Option<Box<dyn Fn(f64)>>,
    pub ints: Option<ArrayRead<c_int>>,
    pub set_ints: Option<ArrayWrite<c_int>>,
    pub floats: Option<ArrayRead<f32>>,
    pub set_floats: Option<ArrayWrite<f32>>,
    pub bytes: Option<ArrayRead<u8>>,
    pub set_bytes: Option<ArrayWrite<u8>>,
}

/// What a [`DataAccessor`] slot holds: the callbacks, the label of the
/// write callbacks (reads use the slot's label) and how writes are guarded.
struct DataSlot {
    access: DataAccess,
    write_label: &'static str,
    writes: Writes,
}

/// The registration shape of a [`DataAccessor`].
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Writes {
    /// Writes run under [`guard`], so not after a fault, and a read-only
    /// accessor is registered with a null write refcon.
    Guarded,
    /// For owners whose write callbacks only set plugin-owned cells and
    /// guard their own change notification: writes run under
    /// [`guard_read`], so they still land after a fault (as reads still
    /// answer), and the refcon is passed as the write refcon even for a
    /// read-only accessor (which registers no write callback).
    Storage,
}

/// An owned dataref accessor registration.
#[must_use = "dropping an accessor immediately unregisters it"]
pub struct DataAccessor {
    // Field order: the accessor is unregistered before its callbacks go.
    handle: SdkOwned,
    slot: Registered<DataSlot>,
}

/// The callbacks of a [`DataAccessor`] that has been unregistered, kept so
/// the caller chooses when they are dropped.
pub(crate) struct RetiredAccess {
    _slot: Registered<DataSlot>,
}

impl DataAccessor {
    /// Publish `name`. Returns `None` if X-Plane refuses the registration.
    pub fn register(
        name: &CStr,
        kind: sdk::XPLMDataTypeID,
        writable: bool,
        label: &'static str,
        access: DataAccess,
    ) -> Option<Self> {
        Self::register_labelled(name, kind, writable, label, label, access)
    }

    /// [`DataAccessor::register`] with separate guard labels for the read
    /// and the write callbacks.
    pub(crate) fn register_labelled(
        name: &CStr,
        kind: sdk::XPLMDataTypeID,
        writable: bool,
        read_label: &'static str,
        write_label: &'static str,
        access: DataAccess,
    ) -> Option<Self> {
        Self::register_shaped(
            name,
            kind,
            writable,
            read_label,
            write_label,
            Writes::Guarded,
            access,
        )
    }

    /// [`DataAccessor::register_labelled`] with the write shape `writes`.
    pub(crate) fn register_shaped(
        name: &CStr,
        kind: sdk::XPLMDataTypeID,
        writable: bool,
        read_label: &'static str,
        write_label: &'static str,
        writes: Writes,
        access: DataAccess,
    ) -> Option<Self> {
        let w = |present: bool| writable && present;
        let slot = Registered::new(
            read_label,
            DataSlot {
                access,
                write_label,
                writes,
            },
        );
        let a = &slot.value.access;
        // SAFETY: the name is NUL-terminated for the call. Every trampoline is
        // a safe function or one of the two array trampolines, whose pointer
        // contract is the SDK's (see `get_array`). Each is registered only
        // when its callback is present. `SdkOwned` unregisters the accessor
        // once, before the refcon leaves the registry (field order).
        let handle = unsafe {
            let handle = sdk::XPLMRegisterDataAccessor(
                name.as_ptr(),
                kind,
                c_int::from(writable),
                a.int.is_some().then_some(get_int as _),
                w(a.set_int.is_some()).then_some(set_int as _),
                a.float.is_some().then_some(get_float as _),
                w(a.set_float.is_some()).then_some(set_float as _),
                a.double.is_some().then_some(get_double as _),
                w(a.set_double.is_some()).then_some(set_double as _),
                a.ints.is_some().then_some(get_array::<Ints> as _),
                w(a.set_ints.is_some()).then_some(set_array::<Ints> as _),
                a.floats.is_some().then_some(get_array::<Floats> as _),
                w(a.set_floats.is_some()).then_some(set_array::<Floats> as _),
                a.bytes.is_some().then_some(get_array::<Bytes> as _),
                w(a.set_bytes.is_some()).then_some(set_array::<Bytes> as _),
                slot.refcon(),
                if writable || writes == Writes::Storage {
                    slot.refcon()
                } else {
                    ptr::null_mut()
                },
            );
            SdkOwned::new(handle, sdk::XPLMUnregisterDataAccessor)
        }?;
        Some(Self { handle, slot })
    }

    pub fn handle(&self) -> sdk::XPLMDataRef {
        self.handle.as_ptr()
    }

    /// Unregister the accessor now and hand back its callbacks.
    pub(crate) fn retire(self) -> RetiredAccess {
        let Self { handle, slot } = self;
        drop(handle);
        RetiredAccess { _slot: slot }
    }
}

/// Runs a scalar read under [`guard_read`] with the slot's (read) label.
fn scalar<T: Default>(
    context: *mut c_void,
    pick: impl FnOnce(&DataAccess) -> Option<&dyn Fn() -> T>,
) -> T {
    let Some(slot) = lease::<Slot<DataSlot>>(context) else {
        return T::default();
    };
    guard_read(slot.label, T::default(), || {
        pick(&slot.value.access).map_or_else(T::default, |read| read())
    })
}

/// Runs a write body with the slot's write label, under [`guard`] or, for
/// [`Writes::Storage`], [`guard_read`].
fn guard_write(slot: &DataSlot, body: impl FnOnce()) {
    match slot.writes {
        Writes::Guarded => guard(slot.write_label, (), body),
        Writes::Storage => guard_read(slot.write_label, (), body),
    }
}

/// Runs a scalar write through [`guard_write`].
fn store<T>(context: *mut c_void, value: T, pick: impl FnOnce(&DataAccess) -> Option<&dyn Fn(T)>) {
    let Some(slot) = lease::<Slot<DataSlot>>(context) else {
        return;
    };
    guard_write(&slot.value, || {
        if let Some(write) = pick(&slot.value.access) {
            write(value);
        }
    });
}

extern "C" fn get_int(context: *mut c_void) -> c_int {
    scalar(context, |a| a.int.as_deref())
}

extern "C" fn set_int(context: *mut c_void, value: c_int) {
    store(context, value, |a| a.set_int.as_deref())
}

extern "C" fn get_float(context: *mut c_void) -> f32 {
    scalar(context, |a| a.float.as_deref())
}

extern "C" fn set_float(context: *mut c_void, value: f32) {
    store(context, value, |a| a.set_float.as_deref())
}

extern "C" fn get_double(context: *mut c_void) -> f64 {
    scalar(context, |a| a.double.as_deref())
}

extern "C" fn set_double(context: *mut c_void, value: f64) {
    store(context, value, |a| a.set_double.as_deref())
}

/// One array representation of a dataref: its element type, the pointer type
/// the SDK callback signature uses for it, and its two callbacks.
trait ArrayKind: 'static {
    type Elem: 'static;
    type Raw;
    fn read(access: &DataAccess) -> Option<&ArrayRead<Self::Elem>>;
    fn write(access: &DataAccess) -> Option<&ArrayWrite<Self::Elem>>;
}

struct Ints;
struct Floats;
struct Bytes;

impl ArrayKind for Ints {
    type Elem = c_int;
    type Raw = c_int;
    fn read(access: &DataAccess) -> Option<&ArrayRead<c_int>> {
        access.ints.as_ref()
    }
    fn write(access: &DataAccess) -> Option<&ArrayWrite<c_int>> {
        access.set_ints.as_ref()
    }
}

impl ArrayKind for Floats {
    type Elem = f32;
    type Raw = f32;
    fn read(access: &DataAccess) -> Option<&ArrayRead<f32>> {
        access.floats.as_ref()
    }
    fn write(access: &DataAccess) -> Option<&ArrayWrite<f32>> {
        access.set_floats.as_ref()
    }
}

impl ArrayKind for Bytes {
    type Elem = u8;
    type Raw = c_void;
    fn read(access: &DataAccess) -> Option<&ArrayRead<u8>> {
        access.bytes.as_ref()
    }
    fn write(access: &DataAccess) -> Option<&ArrayWrite<u8>> {
        access.set_bytes.as_ref()
    }
}

/// The SDK's array read callback (`XPLMGetDatavi_f`, `_vf`, `_b`).
///
/// # Safety
/// `out` is null or valid for `count` writes of `A::Elem` for this call, as
/// X-Plane promises when it calls a registered array reader.
unsafe extern "C" fn get_array<A: ArrayKind>(
    context: *mut c_void,
    out: *mut A::Raw,
    offset: c_int,
    count: c_int,
) -> c_int {
    let Some(slot) = lease::<Slot<DataSlot>>(context) else {
        return 0;
    };
    let out = out.cast::<A::Elem>();
    let out = if out.is_null() {
        None
    } else {
        let length = usize::try_from(count).unwrap_or(0);
        // SAFETY: the caller lends `count` writable elements for this call only.
        Some(unsafe { std::slice::from_raw_parts_mut(out, length) })
    };
    guard_read(slot.label, 0, || {
        A::read(&slot.value.access).map_or(0, |read| read(out, offset))
    })
}

/// The SDK's array write callback (`XPLMSetDatavi_f`, `_vf`, `_b`).
///
/// # Safety
/// `values` is null or valid for `count` reads of `A::Elem` for this call, as
/// X-Plane promises when it calls a registered array writer.
unsafe extern "C" fn set_array<A: ArrayKind>(
    context: *mut c_void,
    values: *mut A::Raw,
    offset: c_int,
    count: c_int,
) {
    if values.is_null() || count <= 0 {
        return;
    }
    let Some(slot) = lease::<Slot<DataSlot>>(context) else {
        return;
    };
    // SAFETY: the caller lends `count` readable elements for this call only.
    let values = unsafe {
        std::slice::from_raw_parts(values.cast_const().cast::<A::Elem>(), count as usize)
    };
    guard_write(&slot.value, || {
        if let Some(write) = A::write(&slot.value.access) {
            write(values, offset);
        }
    });
}

type MenuFn = Box<dyn Fn(usize)>;

/// An owned menu whose item selections call a Rust closure with the item's
/// refcon value.
#[must_use = "dropping a menu immediately destroys it"]
pub struct CallbackMenu {
    handle: MenuHandle,
    // Field order: the menu is destroyed, then recorded as destroyed, then
    // its callback goes.
    _menu: SdkOwned,
    _destroyed: DestroyedNotice,
    _slot: Registered<MenuFn>,
}

/// Records its menu as destroyed (`xplm::menus::menu_destroyed`) when
/// dropped, which is right after the preceding `SdkOwned` field destroys it.
struct DestroyedNotice(MenuHandle);

impl Drop for DestroyedNotice {
    fn drop(&mut self) {
        crate::xplm::menus::menu_destroyed(self.0);
    }
}

impl CallbackMenu {
    /// Create a submenu under `parent` item `item`, or in the menu bar when
    /// `parent` is `None`. Returns `None` if X-Plane refuses, or, without a
    /// call, if one of the crate's menu owners has already destroyed `parent`.
    ///
    /// The menu is destroyed exactly once, on drop, even when its parent or
    /// parent item went first: see "Removal and destroy order" on
    /// [`crate::MenuItem`] for the observed behaviour this relies on.
    pub fn create(
        name: &CStr,
        parent: Option<MenuHandle>,
        item: c_int,
        label: &'static str,
        callback: impl Fn(usize) + 'static,
    ) -> Option<Self> {
        if crate::xplm::menus::refuses_parent(parent) {
            return None;
        }
        let slot = Registered::new(label, Box::new(callback) as MenuFn);
        // SAFETY: the name is NUL-terminated for the call; the parent is null
        // or a live SDK menu (a `MenuHandle` the crate's owners have not
        // destroyed: checked). The handler is a safe function. `SdkOwned`
        // destroys the menu once, before the refcon leaves the registry.
        let menu = unsafe {
            let handle = sdk::XPLMCreateMenu(
                name.as_ptr(),
                parent.map_or(ptr::null_mut(), MenuHandle::as_raw),
                item,
                Some(menu_trampoline),
                slot.refcon(),
            );
            SdkOwned::new(handle, sdk::XPLMDestroyMenu)
        }?;
        // SAFETY: the menu was just created and lives as long as `self`.
        let handle = unsafe { MenuHandle::from_raw(menu.as_ptr()) }?;
        crate::xplm::menus::menu_created(handle);
        Some(Self {
            handle,
            _menu: menu,
            _destroyed: DestroyedNotice(handle),
            _slot: slot,
        })
    }

    /// The SDK handle, for item calls. It is valid while `self` lives.
    pub fn handle(&self) -> MenuHandle {
        self.handle
    }
}

extern "C" fn menu_trampoline(context: *mut c_void, item: *mut c_void) {
    enter(context, (), |slot: &Slot<MenuFn>| (slot.value)(item.addr()));
}

type DrawFn = Box<dyn Fn(sdk::XPLMDrawingPhase, bool) -> c_int>;

/// An owned draw callback registration.
#[must_use = "dropping a draw handler immediately unregisters it"]
pub struct DrawHandler {
    phase: sdk::XPLMDrawingPhase,
    before: c_int,
    slot: Registered<DrawFn>,
}

impl DrawHandler {
    /// Register `callback` for `phase`. For a before-phase callback, return
    /// 1 to let X-Plane draw or 0 to suppress its drawing. After a plugin
    /// fault the handler returns 1. Returns `None` if the phase is unknown.
    pub fn register(
        phase: sdk::XPLMDrawingPhase,
        before: bool,
        label: &'static str,
        callback: impl Fn(sdk::XPLMDrawingPhase, bool) -> c_int + 'static,
    ) -> Option<Self> {
        let slot = Registered::new(label, Box::new(callback) as DrawFn);
        let before = c_int::from(before);
        // SAFETY: the refcon is unique (one registry entry per slot), the
        // callback is a safe function, and Drop unregisters this tuple.
        let ok = unsafe {
            sdk::XPLMRegisterDrawCallback(Some(draw_trampoline), phase, before, slot.refcon())
        };
        (ok != 0).then_some(Self {
            phase,
            before,
            slot,
        })
    }
}

impl Drop for DrawHandler {
    fn drop(&mut self) {
        // SAFETY: this is the tuple registered in `register`.
        unsafe {
            sdk::XPLMUnregisterDrawCallback(
                Some(draw_trampoline),
                self.phase,
                self.before,
                self.slot.refcon(),
            );
        }
    }
}

extern "C" fn draw_trampoline(
    phase: sdk::XPLMDrawingPhase,
    before: c_int,
    context: *mut c_void,
) -> c_int {
    enter(context, 1, |slot: &Slot<DrawFn>| {
        (slot.value)(phase, before != 0)
    })
}

/// Callbacks for a modern XPLM window. Defaults pass events through.
pub trait WindowHandler {
    fn draw(&self, _window: sdk::XPLMWindowID) {}
    fn click(&self, _window: sdk::XPLMWindowID, _x: c_int, _y: c_int, _status: c_int) -> c_int {
        0
    }
    fn right_click(
        &self,
        _window: sdk::XPLMWindowID,
        _x: c_int,
        _y: c_int,
        _status: c_int,
    ) -> c_int {
        0
    }
    fn key(
        &self,
        _window: sdk::XPLMWindowID,
        _key: c_char,
        _flags: sdk::XPLMKeyFlags,
        _virtual_key: c_char,
        _losing_focus: bool,
    ) {
    }
    fn cursor(&self, _window: sdk::XPLMWindowID, _x: c_int, _y: c_int) -> sdk::XPLMCursorStatus {
        sdk::xplm_CursorDefault as sdk::XPLMCursorStatus
    }
    fn wheel(
        &self,
        _window: sdk::XPLMWindowID,
        _x: c_int,
        _y: c_int,
        _wheel: c_int,
        _clicks: c_int,
    ) -> c_int {
        0
    }
}

/// Geometry and style for [`CallbackWindow::create`].
#[derive(Clone, Copy, Debug)]
pub struct WindowParams {
    /// Left, top, right and bottom in global desktop boxels.
    pub bounds: [c_int; 4],
    pub visible: bool,
    pub decoration: sdk::XPLMWindowDecoration,
    pub layer: sdk::XPLMWindowLayer,
}

type WindowSlot = Slot<Box<dyn WindowHandler>>;

/// An owned modern XPLM window whose callbacks reach a Rust handler.
#[must_use = "dropping a window immediately destroys it"]
pub struct CallbackWindow {
    // Field order: the window is destroyed before its handler goes.
    handle: SdkOwned,
    _slot: Registered<Box<dyn WindowHandler>>,
}

impl CallbackWindow {
    /// Create the window. Returns `None` if X-Plane refuses.
    pub fn create(
        params: WindowParams,
        label: &'static str,
        handler: impl WindowHandler + 'static,
    ) -> Option<Self> {
        let slot = Registered::new(label, Box::new(handler) as Box<dyn WindowHandler>);
        let [left, top, right, bottom] = params.bounds;
        let mut create = sdk::XPLMCreateWindow_t {
            structSize: mem::size_of::<sdk::XPLMCreateWindow_t>() as c_int,
            left,
            top,
            right,
            bottom,
            visible: c_int::from(params.visible),
            drawWindowFunc: Some(window_draw),
            handleMouseClickFunc: Some(window_click),
            handleKeyFunc: Some(window_key),
            handleCursorFunc: Some(window_cursor),
            handleMouseWheelFunc: Some(window_wheel),
            refcon: slot.refcon(),
            decorateAsFloatingWindow: params.decoration,
            layer: params.layer,
            handleRightClickFunc: Some(window_right_click),
        };
        // SAFETY: the structure has the SDK-prescribed size and every
        // callback is a safe function with the SDK ABI. `SdkOwned` destroys
        // the window once, before the refcon leaves the registry.
        let handle =
            unsafe { SdkOwned::new(sdk::XPLMCreateWindowEx(&mut create), sdk::XPLMDestroyWindow) }?;
        Some(Self {
            handle,
            _slot: slot,
        })
    }

    /// The SDK handle, for window calls. It is valid while `self` lives.
    pub fn handle(&self) -> sdk::XPLMWindowID {
        self.handle.as_ptr()
    }
}

/// Runs one window callback through [`enter`].
fn window_call<T>(
    context: *mut c_void,
    default: T,
    call: impl FnOnce(&dyn WindowHandler) -> T,
) -> T {
    enter(context, default, |slot: &WindowSlot| {
        call(slot.value.as_ref())
    })
}

extern "C" fn window_draw(window: sdk::XPLMWindowID, context: *mut c_void) {
    window_call(context, (), |h| h.draw(window));
}

extern "C" fn window_click(
    window: sdk::XPLMWindowID,
    x: c_int,
    y: c_int,
    status: sdk::XPLMMouseStatus,
    context: *mut c_void,
) -> c_int {
    window_call(context, 0, |h| h.click(window, x, y, status))
}

extern "C" fn window_right_click(
    window: sdk::XPLMWindowID,
    x: c_int,
    y: c_int,
    status: sdk::XPLMMouseStatus,
    context: *mut c_void,
) -> c_int {
    window_call(context, 0, |h| h.right_click(window, x, y, status))
}

extern "C" fn window_key(
    window: sdk::XPLMWindowID,
    key: c_char,
    flags: sdk::XPLMKeyFlags,
    virtual_key: c_char,
    context: *mut c_void,
    losing_focus: c_int,
) {
    if crate::guard::faulted() {
        release_focus_after_fault(losing_focus != 0, || crate::xplm::take_keyboard_focus(None));
        return;
    }
    window_call(context, (), |h| {
        h.key(window, key, flags, virtual_key, losing_focus != 0)
    });
}

/// After a fault no handler runs, so a window that still holds keyboard
/// focus would swallow every key the pilot presses. X-Plane only sends a key
/// to the window that has focus, so a key arriving here (and not the
/// focus-loss notice) means this plugin holds it: `release` hands focus back
/// to X-Plane. Only that one key is lost.
fn release_focus_after_fault(losing_focus: bool, release: impl FnOnce()) {
    if !losing_focus {
        release();
    }
}

extern "C" fn window_cursor(
    window: sdk::XPLMWindowID,
    x: c_int,
    y: c_int,
    context: *mut c_void,
) -> sdk::XPLMCursorStatus {
    let default = sdk::xplm_CursorDefault as sdk::XPLMCursorStatus;
    window_call(context, default, |h| h.cursor(window, x, y))
}

extern "C" fn window_wheel(
    window: sdk::XPLMWindowID,
    x: c_int,
    y: c_int,
    wheel: c_int,
    clicks: c_int,
    context: *mut c_void,
) -> c_int {
    window_call(context, 0, |h| h.wheel(window, x, y, wheel, clicks))
}

type AvionicsFn = Box<dyn Fn(sdk::XPLMDeviceID, bool) -> c_int>;

/// Owned draw callbacks for a built-in avionics device.
#[must_use = "dropping the callbacks immediately unregisters them"]
pub struct AvionicsDraw {
    // Field order: the callbacks are unregistered before the closure goes.
    handle: SdkOwned,
    _slot: Registered<AvionicsFn>,
}

impl AvionicsDraw {
    /// Register before- and after-draw callbacks for `device`. The callback
    /// receives whether it runs before X-Plane draws; a before callback
    /// returns 1 to let X-Plane draw the device or 0 to suppress it. After a
    /// plugin fault both return 1. Returns `None` if X-Plane refuses.
    pub fn register(
        device: sdk::XPLMDeviceID,
        label: &'static str,
        callback: impl Fn(sdk::XPLMDeviceID, bool) -> c_int + 'static,
    ) -> Option<Self> {
        let slot = Registered::new(label, Box::new(callback) as AvionicsFn);
        let mut params = sdk::XPLMCustomizeAvionics_t {
            structSize: mem::size_of::<sdk::XPLMCustomizeAvionics_t>() as c_int,
            deviceId: device,
            drawCallbackBefore: Some(avionics_trampoline),
            drawCallbackAfter: Some(avionics_trampoline),
            bezelClickCallback: None,
            bezelRightClickCallback: None,
            bezelScrollCallback: None,
            bezelCursorCallback: None,
            screenTouchCallback: None,
            screenRightTouchCallback: None,
            screenScrollCallback: None,
            screenCursorCallback: None,
            keyboardCallback: None,
            refcon: slot.refcon(),
        };
        // SAFETY: the structure has the SDK-prescribed size and the callbacks
        // are safe functions. `SdkOwned` unregisters once, before the refcon
        // leaves the registry.
        let handle = unsafe {
            SdkOwned::new(
                sdk::XPLMRegisterAvionicsCallbacksEx(&mut params),
                sdk::XPLMUnregisterAvionicsCallbacks,
            )
        }?;
        Some(Self {
            handle,
            _slot: slot,
        })
    }

    pub fn handle(&self) -> sdk::XPLMAvionicsID {
        self.handle.as_ptr()
    }

    /// `XPLMSetAvionicsPopupVisible(handle, visible)`: show (non-zero) or
    /// hide (0) the device's 2D pop-up window. `visible` is passed unchanged.
    pub fn set_popup_visible(&self, visible: c_int) {
        // SAFETY: `self` holds the registration until Drop, so the handle is
        // a live `XPLMAvionicsID`; `visible` is a plain number.
        unsafe { sdk::XPLMSetAvionicsPopupVisible(self.handle(), visible) }
    }

    /// `XPLMGetAvionicsBusVoltsRatio(handle)`: the device's bus-volts ratio.
    pub fn bus_volts_ratio(&self) -> f32 {
        // SAFETY: `self` holds the registration until Drop, so the handle is
        // a live `XPLMAvionicsID`.
        unsafe { sdk::XPLMGetAvionicsBusVoltsRatio(self.handle()) }
    }
}

extern "C" fn avionics_trampoline(
    device: sdk::XPLMDeviceID,
    before: c_int,
    context: *mut c_void,
) -> c_int {
    enter(context, 1, |slot: &Slot<AvionicsFn>| {
        (slot.value)(device, before != 0)
    })
}

struct AvionicsCallbacks {
    screen: Box<dyn Fn()>,
    brightness: Box<dyn Fn(f32, f32, f32) -> f32>,
}

/// Size and identity for [`CustomAvionics::create`].
#[derive(Clone, Copy, Debug)]
pub struct AvionicsParams<'a> {
    /// Screen width and height in pixels. The 2D pop-up bezel has the same
    /// size, with the screen at its origin.
    pub size: [c_int; 2],
    /// Draw only after [`CustomAvionics::needs_drawing`], not every frame.
    pub on_demand: bool,
    pub id: &'a CStr,
    pub name: &'a CStr,
}

/// An owned custom avionics device whose screen callbacks reach Rust closures.
#[must_use = "dropping a device immediately destroys it"]
pub struct CustomAvionics {
    // Field order: the device is destroyed before its callbacks go.
    handle: SdkOwned,
    _slot: Registered<AvionicsCallbacks>,
}

impl CustomAvionics {
    /// Create the device. `screen` draws into the device's framebuffer.
    /// `brightness` receives the rheostat value, the ambient brightness and
    /// the bus-volts ratio and returns the screen brightness. After a plugin
    /// fault the screen is not drawn and the brightness is the rheostat
    /// value. Returns `None` if X-Plane refuses.
    pub fn create(
        params: AvionicsParams<'_>,
        label: &'static str,
        screen: impl Fn() + 'static,
        brightness: impl Fn(f32, f32, f32) -> f32 + 'static,
    ) -> Option<Self> {
        let slot = Registered::new(
            label,
            AvionicsCallbacks {
                screen: Box::new(screen),
                brightness: Box::new(brightness),
            },
        );
        let [width, height] = params.size;
        let mut create = sdk::XPLMCreateAvionics_t {
            structSize: mem::size_of::<sdk::XPLMCreateAvionics_t>() as c_int,
            screenWidth: width,
            screenHeight: height,
            bezelWidth: width,
            bezelHeight: height,
            screenOffsetX: 0,
            screenOffsetY: 0,
            drawOnDemand: c_int::from(params.on_demand),
            bezelDrawCallback: None,
            drawCallback: Some(avionics_screen),
            bezelClickCallback: None,
            bezelRightClickCallback: None,
            bezelScrollCallback: None,
            bezelCursorCallback: None,
            screenTouchCallback: None,
            screenRightTouchCallback: None,
            screenScrollCallback: None,
            screenCursorCallback: None,
            keyboardCallback: None,
            brightnessCallback: Some(avionics_brightness),
            deviceID: params.id.as_ptr().cast_mut(),
            deviceName: params.name.as_ptr().cast_mut(),
            refcon: slot.refcon(),
        };
        // SAFETY: the structure has the SDK-prescribed size, the strings are
        // NUL-terminated for the call and the callbacks are safe functions.
        // `SdkOwned` destroys the device once, before the refcon leaves the
        // registry.
        let handle = unsafe {
            SdkOwned::new(
                sdk::XPLMCreateAvionicsEx(&mut create),
                sdk::XPLMDestroyAvionics,
            )
        }?;
        Some(Self {
            handle,
            _slot: slot,
        })
    }

    pub fn handle(&self) -> sdk::XPLMAvionicsID {
        self.handle.as_ptr()
    }

    /// Whether the device is bound to a screen of the current aircraft.
    pub fn bound(&self) -> bool {
        // SAFETY: the device is live while `self` is.
        unsafe { sdk::XPLMIsAvionicsBound(self.handle()) != 0 }
    }

    /// Whether the device's 2D pop-up window is visible.
    pub fn popup_visible(&self) -> bool {
        // SAFETY: the device is live while `self` is.
        unsafe { sdk::XPLMIsAvionicsPopupVisible(self.handle()) != 0 }
    }

    /// `XPLMSetAvionicsPopupVisible(handle, visible)`: show (non-zero) or
    /// hide (0) the device's 2D pop-up window. `visible` is passed unchanged.
    pub fn set_popup_visible(&self, visible: c_int) {
        // SAFETY: the device is live while `self` is; `visible` is a plain number.
        unsafe { sdk::XPLMSetAvionicsPopupVisible(self.handle(), visible) }
    }

    /// `XPLMGetAvionicsBusVoltsRatio(handle)`: the device's bus-volts ratio.
    pub fn bus_volts_ratio(&self) -> f32 {
        // SAFETY: the device is live while `self` is.
        unsafe { sdk::XPLMGetAvionicsBusVoltsRatio(self.handle()) }
    }

    /// Ask X-Plane to draw an on-demand device before the next frame.
    pub fn needs_drawing(&self) {
        // SAFETY: the device is live while `self` is.
        unsafe { sdk::XPLMAvionicsNeedsDrawing(self.handle()) }
    }
}

extern "C" fn avionics_screen(context: *mut c_void) {
    enter(context, (), |slot: &Slot<AvionicsCallbacks>| {
        (slot.value.screen)()
    });
}

extern "C" fn avionics_brightness(
    rheostat: f32,
    ambient: f32,
    bus_ratio: f32,
    context: *mut c_void,
) -> f32 {
    enter(context, rheostat, |slot: &Slot<AvionicsCallbacks>| {
        (slot.value.brightness)(rheostat, ambient, bus_ratio)
    })
}

type LoopFn = Box<dyn Fn(FlightLoopFrame) -> f32>;

/// An owned flight loop whose callback is a Rust closure. It is created
/// dormant; [`CallbackFlightLoop::schedule`] starts it.
#[must_use = "dropping a flight loop immediately destroys it"]
pub struct CallbackFlightLoop {
    // Field order: the loop is destroyed before its callback goes.
    registration: PhaseFlightLoop,
    _slot: Registered<LoopFn>,
}

impl CallbackFlightLoop {
    /// Create a loop for `phase`. The callback returns the next interval in
    /// the SDK convention: negative counts frames, positive counts seconds
    /// and zero stops the loop. After a plugin fault the loop returns 0.
    /// Returns `None` if X-Plane refuses.
    pub fn create(
        phase: sdk::XPLMFlightLoopPhaseType,
        label: &'static str,
        callback: impl Fn(FlightLoopFrame) -> f32 + 'static,
    ) -> Option<Self> {
        let slot = Registered::new(label, Box::new(callback) as LoopFn);
        let registration =
            PhaseFlightLoop::unscheduled(phase, loop_trampoline, slot.refcon()).ok()?;
        Some(Self {
            registration,
            _slot: slot,
        })
    }

    pub fn handle(&self) -> sdk::XPLMFlightLoopID {
        self.registration.handle()
    }

    /// Schedule the loop's next call with the SDK interval convention.
    pub fn schedule(&self, interval: f32, relative_to_now: bool) {
        self.registration.schedule(interval, relative_to_now);
    }
}

extern "C" fn loop_trampoline(
    elapsed_since_last_call: f32,
    elapsed_since_last_loop: f32,
    counter: c_int,
    context: *mut c_void,
) -> f32 {
    enter(context, 0., |slot: &Slot<LoopFn>| {
        (slot.value)(FlightLoopFrame {
            elapsed_since_last_call,
            elapsed_since_last_loop,
            counter,
        })
    })
}

struct CameraCallback {
    position: Box<dyn Fn(bool) -> Option<sdk::XPLMCameraPosition_t>>,
    lost: Cell<bool>,
}

/// Control of X-Plane's camera, held until this value is dropped or X-Plane
/// takes the camera back.
#[must_use = "dropping camera control immediately releases it"]
pub struct CameraControl {
    slot: Registered<CameraCallback>,
}

impl CameraControl {
    /// Take the camera for `duration` (an `xplm_ControlCamera*` value). Each
    /// frame `position` receives `false` and returns the camera position, or
    /// `None` to leave the camera as X-Plane has it. When X-Plane takes the
    /// camera back it calls `position` once with `true`, and ignores the
    /// result. After a plugin fault the camera is left as X-Plane has it.
    pub fn take(
        duration: sdk::XPLMCameraControlDuration,
        label: &'static str,
        position: impl Fn(bool) -> Option<sdk::XPLMCameraPosition_t> + 'static,
    ) -> Self {
        let slot = Registered::new(
            label,
            CameraCallback {
                position: Box::new(position),
                lost: Cell::new(false),
            },
        );
        // SAFETY: the callback's pointer contract is the SDK's (see
        // `camera_trampoline`). Drop releases the camera before the refcon
        // leaves the registry, and X-Plane stops calling after it has taken
        // the camera back.
        unsafe { sdk::XPLMControlCamera(duration, Some(camera_trampoline), slot.refcon()) };
        Self { slot }
    }

    /// Whether X-Plane has taken the camera back.
    pub fn lost(&self) -> bool {
        self.slot.value.lost.get()
    }
}

impl Drop for CameraControl {
    fn drop(&mut self) {
        if !self.lost() {
            // X-Plane has not called the trampoline with the losing flag. That does not mean
            // this plugin still holds the camera: `XPLMCamera.h` says a control function that
            // returns 0 surrenders control (the trampoline returns 0 for `None`, an unknown
            // refcon, a null position, a panic and after a fault), and the header does not say
            // whether that surrender is permanent or only for that draw loop. The header asks
            // callers not to use `XPLMDontControlCamera` without holding the camera, "for
            // maximum compatibility", and documents no undefined behaviour for it; the call is
            // kept as it was on every path.
            // SAFETY: the call takes no arguments and touches no memory of this plugin.
            unsafe { sdk::XPLMDontControlCamera() };
        }
    }
}

/// The SDK's camera control callback.
///
/// # Safety
/// `out` is null or a position writable for this call, as X-Plane promises
/// when it calls a camera controller.
unsafe extern "C" fn camera_trampoline(
    out: *mut sdk::XPLMCameraPosition_t,
    losing: c_int,
    context: *mut c_void,
) -> c_int {
    let Some(slot) = lease::<Slot<CameraCallback>>(context) else {
        return 0;
    };
    let losing = losing != 0;
    // Record the loss even after a fault, so Drop does not release a camera
    // this plugin no longer holds.
    if losing {
        slot.value.lost.set(true);
    }
    guard(slot.label, 0, || match (slot.value.position)(losing) {
        Some(position) if !losing && !out.is_null() => {
            // SAFETY: the caller passes a non-null position writable for the call.
            unsafe { out.write(position) };
            1
        }
        _ => 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};

    #[test]
    fn copy_array_reports_length_and_clips() {
        let values = [1, 2, 3, 4];
        assert_eq!(copy_array(&values, None, 0), 4);
        let mut out = [0; 3];
        assert_eq!(copy_array(&values, Some(&mut out), 2), 2);
        assert_eq!(out, [3, 4, 0]);
        assert_eq!(copy_array(&values, Some(&mut out), 4), 0);
        assert_eq!(copy_array(&values, Some(&mut out), -1), 0);
        assert_eq!(copy_array(&values, Some(&mut []), 0), 0);
    }

    #[test]
    fn a_callback_can_release_its_own_slot() {
        let owner = Rc::new(RefCell::new(None::<Registered<DrawFn>>));
        let weak = Rc::downgrade(&owner);
        let calls = Rc::new(Cell::new(0));
        let seen = calls.clone();
        let slot = Registered::new(
            "test draw",
            Box::new(move |phase, before| {
                // Dropping the owning reference mid-call must not free the
                // closure that is running.
                weak.upgrade().unwrap().borrow_mut().take();
                seen.set(seen.get() + 1);
                phase + c_int::from(before)
            }) as DrawFn,
        );
        let context = slot.refcon();
        *owner.borrow_mut() = Some(slot);
        assert_eq!(draw_trampoline(40, 1, context), 41);
        assert_eq!(calls.get(), 1);
        assert!(owner.borrow().is_none());
        // The released refcon is no longer live: the fallback, no call.
        assert_eq!(draw_trampoline(40, 1, context), 1);
        assert_eq!(calls.get(), 1);
    }

    #[test]
    fn a_menu_callback_survives_destroying_its_own_registration() {
        let owner = Rc::new(RefCell::new(None::<Registered<MenuFn>>));
        let captured = Rc::downgrade(&owner);
        let seen = Rc::new(Cell::new(None));
        let observed = seen.clone();
        let slot = Registered::new(
            "test menu",
            Box::new(move |item: usize| {
                drop(captured.upgrade().unwrap().borrow_mut().take().unwrap());
                observed.set(Some(item));
            }) as MenuFn,
        );
        let context = slot.refcon();
        *owner.borrow_mut() = Some(slot);
        menu_trampoline(context, ptr::without_provenance_mut(42));
        assert!(owner.borrow().is_none());
        assert_eq!(seen.get(), Some(42));
    }

    #[test]
    fn a_menu_under_a_destroyed_parent_is_refused() {
        // The refusal `CallbackMenu::create` (and `PlainMenu`) make before `XPLMCreateMenu`.
        // `create` itself is not called: a test that references it links the XPLM import, and
        // the test binary then fails to load without X-Plane's library.
        use crate::xplm::menus::{menu_created, menu_destroyed, refuses_parent};
        let parent = MenuHandle::from_raw_for_test(0x7000);
        assert!(!refuses_parent(None));
        assert!(!refuses_parent(Some(parent)));
        menu_destroyed(parent);
        assert!(refuses_parent(Some(parent)));
        assert!(!refuses_parent(None));
        // X-Plane handing the address out again for a new menu lifts the refusal.
        menu_created(parent);
        assert!(!refuses_parent(Some(parent)));
    }

    #[test]
    fn unknown_or_mistyped_refcons_get_the_fallback() {
        let calls = Rc::new(Cell::new(0));
        let seen = calls.clone();
        let menu = Registered::new(
            "test menu",
            Box::new(move |_| seen.set(seen.get() + 1)) as MenuFn,
        );
        // A live refcon of another type, an unknown address and null.
        assert_eq!(draw_trampoline(0, 1, menu.refcon()), 1);
        assert_eq!(get_int(menu.refcon()), 0);
        assert_eq!(
            loop_trampoline(0., 0., 0, ptr::without_provenance_mut(64)),
            0.
        );
        menu_trampoline(ptr::null_mut(), ptr::null_mut());
        assert_eq!(calls.get(), 0);
        menu_trampoline(menu.refcon(), ptr::null_mut());
        assert_eq!(calls.get(), 1);
    }

    #[test]
    fn command_trampoline_passes_only_the_registered_command() {
        let command = CommandHandle::from_raw_for_test(0x40);
        let slot = Registered::new(
            "test command",
            CommandSlot {
                command,
                unknown_command: 7,
                callback: Box::new(|command, phase| command.as_raw().addr() as c_int + phase),
            },
        );
        assert_eq!(command_trampoline(command.as_raw(), 2, slot.refcon()), 0x42);
        assert_eq!(command_trampoline(ptr::null_mut(), 2, slot.refcon()), 7);
        assert_eq!(
            command_trampoline(ptr::without_provenance_mut(0x80), 2, slot.refcon()),
            7
        );
    }

    #[test]
    fn data_trampolines_route_to_present_callbacks() {
        let written = Rc::new(Cell::new(0.));
        let sink = written.clone();
        let received = Rc::new(RefCell::new((Vec::new(), 0)));
        let ints = received.clone();
        let slot = Registered::new(
            "test data",
            DataSlot {
                access: DataAccess {
                    float: Some(Box::new(|| 2.5)),
                    set_float: Some(Box::new(move |v| sink.set(v))),
                    floats: Some(Box::new(|out, offset| {
                        copy_array(&[1., 2., 3.], out, offset)
                    })),
                    set_ints: Some(Box::new(move |values, offset| {
                        ints.replace((values.to_vec(), offset));
                    })),
                    ..DataAccess::default()
                },
                write_label: "test data write",
                writes: Writes::Guarded,
            },
        );
        let context = slot.refcon();
        let mut out = [0.; 2];
        let mut input = [7, 8];
        assert_eq!(get_float(context), 2.5);
        assert_eq!(get_int(context), 0);
        set_float(context, 4.);
        // SAFETY: the buffers are null or match their counts.
        unsafe {
            assert_eq!(get_array::<Floats>(context, ptr::null_mut(), 0, 0), 3);
            assert_eq!(get_array::<Floats>(context, out.as_mut_ptr(), 1, 2), 2);
            set_array::<Ints>(context, input.as_mut_ptr(), 1, 2);
            assert_eq!(get_array::<Bytes>(context, ptr::null_mut(), 0, 0), 0);
        }
        assert_eq!(written.get(), 4.);
        assert_eq!(out, [2., 3.]);
        assert_eq!(*received.borrow(), (vec![7, 8], 1));
    }

    #[test]
    fn window_handler_defaults_pass_events_through() {
        struct Quiet;
        impl WindowHandler for Quiet {}
        let slot = Registered::new("test window", Box::new(Quiet) as Box<dyn WindowHandler>);
        let context = slot.refcon();
        let window = ptr::null_mut();
        window_draw(window, context);
        assert_eq!(window_click(window, 1, 2, 1, context), 0);
        assert_eq!(window_wheel(window, 1, 2, 0, 1, context), 0);
        assert_eq!(
            window_cursor(window, 1, 2, context),
            sdk::xplm_CursorDefault as sdk::XPLMCursorStatus
        );
    }

    #[test]
    fn a_faulted_window_gives_focus_back_on_a_key_but_not_on_focus_loss() {
        let released = Cell::new(0);
        release_focus_after_fault(false, || released.set(released.get() + 1));
        assert_eq!(released.get(), 1);
        // The focus-loss notice X-Plane sends when focus moves on must not
        // take focus again.
        release_focus_after_fault(true, || released.set(released.get() + 1));
        assert_eq!(released.get(), 1);
    }

    #[test]
    fn camera_trampoline_writes_positions_and_records_the_loss() {
        let position = sdk::XPLMCameraPosition_t {
            x: 1.,
            y: 2.,
            z: 3.,
            pitch: 4.,
            heading: 5.,
            roll: 6.,
            zoom: 1.,
        };
        let calls = Rc::new(RefCell::new(Vec::new()));
        let seen = calls.clone();
        let slot = Registered::new(
            "test camera",
            CameraCallback {
                position: Box::new(move |losing| {
                    seen.borrow_mut().push(losing);
                    Some(position)
                }),
                lost: Cell::new(false),
            },
        );
        let context = slot.refcon();
        let mut out = sdk::XPLMCameraPosition_t {
            zoom: 0.,
            ..position
        };
        // SAFETY: `context` is a live camera slot and `out` a writable position.
        unsafe {
            assert_eq!(camera_trampoline(&mut out, 0, context), 1);
            assert_eq!(out.zoom, 1.);
            assert!(!slot.value.lost.get());
            out.x = 0.;
            assert_eq!(camera_trampoline(&mut out, 1, context), 0);
            assert_eq!(out.x, 0.);
            assert_eq!(camera_trampoline(ptr::null_mut(), 0, context), 0);
        }
        assert!(slot.value.lost.get());
        assert_eq!(*calls.borrow(), [false, true, false]);
    }

    #[test]
    fn loop_and_brightness_trampolines_pass_their_arguments() {
        let looped = Registered::new(
            "test loop",
            Box::new(|frame: FlightLoopFrame| frame.elapsed_since_last_call + frame.counter as f32)
                as LoopFn,
        );
        let device = Registered::new(
            "test device",
            AvionicsCallbacks {
                screen: Box::new(|| {}),
                brightness: Box::new(|rheostat, ambient, bus| rheostat * ambient + bus),
            },
        );
        assert_eq!(loop_trampoline(0.5, 0., 2, looped.refcon()), 2.5);
        assert_eq!(avionics_brightness(0.5, 0.5, 1., device.refcon()), 1.25);
        avionics_screen(device.refcon());
    }
}
