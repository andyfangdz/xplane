//! Scalar SSE operations preserve the native caller's MXCSR rounding mode.
//! Rust/LLVM may otherwise fold literal expressions assuming round-to-nearest.
use std::ops::{Add, Div, Mul, Sub};

#[derive(Clone, Copy)]
pub(crate) struct Strict(pub f64);
pub(crate) fn f(value: f64) -> Strict {
    Strict(value)
}

macro_rules! operation {
    ($trait:ident, $method:ident) => {
        impl $trait for Strict {
            type Output = Self;
            #[inline]
            fn $method(self, rhs: Self) -> Self {
                Self(crate::native::fp::$method(self.0, rhs.0))
            }
        }
        impl $trait<f64> for Strict {
            type Output = Self;
            #[inline]
            fn $method(self, rhs: f64) -> Self {
                self.$method(Strict(rhs))
            }
        }
        impl $trait<Strict> for f64 {
            type Output = Strict;
            #[inline]
            fn $method(self, rhs: Strict) -> Strict {
                Strict(self).$method(rhs)
            }
        }
    };
}
operation!(Add, add);
operation!(Sub, sub);
operation!(Mul, mul);
operation!(Div, div);
