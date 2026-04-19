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
use dsl_runtime_macros::register_custom_op;
use governed_query_proc::governed_query;
use serde_json::json;
use uuid::Uuid;

use super::CustomOperation;

#[cfg(feature = "database")]
use sqlx::PgPool;

// chrono is used for teaching status timestamps
#[allow(unused_imports)]
use chrono;

// ============================================================================
// Helper Functions
// ============================================================================

/// Set a pending agent-control side-channel entry in the native context extensions.
#[cfg(feature = "database")]
fn set_pending_agent_control(
    ctx: &mut dsl_runtime::VerbExecutionContext,
    value: serde_json::Value,
) {
    if !ctx.extensions.is_object() {
        ctx.extensions = serde_json::Value::Object(serde_json::Map::new());
    }
    ctx.extensions
        .as_object_mut()
        .unwrap()
        .insert("pending_agent_control".to_string(), value);
}

/// Read the session_id from the native context extensions, defaulting to a new UUID.
#[cfg(feature = "database")]
fn extract_session_id(ctx: &dsl_runtime::VerbExecutionContext) -> Uuid {
    ctx.extensions
        .get("session_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
        .unwrap_or_else(Uuid::new_v4)
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
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use super::helpers::{
            json_extract_int_opt, json_extract_string, json_extract_string_opt, json_extract_uuid_opt,
        };

        let session_id = extract_session_id(ctx);
        let task = json_extract_string(args, "task")?;
        let target_entity_id = json_extract_uuid_opt(args, ctx, "entity-id");
        let max_iterations = json_extract_int_opt(args, "max-iterations").unwrap_or(50) as i32;
        let mode =
            json_extract_string_opt(args, "mode").unwrap_or_else(|| "hybrid".to_string());

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
            INSERT INTO "ob-poc".research_decisions
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
        set_pending_agent_control(
            ctx,
            json!({
                "action": "start",
                "agent_session_id": agent_session_id,
                "task": task.clone()
            }),
        );

        Ok(dsl_runtime::VerbExecutionOutcome::Record(json!({
            "agent_session_id": agent_session_id,
            "session_id": session_id,
            "task": task,
            "target_entity_id": target_entity_id,
            "max_iterations": max_iterations,
            "mode": mode,
            "status": "started"
        })))
    }

    fn is_migrated(&self) -> bool {
        true
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
    async fn execute_json(
        &self,
        _args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        _pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        // Signal pause via context
        set_pending_agent_control(ctx, json!({ "action": "pause" }));

        Ok(dsl_runtime::VerbExecutionOutcome::Affected(1))
    }

    fn is_migrated(&self) -> bool {
        true
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
    async fn execute_json(
        &self,
        _args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        _pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        // Signal resume via context
        set_pending_agent_control(ctx, json!({ "action": "resume" }));

        Ok(dsl_runtime::VerbExecutionOutcome::Affected(1))
    }

    fn is_migrated(&self) -> bool {
        true
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
    async fn execute_json(
        &self,
        _args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        _pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        // Signal stop via context
        set_pending_agent_control(ctx, json!({ "action": "stop" }));

        Ok(dsl_runtime::VerbExecutionOutcome::Affected(1))
    }

    fn is_migrated(&self) -> bool {
        true
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
        "confirm-decision"
    }

    fn rationale(&self) -> &'static str {
        "Confirms a checkpoint decision and proceeds"
    }
    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use super::helpers::{json_extract_int_opt, json_extract_uuid_opt};

        let checkpoint_id = json_extract_uuid_opt(args, ctx, "checkpoint-id");
        let selected_candidate =
            json_extract_int_opt(args, "selected-candidate").unwrap_or(0) as i32;

        // Record confirmation
        if let Some(cp_id) = checkpoint_id {
            sqlx::query!(
                r#"
                UPDATE "ob-poc".research_decisions
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
        set_pending_agent_control(
            ctx,
            json!({
                "action": "checkpoint_response",
                "checkpoint_id": checkpoint_id,
                "response_type": "confirm",
                "selected_index": selected_candidate
            }),
        );

        Ok(dsl_runtime::VerbExecutionOutcome::Affected(1))
    }

    fn is_migrated(&self) -> bool {
        true
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
        "reject-decision"
    }

    fn rationale(&self) -> &'static str {
        "Rejects a checkpoint and skips this decision"
    }
    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use super::helpers::{json_extract_string_opt, json_extract_uuid_opt};

        let checkpoint_id = json_extract_uuid_opt(args, ctx, "checkpoint-id");
        let reason = json_extract_string_opt(args, "reason");

        // Record rejection
        if let Some(cp_id) = checkpoint_id {
            sqlx::query!(
                r#"
                UPDATE "ob-poc".research_decisions
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
        set_pending_agent_control(
            ctx,
            json!({
                "action": "checkpoint_response",
                "checkpoint_id": checkpoint_id,
                "response_type": "reject",
                "selected_index": 0
            }),
        );

        Ok(dsl_runtime::VerbExecutionOutcome::Affected(1))
    }

    fn is_migrated(&self) -> bool {
        true
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
        "select-decision-option"
    }

    fn rationale(&self) -> &'static str {
        "Selects a specific candidate from checkpoint options"
    }
    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use super::helpers::{json_extract_int_opt, json_extract_uuid_opt};

        let checkpoint_id = json_extract_uuid_opt(args, ctx, "checkpoint-id");
        let candidate_index = json_extract_int_opt(args, "candidate-index")
            .ok_or_else(|| anyhow::anyhow!("Missing required argument :candidate-index"))?
            as i32;

        // Record selection
        if let Some(cp_id) = checkpoint_id {
            sqlx::query!(
                r#"
                UPDATE "ob-poc".research_decisions
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
        set_pending_agent_control(
            ctx,
            json!({
                "action": "checkpoint_response",
                "checkpoint_id": checkpoint_id,
                "response_type": "select",
                "selected_index": candidate_index
            }),
        );

        Ok(dsl_runtime::VerbExecutionOutcome::Affected(1))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}


// ============================================================================
// Status Operations
// ============================================================================

/// Get current agent status
#[register_custom_op]
pub struct AgentStatusOp;

#[cfg(feature = "database")]
async fn agent_status_impl(session_id: Uuid, pool: &PgPool) -> Result<serde_json::Value> {
    let latest = sqlx::query!(
        r#"
        SELECT decision_id, search_query, decision_type, created_at
        FROM "ob-poc".research_decisions
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
            let stats = sqlx::query!(
                r#"
                SELECT
                    COUNT(*) FILTER (WHERE search_query NOT LIKE 'agent:%') as "decision_count!",
                    COUNT(*) FILTER (WHERE decision_type = 'USER_CONFIRMED') as "confirmed_count!",
                    COUNT(*) FILTER (WHERE decision_type = 'REJECTED') as "rejected_count!"
                FROM "ob-poc".research_decisions
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
                FROM "ob-poc".research_actions
                WHERE session_id = $1
                  AND executed_at >= $2
                "#,
                session_id,
                row.created_at
            )
            .fetch_one(pool)
            .await?;

            Ok(json!({
                "agent_session_id": row.decision_id,
                "task": row.search_query,
                "status": row.decision_type,
                "started_at": row.created_at,
                "decisions_made": stats.decision_count,
                "confirmed": stats.confirmed_count,
                "rejected": stats.rejected_count,
                "actions_taken": action_count
            }))
        }
        None => Ok(json!({
            "status": "idle",
            "message": "No agent session active"
        })),
    }
}

#[async_trait]
impl CustomOperation for AgentStatusOp {
    fn domain(&self) -> &'static str {
        "agent"
    }

    fn verb(&self) -> &'static str {
        "read-status"
    }

    fn rationale(&self) -> &'static str {
        "Gets current agent status"
    }
    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        _args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        let session_id = extract_session_id(ctx);
        Ok(dsl_runtime::VerbExecutionOutcome::Record(
            agent_status_impl(session_id, pool).await?,
        ))
    }
    fn is_migrated(&self) -> bool {
        true
    }
}


