#![doc = include_str!("../README.md")]
#![cfg_attr(not(feature = "std"), no_std)]
extern crate alloc;

use alloc::vec::Vec;
#[cfg(not(feature = "std"))]
use core::alloc::{GlobalAlloc, Layout};
use core::ffi::c_void;
use core::ptr;

unsafe extern "C" {
    fn malloc(size: usize) -> *mut c_void;
    #[cfg(not(feature = "std"))]
    fn realloc(block: *mut c_void, size: usize) -> *mut c_void;
    fn free(block: *mut c_void);
    #[cfg(not(feature = "std"))]
    fn abort() -> !;
    fn floor(x: f64) -> f64;
    fn ceil(x: f64) -> f64;
    fn fmod(x: f64, y: f64) -> f64;
    fn pow(x: f64, y: f64) -> f64;
    fn log(x: f64) -> f64;
    fn log10(x: f64) -> f64;
    fn exp(x: f64) -> f64;
    fn sqrt(x: f64) -> f64;
    fn sin(x: f64) -> f64;
    fn cos(x: f64) -> f64;
    fn tan(x: f64) -> f64;
    fn asin(x: f64) -> f64;
    fn acos(x: f64) -> f64;
    fn atan(x: f64) -> f64;
    fn atan2(y: f64, x: f64) -> f64;
}

// The CRT heap the C++ host uses; malloc is 16-byte aligned on x64.
#[cfg(not(feature = "std"))]
struct Crt;
#[cfg(not(feature = "std"))]
unsafe impl GlobalAlloc for Crt {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if layout.align() > 16 {
            return ptr::null_mut();
        }
        malloc(layout.size()) as *mut u8
    }
    unsafe fn dealloc(&self, block: *mut u8, _: Layout) {
        free(block as *mut c_void)
    }
    unsafe fn realloc(&self, block: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        if layout.align() > 16 {
            return ptr::null_mut();
        }
        realloc(block as *mut c_void, size) as *mut u8
    }
}
#[cfg(not(feature = "std"))]
#[global_allocator]
static ALLOCATOR: Crt = Crt;

#[cfg(not(feature = "std"))]
#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    unsafe { abort() }
}

pub const NUMBER: u32 = 0;
pub const STRING: u32 = 1;

/// A stack/register value: a Lua number or a byte string. String bytes are
/// borrowed: VM constants, caller context storage, or this VM's arena.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct RpnValue {
    pub kind: u32,
    pub number: f64,
    pub text: *const u8,
    pub len: usize,
}
const ZERO: RpnValue = RpnValue {
    kind: NUMBER,
    number: 0.0,
    text: ptr::null(),
    len: 0,
};
const EMPTY: &[u8] = b"\0";
#[inline]
fn num(number: f64) -> RpnValue {
    RpnValue {
        kind: NUMBER,
        number,
        text: ptr::null(),
        len: 0,
    }
}
#[inline]
fn truth(value: bool) -> f64 {
    if value { 1.0 } else { 0.0 }
}
#[inline]
unsafe fn bytes<'a>(value: &RpnValue) -> &'a [u8] {
    unsafe {
        if value.len == 0 {
            &[]
        } else {
            core::slice::from_raw_parts(value.text, value.len)
        }
    }
}

/// Instruction descriptor. `arg` is the 1-based jump target or the register index.
#[repr(C)]
pub struct RpnInstr {
    pub op: u32,
    pub arg: u32,
    pub value: RpnValue,
}
#[repr(C)]
pub struct RpnProgram {
    pub first: u32,
    pub count: u32,
}

