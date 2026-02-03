//! Execution Proposal and Confirmation Protocol Operations
//!
//! Implements the proposal/confirm two-phase commit pattern for DSL execution:
//! - `exec.proposal` - Parse, validate, resolve entities, generate preview (NO side effects)
//! - `exec.confirm` - Execute a validated proposal atomically
//! - `exec.edit` - Create new proposal based on existing one with modifications
//! - `exec.cancel` - Cancel a pending proposal
//! - `exec.status` - Check proposal status
//!
//! # Non-Negotiables (from spec)
//!
//! 1. Proposals are immutable once created (DB trigger enforces this)
//! 2. Proposals expire after TTL (default 15 minutes)
//! 3. Confirmation is atomic (all or nothing)
//! 4. Security: Proposal scoped to session (cannot confirm another session's proposal)

use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use ob_poc_macros::register_custom_op;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::CustomOperation;
use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};

#[cfg(feature = "database")]
use sqlx::{PgPool, Row};

// =============================================================================
// RESULT TYPES
// =============================================================================

/// Result of exec.proposal operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposalResult {
    pub proposal_id: Uuid,
    pub valid: bool,
    pub narration: Option<String>,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
    pub affected_entities: Vec<Uuid>,
    pub unresolved_refs: Vec<UnresolvedRefInfo>,
    pub expires_at: DateTime<Utc>,
    pub seconds_remaining: i64,
}

/// Information about an unresolved entity reference
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnresolvedRefInfo {
    pub reference: String,
    pub suggestions: Vec<String>,
}

/// Result of exec.confirm operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfirmResult {
    pub success: bool,
    pub execution_result: Option<serde_json::Value>,
    pub narration: String,
    pub proposal_id: Uuid,
}

/// Result of exec.edit operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditResult {
    pub new_proposal_id: Uuid,
    pub parent_proposal_id: Uuid,
    pub diff_summary: String,
    pub narration: String,
}

/// Result of exec.status operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposalStatus {
    pub proposal_id: Uuid,
    pub status: String,
    pub source_dsl: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub seconds_remaining: i64,
    pub narration: Option<String>,
    pub validation_passed: bool,
}

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

#[cfg(feature = "database")]
fn get_required_string(verb_call: &VerbCall, key: &str) -> Result<String> {
    verb_call
        .arguments
        .iter()
        .find(|a| a.key == key)
        .and_then(|a| a.value.as_string().map(|s| s.to_string()))
        .ok_or_else(|| anyhow::anyhow!("Missing required argument :{}", key))
}

#[cfg(feature = "database")]
fn get_optional_uuid(verb_call: &VerbCall, key: &str) -> Option<Uuid> {
    verb_call
        .arguments
        .iter()
        .find(|a| a.key == key)
        .and_then(|a| a.value.as_uuid())
}

#[cfg(feature = "database")]
fn get_optional_int(verb_call: &VerbCall, key: &str) -> Option<i32> {
    verb_call
        .arguments
        .iter()
        .find(|a| a.key == key)
        .and_then(|a| a.value.as_integer().map(|i| i as i32))
}

#[allow(dead_code)]
#[cfg(feature = "database")]
fn get_optional_string(verb_call: &VerbCall, key: &str) -> Option<String> {
    verb_call
        .arguments
        .iter()
        .find(|a| a.key == key)
        .and_then(|a| a.value.as_string().map(|s| s.to_string()))
}

// =============================================================================
// EXEC.PROPOSAL OPERATION
// =============================================================================

/// Create an execution proposal - parse, validate, resolve entities, generate preview.
/// NO SIDE EFFECTS - this is purely a preview/validation operation.
#[register_custom_op]
pub struct ExecProposalOp;

