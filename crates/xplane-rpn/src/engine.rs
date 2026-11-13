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

#[inline(always)]
pub(super) fn numeric_binary(op: u32, a: f64, b: f64) -> f64 {
    match op {
        OP_ADD => xplane_numerics::fp::add(a, b),
        OP_SUB => xplane_numerics::fp::sub(a, b),
        OP_MUL => xplane_numerics::fp::mul(a, b),
        OP_DIV => {
            if b == 0.0 {
                0.0
            } else {
                xplane_numerics::fp::div(a, b)
            }
        }
        OP_MOD => fmod(a, b),
        OP_IDIV => {
            if b == 0.0 {
                0.0
            } else {
                int32(xplane_numerics::fp::div(a, b))
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
        OP_LOG => xplane_numerics::fp::div(log(a), log(b)),
        OP_ATG2 => atan2(a, b),
        OP_LT => truth(a < b),
        OP_GT => truth(a > b),
        OP_LE => truth(a <= b),
        OP_GE => truth(a >= b),
        OP_AND => truth(a != 0.0 && b != 0.0),
        _ => truth(a != 0.0 || b != 0.0),
    }
}

#[inline(always)]
pub(super) fn numeric_unary(op: u32, a: f64) -> f64 {
    match op {
        OP_INC => xplane_numerics::fp::add(a, 1.0),
        OP_DEC => xplane_numerics::fp::sub(a, 1.0),
        OP_NEG => -a,
        OP_NOT => truth(a == 0.0),
        OP_ABS => f64::from_bits(a.to_bits() & !(1u64 << 63)),
        OP_INT => int32(a),
        OP_FLR => int32(floor(a)),
        OP_CEIL => int32(ceil(a)),
        OP_NEAR => int32(if a > 0.0 {
            xplane_numerics::fp::add(a, 0.5)
        } else {
            xplane_numerics::fp::sub(a, 0.5)
        }),
        OP_COS => cos(a),
        OP_SIN => sin(a),
        OP_ACOS => acos(a),
        OP_ASIN => asin(a),
        OP_TG => tan(a),
        OP_ATG => atan(a),
        OP_SQRT => sqrt(a),
        OP_SQR => xplane_numerics::fp::mul(a, a),
        OP_EXP => exp(a),
        OP_LN => log(a),
        OP_LG => log10(a),
        OP_RDDG => {
            xplane_numerics::fp::div(xplane_numerics::fp::mul(a, 180.0), core::f64::consts::PI)
        }
        OP_DGRD => {
            xplane_numerics::fp::div(xplane_numerics::fp::mul(a, core::f64::consts::PI), 180.0)
        }
        OP_DNOR => lua_mod(a, 360.0),
        _ => lua_mod(a, 2.0 * core::f64::consts::PI),
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

const DYNAMIC: u32 = u32::MAX;

fn failure<E>(status: u32, instr: i64) -> Fault<E> {
    Fault {
        status,
        instr,
        host_error: None,
    }
}

impl Engine {
    pub(super) unsafe fn eval<H: Host>(
        &mut self,
        program: u32,
        max_steps: u32,
        host: &mut H,
    ) -> Result<ValueRef, Fault<H::Error>> {
        let Some(selected) = self.programs.get(program as usize) else {
            return Err(failure(RPN_BAD_CALL, -1));
        };
        let (first, count) = (selected.first, selected.count);
        let (mut pc, mut steps, mut current) = (1u64, 0u64, -1i64);
        while pc <= count {
            steps += 1;
            if steps > u64::from(max_steps) {
                return Err(failure(RPN_STEPS, current));
            }
            let id = first + (pc - 1) as usize;
            current = id as i64;
            // SAFETY: descriptor validation bounds this program's instruction
            // range; exclusive engine access keeps every descriptor/string live.
            match unsafe { self.operation::<DYNAMIC, DYNAMIC, H>(id, pc + 1, host) }? {
                Some(next) => pc = next,
                None => break,
            }
        }
        self.finish(current)
    }
    fn finish<E>(&mut self, current: i64) -> Result<ValueRef, Fault<E>> {
        let value = peek(&self.stack);
        if self.stack.len() > 32 {
            return Err(failure(RPN_CAPACITY, current));
        }
        self.stack.pop();
        Ok(value)
    }
    #[inline(always)]
    unsafe fn operation<const CODE: u32, const ARG: u32, H: Host>(
        &mut self,
        id: usize,
        mut next: u64,
        host: &mut H,
    ) -> Result<Option<u64>, Fault<H::Error>> {
        // SAFETY: both entry paths validate the opcode/arguments and source
        // index. Owned constants, arena and seed outlive this exclusive call.
        unsafe {
            let current = id as i64;
            let stack = &mut self.stack;
            let op = if CODE == DYNAMIC {
                self.instrs.get_unchecked(id).op
            } else {
                CODE
            };
            let arg = if ARG == DYNAMIC {
                self.instrs.get_unchecked(id).arg
            } else {
                ARG
            };
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
            match op {
                OP_PUSH => {
                    let value = self.instrs.get_unchecked(id).value;
                    if value.kind == NIL {
                        fail!(RPN_MISSING)
                    }
                    put!(value)
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
                OP_QUIT => return Ok(None),
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
                            numeric_binary(op, a, b)
                        }
                    };
                    put!(num(value))
                }
                _ => {
                    let a = numeric!(pop(stack));
                    let value = numeric_unary(op, a);
                    put!(num(value))
                }
            }
            Ok(Some(next))
        }
    }
    pub(super) fn matches_shape(&self, program: u32, shape: &[(u32, u32)]) -> bool {
        self.programs.get(program as usize).is_some_and(|p| {
            p.count == shape.len() as u64
                && self.instrs[p.first..p.first + shape.len()]
                    .iter()
                    .zip(shape)
                    .all(|(i, &(op, arg))| i.op == op && i.arg == arg)
        })
    }
}

/// Checked execution state used by Rust emitted by the AOT source generator.
/// Every instruction retains the VM's value/arena rules, budget point, source
/// location and host callbacks. No descriptor pointer leaves this API.
pub struct AotFrame<'a, H: Host> {
    engine: &'a mut Engine,
    host: &'a mut H,
    first: usize,
    count: u64,
    next: u64,
    current: i64,
    steps: u64,
    maximum: u64,
}
impl<'a, H: Host> AotFrame<'a, H> {
    pub(super) fn new(engine: &'a mut Engine, program: u32, maximum: u32, host: &'a mut H) -> Self {
        let p = &engine.programs[program as usize];
        let (first, count) = (p.first, p.count);
        Self {
            engine,
            host,
            first,
            count,
            next: 1,
            current: -1,
            steps: 0,
            maximum: u64::from(maximum),
        }
    }
    /// Next one-based program counter, or a value beyond the program at exit.
    pub fn next_pc(&self) -> u64 {
        self.next
    }
    /// A generated body's control/opcode mismatch is a bad call, never execution
    /// of another source instruction. Operands always come from the live VM.
    #[inline(always)]
    pub fn step<const OP: u32, const ARG: u32>(&mut self, pc: u32) -> Result<(), Fault<H::Error>> {
        if u64::from(pc) != self.next || pc == 0 || u64::from(pc) > self.count {
            return Err(self.bad_plan());
        }
        let id = self.first + (pc - 1) as usize;
        let ins = &self.engine.instrs[id];
        if ins.op != OP || ins.arg != ARG {
            return Err(self.bad_plan());
        }
        self.steps += 1;
        if self.steps > self.maximum {
            return Err(failure(RPN_STEPS, self.current));
        }
        self.current = id as i64;
        // SAFETY: pc lies in the selected validated program; opcode and argument
        // equality preserves all register/jump descriptor validation. The frame
        // owns exclusive engine access and copies host strings into its arena.
        self.next = unsafe {
            self.engine
                .operation::<OP, ARG, H>(id, u64::from(pc) + 1, self.host)
        }?
        .unwrap_or(self.count + 1);
        Ok(())
    }
    /// Execute one instruction of an already bound, forward-only generated plan.
    /// The VM entry guard must have proved that the budget covers its full length.
    ///
    /// # Safety
    /// `pc` must be the next source instruction, in 1..=the bound program length;
    /// OP and ARG must exactly match that immutable instruction. The body must
    /// follow the source's branches and stop at exit. This is for code produced
    /// by the AOT generator under `Vm::evaluate_aot_prevalidated`, not hand-written
    /// unchecked instruction sequences.
    #[inline(always)]
    pub unsafe fn step_prevalidated<const OP: u32, const ARG: u32>(
        &mut self,
        pc: u32,
    ) -> Result<(), Fault<H::Error>> {
        let id = self.first + (pc - 1) as usize;
        self.current = id as i64;
        // SAFETY: the caller's validated plan proves the instruction identity,
        // bounds, branch path and descriptor invariants; the entry guard proves
        // enough budget for all instructions on any forward-only path.
        self.next = unsafe {
            self.engine
                .operation::<OP, ARG, H>(id, u64::from(pc) + 1, self.host)
        }?
        .unwrap_or(self.count + 1);
        Ok(())
    }
    /// Execute a declined numeric region through the ordinary tagged VM loop.
    /// Keeping this cold path shared avoids expanding a second native copy of
    /// every stack/type/allocation check into each generated arithmetic region.
    ///
    /// # Safety
    /// start..end must be the exact next contiguous arithmetic/PUSH region in
    /// the bound shape, entirely within this program. Entry budget is validated.
    #[inline(never)]
    pub unsafe fn numeric_fallback(&mut self, start: u32, end: u32) -> Result<(), Fault<H::Error>> {
        for pc in start..end {
            let id = self.first + (pc - 1) as usize;
            self.current = id as i64;
            // SAFETY: generated source bounds and shape establish this source
            // region; operation retains the interpreter's runtime type/errors.
            self.next = unsafe {
                self.engine
                    .operation::<DYNAMIC, DYNAMIC, H>(id, u64::from(pc) + 1, self.host)
            }?
            .unwrap_or(self.count + 1);
        }
        Ok(())
    }
    /// Test a numeric region without reading or converting any floating value.
    /// No callbacks or stack changes occur when this guard declines the region.
    #[inline(always)]
    pub fn numeric_guard(&self, inputs: usize, peak_growth: usize, literals: &[u32]) -> bool {
        let stack = &self.engine.stack;
        stack
            .len()
            .max(inputs)
            .checked_add(peak_growth)
            .is_some_and(|peak| peak <= stack.capacity())
            && stack.iter().rev().take(inputs).all(|v| v.kind == NUMBER)
            && literals.iter().all(|&pc| {
                pc > 0
                    && u64::from(pc) <= self.count
                    && self.engine.instrs[self.first + (pc - 1) as usize]
                        .value
                        .kind
                        == NUMBER
            })
    }
    /// Read an original stack operand; underflow is the interpreter's +0.
    #[inline(always)]
    pub fn numeric_input(&self, from_top: usize) -> f64 {
        self.engine
            .stack
            .len()
            .checked_sub(from_top + 1)
            .map(|at| self.engine.stack[at].number)
            .unwrap_or(0.0)
    }
    /// Read a literal from the currently bound VM, never generated source text.
    ///
    /// # Safety
    /// pc must be a PUSH in the bound program, accepted by numeric_guard.
    #[inline(always)]
    pub unsafe fn numeric_literal(&self, pc: u32) -> f64 {
        // SAFETY: trusted generated code and its numeric region guard establish
        // the source position, type and bounds; this borrow keeps storage live.
        unsafe {
            self.engine
                .instrs
                .get_unchecked(self.first + (pc - 1) as usize)
                .value
                .number
        }
    }
    /// Materialize a completed, non-fallible numeric region at its source boundary.
    ///
    /// # Safety
    /// Only generated forward-only code may call this after numeric_guard;
    /// inputs, values and end must exactly describe the executed source region.
    /// Each output has already passed its source push's NaN normalization.
    #[inline(always)]
    pub unsafe fn numeric_commit(&mut self, inputs: usize, values: &[f64], end: u32) {
        let stack = &mut self.engine.stack;
        stack.truncate(stack.len().saturating_sub(inputs));
        stack.extend(values.iter().map(|&value| num(value)));
        self.current = (self.first + (end - 1) as usize) as i64;
        self.next = u64::from(end) + 1;
    }
    /// Report a generated dispatch mismatch without changing source state.
    pub fn bad_plan(&self) -> Fault<H::Error> {
        failure(RPN_BAD_CALL, self.current)
    }
    pub(super) fn finish(self) -> Result<Value, Fault<H::Error>> {
        if self.next <= self.count {
            return Err(self.bad_plan());
        }
        let value = self.engine.finish(self.current)?;
        // SAFETY: the frame keeps all engine/seed/arena bytes live while the
        // result is copied to an independently owned Value.
        Ok(unsafe { Value::copy_from(value) })
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
