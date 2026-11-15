//! `XPLMDataAccess.h`: dataref lookup and reads and writes.

use std::cell::{Cell, RefCell};
use std::ffi::{CStr, c_void};

use xplane_sdk_sys as xp;

use super::{AddressSet, Found, count};

thread_local! {
    static SEEN_DATAREFS: RefCell<AddressSet> = RefCell::new(AddressSet::default());
}

static DATAREFS: Found = Found::new(&SEEN_DATAREFS);

/// A dataref handle from `XPLMFindDataRef`, or the null handle.
///
/// Unlike [`crate::DataRef`], which only ever holds a found dataref and picks
/// types for the caller, this keeps a failed lookup as the null handle and
/// passes it on: X-Plane reads it as zero and ignores writes to it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct DataRef(usize);

impl DataRef {
    /// The null handle; XPLM ignores writes to it and reads it as zero.
    pub const NULL: Self = Self(0);

    /// `XPLMFindDataRef(name)`; the null handle when X-Plane has no such dataref.
    pub fn find(name: &CStr) -> Self {
        // SAFETY: `name` is NUL-terminated and outlives the call; XPLM only reads it.
        let raw = unsafe { xp::XPLMFindDataRef(name.as_ptr()) } as usize;
        DATAREFS.insert(raw);
        Self(raw)
    }

    /// [`DataRef::find`] for a `b"...\0"` byte string, read up to its first NUL
    /// as XPLM would.
    ///
    /// # Panics
    /// If `name` contains no NUL.
    pub fn find_bytes(name: &[u8]) -> Self {
        Self::find(CStr::from_bytes_until_nul(name).expect("NUL-terminated dataref name"))
    }

    /// The handle that was stored as [`DataRef::raw`].
    ///
    /// # Panics
    /// If `raw` is neither 0 nor a handle [`DataRef::find`] returned in this
    /// process.
    pub fn from_raw(raw: usize) -> Self {
        assert!(
            DATAREFS.contains(raw),
            "not an XPLM dataref handle: {raw:#x}"
        );
        Self(raw)
    }

    /// The handle as an integer (0 for null).
    pub const fn raw(self) -> usize {
        self.0
    }

    pub const fn is_null(self) -> bool {
        self.0 == 0
    }

    fn ptr(self) -> xp::XPLMDataRef {
        self.0 as xp::XPLMDataRef
    }

    /// The handle for a call the SDK does not document as null-safe.
    ///
    /// # Panics
    /// If the handle is null.
    fn non_null(self) -> xp::XPLMDataRef {
        assert!(!self.is_null(), "XPLM dataref call with the null handle");
        self.ptr()
    }

    /// `XPLMGetDataRefTypes`.
    ///
    /// # Panics
    /// If the handle is null; check the lookup first.
    pub fn types(self) -> i32 {
        let dataref = self.non_null();
        // SAFETY: a non-null handle is a live dataref (module note); no pointers are passed.
        unsafe { xp::XPLMGetDataRefTypes(dataref) }
    }

    /// `XPLMCanWriteDataRef(..) != 0`.
    ///
    /// # Panics
    /// If the handle is null; check the lookup first.
    pub fn writable(self) -> bool {
        let dataref = self.non_null();
        // SAFETY: a non-null handle is a live dataref (module note); no pointers are passed.
        unsafe { xp::XPLMCanWriteDataRef(dataref) != 0 }
    }

    /// `XPLMIsDataRefGood(..) != 0`.
    ///
    /// # Panics
    /// If the handle is null; check the lookup first.
    pub fn good(self) -> bool {
        let dataref = self.non_null();
        // SAFETY: a non-null handle is a live dataref (module note); no pointers are passed.
        unsafe { xp::XPLMIsDataRefGood(dataref) != 0 }
    }

    /// `XPLMGetDatai`.
    pub fn get_i32(self) -> i32 {
        // SAFETY: the handle is null or a live dataref; XPLM checks its type itself.
        unsafe { xp::XPLMGetDatai(self.ptr()) }
    }

    /// `XPLMGetDataf`.
    pub fn get_f32(self) -> f32 {
        // SAFETY: the handle is null or a live dataref; XPLM checks its type itself.
        unsafe { xp::XPLMGetDataf(self.ptr()) }
    }

    /// `XPLMGetDatad`.
    pub fn get_f64(self) -> f64 {
        // SAFETY: the handle is null or a live dataref; XPLM checks its type itself.
        unsafe { xp::XPLMGetDatad(self.ptr()) }
    }

    /// `XPLMSetDatai`.
    pub fn set_i32(self, value: i32) {
        // SAFETY: the handle is null or a live dataref; XPLM ignores writes of the wrong type.
        unsafe { xp::XPLMSetDatai(self.ptr(), value) }
    }

    /// `XPLMSetDataf`.
    pub fn set_f32(self, value: f32) {
        // SAFETY: the handle is null or a live dataref; XPLM ignores writes of the wrong type.
        unsafe { xp::XPLMSetDataf(self.ptr(), value) }
    }

    /// `XPLMSetDatad`.
    pub fn set_f64(self, value: f64) {
        // SAFETY: the handle is null or a live dataref; XPLM ignores writes of the wrong type.
        unsafe { xp::XPLMSetDatad(self.ptr(), value) }
    }