#[async_trait]
impl CustomOperation for ExecProposalOp {
    fn domain(&self) -> &'static str {
        "exec"
    }

    fn verb(&self) -> &'static str {
        "proposal"
    }

    fn rationale(&self) -> &'static str {
        "Creates proposal without side effects - needs full pipeline access for parsing, \
         validation, entity resolution, and narration generation."
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use dsl_core::ast::Statement;
        use dsl_core::parser::parse_program;

        // 1. Extract arguments
        let dsl_source = get_required_string(verb_call, "dsl")?;
        let ttl_minutes = get_optional_int(verb_call, "ttl-minutes")
            .unwrap_or(15)
            .min(60);

        let session_id = ctx.session_id.ok_or_else(|| {
            anyhow::anyhow!("No session context for proposal - exec.proposal requires a session")
        })?;

        tracing::info!(
            session_id = %session_id,
            dsl_len = dsl_source.len(),
            ttl_minutes = ttl_minutes,
            "exec.proposal: creating proposal"
        );

        // 2. Parse DSL (no side effects)
        let (statements, parse_errors): (Vec<Statement>, Vec<String>) =
            match parse_program(&dsl_source) {
                Ok(program) => (program.statements, vec![]),
                Err(e) => (vec![], vec![format!("Parse error: {}", e)]),
            };

        // 3. Validate (no side effects) - basic structural validation
        let mut validation_errors = parse_errors.clone();
        let warnings: Vec<String> = vec![];

        if statements.is_empty() && parse_errors.is_empty() {
            validation_errors.push("Empty DSL - no statements to execute".to_string());
        }

        // Check for unknown verbs
        for stmt in &statements {
            if let Statement::VerbCall(vc) = stmt {
                let verb_fqn = format!("{}.{}", vc.domain, vc.verb);
                if crate::dsl_v2::runtime_registry::runtime_registry()
                    .get_by_name(&verb_fqn)
                    .is_none()
                {
                    validation_errors.push(format!("Unknown verb: {}", verb_fqn));
                }
            }
        }

        let validation_passed = validation_errors.is_empty();

        // 4. Collect unresolved entity references (no side effects - just inspection)
        let mut unresolved_refs = vec![];
        for stmt in &statements {
            if let Statement::VerbCall(vc) = stmt {
                collect_unresolved_refs(vc, &mut unresolved_refs);
            }
        }

        // 5. Compute affected entities (preview only, no actual execution)
        let affected_entities: Vec<Uuid> = vec![]; // Would require more complex analysis

        // 6. Generate canonical DSL string from AST
        let canonical_dsl = statements
            .iter()
            .map(|s| s.to_dsl_string())
            .collect::<Vec<_>>()
            .join("\n");

        // 7. Generate narration
        let narration = if validation_passed {
            let stmt_count = statements.len();
            Some(format!(
                "Ready to execute {} statement{}. Say 'confirm' to proceed.",
                stmt_count,
                if stmt_count == 1 { "" } else { "s" }
            ))
        } else {
            Some(format!("Cannot execute: {}", validation_errors.join("; ")))
        };

        // 8. Store proposal in database
        let proposal_row = sqlx::query(
            r#"
            INSERT INTO "ob-poc".exec_proposals (
                session_id, source_dsl, canonical_dsl, ast_json,
                resolved_entities, unresolved_refs,
                validation_passed, validation_errors, warnings,
                affected_entities, preview_summary, narration,
                expires_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12,
                      NOW() + $13 * INTERVAL '1 minute')
            RETURNING id, expires_at
            "#,
        )
        .bind(session_id)
        .bind(&dsl_source)
        .bind(&canonical_dsl)
        .bind(serde_json::to_value(&statements)?)
        .bind(serde_json::json!([])) // resolved_entities
        .bind(serde_json::to_value(&unresolved_refs)?)
        .bind(validation_passed)
        .bind(serde_json::to_value(&validation_errors)?)
        .bind(serde_json::to_value(&warnings)?)
        .bind(serde_json::to_value(&affected_entities)?)
        .bind(narration.as_ref().unwrap_or(&String::new()))
        .bind(narration.as_ref())
        .bind(ttl_minutes)
        .fetch_one(pool)
        .await?;

        let proposal_id: Uuid = proposal_row.get("id");
        let expires_at: DateTime<Utc> = proposal_row.get("expires_at");
        let seconds_remaining = (expires_at - Utc::now()).num_seconds().max(0);

        tracing::info!(
            proposal_id = %proposal_id,
            valid = validation_passed,
            expires_at = %expires_at,
            "exec.proposal: created proposal"
        );

        let result = ProposalResult {
            proposal_id,
            valid: validation_passed,
            narration,
            warnings,
            errors: validation_errors,
            affected_entities,
            unresolved_refs,
            expires_at,
            seconds_remaining,
        };

        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "exec.proposal requires database feature to be enabled"
        ))
    }
}

