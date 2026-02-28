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
use governed_query_proc::governed_query;
use ob_poc_macros::register_custom_op;
use serde_json::json;
use uuid::Uuid;

use super::CustomOperation;
use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};

#[cfg(feature = "database")]
use sqlx::PgPool;

// chrono is used for teaching status timestamps
#[allow(unused_imports)]
use chrono;

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
#[register_custom_op]
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
#[register_custom_op]
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
#[register_custom_op]
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
#[register_custom_op]
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
#[register_custom_op]
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
#[register_custom_op]
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
#[register_custom_op]
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
#[register_custom_op]
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
#[register_custom_op]
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
#[register_custom_op]
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
#[register_custom_op]
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
    #[governed_query(verb = "agent.set-mode", skip_principal_check = true)]
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

/// Switch between Research and Governed authoring modes.
///
/// Research mode enables exploration, ChangeSet authoring, and full schema
/// introspection. Governed mode enables business operations and publish.
#[register_custom_op]
pub struct AgentSetAuthoringModeOp;

#[async_trait]
impl CustomOperation for AgentSetAuthoringModeOp {
    fn domain(&self) -> &'static str {
        "agent"
    }

    fn verb(&self) -> &'static str {
        "set-authoring-mode"
    }

    fn rationale(&self) -> &'static str {
        "Controls Research vs Governed authoring mode boundary"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let mode_str = get_required_string(verb_call, "mode")?;
        let confirm = verb_call
            .get_value("confirm")
            .and_then(|v| v.as_boolean())
            .unwrap_or(false);

        let mode =
            sem_os_core::authoring::agent_mode::AgentMode::parse(&mode_str).ok_or_else(|| {
                anyhow::anyhow!(
                    "Invalid authoring mode '{}'. Valid: research, governed",
                    mode_str
                )
            })?;

        if !confirm {
            return Ok(ExecutionResult::Record(json!({
                "requires_confirmation": true,
                "mode": mode_str,
                "message": format!(
                    "Switch to {} mode? This changes which verbs are available. \
                     Re-run with :confirm true to proceed.",
                    mode
                )
            })));
        }

        // Signal authoring mode change via agent control channel
        ctx.bind_json(
            "_agent_control",
            json!({
                "action": "set_authoring_mode",
                "mode": mode.to_string()
            }),
        );

        Ok(ExecutionResult::Record(json!({
            "mode": mode.to_string(),
            "allows_authoring": mode.allows_authoring(),
            "allows_full_introspect": mode.allows_full_introspect(),
            "message": format!("Authoring mode set to {}", mode)
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
// Teaching Operations (Direct Pattern Learning)
// ============================================================================

/// Teach a phrase→verb mapping to improve intent recognition
#[register_custom_op]
pub struct AgentTeachOp;

#[async_trait]
impl CustomOperation for AgentTeachOp {
    fn domain(&self) -> &'static str {
        "agent"
    }

    fn verb(&self) -> &'static str {
        "teach"
    }

    fn rationale(&self) -> &'static str {
        "Teaches a phrase→verb mapping for improved intent recognition"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let phrase = get_required_string(verb_call, "phrase")?;
        let verb = get_required_string(verb_call, "verb")?;
        let source =
            get_optional_string(verb_call, "source").unwrap_or_else(|| "dsl_teaching".to_string());

        // Call the database function
        let result: (bool,) = sqlx::query_as(r#"SELECT agent.teach_phrase($1, $2, $3)"#)
            .bind(&phrase)
            .bind(&verb)
            .bind(&source)
            .fetch_one(pool)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to teach phrase: {}", e))?;

        if result.0 {
            Ok(ExecutionResult::Record(json!({
                "success": true,
                "taught": true,
                "phrase": phrase,
                "verb": verb,
                "source": source,
                "message": format!("Taught: '{}' → {}. Run (agent.learn) to activate.", phrase, verb)
            })))
        } else {
            Ok(ExecutionResult::Record(json!({
                "success": false,
                "taught": false,
                "phrase": phrase,
                "verb": verb,
                "error": "Pattern already exists or verb not found"
            })))
        }
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "Database feature required for teaching operations"
        ))
    }
}

/// Remove a previously taught phrase→verb mapping
#[register_custom_op]
pub struct AgentUnteachOp;

