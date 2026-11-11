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

use std::{
    ffi::{CStr, c_char, c_int, c_void},
    mem,
    ptr::{self, NonNull},
    rc::Rc,
};

use xplane_sdk_sys as sdk;

use crate::{
    guard::{guard, guard_read},
    registrations::{CommandHandle, MenuHandle},
};

/// A callback allocation whose address is an SDK refcon.
struct Slot<T> {
    label: &'static str,
    value: T,
}

impl<T> Slot<T> {
    fn new(label: &'static str, value: T) -> Rc<Self> {
        Rc::new(Self { label, value })
    }
}

fn refcon<T>(slot: &Rc<Slot<T>>) -> *mut c_void {
    Rc::as_ptr(slot).cast_mut().cast()
}

/// Retain the callback behind `refcon` for one SDK call.
///
/// # Safety
/// `refcon` must be the address of an `Rc<Slot<T>>` that a live registration
/// in this module still owns.
unsafe fn lease<T>(refcon: *mut c_void) -> Rc<Slot<T>> {
    let pointer = refcon.cast_const().cast::<Slot<T>>();
    // SAFETY: the caller guarantees a live `Rc` allocation of this type. The
    // extra strong count keeps it alive if the callback drops its owner.
    unsafe {
        Rc::increment_strong_count(pointer);
        Rc::from_raw(pointer)
    }
}

type CommandFn = Box<dyn Fn(CommandHandle, sdk::XPLMCommandPhase) -> c_int>;

/// An owned command handler registration.
#[must_use = "dropping a handler immediately unregisters it"]
pub struct CommandHandler {
    command: CommandHandle,
    before: c_int,
    slot: Rc<Slot<CommandFn>>,
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
        let slot = Slot::new(label, Box::new(callback) as CommandFn);
        let before = c_int::from(before);
        // SAFETY: `command` is a live SDK command. The slot outlives the
        // registration: Drop unregisters this tuple before releasing it.
        unsafe {
            sdk::XPLMRegisterCommandHandler(
                command.as_raw(),
                Some(command_trampoline),
                before,
                refcon(&slot),
            );
        }
        Self {
            command,
            before,
            slot,
        }
    }

    pub fn command(&self) -> CommandHandle {
        self.command
    }
}

impl Drop for CommandHandler {
    fn drop(&mut self) {
        // SAFETY: this is the tuple registered in `register`; the slot is
        // still owned here.
        unsafe {
            sdk::XPLMUnregisterCommandHandler(
                self.command.as_raw(),
                Some(command_trampoline),
                self.before,
                refcon(&self.slot),
            );
        }
    }
}

