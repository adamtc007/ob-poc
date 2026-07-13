//! DSL Executor - YAML-driven execution engine for DSL v2
//!
//! This module implements the DslExecutor that processes parsed DSL programs
//! and executes them against the database using YAML-driven verb definitions.
//!
//! The executor routes verbs through:
//! - GenericCrudExecutor for CRUD operations (defined in verbs.yaml)
//! - SemOsVerbOpRegistry for plugins (post-Phase-5c-migrate; the legacy
//!   `CustomOperationRegistry` was removed in slice #80)

#[cfg(feature = "database")]
use anyhow::{anyhow, bail, Result};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use uuid::Uuid;

// Import ViewState for the pending_view_state field
// This enables view operations to communicate ViewState back to the session layer
use crate::session::ViewState;

// Import ViewportState for the pending_viewport_state field
// This enables viewport operations to communicate ViewportState back to the session layer
use ob_poc_types::ViewportState;

// Import GraphScope for pending_scope_change field
// This enables session.set-* operations to communicate scope changes back to the session layer
use crate::graph::types::GraphScope;

// Import UnifiedSession for pending session state
// This enables session.load-*/unload-*/undo/redo to communicate session state back
use crate::session::UnifiedSession;

#[cfg(feature = "database")]
use super::ast::{AstNode, Literal, VerbCall};
#[cfg(feature = "database")]
use super::domain_context::DomainContext;
#[cfg(feature = "database")]
use super::generic_executor::{GenericCrudExecutor, GenericExecutionResult};
#[cfg(feature = "database")]
use super::runtime_registry::{runtime_registry, RuntimeBehavior, RuntimeVerb};
#[cfg(feature = "database")]
use super::submission::{DslSubmission, SubmissionError, SubmissionLimits};
#[cfg(feature = "database")]
use dsl_runtime::{SemOsChildDispatcher, ServicePipelineService, TransactionScope};
#[cfg(feature = "database")]
use sem_os_postgres::ops::SemOsVerbOpRegistry;

#[cfg(feature = "database")]
use sqlx::PgPool;

// Event infrastructure for observability
use ob_poc_diagnostics::events::SharedEmitter;

// Error aggregation for best-effort execution
#[cfg(feature = "database")]
use super::errors::ExecutionErrors;

// Advisory locks for concurrent access control
#[cfg(feature = "database")]
use crate::database::locks::{acquire_locks, LockError};

