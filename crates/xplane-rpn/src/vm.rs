//! Owned interpreter, byte strings, contexts and typed Rust host calls.
use crate::engine::{self, Engine};
use std::collections::BTreeMap;
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Nil,
    Boolean(bool),
    Number(f64),
    String(Vec<u8>),
}
impl Default for Value {
    fn default() -> Self {
        Self::Number(0.)
    }
}
impl Value {
    fn descriptor(&self) -> engine::ValueRef {
        match self {
            Self::Nil => engine::scalar(engine::NIL, 0.),
            Self::Boolean(value) => engine::scalar(engine::BOOLEAN, f64::from(*value)),
            Self::Number(number) => engine::num(*number),
            Self::String(text) => engine::ValueRef {
                kind: engine::STRING,
                number: 0.,
                text: text.as_ptr(),
                len: text.len(),
            },
        }
    }
    // All callers receive values from a live, exclusively borrowed VM engine.
    pub(super) unsafe fn copy_from(value: engine::ValueRef) -> Self {
        if value.kind == engine::NIL {
            Self::Nil
        } else if value.kind == engine::BOOLEAN {
            Self::Boolean(value.number != 0.)
        } else if value.kind == engine::STRING {
            Self::String(unsafe { engine::bytes(&value) }.to_vec())
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
            op: crate::OP_PUSH,
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

/// Owns one interpreter and its stable string storage. Host calls use ordinary
/// Rust dispatch; panics unwind naturally when unwinding is enabled.
pub struct Vm {
    engine: Engine,
    // Seed strings stay live until the engine clears their borrowed descriptors.
    seed: ExecutionContext,
}
impl Vm {
    pub fn new(instructions: &[Instruction], programs: &[Program]) -> Result<Self, BuildError> {
        Ok(Self {
            engine: Engine::new(instructions, programs)?,
            seed: ExecutionContext::default(),
        })
    }
    pub fn begin(&mut self, zero_registers: bool) {
        self.engine.begin(zero_registers);
        self.seed = ExecutionContext::default();
    }
    /// Invalid indices leave the old context unchanged; allocation failure
    /// while installing the context can leave it partial.
    pub fn set_context(&mut self, context: &ExecutionContext) -> Result<(), BuildError> {
        if context.registers.keys().any(|index| *index >= 50)
            || context.stack.iter().any(|v| matches!(v, Value::Nil))
        {
            return Err(BuildError::InvalidDescriptor);
        }
        let seed = context.clone();
        self.begin(false);
        self.seed = seed;
        for value in &self.seed.stack {
            self.engine.seed_stack(value.descriptor())?;
        }
        for (&index, value) in &self.seed.registers {
            self.engine.seed_register(index, value.descriptor());
        }
        Ok(())
    }
    pub fn context(&self) -> ExecutionContext {
        // SAFETY: constants, arena chunks and seed strings remain owned and
        // immutable for this borrow; the returned snapshot copies every byte.
        unsafe {
            ExecutionContext {
                stack: self
                    .engine
                    .stack
                    .iter()
                    .map(|v| Value::copy_from(*v))
                    .collect(),
                registers: (0..50)
                    .filter(|n| self.engine.present & (1u64 << n) != 0)
                    .map(|n| (n as u32, Value::copy_from(self.engine.registers[n])))
                    .collect(),
            }
        }
    }
    pub fn registers_dirty(&self) -> bool {
        self.engine.dirty
    }
    /// Earlier effects remain on error; context after error or panic is unspecified.
    pub fn evaluate<H: Host>(
        &mut self,
        program: u32,
        max_steps: u32,
        host: &mut H,
    ) -> Result<Value, Fault<H::Error>> {
        // SAFETY: exclusive engine access; external strings are copied into
        // its arena before the next typed host call. Results stay engine-owned
        // until this method copies them into an independent value.
        unsafe {
            self.engine
                .eval(program, max_steps, host)
                .map(|value| Value::copy_from(value))
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::*;
    #[derive(Default)]
    struct Trace {
        calls: Vec<(u32, f64)>,
        fail: bool,
    }
    impl Host for Trace {
        type Error = ();
        fn get_local(&mut self, id: u32) -> Result<f64, ()> {
            self.calls.push((id, 9.));
            Ok(9.)
        }
        fn set_local(&mut self, id: u32, value: &Value) -> Result<(), ()> {
            let Value::Number(n) = value else { panic!() };
            self.calls.push((id, *n));
            if self.fail { Err(()) } else { Ok(()) }
        }
        fn get_external(&mut self, _: u32) -> Result<Value, ()> {
            Ok(Value::Number(0.))
        }
        fn set_external(&mut self, _: u32, _: &Value, _: bool) -> Result<(), ()> {
            Ok(())
        }
    }
    fn vm(code: &[Instruction]) -> Vm {
        Vm::new(
            code,
            &[Program {
                first: 0,
                count: code.len() as u32,
            }],
        )
        .unwrap()
    }
    #[test]
    fn mixed_host_operations_keep_callback_order_and_exact_fault_position() {
        let mut vm = vm(&[
            Instruction::new(OP_GET_L, 0),
            Instruction::push(Value::Number(2.)),
            Instruction::new(OP_ADD, 0),
            Instruction::new(OP_DUP, 0),
            Instruction::new(OP_SET_L, 0),
        ]);
        let mut host = Trace::default();
        vm.begin(true);
        assert_eq!(vm.evaluate(0, 100, &mut host).unwrap(), Value::Number(11.));
        assert_eq!(host.calls, [(0, 9.), (4, 11.)]);
        host.calls.clear();
        host.fail = true;
        vm.begin(true);
        let fault = vm.evaluate(0, 100, &mut host).unwrap_err();
        assert_eq!((fault.status, fault.instr), (RPN_HOST, 4));
        assert_eq!(host.calls, [(0, 9.), (4, 11.)]);
    }
    #[test]
    fn string_constants_are_owned_independently_of_instruction_storage() {
        let mut code = [
            Instruction::push(Value::String(b"AB".to_vec())),
            Instruction::push(Value::String(b"AB".to_vec())),
            Instruction::new(OP_SCMP, 0),
        ];
        let mut vm = vm(&code);
        if let Value::String(bytes) = &mut code[0].value {
            bytes.fill(b'Z');
        }
        drop(code);
        vm.begin(false);
        assert_eq!(
            vm.evaluate(0, 100, &mut Trace::default()).unwrap(),
            Value::Number(0.)
        );
    }
    #[test]
    fn invalid_registers_and_unbounded_jumps_are_rejected() {
        assert!(matches!(
            Vm::new(
                &[Instruction::new(OP_REG_L, 50)],
                &[Program { first: 0, count: 1 }]
            ),
            Err(BuildError::InvalidDescriptor)
        ));
        let mut vm = vm(&[Instruction::new(OP_JUMP, 1)]);
        vm.begin(true);
        let fault = vm.evaluate(0, 5, &mut Trace::default()).unwrap_err();
        assert_eq!((fault.status, fault.instr), (RPN_STEPS, 0));
    }
}
