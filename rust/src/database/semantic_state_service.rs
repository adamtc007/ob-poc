//! Semantic State Derivation Service
//!
//! Derives the semantic onboarding state for a CBU by querying existing entities.
//! This is a session-time view - computed on demand, not persisted.
//!
//! The semantic state helps the agent answer "where are we in the onboarding journey?"
//! by computing which stages are complete, in progress, or blocked.

use ob_poc_types::semantic_stage::{
    EntityStatus, MissingEntity, Progress, SemanticState, StageStatus, StageWithStatus,
};
use sqlx::PgPool;
use std::collections::HashMap;
use uuid::Uuid;

use crate::ontology::SemanticStageRegistry;

/// Derive the semantic state for a CBU
pub async fn derive_semantic_state(
    pool: &PgPool,
    registry: &SemanticStageRegistry,
    cbu_id: Uuid,
) -> Result<SemanticState, sqlx::Error> {
    // 1. Get CBU info
    let cbu = sqlx::query!(
        r#"SELECT name FROM "ob-poc".cbus WHERE cbu_id = $1"#,
        cbu_id
    )
    .fetch_one(pool)
    .await?;

    // 2. Get products for this CBU
    let product_rows = sqlx::query!(
        r#"
        SELECT p.product_code
        FROM "ob-poc".cbu_product_subscriptions cps
        JOIN "ob-poc".products p ON p.product_id = cps.product_id
        WHERE cps.cbu_id = $1 AND cps.status = 'ACTIVE'
        "#,
        cbu_id
    )
    .fetch_all(pool)
    .await?;

    let products: Vec<String> = product_rows
        .into_iter()
        .filter_map(|r| r.product_code)
        .collect();

    // 3. Determine required stages from products
    let required_stage_codes = registry.stages_for_products(&products);

    // 4. Query existing entities for this CBU
    let existing = query_existing_entities(pool, cbu_id).await?;

    // 5. Compute stage statuses
    let stage_statuses =
        compute_stage_statuses(registry, &required_stage_codes, &existing, &products);

    // 6. Find next actionable stages (dependencies met, not complete)
    let next_actionable = find_next_actionable(&stage_statuses, registry);

    // 7. Find blocking stages
    let blocking_stages: Vec<String> = stage_statuses
        .iter()
        .filter(|s| s.is_blocking && s.status != StageStatus::Complete)
        .map(|s| s.code.clone())
        .collect();

    // 8. Compute missing entities
    let missing_entities = compute_missing_entities(&stage_statuses);

    // 9. Compute progress
    let stages_complete = stage_statuses
        .iter()
        .filter(|s| s.status == StageStatus::Complete)
        .count();
    let stages_total = stage_statuses.len();
    let percentage = if stages_total > 0 {
        (stages_complete as f32 / stages_total as f32) * 100.0
    } else {
        0.0
    };

    Ok(SemanticState {
        cbu_id,
        cbu_name: cbu.name,
        products,
        required_stages: stage_statuses,
        overall_progress: Progress {
            stages_complete,
            stages_total,
            percentage,
        },
        next_actionable,
        blocking_stages,
        missing_entities,
    })
}