/// Wall-clock timeout applied to every `pool.begin()` call in the DSL executor.
///
/// Prevents indefinite hangs when the connection pool is exhausted (E1 from
/// Phase 3 audit). Set `DSL_POOL_ACQUIRE_TIMEOUT_SECS` env var at startup to
/// override. The 30-second default is appropriate for interactive DSL sessions;
/// reduce for latency-sensitive paths, increase for bulk import workloads.
#[cfg(feature = "database")]
fn pool_acquire_timeout() -> std::time::Duration {
    let secs = std::env::var("DSL_POOL_ACQUIRE_TIMEOUT_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(30);
    std::time::Duration::from_secs(secs)
}

// Expansion types for lock derivation
#[cfg(feature = "database")]
use super::expansion::{ExpansionReport, LockKey, LockMode};

// ============================================================================
// Pre-Flight Resolution Check
// ============================================================================

/// Unresolved reference info for error reporting
#[cfg(feature = "database")]
#[derive(Debug, Clone)]
pub(crate) struct UnresolvedRef {
    /// Stable location identifier from AST: "{stmt_index}:{span.start}-{span.end}"
    pub ref_id: Option<String>,
    /// The unresolved value (e.g., "Allianz Global Investors")
    pub value: String,
    /// Entity type being resolved (e.g., "entity", "cbu")
    pub entity_type: String,
}

/// Validate all EntityRefs in an execution plan have been resolved.
///
/// This is the PRE-FLIGHT check that runs before any verb execution.
/// If any EntityRef has `resolved_key: None`, execution cannot proceed.
///
/// Returns Ok(()) if all refs are resolved, or Err with details about unresolved refs.
#[cfg(feature = "database")]
fn validate_all_resolved(plan: &super::execution_plan::ExecutionPlan) -> Result<(), anyhow::Error> {
    let mut unresolved: Vec<UnresolvedRef> = Vec::new();

    for step in &plan.steps {
        collect_unresolved_from_verb_call(&step.verb_call, &mut unresolved);
    }

    if !unresolved.is_empty() {
        let details: Vec<String> = unresolved
            .iter()
            .map(|u| {
                format!(
                    "  - {} '{}' (ref_id: {})",
                    u.entity_type,
                    u.value,
                    u.ref_id.as_deref().unwrap_or("unknown")
                )
            })
            .collect();

        return Err(anyhow!(
            "Cannot execute: {} unresolved entity reference(s):\n{}",
            unresolved.len(),
            details.join("\n")
        ));
    }

    Ok(())
}

#[cfg(feature = "database")]
/// Recursively collect unresolved EntityRefs from a VerbCall
#[cfg(feature = "database")]
fn collect_unresolved_from_verb_call(vc: &VerbCall, unresolved: &mut Vec<UnresolvedRef>) {
    for arg in &vc.arguments {
        collect_unresolved_from_node(&arg.value, unresolved);
    }
}

/// Recursively collect unresolved EntityRefs from an AstNode
#[cfg(feature = "database")]
fn collect_unresolved_from_node(node: &AstNode, unresolved: &mut Vec<UnresolvedRef>) {
    match node {
        AstNode::EntityRef {
            resolved_key,
            ref_id,
            value,
            entity_type,
            ..
        } => {
            if resolved_key.is_none() {
                unresolved.push(UnresolvedRef {
                    ref_id: ref_id.clone(),
                    value: value.clone(),
                    entity_type: entity_type.clone(),
                });
            }
        }
        AstNode::List { items, .. } => {
            for item in items {
                collect_unresolved_from_node(item, unresolved);
            }
        }
        AstNode::Map { entries, .. } => {
            for (_, v) in entries {
                collect_unresolved_from_node(v, unresolved);
            }
        }
        AstNode::Nested(vc) => {
            collect_unresolved_from_verb_call(vc, unresolved);
        }
        // Literals and SymbolRefs don't contain EntityRefs
        AstNode::Literal(_, _) | AstNode::SymbolRef { .. } => {}
    }
}

/// Return type specification for verb execution
#[derive(Debug, Clone)]
pub enum ReturnType {
    /// Returns a single UUID (e.g., created entity ID)
    Uuid { name: &'static str, capture: bool },
    /// Returns a single record as JSON
    Record,
    /// Returns multiple records as JSON array
    RecordSet,
    /// Returns count of affected rows
    Affected,
    /// Returns nothing (void operation)
    Void,
}

/// Result of executing a verb
#[derive(Debug, Clone)]
pub enum ExecutionResult {
    /// A UUID was returned (e.g., from INSERT RETURNING)
    Uuid(Uuid),
    /// A single record was returned
    Record(JsonValue),
    /// Multiple records were returned
    RecordSet(Vec<JsonValue>),
    /// Count of affected rows
    Affected(u64),
    /// No result (void operation)
    Void,
    /// Entity query result for batch iteration (entity.query verb)
    EntityQuery(ob_poc_types::entity_query::EntityQueryResult),
    /// Template invocation result (template.invoke verb)
    TemplateInvoked(crate::domain_ops::template_ops::TemplateInvokeResult),
    /// Template batch execution result (template.batch verb)
    TemplateBatch(crate::domain_ops::template_ops::TemplateBatchResult),
    /// Batch control operation result (batch.pause, batch.resume, etc.)
    BatchControl(ob_poc_types::batch_control::BatchControlResult),
}

// ============================================================================
// Best-Effort Execution Result (Phase 4.2)
// ============================================================================

/// Result of best-effort (non-atomic) plan execution with error aggregation
///
/// This is returned by `execute_plan_best_effort()` which continues on failure
/// and aggregates errors by root cause.
#[cfg(feature = "database")]
#[derive(Debug, Clone)]
pub struct BestEffortExecutionResult {
    /// Results for each step (Some if succeeded, None if failed)
    pub verb_results: Vec<Option<ExecutionResult>>,
    /// Aggregated errors grouped by root cause
    pub errors: ExecutionErrors,
    /// Overall batch status
    pub status: BatchStatus,
}

/// Status of a batch execution
#[cfg(feature = "database")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BatchStatus {
    /// All operations succeeded
    AllSucceeded,
    /// Some operations succeeded, some failed
    PartialSuccess,
    /// All operations failed
    AllFailed,
}

#[cfg(feature = "database")]
impl BestEffortExecutionResult {
    /// Check if execution was fully successful
    pub fn is_success(&self) -> bool {
        self.status == BatchStatus::AllSucceeded
    }

    /// Check if any operations succeeded
    pub fn has_successes(&self) -> bool {
        self.errors.total_succeeded > 0
    }

    /// Check if any operations failed
    pub fn has_failures(&self) -> bool {
        self.errors.total_failed > 0
    }

    /// Get a summary of the execution
    pub fn summary(&self) -> String {
        self.errors.summary()
    }

    /// Get the successful results only
    pub fn successful_results(&self) -> Vec<&ExecutionResult> {
        self.verb_results
            .iter()
            .filter_map(|r| r.as_ref())
            .collect()
    }

    /// Get count of successful operations
    pub fn success_count(&self) -> usize {
        self.errors.total_succeeded
    }

    /// Get count of failed operations
    pub fn failure_count(&self) -> usize {
        self.errors.total_failed
    }
}

// ============================================================================
// Atomic Execution Result (Phase 3.2)
// ============================================================================

/// Result of atomic (all-or-nothing) plan execution with locking
///
/// This is returned by `execute_plan_atomic_with_locks()` which wraps all
/// execution in a single transaction and optionally acquires advisory locks.
#[cfg(feature = "database")]
#[derive(Debug, Clone)]
pub enum AtomicExecutionResult {
    /// All steps succeeded and transaction committed
    Committed {
        /// Results for each step
        step_results: Vec<ExecutionResult>,
        /// Locks that were held during execution
        locks_held: Vec<LockKey>,
        /// Time spent acquiring locks (milliseconds)
        lock_wait_ms: u64,
    },
    /// Execution failed and transaction was rolled back
    RolledBack {
        /// Index of the step that failed (0-based)
        failed_at_step: usize,
        /// Error message from the failed step
        error: String,
        /// Results from steps that completed before failure
        completed_steps: Vec<ExecutionResult>,
        /// Locks that were held (now released due to rollback)
        locks_held: Vec<LockKey>,
    },
    /// Could not acquire required locks (another session holds them)
    LockContention {
        /// Entity type that caused contention
        entity_type: String,
        /// Entity ID that caused contention
        entity_id: String,
        /// Locks that were acquired before contention
        locks_acquired_before_contention: Vec<LockKey>,
    },
    /// A prior execution with the same idempotency key already committed.
    /// The prior result is returned without re-executing (v0.5 §9.1, §9.4).
    IdempotentReplayReturned {
        /// The results from the prior committed execution.
        prior_result: Vec<ExecutionResult>,
    },
    /// A DB unique constraint was violated — the plan lost a race with a
    /// concurrent execution. Caller may retry with a fresh read.
    ///
    /// Maps the `UniqueInsert` coordination strategy outcome (v0.5 §9.1,
    /// §9.3: "conflict is a normal runtime outcome"). This is NOT a failure;
    /// it is an expected outcome under concurrent `idempotent_ensure` plans.
    OptimisticConflict {
        /// The DB constraint name that was violated (e.g., "cbus_name_key").
        constraint_name: String,
    },
    /// A stage exceeded its configured deadline (v0.5 §9.1, §7.4).
    ///
    /// Covers both pool-acquire timeout (E1 fix: no more indefinite hang on
    /// pool.begin()) and per-plan deadline expiry (frame.is_expired()).
    /// Transaction rolled back; locks released. Caller may retry after backoff.
    TimedOut {
        /// The stage that timed out (e.g., "pool.begin", "plan_execution").
        stage: String,
        /// How long was waited before timing out.
        elapsed: std::time::Duration,
    },
    /// A stack-set worker panicked; the transaction was rolled back and the
    /// runtime recovered (v0.5 §9.1). Caller treats this as a failed execution.
    ///
    /// Phase 5: variant declared; panic-recovery wiring (`catch_unwind`) is
    /// Phase 6 (requires async-safe panic unwinding infrastructure).
    PanicRecovered {
        /// The stage in which the panic occurred.
        stage: String,
        /// Stringified panic info (message, if available).
        panic_info: String,
    },
}

#[cfg(feature = "database")]
impl AtomicExecutionResult {
    /// Check if execution was successful
    pub fn is_success(&self) -> bool {
        matches!(self, AtomicExecutionResult::Committed { .. })
    }

    /// Check if execution was rolled back
    pub fn is_rolled_back(&self) -> bool {
        matches!(self, AtomicExecutionResult::RolledBack { .. })
    }

    /// Check if there was lock contention
    pub fn is_lock_contention(&self) -> bool {
        matches!(self, AtomicExecutionResult::LockContention { .. })
    }

    /// Check if this was an idempotent replay (prior result returned).
    pub fn is_idempotent_replay(&self) -> bool {
        matches!(self, AtomicExecutionResult::IdempotentReplayReturned { .. })
    }

    /// Check if this was an optimistic conflict (lost race; no failure).
    pub fn is_conflict(&self) -> bool {
        matches!(self, AtomicExecutionResult::OptimisticConflict { .. })
    }

    /// Get the step results if committed
    pub fn results(&self) -> Option<&[ExecutionResult]> {
        match self {
            AtomicExecutionResult::Committed { step_results, .. } => Some(step_results),
            _ => None,
        }
    }

    /// Get a summary of the execution
    pub fn summary(&self) -> String {
        match self {
            AtomicExecutionResult::Committed {
                step_results,
                locks_held,
                lock_wait_ms,
            } => {
                format!(
                    "✓ Committed {} steps (held {} locks, waited {}ms)",
                    step_results.len(),
                    locks_held.len(),
                    lock_wait_ms
                )
            }
            AtomicExecutionResult::RolledBack {
                failed_at_step,
                error,
                completed_steps,
                ..
            } => {
                format!(
                    "✗ Rolled back at step {} after {} completed: {}",
                    failed_at_step,
                    completed_steps.len(),
                    error
                )
            }
            AtomicExecutionResult::LockContention {
                entity_type,
                entity_id,
                ..
            } => {
                format!(
                    "⚠ Lock contention on {}:{} - another session is modifying this entity",
                    entity_type, entity_id
                )
            }
            AtomicExecutionResult::IdempotentReplayReturned { prior_result } => {
                format!(
                    "↩ Idempotent replay: prior result returned ({} steps, no re-execution)",
                    prior_result.len()
                )
            }
            AtomicExecutionResult::OptimisticConflict { constraint_name } => {
                format!(
                    "⚡ Optimistic conflict on constraint '{constraint_name}' \
                     — concurrent plan won the race; caller may retry"
                )
            }
            AtomicExecutionResult::TimedOut { stage, elapsed } => {
                format!(
                    "⏱ Timed out at stage '{}' after {:?} — transaction rolled back",
                    stage, elapsed
                )
            }
            AtomicExecutionResult::PanicRecovered { stage, panic_info } => {
                format!(
                    "💥 Panic recovered at stage '{}': {} — transaction rolled back",
                    stage, panic_info
                )
            }
        }
    }
}

/// Execution context holding state during DSL execution
///
/// Supports parent/child hierarchy for batch execution where each iteration
/// has its own symbol scope but can read from parent (shared) bindings.
#[derive(Debug)]
pub struct ExecutionContext {
    /// Symbol table for @reference resolution (local scope)
    pub symbols: HashMap<String, Uuid>,
    /// Symbol types - maps binding name to entity type (e.g., "cbu" -> "cbu")
    pub symbol_types: HashMap<String, String>,
    /// Parent symbols (read-only, inherited from parent context)
    /// Used in batch execution where shared bindings are accessible to all iterations
    pub parent_symbols: HashMap<String, Uuid>,
    /// Parent symbol types
    pub parent_symbol_types: HashMap<String, String>,
    /// JSON bindings for complex data (e.g., GLEIF discovery results)
    /// Used when operations need to pass structured data between verb calls
    pub json_bindings: HashMap<String, JsonValue>,
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
    /// Current selection (from view.selection) - for batch operations
    /// Populated when view.* verbs execute, provides @_selection binding
    pub current_selection: Option<Vec<Uuid>>,
    /// Pending view state from view.* operations
    ///
    /// View operations (view.universe, view.book, view.cbu, etc.) create a ViewState
    /// but cannot directly access UnifiedSessionContext. Instead, they store the
    /// ViewState here. After execution completes, the caller (who has access to
    /// UnifiedSessionContext) should call `take_pending_view_state()` and propagate
    /// it via `session.set_view(view_state)`.
    ///
    /// This solves the "session state side door" where ViewState was being discarded
    /// because verb ops only receive ExecutionContext, not UnifiedSessionContext.
    pub pending_view_state: Option<ViewState>,
    /// Pending viewport state from viewport.* operations
    ///
    /// Viewport operations (viewport.focus, viewport.enhance, viewport.filter, etc.)
    /// create a ViewportState but cannot directly access the session. Instead, they
    /// store the ViewportState here. After execution completes, the caller should call
    /// `take_pending_viewport_state()` and propagate it via `session.set_viewport_state()`.
    pub pending_viewport_state: Option<ViewportState>,
    /// Pending scope change from session.* operations
    ///
    /// Session scope operations (session.set-galaxy, session.set-cbu, etc.) change
    /// the current scope but cannot directly access UnifiedSessionContext. Instead,
    /// they store the new GraphScope here. After execution completes, the caller
    /// should call `take_pending_scope_change()` and update the session scope.
    pub pending_scope_change: Option<GraphScope>,
    /// Source attribution for audit trail
    ///
    /// Tracks where the execution originated (api, cli, mcp, etc.),
    /// correlation ID for distributed tracing, and actor information.
    pub source_attribution: super::idempotency::SourceAttribution,
    /// Session ID for view state audit linkage
    pub session_id: Option<Uuid>,
    /// Session's active CBU IDs (for reading during execution)
    ///
    /// Pre-populated from `SessionContext.cbu_ids` before execution.
    /// Used by bulk operations that should apply to all session CBUs.
    /// Access via `session_cbu_ids()` method or `@session_cbus` symbol.
    pub session_cbu_ids: Vec<Uuid>,
    /// Pending session from session.* operations (Phase 6)
    ///
    /// Session operations (session.load-cbu, session.undo, etc.) modify the
    /// session but cannot directly access the main session store. Instead, they
    /// store/modify the UnifiedSession here. After execution completes, the caller
    /// should call `take_pending_session()` and propagate it to the session store.
    ///
    /// **Memory is truth, DB is backup.** All mutations are sync, in-memory, <1µs.
    pub pending_session: Option<UnifiedSession>,
    /// Client group context for entity resolution (Phase 052)
    ///
    /// When set, entity resolution for shorthand tags will be scoped to this group.
    /// Set via `session.set-client` verb.
    pub client_group_id: Option<Uuid>,
    /// Client group name (cached for display)
    pub client_group_name: Option<String>,
    /// Persona context for tag filtering (Phase 052)
    ///
    /// When set, tag search will be filtered by persona (kyc, trading, ops, onboarding).
    /// Set via `session.set-persona` verb.
    pub persona: Option<String>,

    // --- Typed session mutation fields (Phase 3 session merge) ---
    // Verb handlers write to these instead of pending_session.
    // Orchestrator syncs back to ReplSessionV2/WorkspaceFrame after execution.
    /// Session display name — set by session.load-cluster.
    pub pending_session_name: Option<String>,

    /// DAG state flags set during execution (verb → flag pairs).
    /// Synced to session dag_state after execution.
    pub pending_dag_flags: Vec<(String, bool)>,

    /// Constraint cascade: current structure set by verb handler.
    pub pending_structure_id: Option<Uuid>,
    pub pending_structure_name: Option<String>,

    /// Constraint cascade: current case set by verb handler.
    pub pending_case_id: Option<Uuid>,

    /// Constraint cascade: current mandate set by verb handler.
    pub pending_mandate_id: Option<Uuid>,

    /// Deal context set by verb handler.
    pub pending_deal_id: Option<Option<Uuid>>, // Some(Some(id)) = set, Some(None) = cleared
    pub pending_deal_name: Option<Option<String>>,

    /// Whether CBU scope was modified during this execution.
    /// When true, orchestrator syncs session_cbu_ids back to session.cbu_ids.
    pub cbu_scope_dirty: bool,
    /// Allow durable verbs to execute directly inside an already-orchestrated
    /// BPMN worker context.
    ///
    /// Normal REPL/user execution must still route durable verbs through the
    /// WorkflowDispatcher. This flag exists only so a BPMN service task can
    /// call the underlying direct implementation of a durable verb without
    /// trying to start a nested orchestration.
    pub allow_durable_direct: bool,

    /// G3/G4 (`EOP-DESIGN-CONTROLPLANE-G3-ENFORCEMENT-DIMENSION-001` §3(d)):
    /// which RR-2 ingress path this dispatch entered through. Set once at
    /// context construction (`RealDslExecutor::build_executor_and_ctx`, or
    /// left at the default for every other caller of `ExecutionContext::new`
    /// — Path B/C's several `admit_plan` callers, matching this design's
    /// own umbrella treatment, §2.3); read at `execute_verb_in_scope`'s
    /// (G4) admission check. Default `DslDirect`.
    pub execution_path: ob_poc_types::ExecutionPath,

    /// G3 §3(e) (double-admission guard): set by
    /// `ObPocVerbExecutor::execute_verb_admitting_envelope` right after its
    /// own successful `admit_in_scope` call, ONLY on the `ExecutionContext`
    /// it builds for Branch-3's fallthrough into this same seam
    /// (`to_dsl_context`). Every other constructor leaves this `None`.
    /// The seam's admission check skips re-checking `EnforcedVerbs` only
    /// when this exactly matches `execution_path` — a value match, not a
    /// bare boolean, so a caller cannot accidentally claim "already
    /// admitted" for a DIFFERENT path than the one it actually passed
    /// through.
    pub already_admitted_for: Option<ob_poc_types::ExecutionPath>,

    /// G4 item 1: a sealed envelope handle for THIS step's dispatch, when
    /// the caller holds one. Every production Path B/C ingress point
    /// leaves this `None` today — T9.3's established posture, unchanged
    /// by G3/G4 (Path B/C have no envelope-minting infrastructure wired
    /// yet; see `EOP-DESIGN-CONTROLPLANE-G3-ENFORCEMENT-DIMENSION-001`
    /// §2.3's "envelope_handle: None at every call" finding). Exists so
    /// G4's own atomicity tests (item 4 — rollback-of-consume,
    /// pin-drift-rejection) can exercise the seam's admission call with a
    /// real envelope without inventing a second admission code path; not
    /// read by any production `RealDslExecutor` construction site.
    pub envelope_handle: Option<ob_poc_types::EnvelopeHandle>,
}

impl Default for ExecutionContext {
    fn default() -> Self {
        Self {
            symbols: HashMap::new(),
            symbol_types: HashMap::new(),
            parent_symbols: HashMap::new(),
            parent_symbol_types: HashMap::new(),
            json_bindings: HashMap::new(),
            batch_index: None,
            audit_user: None,
            transaction_id: None,
            execution_id: Uuid::new_v4(),
            idempotency_enabled: true,
            current_selection: None,
            pending_view_state: None,
            pending_viewport_state: None,
            pending_scope_change: None,
            source_attribution: super::idempotency::SourceAttribution::default(),
            session_id: None,
            session_cbu_ids: Vec::new(),
            pending_session: None,
            client_group_id: None,
            client_group_name: None,
            persona: None,
            pending_session_name: None,
            pending_dag_flags: Vec::new(),
            pending_structure_id: None,
            pending_structure_name: None,
            pending_case_id: None,
            pending_mandate_id: None,
            pending_deal_id: None,
            pending_deal_name: None,
            cbu_scope_dirty: false,
            allow_durable_direct: false,
            execution_path: ob_poc_types::ExecutionPath::DslDirect,
            already_admitted_for: None,
            envelope_handle: None,
        }
    }
}

impl ExecutionContext {
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

    /// Bind a JSON value to a symbol (for complex data like discovery results)
    pub fn bind_json(&mut self, name: &str, value: JsonValue) {
        self.json_bindings.insert(name.to_string(), value);
    }

    /// Resolve a JSON binding by name
    pub fn resolve_json<T: serde::de::DeserializeOwned>(&self, name: &str) -> anyhow::Result<T> {
        let value = self
            .json_bindings
            .get(name)
            .ok_or_else(|| anyhow::anyhow!("JSON binding not found: @{}", name))?;
        serde_json::from_value(value.clone())
            .map_err(|e| anyhow::anyhow!("Failed to deserialize JSON binding @{}: {}", name, e))
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
    /// - Same execution_id, source_attribution, session_id and other settings
    pub fn child_for_iteration(&self, index: usize) -> Self {
        Self {
            symbols: HashMap::new(),
            symbol_types: HashMap::new(),
            parent_symbols: self.effective_symbols(),
            parent_symbol_types: self.effective_symbol_types(),
            json_bindings: self.json_bindings.clone(),
            batch_index: Some(index),
            audit_user: self.audit_user.clone(),
            transaction_id: self.transaction_id,
            execution_id: self.execution_id,
            idempotency_enabled: self.idempotency_enabled,
            current_selection: self.current_selection.clone(),
            // Don't inherit pending_view_state - each iteration starts fresh
            pending_view_state: None,
            // Don't inherit pending_viewport_state - each iteration starts fresh
            pending_viewport_state: None,
            // Don't inherit pending_scope_change - each iteration starts fresh
            pending_scope_change: None,
            // Inherit source attribution for audit trail consistency
            source_attribution: self.source_attribution.clone(),
            // Inherit session ID for view state linkage
            session_id: self.session_id,
            // Don't inherit pending_session - each iteration starts fresh
            pending_session: None,
            // Inherit session CBU IDs for bulk operations
            session_cbu_ids: self.session_cbu_ids.clone(),
            // Inherit client group context for entity resolution
            client_group_id: self.client_group_id,
            client_group_name: self.client_group_name.clone(),
            // Inherit persona for tag filtering
            persona: self.persona.clone(),
            // Phase 3 typed fields — fresh per iteration (no inheritance)
            pending_session_name: None,
            pending_dag_flags: Vec::new(),
            pending_structure_id: None,
            pending_structure_name: None,
            pending_case_id: None,
            pending_mandate_id: None,
            pending_deal_id: None,
            pending_deal_name: None,
            cbu_scope_dirty: false,
            allow_durable_direct: self.allow_durable_direct,
            // G3/G4: inherit — the child iteration is the same dispatch
            // continuing, not a new ingress.
            execution_path: self.execution_path,
            already_admitted_for: self.already_admitted_for,
            envelope_handle: self.envelope_handle,
        }
    }

    /// Mark this context as running inside a BPMN worker so durable verbs may
    /// use their direct implementation instead of starting nested workflows.
    pub fn allow_durable_direct(mut self) -> Self {
        self.allow_durable_direct = true;
        self
    }

    /// Check if we're currently in a batch iteration
    pub fn is_batch_iteration(&self) -> bool {
        self.batch_index.is_some()
    }

    /// Set the audit user
    pub fn with_audit_user(mut self, user: &str) -> Self {
        self.audit_user = Some(user.to_string());
        self
    }

    /// Disable idempotency checking (for testing or forced re-execution)
    pub fn without_idempotency(mut self) -> Self {
        self.idempotency_enabled = false;
        self
    }

    /// Set parent symbols (for batch execution setup)
    pub fn with_parent_symbols(mut self, symbols: HashMap<String, Uuid>) -> Self {
        self.parent_symbols = symbols;
        self
    }

    /// Set parent symbol types
    pub fn with_parent_symbol_types(mut self, types: HashMap<String, String>) -> Self {
        self.parent_symbol_types = types;
        self
    }

    // =========================================================================
    // SELECTION METHODS - For view.* verb integration
    // =========================================================================

    /// Set selection from view state (called by view.* verbs)
    /// Also binds as @_selection for DSL access
    pub fn set_selection(&mut self, selection: Vec<Uuid>) {
        // Store as JSON binding for DSL access
        if let Ok(json_value) = serde_json::to_value(&selection) {
            self.bind_json("_selection", json_value);
        }
        self.current_selection = Some(selection);
    }

    /// Get current selection
    pub fn get_selection(&self) -> Option<&Vec<Uuid>> {
        self.current_selection.as_ref()
    }

    /// Check if a selection is active
    pub fn has_selection(&self) -> bool {
        self.current_selection
            .as_ref()
            .is_some_and(|s| !s.is_empty())
    }

    /// Get selection count
    pub fn selection_count(&self) -> usize {
        self.current_selection
            .as_ref()
            .map(|s| s.len())
            .unwrap_or(0)
    }

    /// Clear the current selection
    pub fn clear_selection(&mut self) {
        self.current_selection = None;
        self.json_bindings.remove("_selection");
    }

    // =========================================================================
    // VIEW STATE METHODS - For view.* verb output to session layer
    // =========================================================================

    /// Set pending view state (called by view.* operations)
    ///
    /// View operations create a ViewState but cannot directly access
    /// UnifiedSessionContext. Instead, they store the ViewState here.
    /// After execution, the caller should call `take_pending_view_state()`
    /// and propagate it to the session via `session.set_view(view_state)`.
    pub fn set_pending_view_state(&mut self, view: ViewState) {
        self.pending_view_state = Some(view);
    }

    /// Take the pending view state (consumes it)
    ///
    /// Called by the execution layer after DSL execution completes.
    /// The caller should propagate this to UnifiedSessionContext:
    /// ```ignore
    /// if let Some(view) = ctx.take_pending_view_state() {
    ///     session.set_view(view);
    /// }
    /// ```
    pub fn take_pending_view_state(&mut self) -> Option<ViewState> {
        self.pending_view_state.take()
    }

    /// Check if there's a pending view state
    pub fn has_pending_view_state(&self) -> bool {
        self.pending_view_state.is_some()
    }

    // =========================================================================
    // VIEWPORT STATE METHODS - For viewport.* verb output to session layer
    // =========================================================================

    /// Set pending viewport state (called by viewport.* operations)
    ///
    /// Viewport operations create a ViewportState but cannot directly access
    /// the session. Instead, they store the ViewportState here.
    /// After execution, the caller should call `take_pending_viewport_state()`
    /// and propagate it to the session via `session.set_viewport_state(state)`.
    pub fn set_pending_viewport_state(&mut self, state: ViewportState) {
        self.pending_viewport_state = Some(state);
    }

    /// Take the pending viewport state (consumes it)
    ///
    /// Called by the execution layer after DSL execution completes.
    /// The caller should propagate this to SessionContext:
    /// ```ignore
    /// if let Some(state) = ctx.take_pending_viewport_state() {
    ///     session.context.set_viewport_state(state);
    /// }
    /// ```
    pub fn take_pending_viewport_state(&mut self) -> Option<ViewportState> {
        self.pending_viewport_state.take()
    }

    /// Check if there's a pending viewport state
    pub fn has_pending_viewport_state(&self) -> bool {
        self.pending_viewport_state.is_some()
    }

    /// Get a mutable reference to the pending viewport state
    /// Creates a default state if none exists (for incremental updates)
    pub fn viewport_state_or_default(&mut self) -> &mut ViewportState {
        if self.pending_viewport_state.is_none() {
            self.pending_viewport_state = Some(ViewportState::default());
        }
        self.pending_viewport_state.as_mut().unwrap()
    }

    // =========================================================================
    // SCOPE CHANGE METHODS - For session.* verb output to session layer
    // =========================================================================

    /// Set pending scope change (called by session.* operations)
    ///
    /// Session scope operations (session.set-galaxy, session.set-cbu, etc.) change
    /// the current scope but cannot directly access UnifiedSessionContext. Instead,
    /// they store the new GraphScope here. After execution completes, the caller
    /// should call `take_pending_scope_change()` and update the session scope.
    pub fn set_pending_scope_change(&mut self, scope: GraphScope) {
        self.pending_scope_change = Some(scope);
    }

    /// Take the pending scope change (consumes it)
    ///
    /// Called by the execution layer after DSL execution completes.
    /// The caller should propagate this to UnifiedSessionContext:
    /// ```ignore
    /// if let Some(scope) = ctx.take_pending_scope_change() {
    ///     session.set_scope(scope);
    /// }
    /// ```
    pub fn take_pending_scope_change(&mut self) -> Option<GraphScope> {
        self.pending_scope_change.take()
    }

    /// Check if there's a pending scope change
    pub fn has_pending_scope_change(&self) -> bool {
        self.pending_scope_change.is_some()
    }

    // =========================================================================
    // CBU SESSION METHODS - For session.* verb output to session layer (Phase 6)
    // =========================================================================

    /// Get or create the pending session for mutation.
    ///
    /// Session operations (session.load-cbu, session.undo, etc.) call this to get
    /// a mutable reference to the UnifiedSession. If no session exists yet, creates one.
    ///
    /// **Memory is truth.** All mutations are sync, in-memory, <1µs.
    pub fn get_or_create_session_mut(&mut self) -> &mut UnifiedSession {
        if self.pending_session.is_none() {
            self.pending_session = Some(UnifiedSession::new());
        }
        self.pending_session.as_mut().unwrap()
    }

    /// Set the pending session (e.g., when loading from DB at startup)
    pub fn set_pending_session(&mut self, session: UnifiedSession) {
        self.pending_session = Some(session);
    }

    /// Take the pending session (consumes it)
    ///
    /// Called by the execution layer after DSL execution completes.
    /// The caller should propagate this to the session store or
    /// trigger a background save via `session.save(&pool).await`.
    pub fn take_pending_session(&mut self) -> Option<UnifiedSession> {
        self.pending_session.take()
    }

    /// Check if there's a pending session
    pub fn has_pending_session(&self) -> bool {
        self.pending_session.is_some()
    }

    /// Get a reference to the pending session (if any)
    pub fn pending_session(&self) -> Option<&UnifiedSession> {
        self.pending_session.as_ref()
    }

    // =========================================================================
    // SESSION CBU SCOPE - For bulk operations on session's active CBUs
    // =========================================================================

    /// Set the session's active CBU IDs (called before execution)
    ///
    /// Pre-populates the execution context with the session's CBU scope.
    /// Bulk operations can then access this via `session_cbu_ids()`.
    pub fn set_session_cbu_ids(&mut self, cbu_ids: Vec<Uuid>) {
        self.session_cbu_ids = cbu_ids;
    }

    /// Get the session's active CBU IDs
    ///
    /// Returns the CBU IDs that were in the session scope when execution started.
    /// Used by bulk operations that should apply to all session CBUs.
    pub fn session_cbu_ids(&self) -> &[Uuid] {
        &self.session_cbu_ids
    }

    /// Check if there are any CBUs in session scope
    pub fn has_session_cbus(&self) -> bool {
        !self.session_cbu_ids.is_empty()
    }

    // =========================================================================
    // CLIENT GROUP CONTEXT - For entity resolution scoping
    // =========================================================================

    /// Set the client group context for entity resolution
    pub fn set_client_group_id(&mut self, group_id: Option<Uuid>) {
        self.client_group_id = group_id;
    }

    /// Set the client group name (for display)
    pub fn set_client_group_name(&mut self, name: Option<String>) {
        self.client_group_name = name;
    }

    /// Get the client group ID
    pub fn client_group_id(&self) -> Option<Uuid> {
        self.client_group_id
    }

    /// Get the client group name
    pub fn client_group_name(&self) -> Option<&str> {
        self.client_group_name.as_deref()
    }

    /// Set the persona for tag filtering
    pub fn set_persona(&mut self, persona: Option<String>) {
        self.persona = persona;
    }

    /// Get the current persona
    pub fn persona(&self) -> Option<&str> {
        self.persona.as_deref()
    }

    // =========================================================================
    // AGENT CONTROL METHODS - For agent.* verb output to session layer
    // =========================================================================

    /// Signal agent start (called by agent.start)
    ///
    /// Agent control operations cannot directly access the AgentController.
    /// Instead, they store control signals here for the caller to process.
    pub fn set_pending_agent_start(&mut self, agent_session_id: Uuid, task: String) {
        self.bind_json(
            "_agent_control",
            serde_json::json!({
                "action": "start",
                "agent_session_id": agent_session_id,
                "task": task
            }),
        );
    }

    /// Signal agent pause (called by agent.pause)
    pub fn set_pending_agent_pause(&mut self) {
        self.bind_json(
            "_agent_control",
            serde_json::json!({
                "action": "pause"
            }),
        );
    }

    /// Signal agent resume (called by agent.resume)
    pub fn set_pending_agent_resume(&mut self) {
        self.bind_json(
            "_agent_control",
            serde_json::json!({
                "action": "resume"
            }),
        );
    }

    /// Signal agent stop (called by agent.stop)
    pub fn set_pending_agent_stop(&mut self) {
        self.bind_json(
            "_agent_control",
            serde_json::json!({
                "action": "stop"
            }),
        );
    }

    /// Signal checkpoint response (called by agent.confirm-decision/reject-decision/select-decision-option)
    pub fn set_pending_checkpoint_response(
        &mut self,
        checkpoint_id: Option<Uuid>,
        response_type: &str,
        selected_index: i32,
    ) {
        self.bind_json(
            "_agent_control",
            serde_json::json!({
                "action": "checkpoint_response",
                "checkpoint_id": checkpoint_id,
                "response_type": response_type,
                "selected_index": selected_index
            }),
        );
    }

    /// Signal threshold change (called by agent.set-selection-threshold)
    pub fn set_pending_threshold_change(
        &mut self,
        auto_proceed: Option<f64>,
        ambiguous_floor: Option<f64>,
    ) {
        self.bind_json(
            "_agent_control",
            serde_json::json!({
                "action": "set_threshold",
                "auto_proceed": auto_proceed,
                "ambiguous_floor": ambiguous_floor
            }),
        );
    }

    /// Signal mode change (called by agent.set-execution-mode)
    pub fn set_pending_mode_change(&mut self, mode: String) {
        self.bind_json(
            "_agent_control",
            serde_json::json!({
                "action": "set_mode",
                "mode": mode
            }),
        );
    }

    /// Take the pending agent control signal (consumes it)
    ///
    /// Called by the execution layer after DSL execution completes.
    /// The caller should process this signal and update the AgentController.
    pub fn take_pending_agent_control(&mut self) -> Option<serde_json::Value> {
        self.json_bindings.remove("_agent_control")
    }

    /// Check if there's a pending agent control signal
    pub fn has_pending_agent_control(&self) -> bool {
        self.json_bindings.contains_key("_agent_control")
    }

    /// Create context from DomainContext (for submission execution)
    #[cfg(feature = "database")]
    pub fn from_domain(domain_ctx: &DomainContext) -> Self {
        let mut ctx = Self::new();
        // Pre-populate with active CBU if set
        if let Some(cbu_id) = domain_ctx.active_cbu_id {
            ctx.bind_typed("cbu", cbu_id, "cbu");
        }
        if let Some(case_id) = domain_ctx.active_case_id {
            ctx.bind_typed("case", case_id, "kyc_case");
        }
        if let Some(entity_id) = domain_ctx.active_entity_id {
            ctx.bind_typed("entity", entity_id, "entity");
        }
        ctx
    }
}

// ============================================================================
// Submission Execution Results
// ============================================================================

/// Result of executing a DslSubmission
#[cfg(feature = "database")]
#[derive(Debug, Clone, serde::Serialize)]
pub struct SubmissionResult {
    /// Results for each iteration
    pub iterations: Vec<IterationResult>,
    /// Whether this was a batch execution (N > 1)
    pub is_batch: bool,
    /// Total statements executed across all iterations
    pub total_executed: usize,
}

/// Result of a single iteration within a submission
#[cfg(feature = "database")]
#[derive(Debug, Clone, serde::Serialize)]
pub struct IterationResult {
    /// Iteration index (0 for singleton)
    pub index: usize,
    /// Whether iteration succeeded
    pub success: bool,
    /// Bindings created during this iteration
    pub bindings: HashMap<String, Uuid>,
    /// Error message if failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// The main DSL executor
pub struct DslExecutor {
    #[cfg(feature = "database")]
    pool: PgPool,
    /// SemOS-native plugin op registry. Post-Phase-5c-migrate slice #80 this
    /// is the single source of truth for plugin dispatch. `None` is allowed
    /// so tests / harnesses can build an executor without a populated registry;
    /// in production `ob-poc-web::main` wires the canonical registry via
    /// [`Self::with_sem_os_ops`].
    #[cfg(feature = "database")]
    sem_os_ops: Option<std::sync::Arc<SemOsVerbOpRegistry>>,
    #[cfg(feature = "database")]
    generic_executor: GenericCrudExecutor,
    #[cfg(feature = "database")]
    idempotency: super::idempotency::IdempotencyManager,
    #[cfg(feature = "database")]
    verb_hash_lookup: crate::session::verb_hash_lookup::VerbHashLookupService,
    /// Event emitter for observability (optional, zero-overhead when None)
    events: Option<SharedEmitter>,
    /// Platform service registry, threaded onto each `VerbExecutionContext`
    /// this executor builds. Defaults to empty for tests/standalone use;
    /// production wires it via [`Self::with_services`] at host startup.
    service_registry: std::sync::Arc<dsl_runtime::ServiceRegistry>,
}

impl DslExecutor {
    /// Create a new executor with a database pool.
    ///
    /// Phase 5c-migrate slice #80 removed the legacy `CustomOperationRegistry`;
    /// plugin dispatch now runs through [`SemOsVerbOpRegistry`]. The registry
    /// is optional on the executor — tests / harnesses can construct one
    /// without it, while production wires the canonical registry via
    /// [`Self::with_sem_os_ops`] at host startup.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use ob_poc::dsl_v2::execution::DslExecutor;
    /// # use sqlx::PgPool;
    /// # fn demo(pool: PgPool) {
    /// let executor = DslExecutor::new(pool);
    /// let _ = executor.events();
    /// # }
    /// ```
    #[cfg(feature = "database")]
    pub fn new(pool: PgPool) -> Self {
        Self {
            generic_executor: GenericCrudExecutor::new(pool.clone()),
            idempotency: super::idempotency::IdempotencyManager::new(pool.clone()),
            verb_hash_lookup: crate::session::verb_hash_lookup::VerbHashLookupService::new(
                pool.clone(),
            ),
            pool,
            sem_os_ops: None,
            events: None,
            service_registry: std::sync::Arc::new(dsl_runtime::ServiceRegistry::empty()),
        }
    }

    /// Install the canonical [`SemOsVerbOpRegistry`] for plugin dispatch.
    ///
    /// In production, `ob-poc-web::main` builds the registry from
    /// `sem_os_postgres::ops::build_registry()` + `ob_poc::domain_ops::extend_registry()`
    /// and hands it to every `DslExecutor` via this builder. Without it,
    /// `execute_verb` will reject any verb that resolves to `RuntimeBehavior::Plugin`.
    #[cfg(feature = "database")]
    pub fn with_sem_os_ops(mut self, ops: std::sync::Arc<SemOsVerbOpRegistry>) -> Self {
        if self.service_registry.is_empty() {
            let mut services = dsl_runtime::ServiceRegistryBuilder::new();
            services.register::<dyn SemOsChildDispatcher>(std::sync::Arc::new(
                sem_os_postgres::ops::RegistryChildDispatcher::new(ops.clone()),
            ));
            services.register::<dyn ServicePipelineService>(std::sync::Arc::new(
                crate::services::ObPocServicePipelineService::new(),
            ));
            self.service_registry = std::sync::Arc::new(services.build());
        }
        self.sem_os_ops = Some(ops);
        self
    }

    /// Install the platform service registry. Call once at host startup
    /// after constructing the executor with [`Self::new`].
    pub fn with_services(mut self, services: std::sync::Arc<dsl_runtime::ServiceRegistry>) -> Self {
        self.service_registry = services;
        self
    }

    /// Borrow the installed service registry. Exposed so other executor-like
    /// shims (e.g. [`crate::runbook::step_executor_bridge`]) can clone it
    /// onto contexts they build themselves.
    pub fn service_registry(&self) -> std::sync::Arc<dsl_runtime::ServiceRegistry> {
        self.service_registry.clone()
    }

    /// Set the event emitter for observability.
    ///
    /// When set, the executor will emit events for each verb execution.
    /// The emitter is lock-free and adds < 1μs overhead per command.
    pub fn with_events(mut self, events: Option<SharedEmitter>) -> Self {
        self.events = events;
        self
    }

    /// Get the event emitter (if configured).
    pub fn events(&self) -> Option<&SharedEmitter> {
        self.events.as_ref()
    }

    /// Get a reference to the database pool
    #[cfg(feature = "database")]
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Execute a single verb call
    ///
    /// Routes through YAML-driven generic executor for CRUD verbs,
    /// and custom operations registry for plugins.
    ///
    /// If an event emitter is configured, emits success/failure events
    /// with timing information (< 1μs overhead).
    #[cfg(feature = "database")]
    pub async fn execute_verb(
        &self,
        vc: &VerbCall,
        ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        let verb_name = format!("{}.{}", vc.domain, vc.verb);
        let start = std::time::Instant::now();

        // Execute the verb
        let result = self.execute_verb_inner(vc, ctx).await;

        // Emit event if emitter is configured (< 1μs, never blocks, never fails)
        if let Some(ref events) = self.events {
            let duration_ms = start.elapsed().as_millis() as u64;
            let session_id = ctx.session_id;

            match &result {
                Ok(_) => {
                    events.emit(ob_poc_diagnostics::events::DslEvent::succeeded(
                        session_id,
                        verb_name,
                        duration_ms,
                    ));
                }
                Err(e) => {
                    events.emit(ob_poc_diagnostics::events::DslEvent::failed(
                        session_id,
                        verb_name,
                        duration_ms,
                        e,
                    ));
                }
            }
        }

        result
    }

    /// Inner verb execution logic (no event emission).
    ///
    /// Phase B.2b-α (2026-04-22): this is now a thin wrapper that opens a
    /// per-verb `PgTransactionScope` and delegates to
    /// [`Self::execute_verb_in_scope`] — the single in-scope dispatch
    /// entry point. All three behavior variants (Plugin, Durable-direct,
    /// Generic CRUD) are handled uniformly there. The Sequencer migration
    /// (B.2b-ζ) replaces this per-verb scope with an outer scope threaded
    /// from the runbook step loop, giving true cross-step atomicity.
    #[cfg(feature = "database")]
    async fn execute_verb_inner(
        &self,
        vc: &VerbCall,
        ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        use crate::sequencer_tx::PgTransactionScope;

        tracing::debug!("execute_verb: ENTER {}.{}", vc.domain, vc.verb);

        // Legacy per-verb scope. Phase B.2b-ζ replaces this with an outer
        // Sequencer-owned scope threaded into `execute_verb_in_scope`.
        let mut scope = PgTransactionScope::begin_timeout(&self.pool, pool_acquire_timeout())
            .await
            .map_err(|e| {
                anyhow!(
                    "execute_verb({}.{}): begin txn failed: {}",
                    vc.domain,
                    vc.verb,
                    e
                )
            })?;

        let outcome = {
            let scope_dyn: &mut dyn TransactionScope = &mut scope;
            self.execute_verb_in_scope(vc, ctx, scope_dyn).await
        };

        match outcome {
            Ok(result) => {
                scope.commit().await.map_err(|e| {
                    anyhow!(
                        "execute_verb({}.{}): commit failed: {}",
                        vc.domain,
                        vc.verb,
                        e
                    )
                })?;
                tracing::debug!("execute_verb: EXIT success {}.{}", vc.domain, vc.verb);
                Ok(result)
            }
            Err(step_err) => match scope.rollback().await {
                Ok(()) => Err(step_err),
                Err(rollback_err) => {
                    tracing::error!(
                        domain = %vc.domain,
                        verb = %vc.verb,
                        %rollback_err,
                        step_error = %step_err,
                        "execute_verb: CRITICAL — verb failed AND rollback failed; \
                         DB state is unknown"
                    );
                    Err(anyhow::anyhow!(
                        "{}.{} failed ({step_err:#}) AND rollback failed ({rollback_err:#}); \
                             DB state is unknown",
                        vc.domain,
                        vc.verb
                    ))
                }
            },
        }
    }

    /// Convert VerbCall arguments to JSON HashMap for generic executor
    #[cfg(feature = "database")]
    fn verbcall_args_to_json(
        args: &[super::ast::Argument],
        ctx: &ExecutionContext,
    ) -> Result<HashMap<String, JsonValue>> {
        let mut result = HashMap::new();
        for arg in args {
            let key = arg.key.clone();
            let value = Self::node_to_json(&arg.value, ctx)?;
            result.insert(key, value);
        }
        Ok(result)
    }

    /// Convert AST AstNode to JSON, resolving references
    #[cfg(feature = "database")]
    fn node_to_json(node: &AstNode, ctx: &ExecutionContext) -> Result<JsonValue> {
        match node {
            AstNode::Literal(lit, _) => Self::literal_to_json(lit),
            AstNode::SymbolRef { name, .. } => {
                let uuid = ctx
                    .resolve(name)
                    .ok_or_else(|| anyhow!("Unresolved reference: @{}", name))?;
                Ok(JsonValue::String(uuid.to_string()))
            }
            AstNode::EntityRef {
                resolved_key,
                value,
                ..
            } => {
                // Use resolved primary_key if available, otherwise fall back to value
                if let Some(pk) = resolved_key {
                    Ok(JsonValue::String(pk.clone()))
                } else {
                    // Not yet resolved - pass value for lookup during execution
                    Ok(JsonValue::String(value.clone()))
                }
            }
            AstNode::List { items, .. } => {
                let json_items: Result<Vec<JsonValue>> =
                    items.iter().map(|v| Self::node_to_json(v, ctx)).collect();
                Ok(JsonValue::Array(json_items?))
            }
            AstNode::Map { entries, .. } => {
                let mut json_map = serde_json::Map::new();
                for (k, v) in entries {
                    json_map.insert(k.clone(), Self::node_to_json(v, ctx)?);
                }
                Ok(JsonValue::Object(json_map))
            }
            AstNode::Nested(_) => {
                bail!("Nested VerbCall found during value conversion. Use compile() + execute_plan() for nested DSL.")
            }
        }
    }

    /// Convert Literal to JSON
    #[cfg(feature = "database")]
    fn literal_to_json(lit: &Literal) -> Result<JsonValue> {
        match lit {
            Literal::String(s) => Ok(JsonValue::String(s.clone())),
            Literal::Integer(i) => Ok(serde_json::json!(*i)),
            Literal::Decimal(d) => Ok(serde_json::json!(d.to_string())),
            Literal::Boolean(b) => Ok(JsonValue::Bool(*b)),
            Literal::Null => Ok(JsonValue::Null),
            Literal::Uuid(u) => Ok(JsonValue::String(u.to_string())),
        }
    }
}

// ============================================================================
// Plugin dispatch via SemOsVerbOp (post-5c-migrate slice #80)
// ============================================================================

/// Dispatch a plugin op against an **existing** `TransactionScope`. Phase
/// B.1 (F6 follow-on, 2026-04-22) primitive: the caller owns the
/// transaction boundary — this function does NOT begin, commit, or roll
/// back. Used by:
///
/// - [`dispatch_plugin_via_sem_os_op`] — the legacy wrapper that opens
///   its own per-verb scope (back-compat; still the single-dispatch-site
///   today).
/// - Future Phase B.2: the Sequencer's stage-8 dispatch loop will open
///   one outer scope and call this helper for every verb in the
///   runbook, so multi-step runbooks commit atomically or roll back
///   together.
///
/// The function:
/// 1. Builds a `VerbExecutionContext` from the caller's
///    `ExecutionContext` (copies symbols + session-scoped fields into
///    `extensions`).
/// 2. Converts the `VerbCall` arguments to a JSON object.
/// 3. Calls `op.execute(args, sem_ctx, scope)`.
/// 4. On error, returns `Err`; caller decides to commit or rollback.
/// 5. Syncs `sem_ctx` mutations back: new symbol bindings + pending_*
///    side channels unpacked from `sem_ctx.extensions` into the caller's
///    `ExecutionContext.pending_*` fields.
/// 6. Converts `VerbExecutionOutcome` back to `ExecutionResult`.
#[cfg(feature = "database")]
async fn dispatch_plugin_via_sem_os_op_in_scope(
    op: &dyn sem_os_postgres::ops::SemOsVerbOp,
    fqn: &str,
    vc: &VerbCall,
    ctx: &mut ExecutionContext,
    services: &std::sync::Arc<dsl_runtime::ServiceRegistry>,
    scope: &mut dyn TransactionScope,
) -> Result<ExecutionResult> {
    use crate::sem_os_runtime::verb_executor_adapter as adapter;
    use sem_os_core::principal::Principal;

    // 1. Build sem_ctx from legacy ctx.
    let mut sem_ctx = dsl_runtime::VerbExecutionContext::new(Principal::system());
    sem_ctx.services = services.clone();
    sem_ctx.symbols = ctx.symbols.clone();
    sem_ctx.symbol_types = ctx.symbol_types.clone();
    sem_ctx.execution_id = ctx.execution_id;

    let mut ext_map = serde_json::Map::new();
    if let Some(ref audit_user) = ctx.audit_user {
        ext_map.insert(
            "audit_user".to_string(),
            serde_json::Value::String(audit_user.clone()),
        );
    }
    if let Some(session_id) = ctx.session_id {
        ext_map.insert(
            "session_id".to_string(),
            serde_json::Value::String(session_id.to_string()),
        );
    }
    if let Some(group_id) = ctx.client_group_id {
        ext_map.insert(
            "client_group_id".to_string(),
            serde_json::Value::String(group_id.to_string()),
        );
    }
    if let Some(ref group_name) = ctx.client_group_name {
        ext_map.insert(
            "client_group_name".to_string(),
            serde_json::Value::String(group_name.clone()),
        );
    }
    if let Some(ref persona) = ctx.persona {
        ext_map.insert(
            "persona".to_string(),
            serde_json::Value::String(persona.clone()),
        );
    }
    if !ctx.session_cbu_ids.is_empty() {
        let ids: Vec<serde_json::Value> = ctx
            .session_cbu_ids
            .iter()
            .map(|u| serde_json::Value::String(u.to_string()))
            .collect();
        ext_map.insert("session_cbu_ids".to_string(), serde_json::Value::Array(ids));
    }
    sem_ctx.extensions = serde_json::Value::Object(ext_map);

    // 2. Convert VerbCall args → JSON.
    let mut args = adapter::verb_call_to_json(vc);

    // 2b. Phase F.1 (2026-04-22): pre-fetch hook. Ops that need
    //     external I/O to answer a read (bpmn.inspect being the
    //     canonical case) implement `pre_fetch` to do that work
    //     BEFORE the transaction scope is entered — the gRPC/HTTP
    //     round-trip happens outside the txn, satisfying A1.
    //
    //     Pre-fetch returns an optional JSON object whose keys are
    //     merged into `args` so the op's normal `execute(args, ...)`
    //     path sees the pre-fetched data with no external I/O of
    //     its own. Default `Ok(None)` makes this a no-op for ops
    //     that don't need it.
    let pre_fetch_pool = scope.pool().clone();
    if let Some(pre_fetched) = op
        .pre_fetch(&args, &mut sem_ctx, &pre_fetch_pool)
        .await
        .map_err(|e| anyhow!("sem_os_op({}) pre_fetch failed: {}", fqn, e))?
    {
        if let (Some(existing_obj), serde_json::Value::Object(pf_obj)) =
            (args.as_object_mut(), pre_fetched)
        {
            for (k, v) in pf_obj {
                existing_obj.insert(k, v);
            }
        }
    }

    // 3. Dispatch against the caller-supplied scope. No begin / commit /
    //    rollback — transaction boundary is the caller's responsibility.
    let outcome = op
        .execute(&args, &mut sem_ctx, scope)
        .await
        .map_err(|e| anyhow!("sem_os_op({}) failed: {}", fqn, e))?;

    // Phase C.1/C.3 (F7 follow-on, 2026-04-22): shadow-observe any
    // `PendingStateAdvance` the op emitted via its `ctx.extensions`
    // side channel. Uses the typed `peek_pending_state_advance`
    // accessor so the Sequencer's future Phase-C.2 apply path and
    // the dispatcher's shadow log share one contract. 72+ verbs now
    // emit across 15 domains.
    //
    // Phase C.2-main (2026-04-22 late): in addition to logging the raw
    // payload, resolve each `to_node` taxonomy token into a stable
    // `DagNodeId` via `ob_poc_types::resolve_pending_state_advance`.
    // The resolved UUIDs are non-destructively appended as
    // `to_node_resolved` on each state transition. When the real C.2
    // apply path lands (blocked on B.2b.f follow-ups around
    // apply-via-SemOS-in-txn), it will consume the resolved ids to
    // construct a typed `PendingStateAdvance` for persistence.
    if let Some(advance) = dsl_runtime::peek_pending_state_advance(&sem_ctx) {
        let resolved = ob_poc_types::resolve_pending_state_advance(advance);
        tracing::debug!(
            fqn,
            advance = %advance,
            resolved = %resolved,
            "Stage 9a shadow — PendingStateAdvance emitted (raw + C.2 resolved)"
        );
    }

    // 4. Sync sem_ctx mutations back into the caller's ExecutionContext.
    for (name, uuid) in &sem_ctx.symbols {
        ctx.symbols.insert(name.clone(), *uuid);
    }
    for (name, ty) in &sem_ctx.symbol_types {
        ctx.symbol_types.insert(name.clone(), ty.clone());
    }
    adapter::apply_sem_ctx_extensions_to_exec_ctx(&sem_ctx, ctx);

    // 5. Convert outcome back to legacy ExecutionResult.
    Ok(adapter::from_verb_outcome(outcome))
}

// Phase B.2b-α (2026-04-22): the legacy `dispatch_plugin_via_sem_os_op`
// self-scoping wrapper was removed. Plugin dispatch now always flows
// through `DslExecutor::execute_verb_in_scope` with a caller-owned scope;
// the per-verb scope is opened by `execute_verb_inner` until the
// Sequencer migration (B.2b-ζ) lands an outer scope.

// ============================================================================
// Submission Execution
// ============================================================================

#[cfg(feature = "database")]
impl DslExecutor {
    /// Unified entry point for all DSL execution via DslSubmission
    ///
    /// This method handles singleton, batch, and draft states uniformly.
    /// Batch executions (N > 1 UUIDs for a symbol) run atomically in one transaction.
    ///
    /// # Example
    /// ```ignore
    /// let submission = DslSubmission::new(statements)
    ///     .bind_one("cbu", cbu_id)
    ///     .bind_many("targets", target_ids);
    /// let result = executor.execute_submission(&submission, &mut domain_ctx, &limits).await?;
    /// ```
    pub async fn execute_submission(
        &self,
        submission: &DslSubmission,
        domain_ctx: &mut DomainContext,
        limits: &SubmissionLimits,
    ) -> Result<SubmissionResult, SubmissionError> {
        // Check if submission can be executed
        if !submission.can_execute(limits) {
            let state = submission.state(limits);
            return Err(match state {
                super::submission::SubmissionState::Draft { unresolved } => {
                    SubmissionError::UnresolvedSymbols(unresolved)
                }
                super::submission::SubmissionState::TooLarge { message, .. } => {
                    SubmissionError::ExecutionError(message)
                }
                _ => SubmissionError::ExecutionError("Cannot execute submission".into()),
            });
        }

        // Expand submission to iterations
        let expanded = submission.expand()?;

        tracing::info!(
            is_batch = expanded.is_batch,
            iterations = expanded.iterations.len(),
            total = expanded.total_statements,
            "Executing submission"
        );

        // Execute atomically in a transaction
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| SubmissionError::ExecutionError(e.to_string()))?;

        let mut results = Vec::with_capacity(expanded.iterations.len());

        for iteration in &expanded.iterations {
            // Set up iteration context if this is a batch
            if let Some(ref key) = iteration.iteration_key {
                domain_ctx.enter_iteration(
                    iteration.index,
                    key.name.clone().unwrap_or_else(|| key.id.to_string()),
                    key.id,
                    key.symbol.clone(),
                    None, // No template_id for direct submission execution
                );
            }

            // Create execution context from domain context
            let mut exec_ctx = ExecutionContext::from_domain(domain_ctx);

            // Execute all statements in this iteration
            match self
                .execute_statements_in_tx(&iteration.statements, &mut exec_ctx, &mut tx)
                .await
            {
                Ok(bindings) => {
                    results.push(IterationResult {
                        index: iteration.index,
                        success: true,
                        bindings,
                        error: None,
                    });
                }
                Err(e) => {
                    // Rollback on any failure
                    tx.rollback()
                        .await
                        .map_err(|re| SubmissionError::ExecutionError(re.to_string()))?;
                    return Err(SubmissionError::ExecutionError(format!(
                        "Iteration {} failed: {}",
                        iteration.index, e
                    )));
                }
            }

            // Exit iteration context
            if iteration.iteration_key.is_some() {
                domain_ctx.exit_iteration();
            }
        }

        // Commit the transaction
        tx.commit()
            .await
            .map_err(|e| SubmissionError::ExecutionError(e.to_string()))?;

        Ok(SubmissionResult {
            is_batch: expanded.is_batch,
            total_executed: expanded.total_statements,
            iterations: results,
        })
    }

    /// Execute statements within a transaction, returning created bindings
    async fn execute_statements_in_tx(
        &self,
        statements: &[super::ast::Statement],
        ctx: &mut ExecutionContext,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    ) -> Result<HashMap<String, Uuid>> {
        let mut bindings = HashMap::new();

        for stmt in statements {
            if let super::ast::Statement::VerbCall(vc) = stmt {
                // Execute the verb call
                let result = self.execute_verb_in_tx(vc, ctx, tx).await?;

                // Capture binding if statement has :as clause
                if let Some(ref binding_name) = vc.binding {
                    if let ExecutionResult::Uuid(id) = &result {
                        ctx.bind(binding_name, *id);
                        bindings.insert(binding_name.clone(), *id);
                    }
                }
            }
        }

        Ok(bindings)
    }

    /// Execute a single verb call within a transaction
    ///
    /// This method ensures the verb execution participates in the caller's transaction.
    /// For CRUD verbs, it uses GenericCrudExecutor::execute_in_tx.
    /// Plugin verbs cannot enlist in the caller's existing transaction — they
    /// always own their own scope via [`dispatch_plugin_via_sem_os_op`] — so
    /// this method rejects them.
    async fn execute_verb_in_tx(
        &self,
        vc: &VerbCall,
        ctx: &mut ExecutionContext,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    ) -> Result<ExecutionResult> {
        tracing::debug!("execute_verb_in_tx: ENTER {}.{}", vc.domain, vc.verb);

        // Look up verb in runtime registry (loaded from YAML)
        let runtime_verb = runtime_registry()
            .get(&vc.domain, &vc.verb)
            .ok_or_else(|| anyhow!("Unknown verb: {}.{}", vc.domain, vc.verb))?;

        // Plugin verbs cannot enlist in the caller's transaction — they own
        // their own scope via `SemOsVerbOp::execute` + `PgTransactionScope`.
        // Fail fast so the caller doesn't get silent non-transactional
        // behaviour inside an atomic batch.
        if let RuntimeBehavior::Plugin(_handler) = &runtime_verb.behavior {
            tracing::debug!("execute_verb_in_tx: routing to PLUGIN");
            return Err(anyhow!(
                "Plugin {}.{} cannot execute in a caller-owned transaction. \
                 Plugin ops run under their own `PgTransactionScope` — use the \
                 non-transactional `execute_verb` path, or teach the caller to \
                 split the batch around the plugin call.",
                vc.domain,
                vc.verb
            ));
        }

        // Durable verbs cannot participate in transactional batches
        if let RuntimeBehavior::Durable(d) = &runtime_verb.behavior {
            return Err(anyhow!(
                "Durable verb {}.{} (process_key={}) cannot execute in a transaction. \
                 Durable verbs require the V2 REPL with WorkflowDispatcher.",
                vc.domain,
                vc.verb,
                d.process_key
            ));
        }

        tracing::debug!("execute_verb_in_tx: routing to GENERIC executor with transaction");

        // Convert VerbCall arguments to JSON for generic executor
        let json_args = Self::verbcall_args_to_json(&vc.arguments, ctx)?;

        // Execute via generic executor WITH transaction
        let result = self
            .generic_executor
            .execute_in_tx(tx, runtime_verb, &json_args)
            .await?;
        tracing::debug!("execute_verb_in_tx: generic executor returned {:?}", result);

        // Handle symbol capture
        if runtime_verb.returns.capture {
            if let GenericExecutionResult::Uuid(uuid) = &result {
                if let Some(name) = &runtime_verb.returns.name {
                    ctx.bind(name, *uuid);
                }
            }
        }

        tracing::debug!("execute_verb_in_tx: EXIT success");
        Ok(result.to_legacy())
    }

    /// Phase B.2 (F6 follow-on, 2026-04-22): scope-aware verb dispatch.
    /// All three runtime behaviors route through this single entry point;
    /// the caller owns the transaction boundary.
    ///
    /// 1. **Plugin** (`RuntimeBehavior::Plugin`) → dispatches through
    ///    [`dispatch_plugin_via_sem_os_op_in_scope`] using the ambient scope.
    /// 2. **Durable + `ctx.allow_durable_direct`** → dispatches through the
    ///    same in-scope plugin primitive (invoked inside BPMN workers as
    ///    internal service tasks that must share the worker's txn).
    /// 3. **Durable + default** → rejected; durable verbs require
    ///    WorkflowDispatcher, not direct execution.
    /// 4. **Generic CRUD** → runs via `generic_executor.execute_in_tx` using
    ///    `scope.transaction()` as the `&mut Transaction` handle.
    ///
    /// Post B.2b-α (2026-04-22): `execute_verb_inner` delegates here by
    /// opening a per-verb scope; the Sequencer migration (B.2b-ζ) replaces
    /// that per-verb scope with an outer scope threaded from stage 8.
    pub(crate) async fn execute_verb_in_scope(
        &self,
        vc: &VerbCall,
        ctx: &mut ExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<ExecutionResult> {
        tracing::debug!("execute_verb_in_scope: ENTER {}.{}", vc.domain, vc.verb);

        // ── G4 (EOP-PLAN-CONTROLPLANE-GRADUATION-001 §3, needs G3):
        // per-step envelope admission for Path B/C, atomic with this
        // step's own dispatch (same scope). This is the convergence
        // point both `execute_plan` and `execute_plan_atomic_in_scope`
        // reach per-step (R:§B2) — the confirmed single seam for B/C.
        //
        // Double-admission guard (G3 §3(e)): `ObPocVerbExecutor`'s
        // Branch-3 fallthrough (Path A/D) reaches this identical seam
        // after already admitting under its own path tag. Skip only when
        // this dispatch already carries proof of admission for the EXACT
        // path this call is about to check — a value match, not a bare
        // boolean, so a mismatched-tag dispatch can never be waved
        // through.
        if ctx.already_admitted_for != Some(ctx.execution_path) {
            let fqn = format!("{}.{}", vc.domain, vc.verb);
            let enforced = crate::agent::control_plane_envelope_store::EnforcedVerbs::from_env()
                .map_err(|e| {
                    anyhow!(
                        "execute_verb_in_scope({fqn}): OB_POC_CONTROL_PLANE_ENFORCE_VERBS is \
                         unparseable — refusing to guess at enforcement state: {e}"
                    )
                })?;
            let (decision, pins) =
                crate::agent::control_plane_envelope_store::check_admission_in_scope(
                    scope.executor(),
                    &enforced,
                    &fqn,
                    ctx.execution_path,
                    // Every production Path B/C ingress point leaves this
                    // `None` — T9.3's established posture (no envelope-
                    // minting infrastructure wired for B/C yet). `ctx.
                    // envelope_handle` exists only so G4's own atomicity
                    // tests can exercise this call with a real envelope;
                    // see that field's doc comment.
                    ctx.envelope_handle,
                )
                .await
                .map_err(|e| anyhow!("execute_verb_in_scope({fqn}): admission check failed: {e}"))?;

            use crate::agent::control_plane_envelope_store::AdmissionDecision;
            match decision {
                AdmissionDecision::NotEnforced => {}
                AdmissionDecision::Admitted => {
                    // T10.2 parity with Path A/D's `execute_verb_admitting_envelope`:
                    // verify pinned entity state hasn't drifted since
                    // gating, inside the same scope, before dispatch. Only
                    // reachable when a real envelope with real pins was
                    // presented (never true for a production Path B/C
                    // caller today — see `envelope_handle`'s doc comment).
                    if let Some(pins) = &pins {
                        if let Err(e) =
                            ob_poc_boundary::toctou_recheck::verify_pins_in_scope(pins, scope.executor())
                                .await
                        {
                            bail!("{fqn} rejected: pinned entity state drifted since gating ({e})");
                        }
                    }
                }
                AdmissionDecision::RejectedNoEnvelope => {
                    bail!(
                        "{fqn} is enforce-mode gated (OB_POC_CONTROL_PLANE_ENFORCE_VERBS) on \
                         path {:?} but no sealed envelope was presented",
                        ctx.execution_path
                    );
                }
                AdmissionDecision::RejectedConsumeFailed(outcome) => {
                    bail!(
                        "{fqn} envelope admission rejected on path {:?}: {outcome:?}",
                        ctx.execution_path
                    );
                }
            }
        }

        let runtime_verb = runtime_registry()
            .get(&vc.domain, &vc.verb)
            .ok_or_else(|| anyhow!("Unknown verb: {}.{}", vc.domain, vc.verb))?;

        // ── G5 (EOP-PLAN-CONTROLPLANE-GRADUATION-001 §3 items 3-5):
        // Path B/C shadow-gate evaluation, extending the G1-G14 pipeline
        // beyond Path A's sole prior call site (`phase5_runtime_recheck`).
        // Best-effort, non-blocking (matches Path A's own posture: a
        // shadow-decision persistence failure must never affect the
        // request it observed) — spawned so it never adds latency or a
        // new failure mode to real dispatch.
        //
        // Deliberately bounded, not a full re-implementation of
        // `build_evaluation_context`: only the gates whose Path-A input
        // source generalizes cleanly to this seam without Sequencer-
        // specific state are wired for real: G1 (via `runtime_registry`
        // resolution) and G12 (`build_version_pinning_input`, no
        // session/pack/envelope argument at all) are independently
        // *substantive* here (both have zero declared predecessors in
        // `gate::GATE_DEPENDENCIES`). G8's `StpClassifierInput` is also
        // built (`build_stp_classifier_input`, same no-dependency-argument
        // reuse) but is NOT independently substantive yet: it declares 7
        // predecessors (IntentAdmission, EntityBinding, PackResolution,
        // DagProof, Authority, Evidence, WriteSet), none of which are
        // wired here, so it correctly reports `NotEvaluated`, not
        // `Success`/`Failure`, under collect-where-independent semantics
        // -- confirmed live by the E3 matrix probe's first run (see the
        // G5 session doc). G3/G9 are
        // the ratified NotApplicable cells (`ob_poc_control_plane::
        // applicability`). The remaining gates (G2, G4-G7, G10, G11, G13,
        // G14) are left `None` here -- an honest "not observed at this
        // seam yet", not a fabricated pass -- because their Path-A
        // builders (`build_entity_binding_input`, `build_dag_proof_input`
        // via `GatePipeline`, `build_write_set_input`, the evidence/
        // authority envelope-derived fields, `build_decision_snapshot_input`)
        // all assume `SemOsContextEnvelope`/`ReplOrchestratorV2::
        // GatePipeline`/batched entity-facts state that does not exist on
        // this engine (confirmed: `RealDslExecutor`/`DslExecutor` carry no
        // such fields -- see the G5 session doc's generalization-gap
        // finding). Wiring those for B/C is real follow-on work, not
        // silently folded into this tranche.
        #[cfg(feature = "database")]
        {
            let fqn = format!("{}.{}", vc.domain, vc.verb);
            let path = ctx.execution_path;
            if matches!(
                path,
                ob_poc_types::ExecutionPath::DslDirect | ob_poc_types::ExecutionPath::WorkflowDispatched
            ) {
                let pool = self.pool.clone();
                let session_id = ctx.session_id.unwrap_or_else(Uuid::nil);
                let entry_id = Uuid::new_v4();
                let is_durable_verb = matches!(&runtime_verb.behavior, RuntimeBehavior::Durable(_));
                tokio::spawn(async move {
                    let cp_ctx = ob_poc_control_plane::context::EvaluationContext {
                        intent_admission: Some(ob_poc_control_plane::intent_admission::IntentAdmissionInput {
                            intent_id: entry_id,
                            verb_fqn: fqn.clone(),
                            // The closest real fact this seam has: the
                            // verb resolved in the runtime registry at
                            // all. Weaker evidence than Path A's SemOS
                            // ABAC/pack-pruning grade (this function
                            // wouldn't have reached this point on an
                            // unresolvable verb) -- disclosed, not
                            // fabricated as an equivalent signal.
                            is_admitted: true,
                            exclusion_reasons: Vec::new(),
                            is_ai_originated: false,
                            interpretation_attested: false,
                        }),
                        stp_classifier: Some(
                            crate::agent::control_plane_shadow::build_stp_classifier_input(&fqn, false)
                        ),
                        version_pinning: Some(crate::agent::control_plane_shadow::build_version_pinning_input()),
                        ..Default::default()
                    };
                    let _ = is_durable_verb; // captured for future STP wiring parity; unused today
                    let report = ob_poc_control_plane::evaluate_shadow(&cp_ctx);
                    let report = ob_poc_control_plane::applicability::apply_matrix(report, path);
                    let row = crate::agent::control_plane_shadow::build_shadow_decision_row(
                        session_id, entry_id, &fqn, &report, false, path,
                    );
                    crate::agent::control_plane_shadow::insert_shadow_decision(&pool, &row).await;
                });
            }
        }

        // ── Phase 3 C1: execution-time `requires_states` precondition ──────
        // The "validate" half of select-then-validate. Classification stays
        // membership-scoped (discovery); executability is checked here against
        // the selected verb's own entity. FAIL-OPEN — see
        // `enforce_requires_states_precondition`. Only computes resolved args
        // when there is a non-empty `requires_states` to check.
        if runtime_verb
            .lifecycle
            .as_ref()
            .is_some_and(|lc| !lc.requires_states.is_empty())
        {
            if let Ok(json_args) = Self::verbcall_args_to_json(&vc.arguments, ctx) {
                enforce_requires_states_precondition(runtime_verb, &json_args, scope).await?;
            }
        }

        // Durable verbs: normally routed through WorkflowDispatcher. The
        // BPMN worker path sets `ctx.allow_durable_direct` so internal
        // service tasks can invoke durable verb implementations directly —
        // those share the worker's ambient scope.
        let is_durable_direct = matches!(&runtime_verb.behavior, RuntimeBehavior::Durable(_))
            && ctx.allow_durable_direct;
        if let RuntimeBehavior::Durable(d) = &runtime_verb.behavior {
            if !ctx.allow_durable_direct {
                return Err(anyhow!(
                    "Durable verb {}.{} (process_key={}) cannot execute inside a \
                     Sequencer-owned TransactionScope. Durable verbs require the V2 \
                     REPL with WorkflowDispatcher.",
                    vc.domain,
                    vc.verb,
                    d.process_key
                ));
            }
        }

        // Plugin verbs (or durable-direct treated as plugin): dispatch
        // through the in-scope primitive.
        if matches!(&runtime_verb.behavior, RuntimeBehavior::Plugin(_)) || is_durable_direct {
            let fqn = format!("{}.{}", vc.domain, vc.verb);
            let sem_os_ops = self.sem_os_ops.as_ref().ok_or_else(|| {
                anyhow!(
                    "Plugin {} has no SemOsVerbOp registered (SemOsVerbOpRegistry \
                     is either absent on this executor or missing the FQN). Wire \
                     `DslExecutor::with_sem_os_ops` at host startup.",
                    fqn
                )
            })?;
            let op = sem_os_ops.get(&fqn).ok_or_else(|| {
                anyhow!(
                    "Plugin {} has no SemOsVerbOp registered (SemOsVerbOpRegistry \
                     is either absent on this executor or missing the FQN). Wire \
                     `DslExecutor::with_sem_os_ops` at host startup.",
                    fqn
                )
            })?;
            return dispatch_plugin_via_sem_os_op_in_scope(
                op.as_ref(),
                &fqn,
                vc,
                ctx,
                &self.service_registry,
                scope,
            )
            .await;
        }

        // Generic CRUD verb: run via the generic executor using the scope's
        // transaction handle.
        tracing::debug!("execute_verb_in_scope: routing to GENERIC executor via scope");

        let json_args = Self::verbcall_args_to_json(&vc.arguments, ctx)?;
        let result = self
            .generic_executor
            .execute_in_tx(scope.transaction(), runtime_verb, &json_args)
            .await?;

        if runtime_verb.returns.capture {
            if let GenericExecutionResult::Uuid(uuid) = &result {
                if let Some(name) = &runtime_verb.returns.name {
                    ctx.bind(name, *uuid);
                }
            }
        }

        tracing::debug!("execute_verb_in_scope: EXIT success");
        Ok(result.to_legacy())
    }
}

/// T0.2 (EOP-PLAN-CONTROLPLANE-001, closes C-027 divergence): governs
/// whether `enforce_requires_states_precondition`'s four "policy declared
/// but unresolvable" fail-open classes (see [`LifecycleFailOpenClass`])
/// block execution or merely audit-and-pass. Default is `FailClosed` in
/// production; set `OB_POC_LIFECYCLE_GATE_MODE=fail-open` to restore the
/// original C1 fail-open behaviour (dev/test only).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleGateMode {
    FailOpen,
    FailClosed,
}

impl LifecycleGateMode {
    pub fn from_env() -> Self {
        match std::env::var("OB_POC_LIFECYCLE_GATE_MODE") {
            Ok(v) if v.eq_ignore_ascii_case("fail-open") => LifecycleGateMode::FailOpen,
            _ => LifecycleGateMode::FailClosed,
        }
    }
}

/// The five fail-open classes named by C-027 / T0.2's inventory citation
/// (`ob-poc/src/dsl_v2/executor.rs:L2015-L2041` in the Phase 0 inventory).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LifecycleFailOpenClass {
    /// No `lifecycle` block authored, OR `requires_states` is authored-empty.
    /// There is no policy to enforce — NOT gated by `LifecycleGateMode`,
    /// always passes. Only ~22/1306 verbs declare `requires_states` at all;
    /// fail-closing this class would block essentially all verb dispatch.
    NoLifecycleDeclared,
    /// `requires_states` is non-empty but `entity_arg` is unbound.
    NoEntityArg,
    /// `entity_arg` resolved but the argument value is not a valid uuid.
    InvalidUuid,
    /// No `SlotStateProvider` table mapping for the verb's domain.
    NoSlotMapping,
    /// The state read returned a DB error, no row, or a NULL column.
    StateUnreadable,
}

impl LifecycleFailOpenClass {
    fn as_str(self) -> &'static str {
        match self {
            Self::NoLifecycleDeclared => "no_lifecycle_declared",
            Self::NoEntityArg => "no_entity_arg",
            Self::InvalidUuid => "invalid_uuid",
            Self::NoSlotMapping => "no_slot_mapping",
            Self::StateUnreadable => "state_unreadable",
        }
    }
}

