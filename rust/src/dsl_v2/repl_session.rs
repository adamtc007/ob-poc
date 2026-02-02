//! REPL Session State
//!
//! Tracks previously executed bindings for incremental REPL execution.
//! Supports undo functionality for interactive development.

use std::collections::HashMap;
use uuid::Uuid;

use super::ast::Program;
use super::binding_context::{BindingContext, BindingInfo};

/// Represents one executed "block" of DSL
#[derive(Clone, Debug)]
pub struct ExecutedBlock {
    /// The parsed program that was executed
    pub program: Program,
    /// Bindings created during this block's execution: symbol name → UUID
    pub bindings_created: HashMap<String, Uuid>,
    /// Entity types for bindings: symbol name → entity type
    pub binding_types: HashMap<String, String>,
    /// Optional block identifier for tracking
    pub block_id: Option<String>,
}

impl ExecutedBlock {
    /// Create a new executed block
    pub fn new(
        program: Program,
        bindings: HashMap<String, Uuid>,
        types: HashMap<String, String>,
    ) -> Self {
        Self {
            program,
            bindings_created: bindings,
            binding_types: types,
            block_id: None,
        }
    }

    /// Create with a block ID
    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.block_id = Some(id.into());
        self
    }

    /// Get binding count
    pub fn binding_count(&self) -> usize {
        self.bindings_created.len()
    }
}

/// Session state for incremental REPL execution
///
/// Tracks all bindings from previously executed DSL blocks,
/// allowing new DSL to reference them.
#[derive(Clone, Debug, Default)]
pub struct ReplSession {
    /// Stack of executed blocks (for undo)
    blocks: Vec<ExecutedBlock>,
    /// Flattened view of all current bindings: name → UUID
    all_bindings: HashMap<String, Uuid>,
    /// Entity types for all bindings: name → entity type
    all_types: HashMap<String, String>,
    /// Session identifier
    session_id: Option<Uuid>,
}

impl ReplSession {
    /// Create a new empty session
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new session with an ID
    pub fn with_id(id: Uuid) -> Self {
        Self {
            session_id: Some(id),
            ..Self::default()
        }
    }

    /// Get session ID if set
    pub fn session_id(&self) -> Option<Uuid> {
        self.session_id
    }

    /// Record a successfully executed block
    pub fn append_executed(
        &mut self,
        program: Program,
        bindings: HashMap<String, Uuid>,
        types: HashMap<String, String>,
    ) {
        // Add bindings to flattened view
        for (name, pk) in &bindings {
            self.all_bindings.insert(name.clone(), *pk);
        }
        for (name, ty) in &types {
            self.all_types.insert(name.clone(), ty.clone());
        }

        // Push block onto stack
        self.blocks
            .push(ExecutedBlock::new(program, bindings, types));
    }

    /// Undo the last executed block
    ///
    /// Returns the undone block if any, None if stack is empty.
    pub fn undo(&mut self) -> Option<ExecutedBlock> {
        if let Some(block) = self.blocks.pop() {
            // Remove bindings from flattened view
            for name in block.bindings_created.keys() {
                self.all_bindings.remove(name);
                self.all_types.remove(name);
            }
            Some(block)
        } else {
            None
        }
    }

    /// Undo multiple blocks
    ///
    /// Returns the number of blocks actually undone.
    pub fn undo_n(&mut self, n: usize) -> usize {
        let mut count = 0;
        for _ in 0..n {
            if self.undo().is_some() {
                count += 1;
            } else {
                break;
            }
        }
        count
    }

    /// Check if a binding exists
    pub fn has_binding(&self, name: &str) -> bool {
        self.all_bindings.contains_key(name)
    }

    /// Get binding PK by name
    pub fn get_binding(&self, name: &str) -> Option<Uuid> {
        self.all_bindings.get(name).copied()
    }

    /// Get entity type for a binding
    pub fn get_binding_type(&self, name: &str) -> Option<&str> {
        self.all_types.get(name).map(|s| s.as_str())
    }

    /// Get all binding names
    pub fn binding_names(&self) -> impl Iterator<Item = &str> {
        self.all_bindings.keys().map(|s| s.as_str())
    }

    /// Get binding count
    pub fn binding_count(&self) -> usize {
        self.all_bindings.len()
    }

    /// Get block count
    pub fn block_count(&self) -> usize {
        self.blocks.len()
    }

