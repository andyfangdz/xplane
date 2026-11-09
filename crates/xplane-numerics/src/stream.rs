//! A byte cursor matching formatted MSVC stream extraction in the classic locale.
//!
//! This module is available on Windows with the MSVC runtime. Integers use the
//! default decimal base. Floating fields use the stream grammar, rather than
//! accepting every spelling understood by `strtod`: hexadecimal fields are
//! supported, while `inf` and `nan` are not. Conversion uses the actual C-locale
//! runtime and observes the caller's floating-point environment.
//!
//! Failed extraction is sticky. Reaching the end after a successful extraction
//! sets only the end flag; a subsequent extraction then also sets failure. This
//! is useful when translating record parsers whose last optional field depends
//! on the distinction between stream exhaustion and failed conversion.

#![cfg(all(windows, target_env = "msvc"))]

use std::{
    ffi::{c_char, c_void},
    ops::{Deref, DerefMut},
    sync::OnceLock,
};

struct Field<const N: usize> {
    bytes: [u8; N],
    used: usize,
}
impl<const N: usize> Field<N> {
    fn new() -> Self {
        Self {
            bytes: [0; N],
            used: 0,
        }
    }
    fn push(&mut self, byte: u8) {
        self.bytes[self.used] = byte;
        self.used += 1;
    }
}
impl<const N: usize> Deref for Field<N> {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        &self.bytes[..self.used]
    }
}
impl<const N: usize> DerefMut for Field<N> {
    fn deref_mut(&mut self) -> &mut [u8] {
        &mut self.bytes[..self.used]
    }
}

/// A classic-locale, decimal-default formatted input stream over borrowed bytes.
#[derive(Debug)]
pub struct Cursor<'a> {
    bytes: &'a [u8],
    position: usize,
    failed: bool,
    eof: bool,
}

