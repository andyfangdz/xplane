//! Parse calculator expressions once and retain ordinary VM instructions.
//!
//! Expressions, strings and macro names are byte sequences. Numeric conversion
//! belongs to the caller, so a host can retain its existing literal semantics.
use crate::*;
use std::collections::BTreeMap;

const MAX_BYTES: usize = 32 * 1024 * 1024;
const MAX_ITEMS: usize = 2_000_000;
const MAX_MACRO_DEPTH: usize = 32;
const MAX_BLOCK_DEPTH: usize = 256;

/// Source syntax extensions are explicit; the default requires balanced blocks
/// and an `els{` immediately following its `if{` block.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub allow_separated_else: bool,
    /// An unattached `els{` loses its conditional marker; its body is retained.
    pub allow_orphan_else: bool,
    pub allow_unmatched_closing_brace: bool,
    pub allow_ternary: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Variable {
    pub namespace: u8,
    pub name: Vec<u8>,
    pub unit: Vec<u8>,
    pub index: i32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ParsedInstruction {
    pub instruction: Instruction,
    /// Source spelling for operators, or `push`, `get`, `set`, `register`,
    /// `jz`, `jump` for the corresponding resolved instruction.
    pub opcode: Vec<u8>,
    pub variable: Option<Variable>,
    /// `source@offset`, with nested `/macro:name@offset` suffixes. Offsets count
    /// Unicode scalar values for UTF-8 input, and bytes for other input.
    pub location: Vec<u8>,
    pub numeric_literal: Option<Vec<u8>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    pub location: Vec<u8>,
    pub message: &'static str,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}: {}",
            String::from_utf8_lossy(&self.location),
            self.message
        )
    }
}
impl std::error::Error for ParseError {}

#[derive(Clone)]
struct Token {
    text: Vec<u8>,
    location: Vec<u8>,
}

fn error(location: &[u8], message: &'static str) -> ParseError {
    ParseError {
        location: location.to_vec(),
        message,
    }
}
fn location(source: &[u8], offset: usize) -> Vec<u8> {
    [source, b"@", offset.to_string().as_bytes()].concat()
}

fn tokenize(input: &[u8], source: &[u8]) -> Result<Vec<Token>, ParseError> {
    if input.len() > MAX_BYTES {
        return Err(error(source, "source exceeds parser limits"));
    }
    let unicode = std::str::from_utf8(input).is_ok();
    let mut tokens = Vec::new();
    let (mut at, mut previous, mut offset) = (0, 0, 0);
    while at < input.len() {
        if input[at].is_ascii_whitespace() {
            at += 1;
            continue;
        }
        let start = at;
        offset += if unicode {
            input[previous..start]
                .iter()
                .filter(|b| **b & 0xc0 != 0x80)
                .count()
        } else {
            start - previous
        };
        previous = start;
        let where_ = location(source, offset);
        if input[at..].starts_with(b"if{") {
            at += 3;
        } else if input[at..].starts_with(b"els{") {
            at += 4;
        } else if input[at] == b'}' {
            at += 1;
        } else if input[at] == b'(' || input[at] == b'\'' {
            let close = if input[at] == b'(' { b')' } else { b'\'' };
            let Some(end) = input[at + 1..].iter().position(|b| *b == close) else {
                return Err(error(
                    &where_,
                    if close == b')' {
                        "unterminated variable"
                    } else {
                        "unterminated string"
                    },
                ));
            };
            at += end + 2;
        } else {
            while at < input.len()
                && !input[at].is_ascii_whitespace()
                && !b"{}('".contains(&input[at])
            {
                at += 1;
            }
            if at == start {
                return Err(error(&where_, "unexpected source character"));
            }
        }
        if tokens.len() == MAX_ITEMS {
            return Err(error(&where_, "source exceeds parser limits"));
        }
        tokens.push(Token {
            text: input[start..at].to_vec(),
            location: where_,
        });
    }
    Ok(tokens)
}

