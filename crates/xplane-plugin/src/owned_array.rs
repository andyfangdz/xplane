use std::{cell::Cell, ffi::c_void, ptr::NonNull};
use xplane_sdk_sys::*;

/// Read-only snapshot whose callback never borrows a plugin's mutable runtime.
pub struct OwnedFloatArray<const N: usize> {
    handle: NonNull<c_void>,
    storage: Box<Cell<[f32; N]>>,
}
impl<const N: usize> OwnedFloatArray<N> {
    pub fn new(name: &str) -> Result<Self, String> {
        i32::try_from(N).map_err(|_| "array too large")?;
        let name = crate::c_string(name);
        let mut storage = Box::new(Cell::new([0.0; N]));
        let pointer = (&mut *storage as *mut Cell<[f32; N]>).cast();
        // SAFETY: stable Box storage outlives registration; drop unregisters
        // before storage is freed. Only the float-array getter is advertised.
        let handle = unsafe {
            XPLMRegisterDataAccessor(
                name.as_ptr(),
                xplmType_FloatArray,
                0,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                Some(read::<N>),
                None,
                None,
                None,
                pointer,
                pointer,
            )
        };
        Ok(Self {
            handle: NonNull::new(handle).ok_or("array registration failed")?,
            storage,
        })
    }
    pub fn set(&self, values: [f32; N]) {
        self.storage.set(values);
    }
}
impl<const N: usize> Drop for OwnedFloatArray<N> {
    fn drop(&mut self) {
        // SAFETY: uniquely owned registration, callback storage remains live.
        unsafe { XPLMUnregisterDataAccessor(self.handle.as_ptr()) };
    }
}
unsafe extern "C" fn read<const N: usize>(
    pointer: *mut c_void,
    output: *mut f32,
    offset: i32,
    count: i32,
) -> i32 {
    if output.is_null() {
        return N as i32;
    }
    if offset < 0 || count <= 0 || offset as usize >= N {
        return 0;
    }
    let offset = offset as usize;
    let count = (count as usize).min(N - offset);
    // SAFETY: SDK supplies live Box refcon and space for requested count.
    // Range is checked above, and the local copy cannot alias output.
    let values = unsafe { &*pointer.cast::<Cell<[f32; N]>>() }.get();
    unsafe { std::ptr::copy_nonoverlapping(values[offset..].as_ptr(), output, count) };
    count as i32
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn snapshot_reads_clip_ranges_and_do_not_overwrite_sentinels() {
        let mut values = Cell::new([1.0_f32, 2.0, 3.0]);
        let pointer = (&mut values as *mut Cell<[f32; 3]>).cast();
        let mut output = [-1.0; 5];
        // SAFETY: test owns both allocations for the entire callback call.
        unsafe {
            assert_eq!(read::<3>(pointer, std::ptr::null_mut(), 0, 0), 3);
            assert_eq!(read::<3>(pointer, output.as_mut_ptr(), -1, 2), 0);
            assert_eq!(read::<3>(pointer, output.as_mut_ptr(), 1, 5), 2);
            assert_eq!(read::<3>(pointer, output.as_mut_ptr(), 3, 1), 0);
        }
        assert_eq!(output, [2.0, 3.0, -1.0, -1.0, -1.0]);
    }
}
