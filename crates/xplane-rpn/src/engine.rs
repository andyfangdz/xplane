//! Interpreter implementation; borrowed arena values never leave this module/API boundary.
use crate::*;
use std::ptr;
use xplane_numerics::fp::{
    acos, asin, atan, atan2, ceil, cos, exp, floor, fmod, log, log10, pow, sin, sqrt, tan,
};
pub(super) const NUMBER: u32 = 0;
pub(super) const STRING: u32 = 1;
pub(super) const BOOLEAN: u32 = 2;
pub(super) const NIL: u32 = 3;
pub(super) fn scalar(kind: u32, number: f64) -> ValueRef {
    ValueRef {
        kind,
        number,
        text: ptr::null(),
        len: 0,
    }
}

/// A stack/register scalar with byte strings borrowed from stable storage.
/// String bytes are
/// borrowed: VM constants, caller context storage, or this VM's arena.
#[derive(Clone, Copy)]
pub(super) struct ValueRef {
    pub kind: u32,
    pub number: f64,
    pub text: *const u8,
    pub len: usize,
}
pub(super) const ZERO: ValueRef = ValueRef {
    kind: NUMBER,
    number: 0.0,
    text: ptr::null(),
    len: 0,
};
const EMPTY: &[u8] = b"\0";
#[inline]
pub(super) fn num(number: f64) -> ValueRef {
    ValueRef {
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
pub(super) unsafe fn bytes<'a>(value: &ValueRef) -> &'a [u8] {
    unsafe {
        if value.len == 0 {
            &[]
        } else {
            core::slice::from_raw_parts(value.text, value.len)
        }
    }
}

// Lua 5.1 luai_nummod.
#[inline]
fn lua_mod(a: f64, b: f64) -> f64 {
    a - floor(a / b) * b
}

// rpn_vm.lua native_int32: CVTTSD2SI r32 with x86 indefinite, +0 for zero.
#[inline]
fn int32(v: f64) -> f64 {
    let rounded = if v < 0.0 { ceil(v) } else { floor(v) };
    if rounded.is_nan() || rounded >= 2147483648.0 || rounded < -2147483648.0 {
        return -2147483648.0;
    }
    if rounded == 0.0 {
        return 0.0;
    }
    rounded
}

#[inline]
unsafe fn lua_equal(a: &ValueRef, b: &ValueRef) -> bool {
    unsafe {
        match (a.kind, b.kind) {
            (NUMBER, NUMBER) | (BOOLEAN, BOOLEAN) => a.number == b.number,
            (NIL, NIL) => true,
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
    value: ValueRef,
}
struct Program {
    first: usize,
    count: u64,
}

pub(super) struct Engine {
    instrs: Vec<Instr>,
    programs: Vec<Program>,
    _constants: Vec<Vec<u8>>,
    pub(super) stack: Vec<ValueRef>,
    pub(super) registers: [ValueRef; 50],
    pub(super) present: u64,
    pub(super) dirty: bool,
    arena: Arena,
}

#[inline]
fn pop(stack: &mut Vec<ValueRef>) -> ValueRef {
    stack.pop().unwrap_or(ZERO)
}
#[inline]
fn peek(stack: &[ValueRef]) -> ValueRef {
    stack.last().copied().unwrap_or(ZERO)
}
// rpn_vm.lua push under native_compat: NaN becomes +0, infinities stay.
#[inline]
fn push(stack: &mut Vec<ValueRef>, mut value: ValueRef) -> bool {
    if value.kind == NUMBER && value.number.is_nan() {
        value.number = 0.0;
    }
    if stack.len() == stack.capacity() && stack.try_reserve(1).is_err() {
        return false;
    }
    stack.push(value);
    true
}

impl Engine {
    pub(super) unsafe fn eval<H: Host>(
        &mut self,
        program: u32,
        max_steps: u32,
        host: &mut H,
    ) -> Result<ValueRef, Fault<H::Error>> {
        unsafe {
            let Some(selected) = self.programs.get(program as usize) else {
                return Err(Fault {
                    status: RPN_BAD_CALL,
                    instr: -1,
                    host_error: None,
                });
            };
            let (first, count) = (selected.first, selected.count);
            let maximum = max_steps as u64;
            let (mut pc, mut steps, mut current) = (1u64, 0u64, -1i64);
            let stack = &mut self.stack;
            macro_rules! fail {
                ($status:expr_2021) => {{
                    return Err(Fault {
                        status: $status,
                        instr: current,
                        host_error: None,
                    });
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
                        fail!(if value.kind == BOOLEAN {
                            RPN_NUMERIC_BOOLEAN
                        } else {
                            RPN_NUMERIC
                        })
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
                    OP_PUSH => {
                        if ins.value.kind == NIL {
                            fail!(RPN_MISSING)
                        }
                        put!(ins.value)
                    }
                    OP_GET_L => {
                        let value = host.get_local(id as u32).map_err(|error| Fault {
                            status: RPN_HOST,
                            instr: current,
                            host_error: Some(error),
                        })?;
                        put!(num(value))
                    }
                    OP_GET_X => {
                        let value = host.get_external(id as u32).map_err(|error| Fault {
                            status: RPN_HOST,
                            instr: current,
                            host_error: Some(error),
                        })?;
                        match value {
                            Value::String(text) => {
                                let Some(at) = self.arena.store(&text) else {
                                    fail!(RPN_ALLOC)
                                };
                                put!(ValueRef {
                                    kind: STRING,
                                    number: 0.,
                                    text: at,
                                    len: text.len()
                                })
                            }
                            Value::Number(number) => put!(num(number)),
                            Value::Boolean(value) => put!(scalar(BOOLEAN, f64::from(value))),
                            Value::Nil => {
                                fail!(RPN_MISSING)
                            }
                        }
                    }
                    OP_SET_L => {
                        let value = Value::copy_from(pop(stack));
                        host.set_local(id as u32, &value).map_err(|error| Fault {
                            status: RPN_HOST,
                            instr: current,
                            host_error: Some(error),
                        })?;
                    }
                    OP_SET_K => {
                        let present = !stack.is_empty();
                        let value = Value::copy_from(if present { pop(stack) } else { ZERO });
                        host.set_external(id as u32, &value, present)
                            .map_err(|error| Fault {
                                status: RPN_HOST,
                                instr: current,
                                host_error: Some(error),
                            })?;
                    }
                    OP_SET_X => {
                        let value = Value::copy_from(pop(stack));
                        host.set_external(id as u32, &value, true)
                            .map_err(|error| Fault {
                                status: RPN_HOST,
                                instr: current,
                                host_error: Some(error),
                            })?;
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
            Ok(value)
        }
    }
}

impl Engine {
    pub(super) fn new(
        instructions: &[crate::Instruction],
        programs: &[crate::Program],
    ) -> Result<Self, BuildError> {
        if instructions.len() > u32::MAX as usize
            || instructions.iter().any(|i| {
                i.op >= OP_COUNT
                    || (matches!(i.op, OP_JZ | OP_JUMP) && i.arg == 0)
                    || (matches!(i.op, OP_REG_L | OP_REG_SP | OP_REG_S) && i.arg >= 50)
            })
            || programs
                .iter()
                .any(|p| u64::from(p.first) + u64::from(p.count) > instructions.len() as u64)
        {
            return Err(BuildError::InvalidDescriptor);
        }
        let mut built = Vec::new();
        let mut constants = Vec::new();
        let mut table = Vec::new();
        built
            .try_reserve_exact(instructions.len())
            .map_err(|_| BuildError::Allocation)?;
        table
            .try_reserve_exact(programs.len())
            .map_err(|_| BuildError::Allocation)?;
        for ins in instructions {
            let mut value = ZERO;
            if ins.op == OP_PUSH {
                value = match &ins.value {
                    Value::Number(n) => num(*n),
                    Value::Boolean(value) => scalar(BOOLEAN, f64::from(*value)),
                    Value::Nil => scalar(NIL, 0.),
                    Value::String(source) => {
                        let mut text = Vec::new();
                        text.try_reserve_exact(source.len())
                            .map_err(|_| BuildError::Allocation)?;
                        constants
                            .try_reserve(1)
                            .map_err(|_| BuildError::Allocation)?;
                        text.extend_from_slice(source);
                        let value = ValueRef {
                            kind: STRING,
                            number: 0.,
                            text: if text.is_empty() {
                                EMPTY.as_ptr()
                            } else {
                                text.as_ptr()
                            },
                            len: text.len(),
                        };
                        constants.push(text);
                        value
                    }
                };
            }
            built.push(Instr {
                op: ins.op,
                arg: ins.arg,
                value,
            });
        }
        for program in programs {
            table.push(Program {
                first: program.first as usize,
                count: u64::from(program.count),
            });
        }
        let mut stack = Vec::new();
        stack.try_reserve(64).map_err(|_| BuildError::Allocation)?;
        Ok(Self {
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
        })
    }
    pub(super) fn begin(&mut self, zero_registers: bool) {
        self.stack.clear();
        self.arena.reset();
        self.registers = [ZERO; 50];
        self.present = if zero_registers { (1u64 << 50) - 1 } else { 0 };
        self.dirty = false;
    }
    pub(super) fn seed_stack(&mut self, value: ValueRef) -> Result<(), BuildError> {
        if self.stack.len() == self.stack.capacity() {
            self.stack
                .try_reserve(1)
                .map_err(|_| BuildError::Allocation)?;
        }
        self.stack.push(value);
        Ok(())
    }
    pub(super) fn seed_register(&mut self, index: u32, value: ValueRef) {
        self.registers[index as usize] = value;
        if value.kind == NIL {
            self.present &= !(1u64 << index);
        } else {
            self.present |= 1u64 << index;
        }
    }
}