pub const OP_PUSH: u32 = 0;
pub const OP_GET_L: u32 = 1;
pub const OP_GET_X: u32 = 2;
pub const OP_SET_L: u32 = 3;
pub const OP_SET_K: u32 = 4;
pub const OP_SET_X: u32 = 5;
pub const OP_JZ: u32 = 6;
pub const OP_JUMP: u32 = 7;
pub const OP_QUIT: u32 = 8;
pub const OP_REG_L: u32 = 9;
pub const OP_REG_SP: u32 = 10;
pub const OP_REG_S: u32 = 11;
pub const OP_DUP: u32 = 12;
pub const OP_POP: u32 = 13;
pub const OP_CLEAR: u32 = 14;
pub const OP_SWAP: u32 = 15;
pub const OP_PI: u32 = 16;
pub const OP_CASE: u32 = 17;
pub const OP_SCMP: u32 = 18;
pub const OP_RNG: u32 = 19;
pub const OP_EQ: u32 = 20;
pub const OP_NE: u32 = 21;
pub const OP_ADD: u32 = 22;
pub const OP_SUB: u32 = 23;
pub const OP_MUL: u32 = 24;
pub const OP_DIV: u32 = 25;
pub const OP_MOD: u32 = 26;
pub const OP_IDIV: u32 = 27;
pub const OP_POW: u32 = 28;
pub const OP_MIN: u32 = 29;
pub const OP_MAX: u32 = 30;
pub const OP_LOG: u32 = 31;
pub const OP_ATG2: u32 = 32;
pub const OP_LT: u32 = 33;
pub const OP_GT: u32 = 34;
pub const OP_LE: u32 = 35;
pub const OP_GE: u32 = 36;
pub const OP_AND: u32 = 37;
pub const OP_OR: u32 = 38;
pub const OP_INC: u32 = 39;
pub const OP_DEC: u32 = 40;
pub const OP_NEG: u32 = 41;
pub const OP_NOT: u32 = 42;
pub const OP_ABS: u32 = 43;
pub const OP_INT: u32 = 44;
pub const OP_FLR: u32 = 45;
pub const OP_CEIL: u32 = 46;
pub const OP_NEAR: u32 = 47;
pub const OP_COS: u32 = 48;
pub const OP_SIN: u32 = 49;
pub const OP_ACOS: u32 = 50;
pub const OP_ASIN: u32 = 51;
pub const OP_TG: u32 = 52;
pub const OP_ATG: u32 = 53;
pub const OP_SQRT: u32 = 54;
pub const OP_SQR: u32 = 55;
pub const OP_EXP: u32 = 56;
pub const OP_LN: u32 = 57;
pub const OP_LG: u32 = 58;
pub const OP_RDDG: u32 = 59;
pub const OP_DGRD: u32 = 60;
pub const OP_DNOR: u32 = 61;
pub const OP_RNOR: u32 = 62;
const OP_COUNT: u32 = 63;

pub const RPN_OK: u32 = 0;
/// A host callback failed and recorded its own message.
pub const RPN_HOST: u32 = 1;
pub const RPN_NUMERIC: u32 = 2;
pub const RPN_SCMP: u32 = 3;
pub const RPN_CASE: u32 = 4;
pub const RPN_STEPS: u32 = 5;
pub const RPN_CAPACITY: u32 = 6;
pub const RPN_ALLOC: u32 = 7;
pub const RPN_BAD_CALL: u32 = 8;

/// Where an evaluation failed: the global instruction id the Lua VM's `fail`
/// would name (`current`), or -1 for none (the program source).
#[repr(C)]
pub struct RpnFault {
    pub status: u32,
    pub instr: i64,
}

/// Host callbacks; each returns 0 on success. They receive the global
/// instruction id. `get_external` may return a string valid for the call only.
#[repr(C)]
pub struct RpnHost {
    pub context: *mut c_void,
    pub get_local: Option<unsafe extern "C" fn(*mut c_void, u32, *mut f64) -> i32>,
    pub set_local: Option<unsafe extern "C" fn(*mut c_void, u32, *const RpnValue) -> i32>,
    pub get_external: Option<unsafe extern "C" fn(*mut c_void, u32, *mut RpnValue) -> i32>,
    pub set_external: Option<unsafe extern "C" fn(*mut c_void, u32, *const RpnValue, i32) -> i32>,
}

// Lua 5.1 luai_nummod.
#[inline]
unsafe fn lua_mod(a: f64, b: f64) -> f64 {
    unsafe { a - floor(a / b) * b }
}

