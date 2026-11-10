use std::collections::BTreeMap;
use xplane_rpn::{source::*, *};

fn number(text: &[u8]) -> Option<f64> {
    std::str::from_utf8(text).ok()?.parse().ok()
}
fn parsed(text: &[u8], options: Options) -> Vec<ParsedInstruction> {
    parse(text, b"fixture", &BTreeMap::new(), options, number).unwrap()
}
struct NoHost;
impl Host for NoHost {
    type Error = ();
    fn get_local(&mut self, _: u32) -> Result<f64, ()> {
        panic!("unexpected local read")
    }
    fn set_local(&mut self, _: u32, _: &Value) -> Result<(), ()> {
        panic!("unexpected local write")
    }
    fn get_external(&mut self, _: u32) -> Result<Value, ()> {
        panic!("unexpected external read")
    }
    fn set_external(&mut self, _: u32, _: &Value, _: bool) -> Result<(), ()> {
        panic!("unexpected external effect")
    }
}
fn evaluate(code: &[ParsedInstruction]) -> Value {
    let instructions: Vec<_> = code.iter().map(|i| i.instruction.clone()).collect();
    Vm::new(
        &instructions,
        &[Program {
            first: 0,
            count: instructions.len() as u32,
        }],
    )
    .unwrap()
    .evaluate(0, 1000, &mut NoHost)
    .unwrap()
}

#[test]
fn original_literal_tokens_reach_the_numeric_policy_without_decimal_roundtrip() {
    let mut seen = Vec::new();
    let code = parse(
        b"-0 0x10 077 .5 2E-3",
        b"numbers",
        &BTreeMap::new(),
        Options::default(),
        |raw| {
            seen.push(raw.to_vec());
            Some(match raw {
                b"-0" => -0.,
                b"0x10" => 16.,
                b"077" => 63.,
                b".5" => 0.5,
                b"2E-3" => 0.002,
                _ => panic!(),
            })
        },
    )
    .unwrap();
    assert_eq!(
        seen,
        [
            b"-0".to_vec(),
            b"0x10".to_vec(),
            b"077".to_vec(),
            b".5".to_vec(),
            b"2E-3".to_vec()
        ]
    );
    for (i, raw) in code.iter().zip(&seen) {
        assert_eq!(i.numeric_literal.as_ref(), Some(raw));
        assert_eq!(i.opcode, b"push");
    }
    let Value::Number(zero) = code[0].instruction.value else {
        panic!()
    };
    assert_eq!(zero.to_bits(), (-0f64).to_bits());
    let failure = parse(
        b"1",
        b"numbers",
        &BTreeMap::new(),
        Options::default(),
        |_| None,
    )
    .unwrap_err();
    assert_eq!(failure.location, b"numbers@0");
    assert_eq!(failure.message, "invalid numeric literal");
}

#[test]
fn variable_operands_and_macro_locations_remain_available_to_the_host() {
    let macros = BTreeMap::from([
        (
            b"write".to_vec(),
            b"@value (>L:Some Name, percent)".to_vec(),
        ),
        (b"value".to_vec(), b"(L:Some Name, percent) 1 +".to_vec()),
    ]);
    let code = parse(
        b"@write (A:TEST:2, string)",
        b"root",
        &macros,
        Options::default(),
        number,
    )
    .unwrap();
    assert_eq!(
        code.iter().map(|i| i.instruction.op).collect::<Vec<_>>(),
        [OP_GET_L, OP_PUSH, OP_ADD, OP_SET_L, OP_GET_X]
    );
    assert_eq!(code[0].location, b"root@0/macro:write@0/macro:value@0");
    assert_eq!(code[1].location, b"root@0/macro:write@0/macro:value@23");
    assert_eq!(code[3].location, b"root@0/macro:write@7");
    assert_eq!(code[4].location, b"root@7");
    assert_eq!(
        code[0].variable,
        Some(Variable {
            namespace: b'L',
            name: b"Some Name".to_vec(),
            unit: b"percent".to_vec(),
            index: 0
        })
    );
    assert_eq!(
        code[4].variable,
        Some(Variable {
            namespace: b'A',
            name: b"TEST".to_vec(),
            unit: b"string".to_vec(),
            index: 2
        })
    );
    let aliases = parsed(
        b"(>K:EVENT) (e:TIME, seconds) (A:NAME:-1, number)",
        Options::default(),
    );
    assert_eq!(aliases[0].instruction.op, OP_SET_K);
    assert_eq!(aliases[0].variable.as_ref().unwrap().unit, b"");
    assert_eq!(aliases[1].variable.as_ref().unwrap().namespace, b'E');
    assert_eq!(aliases[2].variable.as_ref().unwrap().name, b"NAME:-1");
    assert_eq!(aliases[2].variable.as_ref().unwrap().index, 0);
}

