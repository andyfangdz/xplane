//! Presentation data and conditional rendering with host-provided evaluation
//! and numeric formatting. Text is bytes so encoding remains a host decision.
use crate::template::Kind;
pub use crate::template::Kind as Op;

#[derive(Clone, Debug, PartialEq)]
pub struct Instruction<P = i32> {
    pub op: Kind,
    pub program: P,
    pub text: Vec<u8>,
    pub number: f64,
}
#[derive(Debug, PartialEq)]
pub enum Error<E> {
    Policy(&'static [u8]),
    Evaluation(E),
}
#[derive(Clone, Debug, PartialEq)]
pub struct Readout {
    pub name: Vec<u8>,
    pub source: Vec<u8>,
    pub visibility: i32,
    pub colour: i32,
    pub fixed_colour: Vec<u8>,
    pub text: Vec<Instruction>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Clip {
    pub x: i32,
    pub y: i32,
    pub width: f64,
    pub height: f64,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    pub name: Vec<u8>,
    pub visibility: i32,
    pub x: i32,
    pub y: i32,
    pub width: f64,
    pub height: f64,
    pub transparent: bool,
    pub clips: Vec<Clip>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Canvas {
    pub width: f64,
    pub height: f64,
    pub images: Vec<Image>,
}

/// Only active branches call the evaluator and formatter. Output is bounded to
/// 4095 bytes and nesting to 256 levels, including skipped branches.
pub fn render<P: Copy, E>(
    instructions: &[Instruction<P>],
    mut evaluate: impl FnMut(P) -> Result<f64, E>,
    mut format_number: impl FnMut(f64, &[u8]) -> Result<Vec<u8>, Error<E>>,
) -> Result<Vec<u8>, Error<E>> {
    struct Branch {
        parent: bool,
        matched: bool,
        is_case: bool,
        selector: f64,
    }
    fn consume<E>(value: &mut Option<f64>) -> Result<f64, Error<E>> {
        value
            .take()
            .ok_or(Error::Policy(b"tooltip scalar consumer without expression"))
    }
    let mut branches = Vec::<Branch>::new();
    let mut active = true;
    let mut value = None;
    let mut output = Vec::new();
    for instruction in instructions {
        match instruction.op {
            Op::Text => {
                if active {
                    output.extend_from_slice(&instruction.text);
                }
            }
            Op::Expression => {
                if active {
                    if value.is_some() {
                        return Err(Error::Policy(b"unconsumed tooltip expression"));
                    }
                    value = Some(evaluate(instruction.program).map_err(Error::Evaluation)?);
                }
            }
            Op::Format => {
                if active {
                    let text = format_number(consume(&mut value)?, &instruction.text)?;
                    output.extend(text);
                }
            }
            Op::If => {
                let selected = active && consume(&mut value)? != 0.;
                branches.push(Branch {
                    parent: active,
                    matched: selected,
                    is_case: false,
                    selector: 0.,
                });
                active = selected;
            }
            Op::Case => {
                let selector = if active { consume(&mut value)? } else { 0. };
                branches.push(Branch {
                    parent: active,
                    matched: false,
                    is_case: true,
                    selector,
                });
                active = false;
            }
            Op::Label => {
                if let Some(branch) = branches.last_mut() {
                    active = branch.parent
                        && !branch.matched
                        && branch.is_case
                        && branch.selector == instruction.number;
                    if active {
                        branch.matched = true;
                    }
                }
            }
            Op::Else => {
                if let Some(branch) = branches.last_mut() {
                    active = branch.parent && !branch.matched;
                    if active {
                        branch.matched = true;
                    }
                }
            }
            Op::End => {
                if let Some(branch) = branches.pop() {
                    active = branch.parent;
                }
            }
            Op::Ignore => {}
        }
        if output.len() > 4095 || branches.len() > 256 {
            return Err(Error::Policy(b"tooltip output/nesting limit"));
        }
    }
    if value.is_some() {
        return Err(Error::Policy(b"tooltip expression missing consumer"));
    }
    Ok(output)
}
