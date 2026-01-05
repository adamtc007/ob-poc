//! DSL Executor - YAML-driven execution engine for DSL v2
//!
//! This module implements the DslExecutor that processes parsed DSL programs
//! and executes them against the database using YAML-driven verb definitions.
//!
//! The executor routes verbs through:
//! - GenericCrudExecutor for CRUD operations (defined in verbs.yaml)
//! - CustomOperationRegistry for plugins (complex logic, external APIs)

#[cfg(feature = "database")]
use anyhow::{anyhow, bail, Result};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use uuid::Uuid;

// Import ViewState for the pending_view_state field
// This enables view operations to communicate ViewState back to the session layer
use crate::session::ViewState;

#[cfg(feature = "database")]
use super::ast::{AstNode, Literal, VerbCall};
#[cfg(feature = "database")]
use super::compiler::compile_to_ops;
#[cfg(feature = "database")]
use super::custom_ops::CustomOperationRegistry;
#[cfg(feature = "database")]
use super::dag::{build_execution_plan, describe_plan};
#[cfg(feature = "database")]
use super::domain_context::DomainContext;
#[cfg(feature = "database")]
use super::generic_executor::{GenericCrudExecutor, GenericExecutionResult};
#[cfg(feature = "database")]
use super::ops::{EntityKey, Op};
#[cfg(feature = "database")]
use super::runtime_registry::{runtime_registry, RuntimeBehavior};
#[cfg(feature = "database")]
use super::submission::{DslSubmission, SubmissionError, SubmissionLimits};

#[cfg(feature = "database")]
use sqlx::PgPool;

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
    EntityQuery(super::custom_ops::entity_query::EntityQueryResult),
    /// Template invocation result (template.invoke verb)
    TemplateInvoked(super::custom_ops::template_ops::TemplateInvokeResult),
    /// Template batch execution result (template.batch verb)
    TemplateBatch(super::custom_ops::template_ops::TemplateBatchResult),
    /// Batch control operation result (batch.pause, batch.resume, etc.)
    BatchControl(super::custom_ops::batch_control_ops::BatchControlResult),
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
    /// because CustomOperation only receives ExecutionContext, not UnifiedSessionContext.
    pub pending_view_state: Option<ViewState>,
    /// Source attribution for audit trail
    ///
    /// Tracks where the execution originated (api, cli, mcp, etc.),
    /// correlation ID for distributed tracing, and actor information.
    pub source_attribution: super::idempotency::SourceAttribution,
    /// Session ID for view state audit linkage
    pub session_id: Option<Uuid>,
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
            source_attribution: super::idempotency::SourceAttribution::default(),
            session_id: None,
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
            // Inherit source attribution for audit trail consistency
            source_attribution: self.source_attribution.clone(),
            // Inherit session ID for view state linkage
            session_id: self.session_id,
        }
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
    #[cfg(feature = "database")]
    custom_ops: CustomOperationRegistry,
    #[cfg(feature = "database")]
    generic_executor: GenericCrudExecutor,
    #[cfg(feature = "database")]
    idempotency: super::idempotency::IdempotencyManager,
    #[cfg(feature = "database")]
    verb_hash_lookup: crate::session::verb_hash_lookup::VerbHashLookupService,
}