/// Query existing entities for a CBU, organized by entity type
async fn query_existing_entities(
    pool: &PgPool,
    cbu_id: Uuid,
) -> Result<HashMap<String, Vec<Uuid>>, sqlx::Error> {
    let mut existing: HashMap<String, Vec<Uuid>> = HashMap::new();

    // CBU itself always exists if we got here
    existing.insert("cbu".to_string(), vec![cbu_id]);

    // Product subscriptions
    let subscriptions = sqlx::query_scalar!(
        r#"SELECT subscription_id FROM "ob-poc".cbu_product_subscriptions
           WHERE cbu_id = $1 AND status = 'ACTIVE'"#,
        cbu_id
    )
    .fetch_all(pool)
    .await?;
    if !subscriptions.is_empty() {
        existing.insert("cbu_product_subscription".to_string(), subscriptions);
    }

    // KYC cases
    let cases = sqlx::query_scalar!(r#"SELECT case_id FROM kyc.cases WHERE cbu_id = $1"#, cbu_id)
        .fetch_all(pool)
        .await?;
    if !cases.is_empty() {
        let case_ids = cases.clone();
        existing.insert("kyc_case".to_string(), cases);

        // Entity workstreams (linked via kyc_case)
        if !case_ids.is_empty() {
            let workstreams = sqlx::query_scalar!(
                r#"SELECT workstream_id FROM kyc.entity_workstreams
                   WHERE case_id = ANY($1)"#,
                &case_ids
            )
            .fetch_all(pool)
            .await?;
            if !workstreams.is_empty() {
                existing.insert("entity_workstream".to_string(), workstreams);
            }
        }
    }

    // Trading profile
    let profiles = sqlx::query_scalar!(
        r#"SELECT profile_id FROM "ob-poc".cbu_trading_profiles WHERE cbu_id = $1"#,
        cbu_id
    )
    .fetch_all(pool)
    .await?;
    if !profiles.is_empty() {
        existing.insert("trading_profile".to_string(), profiles);
    }

    // Instrument universe entries
    let universe = sqlx::query_scalar!(
        r#"SELECT universe_id FROM custody.cbu_instrument_universe WHERE cbu_id = $1"#,
        cbu_id
    )
    .fetch_all(pool)
    .await?;
    if !universe.is_empty() {
        existing.insert("cbu_instrument_universe".to_string(), universe);
    }

    // SSIs
    let ssis = sqlx::query_scalar!(
        r#"SELECT ssi_id FROM custody.cbu_ssi WHERE cbu_id = $1"#,
        cbu_id
    )
    .fetch_all(pool)
    .await?;
    if !ssis.is_empty() {
        let ssi_ids = ssis.clone();
        existing.insert("cbu_ssi".to_string(), ssis);

        // Booking rules (linked via SSI)
        let rules = sqlx::query_scalar!(
            r#"SELECT rule_id FROM custody.ssi_booking_rules
               WHERE ssi_id = ANY($1)"#,
            &ssi_ids
        )
        .fetch_all(pool)
        .await?;
        if !rules.is_empty() {
            existing.insert("ssi_booking_rule".to_string(), rules);
        }
    }

    // ISDA agreements
    let isdas = sqlx::query_scalar!(
        r#"SELECT isda_id FROM custody.isda_agreements WHERE cbu_id = $1 AND is_active = true"#,
        cbu_id
    )
    .fetch_all(pool)
    .await?;
    if !isdas.is_empty() {
        let isda_ids = isdas.clone();
        existing.insert("isda_agreement".to_string(), isdas);

        // CSA agreements (linked via ISDA)
        let csas = sqlx::query_scalar!(
            r#"SELECT csa_id FROM custody.csa_agreements
               WHERE isda_id = ANY($1)"#,
            &isda_ids
        )
        .fetch_all(pool)
        .await?;
        if !csas.is_empty() {
            existing.insert("csa_agreement".to_string(), csas);
        }
    }

    // Resource instances (lifecycle resources)
    let instances = sqlx::query_scalar!(
        r#"SELECT instance_id FROM "ob-poc".cbu_resource_instances
           WHERE cbu_id = $1 AND status IN ('ACTIVE', 'PENDING')"#,
        cbu_id
    )
    .fetch_all(pool)
    .await?;
    if !instances.is_empty() {
        existing.insert("cbu_resource_instance".to_string(), instances.clone());
        existing.insert("cbu_lifecycle_instance".to_string(), instances);
    }

    // Pricing configs
    let pricing = sqlx::query_scalar!(
        r#"SELECT config_id FROM custody.cbu_pricing_config WHERE cbu_id = $1"#,
        cbu_id
    )
    .fetch_all(pool)
    .await?;
    if !pricing.is_empty() {
        existing.insert("cbu_pricing_config".to_string(), pricing);
    }

    // Share classes (for transfer agency)
    let share_classes = sqlx::query_scalar!(
        r#"SELECT id FROM kyc.share_classes WHERE cbu_id = $1"#,
        cbu_id
    )
    .fetch_all(pool)
    .await?;
    if !share_classes.is_empty() {
        let class_ids = share_classes.clone();
        existing.insert("share_class".to_string(), share_classes);

        // Holdings (linked via share class)
        let holdings = sqlx::query_scalar!(
            r#"SELECT id FROM kyc.holdings
               WHERE share_class_id = ANY($1)"#,
            &class_ids
        )
        .fetch_all(pool)
        .await?;
        if !holdings.is_empty() {
            existing.insert("holding".to_string(), holdings);
        }
    }

    Ok(existing)
}

/// Compute the status of each required stage
fn compute_stage_statuses(
    registry: &SemanticStageRegistry,
    required_stage_codes: &[&str],
    existing: &HashMap<String, Vec<Uuid>>,
    _products: &[String],
) -> Vec<StageWithStatus> {
    // First pass: compute basic status for each stage
    let mut statuses: Vec<StageWithStatus> = required_stage_codes
        .iter()
        .filter_map(|code| {
            let stage_def = registry.get_stage(code)?;

            // Get entity statuses for this stage
            let entity_statuses: Vec<EntityStatus> = stage_def
                .required_entities
                .iter()
                .map(|entity_type| {
                    let ids = existing.get(entity_type).cloned().unwrap_or_default();
                    EntityStatus {
                        entity_type: entity_type.clone(),
                        required: true,
                        exists: !ids.is_empty(),
                        count: ids.len(),
                        ids,
                    }
                })
                .collect();

            // Compute basic status (will refine for blocked in second pass)
            let all_exist = entity_statuses.iter().all(|e| e.exists);
            let any_exist = entity_statuses.iter().any(|e| e.exists);

            let status = if all_exist {
                StageStatus::Complete
            } else if any_exist {
                StageStatus::InProgress
            } else {
                StageStatus::NotStarted
            };

            Some(StageWithStatus {
                code: code.to_string(),
                name: stage_def.name.clone(),
                description: stage_def.description.clone(),
                status,
                required_entities: entity_statuses,
                is_blocking: stage_def.blocking,
            })
        })
        .collect();

    // Second pass: mark stages as Blocked if dependencies not met
    let status_map: HashMap<String, StageStatus> = statuses
        .iter()
        .map(|s| (s.code.clone(), s.status.clone()))
        .collect();

    for stage in &mut statuses {
        if let Some(stage_def) = registry.get_stage(&stage.code) {
            // Check if all dependencies are complete
            let deps_met = stage_def.depends_on.iter().all(|dep| {
                status_map
                    .get(dep)
                    .map(|s| *s == StageStatus::Complete)
                    .unwrap_or(true) // If dep not in required stages, consider met
            });

            if !deps_met && stage.status != StageStatus::Complete {
                stage.status = StageStatus::Blocked;
            }
        }
    }

    statuses
}

/// Find stages that can be worked on next
fn find_next_actionable(
    stages: &[StageWithStatus],
    registry: &SemanticStageRegistry,
) -> Vec<String> {
    let status_map: HashMap<&str, &StageStatus> = stages
        .iter()
        .map(|s| (s.code.as_str(), &s.status))
        .collect();

    stages
        .iter()
        .filter(|s| {
            // Not complete and not blocked
            s.status != StageStatus::Complete && s.status != StageStatus::Blocked
        })
        .filter(|s| {
            // All dependencies are complete
            if let Some(stage_def) = registry.get_stage(&s.code) {
                stage_def.depends_on.iter().all(|dep| {
                    status_map
                        .get(dep.as_str())
                        .map(|status| **status == StageStatus::Complete)
                        .unwrap_or(true)
                })
            } else {
                true
            }
        })
        .map(|s| s.code.clone())
        .collect()
}

/// Compute missing entities across all stages
fn compute_missing_entities(stages: &[StageWithStatus]) -> Vec<MissingEntity> {
    stages
        .iter()
        .filter(|s| s.status != StageStatus::Complete && s.status != StageStatus::NotRequired)
        .flat_map(|stage| {
            stage
                .required_entities
                .iter()
                .filter(|e| !e.exists)
                .map(|e| MissingEntity {
                    entity_type: e.entity_type.clone(),
                    stage: stage.code.clone(),
                    stage_name: stage.name.clone(),
                    semantic_purpose: stage.description.clone(),
                })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_missing_entities() {
        let stages = vec![
            StageWithStatus {
                code: "KYC_REVIEW".to_string(),
                name: "KYC Review".to_string(),
                description: "Know your customer".to_string(),
                status: StageStatus::NotStarted,
                required_entities: vec![
                    EntityStatus {
                        entity_type: "kyc_case".to_string(),
                        required: true,
                        exists: false,
                        count: 0,
                        ids: vec![],
                    },
                    EntityStatus {
                        entity_type: "entity_workstream".to_string(),
                        required: true,
                        exists: false,
                        count: 0,
                        ids: vec![],
                    },
                ],
                is_blocking: true,
            },
            StageWithStatus {
                code: "CLIENT_SETUP".to_string(),
                name: "Client Setup".to_string(),
                description: "Establish client".to_string(),
                status: StageStatus::Complete,
                required_entities: vec![EntityStatus {
                    entity_type: "cbu".to_string(),
                    required: true,
                    exists: true,
                    count: 1,
                    ids: vec![Uuid::nil()],
                }],
                is_blocking: false,
            },
        ];

        let missing = compute_missing_entities(&stages);
        assert_eq!(missing.len(), 2);
        assert!(missing.iter().any(|m| m.entity_type == "kyc_case"));
        assert!(missing.iter().any(|m| m.entity_type == "entity_workstream"));
    }
}