// rpn_vm.lua native_int32: CVTTSD2SI r32 with x86 indefinite, +0 for zero.
#[inline]
unsafe fn int32(v: f64) -> f64 {
    unsafe {
        let rounded = if v < 0.0 { ceil(v) } else { floor(v) };
        if rounded.is_nan() || rounded >= 2147483648.0 || rounded < -2147483648.0 {
            return -2147483648.0;
        }
        if rounded == 0.0 {
            return 0.0;
        }
        rounded
    }
}

#[inline]
unsafe fn lua_equal(a: &RpnValue, b: &RpnValue) -> bool {
    unsafe {
        match (a.kind, b.kind) {
            (NUMBER, NUMBER) => a.number == b.number,
            (STRING, STRING) => bytes(a) == bytes(b),
            _ => false,
        }
    }
}

// Copies of strings returned by external reads, stable until the next begin.
// Chunks never grow past their capacity, so handed-out pointers stay valid.
struct Arena {
    chunks: Vec<Vec<u8>>,
    current: usize,
}
impl Arena {
    fn reset(&mut self) {
        for chunk in &mut self.chunks {
            chunk.clear();
        }
        self.current = 0;
    }
    fn store(&mut self, text: &[u8]) -> Option<*const u8> {
        if text.is_empty() {
            return Some(EMPTY.as_ptr());
        }
        while self.current < self.chunks.len() {
            let chunk = &mut self.chunks[self.current];
            if chunk.capacity() - chunk.len() >= text.len() {
                let start = chunk.len();
                chunk.extend_from_slice(text);
                return Some(unsafe { chunk.as_ptr().add(start) });
            }
            self.current += 1;
        }
        let mut chunk = Vec::new();
        chunk
            .try_reserve_exact(if text.len() > 4096 { text.len() } else { 4096 })
            .ok()?;
        self.chunks.try_reserve(1).ok()?;
        chunk.extend_from_slice(text);
        let at = chunk.as_ptr();
        self.chunks.push(chunk);
        self.current = self.chunks.len() - 1;
        Some(at)
    }
}

struct Instr {
    op: u32,
    arg: u32,
    value: RpnValue,
}
struct Program {
    first: usize,
    count: u64,
}

pub struct Vm {
    instrs: Vec<Instr>,
    programs: Vec<Program>,
    _constants: Vec<Vec<u8>>,
    stack: Vec<RpnValue>,
    registers: [RpnValue; 50],
    present: u64,
    dirty: bool,
    arena: Arena,
}

#[inline]
fn pop(stack: &mut Vec<RpnValue>) -> RpnValue {
    stack.pop().unwrap_or(ZERO)
}
#[inline]
fn peek(stack: &[RpnValue]) -> RpnValue {
    stack.last().copied().unwrap_or(ZERO)
}
// rpn_vm.lua push under native_compat: NaN becomes +0, infinities stay.
#[inline]
fn push(stack: &mut Vec<RpnValue>, mut value: RpnValue) -> bool {
    if value.kind == NUMBER && value.number.is_nan() {
        value.number = 0.0;
    }
    if stack.len() == stack.capacity() && stack.try_reserve(1).is_err() {
        return false;
    }
    stack.push(value);
    true
}

