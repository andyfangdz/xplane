//! Checked x86-64 floating-point environment access.
//!
//! Scopes restore the entire MXCSR word, including sticky exception flags, on
//! their creating thread. They do not change x87 state. Exception masks are
//! caller policy: a supported word with unmasked exceptions is accepted.
use std::{marker::PhantomData, rc::Rc, sync::OnceLock};

/// Read this thread's SSE control and status word without changing it.
#[inline]
pub fn mxcsr() -> u32 {
    let mut value = 0u32;
    // SAFETY: SSE2 is baseline x86-64. STMXCSR writes exactly the live u32.
    unsafe {
        std::arch::asm!("stmxcsr [{p}]", p = in(reg) &mut value, options(nostack, preserves_flags));
    }
    value
}

/// Read this thread's x87 control word without changing control or status.
#[inline]
pub fn x87_control_word() -> u16 {
    let mut value = 0u16;
    // SAFETY: FNSTCW writes exactly the live u16 and does not alter x87 state.
    unsafe {
        std::arch::asm!("fnstcw [{p}]", p = in(reg) &mut value, options(nostack, preserves_flags));
    }
    value
}

fn writable_mask() -> u32 {
    static MASK: OnceLock<u32> = OnceLock::new();
    *MASK.get_or_init(|| {
        #[repr(C, align(16))]
        struct Save([u8; 512]);
        let mut save = Save([0; 512]);
        // SAFETY: FXSAVE is baseline x86-64. The zero-initialized output is
        // writable for 512 bytes and aligned to 16 bytes. No FP state is changed.
        unsafe {
            std::arch::asm!("fxsave64 [{p}]", p = in(reg) save.0.as_mut_ptr(), options(nostack, preserves_flags));
        }
        // Intel SDM vol. 1, section 11.6.6: MXCSR_MASK is at byte 28; zero
        // means the architectural fallback 0xffbf, which excludes optional DAZ.
        let mask = u32::from_le_bytes(save.0[28..32].try_into().unwrap());
        if mask == 0 { 0xffbf } else { mask }
    })
}

/// Whether a legacy 16-bit word can be loaded on this processor, including DAZ.
/// High bits and unsupported features are rejected rather than silently removed.
#[inline]
pub fn valid_mxcsr(mode: u32) -> bool {
    mode & !0xffff == 0 && mode & !writable_mask() == 0
}

// Only checked inputs and values read from the processor reach this leaf.
#[inline]
fn load(mode: u32) {
    // SAFETY: callers validate against MXCSR_MASK or restore an STMXCSR result.
    // The live u32 has no reserved bits set for this processor.
    unsafe {
        std::arch::asm!("ldmxcsr [{p}]", p = in(reg) &mode, options(nostack, preserves_flags));
    }
}

/// Restore the complete previous MXCSR word on scope exit, including unwinding.
/// The guard cannot move to or be shared with another thread.
///
/// ```compile_fail
/// use xplane_numerics::environment::MxcsrGuard;
/// let guard = MxcsrGuard::new(0x1f80);
/// std::thread::spawn(move || drop(guard));
/// ```
///
/// ```compile_fail
/// use xplane_numerics::environment::MxcsrGuard;
/// fn requires_sync<T: Sync>() {}
/// requires_sync::<MxcsrGuard>();
/// ```
#[must_use = "dropping the guard immediately restores the previous environment"]
pub struct MxcsrGuard {
    saved: u32,
    _thread: PhantomData<Rc<()>>,
}
impl MxcsrGuard {
    /// Enter a supported word, or return `None` without changing FP state.
    #[inline]
    pub fn try_new(mode: u32) -> Option<Self> {
        if !valid_mxcsr(mode) {
            return None;
        }
        let saved = mxcsr();
        load(mode);
        Some(Self {
            saved,
            _thread: PhantomData,
        })
    }

    /// Enter a supported word.
    ///
    /// # Panics
    /// Panics before changing FP state if the processor does not support `mode`.
    #[inline]
    pub fn new(mode: u32) -> Self {
        Self::try_new(mode).expect("unsupported MXCSR bits")
    }
}
impl Drop for MxcsrGuard {
    #[inline]
    fn drop(&mut self) {
        load(self.saved);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restores_nested_rounding_denormal_controls_and_all_status_bits() {
        let original = mxcsr();
        for rounding in [0, 0x2000, 0x4000, 0x6000] {
            for denormals in [0, 0x40, 0x8000, 0x8040] {
                for flags in [0, 1, 0x20, 0x3f] {
                    let mode = 0x1f80 | rounding | denormals | flags;
                    let Some(outer) = MxcsrGuard::try_new(mode) else {
                        assert_eq!(denormals & 0x40, 0x40);
                        continue;
                    };
                    assert_eq!(mxcsr(), mode);
                    {
                        let _inner = MxcsrGuard::new(0x1f80);
                        assert_eq!(mxcsr(), 0x1f80);
                    }
                    assert_eq!(mxcsr(), mode);
                    drop(outer);
                    assert_eq!(mxcsr(), original);
                }
            }
        }
    }

    #[test]
    fn rejects_unsupported_words_without_loading_them() {
        let original = mxcsr();
        for bit in 0..32 {
            let mode = 0x1f80 | (1 << bit);
            let guard = MxcsrGuard::try_new(mode);
            assert_eq!(
                guard.is_some(),
                mode & !0xffff == 0 && mode & !writable_mask() == 0,
                "mode {mode:08x}, processor mask {:08x}",
                writable_mask()
            );
            drop(guard);
            assert_eq!(mxcsr(), original);
        }
        assert!(MxcsrGuard::try_new(u32::MAX).is_none());
        assert_eq!(mxcsr(), original);
    }

    #[test]
    fn restores_on_unwind_and_leaves_x87_control_unchanged() {
        let _caller = MxcsrGuard::new(0x5fa5);
        let control = x87_control_word();
        let result = std::panic::catch_unwind(|| {
            let _scope = MxcsrGuard::new(0x1f80);
            assert_eq!(x87_control_word(), control);
            panic!("scope unwind");
        });
        assert!(result.is_err());
        assert_eq!(mxcsr(), 0x5fa5);
        assert_eq!(x87_control_word(), control);
        assert!(std::panic::catch_unwind(|| MxcsrGuard::new(u32::MAX)).is_err());
        assert_eq!(mxcsr(), 0x5fa5);
    }
}
