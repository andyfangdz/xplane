//! Tokenize gauge-string presentation without evaluating calculator expressions.
//!
//! Expression offsets count Unicode scalar values in the original template,
//! allowing the runtime to preserve source diagnostics when loading XML.
#![forbid(unsafe_code)]

const MAX_BYTES: usize = 32 * 1024 * 1024;
const MAX_TOKENS: usize = 2_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Text,
    Expression,
    If,
    Else,
    Case,
    Label,
    End,
    Format,
    Ignore,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Token {
    pub kind: Kind,
    pub text: String,
    pub number: i32,
    pub offset: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error {
    pub offset: usize,
    pub message: &'static str,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "template@{}: {}", self.offset, self.message)
    }
}
impl std::error::Error for Error {}

fn token(kind: Kind, text: String, number: i32, offset: usize) -> Token {
    Token {
        kind,
        text,
        number,
        offset,
    }
}
fn error(offset: usize, message: &'static str) -> Error {
    Error { offset, message }
}
fn push(tokens: &mut Vec<Token>, token: Token) -> Result<(), Error> {
    if tokens.len() == MAX_TOKENS {
        return Err(error(token.offset, "template exceeds token limit"));
    }
    tokens.push(token);
    Ok(())
}
fn flush(tokens: &mut Vec<Token>, literal: &mut String) -> Result<(), Error> {
    if !literal.is_empty() {
        push(tokens, token(Kind::Text, std::mem::take(literal), 0, 0))?;
    }
    Ok(())
}
fn format_supported(spec: &str) -> bool {
    if spec == "d" {
        return true;
    }
    let Some(spec) = spec.strip_suffix('f') else {
        return false;
    };
    let Some((width, precision)) = spec.split_once('.') else {
        return false;
    };
    !width.is_empty()
        && !precision.is_empty()
        && width.bytes().all(|byte| byte.is_ascii_digit())
        && precision.bytes().all(|byte| byte.is_ascii_digit())
}

pub fn parse(text: &str) -> Result<Vec<Token>, Error> {
    if text.len() > MAX_BYTES {
        return Err(error(0, "template exceeds size limit"));
    }
    let chars: Vec<_> = text.chars().collect();
    let mut tokens = Vec::new();
    let mut literal = String::new();
    let mut at = 0;
    while at < chars.len() {
        if chars[at] != '%' {
            literal.push(chars[at]);
            at += 1;
            continue;
        }
        flush(&mut tokens, &mut literal)?;
        at += 1;
        let Some(&marker) = chars.get(at) else { break };
        match marker {
            '%' | ' ' => {
                literal.push(marker);
                at += 1;
            }
            'n' => {
                literal.push('\n');
                at += 1;
            }
            '(' => {
                let start = at + 1;
                at += 1;
                let mut depth = 1usize;
                let mut quoted = false;
                while at < chars.len() && depth != 0 {
                    match chars[at] {
                        '\'' => quoted = !quoted,
                        '(' if !quoted => depth += 1,
                        ')' if !quoted => depth -= 1,
                        _ => {}
                    }
                    at += 1;
                }
                if depth != 0 {
                    return Err(error(start, "unterminated expression"));
                }
                push(
                    &mut tokens,
                    token(
                        Kind::Expression,
                        chars[start..at - 1].iter().collect(),
                        0,
                        start,
                    ),
                )?;
            }
            '{' => {
                let start = at + 1;
                at = start;
                while at < chars.len() && chars[at] != '}' {
                    at += 1;
                }
                if at == chars.len() {
                    return Err(error(start, "unterminated control tag"));
                }
                let tag: String = chars[start..at].iter().collect();
                at += 1;
                let kind = match tag.as_str() {
                    "if" => Some(Kind::If),
                    "else" => Some(Kind::Else),
                    "case" => Some(Kind::Case),
                    "end" => Some(Kind::End),
                    _ => None,
                };
                if let Some(kind) = kind {
                    push(&mut tokens, token(kind, String::new(), 0, 0))?;
                } else if let Some(digits) = tag
                    .strip_prefix(':')
                    .filter(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
                {
                    let number = digits
                        .parse()
                        .map_err(|_| error(start, "label exceeds integer range"))?;
                    push(&mut tokens, token(Kind::Label, String::new(), number, 0))?;
                } else {
                    push(&mut tokens, token(Kind::Ignore, tag, 0, 0))?;
                }
            }
            '!' => {
                let start = at + 1;
                at = start;
                while at < chars.len() && chars[at] != '!' {
                    at += 1;
                }
                if at == chars.len() {
                    return Err(error(start, "unterminated numeric format"));
                }
                let spec: String = chars[start..at].iter().collect();
                at += 1;
                if !format_supported(&spec) {
                    return Err(error(start, "unsupported numeric format"));
                }
                push(&mut tokens, token(Kind::Format, spec, 0, 0))?;
            }
            // An unrecognized escape drops '%' and retains its following text.
            _ => {}
        }
    }
    flush(&mut tokens, &mut literal)?;
    Ok(tokens)
}
