//! Execution Context
//!
//! The `ExecutionContext` holds state during DSL execution, including:
//! - Symbol table for @reference resolution
//! - Parent/child hierarchy for batch execution
//! - Audit and idempotency tracking

use std::collections::HashMap;
use uuid::Uuid;

/// Execution context holding state during DSL execution
///
/// Supports parent/child hierarchy for batch execution where each iteration
/// has its own symbol scope but can read from parent (shared) bindings.
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    /// Symbol table for @reference resolution (local scope)
    pub symbols: HashMap<String, Uuid>,
    /// Symbol types - maps binding name to entity type (e.g., "fund" -> "cbu")
    pub symbol_types: HashMap<String, String>,
    /// Parent symbols (read-only, inherited from parent context)
    /// Used in batch execution where shared bindings are accessible to all iterations
    pub parent_symbols: HashMap<String, Uuid>,
    /// Parent symbol types
    pub parent_symbol_types: HashMap<String, String>,
    /// Batch iteration index (None if not in batch context)
    pub batch_index: Option<usize>,
    /// Audit user for tracking
    pub audit_user: Option<String>,
    /// Transaction ID for grouping operations
    pub transaction_id: Option<Uuid>,
    /// Execution ID for idempotency tracking (auto-generated if not set)
    pub execution_id: Uuid,
    /// Whether idempotency checking is enabled
    pub idempotency_enabled: bool,
}

impl Default for ExecutionContext {
    fn default() -> Self {
        Self {
            symbols: HashMap::new(),
            symbol_types: HashMap::new(),
            parent_symbols: HashMap::new(),
            parent_symbol_types: HashMap::new(),
            batch_index: None,
            audit_user: None,
            transaction_id: None,
            execution_id: Uuid::new_v4(),
            idempotency_enabled: true,
        }
    }
}

impl ExecutionContext {
    /// Create a new empty execution context
    pub fn new() -> Self {
        Self::default()
    }

    /// Create context with a specific execution ID (for resumable executions)
    pub fn with_execution_id(execution_id: Uuid) -> Self {
        Self {
            execution_id,
            ..Self::default()
        }
    }

    /// Bind a symbol to a UUID value
    pub fn bind(&mut self, name: &str, value: Uuid) {
        self.symbols.insert(name.to_string(), value);
    }

    /// Bind a symbol with its entity type
    pub fn bind_typed(&mut self, name: &str, value: Uuid, entity_type: &str) {
        self.symbols.insert(name.to_string(), value);
        self.symbol_types
            .insert(name.to_string(), entity_type.to_string());
    }

    /// Resolve a symbol reference, checking local scope first then parent
    pub fn resolve(&self, name: &str) -> Option<Uuid> {
        // 1. Check local symbols first
        if let Some(pk) = self.symbols.get(name) {
            return Some(*pk);
        }
        // 2. Fall back to parent symbols
        if let Some(pk) = self.parent_symbols.get(name) {
            return Some(*pk);
        }
        None
    }

    /// Check if a symbol exists (in local or parent scope)
    pub fn has(&self, name: &str) -> bool {
        self.symbols.contains_key(name) || self.parent_symbols.contains_key(name)
    }

    /// Get the entity type for a binding
    pub fn get_binding_type(&self, name: &str) -> Option<&str> {
        // Check local first, then parent
        self.symbol_types
            .get(name)
            .or_else(|| self.parent_symbol_types.get(name))
            .map(|s| s.as_str())
    }

    /// Get all effective bindings (local + parent, local wins on conflict)
    pub fn effective_symbols(&self) -> HashMap<String, Uuid> {
        let mut result = self.parent_symbols.clone();
        result.extend(self.symbols.clone());
        result
    }

    /// Get all effective symbol types
    pub fn effective_symbol_types(&self) -> HashMap<String, String> {
        let mut result = self.parent_symbol_types.clone();
        result.extend(self.symbol_types.clone());
        result
    }

    /// Get all bindings as string map (for template expansion)
    pub fn all_bindings_as_strings(&self) -> HashMap<String, String> {
        self.effective_symbols()
            .iter()
            .map(|(k, v)| (k.clone(), v.to_string()))
            .collect()
    }