    /// `XPLMGetDatavi(ref, out, offset, count)`: values copied. `count` is
    /// `out.len()` saturated at `i32::MAX` (module note).
    pub fn get_i32s(self, offset: i32, out: &mut [i32]) -> i32 {
        // SAFETY: `out` is exclusively borrowed for the call and XPLM writes at most the count
        // passed, which is non-negative and at most its length; the handle is null or live.
        unsafe { xp::XPLMGetDatavi(self.ptr(), out.as_mut_ptr(), offset, count(out.len())) }
    }

    /// `XPLMGetDatavf(ref, out, offset, count)`: values copied. `count` is
    /// `out.len()` saturated at `i32::MAX` (module note).
    pub fn get_f32s(self, offset: i32, out: &mut [f32]) -> i32 {
        // SAFETY: `out` is exclusively borrowed for the call and XPLM writes at most the count
        // passed, which is non-negative and at most its length; the handle is null or live.
        unsafe { xp::XPLMGetDatavf(self.ptr(), out.as_mut_ptr(), offset, count(out.len())) }
    }

    /// `XPLMGetDatab(ref, out, offset, count)`: bytes copied. `count` is
    /// `out.len()` saturated at `i32::MAX` (module note).
    pub fn get_bytes(self, offset: i32, out: &mut [u8]) -> i32 {
        // SAFETY: `out` is exclusively borrowed for the call and XPLM writes at most the count
        // passed, which is non-negative and at most its length; the handle is null or live.
        unsafe {
            xp::XPLMGetDatab(
                self.ptr(),
                out.as_mut_ptr().cast::<c_void>(),
                offset,
                count(out.len()),
            )
        }
    }

    /// `XPLMGetDatavf(ref, buffer, offset, N)` for a cell array that a
    /// synchronous dataref callback may also use. X-Plane writes a local copy
    /// of the cell, stored back after the call, so its output buffer is never
    /// the cell's storage: if the dataref is one this plugin registered, the
    /// accessor's view of the buffer cannot alias a closure's use of the cell.
    pub fn get_f32s_cell<const N: usize>(self, offset: i32, out: &Cell<[f32; N]>) -> i32 {
        let mut buffer = out.get();
        let copied = self.get_f32s(offset, &mut buffer);
        out.set(buffer);
        copied
    }

    /// The array size query `XPLMGetDatavi(ref, null, offset, 0)`.
    pub fn count_i32s(self, offset: i32) -> i32 {
        // SAFETY: a null buffer with count 0 is XPLM's documented size query; nothing is written.
        unsafe { xp::XPLMGetDatavi(self.ptr(), std::ptr::null_mut(), offset, 0) }
    }

    /// The array size query `XPLMGetDatavf(ref, null, offset, 0)`.
    pub fn count_f32s(self, offset: i32) -> i32 {
        // SAFETY: a null buffer with count 0 is XPLM's documented size query; nothing is written.
        unsafe { xp::XPLMGetDatavf(self.ptr(), std::ptr::null_mut(), offset, 0) }
    }

    /// The size query `XPLMGetDatab(ref, null, offset, 0)`.
    pub fn count_bytes(self, offset: i32) -> i32 {
        // SAFETY: a null buffer with count 0 is XPLM's documented size query; nothing is written.
        unsafe { xp::XPLMGetDatab(self.ptr(), std::ptr::null_mut(), offset, 0) }
    }

    /// `XPLMSetDatavi(ref, values, offset, count)`, `count` being
    /// `values.len()` saturated at `i32::MAX` (module note).
    pub fn set_i32s(self, offset: i32, values: &[i32]) {
        // SAFETY: XPLM only reads the count passed (non-negative, at most `values.len()`) from
        // the borrowed slice despite the `*mut` in the C signature; the handle is null or live.
        unsafe {
            xp::XPLMSetDatavi(
                self.ptr(),
                values.as_ptr().cast_mut(),
                offset,
                count(values.len()),
            )
        }
    }

    /// `XPLMSetDatavf(ref, values, offset, count)`, `count` being
    /// `values.len()` saturated at `i32::MAX` (module note).
    pub fn set_f32s(self, offset: i32, values: &[f32]) {
        // SAFETY: XPLM only reads the count passed (non-negative, at most `values.len()`) from
        // the borrowed slice despite the `*mut` in the C signature; the handle is null or live.
        unsafe {
            xp::XPLMSetDatavf(
                self.ptr(),
                values.as_ptr().cast_mut(),
                offset,
                count(values.len()),
            )
        }
    }

    /// `XPLMSetDatab(ref, values, offset, count)`, `count` being
    /// `values.len()` saturated at `i32::MAX` (module note).
    pub fn set_bytes(self, offset: i32, values: &[u8]) {
        // SAFETY: XPLM only reads the count passed (non-negative, at most `values.len()`) from
        // the borrowed slice despite the `*mut` in the C signature; the handle is null or live.
        unsafe {
            xp::XPLMSetDatab(
                self.ptr(),
                values.as_ptr().cast_mut().cast::<c_void>(),
                offset,
                count(values.len()),
            )
        }
    }
}