fn expand(
    tokens: Vec<Token>,
    macros: &BTreeMap<Vec<u8>, Vec<u8>>,
    active: &mut Vec<Vec<u8>>,
    result: &mut Vec<Token>,
) -> Result<(), ParseError> {
    for token in tokens {
        if token.text.first() == Some(&b'@') {
            let name = &token.text[1..];
            let Some(body) = macros.get(name) else {
                return Err(error(&token.location, "unknown macro"));
            };
            if active.len() == MAX_MACRO_DEPTH || active.iter().any(|n| n == name) {
                return Err(error(&token.location, "recursive macro"));
            }
            let source = [&token.location[..], b"/macro:", name].concat();
            active.push(name.to_vec());
            let expanded = expand(tokenize(body, &source)?, macros, active, result);
            active.pop();
            expanded?;
        } else {
            if result.len() == MAX_ITEMS {
                return Err(error(&token.location, "source exceeds parser limits"));
            }
            result.push(token);
        }
    }
    Ok(())
}

fn trim(bytes: &[u8]) -> &[u8] {
    bytes.trim_ascii()
}
fn variable(token: &Token) -> Result<Option<(bool, Variable)>, ParseError> {
    if token.text.first() != Some(&b'(') {
        return Ok(None);
    }
    let mut body = trim(&token.text[1..token.text.len() - 1]);
    let write = body.first() == Some(&b'>');
    if write {
        body = trim(&body[1..]);
    }
    if body.is_empty() || !body[0].is_ascii_alphabetic() {
        return Err(error(&token.location, "invalid variable"));
    }
    let namespace = body[0].to_ascii_uppercase();
    body = trim(&body[1..]);
    if body.first() != Some(&b':') {
        return Err(error(&token.location, "invalid variable"));
    }
    body = trim(&body[1..]);
    if body.iter().any(|b| b"()".contains(b)) {
        return Err(error(&token.location, "invalid variable"));
    }
    let (name, unit) = if let Some(comma) = body.iter().position(|b| *b == b',') {
        (trim(&body[..comma]), trim(&body[comma + 1..]))
    } else {
        (trim(body), b"".as_slice())
    };
    if name.is_empty() {
        return Err(error(&token.location, "invalid variable"));
    }
    if !b"ALEPCKGM".contains(&namespace) {
        return Err(error(&token.location, "unsupported variable namespace"));
    }
    if (!write && namespace == b'K') || (write && !b"LAKGC".contains(&namespace)) {
        return Err(error(&token.location, "unsupported variable access"));
    }
    let (mut name, mut index) = (name, 0);
    if namespace == b'A' {
        if let Some(colon) = name.iter().rposition(|b| *b == b':') {
            let suffix = &name[colon + 1..];
            if !suffix.is_empty() && suffix.iter().all(u8::is_ascii_digit) {
                index = std::str::from_utf8(suffix)
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .ok_or_else(|| error(&token.location, "variable index out of range"))?;
                name = &name[..colon];
            }
        }
    }
    Ok(Some((
        write,
        Variable {
            namespace,
            name: name.to_vec(),
            unit: unit.to_vec(),
            index,
        },
    )))
}

/// Recognize numeric syntax without selecting how its value is rounded or
/// interpreted. Leading zeros, hexadecimal and decimal exponents reach the host.
fn is_number(mut input: &[u8]) -> bool {
    if input.first().is_some_and(|b| b"+-".contains(b)) {
        input = &input[1..];
    }
    if input.starts_with(b"0x") || input.starts_with(b"0X") {
        return input.len() > 2 && input[2..].iter().all(u8::is_ascii_hexdigit);
    }
    let mut at = 0;
    while input.get(at).is_some_and(u8::is_ascii_digit) {
        at += 1;
    }
    let mut digits = at;
    if input.get(at) == Some(&b'.') {
        at += 1;
        let start = at;
        while input.get(at).is_some_and(u8::is_ascii_digit) {
            at += 1;
        }
        digits += at - start;
    }
    if digits == 0 {
        return false;
    }
    if input.get(at).is_some_and(|b| b"eE".contains(b)) {
        at += 1;
        if input.get(at).is_some_and(|b| b"+-".contains(b)) {
            at += 1;
        }
        let start = at;
        while input.get(at).is_some_and(u8::is_ascii_digit) {
            at += 1;
        }
        if start == at {
            return false;
        }
    }
    at == input.len()
}

