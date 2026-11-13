#![doc = include_str!("../README.md")]
pub mod aot;
mod engine;
pub mod source;
mod vm;

pub use engine::AotFrame;

pub use vm::{
    AotBinding, BuildError, ExecutionContext, Fault, Host, Instruction, Program, Value, Vm,
};

pub const OP_PUSH: u32 = 0;
pub const OP_GET_L: u32 = 1;
pub const OP_GET_X: u32 = 2;
pub const OP_SET_L: u32 = 3;
pub const OP_SET_K: u32 = 4;
pub const OP_SET_X: u32 = 5;
pub const OP_JZ: u32 = 6;
pub const OP_JUMP: u32 = 7;
pub const OP_QUIT: u32 = 8;
pub const OP_REG_L: u32 = 9;
pub const OP_REG_SP: u32 = 10;
pub const OP_REG_S: u32 = 11;
pub const OP_DUP: u32 = 12;
pub const OP_POP: u32 = 13;
pub const OP_CLEAR: u32 = 14;
pub const OP_SWAP: u32 = 15;
pub const OP_PI: u32 = 16;
pub const OP_CASE: u32 = 17;
pub const OP_SCMP: u32 = 18;
pub const OP_RNG: u32 = 19;
pub const OP_EQ: u32 = 20;
pub const OP_NE: u32 = 21;
pub const OP_ADD: u32 = 22;
pub const OP_SUB: u32 = 23;
pub const OP_MUL: u32 = 24;
pub const OP_DIV: u32 = 25;
pub const OP_MOD: u32 = 26;
pub const OP_IDIV: u32 = 27;
pub const OP_POW: u32 = 28;
pub const OP_MIN: u32 = 29;
pub const OP_MAX: u32 = 30;
pub const OP_LOG: u32 = 31;
pub const OP_ATG2: u32 = 32;
pub const OP_LT: u32 = 33;
pub const OP_GT: u32 = 34;
pub const OP_LE: u32 = 35;
pub const OP_GE: u32 = 36;
pub const OP_AND: u32 = 37;
pub const OP_OR: u32 = 38;
pub const OP_INC: u32 = 39;
pub const OP_DEC: u32 = 40;
pub const OP_NEG: u32 = 41;
pub const OP_NOT: u32 = 42;
pub const OP_ABS: u32 = 43;
pub const OP_INT: u32 = 44;
pub const OP_FLR: u32 = 45;
pub const OP_CEIL: u32 = 46;
pub const OP_NEAR: u32 = 47;
pub const OP_COS: u32 = 48;
pub const OP_SIN: u32 = 49;
pub const OP_ACOS: u32 = 50;
pub const OP_ASIN: u32 = 51;
pub const OP_TG: u32 = 52;
pub const OP_ATG: u32 = 53;
pub const OP_SQRT: u32 = 54;
pub const OP_SQR: u32 = 55;
pub const OP_EXP: u32 = 56;
pub const OP_LN: u32 = 57;
pub const OP_LG: u32 = 58;
pub const OP_RDDG: u32 = 59;
pub const OP_DGRD: u32 = 60;
pub const OP_DNOR: u32 = 61;
pub const OP_RNOR: u32 = 62;
const OP_COUNT: u32 = 63;

pub const RPN_OK: u32 = 0;
/// A host callback failed and recorded its own message.
pub const RPN_HOST: u32 = 1;
pub const RPN_NUMERIC: u32 = 2;
pub const RPN_SCMP: u32 = 3;
pub const RPN_CASE: u32 = 4;
pub const RPN_STEPS: u32 = 5;
pub const RPN_CAPACITY: u32 = 6;
pub const RPN_ALLOC: u32 = 7;
pub const RPN_BAD_CALL: u32 = 8;

/// Numeric operand was a boolean rather than a number.
pub const RPN_NUMERIC_BOOLEAN: u32 = 9;
/// A constant or external read supplied no value.
pub const RPN_MISSING: u32 = 10;
