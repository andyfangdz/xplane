use std::{
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
};
use xplane_rpn::*;

#[derive(Default)]
struct TestHost {
    reads: u8,
    effects: Vec<(u32, Value, bool)>,
    fail: Option<u32>,
    panic: bool,
    external: Option<Value>,
}
impl Host for TestHost {
    type Error = &'static str;
    fn get_local(&mut self, _: u32) -> Result<f64, Self::Error> {
        if self.panic {
            panic!("owned host panic");
        }
        Ok(7.)
    }
    fn set_local(&mut self, id: u32, value: &Value) -> Result<(), Self::Error> {
        self.effects.push((id, value.clone(), true));
        if self.fail == Some(id) {
            Err("local failure")
        } else {
            Ok(())
        }
    }
    fn get_external(&mut self, _: u32) -> Result<Value, Self::Error> {
        self.reads += 1;
        Ok(self
            .external
            .clone()
            .unwrap_or_else(|| Value::String(vec![255, 0, self.reads])))
    }
    fn set_external(&mut self, id: u32, value: &Value, present: bool) -> Result<(), Self::Error> {
        self.effects.push((id, value.clone(), present));
        if self.fail == Some(id) {
            Err("event failure")
        } else {
            Ok(())
        }
    }
}
fn machine(instructions: &[Instruction]) -> xplane_rpn::Vm {
    xplane_rpn::Vm::new(
        instructions,
        &[Program {
            first: 0,
            count: instructions.len() as u32,
        }],
    )
    .unwrap()
}

#[test]
fn constants_and_seed_strings_outlive_their_callers_and_machine_moves() {
    let mut vm = {
        let code = [Instruction::push(Value::String(vec![0xff, 0, 0xfe]))];
        machine(&code)
    };
    let mut host = TestHost::default();
    assert_eq!(
        vm.evaluate(0, 100, &mut host).unwrap(),
        Value::String(vec![255, 0, 254])
    );
    let mut empty = machine(&[]);
    {
        let seed = ExecutionContext {
            stack: vec![Value::String(vec![1, 0, 2])],
            registers: BTreeMap::from([(49, Value::String(vec![3, 0, 4]))]),
        };
        empty.set_context(&seed).unwrap();
    }
    let mut moved = Box::new(empty);
    assert_eq!(
        moved.evaluate(0, 100, &mut host).unwrap(),
        Value::String(vec![1, 0, 2])
    );
    let snapshot = moved.context();
    drop(moved);
    assert_eq!(
        snapshot.registers.get(&49),
        Some(&Value::String(vec![3, 0, 4]))
    );
}

#[test]
fn booleans_keep_their_type_through_registers_effects_and_equality() {
    let mut vm = machine(&[
        Instruction::push(Value::Boolean(true)),
        Instruction::new(OP_REG_S, 7),
        Instruction::new(OP_SET_X, 0),
        Instruction::new(OP_REG_L, 7),
        Instruction::push(Value::Number(1.)),
        Instruction::new(OP_EQ, 0),
    ]);
    let mut host = TestHost::default();
    assert_eq!(vm.evaluate(0, 100, &mut host).unwrap(), Value::Number(0.));
    assert_eq!(host.effects, vec![(2, Value::Boolean(true), true)]);
    assert_eq!(vm.context().registers[&7], Value::Boolean(true));
    let mut same = machine(&[
        Instruction::push(Value::Boolean(false)),
        Instruction::push(Value::Boolean(false)),
        Instruction::new(OP_EQ, 0),
    ]);
    assert_eq!(same.evaluate(0, 100, &mut host).unwrap(), Value::Number(1.));
    let mut numeric = machine(&[
        Instruction::push(Value::Boolean(true)),
        Instruction::new(OP_INC, 0),
    ]);
    let fault = numeric.evaluate(0, 100, &mut host).unwrap_err();
    assert_eq!((fault.status, fault.instr), (RPN_NUMERIC_BOOLEAN, 1));
}

#[test]
fn missing_values_clear_register_seeds_and_reject_dense_stack_or_push() {
    let mut vm = machine(&[Instruction::push(Value::Nil)]);
    vm.set_context(&ExecutionContext {
        stack: vec![Value::Boolean(false)],
        registers: BTreeMap::from([(4, Value::Nil), (8, Value::Boolean(true))]),
    })
    .unwrap();
    let snapshot = vm.context();
    assert_eq!(
        snapshot.registers,
        BTreeMap::from([(8, Value::Boolean(true))])
    );
    assert_eq!(
        vm.set_context(&ExecutionContext {
            stack: vec![Value::Nil],
            registers: BTreeMap::new(),
        }),
        Err(BuildError::InvalidDescriptor)
    );
    assert_eq!(vm.context(), snapshot);
    let fault = vm.evaluate(0, 100, &mut TestHost::default()).unwrap_err();
    assert_eq!((fault.status, fault.instr), (RPN_MISSING, 0));
}

#[test]
fn external_booleans_are_typed_and_absent_values_fault_at_the_read() {
    let mut vm = machine(&[Instruction::new(OP_GET_X, 0)]);
    let mut host = TestHost {
        external: Some(Value::Boolean(false)),
        ..Default::default()
    };
    assert_eq!(
        vm.evaluate(0, 10, &mut host).unwrap(),
        Value::Boolean(false)
    );
    host.external = Some(Value::Nil);
    let fault = vm.evaluate(0, 10, &mut host).unwrap_err();
    assert_eq!((fault.status, fault.instr), (RPN_MISSING, 0));
}