    /// Check if session is empty
    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }

    /// Build a BindingContext for planning/validation
    ///
    /// This context can be passed to the planner so it knows about
    /// previously executed bindings.
    pub fn binding_context(&self) -> BindingContext {
        let mut ctx = BindingContext::new();

        for (name, pk) in &self.all_bindings {
            let entity_type = self.all_types.get(name).cloned().unwrap_or_default();

            // Parse subtype from entity_type if present (e.g., "entity.proper_person")
            let (base_type, subtype) = if let Some((base, sub)) = entity_type.split_once('.') {
                (base.to_string(), Some(sub.to_string()))
            } else {
                (entity_type, None)
            };

            ctx.insert(BindingInfo {
                name: name.clone(),
                produced_type: base_type,
                subtype,
                entity_pk: *pk,
                resolved: true, // Already executed, so it's resolved
            });
        }

        ctx
    }

    /// Clear all session state
    pub fn reset(&mut self) {
        self.blocks.clear();
        self.all_bindings.clear();
        self.all_types.clear();
    }

    /// Get a summary of the session state
    pub fn summary(&self) -> String {
        format!(
            "ReplSession: {} blocks, {} bindings",
            self.blocks.len(),
            self.all_bindings.len()
        )
    }

    /// List all bindings with their types for display
    pub fn list_bindings(&self) -> Vec<String> {
        let mut bindings: Vec<_> = self
            .all_bindings
            .keys()
            .map(|name| {
                let ty = self.all_types.get(name).map(|s| s.as_str()).unwrap_or("?");
                format!("@{} ({})", name, ty)
            })
            .collect();
        bindings.sort();
        bindings
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_program() -> Program {
        Program { statements: vec![] }
    }

    #[test]
    fn test_new_session_is_empty() {
        let session = ReplSession::new();
        assert!(session.is_empty());
        assert_eq!(session.binding_count(), 0);
        assert_eq!(session.block_count(), 0);
    }

    #[test]
    fn test_append_executed() {
        let mut session = ReplSession::new();

        let mut bindings = HashMap::new();
        bindings.insert("fund".to_string(), Uuid::now_v7());

        let mut types = HashMap::new();
        types.insert("fund".to_string(), "cbu".to_string());

        session.append_executed(empty_program(), bindings, types);

        assert!(!session.is_empty());
        assert_eq!(session.block_count(), 1);
        assert!(session.has_binding("fund"));
        assert_eq!(session.get_binding_type("fund"), Some("cbu"));
    }

    #[test]
    fn test_multiple_blocks() {
        let mut session = ReplSession::new();

        // First block
        let mut bindings1 = HashMap::new();
        bindings1.insert("fund".to_string(), Uuid::now_v7());
        session.append_executed(empty_program(), bindings1, HashMap::new());

        // Second block
        let mut bindings2 = HashMap::new();
        bindings2.insert("person".to_string(), Uuid::now_v7());
        session.append_executed(empty_program(), bindings2, HashMap::new());

        assert_eq!(session.block_count(), 2);
        assert!(session.has_binding("fund"));
        assert!(session.has_binding("person"));
    }

    #[test]
    fn test_undo() {
        let mut session = ReplSession::new();

        // Execute two blocks
        let mut bindings1 = HashMap::new();
        bindings1.insert("a".to_string(), Uuid::now_v7());
        session.append_executed(empty_program(), bindings1, HashMap::new());

        let mut bindings2 = HashMap::new();
        bindings2.insert("b".to_string(), Uuid::now_v7());
        session.append_executed(empty_program(), bindings2, HashMap::new());

        assert!(session.has_binding("a"));
        assert!(session.has_binding("b"));

        // Undo second block
        let undone = session.undo();
        assert!(undone.is_some());
        assert!(session.has_binding("a"));
        assert!(!session.has_binding("b"));

        // Undo first block
        let undone = session.undo();
        assert!(undone.is_some());
        assert!(!session.has_binding("a"));

        // Undo empty stack
        let undone = session.undo();
        assert!(undone.is_none());
    }

    #[test]
    fn test_undo_n() {
        let mut session = ReplSession::new();

        // Add 3 blocks
        for i in 0..3 {
            let mut bindings = HashMap::new();
            bindings.insert(format!("b{}", i), Uuid::now_v7());
            session.append_executed(empty_program(), bindings, HashMap::new());
        }

        assert_eq!(session.block_count(), 3);

        // Undo 2
        let undone = session.undo_n(2);
        assert_eq!(undone, 2);
        assert_eq!(session.block_count(), 1);
        assert!(session.has_binding("b0"));
        assert!(!session.has_binding("b1"));
        assert!(!session.has_binding("b2"));
    }

    #[test]
    fn test_binding_context() {
        let mut session = ReplSession::new();

        let pk = Uuid::now_v7();
        let mut bindings = HashMap::new();
        bindings.insert("fund".to_string(), pk);

        let mut types = HashMap::new();
        types.insert("fund".to_string(), "cbu".to_string());

        session.append_executed(empty_program(), bindings, types);

        let ctx = session.binding_context();
        assert!(ctx.contains("fund"));

        let info = ctx.get("fund").unwrap();
        assert_eq!(info.entity_pk, pk);
        assert_eq!(info.produced_type, "cbu");
    }

    #[test]
    fn test_binding_context_with_subtype() {
        let mut session = ReplSession::new();

        let mut bindings = HashMap::new();
        bindings.insert("john".to_string(), Uuid::now_v7());

        let mut types = HashMap::new();
        types.insert("john".to_string(), "entity.proper_person".to_string());

        session.append_executed(empty_program(), bindings, types);

        let ctx = session.binding_context();
        let info = ctx.get("john").unwrap();
        assert_eq!(info.produced_type, "entity");
        assert_eq!(info.subtype, Some("proper_person".to_string()));
    }

    #[test]
    fn test_reset() {
        let mut session = ReplSession::new();

        let mut bindings = HashMap::new();
        bindings.insert("fund".to_string(), Uuid::now_v7());
        session.append_executed(empty_program(), bindings, HashMap::new());

        assert!(!session.is_empty());

        session.reset();

        assert!(session.is_empty());
        assert_eq!(session.binding_count(), 0);
    }

    #[test]
    fn test_list_bindings() {
        let mut session = ReplSession::new();

        let mut bindings = HashMap::new();
        bindings.insert("fund".to_string(), Uuid::now_v7());
        bindings.insert("person".to_string(), Uuid::now_v7());

        let mut types = HashMap::new();
        types.insert("fund".to_string(), "cbu".to_string());
        types.insert("person".to_string(), "entity.proper_person".to_string());

        session.append_executed(empty_program(), bindings, types);

        let list = session.list_bindings();
        assert_eq!(list.len(), 2);
        assert!(list.iter().any(|s| s.contains("fund")));
        assert!(list.iter().any(|s| s.contains("person")));
    }
}
