//! Binding context for dataflow validation
//!
//! Tracks bindings produced by DSL statements and validates that
//! references to bindings are satisfied before use.
//!
//! This module contains the core data types. The functions that depend
//! on RuntimeVerbRegistry remain in ob-poc.

use std::collections::HashMap;
use uuid::Uuid;

use crate::config::types::VerbProduces;

// =============================================================================
// BINDING INFO
// =============================================================================

/// Information about a binding extracted from DSL
#[derive(Debug, Clone)]
pub struct BindingInfo {
    /// The binding name (without @): "fund", "john"
    pub name: String,
    /// The type of entity: "cbu", "entity", "case", "workstream"
    pub produced_type: String,
    /// Optional subtype: "proper_person", "limited_company"
    pub subtype: Option<String>,
    /// The resolved primary key (Uuid::nil() if not yet executed)
    pub entity_pk: Uuid,
    /// True if this is a lookup (resolved existing) vs create (new)
    pub resolved: bool,
    /// Source sheet ID (for audit trail)
    pub source_sheet_id: Option<Uuid>,
}

impl BindingInfo {
    /// Check if this binding matches an expected type
    ///
    /// Supports:
    /// - Exact match: "cbu" matches "cbu"
    /// - Base type match: "entity" matches "entity.proper_person"
    /// - Full type match: "entity.proper_person" matches "entity.proper_person"
    pub fn matches_type(&self, expected: &str) -> bool {
        // Exact match on produced_type
        if self.produced_type == expected {
            return true;
        }

        // Check if expected is a full type "entity.proper_person"
        if let Some((base, sub)) = expected.split_once('.') {
            // Match if base type matches and subtype matches
            if self.produced_type == base {
                if let Some(ref my_sub) = self.subtype {
                    return my_sub == sub;
                }
            }
        }

        // Check if this binding has a subtype that matches
        if let Some(ref subtype) = self.subtype {
            let full_type = format!("{}.{}", self.produced_type, subtype);
            if full_type == expected {
                return true;
            }
        }

        false
    }

    /// Create from a VerbProduces definition
    pub fn from_produces(name: &str, produces: &VerbProduces) -> Self {
        Self {
            name: name.to_string(),
            produced_type: produces.produced_type.clone(),
            subtype: produces.subtype.clone(),
            entity_pk: Uuid::nil(), // Not yet executed
            resolved: produces.resolved,
            source_sheet_id: None,
        }
    }

    /// Format for display: "@fund (cbu)" or "@john (entity/proper_person)"
    pub fn display(&self) -> String {
        let type_str = match &self.subtype {
            Some(sub) => format!("{}/{}", self.produced_type, sub),
            None => self.produced_type.clone(),
        };
        format!("@{} ({})", self.name, type_str)
    }
}

// =============================================================================
// BINDING CONTEXT
// =============================================================================

/// Accumulated binding context for validation
#[derive(Debug, Clone, Default)]
pub struct BindingContext {
    bindings: HashMap<String, BindingInfo>,
}

impl BindingContext {
    pub fn new() -> Self {
        Self::default()
    }

    /// Merge another context into this one
    pub fn merge(&mut self, other: &BindingContext) {
        for (name, info) in &other.bindings {
            self.bindings.insert(name.clone(), info.clone());
        }
    }

    /// Get a binding by name (without @)
    pub fn get(&self, name: &str) -> Option<&BindingInfo> {
        self.bindings.get(name)
    }

    /// Check if a binding exists
    pub fn contains(&self, name: &str) -> bool {
        self.bindings.contains_key(name)
    }

    /// Get all binding names
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.bindings.keys().map(|s| s.as_str())
    }

    /// Get all bindings
    pub fn all(&self) -> impl Iterator<Item = &BindingInfo> {
        self.bindings.values()
    }

    /// Insert a binding
    pub fn insert(&mut self, info: BindingInfo) {
        self.bindings.insert(info.name.clone(), info);
    }

    /// Get the set of available types (for verb satisfaction checking)
    pub fn available_types(&self) -> std::collections::HashSet<String> {
        let mut types = std::collections::HashSet::new();
        for info in self.bindings.values() {
            types.insert(info.produced_type.clone());
            if let Some(ref sub) = info.subtype {
                types.insert(format!("{}.{}", info.produced_type, sub));
            }
        }
        types
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }

    /// Count bindings
    pub fn len(&self) -> usize {
        self.bindings.len()
    }

    /// Format for LLM context
    pub fn to_llm_context(&self) -> String {
        if self.is_empty() {
            return "No bindings available.".to_string();
        }

        let mut lines = vec!["Available bindings:".to_string()];
        for info in self.bindings.values() {
            let pk_str = if info.entity_pk.is_nil() {
                "[pending]".to_string()
            } else {
                info.entity_pk.to_string()
            };
            lines.push(format!("  {} → pk: {}", info.display(), pk_str));
        }
        lines.join("\n")
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_binding_info_matches_type() {
        let cbu = BindingInfo {
            name: "fund".to_string(),
            produced_type: "cbu".to_string(),
            subtype: None,
            entity_pk: Uuid::new_v4(),
            resolved: false,
            source_sheet_id: None,
        };

        assert!(cbu.matches_type("cbu"));
        assert!(!cbu.matches_type("entity"));
        assert!(!cbu.matches_type("cbu.fund"));
    }

    #[test]
    fn test_binding_info_matches_subtype() {
        let person = BindingInfo {
            name: "john".to_string(),
            produced_type: "entity".to_string(),
            subtype: Some("proper_person".to_string()),
            entity_pk: Uuid::new_v4(),
            resolved: false,
            source_sheet_id: None,
        };

        // Base type matches
        assert!(person.matches_type("entity"));
        // Full type matches
        assert!(person.matches_type("entity.proper_person"));
        // Different subtype doesn't match
        assert!(!person.matches_type("entity.limited_company"));
        // Different base type doesn't match
        assert!(!person.matches_type("cbu"));
    }

    #[test]
    fn test_binding_context_available_types() {
        let mut ctx = BindingContext::new();

        ctx.insert(BindingInfo {
            name: "fund".to_string(),
            produced_type: "cbu".to_string(),
            subtype: None,
            entity_pk: Uuid::nil(),
            resolved: false,
            source_sheet_id: None,
        });

        ctx.insert(BindingInfo {
            name: "john".to_string(),
            produced_type: "entity".to_string(),
            subtype: Some("proper_person".to_string()),
            entity_pk: Uuid::nil(),
            resolved: false,
            source_sheet_id: None,
        });

        let types = ctx.available_types();
        assert!(types.contains("cbu"));
        assert!(types.contains("entity"));
        assert!(types.contains("entity.proper_person"));
        assert!(!types.contains("case"));
    }

    #[test]
    fn test_binding_context_merge() {
        let mut ctx1 = BindingContext::new();
        ctx1.insert(BindingInfo {
            name: "fund".to_string(),
            produced_type: "cbu".to_string(),
            subtype: None,
            entity_pk: Uuid::new_v4(),
            resolved: false,
            source_sheet_id: None,
        });

        let mut ctx2 = BindingContext::new();
        ctx2.insert(BindingInfo {
            name: "john".to_string(),
            produced_type: "entity".to_string(),
            subtype: Some("proper_person".to_string()),
            entity_pk: Uuid::new_v4(),
            resolved: false,
            source_sheet_id: None,
        });

        ctx1.merge(&ctx2);

        assert!(ctx1.contains("fund"));
        assert!(ctx1.contains("john"));
        assert_eq!(ctx1.len(), 2);
    }
}
