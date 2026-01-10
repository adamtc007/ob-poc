//! Agent control plugin operations.
//!
//! Controls the agent mode and loop execution:
//! - Start/pause/resume/stop agent loop
//! - Checkpoint confirmations for ambiguous decisions
//! - Status and history queries
//! - Threshold configuration
//!
//! Integration: UnifiedSessionContext, AgentController, AgentState

use anyhow::Result;
use async_trait::async_trait;
use serde_json::json;
use uuid::Uuid;

use super::CustomOperation;
use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};

#[cfg(feature = "database")]
use sqlx::PgPool;

// ============================================================================
// Helper Functions
// ============================================================================

/// Extract a required UUID argument from verb call
#[cfg(feature = "database")]
fn get_required_uuid(verb_call: &VerbCall, key: &str, ctx: &ExecutionContext) -> Result<Uuid> {
    let arg = verb_call
        .arguments
        .iter()
        .find(|a| a.key == key)
        .ok_or_else(|| anyhow::anyhow!("Missing required argument :{}", key))?;

    // Try as symbol reference first
    if let Some(ref_name) = arg.value.as_symbol() {
        let resolved = ctx
            .resolve(ref_name)
            .ok_or_else(|| anyhow::anyhow!("Unresolved reference @{}", ref_name))?;
        return Ok(resolved);
    }

    // Try as UUID directly
    if let Some(uuid_val) = arg.value.as_uuid() {
        return Ok(uuid_val);
    }

    // Try as string (may be UUID string)
    if let Some(str_val) = arg.value.as_string() {
        return Uuid::parse_str(str_val)
            .map_err(|e| anyhow::anyhow!("Invalid UUID for :{}: {}", key, e));
    }

    Err(anyhow::anyhow!(":{} must be a UUID or @reference", key))
}

/// Extract an optional UUID argument from verb call
#[cfg(feature = "database")]
fn get_optional_uuid(verb_call: &VerbCall, key: &str, ctx: &ExecutionContext) -> Option<Uuid> {
    get_required_uuid(verb_call, key, ctx).ok()
}

/// Extract an optional string argument from verb call
#[cfg(feature = "database")]
fn get_optional_string(verb_call: &VerbCall, key: &str) -> Option<String> {
    verb_call
        .arguments
        .iter()
        .find(|a| a.key == key)
        .and_then(|a| a.value.as_string().map(|s| s.to_string()))
}

/// Extract a required string argument from verb call
#[cfg(feature = "database")]
fn get_required_string(verb_call: &VerbCall, key: &str) -> Result<String> {
    get_optional_string(verb_call, key)
        .ok_or_else(|| anyhow::anyhow!("Missing required argument :{}", key))
}

/// Extract an optional integer argument from verb call
#[cfg(feature = "database")]
fn get_optional_integer(verb_call: &VerbCall, key: &str) -> Option<i32> {
    verb_call
        .arguments
        .iter()
        .find(|a| a.key == key)
        .and_then(|a| a.value.as_integer().map(|i| i as i32))
}

/// Extract an optional decimal argument from verb call
#[cfg(feature = "database")]
fn get_optional_decimal(verb_call: &VerbCall, key: &str) -> Option<f64> {
    verb_call
        .arguments
        .iter()
        .find(|a| a.key == key)
        .and_then(|a| {
            a.value
                .as_decimal()
                .map(|d| d.to_string().parse::<f64>().unwrap_or(0.0))
        })
}

/// Get session ID from context, or generate a new one
#[cfg(feature = "database")]
fn get_session_id(ctx: &ExecutionContext) -> Uuid {
    ctx.session_id.unwrap_or_else(Uuid::new_v4)
}

// ============================================================================
// Lifecycle Operations
// ============================================================================

/// Start agent mode with a specific task
pub struct AgentStartOp;

