use crate::c_string;
use std::{cell::Cell, ffi::c_void, ptr::NonNull};
use xplane_sdk_sys::*;

struct Storage {
    integer: Cell<i32>,
    float: Cell<f32>,
    double: Cell<f64>,
    changed: Option<fn()>,
}
/// Owns an exported scalar and its stable callback storage. Reading diagnostics
/// never borrows a plugin's main RefCell, including during reentrant SDK calls.
/// Values and callbacks are restricted to X-Plane's plugin thread.
pub struct OwnedDataRef {
    handle: NonNull<c_void>,
    storage: Box<Storage>,
}
impl OwnedDataRef {
    pub fn integer(
        name: &str,
        value: i32,
        writable: bool,
        changed: Option<fn()>,
    ) -> Result<Self, String> {
        Self::create(name, Some(value), 0.0, None, writable, changed)
    }
    pub fn float(
        name: &str,
        value: f32,
        writable: bool,
        changed: Option<fn()>,
    ) -> Result<Self, String> {
        Self::create(name, None, value, None, writable, changed)
    }
    pub fn double(
        name: &str,
        value: f64,
        writable: bool,
        changed: Option<fn()>,
    ) -> Result<Self, String> {
        Self::create(name, None, 0.0, Some(value), writable, changed)
    }
    fn create(
        name: &str,
        integer: Option<i32>,
        float: f32,
        double: Option<f64>,
        writable: bool,
        changed: Option<fn()>,
    ) -> Result<Self, String> {
        let name = c_string(name);
        let mut storage = Box::new(Storage {
            integer: Cell::new(integer.unwrap_or(0)),
            float: Cell::new(float),
            double: Cell::new(double.unwrap_or(0.0)),
            changed,
        });
        let pointer = (&mut *storage as *mut Storage).cast::<c_void>();
        // SAFETY: Box keeps callback storage at a stable address. Only the
        // registered scalar type's callbacks are supplied. Drop unregisters
        // the accessor before the Box is freed, also on partial startup errors.
        let handle = unsafe {
            XPLMRegisterDataAccessor(
                name.as_ptr(),
                if integer.is_some() {
                    xplmType_Int
                } else if double.is_some() {
                    xplmType_Double
                } else {
                    xplmType_Float
                },
                i32::from(writable),
                if integer.is_some() {
                    Some(read_i32)
                } else {
                    None
                },
                if integer.is_some() && writable {
                    Some(write_i32)
                } else {
                    None
                },
                if integer.is_none() && double.is_none() {
                    Some(read_f32)
                } else {
                    None
                },
                if integer.is_none() && double.is_none() && writable {
                    Some(write_f32)
                } else {
                    None
                },
                if double.is_some() {
                    Some(read_f64)
                } else {
                    None
                },
                if double.is_some() && writable {
                    Some(write_f64)
                } else {
                    None
                },
                None,
                None,
                None,
                None,
                None,
                None,
                pointer,
                pointer,
            )
        };
        let handle =
            NonNull::new(handle).ok_or_else(|| format!("dataref registration failed: {name:?}"))?;
        Ok(Self { handle, storage })
    }
    pub fn get_i32(&self) -> i32 {
        self.storage.integer.get()
    }
    pub fn get_f32(&self) -> f32 {
        self.storage.float.get()
    }
    pub fn get_f64(&self) -> f64 {
        self.storage.double.get()
    }
    pub fn set_f64(&self, value: f64) {
        self.storage.double.set(value);
    }
    pub fn set_i32(&self, value: i32) {
        self.storage.integer.set(value);
    }
    pub fn set_f32(&self, value: f32) {
        self.storage.float.set(value);
    }
}
impl Drop for OwnedDataRef {
    fn drop(&mut self) {
        // SAFETY: this owns the registration, and storage is still live. XPLM
        // guarantees no further callbacks after unregistering the accessor.
        unsafe { XPLMUnregisterDataAccessor(self.handle.as_ptr()) };
    }
}
unsafe extern "C" fn read_i32(pointer: *mut c_void) -> i32 {
    // SAFETY: only registered with a live Box<Storage>, retained by OwnedDataRef.
    unsafe { &*pointer.cast::<Storage>() }.integer.get()
}
unsafe extern "C" fn read_f32(pointer: *mut c_void) -> f32 {
    // SAFETY: same stable registration storage contract as read_i32.
    unsafe { &*pointer.cast::<Storage>() }.float.get()
}
unsafe extern "C" fn write_i32(pointer: *mut c_void, value: i32) {
    // SAFETY: the SDK supplies the live registration's Box<Storage> refcon.
    let storage = unsafe { &*pointer.cast::<Storage>() };
    storage.integer.set(value);
    if let Some(changed) = storage.changed {
        changed();
    }
}
unsafe extern "C" fn write_f32(pointer: *mut c_void, value: f32) {
    if !value.is_finite() {
        return;
    }
    // SAFETY: the SDK supplies the live registration's Box<Storage> refcon.
    let storage = unsafe { &*pointer.cast::<Storage>() };
    storage.float.set(value);
    if let Some(changed) = storage.changed {
        changed();
    }
}
unsafe extern "C" fn read_f64(pointer: *mut c_void) -> f64 {
    // SAFETY: same stable registration storage contract as read_i32.
    unsafe { &*pointer.cast::<Storage>() }.double.get()
}
unsafe extern "C" fn write_f64(pointer: *mut c_void, value: f64) {
    if !value.is_finite() {
        return;
    }
    // SAFETY: the SDK supplies the live registration's Box<Storage> refcon.
    let storage = unsafe { &*pointer.cast::<Storage>() };
    storage.double.set(value);
    if let Some(changed) = storage.changed {
        changed();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn callbacks_keep_storage_stable_and_reject_nan() {
        let mut storage = Box::new(Storage {
            integer: Cell::new(42),
            float: Cell::new(0.5),
            double: Cell::new(40.87829970000001),
            changed: None,
        });
        let pointer = (&mut *storage as *mut Storage).cast();
        // SAFETY: these calls use the same allocated storage as registration.
        unsafe {
            assert_eq!(read_i32(pointer), 42);
            write_i32(pointer, 7);
            assert_eq!(read_i32(pointer), 7);
            write_f32(pointer, f32::NAN);
            assert_eq!(read_f32(pointer), 0.5);
            write_f32(pointer, 0.75);
            assert_eq!(read_f32(pointer), 0.75);
            assert_eq!(read_f64(pointer), 40.87829970000001);
            write_f64(pointer, f64::NAN);
            assert_eq!(read_f64(pointer), 40.87829970000001);
            write_f64(pointer, -74.27844499363563);
            assert_eq!(read_f64(pointer), -74.27844499363563);
        }
    }
}