#[test]
fn external_strings_are_copied_before_later_callbacks_and_snapshots_own_bytes() {
    let mut vm = machine(&[
        Instruction::new(OP_GET_X, 0),
        Instruction::new(OP_REG_S, 7),
        Instruction::new(OP_GET_X, 0),
    ]);
    let mut host = TestHost::default();
    assert_eq!(
        vm.evaluate(0, 100, &mut host).unwrap(),
        Value::String(vec![255, 0, 2])
    );
    let snapshot = vm.context();
    assert_eq!(snapshot.stack, vec![Value::String(vec![255, 0, 1])]);
    assert_eq!(
        snapshot.registers.get(&7),
        Some(&Value::String(vec![255, 0, 1]))
    );
    vm.begin(false);
    drop(vm);
    assert_eq!(snapshot.stack, vec![Value::String(vec![255, 0, 1])]);
}

#[test]
fn sparse_register_presence_signed_zero_and_invalid_seed_preserve_context() {
    let mut vm = machine(&[Instruction::new(OP_REG_L, 0)]);
    vm.set_context(&ExecutionContext {
        stack: vec![],
        registers: BTreeMap::from([(49, Value::Number(-0.))]),
    })
    .unwrap();
    assert!(!vm.registers_dirty());
    let before = vm.context();
    let invalid = ExecutionContext {
        stack: vec![],
        registers: BTreeMap::from([(50, Value::Number(1.))]),
    };
    assert_eq!(vm.set_context(&invalid), Err(BuildError::InvalidDescriptor));
    assert_eq!(vm.context(), before);
    let Value::Number(value) = vm.context().registers[&49] else {
        panic!()
    };
    assert_eq!(value.to_bits(), (-0f64).to_bits());
    assert_eq!(
        vm.evaluate(0, 100, &mut TestHost::default()).unwrap(),
        Value::Number(0.)
    );
    assert!(vm.registers_dirty());
    assert_eq!(vm.context().registers.len(), 2);
    vm.begin(true);
    assert_eq!(vm.context().registers.len(), 50);
    assert!(!vm.registers_dirty());
}

#[test]
fn host_fault_keeps_ordered_effects_instruction_and_payload_presence() {
    let mut vm = machine(&[
        Instruction::push(Value::Number(1.)),
        Instruction::new(OP_SET_L, 0),
        Instruction::new(OP_SET_K, 0),
        Instruction::push(Value::Number(0.)),
        Instruction::new(OP_SET_K, 0),
    ]);
    let mut host = TestHost {
        fail: Some(4),
        ..Default::default()
    };
    let error = vm.evaluate(0, 100, &mut host).unwrap_err();
    assert_eq!(
        (error.status, error.instr, error.host_error),
        (RPN_HOST, 4, Some("event failure"))
    );
    assert_eq!(
        host.effects,
        vec![
            (1, Value::Number(1.), true),
            (2, Value::Number(0.), false),
            (4, Value::Number(0.), true)
        ]
    );
    vm.begin(false);
    host.fail = None;
    host.effects.clear();
    assert_eq!(
        vm.evaluate(99, 100, &mut host).unwrap_err().status,
        RPN_BAD_CALL
    );
    assert!(host.effects.is_empty());
}

#[test]
fn host_panic_unwinds_directly_and_vm_can_be_reset() {
    let mut vm = machine(&[Instruction::new(OP_GET_L, 0)]);
    let mut host = TestHost {
        panic: true,
        ..Default::default()
    };
    let panic = catch_unwind(AssertUnwindSafe(|| vm.evaluate(0, 100, &mut host))).unwrap_err();
    assert_eq!(panic.downcast_ref::<&str>(), Some(&"owned host panic"));
    vm.begin(false);
    host.panic = false;
    assert_eq!(vm.evaluate(0, 100, &mut host).unwrap(), Value::Number(7.));
}

#[test]
fn descriptor_validation_budget_and_persistent_capacity_reuse_existing_engine() {
    for i in [
        Instruction::new(OP_REG_L, 50),
        Instruction::new(OP_JUMP, 0),
        Instruction::new(63, 0),
    ] {
        assert!(matches!(
            xplane_rpn::Vm::new(&[i], &[]),
            Err(BuildError::InvalidDescriptor)
        ));
    }
    assert!(matches!(
        xplane_rpn::Vm::new(&[], &[Program { first: 1, count: 0 }]),
        Err(BuildError::InvalidDescriptor)
    ));
    let mut vm = machine(&[Instruction::new(OP_JUMP, 1)]);
    let fault = vm.evaluate(0, 5, &mut TestHost::default()).unwrap_err();
    assert_eq!((fault.status, fault.instr), (RPN_STEPS, 0));
    let mut vm = machine(&[]);
    vm.set_context(&ExecutionContext {
        stack: vec![Value::Number(0.); 33],
        registers: BTreeMap::new(),
    })
    .unwrap();
    assert_eq!(
        vm.evaluate(0, 100, &mut TestHost::default())
            .unwrap_err()
            .status,
        RPN_CAPACITY
    );
}