#[async_trait]
impl CustomOperation for AgentStartOp {
    fn domain(&self) -> &'static str {
        "agent"
    }

    fn verb(&self) -> &'static str {
        "start"
    }

    fn rationale(&self) -> &'static str {
        "Starts agent mode with a specific research task"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let session_id = get_session_id(ctx);
        let task = get_required_string(verb_call, "task")?;
        let target_entity_id = get_optional_uuid(verb_call, "entity-id", ctx);
        let max_iterations = get_optional_integer(verb_call, "max-iterations").unwrap_or(50);
        let mode = get_optional_string(verb_call, "mode").unwrap_or_else(|| "hybrid".to_string());

        // Validate task type
        let valid_tasks = [
            "resolve-gaps",
            "chain-research",
            "enrich-entity",
            "enrich-group",
            "screen-entities",
        ];
        if !valid_tasks.contains(&task.as_str()) {
            return Err(anyhow::anyhow!(
                "Invalid task type '{}'. Valid: {:?}",
                task,
                valid_tasks
            ));
        }

        // Validate mode
        let valid_modes = ["agent", "hybrid"];
        if !valid_modes.contains(&mode.as_str()) {
            return Err(anyhow::anyhow!(
                "Invalid mode '{}'. Valid: {:?}",
                mode,
                valid_modes
            ));
        }

        // Generate agent session ID
        let agent_session_id = Uuid::new_v4();

        // Record agent start in database
        sqlx::query!(
            r#"
            INSERT INTO kyc.research_decisions
                (decision_id, session_id, source_provider, search_query, decision_type,
                 selection_confidence, selection_reasoning, candidates_found, created_at)
            VALUES ($1, $2, 'manual', $3, 'AUTO_SELECTED', 1.0, $4, '[]'::jsonb, NOW())
            "#,
            agent_session_id,
            session_id,
            format!("agent:{}", task),
            format!("Started {} task in {} mode", task, mode)
        )
        .execute(pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to record agent start: {}", e))?;

        // Set agent mode in session context via pending state
        ctx.set_pending_agent_start(agent_session_id, task.clone());

        Ok(ExecutionResult::Record(json!({
            "agent_session_id": agent_session_id,
            "session_id": session_id,
            "task": task,
            "target_entity_id": target_entity_id,
            "max_iterations": max_iterations,
            "mode": mode,
            "status": "started"
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "Database feature required for agent operations"
        ))
    }
}

/// Pause the running agent loop
pub struct AgentPauseOp;

#[async_trait]
impl CustomOperation for AgentPauseOp {
    fn domain(&self) -> &'static str {
        "agent"
    }

    fn verb(&self) -> &'static str {
        "pause"
    }

    fn rationale(&self) -> &'static str {
        "Pauses the running agent loop"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        // Signal pause via context
        ctx.set_pending_agent_pause();

        Ok(ExecutionResult::Affected(1))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "Database feature required for agent operations"
        ))
    }
}

/// Resume a paused agent loop
pub struct AgentResumeOp;

#[async_trait]
impl CustomOperation for AgentResumeOp {
    fn domain(&self) -> &'static str {
        "agent"
    }

    fn verb(&self) -> &'static str {
        "resume"
    }

    fn rationale(&self) -> &'static str {
        "Resumes a paused agent loop"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        // Signal resume via context
        ctx.set_pending_agent_resume();

        Ok(ExecutionResult::Affected(1))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "Database feature required for agent operations"
        ))
    }
}

/// Stop the agent loop completely
pub struct AgentStopOp;

#[async_trait]
impl CustomOperation for AgentStopOp {
    fn domain(&self) -> &'static str {
        "agent"
    }

    fn verb(&self) -> &'static str {
        "stop"
    }

    fn rationale(&self) -> &'static str {
        "Stops the agent loop completely"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        // Signal stop via context
        ctx.set_pending_agent_stop();

        Ok(ExecutionResult::Affected(1))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "Database feature required for agent operations"
        ))
    }
}

// ============================================================================
// Checkpoint Operations
// ============================================================================

