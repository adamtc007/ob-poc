//! Semantic Stage Operations
//!
//! These operations provide onboarding journey progress tracking.
//! They derive semantic state from existing entities - the entities ARE the truth,
//! this just helps interpret that truth for agent decision support.
//!
//! Key concept: This is NOT a workflow engine. It's a session-time view that
//! helps the agent understand "where we are" in the onboarding journey.

use anyhow::Result;
use async_trait::async_trait;

use crate::dsl_v2::ast::VerbCall;
use crate::domain_ops::CustomOperation;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};

#[cfg(feature = "database")]
use sqlx::PgPool;

#[cfg(feature = "database")]
use crate::database::derive_semantic_state;
#[cfg(feature = "database")]
use crate::ontology::SemanticStageRegistry;

// For non-database builds
#[cfg(not(feature = "database"))]
use crate::ontology::SemanticStageRegistry;

/// Get full semantic state for a CBU - shows stage progress, gaps, and blockers
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

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use uuid::Uuid;

        let cbu_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "cbu-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing cbu-id argument"))?;

        // Load the semantic stage registry
        let registry = SemanticStageRegistry::load_default()
            .map_err(|e| anyhow::anyhow!("Failed to load semantic stage map: {}", e))?;

        // Derive the semantic state
        let state = derive_semantic_state(pool, &registry, cbu_id).await?;

        // Serialize to JSON
        let result = serde_json::to_value(&state)?;

        Ok(ExecutionResult::Record(result))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "semantic.get-state requires database feature"
        ))
    }
}

/// List all defined semantic stages with their dependencies
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

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use serde_json::json;

        let registry = SemanticStageRegistry::load_default()
            .map_err(|e| anyhow::anyhow!("Failed to load semantic stage map: {}", e))?;

        let stages: Vec<serde_json::Value> = registry
            .stages_in_order()
            .map(|stage| {
                json!({
                    "code": stage.code,
                    "name": stage.name,
                    "description": stage.description,
                    "required_entities": stage.required_entities,
                    "depends_on": stage.depends_on,
                    "blocking": stage.blocking
                })
            })
            .collect();

        Ok(ExecutionResult::Record(json!({ "stages": stages })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        use serde_json::json;

        let registry = SemanticStageRegistry::load_default()
            .map_err(|e| anyhow::anyhow!("Failed to load semantic stage map: {}", e))?;

        let stages: Vec<serde_json::Value> = registry
            .stages_in_order()
            .map(|stage| {
                json!({
                    "code": stage.code,
                    "name": stage.name,
                    "description": stage.description,
                    "required_entities": stage.required_entities,
                    "depends_on": stage.depends_on,
                    "blocking": stage.blocking
                })
            })
            .collect();

        Ok(ExecutionResult::Record(json!({ "stages": stages })))
    }
}

/// Get required stages for a specific product
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

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use serde_json::json;

        let product = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "product")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing product argument"))?;

        let registry = SemanticStageRegistry::load_default()
            .map_err(|e| anyhow::anyhow!("Failed to load semantic stage map: {}", e))?;

        let stage_codes = registry.stages_for_product(product);

        let stages: Vec<serde_json::Value> = stage_codes
            .iter()
            .filter_map(|code| registry.get_stage(code))
            .map(|stage| {
                json!({
                    "code": stage.code,
                    "name": stage.name,
                    "description": stage.description,
                    "required_entities": stage.required_entities,
                    "blocking": stage.blocking
                })
            })
            .collect();

        Ok(ExecutionResult::Record(json!({
            "product": product,
            "stages": stages
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        use serde_json::json;

        let product = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "product")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing product argument"))?;

        let registry = SemanticStageRegistry::load_default()
            .map_err(|e| anyhow::anyhow!("Failed to load semantic stage map: {}", e))?;

        let stage_codes = registry.stages_for_product(&product);

        let stages: Vec<serde_json::Value> = stage_codes
            .iter()
            .filter_map(|code| registry.get_stage(code))
            .map(|stage| {
                json!({
                    "code": stage.code,
                    "name": stage.name,
                    "description": stage.description,
                    "required_entities": stage.required_entities,
                    "blocking": stage.blocking
                })
            })
            .collect();

        Ok(ExecutionResult::Record(json!({
            "product": product,
            "stages": stages
        })))
    }
}

/// Get next actionable stages for a CBU (unblocked, incomplete stages)
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
        "Derives next actions from entity state - requires database queries"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use serde_json::json;
        use uuid::Uuid;

        let cbu_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "cbu-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing cbu-id argument"))?;

        let registry = SemanticStageRegistry::load_default()
            .map_err(|e| anyhow::anyhow!("Failed to load semantic stage map: {}", e))?;

        let state = derive_semantic_state(pool, &registry, cbu_id).await?;

        // Build actionable items with suggested verbs
        let actions: Vec<serde_json::Value> = state
            .next_actionable
            .iter()
            .filter_map(|code| {
                registry.get_stage(code).map(|stage| {
                    json!({
                        "stage_code": code,
                        "stage_name": stage.name,
                        "required_entities": stage.required_entities,
                        "suggested_verbs": get_suggested_verbs(&stage.required_entities)
                    })
                })
            })
            .collect();

        Ok(ExecutionResult::Record(json!({
            "cbu_id": cbu_id,
            "cbu_name": state.cbu_name,
            "next_actions": actions,
            "blocking_stages": state.blocking_stages
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "semantic.next-actions requires database feature"
        ))
    }
}

