//! `XPLMDataAccess.h`: dataref lookup and reads and writes.

use std::cell::{Cell, RefCell};
use std::ffi::CStr;

use xplane_sdk_sys as xp;

use super::{AddressSet, Found, count};

thread_local! {
    static SEEN_DATAREFS: RefCell<AddressSet> = RefCell::new(AddressSet::default());
}

static DATAREFS: Found = Found::new(&SEEN_DATAREFS);

/// Methods of [`DataRef`] for five classes of dataref calls, one arm and one
/// safety argument per class:
///
/// - `nonnull`: the handle and nothing else, on a handle checked non-null
///   (these calls are not documented as null-safe). Members:
///   `XPLMGetDataRefTypes`, `XPLMCanWriteDataRef`, `XPLMIsDataRefGood`.
/// - `value`: the handle (null or live) and plain values; XPLM checks the
///   handle for null and the dataref's type itself. Members: `XPLMGetDatai`,
///   `XPLMGetDataf`, `XPLMGetDatad`, `XPLMSetDatai`, `XPLMSetDataf`,
///   `XPLMSetDatad`.
/// - `read`: an exclusively borrowed output slice and its length as the count
///   (module note). Members: `XPLMGetDatavi`, `XPLMGetDatavf`, `XPLMGetDatab`.
/// - `size`: XPLM's documented size query, a null buffer with count 0.
///   Members: `XPLMGetDatavi`, `XPLMGetDatavf`, `XPLMGetDatab`.
/// - `write`: a borrowed input slice and its length as the count, which XPLM
///   only reads despite the `*mut` in its signature. Members:
///   `XPLMSetDatavi`, `XPLMSetDatavf`, `XPLMSetDatab`.
macro_rules! dataref_calls {
    (nonnull: $($(#[$attr:meta])* $vis:vis fn $name:ident(self) -> $ret:ty = $sdk:ident;)+) => {$(
        $(#[$attr])*
        $vis fn $name(self) -> $ret {
            let dataref = self.non_null();
            // SAFETY: a `nonnull` call (macro note): a non-null handle is a live dataref (module
            // note); no pointers are passed.
            unsafe { xp::$sdk(dataref) }
        }
    )+};
    (value: $($(#[$attr:meta])* pub fn $name:ident(self $(, $arg:ident: $ty:ty)?) $(-> $ret:ty)? = $sdk:ident;)+) => {$(
        $(#[$attr])*
        pub fn $name(self $(, $arg: $ty)?) $(-> $ret)? {
            // SAFETY: a `value` call (macro note): the handle is null or a live dataref, which
            // XPLM checks itself, as it checks the dataref's type; plain values otherwise.
            unsafe { xp::$sdk(self.ptr() $(, $arg)?) }
        }
    )+};
    (read: $($(#[$attr:meta])* pub fn $name:ident(self, offset: i32, out: &mut [$t:ty]) = $sdk:ident;)+) => {$(
        $(#[$attr])*
        pub fn $name(self, offset: i32, out: &mut [$t]) -> i32 {
            // SAFETY: a `read` call (macro note): `out` is exclusively borrowed for the call and
            // XPLM writes at most the count passed, which is non-negative and at most its length;
            // the handle is null or live.
            unsafe { xp::$sdk(self.ptr(), out.as_mut_ptr().cast(), offset, count(out.len())) }
        }
    )+};
    (size: $($(#[$attr:meta])* pub fn $name:ident(self, offset: i32) = $sdk:ident;)+) => {$(
        $(#[$attr])*
        pub fn $name(self, offset: i32) -> i32 {
            // SAFETY: a `size` call (macro note): a null buffer with count 0 is XPLM's documented
            // size query, so nothing is written; the handle is null or live.
            unsafe { xp::$sdk(self.ptr(), std::ptr::null_mut(), offset, 0) }
        }
    )+};
    (write: $($(#[$attr:meta])* pub fn $name:ident(self, offset: i32, values: &[$t:ty]) = $sdk:ident;)+) => {$(
        $(#[$attr])*
        pub fn $name(self, offset: i32, values: &[$t]) {
            // SAFETY: a `write` call (macro note): XPLM only reads the count passed
            // (non-negative, at most `values.len()`) from the borrowed slice despite the `*mut`
            // in the C signature; the handle is null or live.
            unsafe {
                xp::$sdk(
                    self.ptr(),
                    values.as_ptr().cast_mut().cast(),
                    offset,
                    count(values.len()),
                )
            }
        }
    )+};
}

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
        str_calls! {
            fn sdk_find(name: &CStr) -> xp::XPLMDataRef = XPLMFindDataRef;
        }
        let raw = sdk_find(name) as usize;
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

    /// An opaque handle for tests that only compare handles and make no call.
    #[cfg(test)]
    pub(crate) const fn for_test(raw: usize) -> Self {
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

    dataref_calls! {
        nonnull:
        /// `XPLMGetDataRefTypes`.
        ///
        /// # Panics
        /// If the handle is null; check the lookup first.
        pub fn types(self) -> i32 = XPLMGetDataRefTypes;
        fn can_write(self) -> i32 = XPLMCanWriteDataRef;
        fn is_good(self) -> i32 = XPLMIsDataRefGood;
    }

    /// `XPLMCanWriteDataRef(..) != 0`.
    ///
    /// # Panics
    /// If the handle is null; check the lookup first.
    pub fn writable(self) -> bool {
        self.can_write() != 0
    }

    /// `XPLMIsDataRefGood(..) != 0`.
    ///
    /// # Panics
    /// If the handle is null; check the lookup first.
    pub fn good(self) -> bool {
        self.is_good() != 0
    }

    dataref_calls! {
        value:
        /// `XPLMGetDatai`.
        pub fn get_i32(self) -> i32 = XPLMGetDatai;
        /// `XPLMGetDataf`.
        pub fn get_f32(self) -> f32 = XPLMGetDataf;
        /// `XPLMGetDatad`.
        pub fn get_f64(self) -> f64 = XPLMGetDatad;
        /// `XPLMSetDatai`.
        pub fn set_i32(self, value: i32) = XPLMSetDatai;
        /// `XPLMSetDataf`.
        pub fn set_f32(self, value: f32) = XPLMSetDataf;
        /// `XPLMSetDatad`.
        pub fn set_f64(self, value: f64) = XPLMSetDatad;
    }

    dataref_calls! {
        read:
        /// `XPLMGetDatavi(ref, out, offset, count)`: values copied. `count` is
        /// `out.len()` saturated at `i32::MAX` (module note).
        pub fn get_i32s(self, offset: i32, out: &mut [i32]) = XPLMGetDatavi;
        /// `XPLMGetDatavf(ref, out, offset, count)`: values copied. `count` is
        /// `out.len()` saturated at `i32::MAX` (module note).
        pub fn get_f32s(self, offset: i32, out: &mut [f32]) = XPLMGetDatavf;
        /// `XPLMGetDatab(ref, out, offset, count)`: bytes copied. `count` is
        /// `out.len()` saturated at `i32::MAX` (module note).
        pub fn get_bytes(self, offset: i32, out: &mut [u8]) = XPLMGetDatab;
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

    dataref_calls! {
        size:
        /// The array size query `XPLMGetDatavi(ref, null, offset, 0)`.
        pub fn count_i32s(self, offset: i32) = XPLMGetDatavi;
        /// The array size query `XPLMGetDatavf(ref, null, offset, 0)`.
        pub fn count_f32s(self, offset: i32) = XPLMGetDatavf;
        /// The size query `XPLMGetDatab(ref, null, offset, 0)`.
        pub fn count_bytes(self, offset: i32) = XPLMGetDatab;
    }

    dataref_calls! {
        write:
        /// `XPLMSetDatavi(ref, values, offset, count)`, `count` being
        /// `values.len()` saturated at `i32::MAX` (module note).
        pub fn set_i32s(self, offset: i32, values: &[i32]) = XPLMSetDatavi;
        /// `XPLMSetDatavf(ref, values, offset, count)`, `count` being
        /// `values.len()` saturated at `i32::MAX` (module note).
        pub fn set_f32s(self, offset: i32, values: &[f32]) = XPLMSetDatavf;
        /// `XPLMSetDatab(ref, values, offset, count)`, `count` being
        /// `values.len()` saturated at `i32::MAX` (module note).
        pub fn set_bytes(self, offset: i32, values: &[u8]) = XPLMSetDatab;
    }
}
