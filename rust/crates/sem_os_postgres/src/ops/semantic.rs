//! Semantic-stage verbs — SemOS-side YAML-first re-implementation.
//!
//! 6 ops deriving onboarding-journey state from existing entities and
//! projecting different views of the catalogue. All delegate to
//! [`SemanticStateService`]; each op chooses its own projection shape
//! (full state, stage catalogue, product filter, missing-entity list,
//! next actions, prompt summary). YAML contracts in
//! `config/verbs/semantic.yaml`.

use anyhow::Result;
use async_trait::async_trait;
use serde_json::json;

use dsl_runtime::domain_ops::helpers::{json_extract_string, json_extract_uuid};
use dsl_runtime::service_traits::SemanticStateService;
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::SemOsVerbOp;

pub struct GetState;

#[async_trait]
impl SemOsVerbOp for GetState {
    fn fqn(&self) -> &str {
        "semantic.get-state"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let state = ctx
            .service::<dyn SemanticStateService>()?
            .derive(cbu_id)
            .await?;
        Ok(VerbExecutionOutcome::Record(serde_json::to_value(&state)?))
    }
}

pub struct ListStages;

#[async_trait]
impl SemOsVerbOp for ListStages {
    fn fqn(&self) -> &str {
        "semantic.list-stages"
    }
    async fn execute(
        &self,
        _args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
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
}

pub struct StagesForProduct;

#[async_trait]
impl SemOsVerbOp for StagesForProduct {
    fn fqn(&self) -> &str {
        "semantic.stages-for-product"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
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
}

pub struct MissingEntities;

#[async_trait]
impl SemOsVerbOp for MissingEntities {
    fn fqn(&self) -> &str {
        "semantic.missing-entities"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let state = ctx
            .service::<dyn SemanticStateService>()?
            .derive(cbu_id)
            .await?;
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
}

pub struct NextActions;

#[async_trait]
impl SemOsVerbOp for NextActions {
    fn fqn(&self) -> &str {
        "semantic.next-actions"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let state = ctx
            .service::<dyn SemanticStateService>()?
            .derive(cbu_id)
            .await?;
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
}

pub struct PromptContext;

#[async_trait]
impl SemOsVerbOp for PromptContext {
    fn fqn(&self) -> &str {
        "semantic.prompt-context"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let state = ctx
            .service::<dyn SemanticStateService>()?
            .derive(cbu_id)
            .await?;
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
}