/// Confirm a checkpoint decision
pub struct AgentConfirmOp;

#[async_trait]
impl CustomOperation for AgentConfirmOp {
    fn domain(&self) -> &'static str {
        "agent"
    }

    fn verb(&self) -> &'static str {
        "confirm"
    }

    fn rationale(&self) -> &'static str {
        "Confirms a checkpoint decision and proceeds"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let checkpoint_id = get_optional_uuid(verb_call, "checkpoint-id", ctx);
        let selected_candidate = get_optional_integer(verb_call, "selected-candidate").unwrap_or(0);

        // Record confirmation
        if let Some(cp_id) = checkpoint_id {
            sqlx::query!(
                r#"
                UPDATE kyc.research_decisions
                SET decision_type = 'USER_CONFIRMED',
                    verified_at = NOW()
                WHERE decision_id = $1
                "#,
                cp_id
            )
            .execute(pool)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to confirm checkpoint: {}", e))?;
        }
        let _ = selected_candidate; // Will be used when we select from candidates

        // Signal confirmation via context
        ctx.set_pending_checkpoint_response(checkpoint_id, "confirm", selected_candidate);

        Ok(ExecutionResult::Affected(1))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "Database feature required for agent operations"
        ))
    }
}

/// Reject a checkpoint and skip this decision
pub struct AgentRejectOp;

#[async_trait]
impl CustomOperation for AgentRejectOp {
    fn domain(&self) -> &'static str {
        "agent"
    }

    fn verb(&self) -> &'static str {
        "reject"
    }

    fn rationale(&self) -> &'static str {
        "Rejects a checkpoint and skips this decision"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let checkpoint_id = get_optional_uuid(verb_call, "checkpoint-id", ctx);
        let reason = get_optional_string(verb_call, "reason");

        // Record rejection
        if let Some(cp_id) = checkpoint_id {
            sqlx::query!(
                r#"
                UPDATE kyc.research_decisions
                SET decision_type = 'REJECTED',
                    verified_at = NOW()
                WHERE decision_id = $1
                "#,
                cp_id
            )
            .execute(pool)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to reject checkpoint: {}", e))?;
        }
        let _ = reason; // Reason could be stored in a separate notes field if needed

        // Signal rejection via context
        ctx.set_pending_checkpoint_response(checkpoint_id, "reject", 0);

        Ok(ExecutionResult::Affected(1))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "Database feature required for agent operations"
        ))
    }
}

/// Select a specific candidate from checkpoint options
pub struct AgentSelectOp;

#[async_trait]
impl CustomOperation for AgentSelectOp {
    fn domain(&self) -> &'static str {
        "agent"
    }

    fn verb(&self) -> &'static str {
        "select"
    }

    fn rationale(&self) -> &'static str {
        "Selects a specific candidate from checkpoint options"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let checkpoint_id = get_optional_uuid(verb_call, "checkpoint-id", ctx);
        let candidate_index = get_optional_integer(verb_call, "candidate-index")
            .ok_or_else(|| anyhow::anyhow!("Missing required argument :candidate-index"))?;

        // Record selection
        if let Some(cp_id) = checkpoint_id {
            sqlx::query!(
                r#"
                UPDATE kyc.research_decisions
                SET decision_type = 'USER_SELECTED',
                    verified_at = NOW()
                WHERE decision_id = $1
                "#,
                cp_id
            )
            .execute(pool)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to select candidate: {}", e))?;
        }
        let _ = candidate_index; // Index will be used to pick from candidates_found

        // Signal selection via context
        ctx.set_pending_checkpoint_response(checkpoint_id, "select", candidate_index);

        Ok(ExecutionResult::Affected(1))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "Database feature required for agent operations"
        ))
    }
}

// ============================================================================
// Status Operations
// ============================================================================

/// Get current agent status
pub struct AgentStatusOp;

