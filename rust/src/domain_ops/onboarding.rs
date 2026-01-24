//! Onboarding Workflow Operations
//!
//! Auto-complete operation for CBU onboarding that:
//! 1. Derives semantic state to find missing entities
//! 2. Generates DSL statements to create them
//! 3. Executes DSL iteratively until complete or blocked
//!
//! Rationale: Requires semantic state derivation, DSL generation, and multi-step
//! orchestration that cannot be expressed as simple CRUD operations.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use ob_poc_macros::register_custom_op;
use uuid::Uuid;

use super::helpers::extract_uuid;
use super::{CustomOperation, ExecutionContext, ExecutionResult, VerbCall};

#[cfg(feature = "database")]
use sqlx::PgPool;

// =============================================================================
// AUTO-COMPLETE OPERATION
// =============================================================================

/// Auto-complete onboarding by iteratively creating missing entities
///
/// This operation derives semantic state, finds missing entities, generates DSL
/// to create them, and optionally executes. It's an "auto-pilot" for onboarding.
#[register_custom_op]
pub struct OnboardingAutoCompleteOp;

/// Result of a single auto-complete step
#[derive(Debug, Clone, serde::Serialize)]
pub struct AutoCompleteStep {
    pub entity_type: String,
    pub stage: String,
    pub dsl: String,
    pub executed: bool,
    pub success: bool,
    pub error: Option<String>,
    pub created_id: Option<Uuid>,
}

/// Result of the auto-complete operation
#[derive(Debug, Clone, serde::Serialize)]
pub struct AutoCompleteResult {
    pub steps_executed: usize,
    pub steps_succeeded: usize,
    pub steps_failed: usize,
    pub steps: Vec<AutoCompleteStep>,
    pub remaining_missing: Vec<String>,
    pub target_reached: bool,
    pub dry_run: bool,
}

#[cfg(feature = "database")]
impl OnboardingAutoCompleteOp {
    /// Generate DSL for creating a missing entity
    fn generate_entity_dsl(
        cbu_id: Uuid,
        entity_type: &str,
        existing: &std::collections::HashMap<String, Vec<Uuid>>,
    ) -> Option<String> {
        match entity_type {
            "kyc_case" => Some(format!(
                r#"(kyc-case.create :cbu-id "{}" :case-type "NEW_CLIENT" :as @case)"#,
                cbu_id
            )),

            "entity_workstream" => {
                // Need a case_id - get from existing if available
                let case_id = existing.get("kyc_case").and_then(|ids| ids.first())?;
                Some(format!(
                    r#"; Entity workstream requires entity selection
(entity-workstream.create :case-id "{}" :entity-id <select-entity> :as @workstream)"#,
                    case_id
                ))
            }

            "trading_profile" => Some(format!(
                r#"(trading-profile.import :cbu-id "{}" :profile-path "config/seed/trading_profiles/default.yaml" :as @profile)"#,
                cbu_id
            )),

            "cbu_instrument_universe" => Some(format!(
                r#"(trading-profile.add-instrument-class :cbu-id "{}" :class-code "EQUITY")
(trading-profile.add-market :cbu-id "{}" :market-code "XNYS" :currencies ["USD"])"#,
                cbu_id, cbu_id
            )),

            "cbu_ssi" => Some(format!(
                r#"(trading-profile.add-standing-instruction :cbu-id "{}" :name "Default SSI" :type "SECURITIES" :safekeeping-account "SAFE-001" :safekeeping-bic "CUSTUS33" :cash-account "CASH-001" :cash-bic "CUSTUS33" :cash-currency "USD" :pset-bic "DTCYUS33" :effective-date "2024-01-01" :as @ssi)"#,
                cbu_id
            )),

            "ssi_booking_rule" => {
                let ssi_id = existing.get("cbu_ssi").and_then(|ids| ids.first())?;
                Some(format!(
                    r#"(trading-profile.add-booking-rule :cbu-id "{}" :ssi-id "{}" :name "Default Rule" :priority 100)"#,
                    cbu_id, ssi_id
                ))
            }

            "isda_agreement" => Some(format!(
                r#"; ISDA requires counterparty selection
(isda.create :cbu-id "{}" :counterparty-id <select-counterparty> :governing-law "NY" :agreement-date "2024-01-01" :as @isda)"#,
                cbu_id
            )),

            "csa_agreement" => {
                let isda_id = existing.get("isda_agreement").and_then(|ids| ids.first())?;
                Some(format!(
                    r#"(isda.add-csa :isda-id "{}" :csa-type "VM" :threshold-amount 0 :minimum-transfer 500000 :as @csa)"#,
                    isda_id
                ))
            }

            "cbu_resource_instance" | "cbu_lifecycle_instance" => Some(format!(
                r#"(lifecycle.provision :cbu-id "{}" :lifecycle-code "CUSTODY_ONBOARD" :as @lifecycle)"#,
                cbu_id
            )),

            "cbu_pricing_config" => Some(format!(
                r#"(pricing-config.set :cbu-id "{}" :instrument-class "EQUITY" :source "BLOOMBERG" :priority 10)"#,
                cbu_id
            )),

            "share_class" => Some(format!(
                r#"(share-class.create :cbu-id "{}" :name "Class A" :currency "USD" :class-category "FUND" :as @share_class)"#,
                cbu_id
            )),

            "holding" => {
                let share_class_id = existing.get("share_class").and_then(|ids| ids.first())?;
                Some(format!(
                    r#"; Holding requires investor entity selection
(holding.create :share-class-id "{}" :investor-entity-id <select-investor> :as @holding)"#,
                    share_class_id
                ))
            }

            _ => None,
        }
    }
}

