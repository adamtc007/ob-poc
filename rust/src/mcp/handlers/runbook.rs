//! Staged Runbook MCP Tool Handlers
//!
//! These handlers implement the anti-hallucination execution model:
//! - Commands are staged, never auto-executed
//! - Entity references are resolved from DB, never invented
//! - Picker loop ensures ambiguous refs are resolved by user
//! - Execution only on explicit "run" command
//!
//! Key tools:
//! - `runbook_stage` - Stage a command (DSL or natural language)
//! - `runbook_pick` - Resolve ambiguous entity references
//! - `runbook_run` - Execute the staged runbook
//! - `runbook_show` - Display current runbook state
//! - `runbook_preview` - Preview without execution
//! - `runbook_remove` - Remove a staged command
//! - `runbook_abort` - Abort and clear runbook

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::PgPool;
use tracing::{info, instrument};
use uuid::Uuid;

use crate::repl::{
    service::RunbookService,
    staged_runbook::{ResolutionStatus, RunbookStatus},
};

// ============================================================================
// Request/Response Types
// ============================================================================

/// Request to stage a command
#[derive(Debug, Deserialize)]
pub struct StageRequest {
    /// Session ID (stable conversation key)
    pub session_id: String,
    /// DSL source or natural language
    pub input: String,
    /// Client group context (for entity resolution)
    pub client_group_id: Option<Uuid>,
    /// Persona (affects tag filtering)
    pub persona: Option<String>,
    /// Description for the command
    pub description: Option<String>,
    /// Original user prompt (for audit trail)
    pub source_prompt: Option<String>,
}

/// Request to pick an entity for resolution
#[derive(Debug, Deserialize)]
pub struct PickRequest {
    /// Command ID to pick for
    pub command_id: Uuid,
    /// Selected entity IDs (must be from candidate set)
    pub entity_ids: Vec<Uuid>,
}

/// Request to run the staged runbook
#[derive(Debug, Deserialize)]
pub struct RunRequest {
    /// Runbook ID
    pub runbook_id: Uuid,
}

/// Request to show runbook state
#[derive(Debug, Deserialize)]
pub struct ShowRequest {
    /// Session ID
    pub session_id: String,
}

/// Request to preview runbook
#[derive(Debug, Deserialize)]
pub struct PreviewRequest {
    /// Runbook ID
    pub runbook_id: Uuid,
}

/// Request to remove a command
#[derive(Debug, Deserialize)]
pub struct RemoveRequest {
    /// Command ID to remove
    pub command_id: Uuid,
}

/// Request to abort runbook
#[derive(Debug, Deserialize)]
pub struct AbortRequest {
    /// Runbook ID
    pub runbook_id: Uuid,
}

/// Response from staging a command
#[derive(Debug, Serialize)]
pub struct StageResponse {
    /// Whether staging succeeded
    pub success: bool,
    /// The staged command ID
    pub command_id: Option<Uuid>,
    /// Resolution status
    pub resolution_status: Option<String>,
    /// Entity count
    pub entity_count: usize,
    /// DSL hash (for audit)
    pub dsl_hash: Option<String>,
    /// If ambiguous, needs picker
    pub needs_pick: bool,
    /// Error message if failed
    pub error: Option<String>,
}

/// Response from picking an entity
#[derive(Debug, Serialize)]
pub struct PickResponse {
    /// Whether pick succeeded
    pub success: bool,
    /// Updated command resolution status
    pub resolution_status: Option<String>,
    /// Number of entities selected
    pub selected_count: usize,
    /// Error message if failed
    pub error: Option<String>,
}

/// Response from running the runbook
#[derive(Debug, Serialize)]
pub struct RunResponse {
    /// Whether execution succeeded
    pub success: bool,
    /// Runbook ID that was executed
    pub runbook_id: Option<Uuid>,
    /// Error message if failed
    pub error: Option<String>,
}

/// Response from showing runbook state
#[derive(Debug, Serialize)]
pub struct ShowResponse {
    /// Runbook exists
    pub exists: bool,
    /// Runbook data
    pub runbook: Option<RunbookDisplay>,
    /// Whether runbook is ready to execute
    pub is_ready: bool,
    /// Blocking issues
    pub blockers: Vec<String>,
}

/// Runbook for display
#[derive(Debug, Serialize)]
pub struct RunbookDisplay {
    pub id: Uuid,
    pub session_id: String,
    pub status: String,
    pub command_count: usize,
    pub commands: Vec<CommandDisplay>,
}

/// Command for display
#[derive(Debug, Serialize)]
pub struct CommandDisplay {
    pub id: Uuid,
    pub order: i32,
    pub verb: String,
    pub description: Option<String>,
    pub dsl_raw: String,
    pub dsl_resolved: Option<String>,
    pub resolution_status: String,
    pub entity_count: usize,
    pub candidates: Vec<CandidateDisplay>,
}