impl DslExecutor {
    /// Create a new executor with a database pool
    #[cfg(feature = "database")]
    pub fn new(pool: PgPool) -> Self {
        Self {
            generic_executor: GenericCrudExecutor::new(pool.clone()),
            idempotency: super::idempotency::IdempotencyManager::new(pool.clone()),
            verb_hash_lookup: crate::session::verb_hash_lookup::VerbHashLookupService::new(
                pool.clone(),
            ),
            pool,
            custom_ops: CustomOperationRegistry::new(),
        }
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
    #[cfg(feature = "database")]
    pub async fn execute_verb(
        &self,
        vc: &VerbCall,
        ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        tracing::debug!("execute_verb: ENTER {}.{}", vc.domain, vc.verb);

        // Look up verb in runtime registry (loaded from YAML)
        let runtime_verb = runtime_registry()
            .get(&vc.domain, &vc.verb)
            .ok_or_else(|| anyhow!("Unknown verb: {}.{}", vc.domain, vc.verb))?;
        tracing::debug!(
            "execute_verb: found verb, behavior={:?}",
            runtime_verb.behavior
        );

        // Check if this is a plugin (custom operation)
        if let RuntimeBehavior::Plugin(_handler) = &runtime_verb.behavior {
            tracing::debug!("execute_verb: routing to PLUGIN");
            // Dispatch to custom operations handler
            if let Some(op) = self.custom_ops.get(&vc.domain, &vc.verb) {
                let result = op.execute(vc, ctx, &self.pool).await;
                tracing::debug!("execute_verb: plugin returned {:?}", result.is_ok());
                return result;
            }
            return Err(anyhow!(
                "Plugin {}.{} has no handler implementation",
                vc.domain,
                vc.verb
            ));
        }
        tracing::debug!("execute_verb: routing to GENERIC executor");

        // Convert VerbCall arguments to JSON for generic executor
        let json_args = Self::verbcall_args_to_json(&vc.arguments, ctx)?;
        tracing::debug!(
            "execute_verb: json_args keys={:?}",
            json_args.keys().collect::<Vec<_>>()
        );

        // Execute via generic executor
        let result = self
            .generic_executor
            .execute(runtime_verb, &json_args)
            .await?;
        tracing::debug!("execute_verb: generic executor returned {:?}", result);

        // Handle symbol capture
        if runtime_verb.returns.capture {
            if let GenericExecutionResult::Uuid(uuid) = &result {
                if let Some(name) = &runtime_verb.returns.name {
                    ctx.bind(name, *uuid);
                }
            }
        }

        tracing::debug!("execute_verb: EXIT success");
        Ok(result.to_legacy())
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
            AstNode::Literal(lit) => Self::literal_to_json(lit),
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
    async fn execute_verb_in_tx(
        &self,
        vc: &VerbCall,
        ctx: &mut ExecutionContext,
        _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    ) -> Result<ExecutionResult> {
        // For now, delegate to execute_verb which uses the pool
        // TODO: Pass transaction through to generic_executor for true transactional execution
        self.execute_verb(vc, ctx).await
    }
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
                        value: AstNode::Literal(Literal::String(id.to_string())),
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

            // Execute the verb call
            tracing::debug!("execute_plan: calling execute_verb...");
            let result = self.execute_verb(&vc, ctx).await?;
            tracing::debug!("execute_plan: execute_verb returned {:?}", result);

            // Trace the result
            tracing::debug!(
                step = step_index,
                verb = %format!("{}.{}", &vc.domain, &vc.verb),
                result = ?result,
                "DSL step completed"
            );

            // Record in idempotency table if enabled (atomically with view state if present)
            if ctx.idempotency_enabled {
                // Look up verb hash for audit trail
                let verb_hash = self
                    .verb_hash_lookup
                    .get_verb_hash(&verb_name)
                    .await
                    .ok()
                    .flatten();

                // Use atomic recording that commits idempotency + view state in single transaction
                // This ensures consistency: either both are recorded or neither is
                let _atomic_result = self
                    .idempotency
                    .record_with_view_state(
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
                        // Bind single Record to json_bindings
                        ctx.bind_json(binding_name, record.clone());
                    }
                    _ => {}
                }
            }

            results.push(result);
        }

        Ok(results)
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
        let registry = super::runtime_registry();
        let enrichment_result = super::enrich_program(raw_program, registry);
        let program = enrichment_result.program;

        // Note: EntityRef resolution happens during execution via GenericCrudExecutor
        // which calls resolve_lookup for args with lookup config

        let plan = super::execution_plan::compile(&program)
            .map_err(|e| anyhow!("Compile error: {}", e))?;

        self.execute_plan(&plan, ctx).await
    }
}

// ============================================================================
// DAG-based Execution (Op primitives)
// ============================================================================

/// Result of DAG-based execution
#[cfg(feature = "database")]
#[derive(Debug)]
pub struct DagExecutionResult {
    /// Results indexed by source statement
    pub results: Vec<OpExecutionResult>,
    /// Symbol table: binding name → UUID
    pub symbols: HashMap<String, Uuid>,
    /// Plan description (for dry-run output)
    pub plan_description: String,
}

