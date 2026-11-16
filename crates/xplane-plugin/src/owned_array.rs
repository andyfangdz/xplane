use crate::callbacks::{ArrayRead, DataAccess, DataAccessor, Writes, copy_array};
use std::{cell::Cell, rc::Rc};
use xplane_sdk_sys::*;

/// Read-only snapshot whose callback never borrows a plugin's mutable runtime.
pub struct OwnedFloatArray<const N: usize> {
    // Field order: the accessor is unregistered before this owner's share of
    // the storage goes (the accessor's callback holds the other share).
    _accessor: DataAccessor,
    storage: Rc<Cell<[f32; N]>>,
}
impl<const N: usize> OwnedFloatArray<N> {
    pub fn new(name: &str) -> Result<Self, String> {
        i32::try_from(N).map_err(|_| "array too large")?;
        let name = crate::c_string(name);
        let storage = Rc::new(Cell::new([0.0; N]));
        // Only the float-array getter is advertised (read-only accessor).
        let access = DataAccess {
            floats: Some(read(&storage)),
            ..DataAccess::default()
        };
        let accessor = DataAccessor::register_shaped(
            &name,
            xplmType_FloatArray,
            false,
            "array read",
            "array write",
            Writes::Storage,
            access,
        )
        .ok_or("array registration failed")?;
        Ok(Self {
            _accessor: accessor,
            storage,
        })
    }
    pub fn set(&self, values: [f32; N]) {
        self.storage.set(values);
    }
}

/// The float-array getter over `storage`: the length for a null output,
/// otherwise the values from `offset`, clipped to the output and the array.
fn read<const N: usize>(storage: &Rc<Cell<[f32; N]>>) -> ArrayRead<f32> {
    let storage = storage.clone();
    Box::new(move |out, offset| copy_array(&storage.get(), out, offset))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn snapshot_reads_clip_ranges_and_do_not_overwrite_sentinels() {
        let values = Rc::new(Cell::new([1.0_f32, 2.0, 3.0]));
        let read = read(&values);
        let mut output = [-1.0; 5];
        assert_eq!(read(None, 0), 3);
        assert_eq!(read(Some(&mut output[..2]), -1), 0);
        assert_eq!(read(Some(&mut output[..5]), 1), 2);
        assert_eq!(read(Some(&mut output[..1]), 3), 0);
        assert_eq!(read(Some(&mut output[..0]), 0), 0);
        assert_eq!(output, [2.0, 3.0, -1.0, -1.0, -1.0]);
    }
}