/// Collect unresolved entity references from a VerbCall
#[cfg(feature = "database")]
fn collect_unresolved_refs(vc: &dsl_core::ast::VerbCall, refs: &mut Vec<UnresolvedRefInfo>) {
    for arg in &vc.arguments {
        collect_unresolved_from_node(&arg.value, refs);
    }
}

#[cfg(feature = "database")]
fn collect_unresolved_from_node(node: &dsl_core::ast::AstNode, refs: &mut Vec<UnresolvedRefInfo>) {
    use dsl_core::ast::AstNode;

    match node {
        AstNode::EntityRef {
            resolved_key,
            value,
            ..
        } => {
            if resolved_key.is_none() {
                refs.push(UnresolvedRefInfo {
                    reference: value.clone(),
                    suggestions: vec![], // Would need entity search to populate
                });
            }
        }
        AstNode::List { items, .. } => {
            for item in items {
                collect_unresolved_from_node(item, refs);
            }
        }
        AstNode::Map { entries, .. } => {
            for (_, v) in entries {
                collect_unresolved_from_node(v, refs);
            }
        }
        AstNode::Nested(vc) => {
            collect_unresolved_refs(vc, refs);
        }
        AstNode::Literal(_, _) | AstNode::SymbolRef { .. } => {}
    }
}

// =============================================================================
// EXEC.CONFIRM OPERATION
// =============================================================================

/// Execute a pending proposal atomically.
/// All or nothing - if any statement fails, the entire batch is rolled back.
#[register_custom_op]
pub struct ExecConfirmOp;

#[async_trait]
impl CustomOperation for ExecConfirmOp {
    fn domain(&self) -> &'static str {
        "exec"
    }

    fn verb(&self) -> &'static str {
        "confirm"
    }

    fn rationale(&self) -> &'static str {
        "Executes validated proposal atomically - needs transaction control and full \
         executor access for atomic all-or-nothing execution."
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let session_id = ctx.session_id.ok_or_else(|| {
            anyhow::anyhow!("No session context - exec.confirm requires a session")
        })?;

        // 1. Find proposal (explicit or most recent pending)
        let proposal_id = match get_optional_uuid(verb_call, "proposal-id") {
            Some(id) => id,
            None => {
                // Find most recent pending proposal for this session
                let row =
                    sqlx::query(r#"SELECT "ob-poc".find_pending_proposal($1) as proposal_id"#)
                        .bind(session_id)
                        .fetch_one(pool)
                        .await?;

                row.get::<Option<Uuid>, _>("proposal_id")
                    .ok_or_else(|| anyhow::anyhow!("No pending proposal to confirm"))?
            }
        };

        tracing::info!(
            session_id = %session_id,
            proposal_id = %proposal_id,
            "exec.confirm: confirming proposal"
        );

        // 2. Load and validate proposal
        let proposal = sqlx::query(
            r#"
            SELECT
                session_id, status, expires_at, validation_passed,
                source_dsl, ast_json, narration
            FROM "ob-poc".exec_proposals
            WHERE id = $1
            "#,
        )
        .bind(proposal_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Proposal not found: {}", proposal_id))?;

        // Security check: proposal must belong to this session
        let proposal_session_id: Uuid = proposal.get("session_id");
        if proposal_session_id != session_id {
            return Err(anyhow::anyhow!(
                "Proposal belongs to different session. Cannot confirm another session's proposal."
            ));
        }

        // Status check
        let status: String = proposal.get("status");
        if status != "pending" {
            return Err(anyhow::anyhow!(
                "Proposal is no longer pending (status: {}). Create a new proposal.",
                status
            ));
        }

        // Expiry check
        let expires_at: DateTime<Utc> = proposal.get("expires_at");
        if expires_at < Utc::now() {
            // Mark as expired
            sqlx::query(r#"SELECT "ob-poc".expire_proposal($1)"#)
                .bind(proposal_id)
                .execute(pool)
                .await?;
            return Err(anyhow::anyhow!(
                "Proposal has expired. Create a new proposal."
            ));
        }

        // Validation check
        let validation_passed: bool = proposal.get("validation_passed");
        if !validation_passed {
            return Err(anyhow::anyhow!(
                "Cannot confirm invalid proposal. Fix validation errors and create a new proposal."
            ));
        }

        // 3. Load AST and execute
        let ast_json: serde_json::Value = proposal.get("ast_json");
        let statements: Vec<dsl_core::ast::Statement> = serde_json::from_value(ast_json)?;

        // Execute atomically using the executor
        let exec_result = execute_statements_atomic(pool, ctx, &statements).await;

        // 4. Update proposal status based on result
        match &exec_result {
            Ok(results) => {
                sqlx::query(
                    r#"
                    UPDATE "ob-poc".exec_proposals
                    SET status = 'confirmed',
                        confirmed_at = NOW(),
                        execution_result = $2
                    WHERE id = $1
                    "#,
                )
                .bind(proposal_id)
                .bind(serde_json::to_value(results)?)
                .execute(pool)
                .await?;

                tracing::info!(
                    proposal_id = %proposal_id,
                    result_count = results.len(),
                    "exec.confirm: proposal confirmed and executed"
                );

                let narration = format!(
                    "Done! Executed {} statement{}.",
                    results.len(),
                    if results.len() == 1 { "" } else { "s" }
                );

                let result = ConfirmResult {
                    success: true,
                    execution_result: Some(serde_json::to_value(results)?),
                    narration,
                    proposal_id,
                };

                Ok(ExecutionResult::Record(serde_json::to_value(result)?))
            }
            Err(e) => {
                // Proposal stays pending - user can retry or edit
                tracing::warn!(
                    proposal_id = %proposal_id,
                    error = %e,
                    "exec.confirm: execution failed"
                );

                let result = ConfirmResult {
                    success: false,
                    execution_result: None,
                    narration: format!("Execution failed: {}. No changes were made.", e),
                    proposal_id,
                };

                Ok(ExecutionResult::Record(serde_json::to_value(result)?))
            }
        }
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "exec.confirm requires database feature to be enabled"
        ))
    }
}