impl Vm {
    unsafe fn eval(
        &mut self,
        program: u32,
        max_steps: u32,
        host: &RpnHost,
        result: &mut RpnValue,
        fault: &mut RpnFault,
    ) -> u32 {
        unsafe {
            let (Some(get_local), Some(set_local), Some(get_external), Some(set_external)) = (
                host.get_local,
                host.set_local,
                host.get_external,
                host.set_external,
            ) else {
                fault.status = RPN_BAD_CALL;
                fault.instr = -1;
                return RPN_BAD_CALL;
            };
            let Some(selected) = self.programs.get(program as usize) else {
                fault.status = RPN_BAD_CALL;
                fault.instr = -1;
                return RPN_BAD_CALL;
            };
            let (first, count) = (selected.first, selected.count);
            let context = host.context;
            let maximum = max_steps as u64;
            let (mut pc, mut steps, mut current) = (1u64, 0u64, -1i64);
            let stack = &mut self.stack;
            macro_rules! fail {
                ($status:expr_2021) => {{
                    fault.status = $status;
                    fault.instr = current;
                    return $status;
                }};
            }
            macro_rules! put {
                ($value:expr_2021) => {{
                    let value = $value;
                    if !push(stack, value) {
                        fail!(RPN_ALLOC)
                    }
                }};
            }
            macro_rules! numeric {
                ($value:expr_2021) => {{
                    let value = $value;
                    if value.kind != NUMBER {
                        fail!(RPN_NUMERIC)
                    }
                    value.number
                }};
            }
            while pc <= count {
                steps += 1;
                if steps > maximum {
                    fail!(RPN_STEPS)
                }
                let id = first + (pc - 1) as usize;
                current = id as i64;
                let ins = &self.instrs[id];
                let (op, arg) = (ins.op, ins.arg);
                let mut next = pc + 1;
                match op {
                    OP_PUSH => put!(ins.value),
                    OP_GET_L => {
                        let mut value = 0.0;
                        if get_local(context, id as u32, &mut value) != 0 {
                            fail!(RPN_HOST)
                        }
                        put!(num(value))
                    }
                    OP_GET_X => {
                        let mut value = ZERO;
                        if get_external(context, id as u32, &mut value) != 0 {
                            fail!(RPN_HOST)
                        }
                        if value.kind == STRING {
                            let text = if value.len == 0 {
                                &[][..]
                            } else {
                                core::slice::from_raw_parts(value.text, value.len)
                            };
                            let Some(at) = self.arena.store(text) else {
                                fail!(RPN_ALLOC)
                            };
                            put!(RpnValue {
                                kind: STRING,
                                number: 0.0,
                                text: at,
                                len: value.len
                            })
                        } else {
                            put!(num(value.number))
                        }
                    }
                    OP_SET_L => {
                        let value = pop(stack);
                        if set_local(context, id as u32, &value) != 0 {
                            fail!(RPN_HOST)
                        }
                    }
                    OP_SET_K => {
                        let present = !stack.is_empty();
                        let value = if present { pop(stack) } else { ZERO };
                        if set_external(context, id as u32, &value, present as i32) != 0 {
                            fail!(RPN_HOST)
                        }
                    }
                    OP_SET_X => {
                        let value = pop(stack);
                        if set_external(context, id as u32, &value, 1) != 0 {
                            fail!(RPN_HOST)
                        }
                    }
                    OP_JZ => {
                        if numeric!(pop(stack)) == 0.0 {
                            next = arg as u64;
                        }
                    }
                    OP_JUMP => next = arg as u64,
                    OP_QUIT => break,
                    OP_REG_L => {
                        let slot = arg as usize;
                        if self.present & (1u64 << slot) == 0 {
                            self.registers[slot] = ZERO;
                            self.present |= 1u64 << slot;
                            self.dirty = true;
                        }
                        put!(self.registers[slot])
                    }
                    OP_REG_SP | OP_REG_S => {
                        let slot = arg as usize;
                        self.registers[slot] = if op == OP_REG_SP {
                            pop(stack)
                        } else {
                            peek(stack)
                        };
                        self.present |= 1u64 << slot;
                        self.dirty = true;
                    }
                    OP_DUP => put!(peek(stack)),
                    OP_POP => {
                        pop(stack);
                    }
                    OP_CLEAR => stack.clear(),
                    OP_SWAP => {
                        let b = pop(stack);
                        let a = pop(stack);
                        put!(b);
                        put!(a)
                    }
                    OP_PI => put!(num(core::f64::consts::PI)),
                    OP_CASE => {
                        let index = numeric!(pop(stack));
                        let cases = numeric!(pop(stack));
                        let depth = stack.len();
                        if cases < 1.0 || cases != floor(cases) || cases > depth as f64 {
                            fail!(RPN_CASE)
                        }
                        let index = if index < 0.0 || index >= cases {
                            0.0
                        } else {
                            index
                        };
                        let cases = cases as usize;
                        let value = stack[depth - 1 - floor(index) as usize];
                        stack.truncate(depth - cases);
                        put!(value)
                    }
                    OP_SCMP => {
                        let b = pop(stack);
                        let a = pop(stack);
                        if a.kind != STRING || b.kind != STRING {
                            fail!(RPN_SCMP)
                        }
                        let (a, b) = (bytes(&a), bytes(&b));
                        let limit = if a.len() > b.len() { a.len() } else { b.len() };
                        let mut order = 0.0;
                        for i in 0..=limit {
                            let av = if i < a.len() { a[i] } else { 0 };
                            let bv = if i < b.len() { b[i] } else { 0 };
                            if av != bv {
                                order = if av < bv { -1.0 } else { 1.0 };
                                break;
                            }
                            if av == 0 {
                                break;
                            }
                        }
                        put!(num(order))
                    }
                    OP_RNG => {
                        let value = numeric!(pop(stack));
                        let hi = numeric!(pop(stack));
                        let lo = numeric!(pop(stack));
                        put!(num(truth(value >= lo && value <= hi)))
                    }
                    OP_EQ..=OP_OR => {
                        let b = pop(stack);
                        let a = pop(stack);
                        let value = match op {
                            OP_EQ => truth(lua_equal(&a, &b)),
                            OP_NE => truth(!lua_equal(&a, &b)),
                            _ => {
                                let a = numeric!(a);
                                let b = numeric!(b);
                                match op {
                                    OP_ADD => a + b,
                                    OP_SUB => a - b,
                                    OP_MUL => a * b,
                                    OP_DIV => {
                                        if b == 0.0 {
                                            0.0
                                        } else {
                                            a / b
                                        }
                                    }
                                    OP_MOD => fmod(a, b),
                                    OP_IDIV => {
                                        if b == 0.0 {
                                            0.0
                                        } else {
                                            int32(a / b)
                                        }
                                    }
                                    OP_POW => pow(a, b),
                                    OP_MIN => {
                                        if b < a {
                                            b
                                        } else {
                                            a
                                        }
                                    }
                                    OP_MAX => {
                                        if b > a {
                                            b
                                        } else {
                                            a
                                        }
                                    }
                                    OP_LOG => log(a) / log(b),
                                    OP_ATG2 => atan2(a, b),
                                    OP_LT => truth(a < b),
                                    OP_GT => truth(a > b),
                                    OP_LE => truth(a <= b),
                                    OP_GE => truth(a >= b),
                                    OP_AND => truth(a != 0.0 && b != 0.0),
                                    _ => truth(a != 0.0 || b != 0.0),
                                }
                            }
                        };
                        put!(num(value))
                    }
                    _ => {
                        let a = numeric!(pop(stack));
                        let value = match op {
                            OP_INC => a + 1.0,
                            OP_DEC => a - 1.0,
                            OP_NEG => -a,
                            OP_NOT => truth(a == 0.0),
                            OP_ABS => f64::from_bits(a.to_bits() & !(1u64 << 63)),
                            OP_INT => int32(a),
                            OP_FLR => int32(floor(a)),
                            OP_CEIL => int32(ceil(a)),
                            OP_NEAR => int32(if a > 0.0 { a + 0.5 } else { a - 0.5 }),
                            OP_COS => cos(a),
                            OP_SIN => sin(a),
                            OP_ACOS => acos(a),
                            OP_ASIN => asin(a),
                            OP_TG => tan(a),
                            OP_ATG => atan(a),
                            OP_SQRT => sqrt(a),
                            OP_SQR => a * a,
                            OP_EXP => exp(a),
                            OP_LN => log(a),
                            OP_LG => log10(a),
                            OP_RDDG => a * 180.0 / core::f64::consts::PI,
                            OP_DGRD => a * core::f64::consts::PI / 180.0,
                            OP_DNOR => lua_mod(a, 360.0),
                            _ => lua_mod(a, 2.0 * core::f64::consts::PI),
                        };
                        put!(num(value))
                    }
                }
                pc = next;
            }
            let value = peek(stack);
            if stack.len() > 32 {
                fail!(RPN_CAPACITY)
            }
            stack.pop();
            *result = value;
            RPN_OK
        }
    }
}