unsafe extern "C" fn command_trampoline(
    command: sdk::XPLMCommandRef,
    phase: sdk::XPLMCommandPhase,
    context: *mut c_void,
) -> c_int {
    // SAFETY: only `CommandHandler` registers this trampoline, with its slot.
    let slot = unsafe { lease::<CommandFn>(context) };
    guard(slot.label, 1, || {
        // SAFETY: X-Plane passes the live command this handler is bound to.
        match unsafe { CommandHandle::from_raw(command) } {
            Some(command) => (slot.value)(command, phase),
            None => 1,
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

/// An owned dataref accessor registration.
#[must_use = "dropping an accessor immediately unregisters it"]
pub struct DataAccessor {
    handle: NonNull<c_void>,
    _slot: Rc<Slot<DataAccess>>,
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
        let w = |present: bool| writable && present;
        let slot = Slot::new(label, access);
        let a = &slot.value;
        // SAFETY: the name is NUL-terminated for the call. Each trampoline is
        // registered only when its callback is present, and both refcons are
        // the slot, which Drop releases after unregistering.
        let handle = unsafe {
            sdk::XPLMRegisterDataAccessor(
                name.as_ptr(),
                kind,
                c_int::from(writable),
                a.int.is_some().then_some(get_int as _),
                w(a.set_int.is_some()).then_some(set_int as _),
                a.float.is_some().then_some(get_float as _),
                w(a.set_float.is_some()).then_some(set_float as _),
                a.double.is_some().then_some(get_double as _),
                w(a.set_double.is_some()).then_some(set_double as _),
                a.ints.is_some().then_some(get_ints as _),
                w(a.set_ints.is_some()).then_some(set_ints as _),
                a.floats.is_some().then_some(get_floats as _),
                w(a.set_floats.is_some()).then_some(set_floats as _),
                a.bytes.is_some().then_some(get_bytes as _),
                w(a.set_bytes.is_some()).then_some(set_bytes as _),
                refcon(&slot),
                if writable {
                    refcon(&slot)
                } else {
                    ptr::null_mut()
                },
            )
        };
        Some(Self {
            handle: NonNull::new(handle)?,
            _slot: slot,
        })
    }

    pub fn handle(&self) -> sdk::XPLMDataRef {
        self.handle.as_ptr()
    }
}

impl Drop for DataAccessor {
    fn drop(&mut self) {
        // SAFETY: this accessor registered the handle and still owns it.
        unsafe { sdk::XPLMUnregisterDataAccessor(self.handle.as_ptr()) };
    }
}

/// # Safety
/// `context` is the refcon of a live `DataAccessor`.
unsafe fn scalar<T: Default>(
    context: *mut c_void,
    pick: impl FnOnce(&DataAccess) -> Option<&dyn Fn() -> T>,
) -> T {
    // SAFETY: guaranteed by the caller.
    let slot = unsafe { lease::<DataAccess>(context) };
    guard_read(slot.label, T::default(), || {
        pick(&slot.value).map_or_else(T::default, |read| read())
    })
}

/// # Safety
/// `context` is the refcon of a live `DataAccessor`.
unsafe fn store<T>(
    context: *mut c_void,
    value: T,
    pick: impl FnOnce(&DataAccess) -> Option<&dyn Fn(T)>,
) {
    // SAFETY: guaranteed by the caller.
    let slot = unsafe { lease::<DataAccess>(context) };
    guard(slot.label, (), || {
        if let Some(write) = pick(&slot.value) {
            write(value);
        }
    });
}

unsafe extern "C" fn get_int(context: *mut c_void) -> c_int {
    // SAFETY: only `DataAccessor` registers this trampoline, with its slot.
    unsafe { scalar(context, |a| a.int.as_deref()) }
}

unsafe extern "C" fn set_int(context: *mut c_void, value: c_int) {
    // SAFETY: only `DataAccessor` registers this trampoline, with its slot.
    unsafe { store(context, value, |a| a.set_int.as_deref()) }
}

unsafe extern "C" fn get_float(context: *mut c_void) -> f32 {
    // SAFETY: only `DataAccessor` registers this trampoline, with its slot.
    unsafe { scalar(context, |a| a.float.as_deref()) }
}

unsafe extern "C" fn set_float(context: *mut c_void, value: f32) {
    // SAFETY: only `DataAccessor` registers this trampoline, with its slot.
    unsafe { store(context, value, |a| a.set_float.as_deref()) }
}

unsafe extern "C" fn get_double(context: *mut c_void) -> f64 {
    // SAFETY: only `DataAccessor` registers this trampoline, with its slot.
    unsafe { scalar(context, |a| a.double.as_deref()) }
}

unsafe extern "C" fn set_double(context: *mut c_void, value: f64) {
    // SAFETY: only `DataAccessor` registers this trampoline, with its slot.
    unsafe { store(context, value, |a| a.set_double.as_deref()) }
}

/// # Safety
/// `context` is the refcon of a live `DataAccessor`; `out` is null or valid
/// for `count` writes of `T`, as the SDK promises.
unsafe fn read_array<T>(
    context: *mut c_void,
    out: *mut T,
    offset: c_int,
    count: c_int,
    pick: impl FnOnce(&DataAccess) -> Option<&ArrayRead<T>>,
) -> c_int {
    // SAFETY: guaranteed by the caller.
    let slot = unsafe { lease::<DataAccess>(context) };
    let out = if out.is_null() {
        None
    } else {
        let length = usize::try_from(count).unwrap_or(0);
        // SAFETY: the SDK lends `count` writable elements for this call only.
        Some(unsafe { std::slice::from_raw_parts_mut(out, length) })
    };
    guard_read(slot.label, 0, || {
        pick(&slot.value).map_or(0, |read| read(out, offset))
    })
}

/// # Safety
/// `context` is the refcon of a live `DataAccessor`; `values` is null or
/// valid for `count` reads of `T`, as the SDK promises.
unsafe fn write_array<T>(
    context: *mut c_void,
    values: *mut T,
    offset: c_int,
    count: c_int,
    pick: impl FnOnce(&DataAccess) -> Option<&ArrayWrite<T>>,
) {
    if values.is_null() || count <= 0 {
        return;
    }
    // SAFETY: guaranteed by the caller.
    let slot = unsafe { lease::<DataAccess>(context) };
    // SAFETY: the SDK lends `count` readable elements for this call only.
    let values = unsafe { std::slice::from_raw_parts(values.cast_const(), count as usize) };
    guard(slot.label, (), || {
        if let Some(write) = pick(&slot.value) {
            write(values, offset);
        }
    });
}

unsafe extern "C" fn get_ints(
    context: *mut c_void,
    out: *mut c_int,
    offset: c_int,
    count: c_int,
) -> c_int {
    // SAFETY: registered only by `DataAccessor` with its slot; X-Plane
    // passes a null or `count`-element output.
    unsafe { read_array(context, out, offset, count, |a| a.ints.as_ref()) }
}

unsafe extern "C" fn set_ints(
    context: *mut c_void,
    values: *mut c_int,
    offset: c_int,
    count: c_int,
) {
    // SAFETY: registered only by `DataAccessor` with its slot; X-Plane
    // passes a null or `count`-element input.
    unsafe { write_array(context, values, offset, count, |a| a.set_ints.as_ref()) }
}

unsafe extern "C" fn get_floats(
    context: *mut c_void,
    out: *mut f32,
    offset: c_int,
    count: c_int,
) -> c_int {
    // SAFETY: registered only by `DataAccessor` with its slot; X-Plane
    // passes a null or `count`-element output.
    unsafe { read_array(context, out, offset, count, |a| a.floats.as_ref()) }
}

unsafe extern "C" fn set_floats(
    context: *mut c_void,
    values: *mut f32,
    offset: c_int,
    count: c_int,
) {
    // SAFETY: registered only by `DataAccessor` with its slot; X-Plane
    // passes a null or `count`-element input.
    unsafe { write_array(context, values, offset, count, |a| a.set_floats.as_ref()) }
}

unsafe extern "C" fn get_bytes(
    context: *mut c_void,
    out: *mut c_void,
    offset: c_int,
    count: c_int,
) -> c_int {
    // SAFETY: registered only by `DataAccessor` with its slot; X-Plane
    // passes a null or `count`-byte output.
    unsafe {
        read_array(context, out.cast::<u8>(), offset, count, |a| {
            a.bytes.as_ref()
        })
    }
}

unsafe extern "C" fn set_bytes(
    context: *mut c_void,
    values: *mut c_void,
    offset: c_int,
    count: c_int,
) {
    // SAFETY: registered only by `DataAccessor` with its slot; X-Plane
    // passes a null or `count`-byte input.
    unsafe {
        write_array(context, values.cast::<u8>(), offset, count, |a| {
            a.set_bytes.as_ref()
        })
    }
}

type MenuFn = Box<dyn Fn(usize)>;

/// An owned menu whose item selections call a Rust closure with the item's
/// refcon value.
#[must_use = "dropping a menu immediately destroys it"]
pub struct CallbackMenu {
    handle: MenuHandle,
    _slot: Rc<Slot<MenuFn>>,
}

impl CallbackMenu {
    /// Create a submenu under `parent` item `item`, or in the menu bar when
    /// `parent` is `None`. Returns `None` if X-Plane refuses.
    pub fn create(
        name: &CStr,
        parent: Option<MenuHandle>,
        item: c_int,
        label: &'static str,
        callback: impl Fn(usize) + 'static,
    ) -> Option<Self> {
        let slot = Slot::new(label, Box::new(callback) as MenuFn);
        // SAFETY: the name is NUL-terminated for the call; the parent is a
        // live SDK menu or null. The slot outlives the menu (see Drop).
        let handle = unsafe {
            sdk::XPLMCreateMenu(
                name.as_ptr(),
                parent.map_or(ptr::null_mut(), MenuHandle::as_raw),
                item,
                Some(menu_trampoline),
                refcon(&slot),
            )
        };
        // SAFETY: a nonnull result is the live menu just created.
        let handle = unsafe { MenuHandle::from_raw(handle) }?;
        Some(Self {
            handle,
            _slot: slot,
        })
    }

    /// The SDK handle, for item calls. It is valid while `self` lives.
    pub fn handle(&self) -> MenuHandle {
        self.handle
    }
}

impl Drop for CallbackMenu {
    fn drop(&mut self) {
        // SAFETY: this value created the menu and destroys it once.
        unsafe { sdk::XPLMDestroyMenu(self.handle.as_raw()) };
    }
}

unsafe extern "C" fn menu_trampoline(context: *mut c_void, item: *mut c_void) {
    // SAFETY: only `CallbackMenu` registers this trampoline, with its slot.
    let slot = unsafe { lease::<MenuFn>(context) };
    guard(slot.label, (), || (slot.value)(item as usize));
}

type DrawFn = Box<dyn Fn(sdk::XPLMDrawingPhase, bool) -> c_int>;

/// An owned draw callback registration.
#[must_use = "dropping a draw handler immediately unregisters it"]
pub struct DrawHandler {
    phase: sdk::XPLMDrawingPhase,
    before: c_int,
    slot: Rc<Slot<DrawFn>>,
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
        let slot = Slot::new(label, Box::new(callback) as DrawFn);
        let before = c_int::from(before);
        // SAFETY: the slot is a unique refcon that outlives the registration.
        let ok = unsafe {
            sdk::XPLMRegisterDrawCallback(Some(draw_trampoline), phase, before, refcon(&slot))
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
                refcon(&self.slot),
            );
        }
    }
}

unsafe extern "C" fn draw_trampoline(
    phase: sdk::XPLMDrawingPhase,
    before: c_int,
    context: *mut c_void,
) -> c_int {
    // SAFETY: only `DrawHandler` registers this trampoline, with its slot.
    let slot = unsafe { lease::<DrawFn>(context) };
    guard(slot.label, 1, || (slot.value)(phase, before != 0))
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
    handle: NonNull<c_void>,
    _slot: Rc<WindowSlot>,
}

impl CallbackWindow {
    /// Create the window. Returns `None` if X-Plane refuses.
    pub fn create(
        params: WindowParams,
        label: &'static str,
        handler: impl WindowHandler + 'static,
    ) -> Option<Self> {
        let slot: Rc<WindowSlot> = Slot::new(label, Box::new(handler));
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
            refcon: refcon(&slot),
            decorateAsFloatingWindow: params.decoration,
            layer: params.layer,
            handleRightClickFunc: Some(window_right_click),
        };
        // SAFETY: the structure has the SDK-prescribed size, every callback
        // has the SDK ABI, and the slot outlives the window (see Drop).
        let handle = NonNull::new(unsafe { sdk::XPLMCreateWindowEx(&mut create) })?;
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

impl Drop for CallbackWindow {
    fn drop(&mut self) {
        // SAFETY: this value created the window and destroys it once.
        unsafe { sdk::XPLMDestroyWindow(self.handle.as_ptr()) };
    }
}

/// # Safety
/// `context` is the refcon of a live `CallbackWindow`.
unsafe fn window_call<T>(
    context: *mut c_void,
    default: T,
    call: impl FnOnce(&dyn WindowHandler) -> T,
) -> T {
    // SAFETY: guaranteed by the caller.
    let slot = unsafe { lease::<Box<dyn WindowHandler>>(context) };
    guard(slot.label, default, || call(slot.value.as_ref()))
}

unsafe extern "C" fn window_draw(window: sdk::XPLMWindowID, context: *mut c_void) {
    // SAFETY: only `CallbackWindow` registers this trampoline, with its slot.
    unsafe { window_call(context, (), |h| h.draw(window)) };
}

unsafe extern "C" fn window_click(
    window: sdk::XPLMWindowID,
    x: c_int,
    y: c_int,
    status: sdk::XPLMMouseStatus,
    context: *mut c_void,
) -> c_int {
    // SAFETY: only `CallbackWindow` registers this trampoline, with its slot.
    unsafe { window_call(context, 0, |h| h.click(window, x, y, status)) }
}

unsafe extern "C" fn window_right_click(
    window: sdk::XPLMWindowID,
    x: c_int,
    y: c_int,
    status: sdk::XPLMMouseStatus,
    context: *mut c_void,
) -> c_int {
    // SAFETY: only `CallbackWindow` registers this trampoline, with its slot.
    unsafe { window_call(context, 0, |h| h.right_click(window, x, y, status)) }
}

unsafe extern "C" fn window_key(
    window: sdk::XPLMWindowID,
    key: c_char,
    flags: sdk::XPLMKeyFlags,
    virtual_key: c_char,
    context: *mut c_void,
    losing_focus: c_int,
) {
    // SAFETY: only `CallbackWindow` registers this trampoline, with its slot.
    unsafe {
        window_call(context, (), |h| {
            h.key(window, key, flags, virtual_key, losing_focus != 0)
        })
    };
}

unsafe extern "C" fn window_cursor(
    window: sdk::XPLMWindowID,
    x: c_int,
    y: c_int,
    context: *mut c_void,
) -> sdk::XPLMCursorStatus {
    let default = sdk::xplm_CursorDefault as sdk::XPLMCursorStatus;
    // SAFETY: only `CallbackWindow` registers this trampoline, with its slot.
    unsafe { window_call(context, default, |h| h.cursor(window, x, y)) }
}

unsafe extern "C" fn window_wheel(
    window: sdk::XPLMWindowID,
    x: c_int,
    y: c_int,
    wheel: c_int,
    clicks: c_int,
    context: *mut c_void,
) -> c_int {
    // SAFETY: only `CallbackWindow` registers this trampoline, with its slot.
    unsafe { window_call(context, 0, |h| h.wheel(window, x, y, wheel, clicks)) }
}

type AvionicsFn = Box<dyn Fn(sdk::XPLMDeviceID, bool) -> c_int>;

/// Owned draw callbacks for a built-in avionics device.
#[must_use = "dropping the callbacks immediately unregisters them"]
pub struct AvionicsDraw {
    handle: NonNull<c_void>,
    _slot: Rc<Slot<AvionicsFn>>,
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
        let slot = Slot::new(label, Box::new(callback) as AvionicsFn);
        // SAFETY: the structure is plain data; all-zero is "no callback" and
        // a null refcon, overwritten below.
        let mut params: sdk::XPLMCustomizeAvionics_t = unsafe { mem::zeroed() };
        params.structSize = mem::size_of_val(&params) as c_int;
        params.deviceId = device;
        params.drawCallbackBefore = Some(avionics_trampoline);
        params.drawCallbackAfter = Some(avionics_trampoline);
        params.refcon = refcon(&slot);
        // SAFETY: the structure has the SDK-prescribed size and the slot
        // outlives the registration (see Drop).
        let handle = NonNull::new(unsafe { sdk::XPLMRegisterAvionicsCallbacksEx(&mut params) })?;
        Some(Self {
            handle,
            _slot: slot,
        })
    }

    pub fn handle(&self) -> sdk::XPLMAvionicsID {
        self.handle.as_ptr()
    }
}

impl Drop for AvionicsDraw {
    fn drop(&mut self) {
        // SAFETY: this value registered the handle and releases it once.
        unsafe { sdk::XPLMUnregisterAvionicsCallbacks(self.handle.as_ptr()) };
    }
}

unsafe extern "C" fn avionics_trampoline(
    device: sdk::XPLMDeviceID,
    before: c_int,
    context: *mut c_void,
) -> c_int {
    // SAFETY: only `AvionicsDraw` registers this trampoline, with its slot.
    let slot = unsafe { lease::<AvionicsFn>(context) };
    guard(slot.label, 1, || (slot.value)(device, before != 0))
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
        let owner = Rc::new(RefCell::new(None::<Rc<Slot<DrawFn>>>));
        let weak = Rc::downgrade(&owner);
        let calls = Rc::new(Cell::new(0));
        let seen = calls.clone();
        let slot = Slot::new(
            "test draw",
            Box::new(move |phase, before| {
                // Dropping the owning reference mid-call must not free the
                // closure that is running.
                weak.upgrade().unwrap().borrow_mut().take();
                seen.set(seen.get() + 1);
                phase + c_int::from(before)
            }) as DrawFn,
        );
        let context = refcon(&slot);
        *owner.borrow_mut() = Some(slot);
        // SAFETY: `context` is the refcon of the live slot owned above.
        let result = unsafe { draw_trampoline(40, 1, context) };
        assert_eq!(result, 41);
        assert_eq!(calls.get(), 1);
        assert!(owner.borrow().is_none());
    }

    #[test]
    fn data_trampolines_route_to_present_callbacks() {
        let written = Rc::new(Cell::new(0.));
        let sink = written.clone();
        let received = Rc::new(RefCell::new((Vec::new(), 0)));
        let ints = received.clone();
        let slot = Slot::new(
            "test data",
            DataAccess {
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
        );
        let context = refcon(&slot);
        let mut out = [0.; 2];
        let mut input = [7, 8];
        // SAFETY: `context` is a live slot; the buffers match their counts.
        unsafe {
            assert_eq!(get_float(context), 2.5);
            assert_eq!(get_int(context), 0);
            set_float(context, 4.);
            assert_eq!(get_floats(context, ptr::null_mut(), 0, 0), 3);
            assert_eq!(get_floats(context, out.as_mut_ptr(), 1, 2), 2);
            set_ints(context, input.as_mut_ptr(), 1, 2);
            assert_eq!(get_bytes(context, ptr::null_mut(), 0, 0), 0);
        }
        assert_eq!(written.get(), 4.);
        assert_eq!(out, [2., 3.]);
        assert_eq!(*received.borrow(), (vec![7, 8], 1));
    }

    #[test]
    fn window_handler_defaults_pass_events_through() {
        struct Quiet;
        impl WindowHandler for Quiet {}
        let slot: Rc<WindowSlot> = Slot::new("test window", Box::new(Quiet));
        let context = refcon(&slot);
        let window = ptr::null_mut();
        // SAFETY: `context` is the refcon of a live window slot.
        unsafe {
            window_draw(window, context);
            assert_eq!(window_click(window, 1, 2, 1, context), 0);
            assert_eq!(window_wheel(window, 1, 2, 0, 1, context), 0);
            assert_eq!(
                window_cursor(window, 1, 2, context),
                sdk::xplm_CursorDefault as sdk::XPLMCursorStatus
            );
        }
    }
}
