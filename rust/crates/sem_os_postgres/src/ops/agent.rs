//! Agent control verbs (20 plugin verbs) — YAML-first
//! re-implementation of `agent.*` from `rust/config/verbs/agent.yaml`.
//!
//! Most ops set a `pending_agent_control` side-channel on
//! `VerbExecutionContext.extensions`; the session's agent loop
//! materializes the signal outside of the scope transaction. The
//! DB-touching ops (start / read-status / read-history / teach /
//! unteach / read-teaching-status / read-telemetry-summary /
//! activate-teaching + checkpoint confirm/reject/select) run under
//! the ambient Sequencer transaction.
//!
//! `sqlx::query!` macros swapped for runtime `sqlx::query_as` /
//! `sqlx::query` (offline-cache free, slice #10 pattern).
//!
//! `activate-teaching` shells out to `cargo run -p ob-semantic-matcher`
//! — that subprocess lives outside the scope, but gate counts before
//! and after are read via `scope.executor()`.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use uuid::Uuid;

use dsl_runtime::domain_ops::helpers::{
    json_extract_bool_opt, json_extract_int_opt, json_extract_string, json_extract_string_opt,
    json_extract_uuid_opt,
};
use dsl_runtime::service_traits::McpToolRegistry;
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::SemOsVerbOp;

fn set_pending_agent_control(ctx: &mut VerbExecutionContext, value: Value) {
    if !ctx.extensions.is_object() {
        ctx.extensions = Value::Object(serde_json::Map::new());
    }
    ctx.extensions
        .as_object_mut()
        .unwrap()
        .insert("pending_agent_control".to_string(), value);
}

fn extract_session_id(ctx: &VerbExecutionContext) -> Uuid {
    ctx.extensions
        .get("session_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
        .unwrap_or_else(Uuid::new_v4)
}

// ---------------------------------------------------------------------------
// Lifecycle: start / pause / resume / stop
// ---------------------------------------------------------------------------

pub struct Start;

#[async_trait]
impl SemOsVerbOp for Start {
    fn fqn(&self) -> &str {
        "agent.start"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let session_id = extract_session_id(ctx);
        let task = json_extract_string(args, "task")?;
        let target_entity_id = json_extract_uuid_opt(args, ctx, "entity-id");
        let max_iterations = json_extract_int_opt(args, "max-iterations").unwrap_or(50) as i32;
        let mode = json_extract_string_opt(args, "mode").unwrap_or_else(|| "hybrid".to_string());

        let valid_tasks = [
            "resolve-gaps",
            "chain-research",
            "enrich-entity",
            "enrich-group",
            "screen-entities",
        ];
        if !valid_tasks.contains(&task.as_str()) {
            return Err(anyhow!(
                "Invalid task type '{}'. Valid: {:?}",
                task,
                valid_tasks
            ));
        }

        let valid_modes = ["agent", "hybrid"];
        if !valid_modes.contains(&mode.as_str()) {
            return Err(anyhow!("Invalid mode '{}'. Valid: {:?}", mode, valid_modes));
        }

        let agent_session_id = Uuid::new_v4();

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".research_decisions
                (decision_id, session_id, source_provider, search_query, decision_type,
                 selection_confidence, selection_reasoning, candidates_found, created_at)
            VALUES ($1, $2, 'manual', $3, 'AUTO_SELECTED', 1.0, $4, '[]'::jsonb, NOW())
            "#,
        )
        .bind(agent_session_id)
        .bind(session_id)
        .bind(format!("agent:{}", task))
        .bind(format!("Started {} task in {} mode", task, mode))
        .execute(scope.executor())
        .await
        .map_err(|e| anyhow!("Failed to record agent start: {}", e))?;

        set_pending_agent_control(
            ctx,
            json!({
                "action": "start",
                "agent_session_id": agent_session_id,
                "task": task.clone()
            }),
        );

        Ok(VerbExecutionOutcome::Record(json!({
            "agent_session_id": agent_session_id,
            "session_id": session_id,
            "task": task,
            "target_entity_id": target_entity_id,
            "max_iterations": max_iterations,
            "mode": mode,
            "status": "started"
        })))
    }
}

