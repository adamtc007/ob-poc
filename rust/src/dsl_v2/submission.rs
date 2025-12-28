//! Unified DSL Submission Model
//!
//! All DSL execution uses ONE uniform model based on binding cardinality:
//! - Cardinality 0: Draft (unresolved, valid REPL state)
//! - Cardinality 1: Singleton execution
//! - Cardinality N: Batch expansion (N iterations, atomic transaction)
//!
//! No templates. No macros. No special batch mode. Just substitution.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use dsl_core::ast::{Argument, AstNode, Literal, Statement, VerbCall};

// ============================================================================
// Part 2: SymbolBinding
// ============================================================================

/// A binding from a symbol to zero, one, or many UUIDs
///
/// Cardinality determines execution behavior:
/// - 0 (empty): Draft state, unresolved
/// - 1: Singleton execution
/// - N: Batch expansion (N iterations)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SymbolBinding {
    /// The bound UUIDs
    pub ids: Vec<Uuid>,
    /// Optional display names (parallel to ids)
    #[serde(default)]
    pub names: Vec<String>,
    /// Entity type hint for resolution
    #[serde(default)]
    pub entity_type: Option<String>,
}

impl SymbolBinding {
    /// Create an unresolved binding (cardinality 0)
    pub fn unresolved() -> Self {
        Self::default()
    }

    /// Create a singleton binding (cardinality 1)
    pub fn singleton(id: Uuid) -> Self {
        Self {
            ids: vec![id],
            names: vec![],
            entity_type: None,
        }
    }

    /// Create a singleton binding with a display name
    pub fn singleton_named(id: Uuid, name: String) -> Self {
        Self {
            ids: vec![id],
            names: vec![name],
            entity_type: None,
        }
    }

    /// Create a multiple binding (cardinality N)
    pub fn multiple(ids: Vec<Uuid>) -> Self {
        Self {
            ids,
            names: vec![],
            entity_type: None,
        }
    }

    /// Create a multiple binding with display names
    pub fn multiple_named(items: Vec<(Uuid, String)>) -> Self {
        let (ids, names) = items.into_iter().unzip();
        Self {
            ids,
            names,
            entity_type: None,
        }
    }

    /// Add entity type hint
    pub fn with_type(mut self, t: String) -> Self {
        self.entity_type = Some(t);
        self
    }

    /// Number of bound UUIDs
    pub fn len(&self) -> usize {
        self.ids.len()
    }

    /// True if no UUIDs bound
    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    /// Cardinality 0 - draft state
    pub fn is_unresolved(&self) -> bool {
        self.ids.is_empty()
    }

    /// Cardinality 1 - singleton
    pub fn is_singleton(&self) -> bool {
        self.ids.len() == 1
    }

    /// Cardinality > 1 - batch
    pub fn is_multiple(&self) -> bool {
        self.ids.len() > 1
    }

    /// Get the single UUID (panics if not singleton)
    pub fn id(&self) -> Uuid {
        assert!(self.is_singleton(), "Expected singleton binding");
        self.ids[0]
    }

    /// Get the single UUID if singleton
    pub fn id_opt(&self) -> Option<Uuid> {
        if self.is_singleton() {
            Some(self.ids[0])
        } else {
            None
        }
    }

    /// Add a UUID to the binding
    pub fn add(&mut self, id: Uuid, name: Option<String>) {
        self.ids.push(id);
        if let Some(n) = name {
            self.names.push(n);
        }
    }

    /// Remove a UUID from the binding
    pub fn remove(&mut self, id: Uuid) -> bool {
        if let Some(idx) = self.ids.iter().position(|i| *i == id) {
            self.ids.remove(idx);
            if idx < self.names.len() {
                self.names.remove(idx);
            }
            true
        } else {
            false
        }
    }

    /// Get name for a given index
    pub fn name_at(&self, idx: usize) -> Option<&str> {
        self.names.get(idx).map(|s| s.as_str())
    }
}

// ============================================================================
// Part 3: DslSubmission
// ============================================================================

/// A DSL submission with statements and symbol bindings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DslSubmission {
    /// The DSL statements to execute
    pub statements: Vec<Statement>,
    /// Symbol bindings: symbol name → bound UUIDs
    pub bindings: HashMap<String, SymbolBinding>,
}