#[test]
fn byte_strings_and_utf8_source_offsets_preserve_the_original_content() {
    let code = parsed(b"'A\0\xff' 'B' scmp", Options::default());
    assert_eq!(code[0].instruction.value, Value::String(vec![b'A', 0, 255]));
    assert_eq!(code[1].location, b"fixture@6");
    let code = parsed("'é' 3".as_bytes(), Options::default());
    assert_eq!(code[0].instruction.value, Value::String(vec![0xc3, 0xa9]));
    assert_eq!(code[1].location, b"fixture@4");
}

#[test]
fn nested_conditionals_and_ternary_targets_execute_the_selected_values() {
    let options = Options {
        allow_ternary: true,
        ..Options::default()
    };
    for (source, expected) in [
        (b"0 if{ 10 20 0 ? } els{ 30 }".as_slice(), 30.),
        (b"1 if{ 10 20 0 ? } els{ 30 }", 20.),
        (b"1 if{ 10 20 1 ? } els{ 30 }", 10.),
        (b"1 if{ 0 if{ 7 } els{ 8 } } els{ 9 }", 8.),
    ] {
        let code = parsed(source, options);
        assert_eq!(evaluate(&code), Value::Number(expected));
    }
    let code = parsed(b"0 if{ 10 20 0 ? } els{ 30 }", options);
    assert_eq!(
        (code[1].instruction.op, code[1].instruction.arg),
        (OP_JZ, 12)
    );
    assert_eq!(
        (code[5].instruction.op, code[5].instruction.arg),
        (OP_JZ, 9)
    );
    assert_eq!(
        (code[7].instruction.op, code[7].instruction.arg),
        (OP_JUMP, 11)
    );
    assert_eq!(
        (code[10].instruction.op, code[10].instruction.arg),
        (OP_JUMP, 13)
    );
    assert!(code[5..10].iter().all(|i| i.location == b"fixture@14"));
}

#[test]
fn permissive_control_syntax_requires_explicit_options() {
    let extensions = Options {
        allow_separated_else: true,
        allow_orphan_else: true,
        allow_unmatched_closing_brace: true,
        allow_ternary: true,
    };
    for (source, expected) in [
        (b"0 if{ 5 } 6 els{ 7 }".as_slice(), 7.),
        (b"1 if{ 5 } 6 els{ 7 }", 6.),
        (b"els{ 2 }", 2.),
        (b"3 }", 3.),
        (b"10 20 0 ?", 20.),
    ] {
        assert!(
            parse(
                source,
                b"source",
                &BTreeMap::new(),
                Options::default(),
                number
            )
            .is_err()
        );
        assert_eq!(
            evaluate(&parsed(source, extensions)),
            Value::Number(expected)
        );
    }
    assert!(parse(b"if{ 1", b"source", &BTreeMap::new(), extensions, number).is_err());
}

#[test]
fn register_spelling_and_malformed_sources_are_checked_before_execution() {
    let code = parsed(b"3 S0 SP49 l0 l49 +", Options::default());
    assert_eq!(code[1].opcode, b"register");
    assert_eq!(
        (code[1].instruction.op, code[1].instruction.arg),
        (OP_REG_S, 0)
    );
    assert_eq!(
        (code[2].instruction.op, code[2].instruction.arg),
        (OP_REG_SP, 49)
    );
    assert_eq!(evaluate(&code), Value::Number(6.));
    for source in [
        b"s50".as_slice(),
        b"l9999999999999999999",
        b"(K:EVENT)",
        b"(>E:TIME,seconds)",
        b"(Q:VALUE,number)",
        b"(L:missing",
        b"'missing",
        b"{",
        b"@missing",
        b"1e+",
        b"0x",
        b"unknown",
    ] {
        assert!(
            parse(source, b"bad", &BTreeMap::new(), Options::default(), number).is_err(),
            "{source:?}"
        );
    }
}

#[test]
fn cyclic_macros_and_unbounded_nesting_are_rejected() {
    let macros = BTreeMap::from([
        (b"a".to_vec(), b"@b".to_vec()),
        (b"b".to_vec(), b"@a".to_vec()),
    ]);
    let failure = parse(b"@a", b"source", &macros, Options::default(), number).unwrap_err();
    assert_eq!(failure.message, "recursive macro");
    assert_eq!(failure.location, b"source@0/macro:a@0/macro:b@0");
    let source = [b"if{".repeat(257), b"}".repeat(257)].concat();
    assert_eq!(
        parse(
            &source,
            b"source",
            &BTreeMap::new(),
            Options::default(),
            number
        )
        .unwrap_err()
        .message,
        "block nesting exceeds parser limits"
    );
}