/// Builds a VM over validated descriptors; push string constants are copied.
/// Returns null for an invalid descriptor or allocation failure.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rpn_vm_new(
    instrs: *const RpnInstr,
    count: usize,
    programs: *const RpnProgram,
    program_count: usize,
) -> *mut Vm {
    unsafe {
        if (count != 0 && instrs.is_null())
            || (program_count != 0 && programs.is_null())
            || count > u32::MAX as usize
        {
            return ptr::null_mut();
        }
        let source = if count == 0 {
            &[][..]
        } else {
            core::slice::from_raw_parts(instrs, count)
        };
        let program_source = if program_count == 0 {
            &[][..]
        } else {
            core::slice::from_raw_parts(programs, program_count)
        };
        let mut built: Vec<Instr> = Vec::new();
        let mut constants: Vec<Vec<u8>> = Vec::new();
        let mut table: Vec<Program> = Vec::new();
        if built.try_reserve_exact(count).is_err()
            || table.try_reserve_exact(program_count).is_err()
        {
            return ptr::null_mut();
        }
        for ins in source {
            if ins.op >= OP_COUNT {
                return ptr::null_mut();
            }
            if (ins.op == OP_JZ || ins.op == OP_JUMP) && ins.arg == 0 {
                return ptr::null_mut();
            }
            if (ins.op == OP_REG_L || ins.op == OP_REG_SP || ins.op == OP_REG_S) && ins.arg >= 50 {
                return ptr::null_mut();
            }
            let mut value = ZERO;
            if ins.op == OP_PUSH {
                match ins.value.kind {
                    NUMBER => value = num(ins.value.number),
                    STRING => {
                        if ins.value.len != 0 && ins.value.text.is_null() {
                            return ptr::null_mut();
                        }
                        let mut text = Vec::new();
                        if text.try_reserve_exact(ins.value.len).is_err()
                            || constants.try_reserve(1).is_err()
                        {
                            return ptr::null_mut();
                        }
                        text.extend_from_slice(bytes(&ins.value));
                        value = RpnValue {
                            kind: STRING,
                            number: 0.0,
                            text: if text.is_empty() {
                                EMPTY.as_ptr()
                            } else {
                                text.as_ptr()
                            },
                            len: text.len(),
                        };
                        constants.push(text);
                    }
                    _ => return ptr::null_mut(),
                }
            }
            built.push(Instr {
                op: ins.op,
                arg: ins.arg,
                value,
            });
        }
        for program in program_source {
            if program.first as u64 + program.count as u64 > count as u64 {
                return ptr::null_mut();
            }
            table.push(Program {
                first: program.first as usize,
                count: program.count as u64,
            });
        }
        let mut stack = Vec::new();
        if stack.try_reserve(64).is_err() {
            return ptr::null_mut();
        }
        let vm = Vm {
            instrs: built,
            programs: table,
            _constants: constants,
            stack,
            registers: [ZERO; 50],
            present: 0,
            dirty: false,
            arena: Arena {
                chunks: Vec::new(),
                current: 0,
            },
        };
        let block = malloc(core::mem::size_of::<Vm>()) as *mut Vm;
        if block.is_null() {
            return ptr::null_mut();
        }
        block.write(vm);
        block
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rpn_vm_free(vm: *mut Vm) {
    unsafe {
        if vm.is_null() {
            return;
        }
        ptr::drop_in_place(vm);
        free(vm as *mut c_void);
    }
}

/// Starts an evaluation's context: an empty stack, and either all 50
/// registers present as +0 or none present.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rpn_begin(vm: *mut Vm, zero_registers: i32) {
    unsafe {
        let vm = &mut *vm;
        vm.stack.clear();
        vm.arena.reset();
        vm.registers = [ZERO; 50];
        vm.present = if zero_registers != 0 {
            (1u64 << 50) - 1
        } else {
            0
        };
        vm.dirty = false;
    }
}

/// Appends a context stack value (bottom first). Strings stay borrowed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rpn_context_push(vm: *mut Vm, value: *const RpnValue) -> i32 {
    unsafe {
        let vm = &mut *vm;
        let value = *value;
        if (value.kind != NUMBER && value.kind != STRING)
            || (value.kind == STRING && value.len != 0 && value.text.is_null())
        {
            return 1;
        }
        let value = if value.kind == STRING && value.len == 0 {
            RpnValue {
                text: EMPTY.as_ptr(),
                ..value
            }
        } else {
            value
        };
        if vm.stack.len() == vm.stack.capacity() && vm.stack.try_reserve(1).is_err() {
            return 1;
        }
        vm.stack.push(value);
        0
    }
}