    /// Create a child context for a batch iteration
    ///
    /// The child has:
    /// - Fresh local symbols (empty)
    /// - Parent symbols inherited from this context's effective symbols
    /// - Same execution_id and other settings
    pub fn child_for_iteration(&self, index: usize) -> Self {
        Self {
            symbols: HashMap::new(),
            symbol_types: HashMap::new(),
            parent_symbols: self.effective_symbols(),
            parent_symbol_types: self.effective_symbol_types(),
            batch_index: Some(index),
            audit_user: self.audit_user.clone(),
            transaction_id: self.transaction_id,
            execution_id: self.execution_id,
            idempotency_enabled: self.idempotency_enabled,
        }
    }

    /// Merge bindings from another context into this one
    pub fn merge_bindings(&mut self, other: &ExecutionContext) {
        self.symbols.extend(other.symbols.clone());
        self.symbol_types.extend(other.symbol_types.clone());
    }

    /// Get the number of local bindings
    pub fn local_binding_count(&self) -> usize {
        self.symbols.len()
    }

    /// Get the total number of bindings (local + parent)
    pub fn total_binding_count(&self) -> usize {
        self.effective_symbols().len()
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_default() {
        let ctx = ExecutionContext::new();
        assert!(ctx.symbols.is_empty());
        assert!(ctx.parent_symbols.is_empty());
        assert!(ctx.batch_index.is_none());
        assert!(ctx.idempotency_enabled);
    }

    #[test]
    fn test_bind_and_resolve() {
        let mut ctx = ExecutionContext::new();
        let pk = Uuid::new_v4();

        ctx.bind("fund", pk);
        assert_eq!(ctx.resolve("fund"), Some(pk));
        assert_eq!(ctx.resolve("nonexistent"), None);
    }

    #[test]
    fn test_bind_typed() {
        let mut ctx = ExecutionContext::new();
        let pk = Uuid::new_v4();

        ctx.bind_typed("fund", pk, "cbu");
        assert_eq!(ctx.resolve("fund"), Some(pk));
        assert_eq!(ctx.get_binding_type("fund"), Some("cbu"));
    }

    #[test]
    fn test_parent_child_resolution() {
        let mut parent = ExecutionContext::new();
        let parent_pk = Uuid::new_v4();
        parent.bind_typed("shared", parent_pk, "cbu");

        let mut child = parent.child_for_iteration(0);
        let child_pk = Uuid::new_v4();
        child.bind_typed("local", child_pk, "entity");

        // Child can resolve both local and parent
        assert_eq!(child.resolve("local"), Some(child_pk));
        assert_eq!(child.resolve("shared"), Some(parent_pk));

        // Parent can only resolve its own
        assert_eq!(parent.resolve("shared"), Some(parent_pk));
        assert_eq!(parent.resolve("local"), None);
    }

    #[test]
    fn test_local_shadows_parent() {
        let mut parent = ExecutionContext::new();
        let parent_pk = Uuid::new_v4();
        parent.bind("x", parent_pk);

        let mut child = parent.child_for_iteration(0);
        let child_pk = Uuid::new_v4();
        child.bind("x", child_pk);

        // Child's local binding shadows parent
        assert_eq!(child.resolve("x"), Some(child_pk));
    }

    #[test]
    fn test_effective_symbols() {
        let mut parent = ExecutionContext::new();
        parent.bind("a", Uuid::new_v4());
        parent.bind("b", Uuid::new_v4());

        let mut child = parent.child_for_iteration(0);
        child.bind("c", Uuid::new_v4());

        let effective = child.effective_symbols();
        assert!(effective.contains_key("a"));
        assert!(effective.contains_key("b"));
        assert!(effective.contains_key("c"));
        assert_eq!(effective.len(), 3);
    }

    #[test]
    fn test_batch_index() {
        let parent = ExecutionContext::new();
        let child = parent.child_for_iteration(5);

        assert_eq!(parent.batch_index, None);
        assert_eq!(child.batch_index, Some(5));
    }

    #[test]
    fn test_with_execution_id() {
        let exec_id = Uuid::new_v4();
        let ctx = ExecutionContext::with_execution_id(exec_id);
        assert_eq!(ctx.execution_id, exec_id);
    }

    #[test]
    fn test_has() {
        let mut ctx = ExecutionContext::new();
        ctx.bind("exists", Uuid::new_v4());

        assert!(ctx.has("exists"));
        assert!(!ctx.has("not_exists"));
    }
}