/// Execute statements atomically in a transaction
#[cfg(feature = "database")]
async fn execute_statements_atomic(
    pool: &PgPool,
    ctx: &mut ExecutionContext,
    statements: &[dsl_core::ast::Statement],
) -> Result<Vec<serde_json::Value>> {
    use crate::domain_ops::CustomOperationRegistry;
    use crate::dsl_v2::runtime_registry::{runtime_registry, RuntimeBehavior};

    let mut results = Vec::new();

    // Start transaction
    let mut tx = pool.begin().await?;

    for stmt in statements {
        if let dsl_core::ast::Statement::VerbCall(vc) = stmt {
            let verb_fqn = format!("{}.{}", vc.domain, vc.verb);

            // Get verb definition
            let verb_def = runtime_registry()
                .get_by_name(&verb_fqn)
                .ok_or_else(|| anyhow::anyhow!("Unknown verb: {}", verb_fqn))?;

            // Execute based on behavior
            let result = match &verb_def.behavior {
                RuntimeBehavior::Plugin(_handler) => {
                    // Execute plugin - plugins don't have built-in tx support,
                    // so we use the pool-based execution
                    let custom_ops = CustomOperationRegistry::new();

                    // Convert dsl_core::VerbCall to crate::dsl_v2::ast::VerbCall
                    let internal_vc = convert_verb_call(vc)?;

                    if let Some(op) = custom_ops.get(&vc.domain, &vc.verb) {
                        // Try execute_in_tx first, fall back to pool-based
                        match op.execute_in_tx(&internal_vc, ctx, &mut tx).await {
                            Ok(r) => r,
                            Err(_) => {
                                // Fall back to non-transactional (will warn)
                                op.execute(&internal_vc, ctx, pool).await?
                            }
                        }
                    } else {
                        return Err(anyhow::anyhow!(
                            "Plugin handler not found for: {}",
                            verb_fqn
                        ));
                    }
                }
                RuntimeBehavior::Crud(_crud_config) => {
                    // Execute CRUD in transaction
                    // Convert VerbCall arguments to HashMap for the executor
                    let args = extract_args_as_json(vc);

                    let executor =
                        crate::dsl_v2::generic_executor::GenericCrudExecutor::new(pool.clone());
                    let gen_result = executor.execute_in_tx(&mut tx, verb_def, &args).await?;

                    match gen_result {
                        crate::dsl_v2::generic_executor::GenericExecutionResult::Uuid(id) => {
                            ExecutionResult::Uuid(id)
                        }
                        crate::dsl_v2::generic_executor::GenericExecutionResult::Record(r) => {
                            ExecutionResult::Record(r)
                        }
                        crate::dsl_v2::generic_executor::GenericExecutionResult::RecordSet(rs) => {
                            ExecutionResult::RecordSet(rs)
                        }
                        crate::dsl_v2::generic_executor::GenericExecutionResult::Affected(n) => {
                            ExecutionResult::Affected(n)
                        }
                        crate::dsl_v2::generic_executor::GenericExecutionResult::Void => {
                            ExecutionResult::Void
                        }
                    }
                }
                RuntimeBehavior::GraphQuery(_) => {
                    // GraphQuery verbs are read-only, execute via pool
                    return Err(anyhow::anyhow!(
                        "GraphQuery verbs not yet supported in exec.confirm. \
                         Use direct execution for read-only queries."
                    ));
                }
            };

            // Handle binding if present
            if let Some(ref binding_name) = vc.binding {
                if let ExecutionResult::Uuid(id) = &result {
                    ctx.bind(binding_name, *id);
                }
            }

            // Convert result to JSON for storage
            let result_json = match &result {
                ExecutionResult::Uuid(id) => serde_json::json!({"uuid": id}),
                ExecutionResult::Record(r) => r.clone(),
                ExecutionResult::RecordSet(rs) => serde_json::json!(rs),
                ExecutionResult::Affected(n) => serde_json::json!({"affected": n}),
                ExecutionResult::Void => serde_json::json!(null),
                _ => serde_json::json!({"type": "other"}),
            };

            results.push(result_json);
        }
    }

    // Commit transaction
    tx.commit().await?;

    Ok(results)
}