/// Picker candidate for display
#[derive(Debug, Serialize)]
pub struct CandidateDisplay {
    pub entity_id: Uuid,
    pub entity_name: String,
    pub arg_name: String,
    pub matched_tag: Option<String>,
    pub confidence: Option<f64>,
    pub match_type: String,
}

/// Preview response
#[derive(Debug, Serialize)]
pub struct PreviewResponse {
    pub success: bool,
    pub is_ready: bool,
    pub command_count: usize,
    pub entity_footprint: Vec<EntityFootprintDisplay>,
    pub blockers: Vec<BlockerDisplay>,
    pub error: Option<String>,
}

/// Entity in footprint
#[derive(Debug, Serialize)]
pub struct EntityFootprintDisplay {
    pub entity_id: Uuid,
    pub entity_name: String,
    pub commands: Vec<Uuid>,
    pub operations: Vec<String>,
}

/// Blocker display
#[derive(Debug, Serialize)]
pub struct BlockerDisplay {
    pub command_id: Uuid,
    pub source_order: i32,
    pub status: String,
    pub error: Option<String>,
}

// ============================================================================
// Handler Implementation
// ============================================================================

/// Runbook tool handlers
pub struct RunbookHandlers<'a> {
    pool: &'a PgPool,
}

impl<'a> RunbookHandlers<'a> {
    /// Create new runbook handlers
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    /// Stage a command (DSL or natural language)
    #[instrument(skip(self, args), fields(session_id))]
    pub async fn runbook_stage(&self, args: Value) -> Result<Value> {
        let req: StageRequest = serde_json::from_value(args)?;
        tracing::Span::current().record("session_id", &req.session_id);

        info!(
            input = %req.input,
            client_group_id = ?req.client_group_id,
            "Staging command in runbook"
        );

        let mut service = RunbookService::new(self.pool);
        let result = service
            .stage(
                &req.session_id,
                req.client_group_id,
                req.persona.as_deref(),
                &req.input,
                req.description.as_deref(),
                req.source_prompt.as_deref(),
            )
            .await;

        match result {
            Ok(stage_result) => {
                let needs_pick = stage_result.resolution_status == ResolutionStatus::Ambiguous;

                Ok(json!(StageResponse {
                    success: true,
                    command_id: Some(stage_result.command_id),
                    resolution_status: Some(stage_result.resolution_status.to_db().to_string()),
                    entity_count: stage_result.entity_count,
                    dsl_hash: Some(stage_result.dsl_hash),
                    needs_pick,
                    error: None,
                }))
            }
            Err(e) => Ok(json!(StageResponse {
                success: false,
                command_id: None,
                resolution_status: None,
                entity_count: 0,
                dsl_hash: None,
                needs_pick: false,
                error: Some(e.to_string()),
            })),
        }
    }

    /// Pick entity for ambiguous resolution
    #[instrument(skip(self, args), fields(command_id))]
    pub async fn runbook_pick(&self, args: Value) -> Result<Value> {
        let req: PickRequest = serde_json::from_value(args)?;
        tracing::Span::current().record("command_id", req.command_id.to_string());

        info!(
            entity_count = req.entity_ids.len(),
            "Picking entities for command"
        );

        let mut service = RunbookService::new(self.pool);
        let result = service.pick(req.command_id, &req.entity_ids).await;

        match result {
            Ok(pick_result) => Ok(json!(PickResponse {
                success: true,
                resolution_status: Some(pick_result.resolution_status.to_db().to_string()),
                selected_count: pick_result.selected_count,
                error: None,
            })),
            Err(e) => Ok(json!(PickResponse {
                success: false,
                resolution_status: None,
                selected_count: 0,
                error: Some(e.to_string()),
            })),
        }
    }

    /// Run the staged runbook
    #[instrument(skip(self, args), fields(runbook_id))]
    pub async fn runbook_run(&self, args: Value) -> Result<Value> {
        let req: RunRequest = serde_json::from_value(args)?;
        tracing::Span::current().record("runbook_id", req.runbook_id.to_string());

        info!("Running runbook");

        let mut service = RunbookService::new(self.pool);
        let result = service.run(req.runbook_id).await;

        match result {
            Ok(runbook_id) => Ok(json!(RunResponse {
                success: true,
                runbook_id: Some(runbook_id),
                error: None,
            })),
            Err(e) => Ok(json!(RunResponse {
                success: false,
                runbook_id: None,
                error: Some(e.to_string()),
            })),
        }
    }

