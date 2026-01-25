//! Runbook custom operations
//!
//! Plugin operations for the staged runbook anti-hallucination execution model.
//! These ops delegate to RunbookService for the actual implementation.
//!
//! Key guarantee: All entity IDs come from DB searches - the agent cannot
//! fabricate UUIDs.
//!
//! ## Operations
//!
//! - `runbook.stage` - Stage a command (DSL or natural language)
//! - `runbook.pick` - Resolve ambiguous entity references via picker
//! - `runbook.run` - Execute all staged commands
//! - `runbook.show` - Display current runbook state
//! - `runbook.preview` - Preview without execution
//! - `runbook.remove` - Remove a staged command
//! - `runbook.abort` - Abort and clear runbook

use anyhow::Result;
use async_trait::async_trait;
use ob_poc_macros::register_custom_op;

use super::CustomOperation;
use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};

#[cfg(feature = "database")]
use sqlx::PgPool;

#[cfg(feature = "database")]
use uuid::Uuid;

// ============================================================================
// runbook.stage - Stage a command
// ============================================================================

/// Stage a DSL command for later execution (anti-hallucination)
///
/// Rationale: Requires entity resolution via EntityArgResolver with
/// fuzzy matching, candidate generation, and DB-backed validation.
/// Cannot be expressed as simple CRUD.
#[register_custom_op]
pub struct RunbookStageOp;

#[async_trait]
impl CustomOperation for RunbookStageOp {
    fn domain(&self) -> &'static str {
        "runbook"
    }
    fn verb(&self) -> &'static str {
        "stage"
    }
    fn rationale(&self) -> &'static str {
        "Requires entity resolution pipeline with fuzzy matching and candidate generation"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use crate::repl::service::RunbookService;

        // Extract arguments
        let input = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "input")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing input argument"))?;

        let client_group_id: Option<Uuid> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "client-group-id")
            .and_then(|a| a.value.as_uuid());

        let persona = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "persona")
            .and_then(|a| a.value.as_string());

        let description = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "description")
            .and_then(|a| a.value.as_string());

        let source_prompt = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "source-prompt")
            .and_then(|a| a.value.as_string());

        // Get session ID from context
        let session_id = ctx
            .session_id
            .map(|id| id.to_string())
            .unwrap_or_else(|| "default".to_string());

        let mut service = RunbookService::new(pool);
        let result = service
            .stage(
                &session_id,
                client_group_id,
                persona,
                input,
                description,
                source_prompt,
            )
            .await?;

        Ok(ExecutionResult::Record(serde_json::json!({
            "command_id": result.command_id,
            "resolution_status": result.resolution_status.to_db(),
            "entity_count": result.entity_count,
            "dsl_hash": result.dsl_hash,
            "needs_pick": result.resolution_status == crate::repl::staged_runbook::ResolutionStatus::Ambiguous,
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("runbook.stage requires database feature"))
    }
}

// ============================================================================
// runbook.pick - Resolve ambiguous entity references
// ============================================================================

/// Select entities for ambiguous command resolution
///
/// Rationale: Requires validation that selected entity IDs are from the
/// candidate set (anti-hallucination). Uses DB function validate_picker_selection.
#[register_custom_op]
pub struct RunbookPickOp;

#[async_trait]
impl CustomOperation for RunbookPickOp {
    fn domain(&self) -> &'static str {
        "runbook"
    }
    fn verb(&self) -> &'static str {
        "pick"
    }
    fn rationale(&self) -> &'static str {
        "Requires DB validation that selected IDs are from candidate set (anti-hallucination)"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use crate::repl::service::RunbookService;

        let command_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "command-id")
            .and_then(|a| a.value.as_uuid())
            .ok_or_else(|| anyhow::anyhow!("Missing command-id argument"))?;

        let entity_ids: Vec<Uuid> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "entity-ids")
            .and_then(|a| a.value.as_list())
            .map(|list| {
                list.iter()
                    .filter_map(|node| node.as_uuid())
                    .collect::<Vec<_>>()
            })
            .ok_or_else(|| anyhow::anyhow!("Missing entity-ids argument"))?;

        let mut service = RunbookService::new(pool);
        let result = service.pick(command_id, &entity_ids).await?;

        Ok(ExecutionResult::Record(serde_json::json!({
            "resolution_status": result.resolution_status.to_db(),
            "selected_count": result.selected_count,
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("runbook.pick requires database feature"))
    }
}

// ============================================================================
// runbook.run - Execute staged runbook
// ============================================================================

/// Execute all staged commands in the runbook
///
/// Rationale: Requires transactional execution of multiple DSL statements
/// with rollback on failure. Complex orchestration logic.
#[register_custom_op]
pub struct RunbookRunOp;

#[async_trait]
impl CustomOperation for RunbookRunOp {
    fn domain(&self) -> &'static str {
        "runbook"
    }
    fn verb(&self) -> &'static str {
        "run"
    }
    fn rationale(&self) -> &'static str {
        "Requires transactional execution of multiple DSL statements with rollback"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use crate::repl::service::RunbookService;

        let runbook_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "runbook-id")
            .and_then(|a| a.value.as_uuid())
            .ok_or_else(|| anyhow::anyhow!("Missing runbook-id argument"))?;

        let mut service = RunbookService::new(pool);
        let result_id = service.run(runbook_id).await?;

        Ok(ExecutionResult::Record(serde_json::json!({
            "success": true,
            "runbook_id": result_id,
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("runbook.run requires database feature"))
    }
}

// ============================================================================
// runbook.show - Display runbook state
// ============================================================================

/// Display the current runbook with all staged commands
///
/// Rationale: Requires joining multiple tables (runbook, commands, entities,
/// candidates) with complex aggregation for display.
#[register_custom_op]
pub struct RunbookShowOp;

#[async_trait]
impl CustomOperation for RunbookShowOp {
    fn domain(&self) -> &'static str {
        "runbook"
    }
    fn verb(&self) -> &'static str {
        "show"
    }
    fn rationale(&self) -> &'static str {
        "Requires complex multi-table join with aggregation for runbook display"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use crate::repl::service::RunbookService;

        let session_id = ctx
            .session_id
            .map(|id| id.to_string())
            .unwrap_or_else(|| "default".to_string());

        let mut service = RunbookService::new(pool);
        let result = service.show(&session_id).await?;

        match result {
            Some(runbook) => {
                let commands: Vec<serde_json::Value> = runbook
                    .commands
                    .iter()
                    .map(|c| {
                        serde_json::json!({
                            "id": c.id,
                            "order": c.source_order,
                            "verb": c.verb,
                            "description": c.description,
                            "dsl_raw": c.dsl_raw,
                            "dsl_resolved": c.dsl_resolved,
                            "resolution_status": c.resolution_status.to_db(),
                            "entity_count": c.entity_footprint.len(),
                        })
                    })
                    .collect();

                Ok(ExecutionResult::Record(serde_json::json!({
                    "exists": true,
                    "runbook_id": runbook.id,
                    "status": runbook.status.to_db(),
                    "command_count": commands.len(),
                    "commands": commands,
                    "is_ready": runbook.status == crate::repl::staged_runbook::RunbookStatus::Ready,
                })))
            }
            None => Ok(ExecutionResult::Record(serde_json::json!({
                "exists": false,
            }))),
        }
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("runbook.show requires database feature"))
    }
}

