//! Owned XML input for loading simulator gauge definitions.
//!
//! Element IDs follow document preorder and exclude comments and text nodes.
//! The tree owns decoded strings, so consumers can retain it after releasing the
//! input buffer. XML parsing uses `roxmltree`; external entities and DTDs are
//! disabled. This crate contains no aircraft definitions or execution policy.
#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

use std::collections::{BTreeMap, HashMap};

pub mod definition;
pub mod geometry;
pub mod presentation;
pub mod programs;
pub mod template;

const MAX_BYTES: usize = 32 * 1024 * 1024;
const MAX_NODES: u32 = 2_000_000;

pub type NodeId = usize;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Node {
    pub tag: String,
    pub attributes: BTreeMap<String, String>,
    /// Decoded direct text before the first child element. Child tail text is
    /// separate XML content and is not included, matching ElementTree `.text`.
    pub text: String,
    pub children: Vec<NodeId>,
    pub parent: Option<NodeId>,
}

impl Node {
    pub fn attribute(&self, name: &str) -> Option<&str> {
        self.attributes.get(name).map(String::as_str)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Document {
    nodes: Vec<Node>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    TooLarge,
    InvalidUtf8,
    Xml(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooLarge => f.write_str("XML input exceeds the size limit"),
            Self::InvalidUtf8 => f.write_str("XML input is not UTF-8"),
            Self::Xml(error) => f.write_str(error),
        }
    }
}
impl std::error::Error for Error {}

fn expanded_name(namespace: Option<&str>, local: &str) -> String {
    match namespace {
        Some(namespace) => format!("{{{namespace}}}{local}"),
        None => local.to_owned(),
    }
}

impl Document {
    pub fn parse(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() > MAX_BYTES {
            return Err(Error::TooLarge);
        }
        let bytes = bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(bytes);
        let text = std::str::from_utf8(bytes).map_err(|_| Error::InvalidUtf8)?;
        let tree = roxmltree::Document::parse_with_options(
            text,
            roxmltree::ParsingOptions {
                allow_dtd: false,
                nodes_limit: MAX_NODES,
                entity_resolver: None,
            },
        )
        .map_err(|error| Error::Xml(error.to_string()))?;
        let elements: Vec<_> = tree
            .descendants()
            .filter(|node| node.is_element())
            .collect();
        let ids: HashMap<_, _> = elements
            .iter()
            .enumerate()
            .map(|(id, node)| (node.id(), id))
            .collect();
        let nodes = elements
            .into_iter()
            .map(|node| {
                let mut text = String::new();
                for child in node.children() {
                    if child.is_element() {
                        break;
                    }
                    if child.is_text() {
                        text.push_str(child.text().unwrap_or(""));
                    }
                }
                Node {
                    tag: expanded_name(node.tag_name().namespace(), node.tag_name().name()),
                    attributes: node
                        .attributes()
                        .map(|attribute| {
                            (
                                expanded_name(attribute.namespace(), attribute.name()),
                                attribute.value().to_owned(),
                            )
                        })
                        .collect(),
                    text,
                    children: node
                        .children()
                        .filter_map(|child| ids.get(&child.id()).copied())
                        .collect(),
                    parent: node
                        .parent()
                        .and_then(|parent| ids.get(&parent.id()).copied()),
                }
            })
            .collect();
        Ok(Self { nodes })
    }

    pub fn root(&self) -> NodeId {
        0
    }

    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    pub fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id]
    }

    pub fn child(&self, id: NodeId, tag: &str) -> Option<NodeId> {
        self.node(id)
            .children
            .iter()
            .copied()
            .find(|child| self.node(*child).tag == tag)
    }

    /// Follow direct child elements. An empty path returns the starting node.
    /// This is deliberately not an XPath interpreter.
    pub fn find(&self, mut id: NodeId, path: &str) -> Option<NodeId> {
        if path.is_empty() {
            return Some(id);
        }
        for tag in path.split('/') {
            if tag.is_empty() {
                return None;
            }
            id = self.child(id, tag)?;
        }
        Some(id)
    }

    pub fn text(&self, id: NodeId, path: &str) -> Option<&str> {
        self.find(id, path)
            .map(|node| self.node(node).text.as_str())
    }

    /// Element preorder, including the starting element.
    pub fn descendants(&self, id: NodeId) -> Vec<NodeId> {
        let mut result = Vec::new();
        let mut pending = vec![id];
        while let Some(id) = pending.pop() {
            result.push(id);
            pending.extend(self.node(id).children.iter().rev().copied());
        }
        result
    }

    pub fn named(&self, id: NodeId, tag: &str) -> Vec<NodeId> {
        self.descendants(id)
            .into_iter()
            .filter(|id| self.node(*id).tag == tag)
            .collect()
    }
}