/// State of a submission
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum SubmissionState {
    /// Has unresolved symbols (cardinality 0)
    Draft { unresolved: Vec<String> },
    /// Ready to execute
    Ready,
    /// Ready but large - user should confirm
    ReadyWithWarning {
        message: String,
        iterations: usize,
        total_ops: usize,
    },
    /// Too large to execute
    TooLarge { message: String, suggestion: String },
}

/// Limits for submission validation
#[derive(Debug, Clone)]
pub struct SubmissionLimits {
    /// Warn if iterations exceed this
    pub warn_iterations: usize,
    /// Reject if iterations exceed this
    pub max_iterations: usize,
    /// Warn if total operations exceed this
    pub warn_total_ops: usize,
    /// Reject if total operations exceed this
    pub max_total_ops: usize,
    /// Chunk size for large batches
    pub chunk_size: usize,
}

impl Default for SubmissionLimits {
    fn default() -> Self {
        Self {
            warn_iterations: 100,
            max_iterations: 10_000,
            warn_total_ops: 500,
            max_total_ops: 50_000,
            chunk_size: 100,
        }
    }
}

/// Errors during submission processing
#[derive(Debug, Clone, thiserror::Error)]
pub enum SubmissionError {
    #[error("Unresolved symbols: {0:?}")]
    UnresolvedSymbols(Vec<String>),
    #[error("Multiple iteration symbols not supported: {0:?}")]
    MultipleIterationSymbols(Vec<String>),
    #[error("Execution error: {0}")]
    ExecutionError(String),
}

impl DslSubmission {
    /// Create a new submission from statements
    pub fn new(statements: Vec<Statement>) -> Self {
        Self {
            statements,
            bindings: HashMap::new(),
        }
    }

    /// Add a binding (builder pattern)
    pub fn bind(mut self, symbol: impl Into<String>, binding: SymbolBinding) -> Self {
        self.bindings.insert(symbol.into(), binding);
        self
    }

    /// Add a singleton binding (builder pattern)
    pub fn bind_one(mut self, symbol: impl Into<String>, id: Uuid) -> Self {
        self.bindings
            .insert(symbol.into(), SymbolBinding::singleton(id));
        self
    }

    /// Add a multiple binding (builder pattern)
    pub fn bind_many(mut self, symbol: impl Into<String>, ids: Vec<Uuid>) -> Self {
        self.bindings
            .insert(symbol.into(), SymbolBinding::multiple(ids));
        self
    }

    /// Set a binding
    pub fn set_binding(&mut self, symbol: &str, binding: SymbolBinding) {
        self.bindings.insert(symbol.to_string(), binding);
    }

    /// Add a UUID to an existing binding (or create one)
    pub fn add_to_binding(&mut self, symbol: &str, id: Uuid, name: Option<String>) {
        self.bindings
            .entry(symbol.to_string())
            .or_insert_with(SymbolBinding::unresolved)
            .add(id, name);
    }

    /// Remove a UUID from a binding
    pub fn remove_from_binding(&mut self, symbol: &str, id: Uuid) -> bool {
        self.bindings
            .get_mut(symbol)
            .map(|b| b.remove(id))
            .unwrap_or(false)
    }

    /// Get all symbols referenced in DSL statements
    pub fn symbols_in_dsl(&self) -> Vec<String> {
        let mut symbols = vec![];
        for stmt in &self.statements {
            collect_symbols(stmt, &mut symbols);
        }
        symbols.sort();
        symbols.dedup();
        symbols
    }

    /// Get symbols with cardinality 0 (unresolved)
    pub fn unresolved_symbols(&self) -> Vec<String> {
        self.symbols_in_dsl()
            .into_iter()
            .filter(|s| {
                self.bindings
                    .get(s)
                    .map(|b| b.is_unresolved())
                    .unwrap_or(true)
            })
            .collect()
    }

    /// True if any symbols are unresolved
    pub fn has_unresolved(&self) -> bool {
        !self.unresolved_symbols().is_empty()
    }

    /// True if all symbols are resolved
    pub fn is_resolved(&self) -> bool {
        self.unresolved_symbols().is_empty()
    }

    /// Find the symbol with cardinality > 1 (for iteration)
    /// Errors if multiple symbols have cardinality > 1
    pub fn iteration_symbol(&self) -> Result<Option<String>, SubmissionError> {
        let multi: Vec<_> = self
            .bindings
            .iter()
            .filter(|(_, b)| b.is_multiple())
            .map(|(k, _)| k.clone())
            .collect();
        match multi.len() {
            0 => Ok(None),
            1 => Ok(Some(multi.into_iter().next().unwrap())),
            _ => Err(SubmissionError::MultipleIterationSymbols(multi)),
        }
    }

