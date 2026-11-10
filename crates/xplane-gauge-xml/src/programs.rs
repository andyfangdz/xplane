//! Ordered calculator sources and their bindings, independent of a VM or file format.
use std::collections::BTreeMap;

pub type Macros = BTreeMap<String, String>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceProgram {
    pub stage: String,
    pub source: String,
    pub expression: String,
    pub macros: Macros,
}

/// IDs are one-based registration ordinals; zero is available to the host for
/// an absent binding. Equal expressions remain separate registrations so their
/// source locations and execution order are retained.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Programs {
    entries: Vec<SourceProgram>,
}
impl Programs {
    pub fn register(&mut self, program: SourceProgram) -> usize {
        self.entries.push(program);
        self.entries.len()
    }
    pub fn entries(&self) -> &[SourceProgram] {
        &self.entries
    }
    pub fn into_entries(self) -> Vec<SourceProgram> {
        self.entries
    }
}
