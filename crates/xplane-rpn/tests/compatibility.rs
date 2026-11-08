//! Synthetic public-ABI regressions for the observed P3D compatibility subset.
//! These programs contain no aircraft expressions or captured vendor fixtures.

use std::ffi::c_void;
use xplane_rpn::*;

const ZERO: RpnValue = number(0.0);

const fn number(value: f64) -> RpnValue {
    RpnValue {
        kind: NUMBER,
        number: value,
        text: std::ptr::null(),
        len: 0,
    }
}

fn instruction(op: u32) -> RpnInstr {
    RpnInstr {
        op,
        arg: 0,
        value: ZERO,
    }
}

fn literal(value: f64) -> RpnInstr {
    RpnInstr {
        value: number(value),
        ..instruction(OP_PUSH)
    }
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

unsafe extern "C" fn get_local(_: *mut c_void, _: u32, out: *mut f64) -> i32 {
    unsafe { *out = 0.0 };
    0
}

unsafe extern "C" fn get_external(_: *mut c_void, _: u32, out: *mut RpnValue) -> i32 {
    unsafe { *out = ZERO };
    0
}

unsafe extern "C" fn set_local(context: *mut c_void, id: u32, value: *const RpnValue) -> i32 {
    unsafe {
        let host = &mut *context.cast::<Host>();
        host.effects.push(Effect::Local(id, (*value).number));
        i32::from(host.fail_at == Some(id))
    }
}

unsafe extern "C" fn set_external(
    context: *mut c_void,
    id: u32,
    value: *const RpnValue,
    present: i32,
) -> i32 {
    unsafe {
        let host = &mut *context.cast::<Host>();
        host.effects
            .push(Effect::External(id, (*value).number, present != 0));
        i32::from(host.fail_at == Some(id))
    }
}

struct Machine(*mut Vm);

impl Machine {
    fn new(instructions: &[RpnInstr], programs: &[RpnProgram]) -> Self {
        // SAFETY: Both descriptor slices are valid for the call; the VM copies them.
        let raw = unsafe {
            rpn_vm_new(
                instructions.as_ptr(),
                instructions.len(),
                programs.as_ptr(),
                programs.len(),
            )
        };
        assert!(!raw.is_null());
        Self(raw)
    }

    fn single(instructions: &[RpnInstr]) -> Self {
        Self::new(
            instructions,
            &[RpnProgram {
                first: 0,
                count: instructions.len() as u32,
            }],
        )
    }

    fn begin(&mut self, zero_registers: bool) {
        // SAFETY: This wrapper uniquely owns a live VM.
        unsafe { rpn_begin(self.0, i32::from(zero_registers)) };
    }

    fn evaluate(&mut self, program: u32, host: &mut Host) -> Result<f64, (u32, i64)> {
        let callbacks = RpnHost {
            context: std::ptr::from_mut(host).cast(),
            get_local: Some(get_local),
            set_local: Some(set_local),
            get_external: Some(get_external),
            set_external: Some(set_external),
        };
        let mut result = ZERO;
        let mut fault = RpnFault {
            status: u32::MAX,
            instr: -2,
        };
        // SAFETY: The VM is exclusively borrowed, outputs and callback context
        // outlive this synchronous call, and the callbacks do not reenter it.
        let status = unsafe { rpn_eval(self.0, program, 100, &callbacks, &mut result, &mut fault) };
        assert_eq!(status, fault.status);
        if status == RPN_OK {
            assert_eq!(result.kind, NUMBER);
            assert_eq!(fault.instr, -1);
            Ok(result.number)
        } else {
            Err((status, fault.instr))
        }
    }

    fn stack(&self) -> Vec<f64> {
        let mut len = 0;
        // SAFETY: The live VM cannot mutate while this shared borrow is held;
        // values are copied before returning from the wrapper.
        unsafe {
            let values = rpn_stack(self.0, &mut len);
            std::slice::from_raw_parts(values, len)
                .iter()
                .map(|value| value.number)
                .collect()
        }
    }
}

impl Drop for Machine {
    fn drop(&mut self) {
        // SAFETY: This wrapper owns the VM and frees it exactly once.
        unsafe { rpn_vm_free(self.0) };
    }
}

fn evaluate(instructions: &[RpnInstr]) -> f64 {
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
            RpnProgram { first: 0, count: 4 },
            RpnProgram { first: 4, count: 2 },
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