    /// True if this is a batch submission (any symbol has cardinality > 1)
    pub fn is_batch(&self) -> bool {
        self.bindings.values().any(|b| b.is_multiple())
    }

    /// Number of iterations (max cardinality across all bindings)
    pub fn iteration_count(&self) -> usize {
        self.bindings
            .values()
            .map(|b| b.len())
            .max()
            .unwrap_or(1)
            .max(1)
    }

    /// Total operations (iterations × statements)
    pub fn total_operations(&self) -> usize {
        self.iteration_count() * self.statements.len()
    }

    /// Get current submission state
    pub fn state(&self, limits: &SubmissionLimits) -> SubmissionState {
        let unresolved = self.unresolved_symbols();
        if !unresolved.is_empty() {
            return SubmissionState::Draft { unresolved };
        }

        let iterations = self.iteration_count();
        let total_ops = self.total_operations();

        if iterations > limits.max_iterations {
            return SubmissionState::TooLarge {
                message: format!("{} items exceeds max {}", iterations, limits.max_iterations),
                suggestion: "Refine selection to reduce items".into(),
            };
        }
        if total_ops > limits.max_total_ops {
            return SubmissionState::TooLarge {
                message: format!("{} ops exceeds max {}", total_ops, limits.max_total_ops),
                suggestion: "Reduce items or operations per item".into(),
            };
        }
        if iterations > limits.warn_iterations || total_ops > limits.warn_total_ops {
            return SubmissionState::ReadyWithWarning {
                message: format!(
                    "{} items × {} ops = {} total",
                    iterations,
                    self.statements.len(),
                    total_ops
                ),
                iterations,
                total_ops,
            };
        }
        SubmissionState::Ready
    }

    /// True if submission can be executed
    pub fn can_execute(&self, limits: &SubmissionLimits) -> bool {
        matches!(
            self.state(limits),
            SubmissionState::Ready | SubmissionState::ReadyWithWarning { .. }
        )
    }
}

// ============================================================================
// Part 4: Expansion
// ============================================================================

/// Expanded submission ready for execution
#[derive(Debug)]
pub struct ExpandedSubmission {
    /// Individual iterations
    pub iterations: Vec<IterationStatements>,
    /// True if this was a batch (N > 1)
    pub is_batch: bool,
    /// Total statements across all iterations
    pub total_statements: usize,
}

/// Statements for a single iteration
#[derive(Debug, Clone)]
pub struct IterationStatements {
    /// Iteration index (0-based)
    pub index: usize,
    /// Key for this iteration (if batch)
    pub iteration_key: Option<IterationKey>,
    /// Statements with symbols substituted
    pub statements: Vec<Statement>,
}

/// Key identifying a batch iteration
#[derive(Debug, Clone)]
pub struct IterationKey {
    /// Symbol that was iterated
    pub symbol: String,
    /// UUID for this iteration
    pub id: Uuid,
    /// Display name (if available)
    pub name: Option<String>,
}

impl DslSubmission {
    /// Expand the submission into executable iterations
    pub fn expand(&self) -> Result<ExpandedSubmission, SubmissionError> {
        let unresolved = self.unresolved_symbols();
        if !unresolved.is_empty() {
            return Err(SubmissionError::UnresolvedSymbols(unresolved));
        }

        let iter_symbol = self.iteration_symbol()?;

        // Fixed bindings (singletons)
        let fixed: HashMap<String, Uuid> = self
            .bindings
            .iter()
            .filter(|(_, b)| b.is_singleton())
            .map(|(k, b)| (k.clone(), b.id()))
            .collect();

        let iterations = match iter_symbol {
            None => {
                // Singleton execution
                vec![IterationStatements {
                    index: 0,
                    iteration_key: None,
                    statements: substitute_all(&self.statements, &fixed),
                }]
            }
            Some(ref symbol) => {
                // Batch execution
                let binding = self.bindings.get(symbol).unwrap();
                binding
                    .ids
                    .iter()
                    .enumerate()
                    .map(|(idx, id)| {
                        let mut bindings = fixed.clone();
                        bindings.insert(symbol.clone(), *id);
                        IterationStatements {
                            index: idx,
                            iteration_key: Some(IterationKey {
                                symbol: symbol.clone(),
                                id: *id,
                                name: binding.name_at(idx).map(|s| s.to_string()),
                            }),
                            statements: substitute_all(&self.statements, &bindings),
                        }
                    })
                    .collect()
            }
        };

        let total = iterations.iter().map(|i| i.statements.len()).sum();
        Ok(ExpandedSubmission {
            is_batch: iter_symbol.is_some(),
            iterations,
            total_statements: total,
        })
    }
}

