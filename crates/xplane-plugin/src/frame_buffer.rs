/// Fixed-capacity storage allocated before a measured interval. Push never grows
/// the allocation; callers must reject overflow rather than silently lose frames.
pub struct FrameBuffer<T> {
    frames: Vec<T>,
    limit: usize,
}
impl<T> FrameBuffer<T> {
    pub fn new(limit: usize) -> Self {
        let mut frames: Vec<T> = Vec::with_capacity(limit);
        let bytes = frames
            .capacity()
            .checked_mul(std::mem::size_of::<T>())
            .expect("buffer size overflow");
        // Commit each Windows memory page before measurement. These bytes are
        // spare capacity, not initialized T values; length stays zero, so Rust
        // never reads or drops a partially initialized value.
        for offset in (0..bytes).step_by(4096) {
            unsafe {
                frames
                    .as_mut_ptr()
                    .cast::<u8>()
                    .add(offset)
                    .write_volatile(0)
            };
        }
        if bytes > 0 {
            // Allocations need not begin at a page boundary.
            unsafe {
                frames
                    .as_mut_ptr()
                    .cast::<u8>()
                    .add(bytes - 1)
                    .write_volatile(0)
            };
        }
        Self { frames, limit }
    }
    pub fn push(&mut self, frame: T) -> Result<(), T> {
        if self.frames.len() == self.limit {
            Err(frame)
        } else {
            self.frames.push(frame);
            Ok(())
        }
    }
    pub fn frames(&self) -> &[T] {
        &self.frames
    }
    pub fn len(&self) -> usize {
        self.frames.len()
    }
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_push_preserves_all_frames_without_reallocation() {
        let mut b = FrameBuffer::new(2);
        let allocation = b.frames.as_ptr();
        assert!(b.push(10).is_ok());
        assert!(b.push(20).is_ok());
        assert_eq!(b.push(30), Err(30));
        assert_eq!(b.frames(), &[10, 20]);
        assert_eq!(allocation, b.frames.as_ptr());
    }
}
