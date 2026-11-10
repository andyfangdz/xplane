//! Synthetic regressions for the observed P3D compatibility subset.
//! These programs contain no aircraft expressions or captured vendor fixtures.
use xplane_rpn::*;
fn instruction(op: u32) -> Instruction {
    Instruction::new(op, 0)
}
fn literal(value: f64) -> Instruction {
    Instruction::push(Value::Number(value))
}
#[derive(Debug, PartialEq)]
enum Effect {
    Local(u32, f64),
    External(u32, f64, bool),
}
#[derive(Default)]
struct Host {
    effects: Vec<Effect>,
    fail_at: Option<u32>,
}
fn number(value: &Value) -> f64 {
    let Value::Number(value) = value else {
        panic!("expected numeric value")
    };
    *value
}
impl xplane_rpn::Host for Host {
    type Error = ();
    fn get_local(&mut self, _: u32) -> Result<f64, ()> {
        Ok(0.)
    }
    fn get_external(&mut self, _: u32) -> Result<Value, ()> {
        Ok(Value::Number(0.))
    }
    fn set_local(&mut self, id: u32, value: &Value) -> Result<(), ()> {
        self.effects.push(Effect::Local(id, number(value)));
        if self.fail_at == Some(id) {
            Err(())
        } else {
            Ok(())
        }
    }
    fn set_external(&mut self, id: u32, value: &Value, present: bool) -> Result<(), ()> {
        self.effects
            .push(Effect::External(id, number(value), present));
        if self.fail_at == Some(id) {
            Err(())
        } else {
            Ok(())
        }
    }
}
struct Machine(Vm);
impl Machine {
    fn new(instructions: &[Instruction], programs: &[Program]) -> Self {
        Self(Vm::new(instructions, programs).unwrap())
    }
    fn single(instructions: &[Instruction]) -> Self {
        Self::new(
            instructions,
            &[Program {
                first: 0,
                count: instructions.len() as u32,
            }],
        )
    }
    fn begin(&mut self, zero: bool) {
        self.0.begin(zero);
    }
    fn evaluate(&mut self, program: u32, host: &mut Host) -> Result<f64, (u32, i64)> {
        self.0
            .evaluate(program, 100, host)
            .map(|v| number(&v))
            .map_err(|f| (f.status, f.instr))
    }
    fn stack(&self) -> Vec<f64> {
        self.0.context().stack.iter().map(number).collect()
    }
}
fn evaluate(instructions: &[Instruction]) -> f64 {
    let mut machine = Machine::single(instructions);
    machine.begin(false);
    machine.evaluate(0, &mut Host::default()).unwrap()
}

#[test]
fn integer_conversion_and_floor_differ_for_negative_fractions() {
    for (op, input, expected) in [
        (OP_INT, -2.2, -2.0),
        (OP_INT, 2.2, 2.0),
        (OP_FLR, -2.2, -3.0),
        (OP_FLR, 2.2, 2.0),
    ] {
        assert_eq!(evaluate(&[literal(input), instruction(op)]), expected);
    }
}

#[test]
fn near_rounds_halfway_away_from_zero() {
    for (input, expected) in [(-2.5, -3.0), (2.5, 3.0), (-2.25, -2.0), (2.25, 2.0)] {
        assert_eq!(evaluate(&[literal(input), instruction(OP_NEAR)]), expected);
    }
}

#[test]
fn integer_division_truncates_the_quotient_toward_zero() {
    for (a, b, expected) in [
        (-5.0, 2.0, -2.0),
        (5.0, -2.0, -2.0),
        (-5.0, -2.0, 2.0),
        (5.9, 1.9, 3.0),
    ] {
        assert_eq!(
            evaluate(&[literal(a), literal(b), instruction(OP_IDIV)]),
            expected
        );
    }
}

#[test]
fn division_by_either_signed_zero_returns_positive_zero() {
    for op in [OP_DIV, OP_IDIV] {
        for denominator in [0.0, -0.0] {
            let value = evaluate(&[literal(-5.0), literal(denominator), instruction(op)]);
            assert_eq!(value.to_bits(), 0.0f64.to_bits());
        }
    }
}