#[async_trait]
impl CustomOperation for OnboardingAutoCompleteOp {
    fn domain(&self) -> &'static str {
        "onboarding"
    }
    fn verb(&self) -> &'static str {
        "auto-complete"
    }
    fn rationale(&self) -> &'static str {
        "Requires semantic state derivation, DSL generation, and iterative execution"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use crate::database::derive_semantic_state;
        use crate::ontology::SemanticStageRegistry;

        // Extract arguments
        let cbu_id = extract_uuid(verb_call, ctx, "cbu-id")?;

        let max_steps: i32 = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "max-steps")
            .and_then(|a| a.value.as_integer())
            .map(|v| v.min(1000) as i32) // Cap at 1000 to prevent runaway
            .unwrap_or(20);

        let dry_run: bool = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "dry-run")
            .and_then(|a| a.value.as_boolean())
            .unwrap_or(false);

        let target_stage: Option<String> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "target-stage")
            .and_then(|a| a.value.as_string().map(|s| s.to_string()));

        tracing::info!(
            cbu_id = %cbu_id,
            max_steps = max_steps,
            dry_run = dry_run,
            target_stage = ?target_stage,
            "onboarding.auto-complete: starting"
        );

        // Load semantic stage registry
        let registry = SemanticStageRegistry::load_default()
            .map_err(|e| anyhow!("Failed to load semantic stage registry: {}", e))?;

        let mut steps: Vec<AutoCompleteStep> = Vec::new();
        let mut steps_executed = 0;
        let mut steps_succeeded = 0;
        let mut steps_failed = 0;

        // Create executor for DSL execution
        let executor = crate::dsl_v2::executor::DslExecutor::new(pool.clone());

        // Iterative loop: derive state, find missing, generate DSL, execute
        for _ in 0..max_steps {
            // Derive current semantic state
            let state = derive_semantic_state(pool, &registry, cbu_id).await?;

            // Check if target stage is complete
            if let Some(ref target) = target_stage {
                let target_complete = state.required_stages.iter().any(|s| {
                    &s.code == target
                        && s.status == ob_poc_types::semantic_stage::StageStatus::Complete
                });
                if target_complete {
                    return Ok(ExecutionResult::Record(serde_json::to_value(
                        AutoCompleteResult {
                            steps_executed,
                            steps_succeeded,
                            steps_failed,
                            steps,
                            remaining_missing: vec![],
                            target_reached: true,
                            dry_run,
                        },
                    )?));
                }
            }

            // Find next missing entity to create
            if state.missing_entities.is_empty() {
                // No more missing entities - we're done
                break;
            }

            // Get the first actionable missing entity
            // Prioritize by stage order (stages that are unblocked first)
            let next_missing = state
                .missing_entities
                .iter()
                .find(|m| state.next_actionable.contains(&m.stage));

            let missing = match next_missing {
                Some(m) => m,
                None => {
                    // No actionable missing entities - might be blocked
                    break;
                }
            };

            // Build existing entities map for DSL generation
            let existing: std::collections::HashMap<String, Vec<Uuid>> = state
                .required_stages
                .iter()
                .flat_map(|s| &s.required_entities)
                .filter(|e| e.exists)
                .map(|e| (e.entity_type.clone(), e.ids.clone()))
                .collect();

            // Generate DSL for this entity
            let dsl = match Self::generate_entity_dsl(cbu_id, &missing.entity_type, &existing) {
                Some(d) => d,
                None => {
                    // Can't generate DSL for this entity type
                    steps.push(AutoCompleteStep {
                        entity_type: missing.entity_type.clone(),
                        stage: missing.stage.clone(),
                        dsl: String::new(),
                        executed: false,
                        success: false,
                        error: Some(format!(
                            "No DSL template for entity type: {}",
                            missing.entity_type
                        )),
                        created_id: None,
                    });
                    steps_failed += 1;
                    continue;
                }
            };

            // Check if DSL contains placeholders that need user input
            if dsl.contains("<select-") {
                steps.push(AutoCompleteStep {
                    entity_type: missing.entity_type.clone(),
                    stage: missing.stage.clone(),
                    dsl: dsl.clone(),
                    executed: false,
                    success: false,
                    error: Some("DSL requires user selection - cannot auto-complete".to_string()),
                    created_id: None,
                });
                // Don't count as failed - it's expected that some entities need user input
                break;
            }

            if dry_run {
                // In dry-run mode, just collect the DSL without executing
                steps.push(AutoCompleteStep {
                    entity_type: missing.entity_type.clone(),
                    stage: missing.stage.clone(),
                    dsl: dsl.clone(),
                    executed: false,
                    success: true,
                    error: None,
                    created_id: None,
                });
                steps_executed += 1;
                steps_succeeded += 1;
            } else {
                // Execute the DSL
                steps_executed += 1;
                let result = executor.execute_dsl(&dsl, ctx).await;

                match result {
                    Ok(_) => {
                        steps_succeeded += 1;
                        steps.push(AutoCompleteStep {
                            entity_type: missing.entity_type.clone(),
                            stage: missing.stage.clone(),
                            dsl: dsl.clone(),
                            executed: true,
                            success: true,
                            error: None,
                            created_id: None, // Could extract from ctx bindings
                        });
                    }
                    Err(e) => {
                        steps_failed += 1;
                        steps.push(AutoCompleteStep {
                            entity_type: missing.entity_type.clone(),
                            stage: missing.stage.clone(),
                            dsl: dsl.clone(),
                            executed: true,
                            success: false,
                            error: Some(e.to_string()),
                            created_id: None,
                        });
                        // Stop on first error to avoid cascading failures
                        break;
                    }
                }
            }
        }

        // Get final state to report remaining missing entities
        let final_state = derive_semantic_state(pool, &registry, cbu_id).await?;
        let remaining_missing: Vec<String> = final_state
            .missing_entities
            .iter()
            .map(|m| format!("{} ({})", m.entity_type, m.stage))
            .collect();

        let target_reached = if let Some(ref target) = target_stage {
            final_state.required_stages.iter().any(|s| {
                &s.code == target && s.status == ob_poc_types::semantic_stage::StageStatus::Complete
            })
        } else {
            remaining_missing.is_empty()
        };

        Ok(ExecutionResult::Record(serde_json::to_value(
            AutoCompleteResult {
                steps_executed,
                steps_succeeded,
                steps_failed,
                steps,
                remaining_missing,
                target_reached,
                dry_run,
            },
        )?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!(
            "onboarding.auto-complete requires database feature"
        ))
    }
}