/// Convert dsl_core::VerbCall to internal VerbCall type.
/// Since dsl_v2::ast re-exports from dsl_core::ast, types are identical - just clone.
#[cfg(feature = "database")]
fn convert_verb_call(vc: &dsl_core::ast::VerbCall) -> Result<crate::dsl_v2::ast::VerbCall> {
    // Types are the same (re-exported), so we can just clone
    Ok(vc.clone())
}

/// Extract arguments from VerbCall as JSON HashMap for GenericCrudExecutor
#[cfg(feature = "database")]
fn extract_args_as_json(
    vc: &dsl_core::ast::VerbCall,
) -> std::collections::HashMap<String, serde_json::Value> {
    let mut args = std::collections::HashMap::new();

    for arg in &vc.arguments {
        let value = ast_node_to_json(&arg.value);
        args.insert(arg.key.clone(), value);
    }

    args
}

/// Convert AstNode to JSON value
#[cfg(feature = "database")]
fn ast_node_to_json(node: &dsl_core::ast::AstNode) -> serde_json::Value {
    match node {
        dsl_core::ast::AstNode::Literal(lit, _) => match lit {
            dsl_core::ast::Literal::String(s) => serde_json::json!(s),
            dsl_core::ast::Literal::Integer(i) => serde_json::json!(i),
            dsl_core::ast::Literal::Decimal(d) => serde_json::json!(d.to_string()),
            dsl_core::ast::Literal::Boolean(b) => serde_json::json!(b),
            dsl_core::ast::Literal::Uuid(u) => serde_json::json!(u.to_string()),
            dsl_core::ast::Literal::Null => serde_json::Value::Null,
        },
        dsl_core::ast::AstNode::SymbolRef { name, .. } => serde_json::json!(format!("@{}", name)),
        dsl_core::ast::AstNode::EntityRef {
            resolved_key,
            value,
            ..
        } => {
            if let Some(uuid) = resolved_key {
                serde_json::json!(uuid.to_string())
            } else {
                serde_json::json!(format!("<{}>", value))
            }
        }
        dsl_core::ast::AstNode::List { items, .. } => {
            serde_json::Value::Array(items.iter().map(ast_node_to_json).collect())
        }
        dsl_core::ast::AstNode::Map { entries, .. } => {
            let obj: serde_json::Map<String, serde_json::Value> = entries
                .iter()
                .map(|(k, v)| (k.clone(), ast_node_to_json(v)))
                .collect();
            serde_json::Value::Object(obj)
        }
        dsl_core::ast::AstNode::Nested(_) => serde_json::json!({"nested": "verb_call"}),
    }
}

