//! Safe owned values, contexts and host callbacks over the existing VM engine.
use crate::{self as raw, RpnFault, RpnHost, RpnInstr, RpnProgram, RpnValue};
use std::{
    any::Any,
    collections::BTreeMap,
    ffi::c_void,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    ptr::NonNull,
};

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Number(f64),
    String(Vec<u8>),
}
impl Default for Value {
    fn default() -> Self {
        Self::Number(0.)
    }
}
impl Value {
    fn descriptor(&self) -> RpnValue {
        match self {
            Self::Number(number) => raw::num(*number),
            Self::String(text) => RpnValue {
                kind: raw::STRING,
                number: 0.,
                text: text.as_ptr(),
                len: text.len(),
            },
        }
    }
    // All callers receive values from a live, exclusively borrowed VM engine.
    unsafe fn copy_from(value: RpnValue) -> Self {
        if value.kind == raw::STRING {
            Self::String(unsafe { raw::bytes(&value) }.to_vec())
        } else {
            Self::Number(value.number)
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Instruction {
    pub op: u32,
    pub arg: u32,
    pub value: Value,
}
impl Instruction {
    pub fn new(op: u32, arg: u32) -> Self {
        Self {
            op,
            arg,
            value: Value::default(),
        }
    }
    pub fn push(value: Value) -> Self {
        Self {
            op: raw::OP_PUSH,
            arg: 0,
            value,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Program {
    pub first: u32,
    pub count: u32,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ExecutionContext {
    /// Bottom of the persistent stack first.
    pub stack: Vec<Value>,
    /// Only present registers appear; absent differs from an explicit +0.
    pub registers: BTreeMap<u32, Value>,
}
pub trait Host {
    type Error;
    fn get_local(&mut self, instruction: u32) -> Result<f64, Self::Error>;
    fn set_local(&mut self, instruction: u32, value: &Value) -> Result<(), Self::Error>;
    fn get_external(&mut self, instruction: u32) -> Result<Value, Self::Error>;
    fn set_external(
        &mut self,
        instruction: u32,
        value: &Value,
        payload_present: bool,
    ) -> Result<(), Self::Error>;
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuildError {
    InvalidDescriptor,
    Allocation,
}
#[derive(Debug, PartialEq)]
pub struct Fault<E> {
    pub status: u32,
    pub instr: i64,
    pub host_error: Option<E>,
}

/// Owns one existing interpreter instance. The low-level ABI and this wrapper
/// execute the same engine. No instruction or numeric behavior is duplicated.
pub struct Vm {
    raw: NonNull<raw::Vm>,
    // The raw context ABI borrows seeded strings. Keep their allocations alive
    // until the next reset, including when evaluation fails or a host panics.
    seed: ExecutionContext,
}
impl Vm {
    pub fn from_owned(
        instructions: &[Instruction],
        programs: &[Program],
    ) -> Result<Self, BuildError> {
        if instructions.len() > u32::MAX as usize
            || instructions.iter().any(|i| {
                i.op >= raw::OP_COUNT
                    || (matches!(i.op, raw::OP_JZ | raw::OP_JUMP) && i.arg == 0)
                    || (matches!(i.op, raw::OP_REG_L | raw::OP_REG_SP | raw::OP_REG_S)
                        && i.arg >= 50)
            })
            || programs
                .iter()
                .any(|p| u64::from(p.first) + u64::from(p.count) > instructions.len() as u64)
        {
            return Err(BuildError::InvalidDescriptor);
        }
        let mut descriptors = Vec::new();
        let mut table = Vec::new();
        descriptors
            .try_reserve_exact(instructions.len())
            .map_err(|_| BuildError::Allocation)?;
        table
            .try_reserve_exact(programs.len())
            .map_err(|_| BuildError::Allocation)?;
        for i in instructions {
            descriptors.push(RpnInstr {
                op: i.op,
                arg: i.arg,
                value: i.value.descriptor(),
            });
        }
        for p in programs {
            table.push(RpnProgram {
                first: p.first,
                count: p.count,
            });
        }
        // SAFETY: Descriptor slices and their borrowed bytes live for the call;
        // the established constructor validates and copies constants.
        let raw = NonNull::new(unsafe {
            raw::rpn_vm_new(
                descriptors.as_ptr(),
                descriptors.len(),
                table.as_ptr(),
                table.len(),
            )
        })
        .ok_or(BuildError::Allocation)?;
        Ok(Self {
            raw,
            seed: ExecutionContext::default(),
        })
    }
    pub fn begin(&mut self, zero_registers: bool) {
        // SAFETY: This wrapper exclusively owns a live VM; reset removes all
        // raw values before their owning seed allocations are released.
        unsafe { raw::rpn_begin(self.raw.as_ptr(), i32::from(zero_registers)) };
        self.seed = ExecutionContext::default();
    }
    /// Copies seed strings into this owner. Invalid register indices leave the
    /// old context unchanged; allocation failure during seeding leaves it partial.
    pub fn set_context(&mut self, context: &ExecutionContext) -> Result<(), BuildError> {
        if context.registers.keys().any(|index| *index >= 50) {
            return Err(BuildError::InvalidDescriptor);
        }
        let seed = context.clone();
        self.begin(false);
        self.seed = seed;
        for value in &self.seed.stack {
            // SAFETY: These owned allocations stay in self.seed until reset.
            if unsafe { raw::rpn_context_push(self.raw.as_ptr(), &value.descriptor()) } != 0 {
                return Err(BuildError::Allocation);
            }
        }
        for (&index, value) in &self.seed.registers {
            // SAFETY: Indices were checked and string storage remains owned.
            if unsafe { raw::rpn_context_register(self.raw.as_ptr(), index, &value.descriptor()) }
                != 0
            {
                return Err(BuildError::Allocation);
            }
        }
        Ok(())
    }
    pub fn context(&self) -> ExecutionContext {
        // SAFETY: No operation mutates this live owner during inspection; every
        // borrowed byte string is copied before returning to the caller.
        unsafe {
            let mut count = 0;
            let stack = raw::rpn_stack(self.raw.as_ptr(), &mut count);
            let stack = std::slice::from_raw_parts(stack, count)
                .iter()
                .map(|v| Value::copy_from(*v))
                .collect();
            let (mut present, mut dirty) = (0, 0);
            let registers = raw::rpn_registers(self.raw.as_ptr(), &mut present, &mut dirty);
            let registers = (0..50)
                .filter(|n| present & (1u64 << n) != 0)
                .map(|n| (n as u32, Value::copy_from(*registers.add(n))))
                .collect();
            ExecutionContext { stack, registers }
        }
    }
    pub fn registers_dirty(&self) -> bool {
        let (mut present, mut dirty) = (0, 0);
        // SAFETY: The owner is live and exclusively immutable during inspection.
        unsafe { raw::rpn_registers(self.raw.as_ptr(), &mut present, &mut dirty) };
        dirty != 0
    }
    /// Earlier host effects remain visible on failure. As with the raw API, a
    /// failed evaluation leaves the context unspecified. With unwinding enabled,
    /// host panics resume after the C callback returns, never across that ABI.
    pub fn evaluate<H: Host>(
        &mut self,
        program: u32,
        max_steps: u32,
        host: &mut H,
    ) -> Result<Value, Fault<H::Error>> {
        let mut bridge = Bridge {
            host,
            error: None,
            panic: None,
            returned: Value::default(),
        };
        let callbacks = RpnHost {
            context: (&mut bridge as *mut Bridge<'_, H>).cast(),
            get_local: Some(get_local::<H>),
            set_local: Some(set_local::<H>),
            get_external: Some(get_external::<H>),
            set_external: Some(set_external::<H>),
        };
        let mut result = raw::ZERO;
        let mut fault = RpnFault {
            status: 0,
            instr: -1,
        };
        // SAFETY: The owner is exclusive; all callbacks use the synchronous
        // bridge and keep external strings alive until the engine copies them.
        let status = unsafe {
            raw::rpn_eval(
                self.raw.as_ptr(),
                program,
                max_steps,
                &callbacks,
                &mut result,
                &mut fault,
            )
        };
        if let Some(panic) = bridge.panic.take() {
            resume_unwind(panic);
        }
        if status != raw::RPN_OK {
            return Err(Fault {
                status,
                instr: fault.instr,
                host_error: bridge.error,
            });
        }
        // SAFETY: The result is still backed by this VM's live owned storage.
        Ok(unsafe { Value::copy_from(result) })
    }
}
impl Drop for Vm {
    fn drop(&mut self) {
        // SAFETY: Only this wrapper releases the constructor's CRT allocation.
        unsafe { raw::rpn_vm_free(self.raw.as_ptr()) };
    }
}
struct Bridge<'a, H: Host> {
    host: &'a mut H,
    error: Option<H::Error>,
    panic: Option<Box<dyn Any + Send>>,
    returned: Value,
}
unsafe fn invoke<H: Host>(
    context: *mut c_void,
    action: impl FnOnce(&mut Bridge<'_, H>) -> Result<(), H::Error>,
) -> i32 {
    // SAFETY: The enclosing evaluation owns this bridge for every callback.
    let bridge = unsafe { &mut *context.cast::<Bridge<'_, H>>() };
    match catch_unwind(AssertUnwindSafe(|| action(bridge))) {
        Ok(Ok(())) => 0,
        Ok(Err(error)) => {
            bridge.error = Some(error);
            1
        }
        Err(panic) => {
            bridge.panic = Some(panic);
            1
        }
    }
}
unsafe extern "C" fn get_local<H: Host>(context: *mut c_void, id: u32, out: *mut f64) -> i32 {
    // SAFETY: The engine supplies valid output storage for this callback.
    unsafe {
        invoke::<H>(context, |b| {
            *out = b.host.get_local(id)?;
            Ok(())
        })
    }
}
unsafe extern "C" fn set_local<H: Host>(
    context: *mut c_void,
    id: u32,
    value: *const RpnValue,
) -> i32 {
    // SAFETY: Values originate in the live engine; copy before calling the host.
    unsafe { invoke::<H>(context, |b| b.host.set_local(id, &Value::copy_from(*value))) }
}
unsafe extern "C" fn get_external<H: Host>(
    context: *mut c_void,
    id: u32,
    out: *mut RpnValue,
) -> i32 {
    // SAFETY: returned owns the bytes until the next callback; the engine
    // immediately copies them into its arena before any further host call.
    unsafe {
        invoke::<H>(context, |b| {
            b.returned = b.host.get_external(id)?;
            *out = b.returned.descriptor();
            Ok(())
        })
    }
}
unsafe extern "C" fn set_external<H: Host>(
    context: *mut c_void,
    id: u32,
    value: *const RpnValue,
    present: i32,
) -> i32 {
    // SAFETY: The live engine supplies the value; no raw pointer reaches Host.
    unsafe {
        invoke::<H>(context, |b| {
            b.host
                .set_external(id, &Value::copy_from(*value), present != 0)
        })
    }
}