macro_rules! simple_signal_op {
    ($name:ident, $fqn:expr, $action:expr) => {
        pub struct $name;

        #[async_trait]
        impl SemOsVerbOp for $name {
            fn fqn(&self) -> &str {
                $fqn
            }
            async fn execute(
                &self,
                _args: &Value,
                ctx: &mut VerbExecutionContext,
                _scope: &mut dyn TransactionScope,
            ) -> Result<VerbExecutionOutcome> {
                set_pending_agent_control(ctx, json!({ "action": $action }));
                Ok(VerbExecutionOutcome::Affected(1))
            }
        }
    };
}

simple_signal_op!(Pause, "agent.pause", "pause");
simple_signal_op!(Resume, "agent.resume", "resume");
simple_signal_op!(Stop, "agent.stop", "stop");

// ---------------------------------------------------------------------------
// Checkpoints: confirm / reject / select
// ---------------------------------------------------------------------------

pub struct ConfirmDecision;

#[async_trait]
impl SemOsVerbOp for ConfirmDecision {
    fn fqn(&self) -> &str {
        "agent.confirm-decision"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let checkpoint_id = json_extract_uuid_opt(args, ctx, "checkpoint-id");
        let selected_candidate = json_extract_int_opt(args, "selected-candidate").unwrap_or(0) as i32;

        if let Some(cp_id) = checkpoint_id {
            sqlx::query(
                r#"
                UPDATE "ob-poc".research_decisions
                SET decision_type = 'USER_CONFIRMED',
                    verified_at = NOW()
                WHERE decision_id = $1
                "#,
            )
            .bind(cp_id)
            .execute(scope.executor())
            .await
            .map_err(|e| anyhow!("Failed to confirm checkpoint: {}", e))?;
        }

        set_pending_agent_control(
            ctx,
            json!({
                "action": "checkpoint_response",
                "checkpoint_id": checkpoint_id,
                "response_type": "confirm",
                "selected_index": selected_candidate
            }),
        );
        Ok(VerbExecutionOutcome::Affected(1))
    }
}

pub struct RejectDecision;

#[async_trait]
impl SemOsVerbOp for RejectDecision {
    fn fqn(&self) -> &str {
        "agent.reject-decision"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let checkpoint_id = json_extract_uuid_opt(args, ctx, "checkpoint-id");
        let _reason = json_extract_string_opt(args, "reason");

        if let Some(cp_id) = checkpoint_id {
            sqlx::query(
                r#"
                UPDATE "ob-poc".research_decisions
                SET decision_type = 'REJECTED',
                    verified_at = NOW()
                WHERE decision_id = $1
                "#,
            )
            .bind(cp_id)
            .execute(scope.executor())
            .await
            .map_err(|e| anyhow!("Failed to reject checkpoint: {}", e))?;
        }

        set_pending_agent_control(
            ctx,
            json!({
                "action": "checkpoint_response",
                "checkpoint_id": checkpoint_id,
                "response_type": "reject",
                "selected_index": 0
            }),
        );
        Ok(VerbExecutionOutcome::Affected(1))
    }
}

pub struct SelectDecisionOption;

#[async_trait]
impl SemOsVerbOp for SelectDecisionOption {
    fn fqn(&self) -> &str {
        "agent.select-decision-option"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let checkpoint_id = json_extract_uuid_opt(args, ctx, "checkpoint-id");
        let candidate_index = json_extract_int_opt(args, "candidate-index")
            .ok_or_else(|| anyhow!("Missing required argument :candidate-index"))?
            as i32;

        if let Some(cp_id) = checkpoint_id {
            sqlx::query(
                r#"
                UPDATE "ob-poc".research_decisions
                SET decision_type = 'USER_SELECTED',
                    verified_at = NOW()
                WHERE decision_id = $1
                "#,
            )
            .bind(cp_id)
            .execute(scope.executor())
            .await
            .map_err(|e| anyhow!("Failed to select candidate: {}", e))?;
        }

        set_pending_agent_control(
            ctx,
            json!({
                "action": "checkpoint_response",
                "checkpoint_id": checkpoint_id,
                "response_type": "select",
                "selected_index": candidate_index
            }),
        );
        Ok(VerbExecutionOutcome::Affected(1))
    }
}

// ---------------------------------------------------------------------------
// Status: read-status / read-history
// ---------------------------------------------------------------------------

pub struct ReadStatus;