fn operator(name: &[u8]) -> Option<u32> {
    Some(match name {
        b"quit" => OP_QUIT,
        b"d" => OP_DUP,
        b"p" => OP_POP,
        b"c" => OP_CLEAR,
        b"r" => OP_SWAP,
        b"pi" => OP_PI,
        b"case" => OP_CASE,
        b"scmp" => OP_SCMP,
        b"rng" => OP_RNG,
        b"==" => OP_EQ,
        b"!=" => OP_NE,
        b"+" => OP_ADD,
        b"-" => OP_SUB,
        b"*" => OP_MUL,
        b"/" => OP_DIV,
        b"%" => OP_MOD,
        b"div" => OP_IDIV,
        b"pow" => OP_POW,
        b"min" => OP_MIN,
        b"max" => OP_MAX,
        b"log" => OP_LOG,
        b"atg2" => OP_ATG2,
        b"<" => OP_LT,
        b">" => OP_GT,
        b"<=" => OP_LE,
        b">=" => OP_GE,
        b"and" | b"&&" => OP_AND,
        b"or" | b"||" => OP_OR,
        b"++" => OP_INC,
        b"--" => OP_DEC,
        b"neg" | b"/-/" => OP_NEG,
        b"!" | b"not" => OP_NOT,
        b"abs" => OP_ABS,
        b"int" => OP_INT,
        b"flr" => OP_FLR,
        b"ceil" => OP_CEIL,
        b"near" => OP_NEAR,
        b"cos" => OP_COS,
        b"sin" => OP_SIN,
        b"acos" => OP_ACOS,
        b"asin" => OP_ASIN,
        b"tg" => OP_TG,
        b"atg" => OP_ATG,
        b"sqrt" => OP_SQRT,
        b"sqr" => OP_SQR,
        b"exp" => OP_EXP,
        b"ln" => OP_LN,
        b"lg" => OP_LG,
        b"rddg" => OP_RDDG,
        b"dgrd" => OP_DGRD,
        b"dnor" | b"d360" | b"rdeg" => OP_DNOR,
        b"rnor" => OP_RNOR,
        _ => return None,
    })
}