    /// Show runbook state
    #[instrument(skip(self, args), fields(session_id))]
    pub async fn runbook_show(&self, args: Value) -> Result<Value> {
        let req: ShowRequest = serde_json::from_value(args)?;
        tracing::Span::current().record("session_id", &req.session_id);

        let mut service = RunbookService::new(self.pool);
        let result = service.show(&req.session_id).await;

        match result {
            Ok(Some(runbook)) => {
                let commands: Vec<CommandDisplay> = runbook
                    .commands
                    .iter()
                    .map(|c| CommandDisplay {
                        id: c.id,
                        order: c.source_order,
                        verb: c.verb.clone(),
                        description: c.description.clone(),
                        dsl_raw: c.dsl_raw.clone(),
                        dsl_resolved: c.dsl_resolved.clone(),
                        resolution_status: c.resolution_status.to_db().to_string(),
                        entity_count: c.entity_footprint.len(),
                        candidates: c
                            .candidates
                            .iter()
                            .map(|cand| CandidateDisplay {
                                entity_id: cand.entity_id,
                                entity_name: cand.entity_name.clone(),
                                arg_name: cand.arg_name.clone(),
                                matched_tag: cand.matched_tag.clone(),
                                confidence: cand.confidence,
                                match_type: cand.match_type.clone(),
                            })
                            .collect(),
                    })
                    .collect();

                let is_ready = runbook.status == RunbookStatus::Ready;
                let blockers: Vec<String> = runbook
                    .commands
                    .iter()
                    .filter(|c| c.resolution_status.blocks_execution())
                    .map(|c| {
                        format!(
                            "Command {} ({}): {}",
                            c.source_order,
                            c.verb,
                            c.resolution_status.to_db()
                        )
                    })
                    .collect();

                let display = RunbookDisplay {
                    id: runbook.id,
                    session_id: runbook.session_id.clone(),
                    status: runbook.status.to_db().to_string(),
                    command_count: commands.len(),
                    commands,
                };

                Ok(json!(ShowResponse {
                    exists: true,
                    runbook: Some(display),
                    is_ready,
                    blockers,
                }))
            }
            Ok(None) => Ok(json!(ShowResponse {
                exists: false,
                runbook: None,
                is_ready: false,
                blockers: vec![],
            })),
            Err(e) => Err(e),
        }
    }

    /// Preview runbook without execution
    #[instrument(skip(self, args), fields(runbook_id))]
    pub async fn runbook_preview(&self, args: Value) -> Result<Value> {
        let req: PreviewRequest = serde_json::from_value(args)?;
        tracing::Span::current().record("runbook_id", req.runbook_id.to_string());

        let mut service = RunbookService::new(self.pool);
        let result = service.preview(req.runbook_id).await;

        match result {
            Ok(preview) => {
                let entity_footprint: Vec<EntityFootprintDisplay> = preview
                    .entity_footprint
                    .into_iter()
                    .map(|e| EntityFootprintDisplay {
                        entity_id: e.entity_id,
                        entity_name: e.entity_name,
                        commands: e.commands,
                        operations: e.operations,
                    })
                    .collect();

                let blockers: Vec<BlockerDisplay> = preview
                    .blockers
                    .into_iter()
                    .map(|b| BlockerDisplay {
                        command_id: b.command_id,
                        source_order: b.source_order,
                        status: b.status.to_db().to_string(),
                        error: b.error,
                    })
                    .collect();

                Ok(json!(PreviewResponse {
                    success: true,
                    is_ready: preview.is_ready,
                    command_count: preview.runbook.commands.len(),
                    entity_footprint,
                    blockers,
                    error: None,
                }))
            }
            Err(e) => Ok(json!(PreviewResponse {
                success: false,
                is_ready: false,
                command_count: 0,
                entity_footprint: vec![],
                blockers: vec![],
                error: Some(e.to_string()),
            })),
        }
    }

    /// Remove a staged command
    #[instrument(skip(self, args), fields(command_id))]
    pub async fn runbook_remove(&self, args: Value) -> Result<Value> {
        let req: RemoveRequest = serde_json::from_value(args)?;
        tracing::Span::current().record("command_id", req.command_id.to_string());

        let mut service = RunbookService::new(self.pool);
        let result = service.remove(req.command_id).await;

        match result {
            Ok(removed_ids) => Ok(json!({
                "success": true,
                "removed_command_ids": removed_ids,
            })),
            Err(e) => Ok(json!({
                "success": false,
                "error": e.to_string(),
            })),
        }
    }

    /// Abort runbook
    #[instrument(skip(self, args), fields(runbook_id))]
    pub async fn runbook_abort(&self, args: Value) -> Result<Value> {
        let req: AbortRequest = serde_json::from_value(args)?;
        tracing::Span::current().record("runbook_id", req.runbook_id.to_string());

        info!("Aborting runbook");

        let mut service = RunbookService::new(self.pool);
        let result = service.abort(req.runbook_id).await;

        match result {
            Ok(aborted) => Ok(json!({
                "success": aborted,
            })),
            Err(e) => Ok(json!({
                "success": false,
                "error": e.to_string(),
            })),
        }
    }
}