impl<'a> Cursor<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            position: 0,
            failed: false,
            eof: false,
        }
    }

    /// Physical input position, including characters consumed by a failed field.
    pub fn position(&self) -> usize {
        self.position
    }
    pub fn failed(&self) -> bool {
        self.failed
    }
    pub fn eof(&self) -> bool {
        self.eof
    }

    fn skip_space(&mut self) {
        while self.bytes.get(self.position).is_some_and(|b| space(*b)) {
            self.position += 1;
        }
    }

    fn sentry(&mut self) -> bool {
        if self.failed || self.eof {
            self.failed = true;
            return false;
        }
        self.skip_space();
        if self.position == self.bytes.len() {
            self.failed = true;
            self.eof = true;
            return false;
        }
        true
    }

    /// Extract a signed 32-bit decimal integer. Overflow consumes the complete
    /// digit field, sets runtime `errno` to `ERANGE`, and fails the stream.
    pub fn integer(&mut self) -> Option<i32> {
        if !self.sentry() {
            return None;
        }
        let sign = self.take_sign();
        let mut field = Field::<33>::new();
        if let Some(sign) = sign {
            field.push(sign);
        }
        let limit = 31;
        let mut seen = false;
        let mut significant = false;
        while let Some(&b) = self.bytes.get(self.position).filter(|b| b.is_ascii_digit()) {
            self.position += 1;
            seen = true;
            if significant || b != b'0' {
                significant = true;
                if field.len() < limit {
                    field.push(b);
                }
            }
        }
        self.eof = self.position == self.bytes.len();
        if !seen {
            self.failed = true;
            return None;
        }
        if !significant {
            field.push(b'0');
        }
        field.push(0);
        let (value, error) = integer_conversion(&field);
        self.failed = error;
        (!error).then_some(value)
    }

    /// Extract a decimal or hexadecimal floating field. A malformed exponent is
    /// consumed and fails without invoking the numeric converter. Stream-style
    /// significant-digit reduction precedes the actual CRT conversion.
    pub fn number(&mut self) -> Option<f64> {
        if !self.sentry() {
            return None;
        }
        let mut field = Field::<800>::new();
        if let Some(sign) = self.take_sign() {
            field.push(sign);
        }
        // A leading carry digit also represents an entirely zero mantissa.
        field.push(b'0');
        let mut hex = false;
        let mut seen = false;
        if self.peek() == Some(b'0') {
            self.position += 1;
            if matches!(self.peek(), Some(b'x' | b'X')) {
                hex = true;
                self.position += 1;
                field.push(b'x');
            } else {
                seen = true;
            }
        }
        let mut significant = 0;
        let mut scale: i128 = 0;
        let mut sticky = false;
        while let Some(b) = self.peek().filter(|b| digit(*b, hex)) {
            seen = true;
            self.position += 1;
            if significant == 768 {
                scale += 1;
                sticky |= b != b'0';
            } else if significant != 0 || b != b'0' {
                field.push(b);
                significant += 1;
            }
        }
        if self.peek() == Some(b'.') {
            field.push(b'.');
            self.position += 1;
        }
        if significant == 0 {
            while self.peek() == Some(b'0') {
                seen = true;
                self.position += 1;
                scale -= 1;
            }
        }
        while let Some(b) = self.peek().filter(|b| digit(*b, hex)) {
            seen = true;
            self.position += 1;
            if significant < 768 {
                field.push(b);
                significant += 1;
            } else {
                sticky |= b != b'0';
            }
        }
        if sticky {
            let index = field.len() - if field.last() == Some(&b'.') { 2 } else { 1 };
            if field[index] == b'0' || field[index] == if hex { b'8' } else { b'5' } {
                field[index] += 1;
            }
        }
        let mut exponent = 0i64;
        if seen
            && self.peek().is_some_and(|b| {
                if hex {
                    matches!(b, b'p' | b'P')
                } else {
                    matches!(b, b'e' | b'E')
                }
            })
        {
            self.position += 1;
            seen = false;
            let negative = self.take_sign() == Some(b'-');
            while let Some(b) = self.peek().filter(u8::is_ascii_digit) {
                seen = true;
                self.position += 1;
                exponent = exponent
                    .saturating_mul(10)
                    .saturating_add(i64::from(b - b'0'));
            }
            if negative {
                exponent = -exponent;
            }
        }
        self.eof = self.position == self.bytes.len();
        if !seen {
            self.failed = true;
            return None;
        }
        let bound = if hex { 4200 } else { 1100 };
        let exponent =
            (i128::from(exponent) + scale * if hex { 4 } else { 1 }).clamp(-bound, bound);
        if exponent != 0 {
            field.push(if hex { b'p' } else { b'e' });
            if exponent < 0 {
                field.push(b'-');
            }
            let mut value = exponent.unsigned_abs() as u32;
            let mut digits = [0; 4];
            let mut count = 0;
            while value != 0 {
                digits[count] = b'0' + (value % 10) as u8;
                count += 1;
                value /= 10;
            }
            for digit in digits[..count].iter().rev() {
                field.push(*digit);
            }
        }
        field.push(0);
        let (value, error) = float_conversion(&field);
        self.failed = error;
        (!error).then_some(value)
    }

    /// Extract bytes up to classic whitespace, without consuming the delimiter.
    pub fn word(&mut self) -> Option<&'a [u8]> {
        if !self.sentry() {
            return None;
        }
        let start = self.position;
        while self.bytes.get(self.position).is_some_and(|b| !space(*b)) {
            self.position += 1;
        }
        self.eof = self.position == self.bytes.len();
        Some(&self.bytes[start..self.position])
    }

    /// Perform `std::ws`, then `getline`: return the remainder through the next
    /// line delimiter. The delimiter is consumed and omitted; a final CR is
    /// preserved. Failure returns an empty slice and remains sticky.
    pub fn rest(&mut self) -> &'a [u8] {
        if !self.sentry() {
            return &self.bytes[self.position..self.position];
        }
        let start = self.position;
        if let Some(length) = self.bytes[start..].iter().position(|b| *b == b'\n') {
            self.position += length + 1;
            &self.bytes[start..start + length]
        } else {
            self.position = self.bytes.len();
            self.eof = true;
            &self.bytes[start..]
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.position).copied()
    }
    fn take_sign(&mut self) -> Option<u8> {
        let b = self.peek().filter(|b| matches!(b, b'+' | b'-'))?;
        self.position += 1;
        Some(b)
    }
}

