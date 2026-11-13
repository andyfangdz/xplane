use xplane_rpn::*;
pub fn programs() -> Vec<Vec<Instruction>> {
    let push = |v| Instruction::push(Value::Number(v));
    let op = |code| Instruction::new(code, 0);
    let mut programs = vec![
        vec![],
        vec![op(OP_GET_X), push(2.), op(OP_MUL), push(3.), op(OP_ADD)],
        vec![
            op(OP_GET_L),
            Instruction::new(OP_JZ, 5),
            push(42.),
            Instruction::new(OP_JUMP, 6),
            push(-1.),
        ],
        vec![
            op(OP_GET_X),
            Instruction::new(OP_REG_SP, 3),
            Instruction::new(OP_REG_L, 3),
            op(OP_DUP),
            push(2.),
            op(OP_DIV),
            op(OP_SWAP),
            op(OP_SUB),
        ],
        vec![
            op(OP_GET_X),
            Instruction::new(OP_REG_S, 4),
            Instruction::push(Value::String(b"tail\0bytes".to_vec())),
            op(OP_SCMP),
        ],
        vec![
            op(OP_GET_L),
            push(1.),
            op(OP_SUB),
            op(OP_DUP),
            op(OP_SET_L),
            Instruction::new(OP_JZ, 8),
            Instruction::new(OP_JUMP, 1),
        ],
        vec![push(7.), op(OP_QUIT), op(OP_GET_X)],
        vec![push(1.); 34],
        vec![
            op(OP_GET_X),
            op(OP_SET_K),
            op(OP_GET_X),
            op(OP_SET_X),
            op(OP_GET_L),
        ],
    ];
    for code in OP_EQ..=OP_OR {
        programs.push(vec![op(OP_GET_X), push(3.), op(code), op(OP_SET_K)]);
    }
    for code in OP_INC..=OP_RNOR {
        programs.push(vec![op(OP_GET_X), op(code), op(OP_SET_K)]);
    }
    programs.push(vec![
        push(1.),
        push(3.),
        op(OP_DIV),
        op(OP_GET_X),
        op(OP_ADD),
        op(OP_SET_K),
    ]);
    programs.push(vec![
        Instruction::push(Value::Boolean(false)),
        op(OP_GET_X),
        op(OP_EQ),
    ]);
    for code in [OP_ADD, OP_SUB, OP_MUL, OP_DIV] {
        programs.push(vec![
            op(code),
            push(1.),
            op(code),
            push(2.),
            op(code),
            op(OP_SET_K),
        ]);
    }
    for code in [OP_INC, OP_DEC, OP_NEG, OP_ABS, OP_SQR] {
        programs.push(vec![op(code), push(1.), op(OP_ADD), op(code), op(OP_SET_K)]);
    }
    programs.push(vec![
        op(OP_GET_X),
        push(1.),
        op(OP_ADD),
        op(OP_GET_X),
        push(2.),
        op(OP_MUL),
        op(OP_SET_K),
    ]);
    programs.push(vec![
        push(1.),
        push(2.),
        op(OP_ADD),
        Instruction::new(OP_JZ, 8),
        push(3.),
        push(4.),
        op(OP_MUL),
        op(OP_SET_K),
    ]);
    programs.push(vec![
        op(OP_GET_X),
        Instruction::new(OP_JZ, 6),
        push(1.),
        push(2.),
        Instruction::new(OP_JUMP, 7),
        push(3.),
        op(OP_ADD),
        op(OP_SET_K),
        op(OP_GET_X),
    ]);
    programs.push(vec![
        op(OP_SWAP),
        op(OP_DUP),
        op(OP_PI),
        op(OP_POP),
        push(-2.),
        push(4.),
        push(1.),
        op(OP_RNG),
        op(OP_CLEAR),
        op(OP_GET_X),
    ]);
    for cases in [0., 1., 2., 3., 0.5, -1., f64::INFINITY] {
        programs.push(vec![
            push(10.),
            push(20.),
            push(30.),
            push(cases),
            op(OP_GET_X),
            op(OP_CASE),
            op(OP_SET_K),
        ]);
    }
    programs.push(vec![
        Instruction::new(OP_REG_L, 3),
        op(OP_GET_X),
        op(OP_ADD),
        Instruction::new(OP_REG_S, 7),
        op(OP_GET_L),
        op(OP_MUL),
        Instruction::new(OP_REG_L, 2),
        op(OP_ADD),
        op(OP_SET_K),
        Instruction::new(OP_REG_L, 7),
    ]);
    let mut long = vec![op(OP_GET_X)];
    for _ in 0..32 {
        long.extend([push(1.), op(OP_ADD), push(0.5), op(OP_MUL)]);
    }
    programs.push(long);
    programs
}
