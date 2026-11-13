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

/// A shape-checked binding belonging to one immutable VM instruction table.
/// Cloning the identity token retains no VM, seed, host, or aircraft resource.
#[derive(Clone)]
pub struct AotBinding {
    program: u32,
    count: u32,
    forward_only: bool,
    numeric: bool,
    numeric_literals: bool,
    identity: std::rc::Rc<()>,
}

/// Owns one interpreter and its stable string storage. Host calls use ordinary
/// Rust dispatch; panics unwind naturally when unwinding is enabled.
pub struct Vm {
    engine: Engine,
    // Seed strings stay live until the engine clears their borrowed descriptors.
    seed: ExecutionContext,
    identity: std::rc::Rc<()>,
    numeric_state: bool,
}
impl Vm {
    pub fn new(instructions: &[Instruction], programs: &[Program]) -> Result<Self, BuildError> {
        Ok(Self {
            engine: Engine::new(instructions, programs)?,
            seed: ExecutionContext::default(),
            identity: std::rc::Rc::new(()),
            numeric_state: true,
        })
    }
    /// Bind a generated opcode/argument shape once. Constants and host operands
    /// remain the current VM's data, so no emitted literal or variable can go stale.
    /// A mismatch selects no AOT binding; callers retain normal `evaluate`.
    pub fn bind_aot(&self, program: u32, shape: &[(u32, u32)]) -> Option<AotBinding> {
        self.engine
            .matches_shape(program, shape)
            .then(|| AotBinding {
                program,
                count: shape.len() as u32,
                forward_only: shape.iter().enumerate().all(|(pc, &(op, arg))| {
                    !matches!(op, crate::OP_JZ | crate::OP_JUMP) || u64::from(arg) > pc as u64 + 1
                }),
                identity: self.identity.clone(),
                numeric: crate::aot::numeric_heights(shape).is_some(),
                numeric_literals: self.engine.numeric_literals(program),
            })
    }
    /// Execute a generated body under a binding to this exact VM. The checked
    /// frame rejects wrong opcode/argument/control flow or premature completion.
    /// Faults, source indices, budgets, effects and owned results use the same
    /// opcode implementation as `evaluate`; host panics unwind unchanged.
    pub fn evaluate_aot<H: Host, F>(
        &mut self,
        binding: &AotBinding,
        max_steps: u32,
        host: &mut H,
        body: F,
    ) -> Result<Value, Fault<H::Error>>
    where
        F: FnOnce(&mut engine::AotFrame<'_, H>) -> Result<(), Fault<H::Error>>,
    {
        if !std::rc::Rc::ptr_eq(&binding.identity, &self.identity) {
            return Err(Fault {
                status: crate::RPN_BAD_CALL,
                instr: -1,
                host_error: None,
            });
        }
        self.numeric_state = false;
        let mut frame = engine::AotFrame::new(&mut self.engine, binding.program, max_steps, host);
        body(&mut frame)?;
        frame.finish()
    }
    /// Run a trusted generated plan without redundant per-instruction guards.
    /// An identity mismatch is rejected; backward branches or a short budget
    /// select the interpreter BEFORE executing any callback. Runtime operation
    /// faults are propagated directly and never retried.
    ///
    /// # Safety
    /// `body` must be generated for exactly the bound opcode/argument shape,
    /// follow all its source control-flow edges, and use the prevalidated frame
    /// operations only at their matching instruction positions. A binding alone
    /// cannot prove the correctness of arbitrary Rust body code. Use the checked
    /// `evaluate_aot` API for custom/untrusted bodies.
    pub unsafe fn evaluate_aot_prevalidated<H: Host, F>(
        &mut self,
        binding: &AotBinding,
        max_steps: u32,
        host: &mut H,
        body: F,
    ) -> Result<Value, Fault<H::Error>>
    where
        F: FnOnce(&mut engine::AotFrame<'_, H>) -> Result<(), Fault<H::Error>>,
    {
        if !std::rc::Rc::ptr_eq(&binding.identity, &self.identity) {
            return Err(Fault {
                status: crate::RPN_BAD_CALL,
                instr: -1,
                host_error: None,
            });
        }
        if !binding.forward_only || max_steps < binding.count {
            return self.evaluate(binding.program, max_steps, host);
        }
        self.numeric_state = false;
        let mut frame = engine::AotFrame::new(&mut self.engine, binding.program, max_steps, host);
        body(&mut frame)?;
        frame.finish()
    }
    /// Execute trusted generated typed code with an empty stack and numeric registers.
    /// Unsupported shapes, non-numeric constants/registers, a nonempty stack or
    /// short budgets select the VM before callbacks. A non-numeric external
    /// read resumes the VM AFTER that read, preserving its value and effects.
    ///
    /// # Safety
    /// body must be generated for this exact binding's shape and numeric stack
    /// heights; follow its source edges and stop immediately on deoptimization.
    pub unsafe fn evaluate_numeric<H: Host, F>(
        &mut self,
        binding: &AotBinding,
        max_steps: u32,
        host: &mut H,
        body: F,
    ) -> Result<Value, Fault<H::Error>>
    where
        F: FnOnce(&mut engine::NumericFrame<'_, H>) -> Result<(), Fault<H::Error>>,
    {
        if !std::rc::Rc::ptr_eq(&binding.identity, &self.identity) {
            return Err(Fault {
                status: crate::RPN_BAD_CALL,
                instr: -1,
                host_error: None,
            });
        }
        if !self.numeric_state
            || !self.engine.stack.is_empty()
            || !binding.numeric
            || !binding.numeric_literals
            || max_steps < binding.count
        {
            return self.evaluate(binding.program, max_steps, host);
        }
        self.numeric_state = false;
        let mut frame =
            engine::NumericFrame::new(&mut self.engine, binding.program, max_steps, host);
        let outcome = body(&mut frame);
        let numeric = outcome.is_ok() && !frame.deoptimized();
        // SAFETY: the trusted generated plan obeys shape/height/deopt contracts;
        // entry guards prove an empty stack and numeric or absent registers.
        let value = unsafe { frame.finish(outcome) };
        self.numeric_state = numeric && value.is_ok();
        value
    }
    pub fn begin(&mut self, zero_registers: bool) {
        self.engine.begin(zero_registers);
        self.numeric_state = true;
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
        self.numeric_state = self
            .seed
            .stack
            .iter()
            .all(|v| matches!(v, Value::Number(_)))
            && self
                .seed
                .registers
                .values()
                .all(|v| matches!(v, Value::Number(_) | Value::Nil));
        Ok(())
    }
    pub fn context(&self) -> ExecutionContext {
        ExecutionContext {
            stack: self.stack_values().collect(),
            registers: self.register_values().collect(),
        }
    }
    /// Copy stack values in bottom-to-top order without allocating a context.
    /// Each yielded value owns its strings independently of the VM.
    pub fn stack_values(&self) -> impl Iterator<Item = Value> + '_ {
        // SAFETY: constants, arena chunks and seed strings remain owned and
        // immutable for this borrow; every yielded value copies its bytes.
        self.engine
            .stack
            .iter()
            .map(|v| unsafe { Value::copy_from(*v) })
    }
    /// Copy present registers in ascending index order. Missing registers are
    /// omitted; each yielded value owns its strings independently of the VM.
    pub fn register_values(&self) -> impl Iterator<Item = (u32, Value)> + '_ {
        (0..50)
            .filter(|n| self.engine.present & (1u64 << n) != 0)
            // SAFETY: the immutable VM borrow keeps every descriptor live.
            .map(|n| {
                (n as u32, unsafe {
                    Value::copy_from(self.engine.registers[n])
                })
            })
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
        self.numeric_state = false;
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
