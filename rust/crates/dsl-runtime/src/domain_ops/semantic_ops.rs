//! Semantic Stage Operations.
//!
//! Onboarding-journey progress tracking. These ops derive semantic state
//! from existing entities (the entities are the truth; this interprets
//! them for agent decision support) and query the ontology's stage
//! catalogue.
//!
//! All six ops delegate to the platform
//! [`crate::service_traits::SemanticStateService`] obtained from
//! [`crate::VerbExecutionContext::service`]. The ob-poc host registers the
//! production impl at startup.

use anyhow::Result;
use async_trait::async_trait;
use dsl_runtime_macros::register_custom_op;
use serde_json::json;
use sqlx::PgPool;

use crate::custom_op::CustomOperation;
use crate::domain_ops::helpers::{json_extract_string, json_extract_uuid};
use crate::execution::{VerbExecutionContext, VerbExecutionOutcome};
use crate::service_traits::SemanticStateService;

// ----------------------------------------------------------------------------
// semantic.get-state
// ----------------------------------------------------------------------------

/// Get full semantic state for a CBU — shows stage progress, gaps, and blockers.
#[register_custom_op]
pub struct SemanticStateOp;

#[async_trait]
impl CustomOperation for SemanticStateOp {
    fn domain(&self) -> &'static str {
        "semantic"
    }
    fn verb(&self) -> &'static str {
        "get-state"
    }
    fn rationale(&self) -> &'static str {
        "Requires multi-table entity existence checks and stage dependency computation"
    }

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        _pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let state = ctx.service::<dyn SemanticStateService>()?.derive(cbu_id).await?;
        Ok(VerbExecutionOutcome::Record(serde_json::to_value(&state)?))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

// ----------------------------------------------------------------------------
// semantic.list-stages
// ----------------------------------------------------------------------------

#[register_custom_op]
pub struct SemanticListStagesOp;

#[async_trait]
impl CustomOperation for SemanticListStagesOp {
    fn domain(&self) -> &'static str {
        "semantic"
    }
    fn verb(&self) -> &'static str {
        "list-stages"
    }
    fn rationale(&self) -> &'static str {
        "Returns static configuration - no database needed but consistent with plugin pattern"
    }

    async fn execute_json(
        &self,
        _args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        _pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let service = ctx.service::<dyn SemanticStateService>()?;
        let stages: Vec<serde_json::Value> = service
            .list_stages()
            .into_iter()
            .map(|stage| {
                json!({
                    "code": stage.code,
                    "name": stage.name,
                    "description": stage.description,
                    "required_entities": stage.required_entities,
                    "depends_on": stage.depends_on,
                    "blocking": stage.blocking,
                })
            })
            .collect();
        Ok(VerbExecutionOutcome::Record(json!({ "stages": stages })))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

// ----------------------------------------------------------------------------
// semantic.stages-for-product
// ----------------------------------------------------------------------------

#[register_custom_op]
pub struct SemanticStagesForProductOp;

#[async_trait]
impl CustomOperation for SemanticStagesForProductOp {
    fn domain(&self) -> &'static str {
        "semantic"
    }
    fn verb(&self) -> &'static str {
        "stages-for-product"
    }
    fn rationale(&self) -> &'static str {
        "Returns static configuration lookup based on product code"
    }

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        _pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let product = json_extract_string(args, "product")?;
        let service = ctx.service::<dyn SemanticStateService>()?;
        let stage_codes = service.stages_for_product(&product);
        let stages: Vec<serde_json::Value> = stage_codes
            .iter()
            .filter_map(|code| service.get_stage(code))
            .map(|stage| {
                json!({
                    "code": stage.code,
                    "name": stage.name,
                    "description": stage.description,
                    "required_entities": stage.required_entities,
                    "blocking": stage.blocking,
                })
            })
            .collect();
        Ok(VerbExecutionOutcome::Record(json!({
            "product": product,
            "stages": stages,
        })))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

// ----------------------------------------------------------------------------
// semantic.missing-entities
// ----------------------------------------------------------------------------

#[register_custom_op]
pub struct SemanticMissingEntitiesOp;

#[async_trait]
impl CustomOperation for SemanticMissingEntitiesOp {
    fn domain(&self) -> &'static str {
        "semantic"
    }
    fn verb(&self) -> &'static str {
        "missing-entities"
    }
    fn rationale(&self) -> &'static str {
        "Filters semantic state to just missing entities for action prompts"
    }

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        _pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let state = ctx.service::<dyn SemanticStateService>()?.derive(cbu_id).await?;
        let missing: Vec<serde_json::Value> = state
            .missing_entities
            .iter()
            .map(|m| {
                json!({
                    "stage": m.stage,
                    "stage_name": m.stage_name,
                    "entity_type": m.entity_type,
                    "semantic_purpose": m.semantic_purpose,
                })
            })
            .collect();
        Ok(VerbExecutionOutcome::Record(json!({
            "cbu_id": cbu_id,
            "missing": missing,
        })))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

// ----------------------------------------------------------------------------
// semantic.next-actions
// ----------------------------------------------------------------------------

#[register_custom_op]
pub struct SemanticNextActionsOp;

#[async_trait]
impl CustomOperation for SemanticNextActionsOp {
    fn domain(&self) -> &'static str {
        "semantic"
    }
    fn verb(&self) -> &'static str {
        "next-actions"
    }
    fn rationale(&self) -> &'static str {
        "Extracts the highest-priority next stages from derived state"
    }

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        _pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let state = ctx.service::<dyn SemanticStateService>()?.derive(cbu_id).await?;
        let next: Vec<serde_json::Value> = state
            .next_actionable
            .iter()
            .map(|stage| json!({ "code": stage }))
            .collect();
        Ok(VerbExecutionOutcome::Record(json!({
            "cbu_id": cbu_id,
            "next_actions": next,
        })))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

// ----------------------------------------------------------------------------
// semantic.prompt-context
// ----------------------------------------------------------------------------

#[register_custom_op]
pub struct SemanticPromptContextOp;

#[async_trait]
impl CustomOperation for SemanticPromptContextOp {
    fn domain(&self) -> &'static str {
        "semantic"
    }
    fn verb(&self) -> &'static str {
        "prompt-context"
    }
    fn rationale(&self) -> &'static str {
        "Assembles structured context string for agent prompts from derived state"
    }

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        _pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let state = ctx.service::<dyn SemanticStateService>()?.derive(cbu_id).await?;
        let summary = format!(
            "CBU {} — progress: {}/{} stages complete. Next: {}",
            cbu_id,
            state.overall_progress.stages_complete,
            state.overall_progress.stages_total,
            state
                .next_actionable
                .first()
                .map(String::as_str)
                .unwrap_or("(none — workflow complete or blocked)"),
        );
        Ok(VerbExecutionOutcome::Record(json!({
            "cbu_id": cbu_id,
            "context": summary,
            "progress": {
                "stages_total": state.overall_progress.stages_total,
                "stages_complete": state.overall_progress.stages_complete,
                "percentage": state.overall_progress.percentage,
            },
        })))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}
