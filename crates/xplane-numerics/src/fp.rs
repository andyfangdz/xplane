//! Runtime scalar arithmetic and explicit operand selection for native interop.
//!
//! On x86-64, add/subtract/multiply/divide issue scalar SSE2 instructions. This
//! prevents constant folding and observes the caller's MXCSR rounding mode.
//! The caller owns the floating-point environment, including exception masks.
//! Other architectures use ordinary Rust arithmetic, with no dynamic-rounding
//! guarantee. C math functions use the platform C runtime, so transcendental
//! results are platform dependent. Functions preserve rounding and exception
//! control settings; arithmetic may set floating-point status flags.

#[cfg(unix)]
#[link(name = "m")]
unsafe extern "C" {}
macro_rules! unary {
    ($name:ident,$foreign:ident,$number:ty) => {
        unsafe extern "C" {
            #[link_name=stringify!($name)]
            fn $foreign(value: $number) -> $number;
        }
        pub fn $name(value: $number) -> $number {
            // SAFETY: CRT scalar math accepts every floating-point bit pattern.
            unsafe { $foreign(value) }
        }
    };
}
unary!(sin, crt_sin, f64);
unary!(cos, crt_cos, f64);
unary!(sqrt, crt_sqrt, f64);
unary!(acos, crt_acos, f64);
unary!(asinf, crt_asinf, f32);
unary!(cosf, crt_cosf, f32);
unary!(sinf, crt_sinf, f32);
unary!(tan, crt_tan, f64);
unary!(atanf, crt_atanf, f32);
unary!(ceil, crt_ceil, f64);
macro_rules! binary {
    ($name:ident,$foreign:ident,$number:ty) => {
        unsafe extern "C" {
            #[link_name=stringify!($name)]
            fn $foreign(base: $number, exponent: $number) -> $number;
        }
        pub fn $name(base: $number, exponent: $number) -> $number {
            // Keep this a runtime CRT call even for constant arguments, rather
            // than allowing an algebraic replacement under assumed rounding.
            let function: unsafe extern "C" fn($number, $number) -> $number = $foreign;
            // SAFETY: CRT scalar math has no pointer preconditions.
            unsafe { std::hint::black_box(function)(base, exponent) }
        }
    };
}
binary!(pow, crt_pow, f64);
binary!(powf, crt_powf, f32);
binary!(fmod, crt_fmod, f64);
unsafe extern "C" {
    #[link_name = "modf"]
    fn crt_modf(value: f64, integral: *mut f64) -> f64;
    #[link_name = "atan2"]
    fn crt_atan2(y: f64, x: f64) -> f64;
}
pub fn modf(value: f64, integral: &mut f64) -> f64 {
    // SAFETY: The exclusive output reference remains valid for this call.
    unsafe { crt_modf(value, integral) }
}
pub fn atan2(y: f64, x: f64) -> f64 {
    // SAFETY: The CRT routine has no pointer preconditions.
    unsafe { crt_atan2(y, x) }
}
/// Truncate toward zero. NaNs and out-of-range inputs return `i64::MIN`.
/// On x86-64 this uses CVTTSD2SI, independent of the rounding direction.
pub fn truncate_i64(value: f64) -> i64 {
    #[cfg(not(target_arch = "x86_64"))]
    {
        return if value >= -9223372036854775808.0 && value < 9223372036854775808.0 {
            value as i64
        } else {
            i64::MIN
        };
    }
    #[cfg(target_arch = "x86_64")]
    {
        let out: i64;
        // SAFETY: Scalar CVTTSD2SI has no memory operands; exceptions are masked by
        // the owning host. Invalid inputs return the integer-indefinite value, i64::MIN.
        unsafe {
            std::arch::asm!("cvttsd2si {out}, {value}",out=out(reg)out,value=in(xmm_reg)value,options(nomem,nostack,preserves_flags));
        }
        out
    }
}