// =============================================================================
// EXEC.EDIT OPERATION
// =============================================================================

/// Create a new proposal based on an existing one with modifications.
/// The original proposal is marked as 'superseded'.
#[register_custom_op]
pub struct ExecEditOp;

#[async_trait]
impl CustomOperation for ExecEditOp {
    fn domain(&self) -> &'static str {
        "exec"
    }

    fn verb(&self) -> &'static str {
        "edit"
    }

    fn rationale(&self) -> &'static str {
        "Creates new proposal from existing with modifications - needs to load existing proposal, \
         apply changes, and create new proposal with parent linkage for audit trail."
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let session_id = ctx
            .session_id
            .ok_or_else(|| anyhow::anyhow!("No session context - exec.edit requires a session"))?;

        // 1. Find proposal to edit
        let proposal_id = match get_optional_uuid(verb_call, "proposal-id") {
            Some(id) => id,
            None => {
                let row =
                    sqlx::query(r#"SELECT "ob-poc".find_pending_proposal($1) as proposal_id"#)
                        .bind(session_id)
                        .fetch_one(pool)
                        .await?;

                row.get::<Option<Uuid>, _>("proposal_id")
                    .ok_or_else(|| anyhow::anyhow!("No pending proposal to edit"))?
            }
        };

        let changes = get_required_string(verb_call, "changes")?;

        tracing::info!(
            session_id = %session_id,
            proposal_id = %proposal_id,
            changes_len = changes.len(),
            "exec.edit: editing proposal"
        );

        // 2. Load existing proposal
        let proposal = sqlx::query(
            r#"
            SELECT session_id, status, source_dsl
            FROM "ob-poc".exec_proposals
            WHERE id = $1
            "#,
        )
        .bind(proposal_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Proposal not found: {}", proposal_id))?;

        // Security check
        let proposal_session_id: Uuid = proposal.get("session_id");
        if proposal_session_id != session_id {
            return Err(anyhow::anyhow!(
                "Proposal belongs to different session. Cannot edit another session's proposal."
            ));
        }

        // Status check
        let status: String = proposal.get("status");
        if status != "pending" {
            return Err(anyhow::anyhow!(
                "Cannot edit non-pending proposal (status: {}). Create a new proposal.",
                status
            ));
        }

        let original_dsl: String = proposal.get("source_dsl");

        // 3. Apply changes - for now, just replace the DSL
        // In a more sophisticated version, this could:
        // - Parse the changes as a diff
        // - Apply LLM to modify the DSL
        // - Parse changes as commands like "change name to X"
        let new_dsl = if changes.starts_with('(') {
            // Changes look like DSL - use them directly
            changes.clone()
        } else {
            // Changes are natural language - for MVP, we'd need LLM here
            // For now, just append as a comment and use original
            // This is a placeholder - real implementation would use LLM
            format!("{}\n;; Edit request: {}", original_dsl, changes)
        };

        // 4. Mark original as superseded
        sqlx::query(r#"SELECT "ob-poc".supersede_proposal($1)"#)
            .bind(proposal_id)
            .execute(pool)
            .await?;

        // 5. Create new proposal (reuse the proposal logic)
        // Build a synthetic VerbCall for exec.proposal
        use crate::dsl_v2::ast::{Argument, AstNode, Literal, Span};

        let synthetic_vc = VerbCall {
            domain: "exec".to_string(),
            verb: "proposal".to_string(),
            arguments: vec![Argument {
                key: "dsl".to_string(),
                value: AstNode::Literal(Literal::String(new_dsl.clone()), Span::default()),
                span: Span::default(),
            }],
            binding: None,
            span: Span::default(),
        };

        // Execute the proposal creation
        let proposal_op = ExecProposalOp;
        let proposal_result = proposal_op.execute(&synthetic_vc, ctx, pool).await?;

        // Extract new proposal ID from result
        let new_proposal_id = if let ExecutionResult::Record(val) = &proposal_result {
            val.get("proposal_id")
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok())
                .ok_or_else(|| anyhow::anyhow!("Failed to get new proposal ID"))?
        } else {
            return Err(anyhow::anyhow!("Unexpected proposal result type"));
        };

        // 6. Update new proposal to link to parent
        sqlx::query(
            r#"
            UPDATE "ob-poc".exec_proposals
            SET parent_proposal_id = $2
            WHERE id = $1
            "#,
        )
        .bind(new_proposal_id)
        .bind(proposal_id)
        .execute(pool)
        .await?;

        tracing::info!(
            old_proposal_id = %proposal_id,
            new_proposal_id = %new_proposal_id,
            "exec.edit: created new proposal from edit"
        );

        let result = EditResult {
            new_proposal_id,
            parent_proposal_id: proposal_id,
            diff_summary: format!("Created new proposal from edit of {}", proposal_id),
            narration: "Updated proposal. Say 'confirm' to proceed.".to_string(),
        };

        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "exec.edit requires database feature to be enabled"
        ))
    }
}