#[async_trait]
impl CustomOperation for AgentStatusOp {
    fn domain(&self) -> &'static str {
        "agent"
    }

    fn verb(&self) -> &'static str {
        "status"
    }

    fn rationale(&self) -> &'static str {
        "Gets current agent status"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let session_id = get_session_id(ctx);

        // Get latest agent session for this session
        let latest = sqlx::query!(
            r#"
            SELECT decision_id, search_query, decision_type, created_at
            FROM kyc.research_decisions
            WHERE session_id = $1
              AND search_query LIKE 'agent:%'
            ORDER BY created_at DESC
            LIMIT 1
            "#,
            session_id
        )
        .fetch_optional(pool)
        .await?;

        match latest {
            Some(row) => {
                // Count decisions and actions for this agent session
                let stats = sqlx::query!(
                    r#"
                    SELECT
                        COUNT(*) FILTER (WHERE search_query NOT LIKE 'agent:%') as "decision_count!",
                        COUNT(*) FILTER (WHERE decision_type = 'USER_CONFIRMED') as "confirmed_count!",
                        COUNT(*) FILTER (WHERE decision_type = 'REJECTED') as "rejected_count!"
                    FROM kyc.research_decisions
                    WHERE session_id = $1
                      AND created_at >= $2
                    "#,
                    session_id,
                    row.created_at
                )
                .fetch_one(pool)
                .await?;

                let action_count = sqlx::query_scalar!(
                    r#"
                    SELECT COUNT(*) as "count!"
                    FROM kyc.research_actions
                    WHERE session_id = $1
                      AND executed_at >= $2
                    "#,
                    session_id,
                    row.created_at
                )
                .fetch_one(pool)
                .await?;

                Ok(ExecutionResult::Record(json!({
                    "agent_session_id": row.decision_id,
                    "task": row.search_query,
                    "status": row.decision_type,
                    "started_at": row.created_at,
                    "decisions_made": stats.decision_count,
                    "confirmed": stats.confirmed_count,
                    "rejected": stats.rejected_count,
                    "actions_taken": action_count
                })))
            }
            None => Ok(ExecutionResult::Record(json!({
                "status": "idle",
                "message": "No agent session active"
            }))),
        }
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "Database feature required for agent operations"
        ))
    }
}

/// Get agent decision and action history for current session
pub struct AgentHistoryOp;

#[async_trait]
impl CustomOperation for AgentHistoryOp {
    fn domain(&self) -> &'static str {
        "agent"
    }

    fn verb(&self) -> &'static str {
        "history"
    }

    fn rationale(&self) -> &'static str {
        "Gets agent decision and action history"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let session_id = get_session_id(ctx);
        let limit = get_optional_integer(verb_call, "limit").unwrap_or(50) as i64;

        // Get decisions
        let decisions = sqlx::query!(
            r#"
            SELECT decision_id, source_provider, search_query, decision_type,
                   selection_confidence, selected_key, created_at
            FROM kyc.research_decisions
            WHERE session_id = $1
            ORDER BY created_at DESC
            LIMIT $2
            "#,
            session_id,
            limit
        )
        .fetch_all(pool)
        .await?;

        let decision_list: Vec<serde_json::Value> = decisions
            .into_iter()
            .map(|d| {
                json!({
                    "decision_id": d.decision_id,
                    "source_provider": d.source_provider,
                    "query": d.search_query,
                    "decision_type": d.decision_type,
                    "confidence": d.selection_confidence,
                    "selected_key": d.selected_key,
                    "created_at": d.created_at
                })
            })
            .collect();

        // Get actions
        let actions = sqlx::query!(
            r#"
            SELECT action_id, decision_id, verb_domain, verb_name,
                   success, entities_created, executed_at
            FROM kyc.research_actions
            WHERE session_id = $1
            ORDER BY executed_at DESC
            LIMIT $2
            "#,
            session_id,
            limit
        )
        .fetch_all(pool)
        .await?;

        let action_list: Vec<serde_json::Value> = actions
            .into_iter()
            .map(|a| {
                json!({
                    "action_id": a.action_id,
                    "decision_id": a.decision_id,
                    "verb": format!("{}:{}", a.verb_domain, a.verb_name),
                    "success": a.success,
                    "entities_created": a.entities_created,
                    "executed_at": a.executed_at
                })
            })
            .collect();

        Ok(ExecutionResult::Record(json!({
            "decisions": decision_list,
            "actions": action_list,
            "decision_count": decision_list.len(),
            "action_count": action_list.len()
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "Database feature required for agent operations"
        ))
    }
}

