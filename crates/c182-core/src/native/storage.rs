//! Native-endian, bounds-checked access to fixed-size byte records.
//!
//! No alignment or pointer casts are required. Offsets and record schemas are
//! supplied by callers. Floating-point payload bits and untouched bytes survive.
//! These records are for in-process interop, not portable wire serialization.
#![forbid(unsafe_code)]
type Vec3 = [f64; 3];
/// Fixed byte storage. Accessors panic when their range is out of bounds.
#[repr(transparent)]
#[derive(Clone)]
pub struct Fields<const N: usize>(pub [u8; N]);
impl<const N: usize> Default for Fields<N> {
    fn default() -> Self {
        Self([0; N])
    }
}
impl<const N: usize> Fields<N> {
    pub fn read_f64(&self, o: usize) -> f64 {
        f64::from_ne_bytes(self.0[o..o + 8].try_into().unwrap())
    }
    pub fn write_f64(&mut self, o: usize, v: f64) {
        self.0[o..o + 8].copy_from_slice(&v.to_ne_bytes());
    }
    pub fn read_f32(&self, o: usize) -> f32 {
        f32::from_ne_bytes(self.0[o..o + 4].try_into().unwrap())
    }
    pub fn write_f32(&mut self, o: usize, v: f32) {
        self.0[o..o + 4].copy_from_slice(&v.to_ne_bytes());
    }
    pub fn read_u32(&self, o: usize) -> u32 {
        u32::from_ne_bytes(self.0[o..o + 4].try_into().unwrap())
    }
    pub fn write_u32(&mut self, o: usize, v: u32) {
        self.0[o..o + 4].copy_from_slice(&v.to_ne_bytes());
    }
    pub fn write_u64(&mut self, o: usize, v: u64) {
        self.0[o..o + 8].copy_from_slice(&v.to_ne_bytes());
    }
    pub fn read_vec3(&self, o: usize) -> Vec3 {
        std::array::from_fn(|n| self.read_f64(o + n * 8))
    }
    pub fn write_vec3(&mut self, o: usize, v: Vec3) {
        for (n, value) in v.into_iter().enumerate() {
            self.write_f64(o + n * 8, value);
        }
    }
}