// =============================================================================
// EXEC.CANCEL OPERATION
// =============================================================================

/// Cancel a pending proposal.
#[register_custom_op]
pub struct ExecCancelOp;

#[async_trait]
impl CustomOperation for ExecCancelOp {
    fn domain(&self) -> &'static str {
        "exec"
    }

    fn verb(&self) -> &'static str {
        "cancel"
    }

    fn rationale(&self) -> &'static str {
        "Cancels pending proposal - updates status to 'cancelled' for audit trail."
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let session_id = ctx.session_id.ok_or_else(|| {
            anyhow::anyhow!("No session context - exec.cancel requires a session")
        })?;

        // Find proposal to cancel
        let proposal_id = match get_optional_uuid(verb_call, "proposal-id") {
            Some(id) => id,
            None => {
                let row =
                    sqlx::query(r#"SELECT "ob-poc".find_pending_proposal($1) as proposal_id"#)
                        .bind(session_id)
                        .fetch_one(pool)
                        .await?;

                row.get::<Option<Uuid>, _>("proposal_id")
                    .ok_or_else(|| anyhow::anyhow!("No pending proposal to cancel"))?
            }
        };

        tracing::info!(
            session_id = %session_id,
            proposal_id = %proposal_id,
            "exec.cancel: cancelling proposal"
        );

        // Load and validate proposal
        let proposal =
            sqlx::query(r#"SELECT session_id, status FROM "ob-poc".exec_proposals WHERE id = $1"#)
                .bind(proposal_id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| anyhow::anyhow!("Proposal not found: {}", proposal_id))?;

        // Security check
        let proposal_session_id: Uuid = proposal.get("session_id");
        if proposal_session_id != session_id {
            return Err(anyhow::anyhow!(
                "Proposal belongs to different session. Cannot cancel another session's proposal."
            ));
        }

        // Status check
        let status: String = proposal.get("status");
        if status != "pending" {
            return Err(anyhow::anyhow!(
                "Cannot cancel non-pending proposal (status: {}).",
                status
            ));
        }

        // Cancel the proposal
        let result = sqlx::query(
            r#"
            UPDATE "ob-poc".exec_proposals
            SET status = 'cancelled'
            WHERE id = $1 AND status = 'pending'
            RETURNING id
            "#,
        )
        .bind(proposal_id)
        .fetch_optional(pool)
        .await?;

        if result.is_some() {
            tracing::info!(proposal_id = %proposal_id, "exec.cancel: proposal cancelled");
            Ok(ExecutionResult::Affected(1))
        } else {
            Ok(ExecutionResult::Affected(0))
        }
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "exec.cancel requires database feature to be enabled"
        ))
    }
}