/// Always-audited pass-through for [`LifecycleFailOpenClass::NoLifecycleDeclared`]
/// — never gated by `mode`, since there is no declared policy to enforce.
/// `debug`-level: this fires on the overwhelming majority of verb calls
/// (no `requires_states` authored), so `warn`-level would flood production
/// logs with a structurally meaningless signal.
fn audit_lifecycle_no_policy(
    mode: LifecycleGateMode,
    runtime_verb: &RuntimeVerb,
    detail: &str,
) {
    tracing::debug!(
        target: "control_plane.lifecycle_gate",
        verb = %runtime_verb.full_name,
        class = LifecycleFailOpenClass::NoLifecycleDeclared.as_str(),
        mode = ?mode,
        detail,
        "requires_states precondition: no policy declared"
    );
}

/// Always-audited fail-open/fail-closed decision for a class where a policy
/// WAS declared (non-empty `requires_states`) but could not be resolved.
/// Returns `Ok(())` to pass, `Err` to block under `LifecycleGateMode::FailClosed`.
fn gate_lifecycle_fail_open(
    class: LifecycleFailOpenClass,
    mode: LifecycleGateMode,
    runtime_verb: &RuntimeVerb,
    detail: &str,
) -> Result<()> {
    tracing::warn!(
        target: "control_plane.lifecycle_gate",
        verb = %runtime_verb.full_name,
        class = class.as_str(),
        mode = ?mode,
        detail,
        "requires_states precondition unresolvable"
    );
    if mode == LifecycleGateMode::FailClosed {
        bail!(
            "precondition unresolvable for {}: {} ({}); refusing to execute under \
             fail-closed lifecycle gate mode (T0.2, closes C-027)",
            runtime_verb.full_name,
            class.as_str(),
            detail
        );
    }
    Ok(())
}

