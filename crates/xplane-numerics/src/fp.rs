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
// One boundary for scalar CRT math. Keep the function pointer opaque so LLVM
// cannot substitute constant-folded math with different rounding/status behavior.
macro_rules! runtime_math {
    ($( $(#[$doc:meta])* fn $name:ident($($arg:ident: $input:ty),+) -> $output:ty; )+) => {
        mod crt {
            unsafe extern "C" {
                $(pub(super) fn $name($($arg: $input),+) -> $output;)+
            }
        }
        $(
            $(#[$doc])*
            pub fn $name($($arg: $input),+) -> $output {
                let function: unsafe extern "C" fn($($input),+) -> $output = crt::$name;
                // SAFETY: These scalar CRT functions accept every floating-point
                // bit pattern and have no pointer preconditions.
                unsafe { std::hint::black_box(function)($($arg),+) }
            }
        )+
    };
}
runtime_math! {
    fn sin(value: f64) -> f64;
    fn cos(value: f64) -> f64;
    fn sqrt(value: f64) -> f64;
    fn acos(value: f64) -> f64;
    fn asin(value: f64) -> f64;
    fn asinf(value: f32) -> f32;
    fn cosf(value: f32) -> f32;
    fn sinf(value: f32) -> f32;
    fn tan(value: f64) -> f64;
    fn atanf(value: f32) -> f32;
    fn atan(value: f64) -> f64;
    fn floor(value: f64) -> f64;
    fn ceil(value: f64) -> f64;
    fn round(value: f64) -> f64;
    fn roundf(value: f32) -> f32;
    fn truncf(value: f32) -> f32;
    fn floorf(value: f32) -> f32;
    fn ceilf(value: f32) -> f32;
    fn log(value: f64) -> f64;
    fn log10(value: f64) -> f64;
    fn exp(value: f64) -> f64;
    /// Round through the platform C runtime to C's `long`, with halfway cases
    /// rounded away from zero. The result width and invalid-input behavior follow
    /// that runtime; `long` is 32 bits on Windows and usually 64 bits on Unix.
    fn lround(value: f64) -> std::ffi::c_long;
    fn pow(base: f64, exponent: f64) -> f64;
    fn powf(base: f32, exponent: f32) -> f32;
    fn fmod(value: f64, divisor: f64) -> f64;
    fn atan2(y: f64, x: f64) -> f64;
    fn atan2f(y: f32, x: f32) -> f32;
    fn hypot(x: f64, y: f64) -> f64;
    fn remainder(value: f64, divisor: f64) -> f64;
    fn nextafterf(value: f32, toward: f32) -> f32;
}
unsafe extern "C" {
    #[cfg_attr(all(target_os = "windows", target_env = "msvc"), link_name = "_hypotf")]
    #[cfg_attr(
        not(all(target_os = "windows", target_env = "msvc")),
        link_name = "hypotf"
    )]
    fn crt_hypotf(x: f32, y: f32) -> f32;
}
/// Binary32 norm through the platform CRT. MSVC's inline `hypotf` forwards to
/// its exported `_hypotf`; no binary64 widening or intermediate square is used.
pub fn hypotf(x: f32, y: f32) -> f32 {
    // SAFETY: CRT scalar math accepts every floating-point bit pattern.
    unsafe { std::hint::black_box(crt_hypotf as unsafe extern "C" fn(f32, f32) -> f32)(x, y) }
}
#[cfg(all(target_os = "windows", target_env = "msvc"))]
#[link(name = "msvcprt")]
unsafe extern "system" {
    fn __std_smf_hypot3(x: f64, y: f64, z: f64) -> f64;
}
/// Three-component norm through the actual MSVC runtime used by C++'s
/// three-argument `std::hypot`. Other platforms use two runtime `hypot` calls;
/// their rounding, exceptional-value and status behavior can differ from MSVC.
pub fn hypot3(x: f64, y: f64, z: f64) -> f64 {
    #[cfg(all(target_os = "windows", target_env = "msvc"))]
    // SAFETY: The vendor scalar routine accepts every floating-point bit pattern.
    unsafe {
        std::hint::black_box(__std_smf_hypot3 as unsafe extern "system" fn(f64, f64, f64) -> f64)(
            x, y, z,
        )
    }
    #[cfg(not(all(target_os = "windows", target_env = "msvc")))]
    {
        hypot(hypot(x, y), z)
    }
}
unsafe extern "C" {
    #[link_name = "modf"]
    fn crt_modf(value: f64, integral: *mut f64) -> f64;
}
pub fn modf(value: f64, integral: &mut f64) -> f64 {
    let function: unsafe extern "C" fn(f64, *mut f64) -> f64 = crt_modf;
    // SAFETY: The exclusive output reference remains valid for this call.
    unsafe { std::hint::black_box(function)(value, integral) }
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
/// Select the smaller scalar, returning the second operand for equal or unordered
/// inputs. On x86-64 issue MINSD, including its MXCSR denormal/status behavior.
/// Other architectures use an ordered comparison without that status guarantee.
pub fn min_second_f64(mut a: f64, b: f64) -> f64 {
    #[cfg(target_arch = "x86_64")]
    // SAFETY: SSE2 is baseline x86-64. Register operands only; the caller owns
    // exception masks, rounding and denormal handling.
    unsafe {
        std::arch::asm!("minsd {a}, {b}",a=inout(xmm_reg)a,b=in(xmm_reg)b,options(nomem,nostack,preserves_flags));
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        a = if a < b { a } else { b };
    }
    a
}
/// Select the larger scalar, returning the second operand for equal or unordered
/// inputs. On x86-64 issue MAXSD, including its MXCSR denormal/status behavior.
/// Other architectures use an ordered comparison without that status guarantee.
pub fn max_second_f64(mut a: f64, b: f64) -> f64 {
    #[cfg(target_arch = "x86_64")]
    // SAFETY: SSE2 is baseline x86-64. Register operands only; the caller owns
    // exception masks, rounding and denormal handling.
    unsafe {
        std::arch::asm!("maxsd {a}, {b}",a=inout(xmm_reg)a,b=in(xmm_reg)b,options(nomem,nostack,preserves_flags));
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        a = if a > b { a } else { b };
    }
    a
}
/// Ordered greater-than comparison. On x86-64 use COMISD: either kind of NaN
/// returns false and sets MXCSR invalid; subnormals follow the caller's DAZ mode.
/// Other architectures use `>` without a floating-point status guarantee.
pub fn ordered_gt_f64(a: f64, b: f64) -> bool {
    #[cfg(target_arch = "x86_64")]
    {
        let result: u8;
        // SAFETY: SSE2 is baseline x86-64. Register operands only. The caller
        // owns exception masks/denormal controls; this instruction changes flags.
        unsafe {
            std::arch::asm!("comisd {a}, {b}","seta {result}",a=in(xmm_reg)a,b=in(xmm_reg)b,result=lateout(reg_byte)result,options(nomem,nostack));
        }
        result != 0
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        a > b
    }
}
/// Ordered greater-or-equal comparison with the same COMISD status and DAZ
/// contract as [`ordered_gt_f64`]. Both signs of zero compare equal.
pub fn ordered_ge_f64(a: f64, b: f64) -> bool {
    #[cfg(target_arch = "x86_64")]
    {
        let result: u8;
        // SAFETY: SSE2 is baseline x86-64. Register operands only. The caller
        // owns exception masks/denormal controls; this instruction changes flags.
        unsafe {
            std::arch::asm!("comisd {a}, {b}","setae {result}",a=in(xmm_reg)a,b=in(xmm_reg)b,result=lateout(reg_byte)result,options(nomem,nostack));
        }
        result != 0
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        a >= b
    }
}
/// Ordered greater-than comparison. On x86-64 use COMISS: either kind of NaN
/// returns false and sets MXCSR invalid; subnormals follow the caller's DAZ mode.
/// Other architectures use `>` without a floating-point status guarantee.
pub fn ordered_gt_f32(a: f32, b: f32) -> bool {
    #[cfg(target_arch = "x86_64")]
    {
        let result: u8;
        // SAFETY: SSE2 is baseline x86-64. Register operands only. The caller
        // owns exception masks/denormal controls; this instruction changes flags.
        unsafe {
            std::arch::asm!("comiss {a}, {b}","seta {result}",a=in(xmm_reg)a,b=in(xmm_reg)b,result=lateout(reg_byte)result,options(nomem,nostack));
        }
        result != 0
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        a > b
    }
}
/// Ordered greater-or-equal comparison with the same COMISS status and DAZ
/// contract as [`ordered_gt_f32`]. Both signs of zero compare equal.
pub fn ordered_ge_f32(a: f32, b: f32) -> bool {
    #[cfg(target_arch = "x86_64")]
    {
        let result: u8;
        // SAFETY: SSE2 is baseline x86-64. Register operands only. The caller
        // owns exception masks/denormal controls; this instruction changes flags.
        unsafe {
            std::arch::asm!("comiss {a}, {b}","setae {result}",a=in(xmm_reg)a,b=in(xmm_reg)b,result=lateout(reg_byte)result,options(nomem,nostack));
        }
        result != 0
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        a >= b
    }
}
/// Select the larger scalar, returning the second operand for equal or unordered
/// inputs. On x86-64 issue MAXSS, including its MXCSR denormal/status behavior.
/// Other architectures use an ordered comparison without that status guarantee.
pub fn max_second_f32(mut a: f32, b: f32) -> f32 {
    #[cfg(target_arch = "x86_64")]
    // SAFETY: SSE is baseline x86-64. Register operands only; the caller owns
    // exception masks, rounding and denormal handling.
    unsafe {
        std::arch::asm!("maxss {a}, {b}",a=inout(xmm_reg)a,b=in(xmm_reg)b,options(nomem,nostack,preserves_flags));
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        a = if a > b { a } else { b };
    }
    a
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

macro_rules! packed {
    ($name:ident, $operation:tt, $instruction:literal) => {
        /// Four binary32 lanes, evaluated at runtime on x86-64. The first
        /// argument is the SSE destination, preserving operand-dependent NaN
        /// selection, denormal controls and accumulated status from every lane.
        /// Other architectures use ordinary arithmetic without that guarantee.
        #[inline]
        pub fn $name(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
            #[cfg(target_arch = "x86_64")]
            // SAFETY: __m128 and four f32 lanes have identical size and bit
            // representation; SSE is baseline x86-64. Register operands only.
            unsafe {
                let mut destination: std::arch::x86_64::__m128 = std::mem::transmute(a);
                let source: std::arch::x86_64::__m128 = std::mem::transmute(b);
                std::arch::asm!($instruction,
                    a = inout(xmm_reg) destination, b = in(xmm_reg) source,
                    options(nomem, nostack, preserves_flags));
                std::mem::transmute(destination)
            }
            #[cfg(not(target_arch = "x86_64"))]
            { std::array::from_fn(|lane| a[lane] $operation b[lane]) }
        }
    };
}
packed!(add4_f32, +, "addps {a}, {b}");
packed!(mul4_f32, *, "mulps {a}, {b}");

/// Widen binary32 at runtime using CVTSS2SD on x86-64.
/// Finite results are exact. The caller's DAZ mode can replace a subnormal
/// input with signed zero; otherwise it sets denormal-operand status. Signaling
/// NaNs are quieted and set invalid status. Other architectures use an ordinary
/// Rust cast without a floating-point status or denormal-control guarantee.
#[inline]
pub fn widen_f64(value: f32) -> f64 {
    #[cfg(target_arch = "x86_64")]
    {
        let out: f64;
        // SAFETY: SSE2 is baseline x86-64. Register operands only; the caller
        // owns exception masks and denormal handling.
        unsafe {
            std::arch::asm!("cvtss2sd {out}, {value}",out=out(xmm_reg)out,value=in(xmm_reg)value,options(nomem,nostack,preserves_flags));
        }
        out
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        f64::from(value)
    }
}

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

/// Truncate binary32 directly to signed32. On x86-64 use CVTTSS2SI, including
/// invalid/inexact status without an intervening binary64 conversion.
/// Invalid inputs return `i32::MIN`. Other targets make no status guarantee.
pub fn truncate_f32_i32(value: f32) -> i32 {
    #[cfg(target_arch = "x86_64")]
    {
        let out: i32;
        // SAFETY: SSE is baseline x86-64. Registers only; caller owns exception masks.
        unsafe {
            std::arch::asm!("cvttss2si {out:e}, {value}",out=out(reg)out,value=in(xmm_reg)value,options(nomem,nostack,preserves_flags));
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
/// Truncate binary32 directly to signed64 using CVTTSS2SI on x86-64.
/// Invalid inputs return `i64::MIN`; other targets make no status guarantee.
pub fn truncate_f32_i64(value: f32) -> i64 {
    #[cfg(target_arch = "x86_64")]
    {
        let out: i64;
        // SAFETY: SSE is baseline x86-64. Registers only; caller owns exception masks.
        unsafe {
            std::arch::asm!("cvttss2si {out}, {value}",out=out(reg)out,value=in(xmm_reg)value,options(nomem,nostack,preserves_flags));
        }
        out
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        if value >= -9223372036854775808.0 && value < 9223372036854775808.0 {
            value as i64
        } else {
            i64::MIN
        }
    }
}
