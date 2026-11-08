use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};

pub const INT: i32 = 1;
pub const FLOAT: i32 = 2;
pub const DOUBLE: i32 = 4;
pub const FLOAT_ARRAY: i32 = 8;
pub const INT_ARRAY: i32 = 16;
pub const DATA: i32 = 32;
pub const MAX_ELEMENTS: usize = 1_048_576;

#[derive(Clone, Debug, Default)]
pub struct Entry {
    pub name: String,
    pub types: i32,
    pub writable: bool,
    pub owned: bool,
    pub alive: bool,
    pub scalar: f64,
    pub floats: Vec<f32>,
    pub ints: Vec<i32>,
    pub bytes: Vec<u8>,
}
impl Entry {
    pub fn new(name: &str, types: i32, writable: bool, owned: bool) -> Self {
        Self {
            name: name.into(),
            types,
            writable,
            owned,
            alive: true,
            ..Self::default()
        }
    }
    pub fn len(&self) -> usize {
        if self.types & FLOAT_ARRAY != 0 {
            self.floats.len()
        } else if self.types & INT_ARRAY != 0 {
            self.ints.len()
        } else if self.types & DATA != 0 {
            self.bytes.len()
        } else {
            1
        }
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn metadata(&self, id: usize) -> Metadata {
        Metadata {
            id,
            name: self.name.clone(),
            types: self.types,
            value_type: type_name(self.types).into(),
            is_writable: self.writable,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Metadata {
    pub id: usize,
    pub name: String,
    pub types: i32,
    pub value_type: String,
    pub is_writable: bool,
}
pub fn type_name(t: i32) -> &'static str {
    if t & FLOAT_ARRAY != 0 {
        "float_array"
    } else if t & INT_ARRAY != 0 {
        "int_array"
    } else if t & DATA != 0 {
        "data"
    } else if t & DOUBLE != 0 && t & FLOAT == 0 {
        "double"
    } else if t & INT != 0 {
        "int"
    } else {
        "float"
    }
}

/// IDs are never recycled. Stale foreign handles cannot alias a new registration.
#[derive(Default)]
pub struct Store {
    pub entries: Vec<Entry>,
    names: HashMap<String, usize>,
    pub misses: BTreeSet<String>,
}
impl Store {
    pub fn insert(&mut self, entry: Entry) -> usize {
        let id = self.entries.len();
        self.names.insert(entry.name.clone(), id);
        self.entries.push(entry);
        id
    }
    pub fn find(&self, name: &str) -> Option<usize> {
        self.names
            .get(name)
            .copied()
            .filter(|&id| self.entries[id].alive)
    }
    pub fn lookup(&mut self, name: &str) -> Option<usize> {
        let id = self.find(name);
        if id.is_none() {
            self.misses.insert(name.into());
        }
        id
    }
    pub fn ensure(&mut self, name: &str, types: i32) -> &mut Entry {
        let id = self
            .find(name)
            .unwrap_or_else(|| self.insert(Entry::new(name, types, true, false)));
        &mut self.entries[id]
    }
    pub fn scalar(&self, name: &str) -> f64 {
        self.find(name)
            .map(|i| self.entries[i].scalar)
            .unwrap_or_default()
    }
    pub fn set(&mut self, name: &str, value: f64) {
        if let Some(i) = self.find(name) {
            self.entries[i].scalar = value;
        }
    }
    pub fn seed(&mut self, name: &str, types: i32, value: f64) {
        self.ensure(name, types).scalar = value;
    }
    pub fn seed_string(&mut self, name: &str, value: &str) {
        let e = self.ensure(name, DATA);
        e.bytes = value.as_bytes().to_vec();
        e.bytes.push(0);
    }
    pub fn load_catalog(&mut self, text: &str) -> Result<usize, String> {
        let mut count = 0;
        for line in text.lines() {
            let p: Vec<_> = line.trim().split('\t').map(str::trim).collect();
            if p.len() < 3 || !p[0].contains('/') || self.find(p[0]).is_some() {
                continue;
            }
            let (base, len) = if let Some((base, length)) = p[1].split_once('[') {
                (
                    base,
                    length
                        .trim_end_matches(']')
                        .parse::<usize>()
                        .map_err(|_| "invalid catalog array length")?
                        .max(1),
                )
            } else {
                (p[1], 1)
            };
            if len > MAX_ELEMENTS {
                return Err("catalog array too large".into());
            }
            let types = match (base, len) {
                ("float" | "double", 1) | ("double", _) => FLOAT | DOUBLE,
                ("int", 1) => INT,
                ("float", _) => FLOAT_ARRAY,
                ("int", _) => INT_ARRAY,
                ("byte", _) => DATA,
                _ => continue,
            };
            let mut e = Entry::new(p[0], types, p[2] == "y", false);
            match types {
                FLOAT_ARRAY => e.floats.resize(len, 0.),
                INT_ARRAY => e.ints.resize(len, 0),
                DATA => e.bytes.resize(len, 0),
                _ => (),
            }
            self.insert(e);
            count += 1;
        }
        Ok(count)
    }
}

pub fn read_range<T: Copy>(values: &[T], offset: usize, out: &mut [T]) -> usize {
    let tail = values.get(offset..).unwrap_or_default();
    let n = tail.len().min(out.len());
    out[..n].copy_from_slice(&tail[..n]);
    n
}
pub fn write_range<T: Copy + Default>(values: &mut Vec<T>, offset: usize, input: &[T]) -> bool {
    let Some(end) = offset
        .checked_add(input.len())
        .filter(|&n| n <= MAX_ELEMENTS)
    else {
        return false;
    };
    if end > values.len() {
        values.resize(end, T::default());
    }
    values[offset..end].copy_from_slice(input);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stale_ids_and_missing_names() {
        let mut s = Store::default();
        let old = s.insert(Entry::new("a/b", INT, true, true));
        s.entries[old].alive = false;
        assert_eq!(s.lookup("a/b"), None);
        let new = s.insert(Entry::new("a/b", FLOAT, true, true));
        assert_ne!(new, old);
        assert_eq!(s.find("a/b"), Some(new));
        assert_eq!(s.misses.len(), 1);
    }
    #[test]
    fn bounded_ranges_and_catalog() {
        let mut a = vec![1, 2];
        assert!(!write_range(&mut a, usize::MAX, &[3]));
        assert_eq!(a, [1, 2]);
        assert!(write_range(&mut a, 3, &[4]));
        assert_eq!(a, [1, 2, 0, 4]);
        let mut s = Store::default();
        assert_eq!(
            s.load_catalog("sim/a\tfloat[4]\ty\nsim/b\tbyte[8]\tn")
                .unwrap(),
            2
        );
        assert_eq!(s.entries[0].types, FLOAT_ARRAY);
        assert_eq!(s.entries[1].bytes.len(), 8);
    }
}