// ============================================================================
// runbook.preview - Preview without execution
// ============================================================================

/// Preview what will happen when runbook is executed
///
/// Rationale: Requires entity footprint analysis across all commands
/// and blocker detection logic.
#[register_custom_op]
pub struct RunbookPreviewOp;

#[async_trait]
impl CustomOperation for RunbookPreviewOp {
    fn domain(&self) -> &'static str {
        "runbook"
    }
    fn verb(&self) -> &'static str {
        "preview"
    }
    fn rationale(&self) -> &'static str {
        "Requires entity footprint analysis and blocker detection across commands"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use crate::repl::service::RunbookService;

        let runbook_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "runbook-id")
            .and_then(|a| a.value.as_uuid())
            .ok_or_else(|| anyhow::anyhow!("Missing runbook-id argument"))?;

        let mut service = RunbookService::new(pool);
        let preview = service.preview(runbook_id).await?;

        let entity_footprint: Vec<serde_json::Value> = preview
            .entity_footprint
            .iter()
            .map(|e| {
                serde_json::json!({
                    "entity_id": e.entity_id,
                    "entity_name": e.entity_name,
                    "commands": e.commands,
                    "operations": e.operations,
                })
            })
            .collect();

        let blockers: Vec<serde_json::Value> = preview
            .blockers
            .iter()
            .map(|b| {
                serde_json::json!({
                    "command_id": b.command_id,
                    "source_order": b.source_order,
                    "status": b.status.to_db(),
                    "error": b.error,
                })
            })
            .collect();

        Ok(ExecutionResult::Record(serde_json::json!({
            "is_ready": preview.is_ready,
            "command_count": preview.runbook.commands.len(),
            "entity_footprint": entity_footprint,
            "blockers": blockers,
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("runbook.preview requires database feature"))
    }
}

// ============================================================================
// runbook.remove - Remove a staged command
// ============================================================================

/// Remove a command from the runbook
///
/// Rationale: Requires cascade delete of entity refs and candidates,
/// plus DAG reordering of dependent commands.
#[register_custom_op]
pub struct RunbookRemoveOp;

#[async_trait]
impl CustomOperation for RunbookRemoveOp {
    fn domain(&self) -> &'static str {
        "runbook"
    }
    fn verb(&self) -> &'static str {
        "remove"
    }
    fn rationale(&self) -> &'static str {
        "Requires cascade delete of entity refs/candidates and DAG reordering"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use crate::repl::service::RunbookService;

        let command_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "command-id")
            .and_then(|a| a.value.as_uuid())
            .ok_or_else(|| anyhow::anyhow!("Missing command-id argument"))?;

        let mut service = RunbookService::new(pool);
        let removed_ids = service.remove(command_id).await?;

        Ok(ExecutionResult::Record(serde_json::json!({
            "removed_command_ids": removed_ids,
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("runbook.remove requires database feature"))
    }
}

// ============================================================================
// runbook.abort - Abort and clear runbook
// ============================================================================

/// Abort the runbook and clear all staged commands
///
/// Rationale: Requires status update and cascade cleanup of all
/// related records with audit trail.
#[register_custom_op]
pub struct RunbookAbortOp;

#[async_trait]
impl CustomOperation for RunbookAbortOp {
    fn domain(&self) -> &'static str {
        "runbook"
    }
    fn verb(&self) -> &'static str {
        "abort"
    }
    fn rationale(&self) -> &'static str {
        "Requires status update and cascade cleanup with audit trail"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use crate::repl::service::RunbookService;

        let runbook_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "runbook-id")
            .and_then(|a| a.value.as_uuid())
            .ok_or_else(|| anyhow::anyhow!("Missing runbook-id argument"))?;

        let mut service = RunbookService::new(pool);
        let success = service.abort(runbook_id).await?;

        Ok(ExecutionResult::Record(serde_json::json!({
            "success": success,
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("runbook.abort requires database feature"))
    }
}