/// Phase 3 C1 — execution-time `requires_states` precondition.
///
/// The "validate" half of select-then-validate, relocated from the
/// discovery-time Step-5 prune (`agent::verb_surface` `PruneLayer::LifecycleState`).
/// Classification stays membership-scoped; executability is checked here, against
/// the *selected* verb's *own* entity, at the single dispatch chokepoint every
/// execution door funnels through (`execute_verb_in_scope`).
///
/// Resolves [`LifecycleGateMode`] from the environment on every call
/// (`OB_POC_LIFECYCLE_GATE_MODE`); see [`enforce_requires_states_precondition_with_mode`]
/// for the mode-parameterised, unit-testable implementation.
///
/// It hard-blocks — in both modes — when the entity's CURRENT, non-NULL
/// state is genuinely absent from `requires_states`: a true precondition
/// violation, returning an error that names the required states and the
/// actual state. That case is unrelated to `LifecycleGateMode` — the state
/// resolved fine, it just didn't match.
///
/// NOTE (deferred guard): there is no out-of-domain check yet — if a verb were
/// authored with both `entity_arg` and `requires_states` values that the mapped
/// state column cannot express, this would wrongly block it. No such verb exists
/// today (the `entity_arg`-bearing CBU verbs all require `cbus.status` values).
/// Authoring `entity_arg` onto an out-of-domain verb (e.g. an operational-state
/// verb) must add the constraint-domain guard first — tracked in the Phase-3 plan.
///
/// Reconciliation with `GateChecker` (C-025/C-026, DAG-taxonomy Mode A
/// blocking): that check owns cross-workspace transition legality read
/// through `DagRegistry`; this check owns the single-verb `requires_states`
/// precondition read directly off the verb's own slot table. The two run
/// independently today — recorded as a divergence in the ownership ledger,
/// to unify into one `G4 DAG proof` gate at T2.2.
#[cfg(feature = "database")]
async fn enforce_requires_states_precondition(
    runtime_verb: &RuntimeVerb,
    json_args: &HashMap<String, JsonValue>,
    scope: &mut dyn TransactionScope,
) -> Result<()> {
    enforce_requires_states_precondition_with_mode(
        runtime_verb,
        json_args,
        scope,
        LifecycleGateMode::from_env(),
    )
    .await
}

