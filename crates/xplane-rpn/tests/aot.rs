use xplane_rpn::*;
#[path = "support/programs.rs"]
mod programs;
#[rustfmt::skip]
#[path="fixtures/aot_generated.rs"] mod compiled;

#[derive(Clone, Debug, PartialEq)]
struct Word(u8, u64, Vec<u8>);
fn word(v: &Value) -> Word {
    match v {
        Value::Nil => Word(0, 0, vec![]),
        Value::Number(n) => Word(1, n.to_bits(), vec![]),
        Value::Boolean(b) => Word(2, u64::from(*b), vec![]),
        Value::String(s) => Word(3, 0, s.clone()),
    }
}
#[derive(Clone, Debug, PartialEq)]
struct Event(u8, u32, Option<Word>, bool, u32);
#[derive(Clone)]
struct Trace {
    external: Value,
    local: f64,
    events: Vec<Event>,
    fail: usize,
    post_mode: Option<u32>,
}
impl Trace {
    fn event(
        &mut self,
        kind: u8,
        id: u32,
        value: Option<&Value>,
        present: bool,
    ) -> Result<(), usize> {
        self.events
            .push(Event(kind, id, value.map(word), present, csr()));
        if self.events.len() == self.fail {
            return Err(self.fail);
        }
        if let Some(mode) = self.post_mode {
            set_csr(mode);
        }
        Ok(())
    }
}
impl Host for Trace {
    type Error = usize;
    fn get_local(&mut self, id: u32) -> Result<f64, usize> {
        self.event(0, id, None, false)?;
        Ok(self.local)
    }
    fn set_local(&mut self, id: u32, v: &Value) -> Result<(), usize> {
        self.event(1, id, Some(v), true)?;
        if let Value::Number(v) = v {
            self.local = *v;
        }
        Ok(())
    }
    fn get_external(&mut self, id: u32) -> Result<Value, usize> {
        self.event(2, id, None, false)?;
        Ok(self.external.clone())
    }
    fn set_external(&mut self, id: u32, v: &Value, present: bool) -> Result<(), usize> {
        self.event(3, id, Some(v), present)
    }
}
fn csr() -> u32 {
    #[cfg(target_arch = "x86_64")]
    {
        let mut v = 0;
        // SAFETY: stmxcsr writes exactly the live four-byte scalar.
        unsafe {
            core::arch::asm!("stmxcsr [{}]",in(reg)&mut v,options(nostack,preserves_flags));
        }
        v
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        0x1f80
    }
}
fn set_csr(v: u32) {
    #[cfg(target_arch = "x86_64")]
    // SAFETY: every caller supplies a saved word or a fixed masked valid word.
    unsafe {
        core::arch::asm!("ldmxcsr [{}]",in(reg)&v,options(nostack,preserves_flags));
    }
    #[cfg(not(target_arch = "x86_64"))]
    let _ = v;
}
struct Restore(u32);
impl Drop for Restore {
    fn drop(&mut self) {
        set_csr(self.0);
    }
}
fn context(v: &Vm) -> (Vec<Word>, Vec<(u32, Word)>) {
    let c = v.context();
    (
        c.stack.iter().map(word).collect(),
        c.registers.iter().map(|(&k, v)| (k, word(v))).collect(),
    )
}
fn flattened() -> (Vec<Instruction>, Vec<Program>, Vec<usize>) {
    let programs = programs::programs();
    let slices: Vec<_> = programs.iter().map(Vec::as_slice).collect();
    let generated = aot::generate(&slices, "xplane_rpn").unwrap();
    assert_eq!(generated.rust, include_str!("fixtures/aot_generated.rs"));
    let mut code = vec![];
    let mut descriptors = vec![];
    for p in programs {
        descriptors.push(Program {
            first: code.len() as u32,
            count: p.len() as u32,
        });
        code.extend(p);
    }
    (code, descriptors, generated.program_ids)
}
#[test]
fn compiled_rust_matches_vm_values_context_callbacks_faults_budgets_and_fp_words() {
    let _restore = Restore(csr());
    let (code, programs, ids) = flattened();
    let (mut vm, mut native) = (
        Vm::new(&code, &programs).unwrap(),
        Vm::new(&code, &programs).unwrap(),
    );
    let modes = if cfg!(target_arch = "x86_64") {
        vec![0x1f80, 0x9f80, 0x1fc0, 0x9fc0, 0x3f80, 0x5f80]
    } else {
        vec![0x1f80]
    };
    for fast in [false, true] {
        for (index, &id) in ids.iter().enumerate() {
            let binding = native
                .bind_aot(index as u32, compiled::shape(id).unwrap())
                .unwrap();
            for external in [
                Value::Number(1.25),
                Value::Number(-0.),
                Value::Number(1e-320),
                Value::Number(f64::from_bits(0x7ff0000000001234)),
                Value::String(b"raw\0text".to_vec()),
                Value::Boolean(true),
                Value::Nil,
            ] {
                for &mode in &modes {
                    for budget in [0, 1, 2, 7, 128, 1000] {
                        for failure in [usize::MAX, 1, 3] {
                            for post in [None, Some(0x7fc1)] {
                                let seed = ExecutionContext {
                                    stack: vec![
                                        Value::Number(-0.),
                                        Value::String(b"seed\0".to_vec()),
                                    ],
                                    registers: [
                                        (7, Value::String(b"register".to_vec())),
                                        (1, Value::Number(f64::from_bits(0x7ff8000000001234))),
                                    ]
                                    .into(),
                                };
                                vm.set_context(&seed).unwrap();
                                native.set_context(&seed).unwrap();
                                let mut left = Trace {
                                    external: external.clone(),
                                    local: 3.,
                                    events: vec![],
                                    fail: failure,
                                    post_mode: post,
                                };
                                let mut right = left.clone();
                                set_csr(mode);
                                let a = vm
                                    .evaluate(index as u32, budget, &mut left)
                                    .map(|v| word(&v));
                                let fp = csr();
                                let c = context(&vm);
                                set_csr(mode);
                                let result = if fast {
                                    // SAFETY: the binding was checked against this
                                    // exact generated plan; the VM handles short
                                    // budgets/backward edges before invoking it.
                                    unsafe {
                                        native.evaluate_aot_prevalidated(
                                            &binding,
                                            budget,
                                            &mut right,
                                            |f| compiled::execute_prevalidated(id, f),
                                        )
                                    }
                                } else {
                                    native.evaluate_aot(&binding, budget, &mut right, |f| {
                                        compiled::execute(id, f)
                                    })
                                };
                                let b = result.map(|v| word(&v));
                                assert_eq!(a, b, "program {index}, budget {budget}, mode {mode:x}");
                                assert_eq!(
                                    fp,
                                    csr(),
                                    "FP program {index}, budget {budget}, mode {mode:x}"
                                );
                                assert_eq!(c, context(&native), "context program {index}");
                                assert_eq!(left.events, right.events, "callbacks program {index}");
                                assert_eq!(left.local.to_bits(), right.local.to_bits());
                            }
                        }
                    }
                }
            }
        }
    }
}
#[test]
fn bindings_reject_changed_code_other_vms_and_incorrect_generated_bodies() {
    let code = [Instruction::push(Value::Number(4.))];
    let programs = [Program { first: 0, count: 1 }];
    let mut vm = Vm::new(&code, &programs).unwrap();
    let mut other = Vm::new(&code, &programs).unwrap();
    assert!(vm.bind_aot(0, &[(OP_DUP, 0)]).is_none());
    assert!(vm.bind_aot(0, &[]).is_none());
    assert!(vm.bind_aot(1, &[(OP_PUSH, 0)]).is_none());
    let bound = vm.bind_aot(0, &[(OP_PUSH, 0)]).unwrap();
    let mut host = Trace {
        external: Value::Number(1.),
        local: 3.,
        events: vec![],
        fail: usize::MAX,
        post_mode: None,
    };
    assert_eq!(
        other
            .evaluate_aot(&bound, 10, &mut host, |_| Ok(()))
            .unwrap_err()
            .status,
        RPN_BAD_CALL
    );
    assert_eq!(
        vm.evaluate_aot(&bound, 10, &mut host, |_| Ok(()))
            .unwrap_err()
            .status,
        RPN_BAD_CALL
    );
    assert_eq!(
        vm.evaluate_aot(&bound, 10, &mut host, |f| f.step::<{ OP_GET_X }, 0>(1))
            .unwrap_err()
            .status,
        RPN_BAD_CALL
    );
    assert_eq!(
        vm.evaluate_aot(&bound, 10, &mut host, |f| f.step::<{ OP_PUSH }, 0>(2))
            .unwrap_err()
            .status,
        RPN_BAD_CALL
    );
    assert!(host.events.is_empty());
    assert_eq!(
        word(
            &vm.evaluate_aot(&bound, 10, &mut host, |f| f.step::<{ OP_PUSH }, 0>(1))
                .unwrap()
        ),
        word(&Value::Number(4.))
    );
    let changed = [Instruction::push(Value::String(
        b"current source literal".to_vec(),
    ))];
    let mut changed = Vm::new(&changed, &programs).unwrap();
    let binding = changed.bind_aot(0, &[(OP_PUSH, 0)]).unwrap();
    assert_eq!(
        word(
            &changed
                .evaluate_aot(&binding, 10, &mut host, |f| f.step::<{ OP_PUSH }, 0>(1))
                .unwrap()
        ),
        word(&Value::String(b"current source literal".to_vec()))
    );
    drop(vm);
    assert_eq!(
        other
            .evaluate_aot(&bound, 10, &mut host, |_| Ok(()))
            .unwrap_err()
            .status,
        RPN_BAD_CALL
    );
}
#[test]
fn generation_validates_shapes_and_never_embeds_runtime_literal_bytes() {
    let code = [Instruction::push(Value::String(
        b"private runtime text".to_vec(),
    ))];
    let generated = aot::generate(&[&code, &code], "xplane_rpn").unwrap();
    assert_eq!(generated.program_ids, [0, 0]);
    assert!(!generated.rust.contains("private runtime text"));
    assert!(matches!(
        aot::generate(&[&code], "x; bad"),
        Err(aot::GenerateError::InvalidRuntimePath)
    ));
    for code in [
        [Instruction::new(OP_REG_L, 50)],
        [Instruction::new(OP_JUMP, 0)],
        [Instruction::new(999, 0)],
    ] {
        assert!(matches!(
            aot::generate(&[&code], "xplane_rpn"),
            Err(aot::GenerateError::InvalidInstruction)
        ));
    }
}

