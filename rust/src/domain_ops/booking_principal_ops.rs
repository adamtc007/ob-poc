//! Booking Principal Operations (32 plugin verbs) — `legal-entity.*`,
//! `rule-field.*`, `booking-location.*`, `booking-principal.*`,
//! `client-principal-relationship.*`, `service-availability.*`, `ruleset.*`,
//! `rule.*`, `contract-pack.*`.
//!
//! Operations for booking principal selection, eligibility evaluation,
//! client-principal relationship management, and coverage analysis.
//!
//! The booking principal selection capability determines "who can contract what,
//! for whom, where" through a rule-driven, boundary-aware evaluation pipeline.
//!
//! Phase 5c-migrate Phase B Pattern B slice #78: ported from
//! `CustomOperation` + `inventory::collect!` to `SemOsVerbOp`. Stays in
//! `ob-poc::domain_ops::booking_principal_ops` because the ops bridge to
//! `crate::database::booking_principal_repository::BookingPrincipalRepository`
//! and `crate::domain_ops::rule_evaluator` (both upstream of
//! `sem_os_postgres`) and depend on the shared
//! `crate::api::booking_principal_types::*` result/context types.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use sem_os_postgres::ops::SemOsVerbOp;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use dsl_runtime::domain_ops::helpers::{
    json_extract_bool_opt, json_extract_int_opt, json_extract_string, json_extract_string_opt,
    json_extract_uuid, json_extract_uuid_opt,
};
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use crate::api::booking_principal_types::*;

#[cfg(feature = "database")]
use crate::database::booking_principal_repository::BookingPrincipalRepository;
#[cfg(feature = "database")]
use crate::domain_ops::rule_evaluator;