/// Result of executing a single Op
#[cfg(feature = "database")]
#[derive(Debug, Clone)]
pub struct OpExecutionResult {
    /// Source statement index
    pub source_stmt: usize,
    /// Op description
    pub description: String,
    /// Result UUID if applicable
    pub result_id: Option<Uuid>,
    /// Whether this was a dry-run (not actually executed)
    pub dry_run: bool,
}

#[cfg(feature = "database")]
impl DslExecutor {
    /// Execute DSL using the DAG-based Op pipeline
    ///
    /// This method:
    /// 1. Parses source to AST
    /// 2. Compiles AST to Ops via `compile_to_ops()`
    /// 3. Builds DAG via `build_execution_plan()` (topological sort)
    /// 4. Executes Ops in dependency order
    ///
    /// If `dry_run` is true, returns the execution plan without executing.
    ///
    /// # Example
    /// ```ignore
    /// let result = executor.execute_with_dag(source, &mut ctx, false).await?;
    /// for r in &result.results {
    ///     println!("{}: {:?}", r.description, r.result_id);
    /// }
    /// ```
    pub async fn execute_with_dag(
        &self,
        source: &str,
        ctx: &mut ExecutionContext,
        dry_run: bool,
    ) -> Result<DagExecutionResult> {
        // Step 1: Parse
        let program =
            super::parser::parse_program(source).map_err(|e| anyhow!("Parse error: {}", e))?;

        // Step 2: Compile to Ops
        let compiled = compile_to_ops(&program);
        if !compiled.is_ok() {
            let errors: Vec<String> = compiled.errors.iter().map(|e| e.to_string()).collect();
            bail!("Compilation errors:\n{}", errors.join("\n"));
        }

        // Step 3: Build DAG (topological sort)
        let dag_plan =
            build_execution_plan(compiled.ops).map_err(|e| anyhow!("DAG cycle error: {}", e))?;

        let plan_description = describe_plan(&dag_plan);

        // Step 4: Execute (or dry-run)
        let mut results = Vec::with_capacity(dag_plan.ops.len());
        let mut symbols: HashMap<String, Uuid> = HashMap::new();

        if dry_run {
            // Dry-run: just describe what would happen
            for op in &dag_plan.ops {
                results.push(OpExecutionResult {
                    source_stmt: op.source_stmt(),
                    description: op.describe(),
                    result_id: None,
                    dry_run: true,
                });
            }
        } else {
            // Execute each Op in topological order
            for op in &dag_plan.ops {
                let result = self.execute_op(op, ctx, &symbols).await?;

                // Capture binding if present
                if let Some(binding) = op.binding() {
                    if let Some(uuid) = result.result_id {
                        ctx.bind(binding, uuid);
                        symbols.insert(binding.to_string(), uuid);
                    }
                }

                results.push(result);
            }
        }

        Ok(DagExecutionResult {
            results,
            symbols,
            plan_description,
        })
    }

