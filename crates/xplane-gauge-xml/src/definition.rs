//! Gauge semantics without aircraft names, simulator callbacks or numeric policy.
use crate::{Document, NodeId, programs::Macros};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error(pub String);
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for Error {}

/// Explicitly select the macro spelling used by a document family.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MacroStyle {
    Direct,
    Nested { direct_fallback: bool },
}
pub fn macros(doc: &Document, style: MacroStyle) -> Macros {
    doc.named(doc.root(), "Macro")
        .into_iter()
        .filter_map(|id| {
            let node = doc.node(id);
            let name = node.attribute("Name")?;
            let value = match style {
                MacroStyle::Direct => node.text.as_str(),
                MacroStyle::Nested { direct_fallback } => doc
                    .text(id, "MacroValue")
                    .filter(|s| !s.is_empty())
                    .unwrap_or(if direct_fallback { &node.text } else { "" }),
            };
            Some((name.to_owned(), value.to_owned()))
        })
        .collect()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyBinding {
    /// Child ordinal, including unsupported siblings (which cause an error).
    pub ordinal: usize,
    pub event: String,
    pub expression: String,
}
pub fn keys(doc: &Document, parent: NodeId) -> Result<Vec<KeyBinding>, Error> {
    doc.node(parent)
        .children
        .iter()
        .enumerate()
        .map(|(ordinal, &id)| {
            let node = doc.node(id);
            if node.tag != "On" || node.attributes.len() != 1 || !node.children.is_empty() {
                return Err(Error("unsupported key handler XML element".into()));
            }
            let event = node
                .attribute("Event")
                .ok_or_else(|| Error("XML key handler has no event".into()))?
                .trim()
                .to_ascii_uppercase();
            if event.is_empty() {
                return Err(Error("XML key handler has empty event".into()));
            }
            Ok(KeyBinding {
                ordinal,
                event,
                expression: node.text.clone(),
            })
        })
        .collect()
}

/// SDK-compatible mouse event masks. Unknown spellings are errors; any legacy
/// aliases or ignored tokens belong to the caller's compatibility policy.
pub fn click_flags(value: &str) -> Result<u32, Error> {
    let value = if value.is_empty() {
        "LeftSingle"
    } else {
        value
    };
    value.split('+').map(str::trim).try_fold(0, |mask, name| {
        let bit = match name {
            "None" | "LeftSingle" => 0x20000000,
            "LeftDrag" => 0x00800000,
            "LeftRelease" => 0x00020000,
            "RightSingle" => 0x80000000,
            "RightDrag" => 0x02000000,
            "RightRelease" => 0x00080000,
            "MiddleSingle" => 0x40000000,
            "MiddleDrag" => 0x01000000,
            "MiddleRelease" => 0x00040000,
            "LeftDouble" => 0x04000000,
            "RightDouble" => 0x10000000,
            "MiddleDouble" => 0x08000000,
            "WheelUp" => 0x00004000,
            "WheelDown" => 0x00002000,
            "Wheel" => 0x00006000,
            _ => return Err(Error(format!("unsupported XML click type {name}"))),
        };
        Ok(mask | bit)
    })
}

/// Ancestors from immediate parent to root.
pub fn ancestors(doc: &Document, node: NodeId) -> Vec<NodeId> {
    let mut result = Vec::new();
    let mut parent = doc.node(node).parent;
    while let Some(id) = parent {
        result.push(id);
        parent = doc.node(id).parent;
    }
    result
}
/// Conditions in root-to-leaf order, including the selected element.
pub fn visibility(doc: &Document, node: NodeId) -> Vec<String> {
    let mut chain = ancestors(doc, node);
    chain.reverse();
    chain.push(node);
    chain
        .into_iter()
        .filter_map(|id| doc.text(id, "Visibility"))
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}
pub fn visibility_expression(doc: &Document, node: NodeId) -> Option<String> {
    let values = visibility(doc, node);
    if values.is_empty() {
        return None;
    }
    Some(
        values
            .iter()
            .enumerate()
            .map(|(i, s)| format!("{s} 0 !={}", if i == 0 { "" } else { " and" }))
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

/// At most one direct MouseClick is supported per area.
pub fn mouse_click(doc: &Document, area: NodeId) -> Result<Option<NodeId>, Error> {
    let mut clicks = doc
        .node(area)
        .children
        .iter()
        .copied()
        .filter(|&id| doc.node(id).tag == "MouseClick");
    let first = clicks.next();
    if clicks.next().is_some() {
        return Err(Error(
            "multiple source XML MouseClick nodes on one area".into(),
        ));
    }
    Ok(first)
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClickAction {
    pub expression: String,
    /// Diagnostic suffix retaining whether the action came from an event or script.
    pub element: &'static str,
}
pub fn click_action(doc: &Document, click: NodeId) -> Result<ClickAction, Error> {
    let key = doc.text(click, "KeyEvent").unwrap_or("").trim();
    let body = doc.text(click, "Script").unwrap_or("");
    if !key.is_empty() && !body.trim().is_empty() {
        return Err(Error("both source XML key event and script".into()));
    }
    Ok(if key.is_empty() {
        ClickAction {
            expression: body.to_owned(),
            element: "Script",
        }
    } else {
        ClickAction {
            expression: format!("(>K:{key})"),
            element: "KeyEvent",
        }
    })
}