#[async_trait]
impl CustomOperation for AgentUnteachOp {
    fn domain(&self) -> &'static str {
        "agent"
    }

    fn verb(&self) -> &'static str {
        "unteach"
    }

    fn rationale(&self) -> &'static str {
        "Removes a taught phrase→verb mapping"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let phrase = get_required_string(verb_call, "phrase")?;
        let verb = get_optional_string(verb_call, "verb");
        let reason =
            get_optional_string(verb_call, "reason").unwrap_or_else(|| "dsl_unteach".to_string());

        // Call the database function
        let result: (i32,) = sqlx::query_as(r#"SELECT agent.unteach_phrase($1, $2, $3)"#)
            .bind(&phrase)
            .bind(verb.as_deref())
            .bind(&reason)
            .fetch_one(pool)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to unteach phrase: {}", e))?;

        let removed_count = result.0;

        Ok(ExecutionResult::Record(json!({
            "success": true,
            "untaught": removed_count > 0,
            "phrase": phrase,
            "verb": verb,
            "removed_count": removed_count,
            "message": if removed_count > 0 {
                format!("Removed {} pattern(s) for phrase '{}'", removed_count, phrase)
            } else {
                "No matching patterns found to remove".to_string()
            }
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "Database feature required for teaching operations"
        ))
    }
}

/// Show recently taught patterns and their embedding status
#[register_custom_op]
pub struct AgentTeachingStatusOp;

#[async_trait]
impl CustomOperation for AgentTeachingStatusOp {
    fn domain(&self) -> &'static str {
        "agent"
    }

    fn verb(&self) -> &'static str {
        "teaching-status"
    }

    fn rationale(&self) -> &'static str {
        "Shows recently taught patterns and statistics"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let limit = get_optional_integer(verb_call, "limit").unwrap_or(20) as i32;
        let include_stats = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "include-stats")
            .and_then(|a| a.value.as_boolean())
            .unwrap_or(true);

        // Get recently taught patterns
        let recent: Vec<(String, String, String, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
            r#"
            SELECT phrase, verb, source, taught_at
            FROM agent.v_recently_taught
            ORDER BY taught_at DESC
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to get recently taught: {}", e))?;

        let recent_json: Vec<serde_json::Value> = recent
            .iter()
            .map(|(phrase, verb, source, taught_at)| {
                json!({
                    "phrase": phrase,
                    "verb": verb,
                    "source": source,
                    "taught_at": taught_at.to_rfc3339()
                })
            })
            .collect();

        // Get stats if requested
        let stats = if include_stats {
            let row: Option<(i64, i64, i64, Option<chrono::DateTime<chrono::Utc>>)> =
                sqlx::query_as(
                    r#"
                    SELECT
                        total_taught,
                        taught_today,
                        taught_this_week,
                        most_recent
                    FROM agent.v_teaching_stats
                    "#,
                )
                .fetch_optional(pool)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to get teaching stats: {}", e))?;

            row.map(|(total, today, week, most_recent)| {
                json!({
                    "total_taught": total,
                    "taught_today": today,
                    "taught_this_week": week,
                    "most_recent": most_recent.map(|t| t.to_rfc3339())
                })
            })
        } else {
            None
        };

        Ok(ExecutionResult::Record(json!({
            "success": true,
            "recent_count": recent_json.len(),
            "recent": recent_json,
            "stats": stats
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "Database feature required for teaching operations"
        ))
    }
}

// ============================================================================
// AgentGetModeOp - Read current agent operating mode
// ============================================================================

#[register_custom_op]
pub struct AgentGetModeOp;

#[async_trait]
impl CustomOperation for AgentGetModeOp {
    fn domain(&self) -> &'static str {
        "agent"
    }

    fn verb(&self) -> &'static str {
        "get-mode"
    }

    fn rationale(&self) -> &'static str {
        "Reads the current AgentMode (Research or Governed) from session context"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        // Read mode from json_bindings (set by AgentSetAuthoringModeOp)
        let mode = ctx
            .json_bindings
            .get("_agent_mode")
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_else(|| "governed".to_string());

        let parsed =
            sem_os_core::authoring::agent_mode::AgentMode::parse(&mode).unwrap_or_default();

        Ok(ExecutionResult::Record(json!({
            "mode": parsed.to_string(),
            "allows_authoring": parsed.allows_authoring(),
            "allows_full_introspect": parsed.allows_full_introspect(),
            "allows_business_verbs": parsed.allows_business_verbs(),
            "message": format!("Current agent mode: {}", parsed)
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
// AgentGetPolicyOp - Read current PolicyGate snapshot
// ============================================================================

#[register_custom_op]
pub struct AgentGetPolicyOp;

#[async_trait]
impl CustomOperation for AgentGetPolicyOp {
    fn domain(&self) -> &'static str {
        "agent"
    }

    fn verb(&self) -> &'static str {
        "get-policy"
    }

    fn rationale(&self) -> &'static str {
        "Reads the current PolicyGate configuration from environment"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        // Read policy flags from environment (same as PolicyGate reads them)
        let strict_pipeline =
            std::env::var("OBPOC_STRICT_SINGLE_PIPELINE").unwrap_or_else(|_| "true".to_string());
        let allow_raw_execute =
            std::env::var("OBPOC_ALLOW_RAW_EXECUTE").unwrap_or_else(|_| "false".to_string());
        let strict_semreg =
            std::env::var("OBPOC_STRICT_SEMREG").unwrap_or_else(|_| "true".to_string());
        let allow_legacy_generate =
            std::env::var("OBPOC_ALLOW_LEGACY_GENERATE").unwrap_or_else(|_| "false".to_string());

        Ok(ExecutionResult::Record(json!({
            "strict_single_pipeline": strict_pipeline == "true",
            "allow_raw_execute": allow_raw_execute == "true",
            "strict_semreg": strict_semreg == "true",
            "allow_legacy_generate": allow_legacy_generate == "true",
            "message": "Current PolicyGate configuration"
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
// AgentListToolsOp - List available MCP tool specifications
// ============================================================================

#[register_custom_op]
pub struct AgentListToolsOp;

#[async_trait]
impl CustomOperation for AgentListToolsOp {
    fn domain(&self) -> &'static str {
        "agent"
    }

    fn verb(&self) -> &'static str {
        "list-tools"
    }

    fn rationale(&self) -> &'static str {
        "Lists available MCP tool specifications from the sem_reg agent module"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let category_filter = get_optional_string(verb_call, "category");

        // Get tool specs from sem_reg agent module
        let all_specs = crate::sem_reg::agent::mcp_tools::all_tool_specs();

        let tools: Vec<serde_json::Value> = all_specs
            .into_iter()
            .filter(|spec| {
                if let Some(ref cat) = category_filter {
                    spec.name.contains(cat.as_str())
                } else {
                    true
                }
            })
            .map(|spec| {
                json!({
                    "name": spec.name,
                    "description": spec.description,
                })
            })
            .collect();

        Ok(ExecutionResult::RecordSet(
            tools
                .into_iter()
                .map(|v| serde_json::to_value(v).unwrap_or_default())
                .collect(),
        ))
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
// AgentTelemetrySummaryOp - Query intent telemetry for pipeline health
// ============================================================================

#[register_custom_op]
pub struct AgentTelemetrySummaryOp;

#[async_trait]
impl CustomOperation for AgentTelemetrySummaryOp {
    fn domain(&self) -> &'static str {
        "agent"
    }

    fn verb(&self) -> &'static str {
        "telemetry-summary"
    }

    fn rationale(&self) -> &'static str {
        "Queries intent_events telemetry views for pipeline health analysis"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let days_back = get_optional_integer(verb_call, "days-back").unwrap_or(7);
        let min_count = get_optional_integer(verb_call, "min-count").unwrap_or(2);
        let include_ccir = verb_call
            .get_value("include-ccir")
            .and_then(|v| v.as_boolean())
            .unwrap_or(false);

        // Query clarify hotspots
        let clarify_rows: Vec<(String, i64)> = sqlx::query_as(
            r#"
            SELECT chosen_verb_fqn, count(*) AS clarify_count
            FROM agent.intent_events
            WHERE outcome = 'needs_clarification'
              AND chosen_verb_fqn IS NOT NULL
              AND ts > now() - make_interval(days => $1)
            GROUP BY chosen_verb_fqn
            HAVING count(*) >= $2
            ORDER BY clarify_count DESC
            LIMIT 20
            "#,
        )
        .bind(days_back as i32)
        .bind(min_count as i64)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        // Query failure modes
        let failure_rows: Vec<(String, Option<String>, i64)> = sqlx::query_as(
            r#"
            SELECT outcome, error_code, count(*) AS event_count
            FROM agent.intent_events
            WHERE outcome NOT IN ('ready', 'scope_resolved', 'macro_expanded')
              AND ts > now() - make_interval(days => $1)
            GROUP BY outcome, error_code
            HAVING count(*) >= $2
            ORDER BY event_count DESC
            LIMIT 20
            "#,
        )
        .bind(days_back as i32)
        .bind(min_count as i64)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        // Query SemReg denies
        let deny_rows: Vec<(String, i64)> = sqlx::query_as(
            r#"
            SELECT denied_verb, count(*) AS deny_count
            FROM (
                SELECT jsonb_array_elements_text(semreg_denied_verbs) AS denied_verb
                FROM agent.intent_events
                WHERE semreg_denied_verbs IS NOT NULL
                  AND jsonb_array_length(semreg_denied_verbs) > 0
                  AND ts > now() - make_interval(days => $1)
            ) sub
            GROUP BY denied_verb
            HAVING count(*) >= $2
            ORDER BY deny_count DESC
            LIMIT 20
            "#,
        )
        .bind(days_back as i32)
        .bind(min_count as i64)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        // Total event count for the period
        let total: (i64,) = sqlx::query_as(
            r#"
            SELECT count(*) FROM agent.intent_events
            WHERE ts > now() - make_interval(days => $1)
            "#,
        )
        .bind(days_back as i32)
        .fetch_one(pool)
        .await
        .unwrap_or((0,));

        let mut result = json!({
            "period_days": days_back,
            "min_count_threshold": min_count,
            "total_events": total.0,
            "clarify_hotspots": clarify_rows.iter().map(|(verb, count)| {
                json!({"verb": verb, "count": count})
            }).collect::<Vec<_>>(),
            "failure_modes": failure_rows.iter().map(|(outcome, code, count)| {
                json!({"outcome": outcome, "error_code": code, "count": count})
            }).collect::<Vec<_>>(),
            "semreg_denies": deny_rows.iter().map(|(verb, count)| {
                json!({"verb": verb, "deny_count": count})
            }).collect::<Vec<_>>(),
        });

        // Optionally include CCIR statistics
        if include_ccir {
            let ccir_row: (i64, i64, i64) = sqlx::query_as(
                r#"
                SELECT
                    count(*) FILTER (WHERE allowed_verbs_fingerprint IS NOT NULL) AS fingerprinted,
                    coalesce(avg(pruned_verbs_count) FILTER (WHERE pruned_verbs_count IS NOT NULL), 0)::bigint AS avg_pruned,
                    count(*) FILTER (WHERE toctou_recheck_performed = true) AS toctou_rechecks
                FROM agent.intent_events
                WHERE ts > now() - make_interval(days => $1)
                "#,
            )
            .bind(days_back as i32)
            .fetch_one(pool)
            .await
            .unwrap_or((0, 0, 0));

            if let serde_json::Value::Object(ref mut map) = result {
                map.insert(
                    "ccir".to_string(),
                    json!({
                        "events_with_fingerprint": ccir_row.0,
                        "avg_pruned_verbs": ccir_row.1,
                        "toctou_rechecks": ccir_row.2,
                    }),
                );
            }
        }

        Ok(ExecutionResult::Record(result))
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
// AgentLearnOp - Activate taught patterns by running populate_embeddings
// ============================================================================

#[register_custom_op]
pub struct AgentLearnOp;

#[async_trait]
impl CustomOperation for AgentLearnOp {
    fn domain(&self) -> &'static str {
        "agent"
    }

    fn verb(&self) -> &'static str {
        "learn"
    }

    fn rationale(&self) -> &'static str {
        "Activates taught patterns by generating embeddings for semantic search"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        // Count pending patterns before
        let pending_before: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*) FROM agent.v_recently_taught rt
            WHERE NOT EXISTS (
                SELECT 1 FROM "ob-poc".verb_pattern_embeddings vpe
                WHERE vpe.phrase = rt.phrase AND vpe.verb_name = rt.verb
            )
            "#,
        )
        .fetch_one(pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to count pending patterns: {}", e))?;

        if pending_before.0 == 0 {
            return Ok(ExecutionResult::Record(json!({
                "success": true,
                "message": "No pending patterns to embed",
                "embedded_count": 0
            })));
        }

        tracing::info!(
            "Running populate_embeddings for {} pending patterns...",
            pending_before.0
        );

        // Run populate_embeddings synchronously (it's fast for delta loads)
        let output = tokio::process::Command::new("cargo")
            .args([
                "run",
                "--release",
                "-p",
                "ob-semantic-matcher",
                "--bin",
                "populate_embeddings",
            ])
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .output()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to run populate_embeddings: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Ok(ExecutionResult::Record(json!({
                "success": false,
                "error": format!("populate_embeddings failed: {}", stderr)
            })));
        }

        // Count how many were actually embedded
        let pending_after: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*) FROM agent.v_recently_taught rt
            WHERE NOT EXISTS (
                SELECT 1 FROM "ob-poc".verb_pattern_embeddings vpe
                WHERE vpe.phrase = rt.phrase AND vpe.verb_name = rt.verb
            )
            "#,
        )
        .fetch_one(pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to count remaining patterns: {}", e))?;

        let embedded_count = pending_before.0 - pending_after.0;

        Ok(ExecutionResult::Record(json!({
            "success": true,
            "message": format!("Activated {} new patterns for semantic search", embedded_count),
            "embedded_count": embedded_count,
            "pending_before": pending_before.0,
            "pending_after": pending_after.0
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "Database feature required for learning operations"
        ))
    }
}