struct Parser<'a, F> {
    tokens: &'a [Token],
    code: Vec<ParsedInstruction>,
    options: Options,
    number: F,
}
impl<F: FnMut(&[u8]) -> Option<f64>> Parser<'_, F> {
    fn emit(
        &mut self,
        token: &Token,
        opcode: &[u8],
        instruction: Instruction,
    ) -> Result<usize, ParseError> {
        if self.code.len() == MAX_ITEMS {
            return Err(error(&token.location, "source exceeds parser limits"));
        }
        let at = self.code.len();
        self.code.push(ParsedInstruction {
            instruction,
            opcode: opcode.to_vec(),
            variable: None,
            location: token.location.clone(),
            numeric_literal: None,
        });
        Ok(at)
    }
    fn target(&mut self, instruction: usize) {
        self.code[instruction].instruction.arg = self.code.len() as u32 + 1;
    }
    fn block(&mut self, mut at: usize, depth: usize) -> Result<usize, ParseError> {
        if depth > MAX_BLOCK_DEPTH {
            return Err(error(
                &self.tokens[at.saturating_sub(1)].location,
                "block nesting exceeds parser limits",
            ));
        }
        let mut pending = None;
        while at < self.tokens.len() {
            let token = &self.tokens[at];
            match token.text.as_slice() {
                b"}" => {
                    if depth != 0 {
                        return Ok(at + 1);
                    }
                    if !self.options.allow_unmatched_closing_brace {
                        return Err(error(&token.location, "unmatched closing brace"));
                    }
                    at += 1;
                }
                b"if{" => {
                    let branch = self.emit(token, b"jz", Instruction::new(OP_JZ, 0))?;
                    at = self.block(at + 1, depth + 1)?;
                    if self.options.allow_separated_else {
                        self.target(branch);
                        pending = Some(branch);
                    } else if self.tokens.get(at).is_some_and(|t| t.text == b"els{") {
                        let jump =
                            self.emit(&self.tokens[at], b"jump", Instruction::new(OP_JUMP, 0))?;
                        self.target(branch);
                        at = self.block(at + 1, depth + 1)?;
                        self.target(jump);
                    } else {
                        self.target(branch);
                    }
                }
                b"els{" => {
                    if let Some(branch) = pending.take() {
                        let jump = self.emit(token, b"jump", Instruction::new(OP_JUMP, 0))?;
                        self.target(branch);
                        at = self.block(at + 1, depth + 1)?;
                        self.target(jump);
                    } else {
                        if !self.options.allow_orphan_else {
                            return Err(error(&token.location, "else without matching if"));
                        }
                        at = self.block(at + 1, depth + 1)?;
                    }
                }
                b"?" if self.options.allow_ternary => {
                    let first = self.code.len() as u32;
                    for (name, op, arg) in [
                        (b"jz".as_slice(), OP_JZ, first + 4),
                        (b"p", OP_POP, 0),
                        (b"jump", OP_JUMP, first + 6),
                        (b"r", OP_SWAP, 0),
                        (b"p", OP_POP, 0),
                    ] {
                        self.emit(token, name, Instruction::new(op, arg))?;
                    }
                    at += 1;
                }
                _ => {
                    self.atom(token)?;
                    at += 1;
                }
            }
        }
        if depth != 0 {
            return Err(error(
                &self.tokens.last().unwrap().location,
                "unclosed conditional block",
            ));
        }
        Ok(at)
    }
    fn atom(&mut self, token: &Token) -> Result<(), ParseError> {
        if let Some((write, variable)) = variable(token)? {
            let op = match (write, variable.namespace) {
                (false, b'L') => OP_GET_L,
                (true, b'L') => OP_SET_L,
                (true, b'K') => OP_SET_K,
                (false, _) => OP_GET_X,
                (true, _) => OP_SET_X,
            };
            let at = self.emit(
                token,
                if write { b"set" } else { b"get" },
                Instruction::new(op, 0),
            )?;
            self.code[at].variable = Some(variable);
        } else if is_number(&token.text) {
            let value = (self.number)(&token.text)
                .ok_or_else(|| error(&token.location, "invalid numeric literal"))?;
            let at = self.emit(token, b"push", Instruction::push(Value::Number(value)))?;
            self.code[at].numeric_literal = Some(token.text.clone());
        } else if token.text.first() == Some(&b'\'') {
            self.emit(
                token,
                b"push",
                Instruction::push(Value::String(token.text[1..token.text.len() - 1].to_vec())),
            )?;
        } else {
            let lower = token.text.to_ascii_lowercase();
            let register = [
                (b"sp".as_slice(), OP_REG_SP),
                (b"s", OP_REG_S),
                (b"l", OP_REG_L),
            ]
            .into_iter()
            .find_map(|(prefix, op)| {
                lower
                    .strip_prefix(prefix)
                    .filter(|s| !s.is_empty() && s.iter().all(u8::is_ascii_digit))
                    .map(|s| (s, op))
            });
            if let Some((digits, op)) = register {
                let index = std::str::from_utf8(digits)
                    .ok()
                    .and_then(|s| s.parse::<u32>().ok())
                    .filter(|n| *n < 50)
                    .ok_or_else(|| error(&token.location, "register index outside 0..49"))?;
                self.emit(token, b"register", Instruction::new(op, index))?;
            } else if let Some(op) = operator(&lower) {
                self.emit(token, &lower, Instruction::new(op, 0))?;
            } else {
                return Err(error(&token.location, "unsupported source token"));
            }
        }
        Ok(())
    }
}

/// Parse one expression into instructions and host/source metadata. Numeric
/// conversion runs once per literal while loading, never during VM evaluation.
/// No XML, filesystem, simulator or Lua dependency is involved.
pub fn parse(
    expression: &[u8],
    name: &[u8],
    macros: &BTreeMap<Vec<u8>, Vec<u8>>,
    options: Options,
    number: impl FnMut(&[u8]) -> Option<f64>,
) -> Result<Vec<ParsedInstruction>, ParseError> {
    if name.len() > MAX_BYTES {
        return Err(error(b"", "source name exceeds parser limits"));
    }
    let mut tokens = Vec::new();
    expand(
        tokenize(expression, name)?,
        macros,
        &mut Vec::new(),
        &mut tokens,
    )?;
    let mut parser = Parser {
        tokens: &tokens,
        code: Vec::new(),
        options,
        number,
    };
    parser.block(0, 0)?;
    Ok(parser.code)
}