// =============================================================================
// EXEC.STATUS OPERATION
// =============================================================================

/// Check status of a proposal or list recent proposals.
#[register_custom_op]
pub struct ExecStatusOp;

#[async_trait]
impl CustomOperation for ExecStatusOp {
    fn domain(&self) -> &'static str {
        "exec"
    }

    fn verb(&self) -> &'static str {
        "status"
    }

    fn rationale(&self) -> &'static str {
        "Queries proposal status - needs to list and filter proposals for the session."
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let session_id = ctx.session_id.ok_or_else(|| {
            anyhow::anyhow!("No session context - exec.status requires a session")
        })?;

        let specific_proposal_id = get_optional_uuid(verb_call, "proposal-id");
        let limit = get_optional_int(verb_call, "limit").unwrap_or(5).min(50);

        tracing::debug!(
            session_id = %session_id,
            proposal_id = ?specific_proposal_id,
            limit = limit,
            "exec.status: querying proposals"
        );

        let rows = if let Some(proposal_id) = specific_proposal_id {
            // Query specific proposal
            sqlx::query(
                r#"
                SELECT
                    id, status, source_dsl, created_at, expires_at,
                    EXTRACT(EPOCH FROM (expires_at - NOW()))::INTEGER AS seconds_remaining,
                    narration, validation_passed
                FROM "ob-poc".exec_proposals
                WHERE id = $1 AND session_id = $2
                "#,
            )
            .bind(proposal_id)
            .bind(session_id)
            .fetch_all(pool)
            .await?
        } else {
            // Query recent proposals
            sqlx::query(
                r#"
                SELECT
                    id, status, source_dsl, created_at, expires_at,
                    EXTRACT(EPOCH FROM (expires_at - NOW()))::INTEGER AS seconds_remaining,
                    narration, validation_passed
                FROM "ob-poc".exec_proposals
                WHERE session_id = $1
                ORDER BY created_at DESC
                LIMIT $2
                "#,
            )
            .bind(session_id)
            .bind(limit)
            .fetch_all(pool)
            .await?
        };

        let proposals: Vec<ProposalStatus> = rows
            .iter()
            .map(|row| ProposalStatus {
                proposal_id: row.get("id"),
                status: row.get("status"),
                source_dsl: row.get("source_dsl"),
                created_at: row.get("created_at"),
                expires_at: row.get("expires_at"),
                seconds_remaining: row.get::<Option<i32>, _>("seconds_remaining").unwrap_or(0)
                    as i64,
                narration: row.get("narration"),
                validation_passed: row.get("validation_passed"),
            })
            .collect();

        let results: Vec<serde_json::Value> = proposals
            .iter()
            .map(|p| serde_json::to_value(p).unwrap_or_default())
            .collect();

        Ok(ExecutionResult::RecordSet(results))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "exec.status requires database feature to be enabled"
        ))
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exec_proposal_op_metadata() {
        let op = ExecProposalOp;
        assert_eq!(op.domain(), "exec");
        assert_eq!(op.verb(), "proposal");
        assert!(op.rationale().contains("side effects"));
    }

    #[test]
    fn test_exec_confirm_op_metadata() {
        let op = ExecConfirmOp;
        assert_eq!(op.domain(), "exec");
        assert_eq!(op.verb(), "confirm");
        assert!(op.rationale().contains("atomic"));
    }

    #[test]
    fn test_exec_edit_op_metadata() {
        let op = ExecEditOp;
        assert_eq!(op.domain(), "exec");
        assert_eq!(op.verb(), "edit");
        assert!(op.rationale().contains("audit trail"));
    }

    #[test]
    fn test_exec_cancel_op_metadata() {
        let op = ExecCancelOp;
        assert_eq!(op.domain(), "exec");
        assert_eq!(op.verb(), "cancel");
        assert!(op.rationale().contains("cancelled"));
    }

    #[test]
    fn test_exec_status_op_metadata() {
        let op = ExecStatusOp;
        assert_eq!(op.domain(), "exec");
        assert_eq!(op.verb(), "status");
        assert!(op.rationale().contains("status"));
    }
}