/// Sets a context register present. Strings stay borrowed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rpn_context_register(
    vm: *mut Vm,
    index: u32,
    value: *const RpnValue,
) -> i32 {
    unsafe {
        let vm = &mut *vm;
        let value = *value;
        if index >= 50
            || (value.kind != NUMBER && value.kind != STRING)
            || (value.kind == STRING && value.len != 0 && value.text.is_null())
        {
            return 1;
        }
        vm.registers[index as usize] = if value.kind == STRING && value.len == 0 {
            RpnValue {
                text: EMPTY.as_ptr(),
                ..value
            }
        } else {
            value
        };
        vm.present |= 1u64 << index;
        0
    }
}

/// Runs one program against the current context (rpn_vm.lua M.eval with an
/// execution_context). On failure the context is unspecified.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rpn_eval(
    vm: *mut Vm,
    program: u32,
    max_steps: u32,
    host: *const RpnHost,
    result: *mut RpnValue,
    fault: *mut RpnFault,
) -> u32 {
    unsafe {
        let fault = &mut *fault;
        fault.status = RPN_OK;
        fault.instr = -1;
        if vm.is_null() || host.is_null() || result.is_null() {
            fault.status = RPN_BAD_CALL;
            return RPN_BAD_CALL;
        }
        (*vm).eval(program, max_steps, &*host, &mut *result, fault)
    }
}