#[test]
fn fast_mode_falls_back_before_callbacks_for_loops_and_short_budgets() {
    let (code, programs, ids) = flattened();
    for (program, budget) in [(1, 0), (1, 2), (5, 1000)] {
        let mut vm = Vm::new(&code, &programs).unwrap();
        let mut native = Vm::new(&code, &programs).unwrap();
        let bound = native
            .bind_aot(program, compiled::shape(ids[program as usize]).unwrap())
            .unwrap();
        let mut a = Trace {
            external: Value::Number(2.),
            local: 3.,
            events: vec![],
            fail: 3,
            post_mode: None,
        };
        let mut b = a.clone();
        let expected = vm.evaluate(program, budget, &mut a).map(|v| word(&v));
        // SAFETY: the verified binding's loop/short budget requires fallback;
        // this sentinel body must never run or touch the frame.
        let actual = unsafe {
            native.evaluate_aot_prevalidated(&bound, budget, &mut b, |_| {
                panic!("native body ran before fallback")
            })
        }
        .map(|v| word(&v));
        assert_eq!(expected, actual);
        assert_eq!(a.events, b.events);
    }
}

#[test]
fn numeric_regions_keep_live_literals_underflow_capacity_types_and_fp_status() {
    let _restore = Restore(csr());
    let (_, programs, ids) = flattened();
    for literal in [
        Value::Number(0.0),
        Value::Number(-0.0),
        Value::Number(1e-320),
        Value::Number(f64::INFINITY),
        Value::Number(f64::NEG_INFINITY),
        Value::Number(f64::from_bits(0x7ff0000000001234)),
        Value::Number(f64::from_bits(0x7ff8000000001234)),
        Value::Boolean(true),
        Value::String(b"changed literal".to_vec()),
        Value::Nil,
    ] {
        let mut code = vec![];
        for p in programs::programs() {
            code.extend(p.into_iter().map(|mut i| {
                if i.op == OP_PUSH {
                    i.value = literal.clone();
                }
                i
            }));
        }
        let mut vm = Vm::new(&code, &programs).unwrap();
        let mut native = Vm::new(&code, &programs).unwrap();
        for (index, &id) in ids.iter().enumerate() {
            let bound = native
                .bind_aot(index as u32, compiled::shape(id).unwrap())
                .unwrap();
            for stack in [
                vec![],
                vec![literal.clone()],
                vec![literal.clone(); 2],
                vec![literal.clone(); 80],
            ] {
                if matches!(literal, Value::Nil) && !stack.is_empty() {
                    continue;
                }
                for mode in [0x1f80, 0x9f80, 0x1fc0, 0x9fc0, 0x3f80, 0x5f80, 0x7f80] {
                    let seed = ExecutionContext {
                        stack: stack.clone(),
                        registers: Default::default(),
                    };
                    vm.set_context(&seed).unwrap();
                    native.set_context(&seed).unwrap();
                    let mut a = Trace {
                        external: Value::Number(1e-320),
                        local: 3.,
                        events: vec![],
                        fail: usize::MAX,
                        post_mode: Some(0x7fc1),
                    };
                    let mut b = a.clone();
                    set_csr(mode);
                    let expected = vm.evaluate(index as u32, 1000, &mut a).map(|v| word(&v));
                    let expected_fp = csr();
                    let expected_context = context(&vm);
                    set_csr(mode);
                    // SAFETY: id and binding describe exactly this generated program.
                    let actual = unsafe {
                        native.evaluate_aot_prevalidated(&bound, 1000, &mut b, |f| {
                            compiled::execute_prevalidated(id, f)
                        })
                    }
                    .map(|v| word(&v));
                    assert_eq!(
                        expected,
                        actual,
                        "program {index}, literal {literal:?}, seed {}, FP {mode:x}",
                        stack.len()
                    );
                    assert_eq!(
                        expected_fp,
                        csr(),
                        "FP program {index}, literal {literal:?}, seed {}, mode {mode:x}",
                        stack.len()
                    );
                    assert_eq!(
                        expected_context,
                        context(&native),
                        "context program {index}"
                    );
                    assert_eq!(a.events, b.events, "callbacks program {index}");
                }
            }
        }
    }
}
