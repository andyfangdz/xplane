use std::ffi::c_void;

use xplane_sdk_sys::{
    XPLMDataRef, XPLMFindDataRef, XPLMGetDatab, XPLMGetDatad, XPLMGetDataf, XPLMGetDatai,
    XPLMGetDatavf, XPLMGetDatavi, XPLMSetDatad, XPLMSetDataf, XPLMSetDatai, XPLMSetDatavf,
};

use crate::c_string;

/// A non-owning handle returned by X-Plane's dataref registry.
///
/// X-Plane owns the referenced data. This wrapper centralizes lookup, slice
/// bounds conversion, and the small unsafe FFI boundary used by plugins.
#[derive(Copy, Clone)]
pub struct DataRef(XPLMDataRef);

impl DataRef {
    pub fn find(name: &str) -> Option<Self> {
        let name = c_string(name);
        // SAFETY: `name` is NUL-terminated and remains live for the call.
        let dataref = unsafe { XPLMFindDataRef(name.as_ptr()) };
        (!dataref.is_null()).then_some(Self(dataref))
    }

    pub fn required(name: &str) -> Result<Self, String> {
        Self::find(name).ok_or_else(|| format!("missing required dataref: {name}"))
    }

    pub fn get_i32(self) -> i32 {
        // SAFETY: `DataRef` is only constructed from a successful XPLM lookup.
        unsafe { XPLMGetDatai(self.0) }
    }

    pub fn get_f32(self) -> f32 {
        // SAFETY: `DataRef` is only constructed from a successful XPLM lookup.
        unsafe { XPLMGetDataf(self.0) }
    }

    pub fn get_f64(self) -> f64 {
        // SAFETY: `DataRef` is only constructed from a successful XPLM lookup.
        unsafe { XPLMGetDatad(self.0) }
    }

    pub fn read_f32(self, values: &mut [f32]) -> usize {
        self.read_f32_from(0, values)
    }

    pub fn read_f32_from(self, offset: i32, values: &mut [f32]) -> usize {
        let count = slice_count(values.len());
        // SAFETY: `values` is writable for `count` floats and the dataref came
        // from XPLM. X-Plane validates the dataref's underlying type.
        let read = unsafe { XPLMGetDatavf(self.0, values.as_mut_ptr(), offset, count) };
        returned_count(read, values.len())
    }

    pub fn read_i32(self, values: &mut [i32]) -> usize {
        let count = slice_count(values.len());
        // SAFETY: `values` is writable for `count` integers.
        let read = unsafe { XPLMGetDatavi(self.0, values.as_mut_ptr(), 0, count) };
        returned_count(read, values.len())
    }

    pub fn read_string(self, limit: usize) -> String {
        let mut bytes = vec![0_u8; limit];
        let count = slice_count(limit);
        // SAFETY: `bytes` is writable for `count` bytes.
        let read = unsafe { XPLMGetDatab(self.0, bytes.as_mut_ptr().cast::<c_void>(), 0, count) };
        bytes.truncate(returned_count(read, limit));
        String::from_utf8_lossy(&bytes)
            .trim_end_matches('\0')
            .to_owned()
    }

    pub fn set_i32(self, value: i32) {
        // SAFETY: the handle came from XPLM; X-Plane owns type/writeability checks.
        unsafe { XPLMSetDatai(self.0, value) }
    }

    pub fn set_f32(self, value: f32) {
        // SAFETY: the handle came from XPLM; X-Plane owns type/writeability checks.
        unsafe { XPLMSetDataf(self.0, value) }
    }

    pub fn set_f64(self, value: f64) {
        // SAFETY: the handle came from XPLM; X-Plane owns type/writeability checks.
        unsafe { XPLMSetDatad(self.0, value) }
    }

    pub fn write_f32(self, values: &[f32]) {
        let count = slice_count(values.len());
        // SAFETY: `values` is readable for `count` floats.
        unsafe { XPLMSetDatavf(self.0, values.as_ptr().cast_mut(), 0, count) }
    }
}

fn slice_count(length: usize) -> i32 {
    i32::try_from(length).expect("dataref slice length exceeds i32::MAX")
}

fn returned_count(count: i32, capacity: usize) -> usize {
    usize::try_from(count).unwrap_or(0).min(capacity)
}
