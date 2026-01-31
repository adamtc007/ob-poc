//! String table builder for deduplication.

use std::collections::HashMap;

/// Builder for deduplicated string tables.
///
/// Strings are stored once and referenced by index.
#[derive(Debug, Default)]
pub struct StringTableBuilder {
    /// Index of strings (string → index).
    index: HashMap<String, u32>,
    /// Ordered strings.
    strings: Vec<String>,
}

impl StringTableBuilder {
    /// Create a new empty string table builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create with pre-allocated capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            index: HashMap::with_capacity(capacity),
            strings: Vec::with_capacity(capacity),
        }
    }

    /// Intern a string, returning its index.
    ///
    /// If the string already exists, returns the existing index.
    pub fn intern(&mut self, s: impl Into<String>) -> u32 {
        let s = s.into();

        if let Some(&idx) = self.index.get(&s) {
            return idx;
        }

        let idx = self.strings.len() as u32;
        self.index.insert(s.clone(), idx);
        self.strings.push(s);
        idx
    }

    /// Get the index of a string without interning.
    pub fn get(&self, s: &str) -> Option<u32> {
        self.index.get(s).copied()
    }

    /// Get a string by index.
    pub fn lookup(&self, idx: u32) -> Option<&str> {
        self.strings.get(idx as usize).map(|s| s.as_str())
    }

    /// Get the current table size.
    pub fn len(&self) -> usize {
        self.strings.len()
    }

    /// Check if the table is empty.
    pub fn is_empty(&self) -> bool {
        self.strings.is_empty()
    }

    /// Consume the builder and return the string table.
    pub fn build(self) -> Vec<String> {
        self.strings
    }

    /// Get total byte size of all strings.
    pub fn total_bytes(&self) -> usize {
        self.strings.iter().map(|s| s.len()).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_table_basic() {
        let mut builder = StringTableBuilder::new();

        let idx_a = builder.intern("hello");
        let idx_b = builder.intern("world");
        let idx_a2 = builder.intern("hello"); // Duplicate

        assert_eq!(idx_a, idx_a2); // Same string, same index
        assert_ne!(idx_a, idx_b);
        assert_eq!(builder.len(), 2);
    }

    #[test]
    fn string_table_lookup() {
        let mut builder = StringTableBuilder::new();
        builder.intern("first");
        builder.intern("second");

        assert_eq!(builder.lookup(0), Some("first"));
        assert_eq!(builder.lookup(1), Some("second"));
        assert_eq!(builder.lookup(2), None);
    }

    #[test]
    fn string_table_get() {
        let mut builder = StringTableBuilder::new();
        builder.intern("exists");

        assert_eq!(builder.get("exists"), Some(0));
        assert_eq!(builder.get("missing"), None);
    }

    #[test]
    fn string_table_build() {
        let mut builder = StringTableBuilder::new();
        builder.intern("a");
        builder.intern("b");
        builder.intern("c");

        let table = builder.build();
        assert_eq!(table, vec!["a", "b", "c"]);
    }
}