// ============================================================================
// Configuration Operations
// ============================================================================

/// Set confidence thresholds for auto-selection
pub struct AgentSetThresholdOp;

#[async_trait]
impl CustomOperation for AgentSetThresholdOp {
    fn domain(&self) -> &'static str {
        "agent"
    }

    fn verb(&self) -> &'static str {
        "set-threshold"
    }

    fn rationale(&self) -> &'static str {
        "Sets confidence thresholds for auto-selection"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let auto_proceed = get_optional_decimal(verb_call, "auto-proceed");
        let ambiguous_floor = get_optional_decimal(verb_call, "ambiguous-floor");

        // Validate thresholds
        if let Some(ap) = auto_proceed {
            if !(0.0..=1.0).contains(&ap) {
                return Err(anyhow::anyhow!("auto-proceed must be between 0.0 and 1.0"));
            }
        }
        if let Some(af) = ambiguous_floor {
            if !(0.0..=1.0).contains(&af) {
                return Err(anyhow::anyhow!(
                    "ambiguous-floor must be between 0.0 and 1.0"
                ));
            }
        }

        // Signal threshold change via context
        ctx.set_pending_threshold_change(auto_proceed, ambiguous_floor);

        Ok(ExecutionResult::Record(json!({
            "auto_proceed": auto_proceed.unwrap_or(0.90),
            "ambiguous_floor": ambiguous_floor.unwrap_or(0.70),
            "message": "Thresholds updated"
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "Database feature required for agent operations"
        ))
    }
}

/// Switch between agent and hybrid modes
pub struct AgentSetModeOp;

#[async_trait]
impl CustomOperation for AgentSetModeOp {
    fn domain(&self) -> &'static str {
        "agent"
    }

    fn verb(&self) -> &'static str {
        "set-mode"
    }

    fn rationale(&self) -> &'static str {
        "Switches between manual, agent, and hybrid modes"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let mode = get_required_string(verb_call, "mode")?;

        // Validate mode
        let valid_modes = ["manual", "agent", "hybrid"];
        if !valid_modes.contains(&mode.as_str()) {
            return Err(anyhow::anyhow!(
                "Invalid mode '{}'. Valid: {:?}",
                mode,
                valid_modes
            ));
        }

        // Signal mode change via context
        ctx.set_pending_mode_change(mode.clone());

        Ok(ExecutionResult::Record(json!({
            "mode": mode,
            "message": format!("Mode set to {}", mode)
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "Database feature required for agent operations"
        ))
    }
}

// ============================================================================
// Registration
// ============================================================================

/// Register all agent operations with the registry
pub fn register_agent_ops(registry: &mut crate::dsl_v2::custom_ops::CustomOperationRegistry) {
    use std::sync::Arc;

    // Lifecycle
    registry.register(Arc::new(AgentStartOp));
    registry.register(Arc::new(AgentPauseOp));
    registry.register(Arc::new(AgentResumeOp));
    registry.register(Arc::new(AgentStopOp));

    // Checkpoints
    registry.register(Arc::new(AgentConfirmOp));
    registry.register(Arc::new(AgentRejectOp));
    registry.register(Arc::new(AgentSelectOp));

    // Status
    registry.register(Arc::new(AgentStatusOp));
    registry.register(Arc::new(AgentHistoryOp));

    // Configuration
    registry.register(Arc::new(AgentSetThresholdOp));
    registry.register(Arc::new(AgentSetModeOp));
}