// =============================================================================
// Result Types
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegalEntityCreateResult {
    pub legal_entity_id: Uuid,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookingLocationCreateResult {
    pub booking_location_id: Uuid,
    pub country_code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookingPrincipalCreateResult {
    pub booking_principal_id: Uuid,
    pub principal_code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookingPrincipalRetireResult {
    pub booking_principal_id: Uuid,
    pub active_relationships: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceAvailabilitySetResult {
    pub service_availability_id: Uuid,
    pub booking_principal_id: Uuid,
    pub service_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationshipRecordResult {
    pub relationship_id: Uuid,
    pub client_group_id: Uuid,
    pub booking_principal_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RulesetCreateResult {
    pub ruleset_id: Uuid,
    pub name: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleAddResult {
    pub rule_id: Uuid,
    pub ruleset_id: Uuid,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractPackCreateResult {
    pub contract_pack_id: Uuid,
    pub code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractTemplateAddResult {
    pub contract_template_id: Uuid,
    pub contract_pack_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossSellResult {
    pub client_group_id: Uuid,
    pub existing_offerings: Vec<String>,
    pub potential_offerings: Vec<String>,
    pub existing_principals: Vec<Uuid>,
}

// =============================================================================
// Legal Entity Operations
// =============================================================================

/// Create a new BNY legal entity
pub struct LegalEntityCreate;

#[async_trait]
impl SemOsVerbOp for LegalEntityCreate {
    fn fqn(&self) -> &str {
        "legal-entity.create"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let pool = scope.pool().clone();
        let name = json_extract_string(args, "name")?;
        let incorporation_jurisdiction = json_extract_string(args, "incorporation-jurisdiction")?;
        let lei = json_extract_string_opt(args, "lei");
        let entity_id = json_extract_uuid_opt(args, ctx, "entity-id");

        let id = BookingPrincipalRepository::insert_legal_entity(
            &pool,
            &name,
            &incorporation_jurisdiction,
            lei.as_deref(),
            entity_id,
        )
        .await?;

        let result = LegalEntityCreateResult {
            legal_entity_id: id,
            name,
        };
        Ok(VerbExecutionOutcome::Record(serde_json::to_value(result)?))
    }
}

/// Update an existing legal entity
pub struct LegalEntityUpdate;

#[async_trait]
impl SemOsVerbOp for LegalEntityUpdate {
    fn fqn(&self) -> &str {
        "legal-entity.update"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let pool = scope.pool().clone();
        let legal_entity_id = json_extract_uuid(args, ctx, "legal-entity-id")?;
        let name = json_extract_string_opt(args, "name");
        let lei = json_extract_string_opt(args, "lei");
        let status = json_extract_string_opt(args, "status");

        let mut set_clauses = Vec::new();
        let mut param_idx = 2u32;
        let mut binds: Vec<String> = Vec::new();

        if let Some(ref n) = name {
            set_clauses.push(format!("name = ${}", param_idx));
            binds.push(n.clone());
            param_idx += 1;
        }
        if let Some(ref l) = lei {
            set_clauses.push(format!("lei = ${}", param_idx));
            binds.push(l.clone());
            param_idx += 1;
        }
        if let Some(ref s) = status {
            set_clauses.push(format!("status = ${}", param_idx));
            binds.push(s.clone());
            param_idx += 1;
        }

        if set_clauses.is_empty() {
            return Err(anyhow!("No fields to update"));
        }

        set_clauses.push("updated_at = now()".to_string());
        let sql = format!(
            r#"UPDATE "ob-poc".legal_entity SET {} WHERE legal_entity_id = $1"#,
            set_clauses.join(", ")
        );

        let mut query = sqlx::query(&sql).bind(legal_entity_id);
        for val in &binds {
            query = query.bind(val);
        }
        let _ = param_idx; // suppress warning

        query.execute(&pool).await?;

        Ok(VerbExecutionOutcome::Uuid(legal_entity_id))
    }
}

/// List active legal entities
pub struct LegalEntityList;

#[async_trait]
impl SemOsVerbOp for LegalEntityList {
    fn fqn(&self) -> &str {
        "legal-entity.list"
    }
    async fn execute(
        &self,
        _args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let pool = scope.pool().clone();
        let entities = BookingPrincipalRepository::list_legal_entities(&pool).await?;
        let values: Vec<serde_json::Value> = entities
            .into_iter()
            .map(|e| serde_json::to_value(e).unwrap_or_default())
            .collect();
        Ok(VerbExecutionOutcome::RecordSet(values))
    }
}

// =============================================================================
// Rule Field Dictionary Operations
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleFieldRegisterResult {
    pub field_key: String,
    pub field_type: String,
    pub description: Option<String>,
    pub source_table: Option<String>,
}

/// Register a new field in the rule field dictionary
pub struct RuleFieldRegister;

#[async_trait]
impl SemOsVerbOp for RuleFieldRegister {
    fn fqn(&self) -> &str {
        "rule-field.register"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let pool = scope.pool().clone();
        let field_key = json_extract_string(args, "field-key")?;
        let field_type = json_extract_string(args, "field-type")?;
        let description = json_extract_string_opt(args, "description");
        let source_table = json_extract_string_opt(args, "source-table");

        // Validate field_type against allowed values
        const VALID_TYPES: &[&str] = &["string", "string_array", "boolean", "number", "date"];
        if !VALID_TYPES.contains(&field_type.as_str()) {
            return Err(anyhow!(
                "Invalid field_type '{}'. Must be one of: {}",
                field_type,
                VALID_TYPES.join(", ")
            ));
        }

        let entry = BookingPrincipalRepository::register_field(
            &pool,
            &field_key,
            &field_type,
            description.as_deref(),
            source_table.as_deref(),
        )
        .await?;

        let result = RuleFieldRegisterResult {
            field_key: entry.field_key,
            field_type: entry.field_type,
            description: entry.description,
            source_table: entry.source_table,
        };
        Ok(VerbExecutionOutcome::Record(serde_json::to_value(result)?))
    }
}

/// List all registered fields in the rule field dictionary
pub struct RuleFieldList;

#[async_trait]
impl SemOsVerbOp for RuleFieldList {
    fn fqn(&self) -> &str {
        "rule-field.list"
    }
    async fn execute(
        &self,
        _args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let pool = scope.pool().clone();
        let entries = BookingPrincipalRepository::get_field_dictionary(&pool).await?;
        let values: Vec<serde_json::Value> = entries
            .into_iter()
            .map(serde_json::to_value)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(VerbExecutionOutcome::RecordSet(values))
    }
}

// =============================================================================
// Booking Location Operations
// =============================================================================

/// Create a new booking location
pub struct BookingLocationCreate;

#[async_trait]
impl SemOsVerbOp for BookingLocationCreate {
    fn fqn(&self) -> &str {
        "booking-location.create"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let pool = scope.pool().clone();
        let country_code = json_extract_string(args, "country-code")?;
        let region_code = json_extract_string_opt(args, "region-code");
        let jurisdiction_code = json_extract_string_opt(args, "jurisdiction-code");
        let regime_tags: Vec<String> = args
            .get("regulatory-regime-tags")
            .and_then(|v| v.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        let id = BookingPrincipalRepository::insert_booking_location(
            &pool,
            &country_code,
            region_code.as_deref(),
            &regime_tags,
            jurisdiction_code.as_deref(),
        )
        .await?;

        let result = BookingLocationCreateResult {
            booking_location_id: id,
            country_code,
        };
        Ok(VerbExecutionOutcome::Record(serde_json::to_value(result)?))
    }
}

/// Update a booking location
pub struct BookingLocationUpdate;

#[async_trait]
impl SemOsVerbOp for BookingLocationUpdate {
    fn fqn(&self) -> &str {
        "booking-location.update"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let pool = scope.pool().clone();
        let id = json_extract_uuid(args, ctx, "booking-location-id")?;
        let region_code = json_extract_string_opt(args, "region-code");
        let jurisdiction_code = json_extract_string_opt(args, "jurisdiction-code");

        let mut updates = vec!["updated_at = now()"];
        if region_code.is_some() {
            updates.push("region_code = $2");
        }
        if jurisdiction_code.is_some() {
            updates.push("jurisdiction_code = $3");
        }

        let sql = format!(
            r#"UPDATE "ob-poc".booking_location SET {} WHERE booking_location_id = $1"#,
            updates.join(", ")
        );

        sqlx::query(&sql)
            .bind(id)
            .bind(&region_code)
            .bind(&jurisdiction_code)
            .execute(&pool)
            .await?;

        Ok(VerbExecutionOutcome::Uuid(id))
    }
}

/// List booking locations
pub struct BookingLocationList;

#[async_trait]
impl SemOsVerbOp for BookingLocationList {
    fn fqn(&self) -> &str {
        "booking-location.list"
    }
    async fn execute(
        &self,
        _args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let pool = scope.pool().clone();
        let rows: Vec<(
            Uuid,
            String,
            Option<String>,
            Option<Vec<String>>,
            Option<String>,
        )> = sqlx::query_as(
            r#"
                SELECT booking_location_id, country_code, region_code,
                       regulatory_regime_tags, jurisdiction_code
                FROM "ob-poc".booking_location
                ORDER BY country_code
                "#,
        )
        .fetch_all(&pool)
        .await?;

        let values: Vec<serde_json::Value> = rows
            .into_iter()
            .map(|(id, cc, rc, tags, jc)| {
                serde_json::json!({
                    "booking_location_id": id,
                    "country_code": cc,
                    "region_code": rc,
                    "regulatory_regime_tags": tags.unwrap_or_default(),
                    "jurisdiction_code": jc,
                })
            })
            .collect();

        Ok(VerbExecutionOutcome::RecordSet(values))
    }
}

// =============================================================================
// Booking Principal Operations
// =============================================================================

/// Create a new booking principal (LE + location envelope)
pub struct BookingPrincipalCreate;

#[async_trait]
impl SemOsVerbOp for BookingPrincipalCreate {
    fn fqn(&self) -> &str {
        "booking-principal.create"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let pool = scope.pool().clone();
        let legal_entity_id = json_extract_uuid(args, ctx, "legal-entity-id")?;
        let booking_location_id = json_extract_uuid_opt(args, ctx, "booking-location-id");
        let principal_code = json_extract_string(args, "principal-code")?;
        let book_code = json_extract_string_opt(args, "book-code");

        let id = BookingPrincipalRepository::insert_booking_principal(
            &pool,
            legal_entity_id,
            booking_location_id,
            &principal_code,
            book_code.as_deref(),
        )
        .await?;

        let result = BookingPrincipalCreateResult {
            booking_principal_id: id,
            principal_code,
        };
        Ok(VerbExecutionOutcome::Record(serde_json::to_value(result)?))
    }
}

/// Update a booking principal
pub struct BookingPrincipalUpdate;

#[async_trait]
impl SemOsVerbOp for BookingPrincipalUpdate {
    fn fqn(&self) -> &str {
        "booking-principal.update"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let pool = scope.pool().clone();
        let id = json_extract_uuid(args, ctx, "booking-principal-id")?;
        let book_code = json_extract_string_opt(args, "book-code");
        let status = json_extract_string_opt(args, "status");

        let mut set_parts = vec!["updated_at = now()".to_string()];
        if book_code.is_some() {
            set_parts.push("book_code = $2".to_string());
        }
        if status.is_some() {
            set_parts.push("status = $3".to_string());
        }

        let sql = format!(
            r#"UPDATE "ob-poc".booking_principal SET {} WHERE booking_principal_id = $1"#,
            set_parts.join(", ")
        );

        sqlx::query(&sql)
            .bind(id)
            .bind(&book_code)
            .bind(&status)
            .execute(&pool)
            .await?;

        Ok(VerbExecutionOutcome::Uuid(id))
    }
}

/// Retire a booking principal (with active relationship check)
pub struct BookingPrincipalRetire;

#[async_trait]
impl SemOsVerbOp for BookingPrincipalRetire {
    fn fqn(&self) -> &str {
        "booking-principal.retire"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let pool = scope.pool().clone();
        let id = json_extract_uuid(args, ctx, "booking-principal-id")?;
        let force = json_extract_bool_opt(args, "force").unwrap_or(false);

        let active_count = BookingPrincipalRepository::retire_booking_principal(&pool, id).await?;

        if active_count > 0 && !force {
            return Err(anyhow!(
                "Cannot retire principal with {} active relationships. Use :force true to override.",
                active_count
            ));
        }

        let result = BookingPrincipalRetireResult {
            booking_principal_id: id,
            active_relationships: active_count,
        };
        Ok(VerbExecutionOutcome::Record(serde_json::to_value(result)?))
    }
}

// =============================================================================
// Evaluation Operations
// =============================================================================

/// Primary eligibility evaluation pipeline
pub struct BookingPrincipalEvaluate;

#[async_trait]
impl SemOsVerbOp for BookingPrincipalEvaluate {
    fn fqn(&self) -> &str {
        "booking-principal.evaluate"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let pool = scope.pool().clone();
        let client_group_id = json_extract_uuid(args, ctx, "client-group-id")?;
        let segment = json_extract_string(args, "segment")?;
        let domicile_country = json_extract_string(args, "domicile-country")?;
        let entity_types: Vec<String> = args
            .get("entity-types")
            .and_then(|v| v.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        let requested_by =
            json_extract_string_opt(args, "requested-by").unwrap_or_else(|| "system".to_string());

        // Offering IDs from product lookup
        let offering_ids: Vec<Uuid> = args
            .get("offering-ids")
            .and_then(|v| v.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|v| v.as_str().and_then(|s| uuid::Uuid::parse_str(s).ok()))
                    .collect()
            })
            .unwrap_or_default();

        // 1. Create client profile snapshot
        let profile_id = BookingPrincipalRepository::insert_client_profile(
            &pool,
            client_group_id,
            &segment,
            &domicile_country,
            &entity_types,
            None,
        )
        .await?;

        // 2. Get all active principals
        let principals = BookingPrincipalRepository::list_active_principals(&pool).await?;
        let principal_ids: Vec<Uuid> = principals.iter().map(|p| p.booking_principal_id).collect();

        // 3. Gather applicable rules
        let rulesets = BookingPrincipalRepository::gather_rules_for_evaluation(
            &pool,
            &offering_ids,
            &principal_ids,
        )
        .await?;

        // 4. Get existing relationships for scoring
        let existing_rels =
            BookingPrincipalRepository::get_active_relationships_for_client(&pool, client_group_id)
                .await?;

        // 5. Build base context
        let base_ctx = rule_evaluator::build_client_context(
            &segment,
            &domicile_country,
            &entity_types,
            None,
            &[], // classifications can be added later
        );

        // 6. Evaluate each principal
        let mut candidates = Vec::new();
        let mut all_gates = Vec::new();
        let mut all_contract_packs = Vec::new();
        let mut all_explains = Vec::new();
        let mut delivery_plan = Vec::new();

        for principal in &principals {
            let mut eval_ctx = base_ctx.clone();

            // Add principal context
            let location = if let Some(loc_id) = principal.booking_location_id {
                BookingPrincipalRepository::get_booking_location(&pool, loc_id).await?
            } else {
                None
            };

            if let Some(ref loc) = location {
                rule_evaluator::add_principal_context(
                    &mut eval_ctx,
                    &principal.principal_code,
                    &loc.country_code,
                    loc.region_code.as_deref(),
                    &loc.regulatory_regime_tags,
                );
            }

            // Evaluate all rulesets against this principal
            let mut all_outcomes = Vec::new();
            for (ruleset, rules) in &rulesets {
                let outcomes = rule_evaluator::evaluate_rules(ruleset, rules, &eval_ctx);
                for outcome in outcomes {
                    all_explains.push(ExplainEntry {
                        rule_id: outcome.rule_id,
                        rule_name: outcome.rule_name.clone(),
                        ruleset_boundary: outcome.boundary.clone(),
                        kind: outcome.kind.clone(),
                        outcome: format!("{:?}", outcome.effect),
                        evaluated_facts: serde_json::to_value(&outcome.evaluated_facts)
                            .unwrap_or_default(),
                        merge_decision: None,
                    });
                    all_outcomes.push(outcome);
                }
            }

            // Merge outcomes for this principal
            let merged = rule_evaluator::merge_outcomes_for_candidate(
                principal.booking_principal_id,
                &all_outcomes,
            );

            // Check existing relationship for scoring boost
            let has_relationship = existing_rels
                .iter()
                .any(|r| r.booking_principal_id == principal.booking_principal_id);
            let existing_offerings: Vec<String> = existing_rels
                .iter()
                .filter(|r| r.booking_principal_id == principal.booking_principal_id)
                .map(|r| r.product_offering_id.to_string())
                .collect();

            // Score: base from status + relationship boost
            let base_score = match &merged.status {
                CandidateStatus::Eligible => 1.0,
                CandidateStatus::EligibleWithGates { .. } => 0.8,
                CandidateStatus::ConditionalDeny { .. } => 0.3,
                CandidateStatus::HardDeny { .. } => 0.0,
            };
            let relationship_boost = if has_relationship { 0.1 } else { 0.0 };

            // Get legal entity name
            let le = BookingPrincipalRepository::get_legal_entity(&pool, principal.legal_entity_id)
                .await?;
            let le_name = le.map(|e| e.name).unwrap_or_default();

            candidates.push(EvaluatedCandidate {
                principal_id: principal.booking_principal_id,
                principal_code: principal.principal_code.clone(),
                legal_entity_name: le_name,
                score: base_score + relationship_boost,
                status: merged.status,
                existing_relationship: has_relationship,
                existing_offerings,
                reasons: merged.deny_reasons,
            });

            // Collect gates
            for gate in &merged.gates {
                all_gates.push(EvaluationGate {
                    gate_code: gate.gate_code.clone(),
                    gate_name: gate.gate_name.clone(),
                    boundary: gate.boundary.clone(),
                    severity: gate.severity.clone(),
                    source_rule_id: gate.source_rule_id,
                    applies_to_principal_ids: vec![principal.booking_principal_id],
                });
            }

            // Collect contract packs
            for cp in &merged.contract_packs {
                all_contract_packs.push(EvaluationContractPack {
                    contract_pack_code: cp.contract_pack_code.clone(),
                    contract_pack_name: cp.contract_pack_code.clone(),
                    template_types: cp.template_types.clone(),
                    applies_to_principal_ids: vec![principal.booking_principal_id],
                });
            }

            // Check delivery (service availability)
            for offering_id in &offering_ids {
                let services =
                    BookingPrincipalRepository::get_services_for_product(&pool, *offering_id)
                        .await?;
                for service_id in services {
                    let avail = BookingPrincipalRepository::get_availability(
                        &pool,
                        principal.booking_principal_id,
                        service_id,
                    )
                    .await?;

                    let (reg, com, ops, dm, available) = match avail {
                        Some(a) => {
                            let ok = a.regulatory_status == "permitted"
                                && a.commercial_status == "offered"
                                && a.operational_status == "supported";
                            (
                                a.regulatory_status,
                                a.commercial_status,
                                a.operational_status,
                                a.delivery_model,
                                ok,
                            )
                        }
                        None => (
                            "unknown".to_string(),
                            "unknown".to_string(),
                            "unknown".to_string(),
                            None,
                            false,
                        ),
                    };

                    delivery_plan.push(DeliveryPlanEntry {
                        principal_id: principal.booking_principal_id,
                        service_code: service_id.to_string(),
                        regulatory_status: reg,
                        commercial_status: com,
                        operational_status: ops,
                        delivery_model: dm,
                        available,
                        constraints_evaluated: None,
                    });
                }
            }
        }

        // Sort candidates by score descending
        candidates.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // 7. Pin evaluation record
        let result_json = serde_json::to_value(&candidates)?;
        let explain_json = serde_json::to_value(&all_explains)?;
        let policy_snapshot = serde_json::json!({
            "rulesets_evaluated": rulesets.len(),
            "principals_evaluated": principals.len(),
            "offerings_evaluated": offering_ids.len(),
        });

        let evaluation_id = BookingPrincipalRepository::insert_evaluation(
            &pool,
            profile_id,
            client_group_id,
            &offering_ids,
            &requested_by,
            &policy_snapshot,
            None,
            &result_json,
            &explain_json,
        )
        .await?;

        let eval_result = EvaluationResult {
            evaluation_id,
            candidates,
            gates: all_gates,
            contract_packs: all_contract_packs,
            delivery_plan,
            explain: all_explains,
            policy_snapshot,
        };

        Ok(VerbExecutionOutcome::Record(serde_json::to_value(
            eval_result,
        )?))
    }
}

/// Record selection of a principal from an evaluation
pub struct BookingPrincipalSelect;

#[async_trait]
impl SemOsVerbOp for BookingPrincipalSelect {
    fn fqn(&self) -> &str {
        "booking-principal.select"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let pool = scope.pool().clone();
        let evaluation_id = json_extract_uuid(args, ctx, "evaluation-id")?;
        let principal_id = json_extract_uuid(args, ctx, "principal-id")?;

        let updated = BookingPrincipalRepository::select_principal_on_evaluation(
            &pool,
            evaluation_id,
            principal_id,
        )
        .await?;

        if !updated {
            return Err(anyhow!(
                "Could not select principal — evaluation already has a selection or does not exist"
            ));
        }

        let result = SelectionResult {
            selected_principal_id: principal_id,
            contract_packs: vec![],
            gates: vec![],
            override_required: false,
            override_gate: None,
        };

        Ok(VerbExecutionOutcome::Record(serde_json::to_value(result)?))
    }
}

/// Retrieve full explain payload for an evaluation
pub struct BookingPrincipalExplain;

#[async_trait]
impl SemOsVerbOp for BookingPrincipalExplain {
    fn fqn(&self) -> &str {
        "booking-principal.explain"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let pool = scope.pool().clone();
        let evaluation_id = json_extract_uuid(args, ctx, "evaluation-id")?;

        let eval = BookingPrincipalRepository::get_evaluation(&pool, evaluation_id)
            .await?
            .ok_or_else(|| anyhow!("Evaluation not found: {}", evaluation_id))?;

        Ok(VerbExecutionOutcome::Record(serde_json::to_value(eval)?))
    }
}

// =============================================================================
// Client-Principal Relationship Operations
// =============================================================================

/// Record a new client-principal-offering relationship
pub struct ClientPrincipalRelationshipRecord;

#[async_trait]
impl SemOsVerbOp for ClientPrincipalRelationshipRecord {
    fn fqn(&self) -> &str {
        "client-principal-relationship.record"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let pool = scope.pool().clone();
        let client_group_id = json_extract_uuid(args, ctx, "client-group-id")?;
        let booking_principal_id = json_extract_uuid(args, ctx, "booking-principal-id")?;
        let product_offering_id = json_extract_uuid(args, ctx, "product-offering-id")?;
        let contract_ref = json_extract_string_opt(args, "contract-ref");

        let id = BookingPrincipalRepository::record_relationship(
            &pool,
            client_group_id,
            booking_principal_id,
            product_offering_id,
            contract_ref.as_deref(),
        )
        .await?;

        let result = RelationshipRecordResult {
            relationship_id: id,
            client_group_id,
            booking_principal_id,
        };
        Ok(VerbExecutionOutcome::Record(serde_json::to_value(result)?))
    }
}

/// Terminate a client-principal relationship
pub struct ClientPrincipalRelationshipTerminate;

#[async_trait]
impl SemOsVerbOp for ClientPrincipalRelationshipTerminate {
    fn fqn(&self) -> &str {
        "client-principal-relationship.terminate"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let pool = scope.pool().clone();
        let relationship_id = json_extract_uuid(args, ctx, "relationship-id")?;

        let affected =
            BookingPrincipalRepository::terminate_relationship(&pool, relationship_id).await?;

        Ok(VerbExecutionOutcome::Affected(affected as u64))
    }
}

/// List relationships for a client group
pub struct ClientPrincipalRelationshipList;

#[async_trait]
impl SemOsVerbOp for ClientPrincipalRelationshipList {
    fn fqn(&self) -> &str {
        "client-principal-relationship.list"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let pool = scope.pool().clone();
        let client_group_id = json_extract_uuid(args, ctx, "client-group-id")?;
        let status = json_extract_string_opt(args, "status");

        let rels = BookingPrincipalRepository::list_relationships(
            &pool,
            client_group_id,
            status.as_deref(),
        )
        .await?;

        let values: Vec<serde_json::Value> = rels
            .into_iter()
            .map(|r| serde_json::to_value(r).unwrap_or_default())
            .collect();

        Ok(VerbExecutionOutcome::RecordSet(values))
    }
}

/// Cross-sell check: what other offerings could this client use?
pub struct ClientPrincipalRelationshipCrossSellCheck;

#[async_trait]
impl SemOsVerbOp for ClientPrincipalRelationshipCrossSellCheck {
    fn fqn(&self) -> &str {
        "client-principal-relationship.cross-sell-check"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let pool = scope.pool().clone();
        let client_group_id = json_extract_uuid(args, ctx, "client-group-id")?;

        let active_rels =
            BookingPrincipalRepository::get_active_relationships_for_client(&pool, client_group_id)
                .await?;

        let existing_offering_ids: Vec<Uuid> =
            active_rels.iter().map(|r| r.product_offering_id).collect();
        let existing_principal_ids: Vec<Uuid> =
            active_rels.iter().map(|r| r.booking_principal_id).collect();

        // Get all available products
        let all_products: Vec<(Uuid, String)> = sqlx::query_as(
            r#"SELECT product_id, COALESCE(product_code, name) FROM "ob-poc".products WHERE is_active = true"#,
        )
        .fetch_all(&pool)
        .await?;

        let potential: Vec<String> = all_products
            .iter()
            .filter(|(id, _)| !existing_offering_ids.contains(id))
            .map(|(_, code)| code.clone())
            .collect();

        let existing: Vec<String> = all_products
            .iter()
            .filter(|(id, _)| existing_offering_ids.contains(id))
            .map(|(_, code)| code.clone())
            .collect();

        let result = CrossSellResult {
            client_group_id,
            existing_offerings: existing,
            potential_offerings: potential,
            existing_principals: existing_principal_ids,
        };

        Ok(VerbExecutionOutcome::Record(serde_json::to_value(result)?))
    }
}

// =============================================================================
// Service Availability Operations
// =============================================================================

/// Set three-lane service availability for a principal x service
pub struct ServiceAvailabilitySet;

#[async_trait]
impl SemOsVerbOp for ServiceAvailabilitySet {
    fn fqn(&self) -> &str {
        "service-availability.set"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let pool = scope.pool().clone();
        let booking_principal_id = json_extract_uuid(args, ctx, "booking-principal-id")?;
        let service_id = json_extract_uuid(args, ctx, "service-id")?;
        let regulatory_status = json_extract_string(args, "regulatory-status")?;
        let commercial_status = json_extract_string(args, "commercial-status")?;
        let operational_status = json_extract_string(args, "operational-status")?;
        let delivery_model = json_extract_string_opt(args, "delivery-model");

        let id = BookingPrincipalRepository::set_service_availability(
            &pool,
            booking_principal_id,
            service_id,
            &regulatory_status,
            None,
            &commercial_status,
            None,
            &operational_status,
            delivery_model.as_deref(),
            None,
            None,
            None,
        )
        .await?;

        let result = ServiceAvailabilitySetResult {
            service_availability_id: id,
            booking_principal_id,
            service_id,
        };
        Ok(VerbExecutionOutcome::Record(serde_json::to_value(result)?))
    }
}

/// List service availability for a booking principal
pub struct ServiceAvailabilityList;

#[async_trait]
impl SemOsVerbOp for ServiceAvailabilityList {
    fn fqn(&self) -> &str {
        "service-availability.list"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let pool = scope.pool().clone();
        let booking_principal_id = json_extract_uuid(args, ctx, "booking-principal-id")?;

        let records =
            BookingPrincipalRepository::list_availability_for_principal(&pool, booking_principal_id)
                .await?;

        let values: Vec<serde_json::Value> = records
            .into_iter()
            .map(|r| serde_json::to_value(r).unwrap_or_default())
            .collect();

        Ok(VerbExecutionOutcome::RecordSet(values))
    }
}

/// List service availability by principal (alias for list)
pub struct ServiceAvailabilityListByPrincipal;

#[async_trait]
impl SemOsVerbOp for ServiceAvailabilityListByPrincipal {
    fn fqn(&self) -> &str {
        "service-availability.list-by-principal"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let pool = scope.pool().clone();
        let booking_principal_id = json_extract_uuid(args, ctx, "booking-principal-id")?;

        let records =
            BookingPrincipalRepository::list_availability_for_principal(&pool, booking_principal_id)
                .await?;

        let values: Vec<serde_json::Value> = records
            .into_iter()
            .map(|r| serde_json::to_value(r).unwrap_or_default())
            .collect();

        Ok(VerbExecutionOutcome::RecordSet(values))
    }
}

// =============================================================================
// Ruleset Operations
// =============================================================================

/// Create a new ruleset (starts in draft status)
pub struct RulesetCreate;

#[async_trait]
impl SemOsVerbOp for RulesetCreate {
    fn fqn(&self) -> &str {
        "ruleset.create"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let pool = scope.pool().clone();
        let name = json_extract_string(args, "name")?;
        let owner_type = json_extract_string(args, "owner-type")?;
        let owner_id = json_extract_uuid_opt(args, ctx, "owner-id");
        let boundary = json_extract_string(args, "boundary")?;

        // Validate boundary
        RulesetBoundary::from_str_val(&boundary).ok_or_else(|| {
            anyhow!(
                "Invalid boundary: {}. Must be regulatory, commercial, or operational",
                boundary
            )
        })?;

        // Validate owner type
        if !["global", "offering", "principal"].contains(&owner_type.as_str()) {
            return Err(anyhow!(
                "Invalid owner type: {}. Must be global, offering, or principal",
                owner_type
            ));
        }

        let id = BookingPrincipalRepository::create_ruleset(
            &pool,
            &owner_type,
            owner_id,
            &name,
            &boundary,
            None,
            None,
        )
        .await?;

        let result = RulesetCreateResult {
            ruleset_id: id,
            name,
            status: "draft".to_string(),
        };
        Ok(VerbExecutionOutcome::Record(serde_json::to_value(result)?))
    }
}

/// Publish a ruleset (draft → active, with field dictionary validation)
pub struct RulesetPublish;

#[async_trait]
impl SemOsVerbOp for RulesetPublish {
    fn fqn(&self) -> &str {
        "ruleset.publish"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let pool = scope.pool().clone();
        let ruleset_id = json_extract_uuid(args, ctx, "ruleset-id")?;

        // Load field dictionary for validation
        let dict = BookingPrincipalRepository::get_field_dictionary(&pool).await?;
        let known_fields: std::collections::HashMap<String, String> = dict
            .into_iter()
            .map(|d| (d.field_key, d.field_type))
            .collect();

        // Load rules for this ruleset
        let rules: Vec<(Uuid, String, serde_json::Value)> = sqlx::query_as(
            r#"
            SELECT rule_id, name, when_expr
            FROM "ob-poc".rule
            WHERE ruleset_id = $1
            "#,
        )
        .bind(ruleset_id)
        .fetch_all(&pool)
        .await?;

        // Validate each rule's field references
        let mut validation_errors = Vec::new();
        for (rule_id, rule_name, when_expr) in &rules {
            if let Ok(condition) = serde_json::from_value::<Condition>(when_expr.clone()) {
                let unknown = rule_evaluator::validate_field_references(&condition, &known_fields);
                for field in unknown {
                    validation_errors.push(format!(
                        "Rule '{}' ({}) references unknown field: {}",
                        rule_name, rule_id, field
                    ));
                }
                let warnings =
                    rule_evaluator::validate_operator_compatibility(&condition, &known_fields);
                for w in warnings {
                    validation_errors.push(format!("Rule '{}' ({}): {}", rule_name, rule_id, w));
                }
            }
        }

        if !validation_errors.is_empty() {
            return Err(anyhow!(
                "Ruleset validation failed:\n{}",
                validation_errors.join("\n")
            ));
        }

        // Publish (will fail on temporal overlap via trigger)
        let published = BookingPrincipalRepository::publish_ruleset(&pool, ruleset_id).await?;

        if !published {
            return Err(anyhow!(
                "Could not publish ruleset — either not in draft status or overlap detected"
            ));
        }

        Ok(VerbExecutionOutcome::Uuid(ruleset_id))
    }
}

/// Retire an active ruleset
pub struct RulesetRetire;

#[async_trait]
impl SemOsVerbOp for RulesetRetire {
    fn fqn(&self) -> &str {
        "ruleset.retire"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let pool = scope.pool().clone();
        let ruleset_id = json_extract_uuid(args, ctx, "ruleset-id")?;

        let retired = BookingPrincipalRepository::retire_ruleset(&pool, ruleset_id).await?;

        if !retired {
            return Err(anyhow!("Could not retire ruleset — not in active status"));
        }

        Ok(VerbExecutionOutcome::Uuid(ruleset_id))
    }
}

// =============================================================================
// Rule Operations
// =============================================================================

/// Add a rule to a ruleset
pub struct RuleAdd;

#[async_trait]
impl SemOsVerbOp for RuleAdd {
    fn fqn(&self) -> &str {
        "rule.add"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let pool = scope.pool().clone();
        let ruleset_id = json_extract_uuid(args, ctx, "ruleset-id")?;
        let name = json_extract_string(args, "name")?;
        let kind = json_extract_string(args, "kind")?;
        let when_expr_str = json_extract_string(args, "when-expr")?;
        let then_effect_str = json_extract_string(args, "then-effect")?;
        let explain = json_extract_string_opt(args, "explain");
        let priority = json_extract_int_opt(args, "priority").map(|v| v as i32);

        // Parse and validate JSON
        let when_expr: serde_json::Value = serde_json::from_str(&when_expr_str)
            .map_err(|e| anyhow!("Invalid when-expr JSON: {}", e))?;
        let then_effect: serde_json::Value = serde_json::from_str(&then_effect_str)
            .map_err(|e| anyhow!("Invalid then-effect JSON: {}", e))?;

        // Validate that when_expr parses as a Condition
        serde_json::from_value::<Condition>(when_expr.clone())
            .map_err(|e| anyhow!("Invalid condition structure: {}", e))?;

        // Validate that then_effect parses as an Effect
        serde_json::from_value::<Effect>(then_effect.clone())
            .map_err(|e| anyhow!("Invalid effect structure: {}", e))?;

        let id = BookingPrincipalRepository::add_rule(
            &pool,
            ruleset_id,
            &name,
            &kind,
            &when_expr,
            &then_effect,
            explain.as_deref(),
            priority,
        )
        .await?;

        let result = RuleAddResult {
            rule_id: id,
            ruleset_id,
            name,
        };
        Ok(VerbExecutionOutcome::Record(serde_json::to_value(result)?))
    }
}

/// Update an existing rule
pub struct RuleUpdate;

#[async_trait]
impl SemOsVerbOp for RuleUpdate {
    fn fqn(&self) -> &str {
        "rule.update"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let pool = scope.pool().clone();
        let rule_id = json_extract_uuid(args, ctx, "rule-id")?;
        let name = json_extract_string_opt(args, "name");
        let when_expr_str = json_extract_string_opt(args, "when-expr");
        let then_effect_str = json_extract_string_opt(args, "then-effect");
        let priority = json_extract_int_opt(args, "priority").map(|v| v as i32);

        // Validate JSON if provided
        let when_expr: Option<serde_json::Value> = if let Some(ref s) = when_expr_str {
            let v: serde_json::Value =
                serde_json::from_str(s).map_err(|e| anyhow!("Invalid when-expr JSON: {}", e))?;
            serde_json::from_value::<Condition>(v.clone())
                .map_err(|e| anyhow!("Invalid condition structure: {}", e))?;
            Some(v)
        } else {
            None
        };

        let then_effect: Option<serde_json::Value> = if let Some(ref s) = then_effect_str {
            let v: serde_json::Value =
                serde_json::from_str(s).map_err(|e| anyhow!("Invalid then-effect JSON: {}", e))?;
            serde_json::from_value::<Effect>(v.clone())
                .map_err(|e| anyhow!("Invalid effect structure: {}", e))?;
            Some(v)
        } else {
            None
        };

        // Build dynamic update
        let mut set_parts = vec!["updated_at = now()".to_string()];
        let mut idx = 2u32;

        if name.is_some() {
            set_parts.push(format!("name = ${idx}"));
            idx += 1;
        }
        if when_expr.is_some() {
            set_parts.push(format!("when_expr = ${idx}"));
            idx += 1;
        }
        if then_effect.is_some() {
            set_parts.push(format!("then_effect = ${idx}"));
            idx += 1;
        }
        if priority.is_some() {
            set_parts.push(format!("priority = ${idx}"));
            idx += 1;
        }
        let _ = idx;

        let sql = format!(
            r#"UPDATE "ob-poc".rule SET {} WHERE rule_id = $1"#,
            set_parts.join(", ")
        );

        let mut query = sqlx::query(&sql).bind(rule_id);
        if let Some(ref n) = name {
            query = query.bind(n);
        }
        if let Some(ref w) = when_expr {
            query = query.bind(w);
        }
        if let Some(ref t) = then_effect {
            query = query.bind(t);
        }
        if let Some(p) = priority {
            query = query.bind(p);
        }

        query.execute(&pool).await?;

        Ok(VerbExecutionOutcome::Uuid(rule_id))
    }
}

/// Disable a rule (soft delete)
pub struct RuleDisable;

#[async_trait]
impl SemOsVerbOp for RuleDisable {
    fn fqn(&self) -> &str {
        "rule.disable"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let pool = scope.pool().clone();
        let rule_id = json_extract_uuid(args, ctx, "rule-id")?;

        sqlx::query(
            r#"
            UPDATE "ob-poc".rule
            SET kind = 'disabled', updated_at = now()
            WHERE rule_id = $1
            "#,
        )
        .bind(rule_id)
        .execute(&pool)
        .await?;

        Ok(VerbExecutionOutcome::Uuid(rule_id))
    }
}

// =============================================================================
// Contract Pack Operations
// =============================================================================

/// Create a new contract pack
pub struct ContractPackCreate;

#[async_trait]
impl SemOsVerbOp for ContractPackCreate {
    fn fqn(&self) -> &str {
        "contract-pack.create"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let pool = scope.pool().clone();
        let code = json_extract_string(args, "code")?;
        let name = json_extract_string(args, "name")?;
        let description = json_extract_string_opt(args, "description");

        let id = BookingPrincipalRepository::create_contract_pack(
            &pool,
            &code,
            &name,
            description.as_deref(),
        )
        .await?;

        let result = ContractPackCreateResult {
            contract_pack_id: id,
            code,
        };
        Ok(VerbExecutionOutcome::Record(serde_json::to_value(result)?))
    }
}

/// Add a template to a contract pack
pub struct ContractPackAddTemplate;

#[async_trait]
impl SemOsVerbOp for ContractPackAddTemplate {
    fn fqn(&self) -> &str {
        "contract-pack.add-template"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let pool = scope.pool().clone();
        let contract_pack_id = json_extract_uuid(args, ctx, "contract-pack-id")?;
        let template_type = json_extract_string(args, "template-type")?;
        let template_ref = json_extract_string_opt(args, "template-ref");

        let id = BookingPrincipalRepository::add_contract_template(
            &pool,
            contract_pack_id,
            &template_type,
            template_ref.as_deref(),
        )
        .await?;

        let result = ContractTemplateAddResult {
            contract_template_id: id,
            contract_pack_id,
        };
        Ok(VerbExecutionOutcome::Record(serde_json::to_value(result)?))
    }
}

// =============================================================================
// Coverage & Analysis Operations
// =============================================================================

/// Generate coverage matrix: segments x jurisdictions x principals
pub struct BookingPrincipalCoverageMatrix;

#[async_trait]
impl SemOsVerbOp for BookingPrincipalCoverageMatrix {
    fn fqn(&self) -> &str {
        "booking-principal.coverage-matrix"
    }
    async fn execute(
        &self,
        _args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let pool = scope.pool().clone();
        // Build coverage matrix from principals x services
        let rows: Vec<(Uuid, String, String, String, String, String, String)> = sqlx::query_as(
            r#"
            SELECT
                bp.booking_principal_id,
                bp.principal_code,
                bl.country_code as jurisdiction,
                sa.regulatory_status,
                sa.commercial_status,
                sa.operational_status,
                COALESCE(sa.delivery_model, 'unknown') as delivery_model
            FROM "ob-poc".booking_principal bp
            JOIN "ob-poc".booking_location bl ON bl.booking_location_id = bp.booking_location_id
            LEFT JOIN "ob-poc".service_availability sa ON sa.booking_principal_id = bp.booking_principal_id
                AND now() BETWEEN sa.effective_from AND COALESCE(sa.effective_to, 'infinity'::timestamptz)
            WHERE bp.status = 'active'
            ORDER BY bl.country_code, bp.principal_code
            "#,
        )
        .fetch_all(&pool)
        .await?;

        let values: Vec<serde_json::Value> = rows
            .into_iter()
            .map(|(pid, pc, jur, reg, com, ops, dm)| {
                let overall = if reg == "permitted" && com == "offered" && ops == "supported" {
                    "full"
                } else if reg == "prohibited" {
                    "blocked"
                } else {
                    "partial"
                };
                serde_json::json!({
                    "principal_id": pid,
                    "principal_code": pc,
                    "jurisdiction": jur,
                    "regulatory": reg,
                    "commercial": com,
                    "operational": ops,
                    "delivery_model": dm,
                    "overall": overall,
                })
            })
            .collect();

        Ok(VerbExecutionOutcome::RecordSet(values))
    }
}

/// Generate gap report across all three boundaries
pub struct BookingPrincipalGapReport;

#[async_trait]
impl SemOsVerbOp for BookingPrincipalGapReport {
    fn fqn(&self) -> &str {
        "booking-principal.gap-report"
    }
    async fn execute(
        &self,
        _args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let pool = scope.pool().clone();
        let mut all_gaps = Vec::new();

        let reg_gaps = BookingPrincipalRepository::get_regulatory_gaps(&pool).await?;
        for g in reg_gaps {
            all_gaps.push(serde_json::to_value(g)?);
        }

        let com_gaps = BookingPrincipalRepository::get_commercial_gaps(&pool).await?;
        for g in com_gaps {
            all_gaps.push(serde_json::to_value(g)?);
        }

        let ops_gaps = BookingPrincipalRepository::get_operational_gaps(&pool).await?;
        for g in ops_gaps {
            all_gaps.push(serde_json::to_value(g)?);
        }

        Ok(VerbExecutionOutcome::RecordSet(all_gaps))
    }
}

/// Impact analysis for principal retirement
pub struct BookingPrincipalImpactAnalysis;

#[async_trait]
impl SemOsVerbOp for BookingPrincipalImpactAnalysis {
    fn fqn(&self) -> &str {
        "booking-principal.impact-analysis"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let pool = scope.pool().clone();
        let principal_id = json_extract_uuid(args, ctx, "booking-principal-id")?;

        // Find all active relationships for this principal
        let affected: Vec<(Uuid, Uuid, String)> = sqlx::query_as(
            r#"
            SELECT client_group_id, product_offering_id, relationship_status
            FROM "ob-poc".client_principal_relationship
            WHERE booking_principal_id = $1
              AND relationship_status = 'active'
            "#,
        )
        .bind(principal_id)
        .fetch_all(&pool)
        .await?;

        // Find alternative principals (same location or broader)
        let alternatives: Vec<(Uuid, String)> = sqlx::query_as(
            r#"
            SELECT bp2.booking_principal_id, bp2.principal_code
            FROM "ob-poc".booking_principal bp2
            WHERE bp2.booking_principal_id != $1
              AND bp2.status = 'active'
              AND bp2.booking_location_id IN (
                  SELECT booking_location_id
                  FROM "ob-poc".booking_principal
                  WHERE booking_principal_id = $1
              )
            "#,
        )
        .bind(principal_id)
        .fetch_all(&pool)
        .await?;

        let impact_entries: Vec<serde_json::Value> = affected
            .into_iter()
            .map(|(cg_id, po_id, status)| {
                let alt_list: Vec<serde_json::Value> = alternatives
                    .iter()
                    .map(|(id, code)| {
                        serde_json::json!({
                            "principal_id": id,
                            "principal_code": code,
                        })
                    })
                    .collect();

                serde_json::json!({
                    "client_group_id": cg_id,
                    "offering_id": po_id,
                    "relationship_status": status,
                    "alternative_principals": alt_list,
                })
            })
            .collect();

        Ok(VerbExecutionOutcome::RecordSet(impact_entries))
    }
}