/// Get agent decision and action history for current session
#[register_custom_op]
pub struct AgentHistoryOp;

#[cfg(feature = "database")]
async fn agent_history_impl(
    session_id: Uuid,
    limit: i64,
    pool: &PgPool,
) -> Result<serde_json::Value> {
    let decisions = sqlx::query!(
        r#"
        SELECT decision_id, source_provider, search_query, decision_type,
               selection_confidence, selected_key, created_at
        FROM "ob-poc".research_decisions
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

    let actions = sqlx::query!(
        r#"
        SELECT action_id, decision_id, verb_domain, verb_name,
               success, entities_created, executed_at
        FROM "ob-poc".research_actions
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

    Ok(json!({
        "decisions": decision_list,
        "actions": action_list,
        "decision_count": decision_list.len(),
        "action_count": action_list.len()
    }))
}

#[async_trait]
impl CustomOperation for AgentHistoryOp {
    fn domain(&self) -> &'static str {
        "agent"
    }

    fn verb(&self) -> &'static str {
        "read-history"
    }

    fn rationale(&self) -> &'static str {
        "Gets agent decision and action history"
    }
    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use super::helpers::json_extract_int_opt;
        let session_id = extract_session_id(ctx);
        let limit = json_extract_int_opt(args, "limit").unwrap_or(50);
        Ok(dsl_runtime::VerbExecutionOutcome::Record(
            agent_history_impl(session_id, limit, pool).await?,
        ))
    }
    fn is_migrated(&self) -> bool {
        true
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
        "set-selection-threshold"
    }

    fn rationale(&self) -> &'static str {
        "Sets confidence thresholds for auto-selection"
    }
    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        _pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        let auto_proceed: Option<f64> = args.get("auto-proceed").and_then(|v| v.as_f64());
        let ambiguous_floor: Option<f64> = args.get("ambiguous-floor").and_then(|v| v.as_f64());

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
        set_pending_agent_control(
            ctx,
            json!({
                "action": "set_threshold",
                "auto_proceed": auto_proceed,
                "ambiguous_floor": ambiguous_floor
            }),
        );

        Ok(dsl_runtime::VerbExecutionOutcome::Record(json!({
            "auto_proceed": auto_proceed.unwrap_or(0.90),
            "ambiguous_floor": ambiguous_floor.unwrap_or(0.70),
            "message": "Thresholds updated"
        })))
    }

    fn is_migrated(&self) -> bool {
        true
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
        "set-execution-mode"
    }

    fn rationale(&self) -> &'static str {
        "Switches between manual, agent, and hybrid modes"
    }
    #[cfg(feature = "database")]
    #[governed_query(verb = "agent.set-mode", skip_principal_check = true)]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        _pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use super::helpers::json_extract_string;
        let mode = json_extract_string(args, "mode")?;

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
        set_pending_agent_control(
            ctx,
            json!({
                "action": "set_mode",
                "mode": mode.clone()
            }),
        );

        Ok(dsl_runtime::VerbExecutionOutcome::Record(json!({
            "mode": mode,
            "message": format!("Mode set to {}", mode)
        })))
    }

    fn is_migrated(&self) -> bool {
        true
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
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        _pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use super::helpers::{json_extract_bool_opt, json_extract_string};
        let mode_str = json_extract_string(args, "mode")?;
        let confirm = json_extract_bool_opt(args, "confirm").unwrap_or(false);

        let mode =
            sem_os_core::authoring::agent_mode::AgentMode::parse(&mode_str).ok_or_else(|| {
                anyhow::anyhow!(
                    "Invalid authoring mode '{}'. Valid: research, governed",
                    mode_str
                )
            })?;

        if !confirm {
            return Ok(dsl_runtime::VerbExecutionOutcome::Record(json!({
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
        set_pending_agent_control(
            ctx,
            json!({
                "action": "set_authoring_mode",
                "mode": mode.to_string()
            }),
        );

        Ok(dsl_runtime::VerbExecutionOutcome::Record(json!({
            "mode": mode.to_string(),
            "allows_authoring": mode.allows_authoring(),
            "allows_full_introspect": mode.allows_full_introspect(),
            "message": format!("Authoring mode set to {}", mode)
        })))
    }

    fn is_migrated(&self) -> bool {
        true
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
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        _ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use super::helpers::{json_extract_string, json_extract_string_opt};
        let phrase = json_extract_string(args, "phrase")?;
        let verb = json_extract_string(args, "verb")?;
        let source =
            json_extract_string_opt(args, "source").unwrap_or_else(|| "dsl_teaching".to_string());

        let result: (bool,) = sqlx::query_as(r#"SELECT "ob-poc".teach_phrase($1, $2, $3)"#)
            .bind(&phrase)
            .bind(&verb)
            .bind(&source)
            .fetch_one(pool)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to teach phrase: {}", e))?;

        let val = if result.0 {
            json!({
                "success": true, "taught": true, "phrase": phrase, "verb": verb, "source": source,
                "message": format!("Taught: '{}' → {}. Run (agent.activate-teaching) to activate.", phrase, verb)
            })
        } else {
            json!({
                "success": false, "taught": false, "phrase": phrase, "verb": verb,
                "error": "Pattern already exists or verb not found"
            })
        };
        Ok(dsl_runtime::VerbExecutionOutcome::Record(val))
    }
    fn is_migrated(&self) -> bool {
        true
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
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        _ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use super::helpers::{json_extract_string, json_extract_string_opt};
        let phrase = json_extract_string(args, "phrase")?;
        let verb = json_extract_string_opt(args, "verb");
        let reason =
            json_extract_string_opt(args, "reason").unwrap_or_else(|| "dsl_unteach".to_string());

        let result: (i32,) = sqlx::query_as(r#"SELECT "ob-poc".unteach_phrase($1, $2, $3)"#)
            .bind(&phrase)
            .bind(verb.as_deref())
            .bind(&reason)
            .fetch_one(pool)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to unteach phrase: {}", e))?;

        let removed_count = result.0;
        Ok(dsl_runtime::VerbExecutionOutcome::Record(
            json!({
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
            }),
        ))
    }
    fn is_migrated(&self) -> bool {
        true
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
        "read-teaching-status"
    }

    fn rationale(&self) -> &'static str {
        "Shows recently taught patterns and statistics"
    }
    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        _ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use super::helpers::{json_extract_bool_opt, json_extract_int_opt};
        let limit = json_extract_int_opt(args, "limit").unwrap_or(20) as i32;
        let include_stats = json_extract_bool_opt(args, "include-stats").unwrap_or(true);

        let recent: Vec<(String, String, String, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
            r#"
            SELECT phrase, verb, source, taught_at
            FROM "ob-poc".v_recently_taught
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
                    "phrase": phrase, "verb": verb, "source": source,
                    "taught_at": taught_at.to_rfc3339()
                })
            })
            .collect();

        let stats = if include_stats {
            let row: Option<(i64, i64, i64, Option<chrono::DateTime<chrono::Utc>>)> =
                sqlx::query_as(
                    r#"
                    SELECT total_taught, taught_today, taught_this_week, most_recent
                    FROM "ob-poc".v_teaching_stats
                    "#,
                )
                .fetch_optional(pool)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to get teaching stats: {}", e))?;

            row.map(|(total, today, week, most_recent)| {
                json!({
                    "total_taught": total, "taught_today": today,
                    "taught_this_week": week,
                    "most_recent": most_recent.map(|t| t.to_rfc3339())
                })
            })
        } else {
            None
        };

        Ok(dsl_runtime::VerbExecutionOutcome::Record(
            json!({
                "success": true,
                "recent_count": recent_json.len(),
                "recent": recent_json,
                "stats": stats
            }),
        ))
    }
    fn is_migrated(&self) -> bool {
        true
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
        "read-mode"
    }

    fn rationale(&self) -> &'static str {
        "Reads the current AgentMode (Research or Governed) from session context"
    }
    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        _args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        _pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        // Read mode from extensions (set by AgentSetAuthoringModeOp via pending_agent_control;
        // the session materializes it to `_agent_mode` once applied)
        let mode = ctx
            .extensions
            .get("_agent_mode")
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_else(|| "governed".to_string());

        let parsed =
            sem_os_core::authoring::agent_mode::AgentMode::parse(&mode).unwrap_or_default();

        Ok(dsl_runtime::VerbExecutionOutcome::Record(json!({
            "mode": parsed.to_string(),
            "allows_authoring": parsed.allows_authoring(),
            "allows_full_introspect": parsed.allows_full_introspect(),
            "allows_business_verbs": parsed.allows_business_verbs(),
            "message": format!("Current agent mode: {}", parsed)
        })))
    }

    fn is_migrated(&self) -> bool {
        true
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
        "read-policy"
    }

    fn rationale(&self) -> &'static str {
        "Reads the current PolicyGate configuration from environment"
    }
    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        _args: &serde_json::Value,
        _ctx: &mut dsl_runtime::VerbExecutionContext,
        _pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        let strict_pipeline =
            std::env::var("OBPOC_STRICT_SINGLE_PIPELINE").unwrap_or_else(|_| "true".to_string());
        let allow_raw_execute =
            std::env::var("OBPOC_ALLOW_RAW_EXECUTE").unwrap_or_else(|_| "false".to_string());
        let strict_semreg =
            std::env::var("OBPOC_STRICT_SEMREG").unwrap_or_else(|_| "true".to_string());
        let allow_legacy_generate =
            std::env::var("OBPOC_ALLOW_LEGACY_GENERATE").unwrap_or_else(|_| "false".to_string());

        Ok(dsl_runtime::VerbExecutionOutcome::Record(
            json!({
                "strict_single_pipeline": strict_pipeline == "true",
                "allow_raw_execute": allow_raw_execute == "true",
                "strict_semreg": strict_semreg == "true",
                "allow_legacy_generate": allow_legacy_generate == "true",
                "message": "Current PolicyGate configuration"
            }),
        ))
    }
    fn is_migrated(&self) -> bool {
        true
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
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        _ctx: &mut dsl_runtime::VerbExecutionContext,
        _pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use super::helpers::json_extract_string_opt;
        let category_filter = json_extract_string_opt(args, "category");

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
            .map(|spec| json!({ "name": spec.name, "description": spec.description }))
            .collect();

        Ok(dsl_runtime::VerbExecutionOutcome::RecordSet(
            tools
                .into_iter()
                .map(|v| serde_json::to_value(v).unwrap_or_default())
                .collect(),
        ))
    }
    fn is_migrated(&self) -> bool {
        true
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
        "read-telemetry-summary"
    }

    fn rationale(&self) -> &'static str {
        "Queries intent_events telemetry views for pipeline health analysis"
    }
    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        _ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use super::helpers::{json_extract_bool_opt, json_extract_int_opt};
        let days_back = json_extract_int_opt(args, "days-back").unwrap_or(7) as i32;
        let min_count = json_extract_int_opt(args, "min-count").unwrap_or(2) as i64;
        let include_ccir = json_extract_bool_opt(args, "include-ccir").unwrap_or(false);

        let clarify_rows: Vec<(String, i64)> = sqlx::query_as(
            r#"
            SELECT chosen_verb_fqn, count(*) AS clarify_count
            FROM "ob-poc".intent_events
            WHERE outcome = 'needs_clarification'
              AND chosen_verb_fqn IS NOT NULL
              AND ts > now() - make_interval(days => $1)
            GROUP BY chosen_verb_fqn
            HAVING count(*) >= $2
            ORDER BY clarify_count DESC
            LIMIT 20
            "#,
        )
        .bind(days_back)
        .bind(min_count)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        let failure_rows: Vec<(String, Option<String>, i64)> = sqlx::query_as(
            r#"
            SELECT outcome, error_code, count(*) AS event_count
            FROM "ob-poc".intent_events
            WHERE outcome NOT IN ('ready', 'scope_resolved', 'macro_expanded')
              AND ts > now() - make_interval(days => $1)
            GROUP BY outcome, error_code
            HAVING count(*) >= $2
            ORDER BY event_count DESC
            LIMIT 20
            "#,
        )
        .bind(days_back)
        .bind(min_count)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        let deny_rows: Vec<(String, i64)> = sqlx::query_as(
            r#"
            SELECT denied_verb, count(*) AS deny_count
            FROM (
                SELECT jsonb_array_elements_text(semreg_denied_verbs) AS denied_verb
                FROM "ob-poc".intent_events
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
        .bind(days_back)
        .bind(min_count)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        let total: (i64,) = sqlx::query_as(
            r#"
            SELECT count(*) FROM "ob-poc".intent_events
            WHERE ts > now() - make_interval(days => $1)
            "#,
        )
        .bind(days_back)
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

        if include_ccir {
            let ccir_row: (i64, i64, i64) = sqlx::query_as(
                r#"
                SELECT
                    count(*) FILTER (WHERE allowed_verbs_fingerprint IS NOT NULL) AS fingerprinted,
                    coalesce(avg(pruned_verbs_count) FILTER (WHERE pruned_verbs_count IS NOT NULL), 0)::bigint AS avg_pruned,
                    count(*) FILTER (WHERE toctou_recheck_performed = true) AS toctou_rechecks
                FROM "ob-poc".intent_events
                WHERE ts > now() - make_interval(days => $1)
                "#,
            )
            .bind(days_back)
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

        Ok(dsl_runtime::VerbExecutionOutcome::Record(result))
    }
    fn is_migrated(&self) -> bool {
        true
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
        "activate-teaching"
    }

    fn rationale(&self) -> &'static str {
        "Activates taught patterns by generating embeddings for semantic search"
    }
    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        _args: &serde_json::Value,
        _ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        let pending_before: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*) FROM "ob-poc".v_recently_taught rt
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
            return Ok(dsl_runtime::VerbExecutionOutcome::Record(
                json!({
                    "success": true,
                    "message": "No pending patterns to embed",
                    "embedded_count": 0
                }),
            ));
        }

        tracing::info!(
            "Running populate_embeddings for {} pending patterns...",
            pending_before.0
        );

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
            return Ok(dsl_runtime::VerbExecutionOutcome::Record(
                json!({
                    "success": false,
                    "error": format!("populate_embeddings failed: {}", stderr)
                }),
            ));
        }

        let pending_after: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*) FROM "ob-poc".v_recently_taught rt
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
        Ok(dsl_runtime::VerbExecutionOutcome::Record(
            json!({
                "success": true,
                "message": format!("Activated {} new patterns for semantic search", embedded_count),
                "embedded_count": embedded_count,
                "pending_before": pending_before.0,
                "pending_after": pending_after.0
            }),
        ))
    }
    fn is_migrated(&self) -> bool {
        true
    }
}
