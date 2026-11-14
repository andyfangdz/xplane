//! Trusted all-number stack/register execution with effect-preserving deopt.
use super::*;
use std::mem::MaybeUninit;
/// Private numeric storage for a generated forward-only program. The entry
/// guard proves an empty stack with numeric/absent registers and a maximum stack depth of 64.
pub struct NumericFrame<'a, H: Host> {
    engine: &'a mut Engine,
    host: &'a mut H,
    first: usize,
    count: u64,
    maximum: u32,
    stack: [MaybeUninit<f64>; 64],
    len: usize,
    registers: [MaybeUninit<f64>; 50],
    dirty: u64,
    present: u64,
    next: u64,
    current: i64,
    steps: u64,
    materialized: bool,
}
impl<'a, H: Host> NumericFrame<'a, H> {
    pub(crate) fn new(engine: &'a mut Engine, program: u32, maximum: u32, host: &'a mut H) -> Self {
        let p = &engine.programs[program as usize];
        let (first, count) = (p.first, p.count);
        let present = engine.present;
        Self {
            engine,
            host,
            first,
            count,
            maximum,
            stack: [MaybeUninit::uninit(); 64],
            len: 0,
            registers: [MaybeUninit::uninit(); 50],
            dirty: 0,
            present,
            next: 1,
            current: -1,
            steps: 0,
            materialized: false,
        }
    }
    pub(crate) fn deoptimized(&self) -> bool {
        self.materialized
    }
    pub fn next_pc(&self) -> u64 {
        self.next
    }
    pub fn bad_plan(&self) -> Fault<H::Error> {
        failure(RPN_BAD_CALL, self.current)
    }
    #[inline(always)]
    unsafe fn pop(&mut self) -> f64 {
        if self.len == 0 {
            return 0.0;
        }
        self.len -= 1;
        // SAFETY: the numeric stack invariant initializes exactly 0..len.
        unsafe { self.stack.get_unchecked(self.len).assume_init() }
    }
    #[inline(always)]
    unsafe fn peek(&self) -> f64 {
        if self.len == 0 {
            0.0
        } else {
            // SAFETY: every live slot was initialized by a source push.
            unsafe { self.stack.get_unchecked(self.len - 1).assume_init() }
        }
    }
    #[inline(always)]
    unsafe fn put(&mut self, value: f64) {
        let value = crate::aot::normalize(value);
        // SAFETY: the bound source CFG proves all stack heights at most 64.
        unsafe {
            self.stack.get_unchecked_mut(self.len).write(value);
        }
        self.len += 1;
    }
    fn flush(&mut self) {
        if self.materialized {
            return;
        }
        self.engine.stack.clear();
        for at in 0..self.len {
            // SAFETY: the numeric stack invariant initializes every live slot;
            // Engine construction reserved at least 64 descriptor slots.
            self.engine
                .stack
                .push(num(unsafe { self.stack[at].assume_init() }));
        }
        let mut mask = self.dirty;
        while mask != 0 {
            let slot = mask.trailing_zeros() as usize;
            mask &= mask - 1;
            // SAFETY: a dirty bit is set only after writing that register.
            self.engine.registers[slot] = num(unsafe { self.registers[slot].assume_init() });
        }
        self.engine.dirty |= self.dirty != 0;
        self.engine.present = self.present;
        self.materialized = true;
    }
    /// Execute the next instruction of the exact bound numeric plan.
    /// Returns false after a non-number read; the body must stop immediately.
    ///
    /// # Safety
    /// OP/ARG/pc must match the next immutable source instruction. HEIGHT must
    /// be numeric_heights[pc-1], or usize::MAX when the height is not unique.
    /// Follow source branches, stop at exit, and never run after false/error.
    #[inline(always)]
    pub unsafe fn step<const OP: u32, const ARG: u32, const HEIGHT: usize>(
        &mut self,
        pc: u32,
    ) -> Result<bool, Fault<H::Error>> {
        if HEIGHT != usize::MAX {
            self.len = HEIGHT;
        }
        let id = self.first + (pc - 1) as usize;
        self.current = id as i64;
        self.steps += 1;
        self.next = u64::from(pc) + 1;
        let host_fault = |error| Fault {
            status: RPN_HOST,
            instr: id as i64,
            host_error: Some(error),
        };
        // SAFETY: the trusted generated plan and VM entry guard prove opcode,
        // source bounds, initialized numeric state, CFG heights and register ids.
        unsafe {
            match OP {
                OP_PUSH => {
                    let value = self.engine.instrs.get_unchecked(id).value.number;
                    self.put(value);
                }
                OP_GET_L => {
                    let value = self.host.get_local(id as u32).map_err(host_fault)?;
                    self.put(value);
                }
                OP_GET_X => {
                    let value = self.host.get_external(id as u32).map_err(host_fault)?;
                    if let Value::Number(value) = value {
                        self.put(value);
                    } else {
                        self.flush();
                        self.engine.accept_external(value, id as i64)?;
                        return Ok(false);
                    }
                }
                OP_SET_L => {
                    let value = Value::Number(self.pop());
                    self.host.set_local(id as u32, &value).map_err(host_fault)?;
                }
                OP_SET_K | OP_SET_X => {
                    let present = OP == OP_SET_X || self.len > 0;
                    let value = Value::Number(self.pop());
                    self.host
                        .set_external(id as u32, &value, present)
                        .map_err(host_fault)?;
                }
                OP_JZ => {
                    if zero_test(self.pop()) {
                        self.next = u64::from(ARG);
                    }
                }
                OP_JUMP => self.next = u64::from(ARG),
                OP_QUIT => self.next = self.count + 1,
                OP_REG_L => {
                    let bit = 1u64 << ARG;
                    let value = if self.present & bit == 0 {
                        self.registers.get_unchecked_mut(ARG as usize).write(0.0);
                        self.present |= bit;
                        self.dirty |= bit;
                        0.0
                    } else if self.dirty & bit != 0 {
                        self.registers.get_unchecked(ARG as usize).assume_init()
                    } else {
                        self.engine.registers.get_unchecked(ARG as usize).number
                    };
                    self.put(value);
                }
                OP_REG_S | OP_REG_SP => {
                    let value = if OP == OP_REG_S {
                        self.peek()
                    } else {
                        self.pop()
                    };
                    self.registers.get_unchecked_mut(ARG as usize).write(value);
                    self.dirty |= 1u64 << ARG;
                    self.present |= 1u64 << ARG;
                }
                OP_DUP => {
                    let value = self.peek();
                    self.put(value);
                }
                OP_POP => {
                    self.pop();
                }
                OP_CLEAR => self.len = 0,
                OP_SWAP => {
                    let b = self.pop();
                    let a = self.pop();
                    self.put(b);
                    self.put(a);
                }
                OP_PI => self.put(core::f64::consts::PI),
                OP_CASE => {
                    let index = self.pop();
                    let cases = self.pop();
                    let depth = self.len;
                    if cases < 1.0 || cases != floor(cases) || cases > depth as f64 {
                        return Err(failure(RPN_CASE, id as i64));
                    }
                    let index = if index < 0.0 || index >= cases {
                        0.0
                    } else {
                        index
                    };
                    // Validated integral count is in 1..=depth; clamped/floored
                    // index is in 0..cases, so this is an initialized live slot.
                    let value = self
                        .stack
                        .get_unchecked(depth - 1 - floor(index) as usize)
                        .assume_init();
                    self.len = depth - cases as usize;
                    self.put(value);
                }
                OP_RNG => {
                    let v = self.pop();
                    let hi = self.pop();
                    let lo = self.pop();
                    self.put(truth(v >= lo && v <= hi));
                }
                OP_EQ..=OP_OR => {
                    let b = self.pop();
                    let a = self.pop();
                    self.put(numeric_binary(OP, a, b));
                }
                OP_INC..=OP_RNOR => {
                    let a = self.pop();
                    self.put(numeric_unary(OP, a));
                }
                _ => return Err(self.bad_plan()),
            }
        }
        Ok(true)
    }
    pub(crate) unsafe fn finish(
        mut self,
        outcome: Result<(), Fault<H::Error>>,
    ) -> Result<Value, Fault<H::Error>> {
        let deopt = self.materialized;
        self.flush();
        outcome?;
        let value = if deopt {
            // SAFETY: the value/effects of the external read are materialized;
            // continue strictly after it with the original counter and budget.
            unsafe {
                self.engine.eval_from(
                    self.first,
                    self.count,
                    self.next,
                    self.steps,
                    self.current,
                    self.maximum,
                    self.host,
                )
            }?
        } else {
            if self.next <= self.count {
                return Err(self.bad_plan());
            }
            self.engine.finish(self.current)?
        };
        // SAFETY: engine/arena/constant storage stays borrowed until the owned copy.
        Ok(unsafe { Value::copy_from(value) })
    }
}
