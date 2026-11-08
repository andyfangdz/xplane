//! Scalar SSE operations preserve the native caller's MXCSR rounding mode.
//! Rust/LLVM may otherwise fold literal expressions assuming round-to-nearest.
use std::ops::{Add, Div, Mul, Sub};

#[derive(Clone, Copy)]
pub(crate) struct Strict(pub f64);
pub(crate) fn f(value: f64) -> Strict {
    Strict(value)
}

macro_rules! operation {
    ($trait:ident, $method:ident, $instruction:literal) => {
        impl $trait for Strict {
            type Output = Self;
            #[inline]
            fn $method(self, rhs: Self) -> Self {
                #[cfg(target_arch = "x86_64")]
                {
                    let mut value = self.0;
                    // Not `pure`: the operation observes the caller's FP environment.
                    unsafe { std::arch::asm!($instruction, value = inout(xmm_reg) value, rhs = in(xmm_reg) rhs.0, options(nomem, nostack, preserves_flags)); }
                    Self(value)
                }
                #[cfg(not(target_arch = "x86_64"))]
                { Self(self.0.$method(rhs.0)) }
            }
        }
        impl $trait<f64> for Strict {
            type Output = Self;
            #[inline] fn $method(self, rhs: f64) -> Self { self.$method(Strict(rhs)) }
        }
        impl $trait<Strict> for f64 {
            type Output = Strict;
            #[inline] fn $method(self, rhs: Strict) -> Strict { Strict(self).$method(rhs) }
        }
    };
}
operation!(Add, add, "addsd {value}, {rhs}");
operation!(Sub, sub, "subsd {value}, {rhs}");
operation!(Mul, mul, "mulsd {value}, {rhs}");
operation!(Div, div, "divsd {value}, {rhs}");