#[async_trait]
impl SemOsVerbOp for ReadStatus {
    fn fqn(&self) -> &str {
        "agent.read-status"
    }
    async fn execute(
        &self,
        _args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let session_id = extract_session_id(ctx);

        type LatestRow = (Uuid, String, String, DateTime<Utc>);
        let latest: Option<LatestRow> = sqlx::query_as(
            r#"
            SELECT decision_id, search_query, decision_type, created_at
            FROM "ob-poc".research_decisions
            WHERE session_id = $1
              AND search_query LIKE 'agent:%'
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(session_id)
        .fetch_optional(scope.executor())
        .await?;

        match latest {
            Some((decision_id, search_query, decision_type, created_at)) => {
                let (decision_count, confirmed_count, rejected_count): (i64, i64, i64) = sqlx::query_as(
                    r#"
                    SELECT
                        COUNT(*) FILTER (WHERE search_query NOT LIKE 'agent:%'),
                        COUNT(*) FILTER (WHERE decision_type = 'USER_CONFIRMED'),
                        COUNT(*) FILTER (WHERE decision_type = 'REJECTED')
                    FROM "ob-poc".research_decisions
                    WHERE session_id = $1
                      AND created_at >= $2
                    "#,
                )
                .bind(session_id)
                .bind(created_at)
                .fetch_one(scope.executor())
                .await?;

                let action_count: i64 = sqlx::query_scalar(
                    r#"
                    SELECT COUNT(*)
                    FROM "ob-poc".research_actions
                    WHERE session_id = $1
                      AND executed_at >= $2
                    "#,
                )
                .bind(session_id)
                .bind(created_at)
                .fetch_one(scope.executor())
                .await?;

                Ok(VerbExecutionOutcome::Record(json!({
                    "agent_session_id": decision_id,
                    "task": search_query,
                    "status": decision_type,
                    "started_at": created_at,
                    "decisions_made": decision_count,
                    "confirmed": confirmed_count,
                    "rejected": rejected_count,
                    "actions_taken": action_count
                })))
            }
            None => Ok(VerbExecutionOutcome::Record(json!({
                "status": "idle",
                "message": "No agent session active"
            }))),
        }
    }
}

pub struct ReadHistory;

#[async_trait]
impl SemOsVerbOp for ReadHistory {
    fn fqn(&self) -> &str {
        "agent.read-history"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let session_id = extract_session_id(ctx);
        let limit = json_extract_int_opt(args, "limit").unwrap_or(50);

        type DecRow = (
            Uuid, String, String, String,
            Option<rust_decimal::Decimal>, Option<String>, DateTime<Utc>,
        );
        let decisions: Vec<DecRow> = sqlx::query_as(
            r#"
            SELECT decision_id, source_provider, search_query, decision_type,
                   selection_confidence, selected_key, created_at
            FROM "ob-poc".research_decisions
            WHERE session_id = $1
            ORDER BY created_at DESC
            LIMIT $2
            "#,
        )
        .bind(session_id)
        .bind(limit)
        .fetch_all(scope.executor())
        .await?;

        let decision_list: Vec<Value> = decisions
            .into_iter()
            .map(|d| json!({
                "decision_id": d.0,
                "source_provider": d.1,
                "query": d.2,
                "decision_type": d.3,
                "confidence": d.4.map(|c| c.to_string()),
                "selected_key": d.5,
                "created_at": d.6
            }))
            .collect();

        type ActRow = (
            Uuid, Option<Uuid>, Option<String>, Option<String>,
            bool, i32, DateTime<Utc>,
        );
        let actions: Vec<ActRow> = sqlx::query_as(
            r#"
            SELECT action_id, decision_id, verb_domain, verb_name,
                   success, entities_created, executed_at
            FROM "ob-poc".research_actions
            WHERE session_id = $1
            ORDER BY executed_at DESC
            LIMIT $2
            "#,
        )
        .bind(session_id)
        .bind(limit)
        .fetch_all(scope.executor())
        .await?;

        let action_list: Vec<Value> = actions
            .into_iter()
            .map(|a| json!({
                "action_id": a.0,
                "decision_id": a.1,
                "verb": format!("{}:{}", a.2.unwrap_or_default(), a.3.unwrap_or_default()),
                "success": a.4,
                "entities_created": a.5,
                "executed_at": a.6
            }))
            .collect();

        Ok(VerbExecutionOutcome::Record(json!({
            "decisions": decision_list,
            "actions": action_list,
            "decision_count": decision_list.len(),
            "action_count": action_list.len()
        })))
    }
}

// ---------------------------------------------------------------------------
// Configuration: set-selection-threshold / set-execution-mode / set-authoring-mode
// ---------------------------------------------------------------------------

pub struct SetSelectionThreshold;

#[async_trait]
impl SemOsVerbOp for SetSelectionThreshold {
    fn fqn(&self) -> &str {
        "agent.set-selection-threshold"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let auto_proceed: Option<f64> = args.get("auto-proceed").and_then(|v| v.as_f64());
        let ambiguous_floor: Option<f64> = args.get("ambiguous-floor").and_then(|v| v.as_f64());

        if let Some(ap) = auto_proceed {
            if !(0.0..=1.0).contains(&ap) {
                return Err(anyhow!("auto-proceed must be between 0.0 and 1.0"));
            }
        }
        if let Some(af) = ambiguous_floor {
            if !(0.0..=1.0).contains(&af) {
                return Err(anyhow!("ambiguous-floor must be between 0.0 and 1.0"));
            }
        }

        set_pending_agent_control(
            ctx,
            json!({
                "action": "set_threshold",
                "auto_proceed": auto_proceed,
                "ambiguous_floor": ambiguous_floor
            }),
        );

        Ok(VerbExecutionOutcome::Record(json!({
            "auto_proceed": auto_proceed.unwrap_or(0.90),
            "ambiguous_floor": ambiguous_floor.unwrap_or(0.70),
            "message": "Thresholds updated"
        })))
    }
}

pub struct SetExecutionMode;

#[async_trait]
impl SemOsVerbOp for SetExecutionMode {
    fn fqn(&self) -> &str {
        "agent.set-execution-mode"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let mode = json_extract_string(args, "mode")?;
        let valid_modes = ["manual", "agent", "hybrid"];
        if !valid_modes.contains(&mode.as_str()) {
            return Err(anyhow!("Invalid mode '{}'. Valid: {:?}", mode, valid_modes));
        }

        set_pending_agent_control(
            ctx,
            json!({
                "action": "set_mode",
                "mode": mode.clone()
            }),
        );

        Ok(VerbExecutionOutcome::Record(json!({
            "mode": mode,
            "message": format!("Mode set to {}", mode)
        })))
    }
}

pub struct SetAuthoringMode;

#[async_trait]
impl SemOsVerbOp for SetAuthoringMode {
    fn fqn(&self) -> &str {
        "agent.set-authoring-mode"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let mode_str = json_extract_string(args, "mode")?;
        let confirm = json_extract_bool_opt(args, "confirm").unwrap_or(false);

        let mode =
            sem_os_core::authoring::agent_mode::AgentMode::parse(&mode_str).ok_or_else(|| {
                anyhow!(
                    "Invalid authoring mode '{}'. Valid: research, governed",
                    mode_str
                )
            })?;

        if !confirm {
            return Ok(VerbExecutionOutcome::Record(json!({
                "requires_confirmation": true,
                "mode": mode_str,
                "message": format!(
                    "Switch to {} mode? This changes which verbs are available. \
                     Re-run with :confirm true to proceed.",
                    mode
                )
            })));
        }

        set_pending_agent_control(
            ctx,
            json!({
                "action": "set_authoring_mode",
                "mode": mode.to_string()
            }),
        );

        Ok(VerbExecutionOutcome::Record(json!({
            "mode": mode.to_string(),
            "allows_authoring": mode.allows_authoring(),
            "allows_full_introspect": mode.allows_full_introspect(),
            "message": format!("Authoring mode set to {}", mode)
        })))
    }
}

// ---------------------------------------------------------------------------
// Teaching: teach / unteach / read-teaching-status / activate-teaching
// ---------------------------------------------------------------------------

pub struct Teach;

#[async_trait]
impl SemOsVerbOp for Teach {
    fn fqn(&self) -> &str {
        "agent.teach"
    }
    async fn execute(
        &self,
        args: &Value,
        _ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let phrase = json_extract_string(args, "phrase")?;
        let verb = json_extract_string(args, "verb")?;
        let source =
            json_extract_string_opt(args, "source").unwrap_or_else(|| "dsl_teaching".to_string());

        let (ok,): (bool,) = sqlx::query_as(r#"SELECT "ob-poc".teach_phrase($1, $2, $3)"#)
            .bind(&phrase)
            .bind(&verb)
            .bind(&source)
            .fetch_one(scope.executor())
            .await
            .map_err(|e| anyhow!("Failed to teach phrase: {}", e))?;

        let val = if ok {
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
        Ok(VerbExecutionOutcome::Record(val))
    }
}

pub struct Unteach;

#[async_trait]
impl SemOsVerbOp for Unteach {
    fn fqn(&self) -> &str {
        "agent.unteach"
    }
    async fn execute(
        &self,
        args: &Value,
        _ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let phrase = json_extract_string(args, "phrase")?;
        let verb = json_extract_string_opt(args, "verb");
        let reason =
            json_extract_string_opt(args, "reason").unwrap_or_else(|| "dsl_unteach".to_string());

        let (removed_count,): (i32,) = sqlx::query_as(r#"SELECT "ob-poc".unteach_phrase($1, $2, $3)"#)
            .bind(&phrase)
            .bind(verb.as_deref())
            .bind(&reason)
            .fetch_one(scope.executor())
            .await
            .map_err(|e| anyhow!("Failed to unteach phrase: {}", e))?;

        Ok(VerbExecutionOutcome::Record(json!({
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
}

pub struct ReadTeachingStatus;

#[async_trait]
impl SemOsVerbOp for ReadTeachingStatus {
    fn fqn(&self) -> &str {
        "agent.read-teaching-status"
    }
    async fn execute(
        &self,
        args: &Value,
        _ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let limit = json_extract_int_opt(args, "limit").unwrap_or(20) as i32;
        let include_stats = json_extract_bool_opt(args, "include-stats").unwrap_or(true);

        let recent: Vec<(String, String, String, DateTime<Utc>)> = sqlx::query_as(
            r#"
            SELECT phrase, verb, source, taught_at
            FROM "ob-poc".v_recently_taught
            ORDER BY taught_at DESC
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(scope.executor())
        .await
        .map_err(|e| anyhow!("Failed to get recently taught: {}", e))?;

        let recent_json: Vec<Value> = recent
            .iter()
            .map(|(phrase, verb, source, taught_at)| json!({
                "phrase": phrase, "verb": verb, "source": source,
                "taught_at": taught_at.to_rfc3339()
            }))
            .collect();

        let stats = if include_stats {
            let row: Option<(i64, i64, i64, Option<DateTime<Utc>>)> = sqlx::query_as(
                r#"
                SELECT total_taught, taught_today, taught_this_week, most_recent
                FROM "ob-poc".v_teaching_stats
                "#,
            )
            .fetch_optional(scope.executor())
            .await
            .map_err(|e| anyhow!("Failed to get teaching stats: {}", e))?;
            row.map(|(total, today, week, most_recent)| json!({
                "total_taught": total, "taught_today": today,
                "taught_this_week": week,
                "most_recent": most_recent.map(|t| t.to_rfc3339())
            }))
        } else {
            None
        };

        Ok(VerbExecutionOutcome::Record(json!({
            "success": true,
            "recent_count": recent_json.len(),
            "recent": recent_json,
            "stats": stats
        })))
    }
}

pub struct ActivateTeaching;

#[async_trait]
impl SemOsVerbOp for ActivateTeaching {
    fn fqn(&self) -> &str {
        "agent.activate-teaching"
    }
    async fn execute(
        &self,
        _args: &Value,
        _ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let (pending_before,): (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*) FROM "ob-poc".v_recently_taught rt
            WHERE NOT EXISTS (
                SELECT 1 FROM "ob-poc".verb_pattern_embeddings vpe
                WHERE vpe.phrase = rt.phrase AND vpe.verb_name = rt.verb
            )
            "#,
        )
        .fetch_one(scope.executor())
        .await
        .map_err(|e| anyhow!("Failed to count pending patterns: {}", e))?;

        if pending_before == 0 {
            return Ok(VerbExecutionOutcome::Record(json!({
                "success": true,
                "message": "No pending patterns to embed",
                "embedded_count": 0
            })));
        }

        tracing::info!(
            "Running populate_embeddings for {} pending patterns...",
            pending_before
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
            .output()
            .await
            .map_err(|e| anyhow!("Failed to run populate_embeddings: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Ok(VerbExecutionOutcome::Record(json!({
                "success": false,
                "error": format!("populate_embeddings failed: {}", stderr)
            })));
        }

        let (pending_after,): (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*) FROM "ob-poc".v_recently_taught rt
            WHERE NOT EXISTS (
                SELECT 1 FROM "ob-poc".verb_pattern_embeddings vpe
                WHERE vpe.phrase = rt.phrase AND vpe.verb_name = rt.verb
            )
            "#,
        )
        .fetch_one(scope.executor())
        .await
        .map_err(|e| anyhow!("Failed to count remaining patterns: {}", e))?;

        let embedded_count = pending_before - pending_after;
        Ok(VerbExecutionOutcome::Record(json!({
            "success": true,
            "message": format!("Activated {} new patterns for semantic search", embedded_count),
            "embedded_count": embedded_count,
            "pending_before": pending_before,
            "pending_after": pending_after
        })))
    }
}

// ---------------------------------------------------------------------------
// Read-only introspection: read-mode / read-policy / list-tools / read-telemetry-summary
// ---------------------------------------------------------------------------

pub struct ReadMode;

#[async_trait]
impl SemOsVerbOp for ReadMode {
    fn fqn(&self) -> &str {
        "agent.read-mode"
    }
    async fn execute(
        &self,
        _args: &Value,
        ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let mode = ctx
            .extensions
            .get("_agent_mode")
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_else(|| "governed".to_string());
        let parsed =
            sem_os_core::authoring::agent_mode::AgentMode::parse(&mode).unwrap_or_default();
        Ok(VerbExecutionOutcome::Record(json!({
            "mode": parsed.to_string(),
            "allows_authoring": parsed.allows_authoring(),
            "allows_full_introspect": parsed.allows_full_introspect(),
            "allows_business_verbs": parsed.allows_business_verbs(),
            "message": format!("Current agent mode: {}", parsed)
        })))
    }
}

pub struct ReadPolicy;

#[async_trait]
impl SemOsVerbOp for ReadPolicy {
    fn fqn(&self) -> &str {
        "agent.read-policy"
    }
    async fn execute(
        &self,
        _args: &Value,
        _ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let strict_pipeline =
            std::env::var("OBPOC_STRICT_SINGLE_PIPELINE").unwrap_or_else(|_| "true".to_string());
        let strict_semreg =
            std::env::var("OBPOC_STRICT_SEMREG").unwrap_or_else(|_| "true".to_string());
        let allow_legacy_generate =
            std::env::var("OBPOC_ALLOW_LEGACY_GENERATE").unwrap_or_else(|_| "false".to_string());

        // F16 fix (Slice 3.1, 2026-04-22): `allow_raw_execute` removed — raw
        // DSL bypass no longer exists. Always report false for API
        // backwards-compat in consumers that haven't migrated yet.
        Ok(VerbExecutionOutcome::Record(json!({
            "strict_single_pipeline": strict_pipeline == "true",
            "allow_raw_execute": false,
            "strict_semreg": strict_semreg == "true",
            "allow_legacy_generate": allow_legacy_generate == "true",
            "message": "Current PolicyGate configuration"
        })))
    }
}

pub struct ListTools;

#[async_trait]
impl SemOsVerbOp for ListTools {
    fn fqn(&self) -> &str {
        "agent.list-tools"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let category_filter = json_extract_string_opt(args, "category");

        let all_specs = ctx.service::<dyn McpToolRegistry>()?.list_specs().await;
        let tools: Vec<Value> = all_specs
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
        Ok(VerbExecutionOutcome::RecordSet(tools))
    }
}

pub struct ReadTelemetrySummary;

#[async_trait]
impl SemOsVerbOp for ReadTelemetrySummary {
    fn fqn(&self) -> &str {
        "agent.read-telemetry-summary"
    }
    async fn execute(
        &self,
        args: &Value,
        _ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
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
        .fetch_all(scope.executor())
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
        .fetch_all(scope.executor())
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
        .fetch_all(scope.executor())
        .await
        .unwrap_or_default();

        let (total,): (i64,) = sqlx::query_as(
            r#"
            SELECT count(*) FROM "ob-poc".intent_events
            WHERE ts > now() - make_interval(days => $1)
            "#,
        )
        .bind(days_back)
        .fetch_one(scope.executor())
        .await
        .unwrap_or((0,));

        let mut result = json!({
            "period_days": days_back,
            "min_count_threshold": min_count,
            "total_events": total,
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
            .fetch_one(scope.executor())
            .await
            .unwrap_or((0, 0, 0));

            if let Value::Object(ref mut map) = result {
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

        Ok(VerbExecutionOutcome::Record(result))
    }
}