// ============================================================================
// Symbol Collection and Substitution
// ============================================================================

fn collect_symbols(stmt: &Statement, out: &mut Vec<String>) {
    if let Statement::VerbCall(vc) = stmt {
        for arg in &vc.arguments {
            collect_symbols_node(&arg.value, out);
        }
    }
}

fn collect_symbols_node(node: &AstNode, out: &mut Vec<String>) {
    match node {
        AstNode::SymbolRef { name, .. } => out.push(name.clone()),
        AstNode::List { items, .. } => items.iter().for_each(|n| collect_symbols_node(n, out)),
        AstNode::Map { entries, .. } => entries
            .iter()
            .for_each(|(_, v)| collect_symbols_node(v, out)),
        AstNode::Nested(vc) => {
            for arg in &vc.arguments {
                collect_symbols_node(&arg.value, out);
            }
        }
        _ => {}
    }
}

fn substitute_all(statements: &[Statement], bindings: &HashMap<String, Uuid>) -> Vec<Statement> {
    statements
        .iter()
        .map(|s| substitute_statement(s, bindings))
        .collect()
}

fn substitute_statement(stmt: &Statement, bindings: &HashMap<String, Uuid>) -> Statement {
    match stmt {
        Statement::VerbCall(vc) => Statement::VerbCall(VerbCall {
            domain: vc.domain.clone(),
            verb: vc.verb.clone(),
            arguments: vc
                .arguments
                .iter()
                .map(|arg| Argument {
                    key: arg.key.clone(),
                    value: substitute_node(&arg.value, bindings),
                    span: arg.span,
                })
                .collect(),
            binding: vc.binding.clone(),
            span: vc.span,
        }),
        other => other.clone(),
    }
}