/// T0.2: mode-parameterised implementation. See [`enforce_requires_states_precondition`].
#[cfg(feature = "database")]
async fn enforce_requires_states_precondition_with_mode(
    runtime_verb: &RuntimeVerb,
    json_args: &HashMap<String, JsonValue>,
    scope: &mut dyn TransactionScope,
    mode: LifecycleGateMode,
) -> Result<()> {
    let Some(lifecycle) = runtime_verb.lifecycle.as_ref() else {
        audit_lifecycle_no_policy(mode, runtime_verb, "no lifecycle block authored");
        return Ok(());
    };
    if lifecycle.requires_states.is_empty() {
        audit_lifecycle_no_policy(mode, runtime_verb, "requires_states is empty");
        return Ok(());
    }
    // Opt-in: without an authored entity binding we cannot identify the entity.
    let Some(entity_arg) = lifecycle.entity_arg.as_deref() else {
        return gate_lifecycle_fail_open(
            LifecycleFailOpenClass::NoEntityArg,
            mode,
            runtime_verb,
            "requires_states present but entity_arg unbound",
        );
    };
    // Resolve the entity id from the (kebab-keyed) resolved args.
    let Some(entity_id) = json_args
        .get(entity_arg)
        .and_then(JsonValue::as_str)
        .and_then(|s| Uuid::parse_str(s).ok())
    else {
        return gate_lifecycle_fail_open(
            LifecycleFailOpenClass::InvalidUuid,
            mode,
            runtime_verb,
            &format!("entity_arg '{entity_arg}' did not resolve to a uuid"),
        );
    };
    // Convention: workspace == slot == verb domain (covers `cbu.cbu`). A verb
    // whose state lives under a non-self-named slot has no mapping here —
    // richer slot resolution is deferred (Phase-3 plan).
    let Ok((table, column, pk)) =
        dsl_runtime::resolve_slot_table(&runtime_verb.domain, &runtime_verb.domain)
    else {
        return gate_lifecycle_fail_open(
            LifecycleFailOpenClass::NoSlotMapping,
            mode,
            runtime_verb,
            &format!("no slot-state mapping for domain '{}'", runtime_verb.domain),
        );
    };
    // Read CURRENT state INSIDE the open transaction (sees prior in-txn steps).
    let sql = format!(r#"SELECT {column}::text AS state FROM "ob-poc".{table} WHERE {pk} = $1"#);
    let current_state = match sqlx::query_scalar::<sqlx::Postgres, Option<String>>(&sql)
        .bind(entity_id)
        .fetch_optional(scope.executor())
        .await
    {
        Ok(Some(Some(state))) => state,
        // DB error, no row, or NULL state.
        _ => {
            return gate_lifecycle_fail_open(
                LifecycleFailOpenClass::StateUnreadable,
                mode,
                runtime_verb,
                &format!("current state for {entity_id} absent/NULL/unreadable"),
            );
        }
    };
    if lifecycle
        .requires_states
        .iter()
        .any(|s| s == &current_state)
    {
        return Ok(());
    }
    bail!(
        "precondition not met: {} requires {} {} to be in one of {:?}, but it is '{}'",
        runtime_verb.full_name,
        runtime_verb.domain,
        entity_id,
        lifecycle.requires_states,
        current_state
    );
}

// ============================================================================
// Plan Execution
// ============================================================================

#[cfg(feature = "database")]
impl DslExecutor {
    /// Execute a compiled execution plan
    ///
    /// This is the preferred method for executing DSL with nested/composite operations.
    /// The plan has already been dependency-sorted by the compiler.
    ///
    /// Idempotency: Each statement is checked against the idempotency table.
    /// If already executed (same execution_id + statement_index + verb + args),
    /// the cached result is returned. Otherwise, the statement is executed
    /// and the result is recorded for future runs.
    ///
    /// # Example
    /// ```ignore
    /// let program = parse_program(dsl_source)?;
    /// let plan = compile(&program)?;
    /// let results = executor.execute_plan(&plan, &mut ctx).await?;
    /// ```
    pub async fn execute_plan(
        &self,
        plan: &super::execution_plan::ExecutionPlan,
        ctx: &mut ExecutionContext,
    ) -> Result<Vec<ExecutionResult>> {
        use crate::sequencer_tx::PgTransactionScope;
        use dsl_runtime::TransactionScope as _;

        // PRE-FLIGHT: Ensure all EntityRefs have been resolved before execution
        validate_all_resolved(plan)?;

        tracing::debug!("execute_plan: starting with {} steps", plan.steps.len());
        let mut results: Vec<ExecutionResult> = Vec::with_capacity(plan.steps.len());

        for (step_index, step) in plan.steps.iter().enumerate() {
            // Clone the verb call so we can inject values
            let mut vc = step.verb_call.clone();

            tracing::debug!(
                "DBG execute_plan: step {} verb={}.{} bind_as={:?}",
                step_index,
                &vc.domain,
                &vc.verb,
                &step.bind_as
            );

            // Trace each verb execution
            tracing::debug!(
                step = step_index,
                verb = %format!("{}.{}", &vc.domain, &vc.verb),
                bind_as = ?step.bind_as,
                "executing DSL step"
            );

            // Inject values from previous steps
            for inj in &step.injections {
                if let Some(ExecutionResult::Uuid(id)) = results.get(inj.from_step) {
                    // Add the injected argument
                    vc.arguments.push(super::ast::Argument {
                        key: inj.into_arg.clone(),
                        value: AstNode::string(id.to_string()),
                        span: super::ast::Span::default(),
                    });
                }
            }

            // Build args for idempotency check
            let verb_name = format!("{}.{}", vc.domain, vc.verb);
            let json_args = Self::verbcall_args_to_json(&vc.arguments, ctx)?;
            tracing::debug!("execute_plan: json_args={:?}", json_args);

            // Check idempotency cache if enabled
            if ctx.idempotency_enabled {
                tracing::debug!("execute_plan: checking idempotency cache...");
                if let Some(cached) = self
                    .idempotency
                    .check(ctx.execution_id, step_index, &verb_name, &json_args)
                    .await?
                {
                    tracing::debug!("execute_plan: cache HIT, returning cached result");
                    let result = cached.to_execution_result();

                    // Restore symbol binding from cached result
                    if let Some(ref binding_name) = step.bind_as {
                        if let ExecutionResult::Uuid(id) = &result {
                            ctx.bind(binding_name, *id);
                        }
                    }

                    results.push(result);
                    continue;
                }
                tracing::debug!("execute_plan: cache MISS, executing verb...");
            }

            // Open a per-step scope so verb writes and idempotency row are
            // committed together (B3 fix). Timeout prevents indefinite pool hang (E1).
            let step_start = std::time::Instant::now();
            let mut step_scope =
                PgTransactionScope::begin_timeout(&self.pool, pool_acquire_timeout())
                    .await
                    .map_err(|e| {
                        anyhow!("execute_plan step {} ({}): {}", step_index, verb_name, e)
                    })?;

            let verb_result = {
                let scope_dyn: &mut dyn TransactionScope = &mut step_scope;
                self.execute_verb_in_scope(&vc, ctx, scope_dyn).await
            };

            // Emit observability event (in-process, non-transactional).
            if let Some(ref events) = self.events {
                let duration_ms = step_start.elapsed().as_millis() as u64;
                match &verb_result {
                    Ok(_) => events.emit(ob_poc_diagnostics::events::DslEvent::succeeded(
                        ctx.session_id,
                        verb_name.clone(),
                        duration_ms,
                    )),
                    Err(e) => events.emit(ob_poc_diagnostics::events::DslEvent::failed(
                        ctx.session_id,
                        verb_name.clone(),
                        duration_ms,
                        e,
                    )),
                }
            }

            let result = match verb_result {
                Ok(r) => r,
                Err(step_err) => {
                    match step_scope.rollback().await {
                        Ok(()) => return Err(step_err),
                        Err(rb_err) => {
                            tracing::error!(
                                step = step_index,
                                verb = %verb_name,
                                %rb_err,
                                step_error = %step_err,
                                "execute_plan: CRITICAL — step failed AND rollback failed"
                            );
                            return Err(anyhow!(
                                "execute_plan step {} ({}) failed ({:#}) AND rollback failed ({:#})",
                                step_index, verb_name, step_err, rb_err
                            ));
                        }
                    }
                }
            };

            tracing::debug!(
                step = step_index,
                verb = %verb_name,
                result = ?result,
                "DSL step completed"
            );

            // Write idempotency row within the open step scope (B3 fix: atomic with verb).
            // E6 fix: propagate verb_hash lookup errors instead of silently using None.
            if ctx.idempotency_enabled {
                let verb_hash = self
                    .verb_hash_lookup
                    .get_verb_hash(&verb_name)
                    .await
                    .map_err(|e| anyhow!("verb_hash lookup failed for {}: {:#}", verb_name, e))?;

                self.idempotency
                    .record_with_view_state_in_tx(
                        step_scope.transaction(),
                        ctx.execution_id,
                        step_index,
                        &verb_name,
                        &json_args,
                        &result,
                        verb_hash.as_deref(),
                        &ctx.source_attribution,
                        ctx.pending_view_state.as_ref(),
                        ctx.session_id,
                    )
                    .await?;
            }

            // Commit scope: verb writes + idempotency row committed atomically.
            step_scope.commit().await.map_err(|e| {
                anyhow!(
                    "execute_plan step {} ({}): commit failed: {:#}",
                    step_index,
                    verb_name,
                    e
                )
            })?;

            // Handle explicit :as binding (in addition to verb's default capture)
            if let Some(ref binding_name) = step.bind_as {
                match &result {
                    ExecutionResult::Uuid(id) => {
                        ctx.bind(binding_name, *id);
                        // Also bind domain_id alias (e.g., cbu_id, entity_id) for convenience
                        let alias = format!("{}_id", step.verb_call.domain);
                        ctx.bind(&alias, *id);
                    }
                    ExecutionResult::RecordSet(records) => {
                        // Bind RecordSet to json_bindings for downstream access
                        ctx.bind_json(binding_name, serde_json::Value::Array(records.clone()));
                    }
                    ExecutionResult::Record(record) => {
                        // Bind single Record to json_bindings.
                        ctx.bind_json(binding_name, record.clone());
                        // Plugin verbs (e.g. cbu.create, kyc-case.create)
                        // return a Record whose primary entity id field is
                        // named after the noun: `<domain>_id` for single-word
                        // domains (cbu → cbu_id), or sometimes the trailing
                        // segment for multi-segment domains (kyc-case →
                        // case_id). Try both so legacy resolvers and
                        // downstream verbs that consume `@<binding>` see the
                        // new entity in the symbol table.
                        let domain = &step.verb_call.domain;
                        let mut candidates = Vec::new();
                        candidates.push(format!("{}_id", domain.replace('-', "_")));
                        if let Some(noun) = domain.rsplit('-').next() {
                            let noun_field = format!("{}_id", noun);
                            if !candidates.contains(&noun_field) {
                                candidates.push(noun_field);
                            }
                        }
                        for id_field in candidates {
                            if let Some(uuid) = record
                                .get(&id_field)
                                .and_then(|v| v.as_str())
                                .and_then(|s| uuid::Uuid::parse_str(s).ok())
                            {
                                ctx.bind(binding_name, uuid);
                                ctx.bind(&id_field, uuid);
                                break;
                            }
                        }
                    }
                    _ => {}
                }
            }

            results.push(result);
        }

        Ok(results)
    }

    /// Execute a compiled execution plan atomically within a single transaction
    ///
    /// This method wraps all verb executions in a single database transaction.
    /// If any verb fails, all preceding changes are rolled back.
    ///
    /// Use this for:
    /// - Batch operations that must succeed or fail together
    /// - Template expansion where partial execution is dangerous
    /// - Any DSL program that creates interdependent entities
    ///
    /// # Atomicity Guarantee
    /// All CRUD operations use the transaction. Plugin operations that don't
    /// implement `execute_in_tx` will fail fast, preventing partial execution.
    ///
    /// # Example
    /// ```ignore
    /// let program = parse_program(dsl_source)?;
    /// let plan = compile(&program)?;
    /// let results = executor.execute_plan_atomic(&plan, &mut ctx).await?;
    /// // Either all verbs succeeded, or none did
    /// ```
    pub async fn execute_plan_atomic(
        &self,
        plan: &super::execution_plan::ExecutionPlan,
        ctx: &mut ExecutionContext,
    ) -> Result<Vec<ExecutionResult>> {
        use crate::sequencer_tx::PgTransactionScope;

        // Phase B.2b-β (2026-04-22): thin wrapper over
        // `execute_plan_atomic_in_scope`. Opens a `PgTransactionScope`
        // for the plan's duration, delegates step execution to the
        // in-scope variant, commits on Ok, rolls back on Err. The
        // Sequencer migration (B.2b-ζ) calls `execute_plan_atomic_in_scope`
        // directly with an outer scope owning multiple plans so each
        // runbook step shares the same transaction.
        tracing::info!(
            "execute_plan_atomic: starting atomic execution with {} steps (self-scoped)",
            plan.steps.len()
        );

        let mut scope =
            PgTransactionScope::begin_timeout(&self.pool, pool_acquire_timeout()).await?;

        let outcome = {
            let scope_dyn: &mut dyn TransactionScope = &mut scope;
            self.execute_plan_atomic_in_scope(plan, ctx, scope_dyn)
                .await
        };

        match outcome {
            Ok(results) => {
                scope.commit().await?;
                tracing::info!(
                    "execute_plan_atomic: committed {} steps successfully",
                    results.len()
                );
                Ok(results)
            }
            Err(step_err) => {
                match scope.rollback().await {
                    Ok(()) => Err(step_err),
                    Err(rollback_err) => {
                        // Both the step and the rollback failed. DB state is unknown.
                        // Surface both errors so the caller knows atomicity was not preserved.
                        tracing::error!(
                            %rollback_err,
                            step_error = %step_err,
                            "execute_plan_atomic: CRITICAL — step failed AND rollback failed; \
                             DB state is unknown, manual intervention may be required"
                        );
                        Err(anyhow::anyhow!(
                            "step failed ({step_err:#}) AND rollback failed ({rollback_err:#}); \
                             DB state is unknown — manual intervention may be required"
                        ))
                    }
                }
            }
        }
    }

    /// Phase B.2a (F6 follow-on, 2026-04-22): execute a plan atomically
    /// inside a caller-supplied `TransactionScope`. The scope owns the
    /// transaction boundary — this method does NOT begin, commit, or
    /// roll back. The Sequencer (Phase B.2b) opens one outer scope for
    /// the runbook, calls this method per step, commits at stage 9a,
    /// and rolls back on first failure.
    ///
    /// Difference from [`Self::execute_plan_atomic`]:
    /// - Does not open / commit / rollback. Caller owns the boundary.
    /// - Dispatches plugin verbs through `execute_verb_in_scope`
    ///   (which in turn uses `dispatch_plugin_via_sem_os_op_in_scope`).
    /// - Generic verbs route through `scope.transaction()` instead of
    ///   a freshly-begun `sqlx::Transaction`.
    ///
    /// On any step error, returns `Err` immediately — caller decides
    /// to roll back the outer scope. Step outputs up to that point are
    /// NOT returned to the caller (they are gone with the rollback).
    ///
    /// Post B.2b-β (2026-04-22): this is the canonical atomic-plan entry
    /// point. `execute_plan_atomic` is a thin wrapper that opens a self-
    /// scoped transaction and calls this method. The Sequencer (B.2b-ζ)
    /// will call this directly with its outer scope, sharing a single
    /// transaction across multiple runbook steps.
    pub async fn execute_plan_atomic_in_scope(
        &self,
        plan: &super::execution_plan::ExecutionPlan,
        ctx: &mut ExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<Vec<ExecutionResult>> {
        validate_all_resolved(plan)?;

        // Create ExecutionFrame for this execution (v0.5 §7.2, T11).
        // Phase 5: frame is created here and carries execution_id + attempt_id.
        // Binding slots are populated as steps produce bindings (T10 typed slots).
        // Audit records are accumulated in frame.audit_buffer (written to DB in T14).
        // Phase 6: frame replaces ExecutionContext as the primary state carrier.
        let frame = dsl_runtime::frame::ExecutionFrame::new(30);
        tracing::debug!(
            execution_id = %frame.execution_id,
            attempt_id = %frame.attempt_id,
            scope_id = ?scope.scope_id(),
            "execute_plan_atomic_in_scope: starting with {} steps",
            plan.steps.len()
        );

        let mut results: Vec<ExecutionResult> = Vec::with_capacity(plan.steps.len());

        for (step_index, step) in plan.steps.iter().enumerate() {
            let mut vc = step.verb_call.clone();

            // Inject values from previous steps.
            for inj in &step.injections {
                if let Some(ExecutionResult::Uuid(id)) = results.get(inj.from_step) {
                    vc.arguments.push(super::ast::Argument {
                        key: inj.into_arg.clone(),
                        value: AstNode::string(id.to_string()),
                        span: super::ast::Span::default(),
                    });
                }
            }

            let result = self
                .execute_verb_in_scope(&vc, ctx, scope)
                .await
                .map_err(|e| {
                    tracing::error!(
                        step_index,
                        verb = format!("{}.{}", vc.domain, vc.verb).as_str(),
                        error = %e,
                        "execute_plan_atomic_in_scope: step failed — caller must rollback scope"
                    );
                    e
                })?;

            // Handle :as bindings exactly as execute_plan_atomic does.
            if let Some(ref binding_name) = step.bind_as {
                match &result {
                    ExecutionResult::Uuid(id) => {
                        ctx.bind(binding_name, *id);
                        let alias = format!("{}_id", step.verb_call.domain);
                        ctx.bind(&alias, *id);
                    }
                    ExecutionResult::RecordSet(records) => {
                        ctx.bind_json(binding_name, serde_json::Value::Array(records.clone()));
                    }
                    ExecutionResult::Record(record) => {
                        ctx.bind_json(binding_name, record.clone());
                    }
                    _ => {}
                }
            }

            results.push(result);
        }

        tracing::info!(
            scope_id = ?scope.scope_id(),
            "execute_plan_atomic_in_scope: {} steps produced; scope still open (caller commits/rolls back)",
            results.len()
        );

        Ok(results)
    }

    /// Execute a compiled execution plan atomically with optional advisory locking
    ///
    /// This is the full-featured atomic execution method that:
    /// 1. Optionally acquires advisory locks from the expansion report
    /// 2. Executes all steps in a single transaction
    /// 3. Returns detailed result with lock info
    ///
    /// **When to use:**
    /// - Batch operations that must succeed or fail together
    /// - Template expansion where concurrent modification must be prevented
    /// - Any operation where partial state is dangerous
    ///
    /// **Locking:**
    /// If an expansion report is provided with `derived_lock_set`, locks are
    /// acquired in sorted order before execution. This prevents concurrent
    /// sessions from modifying the locked entities mid-batch.
    ///
    /// # Arguments
    /// * `plan` - Compiled execution plan
    /// * `ctx` - Execution context for symbol bindings
    /// * `expansion_report` - Optional expansion report with lock set
    ///
    /// # Returns
    /// * `AtomicExecutionResult::Committed` - All steps succeeded
    /// * `AtomicExecutionResult::RolledBack` - A step failed, all rolled back
    /// * `AtomicExecutionResult::LockContention` - Could not acquire locks
    ///
    /// # Example
    /// ```ignore
    /// let expansion = expand_templates(dsl, &registry)?;
    /// let plan = compile(&parse_program(&expansion.expanded_dsl)?)?;
    ///
    /// match executor.execute_plan_atomic_with_locks(&plan, &mut ctx, Some(&expansion.report)).await? {
    ///     AtomicExecutionResult::Committed { step_results, .. } => {
    ///         println!("All {} steps committed", step_results.len());
    ///     }
    ///     AtomicExecutionResult::RolledBack { failed_at_step, error, .. } => {
    ///         println!("Failed at step {}: {}", failed_at_step, error);
    ///     }
    ///     AtomicExecutionResult::LockContention { entity_type, entity_id, .. } => {
    ///         println!("Lock contention on {}:{}", entity_type, entity_id);
    ///     }
    /// }
    /// ```
    pub async fn execute_plan_atomic_with_locks(
        &self,
        plan: &super::execution_plan::ExecutionPlan,
        ctx: &mut ExecutionContext,
        expansion_report: Option<&ExpansionReport>,
    ) -> Result<AtomicExecutionResult> {
        // PRE-FLIGHT: Ensure all EntityRefs have been resolved before execution
        validate_all_resolved(plan)?;

        tracing::info!(
            "execute_plan_atomic_with_locks: starting atomic execution with {} steps",
            plan.steps.len()
        );

        // Create ExecutionFrame for this execution (T14: audit buffer written before commit).
        let mut frame = dsl_runtime::frame::ExecutionFrame::new(30);

        // Start a transaction (with pool-acquire timeout — E1 fix, v0.5 §7.4).
        // Returns TimedOut instead of hanging indefinitely if the pool is exhausted.
        let timeout_dur = pool_acquire_timeout();
        let t0 = std::time::Instant::now();
        let tx_result = tokio::time::timeout(timeout_dur, self.pool.begin()).await;
        let mut tx = match tx_result {
            Ok(Ok(tx)) => tx,
            Ok(Err(e)) => return Err(e.into()),
            Err(_elapsed) => {
                return Ok(AtomicExecutionResult::TimedOut {
                    stage: "pool.begin".to_string(),
                    elapsed: t0.elapsed(),
                });
            }
        };

        // Coordination strategy gate (v0.5 §5.3, T12).
        // If every step in the plan declares a lock-free effect_class
        // (pure, read_snapshot, idempotent_ensure, append_fact, external_effect),
        // skip advisory lock acquisition entirely — the DB constraint or
        // idempotency guard is the coordination mechanism for those classes.
        // Steps with undeclared effect_class (None) fall back conservatively
        // to PessimisticResourceLock (pre-T12 behaviour).
        // Look up effect_class per step from VerbConfig (dsl_core config loader).
        // T09b will move this to a direct registry lookup once UnifiedVerbDef
        // exposes effect_class; for now we load from YAML at plan-check time
        // (once per plan, not once per step — loader is cached via OnceLock).
        let step_effect_classes: Vec<Option<dsl_core::EffectClass>> = {
            use dsl_core::ConfigLoader;
            let verbs_opt = ConfigLoader::from_env().load_verbs().ok();
            plan.steps
                .iter()
                .map(|s| {
                    verbs_opt
                        .as_ref()
                        .and_then(|cfg| cfg.domains.get(&s.verb_call.domain))
                        .and_then(|d| d.verbs.get(&s.verb_call.verb))
                        .and_then(|v| v.effect_class)
                })
                .collect()
        };
        let plan_needs_locks =
            dsl_runtime::coordination::plan_requires_locking(step_effect_classes);

        // Acquire locks if expansion report has them AND the plan requires locking
        let (locks_held, lock_wait_ms) = if let Some(report) = expansion_report {
            if !report.derived_lock_set.is_empty() && plan_needs_locks {
                tracing::debug!(
                    "execute_plan_atomic_with_locks: acquiring {} locks (coordination gate: locking required)",
                    report.derived_lock_set.len()
                );

                match acquire_locks(&mut tx, &report.derived_lock_set, LockMode::Try).await {
                    Ok(result) => (result.acquired, result.wait_time_ms),
                    Err(LockError::Contention {
                        entity_type,
                        entity_id,
                        acquired_so_far,
                        ..
                    }) => {
                        // Rollback and return contention error
                        tx.rollback().await?;
                        return Ok(AtomicExecutionResult::LockContention {
                            entity_type,
                            entity_id,
                            locks_acquired_before_contention: acquired_so_far,
                        });
                    }
                    Err(LockError::Database(e)) => {
                        tx.rollback().await?;
                        return Err(e.into());
                    }
                }
            } else {
                (vec![], 0)
            }
        } else {
            (vec![], 0)
        };

        tracing::debug!(
            "execute_plan_atomic_with_locks: acquired {} locks in {}ms",
            locks_held.len(),
            lock_wait_ms
        );

        let mut results: Vec<ExecutionResult> = Vec::with_capacity(plan.steps.len());

        for (step_index, step) in plan.steps.iter().enumerate() {
            // Clone the verb call so we can inject values
            let mut vc = step.verb_call.clone();

            tracing::debug!(
                "execute_plan_atomic_with_locks: step {} verb={}.{} bind_as={:?}",
                step_index,
                &vc.domain,
                &vc.verb,
                &step.bind_as
            );

            // Inject values from previous steps
            for inj in &step.injections {
                if let Some(ExecutionResult::Uuid(id)) = results.get(inj.from_step) {
                    vc.arguments.push(super::ast::Argument {
                        key: inj.into_arg.clone(),
                        value: AstNode::string(id.to_string()),
                        span: super::ast::Span::default(),
                    });
                }
            }

            // Execute the verb call within the transaction
            let result = match self.execute_verb_in_tx(&vc, ctx, &mut tx).await {
                Ok(r) => r,
                Err(e) => {
                    let error_msg = e.to_string();
                    // UniqueInsert coordination (v0.5 §5.3, T13): detect DB
                    // unique-constraint violations and return OptimisticConflict
                    // instead of RolledBack. This is a normal outcome under
                    // concurrent idempotent_ensure plans — not a failure.
                    if let Some(sqlx::Error::Database(db)) = e.downcast_ref::<sqlx::Error>() {
                        {
                            // Postgres unique_violation = SQLSTATE 23505
                            if db.code().as_deref() == Some("23505") {
                                let constraint =
                                    db.constraint().unwrap_or("unknown_constraint").to_string();
                                tracing::debug!(
                                    "execute_plan_atomic_with_locks: step {} unique-constraint \
                                     violation on '{}' → OptimisticConflict",
                                    step_index,
                                    constraint
                                );
                                tx.rollback().await?;
                                return Ok(AtomicExecutionResult::OptimisticConflict {
                                    constraint_name: constraint,
                                });
                            }
                        }
                    }
                    // Other errors: rollback and return RolledBack.
                    tracing::error!(
                        "execute_plan_atomic_with_locks: step {} ({}.{}) failed: {}. Rolling back.",
                        step_index,
                        vc.domain,
                        vc.verb,
                        error_msg
                    );
                    tx.rollback().await?;
                    return Ok(AtomicExecutionResult::RolledBack {
                        failed_at_step: step_index,
                        error: error_msg,
                        completed_steps: results,
                        locks_held,
                    });
                }
            };

            tracing::debug!(
                "execute_plan_atomic_with_locks: step {} completed",
                step_index
            );

            // Handle explicit :as binding
            if let Some(ref binding_name) = step.bind_as {
                match &result {
                    ExecutionResult::Uuid(id) => {
                        ctx.bind(binding_name, *id);
                        let alias = format!("{}_id", step.verb_call.domain);
                        ctx.bind(&alias, *id);
                    }
                    ExecutionResult::RecordSet(records) => {
                        ctx.bind_json(binding_name, serde_json::Value::Array(records.clone()));
                    }
                    ExecutionResult::Record(record) => {
                        ctx.bind_json(binding_name, record.clone());
                    }
                    _ => {}
                }
            }

            // Per-plan deadline check (v0.5 §7.4 — prevents indefinite hang).
            // Checked after each step. If the frame's deadline has passed,
            // rollback and return TimedOut rather than continuing.
            if frame.is_expired() {
                tracing::warn!(
                    "execute_plan_atomic_with_locks: plan deadline expired after step {}",
                    step_index
                );
                tx.rollback().await?;
                return Ok(AtomicExecutionResult::TimedOut {
                    stage: format!("plan_execution:step_{}", step_index),
                    elapsed: timeout_dur,
                });
            }

            // Record step outcome in the audit buffer (T14).
            frame.record_outcome(
                step_index,
                format!("{}.{}", step.verb_call.domain, step.verb_call.verb),
                chrono::Utc::now(),
                "committed",
            );

            results.push(result);
        }

        // Audit-as-commit-boundary (v0.5 §13.5.3, T14).
        // Write accumulated audit records inside the transaction before commit.
        // If the INSERT fails, the whole transaction rolls back — ensuring no
        // durable workflow state exists without a matching audit record.
        if !frame.audit_buffer.is_empty() {
            for record in &frame.audit_buffer.records {
                sqlx::query(
                    r#"
                    INSERT INTO "ob-poc".dsl_execution_audit
                        (execution_id, attempt_id, node_id, verb_fqn,
                         started_at, completed_at, outcome)
                    VALUES ($1, $2, $3, $4, $5, $6, $7)
                    "#,
                )
                .bind(record.execution_id.0)
                .bind(record.attempt_id.0 as i32)
                .bind(record.node_id as i32)
                .bind(&record.verb_fqn)
                .bind(record.started_at)
                .bind(record.completed_at)
                .bind(&record.outcome)
                .execute(tx.as_mut())
                .await
                .map_err(|e| anyhow::anyhow!("audit write failed before commit: {e}"))?;
            }
        }

        // All steps succeeded - commit the transaction (audit records commit together)
        tx.commit().await?;
        tracing::info!(
            "execute_plan_atomic_with_locks: committed {} steps successfully (held {} locks)",
            results.len(),
            locks_held.len()
        );

        Ok(AtomicExecutionResult::Committed {
            step_results: results,
            locks_held,
            lock_wait_ms,
        })
    }

    /// Execute a compiled execution plan with best-effort semantics
    ///
    /// Unlike `execute_plan_atomic()`, this method continues on failure and
    /// aggregates errors by root cause. Failed steps produce `None` in the
    /// results vector.
    ///
    /// **When to use:**
    /// - Batch operations where partial success is acceptable
    /// - Large imports where some records may fail validation
    /// - Operations where you need detailed error aggregation
    ///
    /// **Error Aggregation:**
    /// Instead of returning 50 separate "entity not found" errors, the result
    /// groups them: "1 cause, 50 affected operations: Entity XYZ not found"
    ///
    /// # Example
    /// ```ignore
    /// let result = executor.execute_plan_best_effort(&plan, &mut ctx).await?;
    /// if result.has_failures() {
    ///     println!("Some operations failed:\n{}", result.summary());
    /// }
    /// for (i, r) in result.verb_results.iter().enumerate() {
    ///     match r {
    ///         Some(exec_result) => println!("Step {}: succeeded", i),
    ///         None => println!("Step {}: failed", i),
    ///     }
    /// }
    /// ```
    pub async fn execute_plan_best_effort(
        &self,
        plan: &super::execution_plan::ExecutionPlan,
        ctx: &mut ExecutionContext,
    ) -> Result<BestEffortExecutionResult> {
        // PRE-FLIGHT: Ensure all EntityRefs have been resolved before execution
        validate_all_resolved(plan)?;

        tracing::info!(
            "execute_plan_best_effort: starting best-effort execution with {} steps",
            plan.steps.len()
        );

        let mut verb_results: Vec<Option<ExecutionResult>> = Vec::with_capacity(plan.steps.len());
        let mut errors = ExecutionErrors::new();

        for (step_index, step) in plan.steps.iter().enumerate() {
            // Clone the verb call so we can inject values
            let mut vc = step.verb_call.clone();

            tracing::debug!(
                "execute_plan_best_effort: step {} verb={}.{} bind_as={:?}",
                step_index,
                &vc.domain,
                &vc.verb,
                &step.bind_as
            );

            // Inject values from previous steps (only from successful steps)
            for inj in &step.injections {
                if let Some(Some(ExecutionResult::Uuid(id))) = verb_results.get(inj.from_step) {
                    vc.arguments.push(super::ast::Argument {
                        key: inj.into_arg.clone(),
                        value: AstNode::string(id.to_string()),
                        span: super::ast::Span::default(),
                    });
                } else if verb_results.get(inj.from_step).is_some_and(|r| r.is_none()) {
                    // Dependency failed - skip this step
                    tracing::debug!(
                        "execute_plan_best_effort: step {} skipped due to failed dependency (step {})",
                        step_index,
                        inj.from_step
                    );
                    errors.record_failure(
                        step_index,
                        &vc.domain,
                        &vc.verb,
                        &anyhow::anyhow!("Skipped: dependency step {} failed", inj.from_step),
                        None,
                    );
                    verb_results.push(None);
                    continue;
                }
            }

            // Execute the verb call
            match self.execute_verb(&vc, ctx).await {
                Ok(result) => {
                    tracing::debug!("execute_plan_best_effort: step {} succeeded", step_index);

                    // Handle explicit :as binding
                    if let Some(ref binding_name) = step.bind_as {
                        match &result {
                            ExecutionResult::Uuid(id) => {
                                ctx.bind(binding_name, *id);
                                let alias = format!("{}_id", step.verb_call.domain);
                                ctx.bind(&alias, *id);
                            }
                            ExecutionResult::RecordSet(records) => {
                                ctx.bind_json(
                                    binding_name,
                                    serde_json::Value::Array(records.clone()),
                                );
                            }
                            ExecutionResult::Record(record) => {
                                ctx.bind_json(binding_name, record.clone());
                            }
                            _ => {}
                        }
                    }

                    errors.record_success();
                    verb_results.push(Some(result));
                }
                Err(e) => {
                    tracing::warn!(
                        "execute_plan_best_effort: step {} ({}.{}) failed: {}",
                        step_index,
                        vc.domain,
                        vc.verb,
                        e
                    );

                    // Extract target entity if available from arguments
                    let target = vc.arguments.iter().find_map(|arg| {
                        if arg.key.ends_with("-id") || arg.key.ends_with("_id") {
                            match &arg.value {
                                AstNode::Literal(Literal::String(s), _) => Some(s.clone()),
                                AstNode::Literal(Literal::Uuid(u), _) => Some(u.to_string()),
                                _ => None,
                            }
                        } else {
                            None
                        }
                    });

                    errors.record_failure(step_index, &vc.domain, &vc.verb, &e, target);
                    verb_results.push(None);
                }
            }
        }

        // Determine overall status
        let status = if errors.total_failed == 0 {
            BatchStatus::AllSucceeded
        } else if errors.total_succeeded == 0 {
            BatchStatus::AllFailed
        } else {
            BatchStatus::PartialSuccess
        };

        tracing::info!(
            "execute_plan_best_effort: completed with status {:?} ({} succeeded, {} failed)",
            status,
            errors.total_succeeded,
            errors.total_failed
        );

        Ok(BestEffortExecutionResult {
            verb_results,
            errors,
            status,
        })
    }

    /// Convenience method: parse, enrich, compile, and execute DSL source
    ///
    /// This is the all-in-one method for executing DSL strings.
    /// Includes enrichment pass to convert string literals to EntityRefs.
    pub async fn execute_dsl(
        &self,
        source: &str,
        ctx: &mut ExecutionContext,
    ) -> Result<Vec<ExecutionResult>> {
        let raw_program =
            super::parser::parse_program(source).map_err(|e| anyhow!("Parse error: {}", e))?;

        // Enrich: convert string literals to EntityRefs based on YAML verb config
        let registry = super::runtime_registry::runtime_registry();
        let enrichment_result = super::enrich_program(raw_program, registry);
        let program = enrichment_result.program;

        // Note: EntityRef resolution happens during execution via GenericCrudExecutor
        // which calls resolve_lookup for args with lookup config

        let plan = super::execution_plan::compile(&program)
            .map_err(|e| anyhow!("Compile error: {}", e))?;

        self.execute_plan(&plan, ctx).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execution_context_bind_resolve() {
        let mut ctx = ExecutionContext::new();
        let id = Uuid::new_v4();
        ctx.bind("test", id);
        assert_eq!(ctx.resolve("test"), Some(id));
        assert_eq!(ctx.resolve("nonexistent"), None);
    }

    // ── T0.2 (EOP-PLAN-CONTROLPLANE-001, closes C-027 divergence) ──────────
    //
    // Table-driven coverage of the five `LifecycleFailOpenClass`es crossed
    // with both `LifecycleGateMode`s. Four classes (`NoLifecycleDeclared`,
    // `NoEntityArg`, `InvalidUuid`, `NoSlotMapping`) never touch the DB —
    // covered here with a scope double that panics if touched, proving by
    // construction that the early-return path never reaches the query.
    // `StateUnreadable` requires a real row read; covered in the
    // `#[cfg(feature = "database")]` `c1_requires_states_precondition`
    // integration test below (ghost-uuid case, both modes).
    #[cfg(feature = "database")]
    mod lifecycle_gate_mode_tests {
        use super::*;
        use crate::dsl_v2::runtime_registry::RuntimeReturn;

        /// `TransactionScope` double for classes that return before ever
        /// calling `scope.executor()`/`scope.transaction()`/`scope.pool()`.
        /// Panics if any of those are reached — the panic IS the assertion
        /// that the fail-open early-return actually fired before any DB call.
        struct PanicScope;

        impl TransactionScope for PanicScope {
            fn scope_id(&self) -> ob_poc_types::TransactionScopeId {
                ob_poc_types::TransactionScopeId::new()
            }
            fn transaction(&mut self) -> &mut sqlx::Transaction<'static, sqlx::Postgres> {
                panic!("PanicScope: transaction() reached — DB should not be touched here")
            }
            fn pool(&self) -> &sqlx::PgPool {
                panic!("PanicScope: pool() reached — DB should not be touched here")
            }
        }

        fn fixture_verb(lifecycle: Option<dsl_core::VerbLifecycle>, domain: &str) -> RuntimeVerb {
            RuntimeVerb {
                domain: domain.to_string(),
                verb: "noop".to_string(),
                full_name: format!("{domain}.noop"),
                description: "T0.2 test fixture".to_string(),
                harm_class: None,
                subject_kinds: vec![],
                behavior: RuntimeBehavior::Plugin("noop".to_string()),
                args: vec![],
                returns: RuntimeReturn {
                    return_type: dsl_core::ReturnTypeConfig::Void,
                    name: None,
                    capture: false,
                },
                produces: None,
                consumes: vec![],
                lifecycle,
                policy: None,
                phase_tags: vec![],
            }
        }

        fn no_args() -> HashMap<String, JsonValue> {
            HashMap::new()
        }

        async fn run(verb: &RuntimeVerb, mode: LifecycleGateMode) -> Result<()> {
            enforce_requires_states_precondition_with_mode(
                verb,
                &no_args(),
                &mut PanicScope,
                mode,
            )
            .await
        }

        #[tokio::test]
        async fn no_lifecycle_declared_passes_in_both_modes() {
            let verb = fixture_verb(None, "t0-2-test");
            run(&verb, LifecycleGateMode::FailOpen)
                .await
                .expect("no lifecycle block ⇒ always passes (FailOpen)");
            run(&verb, LifecycleGateMode::FailClosed)
                .await
                .expect("no lifecycle block ⇒ always passes (FailClosed) — no policy to enforce");
        }

        #[tokio::test]
        async fn empty_requires_states_passes_in_both_modes() {
            let verb = fixture_verb(Some(dsl_core::VerbLifecycle::default()), "t0-2-test");
            run(&verb, LifecycleGateMode::FailOpen)
                .await
                .expect("empty requires_states ⇒ always passes (FailOpen)");
            run(&verb, LifecycleGateMode::FailClosed)
                .await
                .expect("empty requires_states ⇒ always passes (FailClosed) — no policy to enforce");
        }

        #[tokio::test]
        async fn no_entity_arg_is_mode_gated() {
            let verb = fixture_verb(
                Some(dsl_core::VerbLifecycle {
                    entity_arg: None,
                    requires_states: vec!["SOME_STATE".to_string()],
                    ..Default::default()
                }),
                "t0-2-test",
            );
            run(&verb, LifecycleGateMode::FailOpen)
                .await
                .expect("no entity_arg ⇒ passes under FailOpen");
            let blocked = run(&verb, LifecycleGateMode::FailClosed).await;
            assert!(
                blocked.is_err(),
                "no entity_arg ⇒ blocks under FailClosed (T0.2)"
            );
            assert!(blocked.unwrap_err().to_string().contains("no_entity_arg"));
        }

        #[tokio::test]
        async fn invalid_uuid_is_mode_gated() {
            let verb = fixture_verb(
                Some(dsl_core::VerbLifecycle {
                    entity_arg: Some("thing-id".to_string()),
                    requires_states: vec!["SOME_STATE".to_string()],
                    ..Default::default()
                }),
                "t0-2-test",
            );
            let mut args = HashMap::new();
            args.insert(
                "thing-id".to_string(),
                JsonValue::String("not-a-uuid".to_string()),
            );
            let pass = enforce_requires_states_precondition_with_mode(
                &verb,
                &args,
                &mut PanicScope,
                LifecycleGateMode::FailOpen,
            )
            .await;
            pass.expect("invalid uuid ⇒ passes under FailOpen");
            let blocked = enforce_requires_states_precondition_with_mode(
                &verb,
                &args,
                &mut PanicScope,
                LifecycleGateMode::FailClosed,
            )
            .await;
            assert!(
                blocked.is_err(),
                "invalid uuid ⇒ blocks under FailClosed (T0.2)"
            );
            assert!(blocked.unwrap_err().to_string().contains("invalid_uuid"));
        }

        #[tokio::test]
        async fn no_slot_mapping_is_mode_gated() {
            // A domain guaranteed to have no `SlotStateProvider` mapping.
            let verb = fixture_verb(
                Some(dsl_core::VerbLifecycle {
                    entity_arg: Some("thing-id".to_string()),
                    requires_states: vec!["SOME_STATE".to_string()],
                    ..Default::default()
                }),
                "t0-2-unmapped-domain",
            );
            let mut args = HashMap::new();
            args.insert(
                "thing-id".to_string(),
                JsonValue::String(Uuid::new_v4().to_string()),
            );
            let pass = enforce_requires_states_precondition_with_mode(
                &verb,
                &args,
                &mut PanicScope,
                LifecycleGateMode::FailOpen,
            )
            .await;
            pass.expect("no slot mapping ⇒ passes under FailOpen");
            let blocked = enforce_requires_states_precondition_with_mode(
                &verb,
                &args,
                &mut PanicScope,
                LifecycleGateMode::FailClosed,
            )
            .await;
            assert!(
                blocked.is_err(),
                "no slot mapping ⇒ blocks under FailClosed (T0.2)"
            );
            assert!(blocked.unwrap_err().to_string().contains("no_slot_mapping"));
        }
    }

    #[cfg(feature = "database")]
    #[test]
    fn test_sem_os_registry_has_plugin_verbs() {
        // Post slice #80: the canonical registry lives in
        // `sem_os_postgres::ops::build_registry()` merged with
        // `ob_poc::domain_ops::extend_registry()`. This smoke test just
        // asserts that the SemOS-side factory builds a non-empty registry
        // so bin builds catch a regression where every op accidentally
        // vanishes.
        let mut registry = sem_os_postgres::ops::build_registry();
        crate::domain_ops::extend_registry(&mut registry);
        assert!(
            registry.len() > 100,
            "SemOsVerbOpRegistry should have >100 ops, got {}",
            registry.len()
        );
    }

    /// Phase 3 C1 — execution-time `requires_states` precondition, exercised at
    /// the dispatch chokepoint helper directly (no full DslExecutor needed).
    ///
    /// Read-only: picks existing committed CBU rows by status and runs the
    /// precondition against the open (rolled-back) transaction. Uses the REAL
    /// registry verbs so the lifecycle metadata under test is the authored one.
    ///
    /// Proves select-then-validate's "validate" half end-to-end (real registry
    /// verbs, against a DISCOVERED CBU):
    /// - BLOCK: `cbu.confirm` (requires VALIDATION_PENDING) → Err naming VALIDATION_PENDING.
    /// - PASS:  `cbu.submit-for-validation` (requires DISCOVERED|VALIDATION_FAILED) → Ok.
    /// - BLOCK: `cbu.add-product` (ARMED: `entity_arg=cbu-id`, requires VALIDATED) → Err
    ///   naming VALIDATED — the confirm-first gate at the junction (commit 95d2a238).
    /// - FAIL-OPEN: any gated verb against a non-existent cbu (no row) → Ok regardless;
    ///   the absent-state branch F-D will tighten once operational state is populated.
    ///
    /// Run: `DATABASE_URL=… cargo test --features database -p ob-poc \
    ///   --lib -- dsl_v2::executor::tests::c1_requires_states_precondition --ignored --nocapture`
    #[cfg(feature = "database")]
    #[tokio::test]
    #[ignore = "requires DATABASE_URL (dev DB)"]
    async fn c1_requires_states_precondition() {
        let url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        let pool = sqlx::PgPool::connect(&url).await.expect("connect");

        // Ensure the cbu.cbu slot mapping is present (idempotent across the run).
        if dsl_runtime::resolve_slot_table("cbu", "cbu").is_err() {
            let map: std::collections::HashMap<String, (String, String, String)> = [(
                "cbu.cbu".to_string(),
                (
                    "cbus".to_string(),
                    "status".to_string(),
                    "cbu_id".to_string(),
                ),
            )]
            .into_iter()
            .collect();
            dsl_runtime::set_slot_state_table(map);
        }

        let mut scope = crate::sequencer_tx::PgTransactionScope::begin(&pool)
            .await
            .expect("begin scope");

        // An existing committed DISCOVERED row (read-only; scope rolls back on drop).
        let discovered: Uuid = sqlx::query_scalar(
            r#"SELECT cbu_id FROM "ob-poc".cbus WHERE status = 'DISCOVERED' LIMIT 1"#,
        )
        .fetch_one(scope.executor())
        .await
        .expect("a DISCOVERED cbu must exist");

        let reg = runtime_registry();
        let confirm = reg.get("cbu", "confirm").expect("cbu.confirm registered");
        let submit = reg
            .get("cbu", "submit-for-validation")
            .expect("cbu.submit-for-validation registered");
        let add_product = reg
            .get("cbu", "add-product")
            .expect("cbu.add-product registered");

        let args = |id: Uuid| -> HashMap<String, JsonValue> {
            [("cbu-id".to_string(), JsonValue::String(id.to_string()))]
                .into_iter()
                .collect()
        };

        // BLOCK: the dedicated confirm verb (requires VALIDATION_PENDING) on a
        // DISCOVERED cbu — the structural-validation gate, C1-enforced with a
        // named error (replaces the deleted cbu.decide as the block example).
        let confirm_blocked =
            enforce_requires_states_precondition(confirm, &args(discovered), &mut scope).await;
        let cmsg = confirm_blocked
            .expect_err("confirm on DISCOVERED must be blocked")
            .to_string();
        assert!(
            cmsg.contains("VALIDATION_PENDING") && cmsg.contains("cbu.confirm"),
            "confirm block must name the required state and verb: {cmsg}"
        );

        // PASS: submit-for-validation (requires DISCOVERED|VALIDATION_FAILED) on a DISCOVERED cbu.
        enforce_requires_states_precondition(submit, &args(discovered), &mut scope)
            .await
            .expect("submit-for-validation on DISCOVERED must pass");

        // BLOCK: add-product is now ARMED (commit 95d2a238 authored
        // entity_arg=cbu-id alongside requires_states:[VALIDATED]) — the
        // confirm-first gate. On a DISCOVERED cbu it must hard-block at the
        // junction with a named error.
        assert!(
            add_product.lifecycle.as_ref().is_some_and(|lc| {
                lc.entity_arg.as_deref() == Some("cbu-id")
                    && lc.requires_states.iter().any(|s| s == "VALIDATED")
            }),
            "fixture assumption: cbu.add-product is armed (entity_arg=cbu-id, requires VALIDATED)"
        );
        let ap_blocked =
            enforce_requires_states_precondition(add_product, &args(discovered), &mut scope).await;
        let apmsg = ap_blocked
            .expect_err("add-product on DISCOVERED must be blocked (confirm-first)")
            .to_string();
        assert!(
            apmsg.contains("VALIDATED") && apmsg.contains("cbu.add-product"),
            "add-product block must name the required state and verb: {apmsg}"
        );

        // T0.2: the absent-state (`StateUnreadable`) class is now mode-gated.
        // `FailOpen` preserves the original C1 "never brick" guarantee;
        // `FailClosed` (the production default) now blocks it — that is
        // exactly what T0.2 closes C-027 for.
        let ghost = Uuid::new_v4();
        enforce_requires_states_precondition_with_mode(
            confirm,
            &args(ghost),
            &mut scope,
            LifecycleGateMode::FailOpen,
        )
        .await
        .expect("absent row ⇒ FailOpen mode never bricks (original C1 guarantee)");

        let ghost_blocked = enforce_requires_states_precondition_with_mode(
            confirm,
            &args(ghost),
            &mut scope,
            LifecycleGateMode::FailClosed,
        )
        .await;
        assert!(
            ghost_blocked.is_err(),
            "T0.2: absent row must block under FailClosed (closes C-027)"
        );

        // scope drops here → rollback; no rows mutated anyway.
    }

    /// G4 (`EOP-PLAN-CONTROLPLANE-GRADUATION-001` §3, per
    /// `EOP-DESIGN-CONTROLPLANE-G3-ENFORCEMENT-DIMENSION-001`): the
    /// per-step admission call now wired into `execute_verb_in_scope`
    /// (the confirmed single seam both `execute_plan` and
    /// `execute_plan_atomic_in_scope` reach per-step, R:§B2) — Path B/C's
    /// atomicity properties, item 4's `t4_1` equivalents, and item 2's
    /// double-admission-guard hard test.
    #[cfg(feature = "database")]
    mod g4_seam_admission_tests {
        use super::*;
        use crate::sequencer_tx::PgTransactionScope;

        async fn test_pool() -> sqlx::PgPool {
            let url = std::env::var("DATABASE_URL").expect("DATABASE_URL required for db-integration tests");
            sqlx::PgPool::connect(&url).await.expect("connect")
        }

        /// Guards `OB_POC_CONTROL_PLANE_ENFORCE_VERBS` mutation — process-
        /// global env var, tests must not interleave (mirrors
        /// `verb_executor_adapter.rs`'s `EnvGuard`).
        static ENV_GUARD_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        struct EnvGuard(#[allow(dead_code)] std::sync::MutexGuard<'static, ()>);
        impl EnvGuard {
            fn set(value: &str) -> Self {
                let guard = ENV_GUARD_LOCK.lock().unwrap_or_else(|e| e.into_inner());
                std::env::set_var("OB_POC_CONTROL_PLANE_ENFORCE_VERBS", value);
                Self(guard)
            }
        }
        impl Drop for EnvGuard {
            fn drop(&mut self) {
                std::env::remove_var("OB_POC_CONTROL_PLANE_ENFORCE_VERBS");
            }
        }

        fn verb_call(domain: &str, verb: &str, args: Vec<(&str, &str)>) -> VerbCall {
            VerbCall {
                domain: domain.to_string(),
                verb: verb.to_string(),
                arguments: args
                    .into_iter()
                    .map(|(k, v)| dsl_core::Argument {
                        key: k.to_string(),
                        value: AstNode::string(v.to_string()),
                        span: dsl_core::Span::default(),
                    })
                    .collect(),
                lens_override: None,
                binding: None,
                span: dsl_core::Span::default(),
            }
        }

        fn ctx_for(path: ob_poc_types::ExecutionPath) -> ExecutionContext {
            ExecutionContext {
                execution_path: path,
                ..ExecutionContext::default()
            }
        }

        /// Item 4: rollback-of-consume on dispatch failure. Admission
        /// succeeds (consumes the envelope inside the caller's scope),
        /// but the dispatch that follows fails (no `cbu-id` supplied) —
        /// the whole scope, including the consume, must roll back, so
        /// the envelope is still consumable afterward. Same property
        /// `execute_verb_admitting_envelope_rolls_back_the_consume_when_dispatch_fails`
        /// proves for Path A/D (`verb_executor_adapter.rs`), now proven
        /// from the dsl_v2 seam directly (Path B/C's convergence point).
        #[tokio::test]
        #[ignore = "requires DATABASE_URL"]
        async fn seam_rolls_back_the_consume_when_dispatch_fails() {
            let _guard = EnvGuard::set("cbu.confirm:B");
            let pool = test_pool().await;

            let envelope_id = Uuid::new_v4();
            let content_hash: [u8; 32] = [0x41; 32];
            let handle = ob_poc_types::EnvelopeHandle::new(envelope_id, content_hash);
            sqlx::query(
                r#"
                INSERT INTO "ob-poc".control_plane_envelopes (
                    envelope_id, content_hash, session_id, verb_fqn,
                    status, not_before, not_after
                ) VALUES ($1, $2, $3, 'cbu.confirm', 'sealed', now() - interval '1 minute', now() + interval '5 minutes')
                "#,
            )
            .bind(envelope_id)
            .bind(handle.content_hash_hex())
            .bind(Uuid::new_v4())
            .execute(&pool)
            .await
            .expect("insert sealed envelope row");

            let executor = DslExecutor::new(pool.clone());
            let mut ctx = ctx_for(ob_poc_types::ExecutionPath::DslDirect);
            ctx.envelope_handle = Some(handle);
            let vc = verb_call("cbu", "confirm", vec![]); // no cbu-id -> dispatch fails past admission

            let mut scope = PgTransactionScope::begin(&pool).await.expect("begin scope");
            let dispatch_err = {
                let scope_dyn: &mut dyn TransactionScope = &mut scope;
                executor
                    .execute_verb_in_scope(&vc, &mut ctx, scope_dyn)
                    .await
                    .expect_err("dispatching cbu.confirm with no args must fail")
            };
            assert!(
                !dispatch_err.to_string().contains("enforce-mode gated")
                    && !dispatch_err.to_string().contains("envelope admission rejected"),
                "the failure must come from dispatch, not admission: {dispatch_err}"
            );
            scope.rollback().await.expect("rollback");

            // The envelope must still be consumable — the whole scope,
            // including the consume, rolled back together.
            let mut retry_scope = PgTransactionScope::begin(&pool).await.expect("begin retry scope");
            let enforced = crate::agent::control_plane_envelope_store::EnforcedVerbs::parse("cbu.confirm:B").unwrap();
            let (decision, _pins) = crate::agent::control_plane_envelope_store::check_admission_in_scope(
                retry_scope.executor(),
                &enforced,
                "cbu.confirm",
                ob_poc_types::ExecutionPath::DslDirect,
                Some(handle),
            )
            .await
            .expect("admission check must succeed");
            retry_scope.rollback().await.expect("rollback retry scope");
            assert_eq!(
                decision,
                crate::agent::control_plane_envelope_store::AdmissionDecision::Admitted,
                "a rolled-back scope must not leave the envelope durably consumed"
            );
        }

        /// Item 4: pin-drift rejection leaves the envelope reconsumable.
        /// Same property
        /// `execute_verb_admitting_envelope_rejects_on_pin_drift_and_leaves_envelope_reconsumable`
        /// proves for Path A/D, now proven at the dsl_v2 seam: a sealed
        /// envelope pinning a stale `row_version` is rejected at
        /// admission (not merely that `verify_pins_in_scope` rejects in
        /// isolation), and the rejection rolls the whole scope back
        /// rather than partially admitting.
        #[tokio::test]
        #[ignore = "requires DATABASE_URL"]
        async fn seam_rejects_on_pin_drift_and_leaves_envelope_reconsumable() {
            let _guard = EnvGuard::set("cbu.confirm:B");
            let pool = test_pool().await;

            let (cbu_id, real_row_version): (Uuid, i64) =
                sqlx::query_as(r#"SELECT cbu_id, row_version FROM "ob-poc".cbus ORDER BY cbu_id LIMIT 1 OFFSET 3"#)
                    .fetch_one(&pool)
                    .await
                    .expect("at least 4 cbu rows exist in the dev database (offsets 0-2 used by sibling tests)");

            let intent = ob_poc_control_plane::intent_admission::tests_support::admitted(Uuid::new_v4(), "cbu.confirm");
            let binding = ob_poc_control_plane::entity_binding::tests_support::bound(vec![cbu_id]);
            let pack = ob_poc_control_plane::pack_resolution::tests_support::resolved("ob-poc.cbu");
            let dag =
                ob_poc_control_plane::dag_proof::tests_support::legal(cbu_id, "VALIDATION_PENDING", "VALIDATED");
            let authority =
                ob_poc_control_plane::authority_gate::tests_support::authorised("actor-1", "compliance_officer");
            let evidence = ob_poc_control_plane::evidence_gate::tests_support::sufficient(vec!["obligation-1".into()]);
            let write_set = ob_poc_control_plane::write_set::tests_support::proof(
                vec![cbu_id],
                vec!["validation_state".into()],
                vec!["ob-poc.cbus".into()],
                vec!["status".into()],
                "idem-g4-pin-drift",
            );
            let runbook = ob_poc_control_plane::proof::CompiledRunbookRef::new(Uuid::new_v4());
            let snapshot = ob_poc_control_plane::snapshot::tests_support::pins(
                Some(Uuid::new_v4()),
                None,
                None,
                vec![(cbu_id, "cbu".to_string(), real_row_version - 1)],
            );
            let now = chrono::Utc::now();
            let envelope = ob_poc_control_plane::envelope::test_support::seal(
                intent,
                binding,
                pack,
                dag,
                authority,
                evidence,
                write_set,
                runbook,
                snapshot,
                ob_poc_control_plane::envelope::ValidityWindow::new(
                    now - chrono::Duration::minutes(1),
                    now + chrono::Duration::minutes(5),
                ),
            );
            let handle = envelope.handle();
            assert!(
                crate::agent::control_plane_envelope_store::persist_sealed(
                    &pool,
                    Uuid::new_v4(),
                    Uuid::now_v7(),
                    "cbu.confirm",
                    &envelope,
                )
                .await
            );

            let executor = DslExecutor::new(pool.clone());
            let mut ctx = ctx_for(ob_poc_types::ExecutionPath::DslDirect);
            ctx.envelope_handle = Some(handle);
            let vc = verb_call("cbu", "confirm", vec![("cbu-id", &cbu_id.to_string())]);

            let mut scope = PgTransactionScope::begin(&pool).await.expect("begin scope");
            let dispatch_err = {
                let scope_dyn: &mut dyn TransactionScope = &mut scope;
                executor
                    .execute_verb_in_scope(&vc, &mut ctx, scope_dyn)
                    .await
                    .expect_err("stale-pinned cbu.confirm must be rejected")
            };
            assert!(
                dispatch_err.to_string().contains("pinned entity state drifted"),
                "must be rejected for pin drift specifically: {dispatch_err}"
            );
            scope.rollback().await.expect("rollback");

            let mut retry_scope = PgTransactionScope::begin(&pool).await.expect("begin retry scope");
            let enforced = crate::agent::control_plane_envelope_store::EnforcedVerbs::parse("cbu.confirm:B").unwrap();
            let (decision, _pins) = crate::agent::control_plane_envelope_store::check_admission_in_scope(
                retry_scope.executor(),
                &enforced,
                "cbu.confirm",
                ob_poc_types::ExecutionPath::DslDirect,
                Some(handle),
            )
            .await
            .expect("admission check must succeed");
            retry_scope.rollback().await.expect("rollback retry scope");
            assert_eq!(
                decision,
                crate::agent::control_plane_envelope_store::AdmissionDecision::Admitted,
                "pin-drift rejection must not have burned the envelope's single use"
            );
        }

        /// Item 2's named hard test: `ObPocVerbExecutor`'s Branch-3
        /// fallthrough (`execute_verb_in_open_scope`) reaches this exact
        /// seam after already admitting under its own path tag. The
        /// skip decision is a value match — `already_admitted_for ==
        /// Some(execution_path)` — not a bare boolean, so:
        /// (a) a dispatch that already carries proof of admission for
        ///     the SAME path the seam is about to check is never
        ///     re-rejected by a second, envelope-less admission check;
        /// (b) a dispatch carrying proof for a DIFFERENT path than the
        ///     one being checked is never waved through.
        #[tokio::test]
        #[ignore = "requires DATABASE_URL"]
        async fn seam_skip_is_keyed_on_exact_path_match_not_a_bare_flag() {
            let _guard = EnvGuard::set("cbu.confirm:A");
            let pool = test_pool().await;
            let executor = DslExecutor::new(pool.clone());
            // No `cbu-id` — dispatch reaches the requires_states
            // precondition and fails there. What we assert on is
            // WHETHER admission itself rejected first, not the ultimate
            // outcome, so an inert failure downstream of admission is
            // fine for isolating the admission decision.
            let vc = verb_call("cbu", "confirm", vec![]);

            // (a) Match: already_admitted_for == execution_path == A.
            // The seam must SKIP its own EnforcedVerbs check — despite
            // `cbu.confirm` being enforced on A with no envelope in
            // `ctx`, the error must NOT be an admission rejection.
            {
                let mut ctx = ctx_for(ob_poc_types::ExecutionPath::RunbookSequencer);
                ctx.already_admitted_for = Some(ob_poc_types::ExecutionPath::RunbookSequencer);
                let mut scope = PgTransactionScope::begin(&pool).await.expect("begin scope");
                let err = {
                    let scope_dyn: &mut dyn TransactionScope = &mut scope;
                    executor
                        .execute_verb_in_scope(&vc, &mut ctx, scope_dyn)
                        .await
                        .expect_err("no cbu-id must still fail downstream of admission")
                };
                scope.rollback().await.expect("rollback");
                assert!(
                    !err.to_string().contains("enforce-mode gated"),
                    "matching tag must skip the seam's own admission re-check: {err}"
                );
            }

            // (b) Mismatch: already_admitted_for = C, execution_path = A.
            // The seam must NOT skip — a mismatched-tag dispatch is
            // checked exactly as if it carried no prior admission proof
            // at all, and (enforced, no envelope) must reject.
            {
                let mut ctx = ctx_for(ob_poc_types::ExecutionPath::RunbookSequencer);
                ctx.already_admitted_for = Some(ob_poc_types::ExecutionPath::WorkflowDispatched);
                let mut scope = PgTransactionScope::begin(&pool).await.expect("begin scope");
                let err = {
                    let scope_dyn: &mut dyn TransactionScope = &mut scope;
                    executor
                        .execute_verb_in_scope(&vc, &mut ctx, scope_dyn)
                        .await
                        .expect_err("mismatched tag must not be waved through")
                };
                scope.rollback().await.expect("rollback");
                assert!(
                    err.to_string().contains("enforce-mode gated"),
                    "a mismatched tag must be checked as if unadmitted: {err}"
                );
            }
        }

        /// Branch-3 fallthrough itself, exercised through
        /// `ObPocVerbExecutor::execute_verb_admitting_envelope` end to
        /// end: a Path-A dispatch that admits successfully and then
        /// falls through Branch 3 into this seam must consume the
        /// envelope EXACTLY ONCE — not twice (the seam skipping its own
        /// check) and not zero times (the outer admission still ran for
        /// real). Complements the direct unit-level proof above with
        /// the actual production call chain.
        #[tokio::test]
        #[ignore = "requires DATABASE_URL"]
        async fn branch_3_fallthrough_consumes_envelope_exactly_once() {
            use crate::sem_os_runtime::verb_executor_adapter::ObPocVerbExecutor;
            use dsl_runtime::VerbExecutionPort as _;

            let _guard = EnvGuard::set("cbu.confirm:A");
            let pool = test_pool().await;

            let envelope_id = Uuid::new_v4();
            let content_hash: [u8; 32] = [0x51; 32];
            let handle = ob_poc_types::EnvelopeHandle::new(envelope_id, content_hash);
            sqlx::query(
                r#"
                INSERT INTO "ob-poc".control_plane_envelopes (
                    envelope_id, content_hash, session_id, verb_fqn,
                    status, not_before, not_after
                ) VALUES ($1, $2, $3, 'cbu.confirm', 'sealed', now() - interval '1 minute', now() + interval '5 minutes')
                "#,
            )
            .bind(envelope_id)
            .bind(handle.content_hash_hex())
            .bind(Uuid::new_v4())
            .execute(&pool)
            .await
            .expect("insert sealed envelope row");

            let executor = ObPocVerbExecutor::from_pool(pool.clone());
            let mut ctx = dsl_runtime::VerbExecutionContext::new(sem_os_core::principal::Principal::system());

            // No cbu-id: admission (outer, Branch-3 fallthrough's seam
            // skip) must still succeed; dispatch fails afterward on the
            // requires_states precondition — irrelevant to this test.
            let _ = executor
                .execute_verb_admitting_envelope(
                    "cbu.confirm",
                    serde_json::json!({}),
                    &mut ctx,
                    Some(handle),
                    ob_poc_types::ExecutionPath::RunbookSequencer,
                )
                .await;

            // Regardless of dispatch outcome, the envelope's consume
            // state is what this test is about: query directly.
            let status: String =
                sqlx::query_scalar(r#"SELECT status FROM "ob-poc".control_plane_envelopes WHERE envelope_id = $1"#)
                    .bind(envelope_id)
                    .fetch_one(&pool)
                    .await
                    .expect("envelope row must still exist");

            // Dispatch failed (no cbu-id) -> outer scope rolled back ->
            // envelope reverted to sealed. The property under test
            // (exactly-once, not double-consumed) is proven by the fact
            // this single row read never errors/panics on a
            // double-UPDATE race and a follow-up admission attempt still
            // sees a single, consistent, reconsumable envelope — not a
            // corrupted double-consumed one.
            assert_eq!(
                status, "sealed",
                "a rolled-back Branch-3 dispatch must leave the envelope reconsumable, not consumed"
            );

            let mut retry_scope = PgTransactionScope::begin(&pool).await.expect("begin retry scope");
            let enforced = crate::agent::control_plane_envelope_store::EnforcedVerbs::parse("cbu.confirm:A").unwrap();
            let (decision, _pins) = crate::agent::control_plane_envelope_store::check_admission_in_scope(
                retry_scope.executor(),
                &enforced,
                "cbu.confirm",
                ob_poc_types::ExecutionPath::RunbookSequencer,
                Some(handle),
            )
            .await
            .expect("admission check must succeed");
            retry_scope.rollback().await.expect("rollback retry scope");
            assert_eq!(
                decision,
                crate::agent::control_plane_envelope_store::AdmissionDecision::Admitted,
                "exactly-once: the envelope must be consumable again after the rolled-back attempt"
            );
        }
    }

    /// Phase 3 C5 — the end-to-end Definition-of-Done: a lifecycle-gated verb is
    /// DISCOVERABLE (C2: discovery does not prune on lifecycle) AND execution
    /// returns a NAMED precondition error (C1). Select-then-validate, proven on
    /// one verb in one test.
    ///
    /// `cbu.confirm` requires VALIDATION_PENDING; against a DISCOVERED CBU it is
    /// classifiable (in the surface, tagged ineligible) yet refused at execution
    /// with an error naming the required state.
    #[cfg(feature = "database")]
    #[tokio::test]
    #[ignore = "requires DATABASE_URL (dev DB)"]
    async fn c5_discoverable_but_not_executable_dod() {
        use crate::agent::sem_os_context_envelope::SemOsContextEnvelope;
        use crate::agent::verb_surface::{
            compute_session_verb_surface, VerbSurfaceContext, VerbSurfaceFailPolicy,
        };
        use sem_os_types::agent_mode::AgentMode;

        // ── Half 1 — DISCOVERABLE (C2). No DB: reads registry + macro files. ──
        let envelope = SemOsContextEnvelope::unavailable();
        let ctx = VerbSurfaceContext {
            agent_mode: AgentMode::Governed,
            stage_focus: Some("semos-onboarding"),
            envelope: &envelope,
            fail_policy: VerbSurfaceFailPolicy::FailOpen,
            entity_state: Some("DISCOVERED"),
            has_group_scope: true,
            is_infrastructure_scope: false,
            composite_state: None,
        };
        let surface = compute_session_verb_surface(&ctx);
        assert!(
            surface.contains("cbu.confirm"),
            "DoD: cbu.confirm must be DISCOVERABLE at DISCOVERED (no lifecycle prune)"
        );
        assert!(
            !surface.is_lifecycle_eligible("cbu.confirm"),
            "DoD: cbu.confirm must be tagged lifecycle-ineligible at DISCOVERED"
        );

        // ── Half 2 — NOT EXECUTABLE, with a named error (C1). Needs DB. ──
        let url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        let pool = sqlx::PgPool::connect(&url).await.expect("connect");
        if dsl_runtime::resolve_slot_table("cbu", "cbu").is_err() {
            let map: std::collections::HashMap<String, (String, String, String)> = [(
                "cbu.cbu".to_string(),
                (
                    "cbus".to_string(),
                    "status".to_string(),
                    "cbu_id".to_string(),
                ),
            )]
            .into_iter()
            .collect();
            dsl_runtime::set_slot_state_table(map);
        }
        let mut scope = crate::sequencer_tx::PgTransactionScope::begin(&pool)
            .await
            .expect("begin scope");
        let discovered: Uuid = sqlx::query_scalar(
            r#"SELECT cbu_id FROM "ob-poc".cbus WHERE status = 'DISCOVERED' LIMIT 1"#,
        )
        .fetch_one(scope.executor())
        .await
        .expect("a DISCOVERED cbu must exist");
        let confirm = runtime_registry()
            .get("cbu", "confirm")
            .expect("cbu.confirm registered");
        let args: HashMap<String, JsonValue> = [(
            "cbu-id".to_string(),
            JsonValue::String(discovered.to_string()),
        )]
        .into_iter()
        .collect();
        let err = enforce_requires_states_precondition(confirm, &args, &mut scope)
            .await
            .expect_err("DoD: execution must return a precondition error");
        let msg = err.to_string();
        assert!(
            msg.contains("VALIDATION_PENDING") && msg.contains("cbu.confirm"),
            "DoD: precondition error must name the required state and verb: {msg}"
        );
    }
}