    /// Execute a single Op by converting it to a VerbCall and routing through execute_verb
    async fn execute_op(
        &self,
        op: &Op,
        ctx: &mut ExecutionContext,
        symbols: &HashMap<String, Uuid>,
    ) -> Result<OpExecutionResult> {
        let source_stmt = op.source_stmt();
        let description = op.describe();

        // Convert Op to VerbCall and execute
        let result_id = match op {
            Op::EnsureEntity {
                entity_type, attrs, ..
            } => {
                // Map entity_type to domain.verb
                let (domain, verb) = match entity_type.as_str() {
                    "cbu" => ("cbu", "ensure"),
                    "proper_person" => ("entity", "create-proper-person"),
                    "limited_company" => ("entity", "create-limited-company"),
                    "partnership" => ("entity", "create-partnership-limited"),
                    "trust" => ("entity", "create-trust-discretionary"),
                    _ => bail!("Unknown entity type for execution: {}", entity_type),
                };

                let vc = self.build_verb_call(domain, verb, attrs)?;
                let result = self.execute_verb(&vc, ctx).await?;

                match result {
                    ExecutionResult::Uuid(id) => Some(id),
                    _ => None,
                }
            }

            Op::LinkRole {
                cbu,
                entity,
                role,
                ownership_percentage,
                ..
            } => {
                let mut attrs = HashMap::new();
                attrs.insert("cbu-id".to_string(), self.resolve_key(cbu, symbols)?);
                attrs.insert("entity-id".to_string(), self.resolve_key(entity, symbols)?);
                attrs.insert("role".to_string(), serde_json::json!(role));
                if let Some(pct) = ownership_percentage {
                    attrs.insert(
                        "ownership-percentage".to_string(),
                        serde_json::json!(pct.to_string()),
                    );
                }

                let vc = self.build_verb_call("cbu", "assign-role", &attrs)?;
                self.execute_verb(&vc, ctx).await?;
                None
            }

            Op::UnlinkRole {
                cbu, entity, role, ..
            } => {
                let mut attrs = HashMap::new();
                attrs.insert("cbu-id".to_string(), self.resolve_key(cbu, symbols)?);
                attrs.insert("entity-id".to_string(), self.resolve_key(entity, symbols)?);
                attrs.insert("role".to_string(), serde_json::json!(role));

                let vc = self.build_verb_call("cbu", "remove-role", &attrs)?;
                self.execute_verb(&vc, ctx).await?;
                None
            }

            Op::AddOwnership {
                owner,
                owned,
                percentage,
                ownership_type,
                ..
            } => {
                let mut attrs = HashMap::new();
                attrs.insert(
                    "owner-entity-id".to_string(),
                    self.resolve_key(owner, symbols)?,
                );
                attrs.insert(
                    "owned-entity-id".to_string(),
                    self.resolve_key(owned, symbols)?,
                );
                attrs.insert(
                    "percentage".to_string(),
                    serde_json::json!(percentage.to_string()),
                );
                attrs.insert(
                    "ownership-type".to_string(),
                    serde_json::json!(ownership_type),
                );

                let vc = self.build_verb_call("ubo", "add-ownership", &attrs)?;
                self.execute_verb(&vc, ctx).await?;
                None
            }

            Op::RegisterUBO {
                cbu,
                subject,
                ubo_person,
                qualifying_reason,
                ownership_percentage,
                ..
            } => {
                let mut attrs = HashMap::new();
                attrs.insert("cbu-id".to_string(), self.resolve_key(cbu, symbols)?);
                attrs.insert(
                    "subject-entity-id".to_string(),
                    self.resolve_key(subject, symbols)?,
                );
                attrs.insert(
                    "ubo-person-id".to_string(),
                    self.resolve_key(ubo_person, symbols)?,
                );
                attrs.insert(
                    "qualifying-reason".to_string(),
                    serde_json::json!(qualifying_reason),
                );
                if let Some(pct) = ownership_percentage {
                    attrs.insert(
                        "ownership-percentage".to_string(),
                        serde_json::json!(pct.to_string()),
                    );
                }

                let vc = self.build_verb_call("ubo", "register-ubo", &attrs)?;
                self.execute_verb(&vc, ctx).await?;
                None
            }

            Op::CreateCase { cbu, case_type, .. } => {
                let mut attrs = HashMap::new();
                attrs.insert("cbu-id".to_string(), self.resolve_key(cbu, symbols)?);
                attrs.insert("case-type".to_string(), serde_json::json!(case_type));

                let vc = self.build_verb_call("kyc-case", "create", &attrs)?;
                let result = self.execute_verb(&vc, ctx).await?;

                match result {
                    ExecutionResult::Uuid(id) => Some(id),
                    _ => None,
                }
            }

            Op::UpdateCaseStatus { case, status, .. } => {
                let mut attrs = HashMap::new();
                attrs.insert("case-id".to_string(), self.resolve_key(case, symbols)?);
                attrs.insert("status".to_string(), serde_json::json!(status));

                let vc = self.build_verb_call("kyc-case", "update-status", &attrs)?;
                self.execute_verb(&vc, ctx).await?;
                None
            }

            Op::CreateWorkstream { case, entity, .. } => {
                let mut attrs = HashMap::new();
                attrs.insert("case-id".to_string(), self.resolve_key(case, symbols)?);
                attrs.insert("entity-id".to_string(), self.resolve_key(entity, symbols)?);

                let vc = self.build_verb_call("entity-workstream", "create", &attrs)?;
                let result = self.execute_verb(&vc, ctx).await?;

                match result {
                    ExecutionResult::Uuid(id) => Some(id),
                    _ => None,
                }
            }

            Op::RunScreening {
                workstream,
                screening_type,
                ..
            } => {
                let mut attrs = HashMap::new();
                attrs.insert(
                    "workstream-id".to_string(),
                    self.resolve_key(workstream, symbols)?,
                );
                attrs.insert(
                    "screening-type".to_string(),
                    serde_json::json!(screening_type),
                );

                let vc = self.build_verb_call("case-screening", "run", &attrs)?;
                self.execute_verb(&vc, ctx).await?;
                None
            }

            Op::AddUniverse {
                cbu,
                instrument_class,
                market,
                currencies,
                settlement_types,
                ..
            } => {
                let mut attrs = HashMap::new();
                attrs.insert("cbu-id".to_string(), self.resolve_key(cbu, symbols)?);
                attrs.insert(
                    "instrument-class".to_string(),
                    serde_json::json!(instrument_class),
                );
                if let Some(m) = market {
                    attrs.insert("market".to_string(), serde_json::json!(m));
                }
                attrs.insert("currencies".to_string(), serde_json::json!(currencies));
                attrs.insert(
                    "settlement-types".to_string(),
                    serde_json::json!(settlement_types),
                );

                let vc = self.build_verb_call("cbu-custody", "add-universe", &attrs)?;
                self.execute_verb(&vc, ctx).await?;
                None
            }

            Op::CreateSSI {
                cbu,
                name,
                ssi_type,
                attrs: ssi_attrs,
                ..
            } => {
                let mut attrs = HashMap::new();
                attrs.insert("cbu-id".to_string(), self.resolve_key(cbu, symbols)?);
                attrs.insert("name".to_string(), serde_json::json!(name));
                attrs.insert("type".to_string(), serde_json::json!(ssi_type));
                // Merge SSI-specific attrs
                for (k, v) in ssi_attrs {
                    attrs.insert(k.clone(), v.clone());
                }

                let vc = self.build_verb_call("cbu-custody", "create-ssi", &attrs)?;
                let result = self.execute_verb(&vc, ctx).await?;

                match result {
                    ExecutionResult::Uuid(id) => Some(id),
                    _ => None,
                }
            }

            Op::AddBookingRule {
                cbu,
                ssi,
                name,
                priority,
                criteria,
                ..
            } => {
                let mut attrs = HashMap::new();
                attrs.insert("cbu-id".to_string(), self.resolve_key(cbu, symbols)?);
                attrs.insert("ssi-id".to_string(), self.resolve_key(ssi, symbols)?);
                attrs.insert("name".to_string(), serde_json::json!(name));
                attrs.insert("priority".to_string(), serde_json::json!(priority));
                // Merge criteria
                for (k, v) in criteria {
                    attrs.insert(k.clone(), v.clone());
                }

                let vc = self.build_verb_call("cbu-custody", "add-booking-rule", &attrs)?;
                self.execute_verb(&vc, ctx).await?;
                None
            }

            Op::UpsertDoc { key, cbu, .. } => {
                let mut attrs = HashMap::new();
                attrs.insert("doc-type".to_string(), serde_json::json!(&key.doc_type));
                attrs.insert("title".to_string(), serde_json::json!(&key.key));
                if let Some(c) = cbu {
                    attrs.insert("cbu-id".to_string(), self.resolve_key(c, symbols)?);
                }

                let vc = self.build_verb_call("document", "catalog", &attrs)?;
                let result = self.execute_verb(&vc, ctx).await?;

                match result {
                    ExecutionResult::Uuid(id) => Some(id),
                    _ => None,
                }
            }

            Op::AttachEvidence {
                cbu,
                evidence_type,
                attestation_ref,
                ..
            } => {
                let mut attrs = HashMap::new();
                attrs.insert("cbu-id".to_string(), self.resolve_key(cbu, symbols)?);
                attrs.insert(
                    "evidence-type".to_string(),
                    serde_json::json!(evidence_type),
                );
                if let Some(a) = attestation_ref {
                    attrs.insert("attestation-ref".to_string(), serde_json::json!(a));
                }

                let vc = self.build_verb_call("cbu", "attach-evidence", &attrs)?;
                self.execute_verb(&vc, ctx).await?;
                None
            }

            Op::SetFK { .. } => {
                // SetFK is handled by the entity creation - FKs are set via attrs
                // This is a placeholder for future two-phase execution
                tracing::debug!("SetFK op - skipping (handled during entity creation)");
                None
            }

            Op::Materialize { .. } => {
                // Materialize is a custom operation - skip for now
                tracing::warn!("Materialize op not yet implemented in DAG executor");
                None
            }

            Op::RequireRef { .. } => {
                // RequireRef is validation-only, no execution needed
                None
            }

            Op::GenericCrud { verb, args, .. } => {
                // GenericCrud ops are executed via the generic executor
                // Parse verb into domain.verb format
                let parts: Vec<&str> = verb.split('.').collect();
                if parts.len() != 2 {
                    bail!("Invalid verb format for GenericCrud: {}", verb);
                }
                let (domain, verb_name) = (parts[0], parts[1]);

                // Convert args HashMap to the format expected by build_verb_call
                let attrs: HashMap<String, serde_json::Value> =
                    args.iter().map(|(k, v)| (k.clone(), v.clone())).collect();

                let vc = self.build_verb_call(domain, verb_name, &attrs)?;
                let result = self.execute_verb(&vc, ctx).await?;

                match result {
                    ExecutionResult::Uuid(id) => Some(id),
                    _ => None,
                }
            }
        };

        Ok(OpExecutionResult {
            source_stmt,
            description,
            result_id,
            dry_run: false,
        })
    }

