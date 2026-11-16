use crate::{
    c_string,
    callbacks::{DataAccess, DataAccessor, Writes},
};
use std::{cell::Cell, rc::Rc};
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
    // Field order: the accessor is unregistered before this owner's share of
    // the storage goes (the accessor's callbacks hold the other shares).
    _accessor: DataAccessor,
    storage: Rc<Storage>,
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
        let storage = Rc::new(Storage {
            integer: Cell::new(integer.unwrap_or(0)),
            float: Cell::new(float),
            double: Cell::new(double.unwrap_or(0.0)),
            changed,
        });
        let kind = if integer.is_some() {
            xplmType_Int
        } else if double.is_some() {
            xplmType_Double
        } else {
            xplmType_Float
        };
        // Only the registered scalar type's callbacks are supplied; the
        // accessor registers the setter only when `writable`.
        let access = access(&storage, kind);
        let accessor = DataAccessor::register_shaped(
            &name,
            kind,
            writable,
            "dataref read",
            "dataref write",
            Writes::Storage,
            access,
        )
        .ok_or_else(|| format!("dataref registration failed: {name:?}"))?;
        Ok(Self {
            _accessor: accessor,
            storage,
        })
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
fn notify(storage: &Storage) {
    if let Some(changed) = storage.changed {
        crate::guard::guard("dataref write", (), changed);
    }
}

/// The getter and setter of the scalar type `kind` over `storage`. Writes of
/// non-finite floats and doubles are ignored.
fn access(storage: &Rc<Storage>, kind: XPLMDataTypeID) -> DataAccess {
    let (read, write) = (storage.clone(), storage.clone());
    if kind == xplmType_Int {
        DataAccess {
            int: Some(Box::new(move || read.integer.get())),
            set_int: Some(Box::new(move |value| {
                write.integer.set(value);
                notify(&write);
            })),
            ..DataAccess::default()
        }
    } else if kind == xplmType_Double {
        DataAccess {
            double: Some(Box::new(move || read.double.get())),
            set_double: Some(Box::new(move |value: f64| {
                if value.is_finite() {
                    write.double.set(value);
                    notify(&write);
                }
            })),
            ..DataAccess::default()
        }
    } else {
        DataAccess {
            float: Some(Box::new(move || read.float.get())),
            set_float: Some(Box::new(move |value: f32| {
                if value.is_finite() {
                    write.float.set(value);
                    notify(&write);
                }
            })),
            ..DataAccess::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn callbacks_share_storage_and_reject_nan() {
        let storage = Rc::new(Storage {
            integer: Cell::new(42),
            float: Cell::new(0.5),
            double: Cell::new(40.87829970000001),
            changed: None,
        });
        let int = access(&storage, xplmType_Int);
        assert!(int.float.is_none() && int.double.is_none());
        let (read, write) = (int.int.unwrap(), int.set_int.unwrap());
        assert_eq!(read(), 42);
        write(7);
        assert_eq!(read(), 7);
        let float = access(&storage, xplmType_Float);
        assert!(float.int.is_none() && float.double.is_none());
        let (read, write) = (float.float.unwrap(), float.set_float.unwrap());
        write(f32::NAN);
        assert_eq!(read(), 0.5);
        write(0.75);
        assert_eq!(read(), 0.75);
        let double = access(&storage, xplmType_Double);
        assert!(double.int.is_none() && double.float.is_none());
        let (read, write) = (double.double.unwrap(), double.set_double.unwrap());
        assert_eq!(read(), 40.87829970000001);
        write(f64::NAN);
        assert_eq!(read(), 40.87829970000001);
        write(-74.27844499363563);
        assert_eq!(read(), -74.27844499363563);
        assert_eq!(storage.integer.get(), 7);
    }
}
