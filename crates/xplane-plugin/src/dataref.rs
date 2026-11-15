use std::{cell::RefCell, collections::HashMap};

use crate::{c_string, xplm};

/// A non-owning handle returned by X-Plane's dataref registry.
///
/// X-Plane owns the referenced data. This wrapper centralizes lookup, slice
/// bounds conversion, and the small unsafe FFI boundary used by plugins.
#[derive(Copy, Clone)]
pub struct DataRef(xplm::DataRef);

/// Lazy, plugin-thread lookup for datarefs that may register after plugin startup.
///
/// Only successful lookups are cached. Missing datarefs are retried on the next
/// access. Call `clear` when aircraft or provider lifecycle changes require fresh
/// lookups. Value defaults and write policies belong to the caller.
#[derive(Default)]
pub struct DataRefCache {
    refs: RefCell<HashMap<&'static str, DataRef>>,
}

impl DataRefCache {
    pub fn find(&self, name: &'static str) -> Option<DataRef> {
        self.find_with(name, DataRef::find)
    }

    pub fn clear(&mut self) {
        self.refs.get_mut().clear();
    }

    fn find_with(
        &self,
        name: &'static str,
        lookup: impl FnOnce(&str) -> Option<DataRef>,
    ) -> Option<DataRef> {
        if let Some(reference) = self.refs.borrow().get(name).copied() {
            return Some(reference);
        }
        // Release the cache borrow before calling into the SDK: it may invoke
        // plugin callbacks synchronously.
        let reference = lookup(name)?;
        self.refs.borrow_mut().insert(name, reference);
        Some(reference)
    }
}

impl DataRef {
    /// Writes the scalar type exposed by the SDK, preferring float like the
    /// native actuator adapters. Array writes require an explicit slice API.
    pub fn set_scalar(self, value: f64) {
        let kind = self.0.types();
        if kind & xplane_sdk_sys::xplmType_Float != 0 {
            self.set_f32(value as f32);
        } else if kind & xplane_sdk_sys::xplmType_Int != 0 {
            self.set_i32(value as i32);
        } else if kind & xplane_sdk_sys::xplmType_Double != 0 {
            self.set_f64(value);
        }
    }
    /// Selects the native scalar representation, preferring double precision.
    pub fn scalar(self) -> Option<f64> {
        let kind = self.0.types();
        if kind & xplane_sdk_sys::xplmType_Double != 0 {
            Some(self.get_f64())
        } else if kind & xplane_sdk_sys::xplmType_Float != 0 {
            Some(f64::from(self.get_f32()))
        } else if kind & xplane_sdk_sys::xplmType_Int != 0 {
            Some(f64::from(self.get_i32()))
        } else {
            None
        }
    }

    pub fn writable(self) -> bool {
        self.0.writable()
    }

    pub fn array_element(self, index: i32) -> f64 {
        if index < 0 {
            return 0.0;
        }
        let kind = self.0.types();
        if kind & xplane_sdk_sys::xplmType_IntArray != 0 {
            let mut value = [0];
            self.0.get_i32s(index, &mut value);
            f64::from(value[0])
        } else {
            let mut value = [0.0];
            self.read_f32_from(index, &mut value);
            f64::from(value[0])
        }
    }

    pub fn find(name: &str) -> Option<Self> {
        let dataref = xplm::DataRef::find(&c_string(name));
        (!dataref.is_null()).then_some(Self(dataref))
    }

    pub fn required(name: &str) -> Result<Self, String> {
        Self::find(name).ok_or_else(|| format!("missing required dataref: {name}"))
    }

    pub fn get_i32(self) -> i32 {
        self.0.get_i32()
    }

    pub fn get_f32(self) -> f32 {
        self.0.get_f32()
    }

    pub fn get_f64(self) -> f64 {
        self.0.get_f64()
    }

    pub fn read_f32(self, values: &mut [f32]) -> usize {
        self.read_f32_from(0, values)
    }

    pub fn read_f32_from(self, offset: i32, values: &mut [f32]) -> usize {
        let count = slice_count(values.len());
        let read = self.0.get_f32s(offset, &mut values[..count as usize]);
        returned_count(read, values.len())
    }

    pub fn read_i32(self, values: &mut [i32]) -> usize {
        let count = slice_count(values.len());
        let read = self.0.get_i32s(0, &mut values[..count as usize]);
        returned_count(read, values.len())
    }

    pub fn read_string(self, limit: usize) -> String {
        let mut bytes = vec![0_u8; limit];
        let count = slice_count(limit);
        let read = self.0.get_bytes(0, &mut bytes[..count as usize]);
        bytes.truncate(returned_count(read, limit));
        String::from_utf8_lossy(&bytes)
            .trim_end_matches('\0')
            .to_owned()
    }

    pub fn set_i32(self, value: i32) {
        self.0.set_i32(value)
    }

    pub fn set_f32(self, value: f32) {
        self.0.set_f32(value)
    }

    pub fn set_f64(self, value: f64) {
        self.0.set_f64(value)
    }

    pub fn write_f32(self, values: &[f32]) {
        let count = slice_count(values.len());
        self.0.set_f32s(0, &values[..count as usize])
    }
}

fn slice_count(length: usize) -> i32 {
    i32::try_from(length).expect("dataref slice length exceeds i32::MAX")
}

fn returned_count(count: i32, capacity: usize) -> usize {
    usize::try_from(count).unwrap_or(0).min(capacity)
}

#[cfg(test)]
mod tests {
    use super::{DataRef, DataRefCache};

    #[test]
    fn cache_retries_missing_refs_reuses_hits_and_clears_for_reload() {
        let mut cache = DataRefCache::default();
        // Opaque test handles are only compared; no SDK reads or writes occur.
        let first = DataRef(crate::xplm::DataRef::for_test(8));
        let second = DataRef(crate::xplm::DataRef::for_test(16));
        assert!(cache.find_with("late/provider", |_| None).is_none());
        assert_eq!(
            cache.find_with("late/provider", |_| Some(first)).unwrap().0,
            first.0
        );
        assert_eq!(
            cache
                .find_with("late/provider", |_| panic!("cached lookup reached SDK"))
                .unwrap()
                .0,
            first.0
        );
        cache.clear();
        assert_eq!(
            cache
                .find_with("late/provider", |_| Some(second))
                .unwrap()
                .0,
            second.0
        );
    }

    #[test]
    fn cache_does_not_hold_a_borrow_during_lookup() {
        let cache = DataRefCache::default();
        let reference = DataRef(crate::xplm::DataRef::for_test(8));
        let found = cache.find_with("outer", |_| {
            cache.find_with("callback", |_| Some(reference))
        });
        assert_eq!(found.unwrap().0, reference.0);
        assert_eq!(cache.refs.borrow().len(), 2);
    }
}
