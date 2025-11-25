//! Symbol table for tracking @symbol references.

use std::collections::HashMap;
use uuid::Uuid;
use crate::forth_engine::schema::ast::span::Span;
use crate::forth_engine::schema::types::ContextKey;

/// Symbol table tracks @symbol definitions and resolutions.
#[derive(Debug, Clone, Default)]
pub struct SymbolTable {
    symbols: HashMap<String, SymbolInfo>,
}

/// Information about a defined symbol.
#[derive(Debug, Clone)]
pub struct SymbolInfo {
    /// What type of ID this symbol holds
    pub id_type: ContextKey,
    /// Where it was defined in source
    pub defined_at: Span,
    /// Which verb defined it
    pub defined_by: &'static str,
    /// Resolved UUID (known after execution)
    pub resolved_id: Option<Uuid>,
}

/// Error when defining symbols.
#[derive(Debug, Clone)]
pub enum SymbolError {
    /// Symbol was already defined
    AlreadyDefined {
        name: String,
        first_defined: Span,
        second_defined: Span,
    },
}

impl SymbolTable {
    /// Create a new empty symbol table.
    pub fn new() -> Self {
        Self { symbols: HashMap::new() }
    }

    /// Define a new symbol.
    pub fn define(
        &mut self,
        name: &str,
        id_type: ContextKey,
        span: Span,
        verb_name: &'static str,
    ) -> Result<(), SymbolError> {
        if let Some(existing) = self.symbols.get(name) {
            return Err(SymbolError::AlreadyDefined {
                name: name.to_string(),
                first_defined: existing.defined_at,
                second_defined: span,
            });
        }

        self.symbols.insert(name.to_string(), SymbolInfo {
            id_type,
            defined_at: span,
            defined_by: verb_name,
            resolved_id: None,
        });

        Ok(())
    }

    /// Get symbol info by name.
    pub fn get(&self, name: &str) -> Option<&SymbolInfo> {
        self.symbols.get(name)
    }

    /// Check if a symbol is defined.
    pub fn is_defined(&self, name: &str) -> bool {
        self.symbols.contains_key(name)
    }

    /// Resolve a symbol to its UUID.
    pub fn resolve(&mut self, name: &str, id: Uuid) {
        if let Some(info) = self.symbols.get_mut(name) {
            info.resolved_id = Some(id);
        }
    }

    /// Get the resolved UUID for a symbol.
    pub fn get_resolved(&self, name: &str) -> Option<Uuid> {
        self.symbols.get(name).and_then(|s| s.resolved_id)
    }

    /// Get all defined symbol names.
    pub fn all_names(&self) -> Vec<&str> {
        self.symbols.keys().map(|s| s.as_str()).collect()
    }

    /// Get count of defined symbols.
    pub fn len(&self) -> usize {
        self.symbols.len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.symbols.is_empty()
    }

    /// Iterate over all symbols.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &SymbolInfo)> {
        self.symbols.iter().map(|(k, v)| (k.as_str(), v))
    }
}

impl std::fmt::Display for SymbolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AlreadyDefined { name, first_defined, second_defined } => {
                write!(
                    f,
                    "symbol @{} already defined at line {}, cannot redefine at line {}",
                    name, first_defined.line, second_defined.line
                )
            }
        }
    }
}

impl std::error::Error for SymbolError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_define_and_get() {
        let mut table = SymbolTable::new();
        let span = Span::new(0, 10, 1, 1);
        
        table.define("cbu", ContextKey::CbuId, span, "cbu.ensure").unwrap();
        
        let info = table.get("cbu").unwrap();
        assert_eq!(info.id_type, ContextKey::CbuId);
        assert_eq!(info.defined_by, "cbu.ensure");
    }

    #[test]
    fn test_duplicate_definition() {
        let mut table = SymbolTable::new();
        let span1 = Span::new(0, 10, 1, 1);
        let span2 = Span::new(20, 30, 2, 1);
        
        table.define("cbu", ContextKey::CbuId, span1, "cbu.ensure").unwrap();
        let result = table.define("cbu", ContextKey::CbuId, span2, "cbu.create");
        
        assert!(matches!(result, Err(SymbolError::AlreadyDefined { .. })));
    }

    #[test]
    fn test_resolve() {
        let mut table = SymbolTable::new();
        let span = Span::new(0, 10, 1, 1);
        let id = Uuid::new_v4();
        
        table.define("cbu", ContextKey::CbuId, span, "cbu.ensure").unwrap();
        table.resolve("cbu", id);
        
        assert_eq!(table.get_resolved("cbu"), Some(id));
    }
}
