//! Artwork transforms expressed as calculator sources, without execution or IDs.
use crate::{
    Document, NodeId,
    definition::{Error, ancestors},
};
use std::{fs::File, io::Read};
fn fail(message: impl AsRef<str>) -> Error {
    Error(message.as_ref().to_owned())
}
fn text(doc: &Document, node: NodeId, path: &str) -> String {
    doc.text(node, path).unwrap_or("").to_owned()
}
pub fn coordinates(value: &str) -> Result<(f64, f64), Error> {
    if value.is_empty() {
        return Ok((0., 0.));
    }
    let values = value
        .split(',')
        .map(|s| s.trim().parse::<f64>().ok().filter(|v| v.is_finite()))
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| fail("invalid XML coordinate"))?;
    if values.len() != 2 {
        return Err(fail("XML coordinate needs two values"));
    }
    Ok((values[0], values[1]))
}
fn decimal(value: f64) -> String {
    let mut s = format!("{value:?}");
    if let Some(e) = s.find('e') {
        let exponent = s[e + 1..].parse::<i32>().unwrap();
        s.truncate(e + 1);
        s.push_str(&format!("{exponent:+03}"));
    }
    s
}
fn sum(parts: &[String]) -> String {
    if parts.is_empty() {
        return "0".to_owned();
    }
    let mut result = parts[0].clone();
    for value in &parts[1..] {
        result.push(' ');
        result.push_str(value);
        result.push_str(" +");
    }
    result
}
pub fn image_size(path: &std::path::Path) -> Result<(f64, f64), Error> {
    let mut file = File::open(path).map_err(|e| fail(format!("{}: {e}", path.display())))?;
    let mut head = [0u8; 26];
    file.read_exact(&mut head)
        .map_err(|e| fail(format!("{}: {e}", path.display())))?;
    if &head[..2] == b"BM" {
        let header = u32::from_le_bytes(head[14..18].try_into().unwrap());
        let (w, h) = if header == 12 {
            (
                u16::from_le_bytes(head[18..20].try_into().unwrap()) as i32,
                u16::from_le_bytes(head[20..22].try_into().unwrap()) as i32,
            )
        } else if header >= 40 {
            (
                i32::from_le_bytes(head[18..22].try_into().unwrap()),
                i32::from_le_bytes(head[22..26].try_into().unwrap())
                    .checked_abs()
                    .ok_or_else(|| fail("invalid bitmap height"))?,
            )
        } else {
            return Err(fail("unsupported bitmap header"));
        };
        if w <= 0 || h <= 0 {
            return Err(fail("invalid bitmap dimensions"));
        }
        return Ok((f64::from(w), f64::from(h)));
    }
    if &head[..8] == b"\x89PNG\r\n\x1a\n" {
        let w = u32::from_be_bytes(head[16..20].try_into().unwrap());
        let h = u32::from_be_bytes(head[20..24].try_into().unwrap());
        if w > 0 && h > 0 {
            return Ok((f64::from(w), f64::from(h)));
        }
    }
    Err(fail(format!(
        "unsupported artwork image {}",
        path.display()
    )))
}

#[derive(Clone, Debug, PartialEq)]
pub struct ClipDefinition {
    pub depth: usize,
    pub x: String,
    pub y: String,
    pub width: f64,
    pub height: f64,
}
#[derive(Clone, Debug, PartialEq)]
pub struct ImageDefinition {
    pub x: String,
    pub y: String,
    pub visibility: Option<String>,
    pub clips: Vec<ClipDefinition>,
}
/// Compose root-to-parent positions, clamped shifts and clips, then subtract
/// the image axis. Rotation is rejected rather than silently discarded.
pub fn image(doc: &Document, node: NodeId) -> Result<ImageDefinition, Error> {
    let mut chain = ancestors(doc, node);
    chain.reverse();
    let mut x = Vec::new();
    let mut y = Vec::new();
    let mut visible = Vec::new();
    let mut clips = Vec::new();
    for (depth, parent) in chain.into_iter().enumerate() {
        let (a, b) = coordinates(&text(doc, parent, "FloatPosition"))?;
        x.push(decimal(a));
        y.push(decimal(b));
        let value = text(doc, parent, "Visibility").trim().to_owned();
        if !value.is_empty() {
            visible.push(value + " 0 !=");
        }
        if doc.child(parent, "Rotation").is_some() {
            return Err(fail("unsupported artwork rotation"));
        }
        for &shift in doc
            .node(parent)
            .children
            .iter()
            .filter(|&&n| doc.node(n).tag == "Shift")
        {
            let (a, b) = coordinates(&text(doc, shift, "Scale"))?;
            let expression = doc
                .child(shift, "Expression")
                .ok_or_else(|| fail("artwork Shift has no Expression"))?;
            let mut script = text(doc, expression, "Script").trim().to_owned();
            if script.is_empty() {
                return Err(fail("artwork Shift has no Script"));
            }
            if let Some(min) = doc.text(expression, "Minimum") {
                script.push(' ');
                script.push_str(min);
                script.push_str(" max");
            }
            if let Some(max) = doc.text(expression, "Maximum") {
                script.push(' ');
                script.push_str(max);
                script.push_str(" min");
            }
            if a != 0. {
                x.push(format!("{script} {} *", decimal(a)));
            }
            if b != 0. {
                y.push(format!("{script} {} *", decimal(b)));
            }
        }
        for &clip in doc
            .node(parent)
            .children
            .iter()
            .filter(|&&n| doc.node(n).tag == "ClipRect")
        {
            let (a, b) = coordinates(&text(doc, clip, "FloatPosition"))?;
            let (w, h) = coordinates(&text(doc, clip, "Size"))?;
            if w <= 0. || h <= 0. {
                return Err(fail("invalid artwork clip size"));
            }
            let mut xp = x.clone();
            xp.push(decimal(a));
            let mut yp = y.clone();
            yp.push(decimal(b));
            clips.push(ClipDefinition {
                depth,
                x: sum(&xp),
                y: sum(&yp),
                width: w,
                height: h,
            });
        }
    }
    let value = text(doc, node, "Visibility").trim().to_owned();
    if !value.is_empty() {
        visible.push(value + " 0 !=");
    }
    let (ax, ay) = coordinates(&text(doc, node, "Axis"))?;
    x.push(decimal(-ax));
    y.push(decimal(-ay));
    let visibility = if visible.is_empty() {
        None
    } else {
        let mut expression = visible[0].clone();
        for value in &visible[1..] {
            expression.push(' ');
            expression.push_str(value);
            expression.push_str(" and");
        }
        Some(expression)
    };
    Ok(ImageDefinition {
        x: sum(&x),
        y: sum(&y),
        visibility,
        clips,
    })
}