#[test]
fn numeric_pushes_normalize_nan_but_preserve_infinity_and_signed_zero() {
    for (input, expected) in [
        (f64::NAN, 0.0),
        (f64::INFINITY, f64::INFINITY),
        (f64::NEG_INFINITY, f64::NEG_INFINITY),
        (-0.0, -0.0),
    ] {
        assert_eq!(evaluate(&[literal(input)]).to_bits(), expected.to_bits());
    }
    assert_eq!(
        evaluate(&[literal(-1.0), instruction(OP_SQRT)]).to_bits(),
        0.0f64.to_bits()
    );
}

#[test]
fn empty_stack_results_and_missing_registers_return_positive_zero() {
    for instructions in [
        vec![],
        vec![instruction(OP_REG_L)],
        vec![instruction(OP_DUP)],
        vec![literal(7.0), instruction(OP_POP)],
    ] {
        assert_eq!(evaluate(&instructions).to_bits(), 0.0f64.to_bits());
    }
}

#[test]
fn consecutive_programs_share_remaining_stack_and_registers_until_reset() {
    // Program 0 leaves 10 on the stack, stores 7 in register 0 and returns 3.
    // Program 1 consumes that retained stack/register state, returning 17.
    let instructions = [
        literal(10.0),
        literal(7.0),
        instruction(OP_REG_SP),
        literal(3.0),
        instruction(OP_REG_L),
        instruction(OP_ADD),
    ];
    let mut machine = Machine::new(
        &instructions,
        &[
            Program { first: 0, count: 4 },
            Program { first: 4, count: 2 },
        ],
    );
    let mut host = Host::default();
    machine.begin(false);
    assert_eq!(machine.evaluate(0, &mut host), Ok(3.0));
    assert_eq!(machine.stack(), [10.0]);
    assert_eq!(machine.evaluate(1, &mut host), Ok(17.0));
    assert!(machine.stack().is_empty());
    for zero_registers in [false, true] {
        assert_eq!(machine.evaluate(0, &mut host), Ok(3.0));
        assert_eq!(machine.stack(), [10.0]);
        machine.begin(zero_registers);
        assert!(machine.stack().is_empty());
        assert_eq!(machine.evaluate(1, &mut host), Ok(0.0));
        assert!(machine.stack().is_empty());
    }
}

#[test]
fn writes_and_events_preserve_order_and_distinguish_absent_from_zero_payload() {
    let instructions = [
        literal(1.0),
        instruction(OP_SET_L),
        instruction(OP_SET_K),
        literal(0.0),
        instruction(OP_SET_K),
        literal(42.0),
        instruction(OP_SET_K),
        literal(8.0),
        instruction(OP_SET_X),
    ];
    let mut machine = Machine::single(&instructions);
    let mut host = Host::default();
    machine.begin(false);
    assert_eq!(machine.evaluate(0, &mut host), Ok(0.0));
    assert_eq!(
        host.effects,
        [
            Effect::Local(1, 1.0),
            Effect::External(2, 0.0, false),
            Effect::External(4, 0.0, true),
            Effect::External(6, 42.0, true),
            Effect::External(8, 8.0, true)
        ]
    );
}

#[test]
fn failed_event_stops_execution_without_rolling_back_earlier_host_effects() {
    let instructions = [
        literal(1.0),
        instruction(OP_SET_L),
        literal(42.0),
        instruction(OP_SET_K),
        literal(9.0),
        instruction(OP_SET_L),
    ];
    let mut machine = Machine::single(&instructions);
    let mut host = Host {
        fail_at: Some(3),
        ..Host::default()
    };
    machine.begin(false);
    assert_eq!(machine.evaluate(0, &mut host), Err((RPN_HOST, 3)));
    assert_eq!(
        host.effects,
        [Effect::Local(1, 1.0), Effect::External(3, 42.0, true)]
    );
}