/// Get missing entities for stage completion
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
        "Requires entity existence checks across multiple tables"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use serde_json::json;
        use uuid::Uuid;

        let cbu_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "cbu-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing cbu-id argument"))?;

        let stage_filter = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "stage")
            .and_then(|a| a.value.as_string());

        let registry = SemanticStageRegistry::load_default()
            .map_err(|e| anyhow::anyhow!("Failed to load semantic stage map: {}", e))?;

        let state = derive_semantic_state(pool, &registry, cbu_id).await?;

        // Filter missing entities by stage if specified
        let missing: Vec<&ob_poc_types::semantic_stage::MissingEntity> =
            if let Some(ref stage) = stage_filter {
                state
                    .missing_entities
                    .iter()
                    .filter(|e| &e.stage == stage)
                    .collect()
            } else {
                state.missing_entities.iter().collect()
            };

        let result: Vec<serde_json::Value> = missing
            .iter()
            .map(|e| {
                json!({
                    "entity_type": e.entity_type,
                    "stage": e.stage,
                    "stage_name": e.stage_name,
                    "semantic_purpose": e.semantic_purpose,
                    "suggested_verb": get_creation_verb(&e.entity_type)
                })
            })
            .collect();

        Ok(ExecutionResult::Record(json!({
            "cbu_id": cbu_id,
            "cbu_name": state.cbu_name,
            "missing_entities": result
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "semantic.missing-entities requires database feature"
        ))
    }
}

/// Get semantic state formatted for agent prompt injection
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
        "Formats semantic state for agent system prompt - requires full state derivation"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use serde_json::json;
        use uuid::Uuid;

        let cbu_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "cbu-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing cbu-id argument"))?;

        let registry = SemanticStageRegistry::load_default()
            .map_err(|e| anyhow::anyhow!("Failed to load semantic stage map: {}", e))?;

        let state = derive_semantic_state(pool, &registry, cbu_id).await?;

        // Use the built-in to_prompt_context method
        let prompt_context = state.to_prompt_context();

        Ok(ExecutionResult::Record(json!({
            "prompt_context": prompt_context
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "semantic.prompt-context requires database feature"
        ))
    }
}

// Helper functions

/// Get suggested DSL verbs for creating entities of given types
fn get_suggested_verbs(entity_types: &[String]) -> Vec<String> {
    entity_types
        .iter()
        .filter_map(|et| get_creation_verb(et))
        .collect()
}

/// Get the DSL verb for creating a specific entity type
fn get_creation_verb(entity_type: &str) -> Option<String> {
    match entity_type {
        "cbu" => Some("cbu.ensure".to_string()),
        "cbu_product" => Some("cbu.add-product".to_string()),
        "kyc_case" => Some("kyc-case.create".to_string()),
        "entity_workstream" => Some("entity-workstream.create".to_string()),
        "trading_profile" => Some("trading-profile.import".to_string()),
        "cbu_instrument_universe" => Some("cbu-custody.add-universe".to_string()),
        "cbu_ssi" => Some("cbu-custody.create-ssi".to_string()),
        "ssi_booking_rule" => Some("cbu-custody.add-booking-rule".to_string()),
        "isda_agreement" => Some("isda.create".to_string()),
        "csa_agreement" => Some("isda.add-csa".to_string()),
        "cbu_resource_instance" => Some("service-resource.provision".to_string()),
        "cbu_lifecycle_instance" => Some("lifecycle.provision".to_string()),
        "cbu_pricing_config" => Some("pricing-config.set".to_string()),
        "share_class" => Some("share-class.create".to_string()),
        "holding" => Some("holding.create".to_string()),
        _ => None,
    }
}