/// The context stack, bottom first.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rpn_stack(vm: *const Vm, len: *mut usize) -> *const RpnValue {
    unsafe {
        let vm = &*vm;
        *len = vm.stack.len();
        vm.stack.as_ptr()
    }
}

/// The 50 register slots; bit i of `present` marks slot i present. `dirty`
/// is nonzero when any register was created or assigned since rpn_begin.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rpn_registers(
    vm: *const Vm,
    present: *mut u64,
    dirty: *mut i32,
) -> *const RpnValue {
    unsafe {
        let vm = &*vm;
        *present = vm.present;
        *dirty = vm.dirty as i32;
        vm.registers.as_ptr()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct OwnedVm(*mut Vm);
    impl Drop for OwnedVm {
        fn drop(&mut self) {
            unsafe {
                rpn_vm_free(self.0);
            }
        }
    }
    fn vm(instructions: &[RpnInstr]) -> OwnedVm {
        let program = RpnProgram {
            first: 0,
            count: instructions.len() as u32,
        };
        let value = unsafe { rpn_vm_new(instructions.as_ptr(), instructions.len(), &program, 1) };
        assert!(!value.is_null());
        OwnedVm(value)
    }
    #[derive(Default)]
    struct Trace {
        calls: Vec<(u32, f64)>,
        fail: bool,
    }
    unsafe extern "C" fn get(context: *mut c_void, id: u32, out: *mut f64) -> i32 {
        unsafe {
            if !context.is_null() {
                (*(context as *mut Trace)).calls.push((id, 9.0));
            }
            *out = 9.0;
            0
        }
    }
    unsafe extern "C" fn set(context: *mut c_void, id: u32, value: *const RpnValue) -> i32 {
        unsafe {
            if context.is_null() {
                return 0;
            }
            let trace = &mut *(context as *mut Trace);
            trace.calls.push((id, (*value).number));
            i32::from(trace.fail)
        }
    }
    unsafe extern "C" fn get_external(_: *mut c_void, _: u32, out: *mut RpnValue) -> i32 {
        unsafe {
            *out = ZERO;
            0
        }
    }
    unsafe extern "C" fn set_external(_: *mut c_void, _: u32, _: *const RpnValue, _: i32) -> i32 {
        0
    }
    fn host(trace: &mut Trace) -> RpnHost {
        RpnHost {
            context: (trace as *mut Trace).cast(),
            get_local: Some(get),
            set_local: Some(set),
            get_external: Some(get_external),
            set_external: Some(set_external),
        }
    }
    #[test]
    fn mixed_host_operations_keep_callback_order_and_exact_fault_position() {
        let instructions = [
            RpnInstr {
                op: OP_GET_L,
                arg: 0,
                value: ZERO,
            },
            RpnInstr {
                op: OP_PUSH,
                arg: 0,
                value: num(2.0),
            },
            RpnInstr {
                op: OP_ADD,
                arg: 0,
                value: ZERO,
            },
            RpnInstr {
                op: OP_DUP,
                arg: 0,
                value: ZERO,
            },
            RpnInstr {
                op: OP_SET_L,
                arg: 0,
                value: ZERO,
            },
        ];
        let vm = vm(&instructions);
        let mut trace = Trace::default();
        let mut result = ZERO;
        let mut fault = RpnFault {
            status: 999,
            instr: 999,
        };
        unsafe {
            rpn_begin(vm.0, 1);
            assert_eq!(
                rpn_eval(vm.0, 0, 100, &host(&mut trace), &mut result, &mut fault),
                RPN_OK
            );
        }
        assert_eq!(trace.calls, [(0, 9.0), (4, 11.0)]);
        assert_eq!(result.number, 11.0);
        trace.calls.clear();
        trace.fail = true;
        unsafe {
            rpn_begin(vm.0, 1);
            assert_eq!(
                rpn_eval(vm.0, 0, 100, &host(&mut trace), &mut result, &mut fault),
                RPN_HOST
            );
        }
        assert_eq!(fault.instr, 4);
        assert_eq!(trace.calls, [(0, 9.0), (4, 11.0)]);
    }
    #[test]
    fn string_constants_are_owned_across_the_ffi_boundary() {
        let mut bytes = b"AB".to_vec();
        let text = RpnValue {
            kind: STRING,
            number: 0.0,
            text: bytes.as_ptr(),
            len: bytes.len(),
        };
        let literal = RpnValue {
            kind: STRING,
            number: 0.0,
            text: b"AB".as_ptr(),
            len: 2,
        };
        let vm = vm(&[
            RpnInstr {
                op: OP_PUSH,
                arg: 0,
                value: text,
            },
            RpnInstr {
                op: OP_PUSH,
                arg: 0,
                value: literal,
            },
            RpnInstr {
                op: OP_SCMP,
                arg: 0,
                value: ZERO,
            },
        ]);
        bytes.fill(b'Z');
        drop(bytes);
        let mut trace = Trace::default();
        let mut result = ZERO;
        let mut fault = RpnFault {
            status: 0,
            instr: -1,
        };
        unsafe {
            rpn_begin(vm.0, 0);
            assert_eq!(
                rpn_eval(vm.0, 0, 100, &host(&mut trace), &mut result, &mut fault),
                RPN_OK
            );
        }
        assert_eq!(result.number, 0.0);
    }
    #[test]
    fn invalid_registers_and_unbounded_jumps_are_rejected() {
        let bad = RpnInstr {
            op: OP_REG_L,
            arg: 50,
            value: ZERO,
        };
        let program = RpnProgram { first: 0, count: 1 };
        assert!(unsafe { rpn_vm_new(&bad, 1, &program, 1) }.is_null());
        let vm = vm(&[RpnInstr {
            op: OP_JUMP,
            arg: 1,
            value: ZERO,
        }]);
        let mut trace = Trace::default();
        let mut result = ZERO;
        let mut fault = RpnFault {
            status: 0,
            instr: -1,
        };
        unsafe {
            rpn_begin(vm.0, 1);
            assert_eq!(
                rpn_eval(vm.0, 0, 5, &host(&mut trace), &mut result, &mut fault),
                RPN_STEPS
            );
        }
        assert_eq!(fault.instr, 0);
    }
}
