//! These declarations are called only by the SDK boundary and plugin loader.
//! Every pointer must obey its SDK lifetime/size contract. SEH catches plugin
//! faults, but cannot make an arbitrary native plugin memory safe.
use std::ffi::{c_char, c_void};
pub type Message = unsafe extern "C" fn(i32, i32, *mut c_void);
unsafe extern "C" {
    pub fn xp_start(
        f: unsafe extern "C" fn(*mut c_char, *mut c_char, *mut c_char) -> i32,
        a: *mut c_char,
        b: *mut c_char,
        c: *mut c_char,
        result: *mut i32,
    ) -> u32;
    pub fn xp_enable(f: unsafe extern "C" fn() -> i32, result: *mut i32) -> u32;
    pub fn xp_void(f: unsafe extern "C" fn()) -> u32;
    pub fn xp_message(f: Message, from: i32, msg: i32, p: *mut c_void) -> u32;
    pub fn xp_loop(
        f: unsafe extern "C" fn(f32, f32, i32, *mut c_void) -> f32,
        a: f32,
        b: f32,
        c: i32,
        r: *mut c_void,
        result: *mut f32,
    ) -> u32;
    pub fn xp_command(
        f: unsafe extern "C" fn(*mut c_void, i32, *mut c_void) -> i32,
        c: *mut c_void,
        p: i32,
        r: *mut c_void,
        result: *mut i32,
    ) -> u32;
    pub fn xp_get_i(
        f: unsafe extern "C" fn(*mut c_void) -> i32,
        r: *mut c_void,
        result: *mut i32,
    ) -> u32;
    pub fn xp_get_f(
        f: unsafe extern "C" fn(*mut c_void) -> f32,
        r: *mut c_void,
        result: *mut f32,
    ) -> u32;
    pub fn xp_get_d(
        f: unsafe extern "C" fn(*mut c_void) -> f64,
        r: *mut c_void,
        result: *mut f64,
    ) -> u32;
    pub fn xp_set_i(f: unsafe extern "C" fn(*mut c_void, i32), r: *mut c_void, value: i32) -> u32;
    pub fn xp_set_f(f: unsafe extern "C" fn(*mut c_void, f32), r: *mut c_void, value: f32) -> u32;
    pub fn xp_set_d(f: unsafe extern "C" fn(*mut c_void, f64), r: *mut c_void, value: f64) -> u32;
    pub fn xp_get_vi(
        f: unsafe extern "C" fn(*mut c_void, *mut i32, i32, i32) -> i32,
        r: *mut c_void,
        values: *mut i32,
        off: i32,
        count: i32,
        result: *mut i32,
    ) -> u32;
    pub fn xp_get_vf(
        f: unsafe extern "C" fn(*mut c_void, *mut f32, i32, i32) -> i32,
        r: *mut c_void,
        values: *mut f32,
        off: i32,
        count: i32,
        result: *mut i32,
    ) -> u32;
    pub fn xp_get_b(
        f: unsafe extern "C" fn(*mut c_void, *mut c_void, i32, i32) -> i32,
        r: *mut c_void,
        values: *mut c_void,
        off: i32,
        count: i32,
        result: *mut i32,
    ) -> u32;
    pub fn xp_set_vi(
        f: unsafe extern "C" fn(*mut c_void, *mut i32, i32, i32),
        r: *mut c_void,
        values: *mut i32,
        off: i32,
        count: i32,
    ) -> u32;
    pub fn xp_set_vf(
        f: unsafe extern "C" fn(*mut c_void, *mut f32, i32, i32),
        r: *mut c_void,
        values: *mut f32,
        off: i32,
        count: i32,
    ) -> u32;
    pub fn xp_set_b(
        f: unsafe extern "C" fn(*mut c_void, *mut c_void, i32, i32),
        r: *mut c_void,
        values: *mut c_void,
        off: i32,
        count: i32,
    ) -> u32;
}