/// Truncate toward zero to 32 bits. Invalid inputs return `i32::MIN`.
/// On x86-64 this uses the 32-bit CVTTSD2SI form, independent of rounding.
pub fn truncate_i32(value: f64) -> i32 {
    #[cfg(target_arch = "x86_64")]
    {
        let out: i32;
        // SAFETY: SSE2 is baseline x86-64. Register operands only; the host
        // owns exception masks. Invalid conversion yields integer-indefinite.
        unsafe {
            std::arch::asm!("cvttsd2si {out:e}, {value}",out=out(reg)out,value=in(xmm_reg)value,options(nomem,nostack,preserves_flags));
        }
        out
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        if value >= -2147483648.0 && value < 2147483648.0 {
            value as i32
        } else {
            i32::MIN
        }
    }
}

/// Convert a signed 64-bit integer using the caller's MXCSR direction on x86-64.
/// Other architectures use the ordinary Rust cast.
pub fn from_i64(value: i64) -> f64 {
    #[cfg(target_arch = "x86_64")]
    {
        let out: f64;
        // SAFETY: SSE2 is baseline x86-64. CVTSI2SD uses register operands;
        // the host owns exception masks and rounding control.
        unsafe {
            std::arch::asm!("cvtsi2sd {out}, {value}",out=out(xmm_reg)out,value=in(reg)value,options(nomem,nostack,preserves_flags));
        }
        out
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        value as f64
    }
}

/// MINSD selection: return the second operand for equal or unordered inputs.
pub fn min_second(a: f64, b: f64) -> f64 {
    if a < b { a } else { b }
}
/// MAXSD selection: return the second operand for equal or unordered inputs.
pub fn max_second(a: f64, b: f64) -> f64 {
    if a > b { a } else { b }
}
/// C++ std::min selection: return the first operand for equal or unordered inputs.
pub fn min_first(a: f64, b: f64) -> f64 {
    if b < a { b } else { a }
}
/// C++ std::max selection: return the first operand for equal or unordered inputs.
pub fn max_first(a: f64, b: f64) -> f64 {
    if a < b { b } else { a }
}
/// Bounds-first clamp: `min_first(high, max_first(low, value))`.
/// Bounds are not validated. NaN and signed-zero selection follows operand order.
pub fn clamp_bounds_first(value: f64, low: f64, high: f64) -> f64 {
    min_first(high, max_first(low, value))
}
// Issue each arithmetic operation at runtime under the caller's MXCSR.
macro_rules! scalar {
    ($name:ident,$operation:tt,$instruction:literal,$number:ty) => {
        #[inline]
        pub fn $name(mut a:$number,b:$number)->$number {
            // SAFETY: SSE2 is available on x86-64; registers only, no memory.
            #[cfg(target_arch="x86_64")]
            unsafe {std::arch::asm!($instruction,a=inout(xmm_reg) a,b=in(xmm_reg) b,options(nomem,nostack,preserves_flags));}
            #[cfg(not(target_arch="x86_64"))]
            {a=a $operation b;}
            a
        }
    };
}
scalar!(add,+,"addsd {a}, {b}",f64);
scalar!(sub,-,"subsd {a}, {b}",f64);
scalar!(mul,*,"mulsd {a}, {b}",f64);
scalar!(div,/,"divsd {a}, {b}",f64);
scalar!(add_f32,+,"addss {a}, {b}",f32);
scalar!(sub_f32,-,"subss {a}, {b}",f32);
scalar!(mul_f32,*,"mulss {a}, {b}",f32);
scalar!(div_f32,/,"divss {a}, {b}",f32);

/// Narrow at runtime using the caller's MXCSR direction on x86-64.
/// Other architectures use the ordinary Rust cast, without that guarantee.
#[inline]
pub fn narrow_f32(value: f64) -> f32 {
    #[cfg(target_arch = "x86_64")]
    {
        let out: f32;
        // SAFETY: SSE2 is baseline x86-64. Register operands only; the host owns
        // exception masks and the floating-point environment.
        unsafe {
            std::arch::asm!("cvtsd2ss {out}, {value}",out=out(xmm_reg) out,value=in(xmm_reg)value,options(nomem,nostack,preserves_flags));
        }
        out
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        value as f32
    }
}