    /// Build a VerbCall from domain, verb, and attributes
    fn build_verb_call(
        &self,
        domain: &str,
        verb: &str,
        attrs: &HashMap<String, JsonValue>,
    ) -> Result<VerbCall> {
        let mut arguments = Vec::new();

        for (key, value) in attrs {
            let ast_value = self.json_to_ast(value)?;
            arguments.push(super::ast::Argument {
                key: key.clone(),
                value: ast_value,
                span: super::ast::Span::default(),
            });
        }

        Ok(VerbCall {
            domain: domain.to_string(),
            verb: verb.to_string(),
            arguments,
            binding: None,
            span: super::ast::Span::default(),
        })
    }

    /// Convert JSON value to AST node
    #[allow(clippy::only_used_in_recursion)]
    fn json_to_ast(&self, value: &JsonValue) -> Result<AstNode> {
        match value {
            JsonValue::String(s) => Ok(AstNode::Literal(Literal::String(s.clone()))),
            JsonValue::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Ok(AstNode::Literal(Literal::Integer(i)))
                } else if let Some(f) = n.as_f64() {
                    Ok(AstNode::Literal(Literal::Decimal(
                        rust_decimal::Decimal::from_f64_retain(f)
                            .ok_or_else(|| anyhow!("Invalid decimal: {}", f))?,
                    )))
                } else {
                    bail!("Invalid number: {}", n)
                }
            }
            JsonValue::Bool(b) => Ok(AstNode::Literal(Literal::Boolean(*b))),
            JsonValue::Null => Ok(AstNode::Literal(Literal::Null)),
            JsonValue::Array(arr) => {
                let items: Result<Vec<AstNode>> = arr.iter().map(|v| self.json_to_ast(v)).collect();
                Ok(AstNode::List {
                    items: items?,
                    span: super::ast::Span::default(),
                })
            }
            JsonValue::Object(obj) => {
                let mut entries = Vec::new();
                for (k, v) in obj {
                    entries.push((k.clone(), self.json_to_ast(v)?));
                }
                Ok(AstNode::Map {
                    entries,
                    span: super::ast::Span::default(),
                })
            }
        }
    }

    /// Resolve an EntityKey to a JSON value (UUID string or name)
    fn resolve_key(&self, key: &EntityKey, symbols: &HashMap<String, Uuid>) -> Result<JsonValue> {
        // Check if key refers to a symbol
        if key.entity_type == "symbol" {
            if let Some(uuid) = symbols.get(&key.key) {
                return Ok(JsonValue::String(uuid.to_string()));
            }
            bail!("Unresolved symbol: @{}", key.key);
        }

        // Otherwise return the key value (will be resolved by GenericCrudExecutor)
        Ok(JsonValue::String(key.key.clone()))
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
}