fn space(b: u8) -> bool {
    b == b' ' || (b'\t'..=b'\r').contains(&b)
}
fn digit(b: u8, hex: bool) -> bool {
    if hex {
        b.is_ascii_hexdigit()
    } else {
        b.is_ascii_digit()
    }
}

unsafe extern "C" {
    fn _errno() -> *mut i32;
    fn _create_locale(category: i32, name: *const c_char) -> *mut c_void;
    fn _strtod_l(text: *const c_char, end: *mut *mut c_char, locale: *mut c_void) -> f64;
    fn _strtol_l(text: *const c_char, end: *mut *mut c_char, base: i32, locale: *mut c_void)
    -> i32;
}
fn locale() -> *mut c_void {
    // One immutable process-lifetime CRT locale can be shared by conversions.
    static C: OnceLock<usize> = OnceLock::new();
    *C.get_or_init(|| {
        // SAFETY: LC_NUMERIC is 4 in the MSVC CRT; the name is terminated.
        let locale = unsafe { _create_locale(4, c"C".as_ptr()) };
        assert!(!locale.is_null(), "C numeric locale allocation failed");
        locale as usize
    }) as *mut c_void
}
fn integer_conversion(field: &[u8]) -> (i32, bool) {
    // SAFETY: callers provide a terminated ASCII decimal field; CRT locale is
    // immutable and retained. Only this thread's errno is read or written.
    unsafe {
        let slot = _errno();
        let old = *slot;
        let locale = locale();
        *slot = 0;
        let mut end = std::ptr::null_mut();
        let value = std::hint::black_box(_strtol_l)(field.as_ptr().cast(), &mut end, 10, locale);
        let error = *slot != 0 || end == field.as_ptr().cast_mut().cast();
        if !error {
            *slot = old;
        }
        (value, error)
    }
}
fn float_conversion(field: &[u8]) -> (f64, bool) {
    // SAFETY: callers provide a terminated stream-normalized numeric field;
    // the locale and thread-local errno obey the same contract as above.
    unsafe {
        let slot = _errno();
        let old = *slot;
        let locale = locale();
        *slot = 0;
        let mut end = std::ptr::null_mut();
        let value = std::hint::black_box(_strtod_l)(field.as_ptr().cast(), &mut end, locale);
        let error = *slot != 0 || end == field.as_ptr().cast_mut().cast();
        *slot = old;
        (value, error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn numeric_prefixes_and_eof_remain_separate_from_failure() {
        let mut input = Cursor::new(b" 42tail");
        assert_eq!(input.integer(), Some(42));
        assert_eq!(input.position(), 3);
        assert_eq!(input.word(), Some(b"tail".as_slice()));
        assert!(input.eof());
        assert!(!input.failed());
        assert_eq!(input.integer(), None);
        assert!(input.failed());
    }
    #[test]
    fn stream_hex_and_malformed_exponents_keep_their_consumption() {
        let mut input = Cursor::new(b"0x1.8p+1 rest");
        assert_eq!(input.number(), Some(3.));
        assert_eq!(input.rest(), b"rest");
        let mut bad = Cursor::new(b"1e+suffix");
        assert_eq!(bad.number(), None);
        assert_eq!(bad.position(), 3);
        assert!(bad.failed());
        assert_eq!(bad.word(), None);
        assert_eq!(Cursor::new(b"nan").number(), None);
        assert_eq!(Cursor::new(b"inf").number(), None);
    }
    #[test]
    fn rest_consumes_getline_lf_but_keeps_carriage_return() {
        let mut input = Cursor::new(b" \talpha\r\nbeta");
        assert_eq!(input.rest(), b"alpha\r");
        assert!(!input.eof());
        assert_eq!(input.word(), Some(b"beta".as_slice()));
    }
}