fn substitute_node(node: &AstNode, bindings: &HashMap<String, Uuid>) -> AstNode {
    match node {
        AstNode::SymbolRef { name, span } => {
            if let Some(id) = bindings.get(name) {
                AstNode::Literal(Literal::Uuid(*id))
            } else {
                // Keep as symbol if not in bindings
                AstNode::SymbolRef {
                    name: name.clone(),
                    span: *span,
                }
            }
        }
        AstNode::List { items, span } => AstNode::List {
            items: items.iter().map(|n| substitute_node(n, bindings)).collect(),
            span: *span,
        },
        AstNode::Map { entries, span } => AstNode::Map {
            entries: entries
                .iter()
                .map(|(k, v)| (k.clone(), substitute_node(v, bindings)))
                .collect(),
            span: *span,
        },
        AstNode::Nested(vc) => AstNode::Nested(Box::new(VerbCall {
            domain: vc.domain.clone(),
            verb: vc.verb.clone(),
            arguments: vc
                .arguments
                .iter()
                .map(|arg| Argument {
                    key: arg.key.clone(),
                    value: substitute_node(&arg.value, bindings),
                    span: arg.span,
                })
                .collect(),
            binding: vc.binding.clone(),
            span: vc.span,
        })),
        _ => node.clone(),
    }
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use dsl_core::ast::Span;

    #[test]
    fn test_symbol_binding_unresolved() {
        let b = SymbolBinding::unresolved();
        assert!(b.is_unresolved());
        assert!(b.is_empty());
        assert_eq!(b.len(), 0);
    }

    #[test]
    fn test_symbol_binding_singleton() {
        let id = Uuid::new_v4();
        let b = SymbolBinding::singleton(id);
        assert!(b.is_singleton());
        assert!(!b.is_unresolved());
        assert!(!b.is_multiple());
        assert_eq!(b.id(), id);
    }

    #[test]
    fn test_symbol_binding_multiple() {
        let ids = vec![Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()];
        let b = SymbolBinding::multiple(ids.clone());
        assert!(b.is_multiple());
        assert!(!b.is_singleton());
        assert_eq!(b.len(), 3);
    }

    #[test]
    fn test_symbol_binding_add_remove() {
        let mut b = SymbolBinding::unresolved();
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        b.add(id1, Some("first".to_string()));
        assert!(b.is_singleton());

        b.add(id2, Some("second".to_string()));
        assert!(b.is_multiple());
        assert_eq!(b.len(), 2);

        assert!(b.remove(id1));
        assert!(b.is_singleton());

        assert!(!b.remove(id1)); // Already removed
    }

    #[test]
    fn test_submission_state_draft() {
        // Create a submission with a symbol reference
        let stmt = Statement::VerbCall(VerbCall {
            domain: "cbu".to_string(),
            verb: "add-product".to_string(),
            arguments: vec![Argument {
                key: "cbu-id".to_string(),
                value: AstNode::SymbolRef {
                    name: "target".to_string(),
                    span: Span::default(),
                },
                span: Span::default(),
            }],
            binding: None,
            span: Span::default(),
        });
        let submission = DslSubmission::new(vec![stmt]);

        let state = submission.state(&SubmissionLimits::default());
        match state {
            SubmissionState::Draft { unresolved } => {
                assert_eq!(unresolved, vec!["target".to_string()]);
            }
            _ => panic!("Expected Draft state"),
        }
    }

    #[test]
    fn test_submission_state_ready() {
        let stmt = Statement::VerbCall(VerbCall {
            domain: "cbu".to_string(),
            verb: "add-product".to_string(),
            arguments: vec![Argument {
                key: "cbu-id".to_string(),
                value: AstNode::SymbolRef {
                    name: "target".to_string(),
                    span: Span::default(),
                },
                span: Span::default(),
            }],
            binding: None,
            span: Span::default(),
        });
        let submission = DslSubmission::new(vec![stmt]).bind_one("target", Uuid::new_v4());

        let state = submission.state(&SubmissionLimits::default());
        assert!(matches!(state, SubmissionState::Ready));
    }

    #[test]
    fn test_submission_batch_expansion() {
        let stmt = Statement::VerbCall(VerbCall {
            domain: "cbu".to_string(),
            verb: "add-product".to_string(),
            arguments: vec![Argument {
                key: "cbu-id".to_string(),
                value: AstNode::SymbolRef {
                    name: "target".to_string(),
                    span: Span::default(),
                },
                span: Span::default(),
            }],
            binding: None,
            span: Span::default(),
        });

        let ids = vec![Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()];
        let submission = DslSubmission::new(vec![stmt]).bind_many("target", ids.clone());

        assert!(submission.is_batch());
        assert_eq!(submission.iteration_count(), 3);

        let expanded = submission.expand().unwrap();
        assert!(expanded.is_batch);
        assert_eq!(expanded.iterations.len(), 3);

        // Each iteration should have the UUID substituted
        for (idx, iter) in expanded.iterations.iter().enumerate() {
            assert_eq!(iter.index, idx);
            assert!(iter.iteration_key.is_some());
            let key = iter.iteration_key.as_ref().unwrap();
            assert_eq!(key.id, ids[idx]);
        }
    }

    #[test]
    fn test_submission_too_large() {
        let stmt = Statement::VerbCall(VerbCall {
            domain: "cbu".to_string(),
            verb: "test".to_string(),
            arguments: vec![],
            binding: None,
            span: Span::default(),
        });

        let ids: Vec<Uuid> = (0..15000).map(|_| Uuid::new_v4()).collect();
        let submission = DslSubmission::new(vec![stmt]).bind_many("target", ids);

        let state = submission.state(&SubmissionLimits::default());
        assert!(matches!(state, SubmissionState::TooLarge { .. }));
    }

    #[test]
    fn test_multiple_iteration_symbols_error() {
        let stmt = Statement::VerbCall(VerbCall {
            domain: "test".to_string(),
            verb: "test".to_string(),
            arguments: vec![
                Argument {
                    key: "a".to_string(),
                    value: AstNode::SymbolRef {
                        name: "sym1".to_string(),
                        span: Span::default(),
                    },
                    span: Span::default(),
                },
                Argument {
                    key: "b".to_string(),
                    value: AstNode::SymbolRef {
                        name: "sym2".to_string(),
                        span: Span::default(),
                    },
                    span: Span::default(),
                },
            ],
            binding: None,
            span: Span::default(),
        });

        let submission = DslSubmission::new(vec![stmt])
            .bind_many("sym1", vec![Uuid::new_v4(), Uuid::new_v4()])
            .bind_many("sym2", vec![Uuid::new_v4(), Uuid::new_v4()]);

        let result = submission.iteration_symbol();
        assert!(matches!(
            result,
            Err(SubmissionError::MultipleIterationSymbols(_))
        ));
    }
}
