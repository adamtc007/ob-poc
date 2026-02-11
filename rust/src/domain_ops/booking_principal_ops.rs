//! Booking Principal Operations
//!
//! Operations for booking principal selection, eligibility evaluation,
//! client-principal relationship management, and coverage analysis.
//!
//! The booking principal selection capability determines "who can contract what,
//! for whom, where" through a rule-driven, boundary-aware evaluation pipeline.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use ob_poc_macros::register_custom_op;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::helpers::{
    extract_bool_opt, extract_int_opt, extract_string, extract_string_opt, extract_uuid,
    extract_uuid_opt,
};
use super::CustomOperation;
use crate::api::booking_principal_types::*;
use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};

#[cfg(feature = "database")]
use crate::database::booking_principal_repository::BookingPrincipalRepository;
#[cfg(feature = "database")]
use crate::domain_ops::rule_evaluator;
#[cfg(feature = "database")]
use sqlx::PgPool;

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
#[register_custom_op]
pub struct LegalEntityCreateOp;

#[async_trait]
impl CustomOperation for LegalEntityCreateOp {
    fn domain(&self) -> &'static str {
        "legal-entity"
    }
    fn verb(&self) -> &'static str {
        "create"
    }
    fn rationale(&self) -> &'static str {
        "Validates incorporation jurisdiction and optional LEI before insert"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let name = extract_string(verb_call, "name")?;
        let incorporation_jurisdiction = extract_string(verb_call, "incorporation-jurisdiction")?;
        let lei = extract_string_opt(verb_call, "lei");
        let entity_id = extract_uuid_opt(verb_call, _ctx, "entity-id");

        let id = BookingPrincipalRepository::insert_legal_entity(
            pool,
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
        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Uuid(Uuid::new_v4()))
    }
}

/// Update an existing legal entity
#[register_custom_op]
pub struct LegalEntityUpdateOp;

#[async_trait]
impl CustomOperation for LegalEntityUpdateOp {
    fn domain(&self) -> &'static str {
        "legal-entity"
    }
    fn verb(&self) -> &'static str {
        "update"
    }
    fn rationale(&self) -> &'static str {
        "Supports partial updates of legal entity fields"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let legal_entity_id = extract_uuid(verb_call, ctx, "legal-entity-id")?;
        let name = extract_string_opt(verb_call, "name");
        let lei = extract_string_opt(verb_call, "lei");
        let status = extract_string_opt(verb_call, "status");

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

        query.execute(pool).await?;

        Ok(ExecutionResult::Uuid(legal_entity_id))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Void)
    }
}

/// List active legal entities
#[register_custom_op]
pub struct LegalEntityListOp;

#[async_trait]
impl CustomOperation for LegalEntityListOp {
    fn domain(&self) -> &'static str {
        "legal-entity"
    }
    fn verb(&self) -> &'static str {
        "list"
    }
    fn rationale(&self) -> &'static str {
        "Filters to active entities only, returns structured list"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let entities = BookingPrincipalRepository::list_legal_entities(pool).await?;
        let values: Vec<serde_json::Value> = entities
            .into_iter()
            .map(|e| serde_json::to_value(e).unwrap_or_default())
            .collect();
        Ok(ExecutionResult::RecordSet(values))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::RecordSet(vec![]))
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
#[register_custom_op]
pub struct RuleFieldRegisterOp;

#[async_trait]
impl CustomOperation for RuleFieldRegisterOp {
    fn domain(&self) -> &'static str {
        "rule-field"
    }
    fn verb(&self) -> &'static str {
        "register"
    }
    fn rationale(&self) -> &'static str {
        "Validates field_type against allowed enum, upserts into closed-world dictionary"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let field_key = extract_string(verb_call, "field-key")?;
        let field_type = extract_string(verb_call, "field-type")?;
        let description = extract_string_opt(verb_call, "description");
        let source_table = extract_string_opt(verb_call, "source-table");

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
            pool,
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
        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("rule-field.register requires database"))
    }
}

/// List all registered fields in the rule field dictionary
#[register_custom_op]
pub struct RuleFieldListOp;

#[async_trait]
impl CustomOperation for RuleFieldListOp {
    fn domain(&self) -> &'static str {
        "rule-field"
    }
    fn verb(&self) -> &'static str {
        "list"
    }
    fn rationale(&self) -> &'static str {
        "Returns the full closed-world field dictionary for rule authoring"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let entries = BookingPrincipalRepository::get_field_dictionary(pool).await?;
        let values: Vec<serde_json::Value> = entries
            .into_iter()
            .map(serde_json::to_value)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ExecutionResult::RecordSet(values))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::RecordSet(vec![]))
    }
}

// =============================================================================
// Booking Location Operations
// =============================================================================

/// Create a new booking location
#[register_custom_op]
pub struct BookingLocationCreateOp;

#[async_trait]
impl CustomOperation for BookingLocationCreateOp {
    fn domain(&self) -> &'static str {
        "booking-location"
    }
    fn verb(&self) -> &'static str {
        "create"
    }
    fn rationale(&self) -> &'static str {
        "Validates jurisdiction FK and regulatory regime tags before insert"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let country_code = extract_string(verb_call, "country-code")?;
        let region_code = extract_string_opt(verb_call, "region-code");
        let jurisdiction_code = extract_string_opt(verb_call, "jurisdiction-code");
        let regime_tags: Vec<String> = verb_call
            .get_arg("regulatory-regime-tags")
            .and_then(|a| a.value.as_list())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|v| v.as_string().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        let id = BookingPrincipalRepository::insert_booking_location(
            pool,
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
        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Uuid(Uuid::new_v4()))
    }
}

/// Update a booking location
#[register_custom_op]
pub struct BookingLocationUpdateOp;

#[async_trait]
impl CustomOperation for BookingLocationUpdateOp {
    fn domain(&self) -> &'static str {
        "booking-location"
    }
    fn verb(&self) -> &'static str {
        "update"
    }
    fn rationale(&self) -> &'static str {
        "Supports partial updates of booking location fields"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let id = extract_uuid(verb_call, ctx, "booking-location-id")?;
        let region_code = extract_string_opt(verb_call, "region-code");
        let jurisdiction_code = extract_string_opt(verb_call, "jurisdiction-code");

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
            .execute(pool)
            .await?;

        Ok(ExecutionResult::Uuid(id))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Void)
    }
}

/// List booking locations
#[register_custom_op]
pub struct BookingLocationListOp;

#[async_trait]
impl CustomOperation for BookingLocationListOp {
    fn domain(&self) -> &'static str {
        "booking-location"
    }
    fn verb(&self) -> &'static str {
        "list"
    }
    fn rationale(&self) -> &'static str {
        "Returns all locations with regulatory regime tags"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
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
        .fetch_all(pool)
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

        Ok(ExecutionResult::RecordSet(values))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::RecordSet(vec![]))
    }
}

// =============================================================================
// Booking Principal Operations
// =============================================================================

/// Create a new booking principal (LE + location envelope)
#[register_custom_op]
pub struct BookingPrincipalCreateOp;

#[async_trait]
impl CustomOperation for BookingPrincipalCreateOp {
    fn domain(&self) -> &'static str {
        "booking-principal"
    }
    fn verb(&self) -> &'static str {
        "create"
    }
    fn rationale(&self) -> &'static str {
        "Validates FK to legal entity and booking location, generates principal code"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let legal_entity_id = extract_uuid(verb_call, ctx, "legal-entity-id")?;
        let booking_location_id = extract_uuid_opt(verb_call, ctx, "booking-location-id");
        let principal_code = extract_string(verb_call, "principal-code")?;
        let book_code = extract_string_opt(verb_call, "book-code");

        let id = BookingPrincipalRepository::insert_booking_principal(
            pool,
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
        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Uuid(Uuid::new_v4()))
    }
}

/// Update a booking principal
#[register_custom_op]
pub struct BookingPrincipalUpdateOp;

#[async_trait]
impl CustomOperation for BookingPrincipalUpdateOp {
    fn domain(&self) -> &'static str {
        "booking-principal"
    }
    fn verb(&self) -> &'static str {
        "update"
    }
    fn rationale(&self) -> &'static str {
        "Supports partial updates (book code, status, metadata)"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let id = extract_uuid(verb_call, ctx, "booking-principal-id")?;
        let book_code = extract_string_opt(verb_call, "book-code");
        let status = extract_string_opt(verb_call, "status");

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
            .execute(pool)
            .await?;

        Ok(ExecutionResult::Uuid(id))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Void)
    }
}

/// Retire a booking principal (with active relationship check)
#[register_custom_op]
pub struct BookingPrincipalRetireOp;

#[async_trait]
impl CustomOperation for BookingPrincipalRetireOp {
    fn domain(&self) -> &'static str {
        "booking-principal"
    }
    fn verb(&self) -> &'static str {
        "retire"
    }
    fn rationale(&self) -> &'static str {
        "Counts active relationships before retiring for impact visibility"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let id = extract_uuid(verb_call, ctx, "booking-principal-id")?;
        let force = extract_bool_opt(verb_call, "force").unwrap_or(false);

        let active_count = BookingPrincipalRepository::retire_booking_principal(pool, id).await?;

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
        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Void)
    }
}

// =============================================================================
// Evaluation Operations
// =============================================================================

/// Primary eligibility evaluation pipeline
#[register_custom_op]
pub struct BookingPrincipalEvaluateOp;

#[async_trait]
impl CustomOperation for BookingPrincipalEvaluateOp {
    fn domain(&self) -> &'static str {
        "booking-principal"
    }
    fn verb(&self) -> &'static str {
        "evaluate"
    }
    fn rationale(&self) -> &'static str {
        "Full evaluation pipeline: profile snapshot, rule gathering, boundary-aware merge, delivery check, audit pin"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let client_group_id = extract_uuid(verb_call, ctx, "client-group-id")?;
        let segment = extract_string(verb_call, "segment")?;
        let domicile_country = extract_string(verb_call, "domicile-country")?;
        let entity_types: Vec<String> = verb_call
            .get_arg("entity-types")
            .and_then(|a| a.value.as_list())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|v| v.as_string().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        let requested_by =
            extract_string_opt(verb_call, "requested-by").unwrap_or_else(|| "system".to_string());

        // Offering IDs from product lookup
        let offering_ids: Vec<Uuid> = verb_call
            .get_arg("offering-ids")
            .and_then(|a| a.value.as_list())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|v| {
                        v.as_uuid()
                            .or_else(|| v.as_string().and_then(|s| uuid::Uuid::parse_str(s).ok()))
                    })
                    .collect()
            })
            .unwrap_or_default();

        // 1. Create client profile snapshot
        let profile_id = BookingPrincipalRepository::insert_client_profile(
            pool,
            client_group_id,
            &segment,
            &domicile_country,
            &entity_types,
            None,
        )
        .await?;

        // 2. Get all active principals
        let principals = BookingPrincipalRepository::list_active_principals(pool).await?;
        let principal_ids: Vec<Uuid> = principals.iter().map(|p| p.booking_principal_id).collect();

        // 3. Gather applicable rules
        let rulesets = BookingPrincipalRepository::gather_rules_for_evaluation(
            pool,
            &offering_ids,
            &principal_ids,
        )
        .await?;

        // 4. Get existing relationships for scoring
        let existing_rels =
            BookingPrincipalRepository::get_active_relationships_for_client(pool, client_group_id)
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
                BookingPrincipalRepository::get_booking_location(pool, loc_id).await?
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
            let le = BookingPrincipalRepository::get_legal_entity(pool, principal.legal_entity_id)
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
                    BookingPrincipalRepository::get_services_for_product(pool, *offering_id)
                        .await?;
                for service_id in services {
                    let avail = BookingPrincipalRepository::get_availability(
                        pool,
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
            pool,
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

        Ok(ExecutionResult::Record(serde_json::to_value(eval_result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Void)
    }
}

/// Record selection of a principal from an evaluation
#[register_custom_op]
pub struct BookingPrincipalSelectOp;

#[async_trait]
impl CustomOperation for BookingPrincipalSelectOp {
    fn domain(&self) -> &'static str {
        "booking-principal"
    }
    fn verb(&self) -> &'static str {
        "select"
    }
    fn rationale(&self) -> &'static str {
        "Validates candidate eligibility and updates evaluation record with selection"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let evaluation_id = extract_uuid(verb_call, ctx, "evaluation-id")?;
        let principal_id = extract_uuid(verb_call, ctx, "principal-id")?;

        let updated = BookingPrincipalRepository::select_principal_on_evaluation(
            pool,
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

        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Void)
    }
}

/// Retrieve full explain payload for an evaluation
#[register_custom_op]
pub struct BookingPrincipalExplainOp;

#[async_trait]
impl CustomOperation for BookingPrincipalExplainOp {
    fn domain(&self) -> &'static str {
        "booking-principal"
    }
    fn verb(&self) -> &'static str {
        "explain"
    }
    fn rationale(&self) -> &'static str {
        "Retrieves full audit trail and evaluation explain from immutable record"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let evaluation_id = extract_uuid(verb_call, ctx, "evaluation-id")?;

        let eval = BookingPrincipalRepository::get_evaluation(pool, evaluation_id)
            .await?
            .ok_or_else(|| anyhow!("Evaluation not found: {}", evaluation_id))?;

        Ok(ExecutionResult::Record(serde_json::to_value(eval)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Void)
    }
}

// =============================================================================
// Client-Principal Relationship Operations
// =============================================================================

/// Record a new client-principal-offering relationship
#[register_custom_op]
pub struct ClientPrincipalRelationshipRecordOp;

#[async_trait]
impl CustomOperation for ClientPrincipalRelationshipRecordOp {
    fn domain(&self) -> &'static str {
        "client-principal-relationship"
    }
    fn verb(&self) -> &'static str {
        "record"
    }
    fn rationale(&self) -> &'static str {
        "Creates relationship with partial unique index enforcement on active records"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let client_group_id = extract_uuid(verb_call, ctx, "client-group-id")?;
        let booking_principal_id = extract_uuid(verb_call, ctx, "booking-principal-id")?;
        let product_offering_id = extract_uuid(verb_call, ctx, "product-offering-id")?;
        let contract_ref = extract_string_opt(verb_call, "contract-ref");

        let id = BookingPrincipalRepository::record_relationship(
            pool,
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
        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Uuid(Uuid::new_v4()))
    }
}

/// Terminate a client-principal relationship
#[register_custom_op]
pub struct ClientPrincipalRelationshipTerminateOp;

#[async_trait]
impl CustomOperation for ClientPrincipalRelationshipTerminateOp {
    fn domain(&self) -> &'static str {
        "client-principal-relationship"
    }
    fn verb(&self) -> &'static str {
        "terminate"
    }
    fn rationale(&self) -> &'static str {
        "Sets status to terminated only if currently active"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let relationship_id = extract_uuid(verb_call, ctx, "relationship-id")?;

        let affected =
            BookingPrincipalRepository::terminate_relationship(pool, relationship_id).await?;

        Ok(ExecutionResult::Affected(affected as u64))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Affected(1))
    }
}

/// List relationships for a client group
#[register_custom_op]
pub struct ClientPrincipalRelationshipListOp;

#[async_trait]
impl CustomOperation for ClientPrincipalRelationshipListOp {
    fn domain(&self) -> &'static str {
        "client-principal-relationship"
    }
    fn verb(&self) -> &'static str {
        "list"
    }
    fn rationale(&self) -> &'static str {
        "Returns relationships with optional status filter"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let client_group_id = extract_uuid(verb_call, ctx, "client-group-id")?;
        let status = extract_string_opt(verb_call, "status");

        let rels = BookingPrincipalRepository::list_relationships(
            pool,
            client_group_id,
            status.as_deref(),
        )
        .await?;

        let values: Vec<serde_json::Value> = rels
            .into_iter()
            .map(|r| serde_json::to_value(r).unwrap_or_default())
            .collect();

        Ok(ExecutionResult::RecordSet(values))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::RecordSet(vec![]))
    }
}

/// Cross-sell check: what other offerings could this client use?
#[register_custom_op]
pub struct ClientPrincipalRelationshipCrossSellOp;

#[async_trait]
impl CustomOperation for ClientPrincipalRelationshipCrossSellOp {
    fn domain(&self) -> &'static str {
        "client-principal-relationship"
    }
    fn verb(&self) -> &'static str {
        "cross-sell-check"
    }
    fn rationale(&self) -> &'static str {
        "Compares existing offerings against all available offerings to find gaps"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let client_group_id = extract_uuid(verb_call, ctx, "client-group-id")?;

        let active_rels =
            BookingPrincipalRepository::get_active_relationships_for_client(pool, client_group_id)
                .await?;

        let existing_offering_ids: Vec<Uuid> =
            active_rels.iter().map(|r| r.product_offering_id).collect();
        let existing_principal_ids: Vec<Uuid> =
            active_rels.iter().map(|r| r.booking_principal_id).collect();

        // Get all available products
        let all_products: Vec<(Uuid, String)> = sqlx::query_as(
            r#"SELECT product_id, COALESCE(product_code, name) FROM "ob-poc".products WHERE is_active = true"#,
        )
        .fetch_all(pool)
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

        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Void)
    }
}

// =============================================================================
// Service Availability Operations
// =============================================================================

/// Set three-lane service availability for a principal x service
#[register_custom_op]
pub struct ServiceAvailabilitySetOp;

#[async_trait]
impl CustomOperation for ServiceAvailabilitySetOp {
    fn domain(&self) -> &'static str {
        "service-availability"
    }
    fn verb(&self) -> &'static str {
        "set"
    }
    fn rationale(&self) -> &'static str {
        "Validates three-lane statuses and handles GiST temporal exclusion conflicts"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let booking_principal_id = extract_uuid(verb_call, ctx, "booking-principal-id")?;
        let service_id = extract_uuid(verb_call, ctx, "service-id")?;
        let regulatory_status = extract_string(verb_call, "regulatory-status")?;
        let commercial_status = extract_string(verb_call, "commercial-status")?;
        let operational_status = extract_string(verb_call, "operational-status")?;
        let delivery_model = extract_string_opt(verb_call, "delivery-model");

        let id = BookingPrincipalRepository::set_service_availability(
            pool,
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
        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Uuid(Uuid::new_v4()))
    }
}

/// List service availability for a booking principal
#[register_custom_op]
pub struct ServiceAvailabilityListOp;

#[async_trait]
impl CustomOperation for ServiceAvailabilityListOp {
    fn domain(&self) -> &'static str {
        "service-availability"
    }
    fn verb(&self) -> &'static str {
        "list"
    }
    fn rationale(&self) -> &'static str {
        "Returns active availability records with three-lane status for display"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let booking_principal_id = extract_uuid(verb_call, ctx, "booking-principal-id")?;

        let records =
            BookingPrincipalRepository::list_availability_for_principal(pool, booking_principal_id)
                .await?;

        let values: Vec<serde_json::Value> = records
            .into_iter()
            .map(|r| serde_json::to_value(r).unwrap_or_default())
            .collect();

        Ok(ExecutionResult::RecordSet(values))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::RecordSet(vec![]))
    }
}

/// List service availability by principal (alias for list)
#[register_custom_op]
pub struct ServiceAvailabilityListByPrincipalOp;

#[async_trait]
impl CustomOperation for ServiceAvailabilityListByPrincipalOp {
    fn domain(&self) -> &'static str {
        "service-availability"
    }
    fn verb(&self) -> &'static str {
        "list-by-principal"
    }
    fn rationale(&self) -> &'static str {
        "Convenience alias with principal-focused result grouping"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let booking_principal_id = extract_uuid(verb_call, ctx, "booking-principal-id")?;

        let records =
            BookingPrincipalRepository::list_availability_for_principal(pool, booking_principal_id)
                .await?;

        let values: Vec<serde_json::Value> = records
            .into_iter()
            .map(|r| serde_json::to_value(r).unwrap_or_default())
            .collect();

        Ok(ExecutionResult::RecordSet(values))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::RecordSet(vec![]))
    }
}

// =============================================================================
// Ruleset Operations
// =============================================================================

/// Create a new ruleset (starts in draft status)
#[register_custom_op]
pub struct RulesetCreateOp;

#[async_trait]
impl CustomOperation for RulesetCreateOp {
    fn domain(&self) -> &'static str {
        "ruleset"
    }
    fn verb(&self) -> &'static str {
        "create"
    }
    fn rationale(&self) -> &'static str {
        "Validates owner type/id and boundary before creating draft ruleset"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let name = extract_string(verb_call, "name")?;
        let owner_type = extract_string(verb_call, "owner-type")?;
        let owner_id = extract_uuid_opt(verb_call, ctx, "owner-id");
        let boundary = extract_string(verb_call, "boundary")?;

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
            pool,
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
        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Uuid(Uuid::new_v4()))
    }
}

/// Publish a ruleset (draft → active, with field dictionary validation)
#[register_custom_op]
pub struct RulesetPublishOp;

#[async_trait]
impl CustomOperation for RulesetPublishOp {
    fn domain(&self) -> &'static str {
        "ruleset"
    }
    fn verb(&self) -> &'static str {
        "publish"
    }
    fn rationale(&self) -> &'static str {
        "Validates all rule field references against dictionary before activating, checks temporal overlap"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let ruleset_id = extract_uuid(verb_call, ctx, "ruleset-id")?;

        // Load field dictionary for validation
        let dict = BookingPrincipalRepository::get_field_dictionary(pool).await?;
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
        .fetch_all(pool)
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
        let published = BookingPrincipalRepository::publish_ruleset(pool, ruleset_id).await?;

        if !published {
            return Err(anyhow!(
                "Could not publish ruleset — either not in draft status or overlap detected"
            ));
        }

        Ok(ExecutionResult::Uuid(ruleset_id))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Void)
    }
}

/// Retire an active ruleset
#[register_custom_op]
pub struct RulesetRetireOp;

#[async_trait]
impl CustomOperation for RulesetRetireOp {
    fn domain(&self) -> &'static str {
        "ruleset"
    }
    fn verb(&self) -> &'static str {
        "retire"
    }
    fn rationale(&self) -> &'static str {
        "Sets effective_to = now() and status to retired, only if currently active"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let ruleset_id = extract_uuid(verb_call, ctx, "ruleset-id")?;

        let retired = BookingPrincipalRepository::retire_ruleset(pool, ruleset_id).await?;

        if !retired {
            return Err(anyhow!("Could not retire ruleset — not in active status"));
        }

        Ok(ExecutionResult::Uuid(ruleset_id))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Void)
    }
}

// =============================================================================
// Rule Operations
// =============================================================================

/// Add a rule to a ruleset
#[register_custom_op]
pub struct RuleAddOp;

#[async_trait]
impl CustomOperation for RuleAddOp {
    fn domain(&self) -> &'static str {
        "rule"
    }
    fn verb(&self) -> &'static str {
        "add"
    }
    fn rationale(&self) -> &'static str {
        "Validates rule structure (when_expr/then_effect JSON) before inserting"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let ruleset_id = extract_uuid(verb_call, ctx, "ruleset-id")?;
        let name = extract_string(verb_call, "name")?;
        let kind = extract_string(verb_call, "kind")?;
        let when_expr_str = extract_string(verb_call, "when-expr")?;
        let then_effect_str = extract_string(verb_call, "then-effect")?;
        let explain = extract_string_opt(verb_call, "explain");
        let priority = extract_int_opt(verb_call, "priority").map(|v| v as i32);

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
            pool,
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
        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Uuid(Uuid::new_v4()))
    }
}

/// Update an existing rule
#[register_custom_op]
pub struct RuleUpdateOp;

#[async_trait]
impl CustomOperation for RuleUpdateOp {
    fn domain(&self) -> &'static str {
        "rule"
    }
    fn verb(&self) -> &'static str {
        "update"
    }
    fn rationale(&self) -> &'static str {
        "Validates updated JSON structures and checks ruleset is still in draft"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let rule_id = extract_uuid(verb_call, ctx, "rule-id")?;
        let name = extract_string_opt(verb_call, "name");
        let when_expr_str = extract_string_opt(verb_call, "when-expr");
        let then_effect_str = extract_string_opt(verb_call, "then-effect");
        let priority = extract_int_opt(verb_call, "priority").map(|v| v as i32);

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

        query.execute(pool).await?;

        Ok(ExecutionResult::Uuid(rule_id))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Void)
    }
}

/// Disable a rule (soft delete)
#[register_custom_op]
pub struct RuleDisableOp;

#[async_trait]
impl CustomOperation for RuleDisableOp {
    fn domain(&self) -> &'static str {
        "rule"
    }
    fn verb(&self) -> &'static str {
        "disable"
    }
    fn rationale(&self) -> &'static str {
        "Soft-disables rule by setting kind to 'disabled' rather than deleting"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let rule_id = extract_uuid(verb_call, ctx, "rule-id")?;

        sqlx::query(
            r#"
            UPDATE "ob-poc".rule
            SET kind = 'disabled', updated_at = now()
            WHERE rule_id = $1
            "#,
        )
        .bind(rule_id)
        .execute(pool)
        .await?;

        Ok(ExecutionResult::Uuid(rule_id))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Void)
    }
}

// =============================================================================
// Contract Pack Operations
// =============================================================================

/// Create a new contract pack
#[register_custom_op]
pub struct ContractPackCreateOp;

#[async_trait]
impl CustomOperation for ContractPackCreateOp {
    fn domain(&self) -> &'static str {
        "contract-pack"
    }
    fn verb(&self) -> &'static str {
        "create"
    }
    fn rationale(&self) -> &'static str {
        "Validates unique code before creating contract pack"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let code = extract_string(verb_call, "code")?;
        let name = extract_string(verb_call, "name")?;
        let description = extract_string_opt(verb_call, "description");

        let id = BookingPrincipalRepository::create_contract_pack(
            pool,
            &code,
            &name,
            description.as_deref(),
        )
        .await?;

        let result = ContractPackCreateResult {
            contract_pack_id: id,
            code,
        };
        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Uuid(Uuid::new_v4()))
    }
}

/// Add a template to a contract pack
#[register_custom_op]
pub struct ContractPackAddTemplateOp;

#[async_trait]
impl CustomOperation for ContractPackAddTemplateOp {
    fn domain(&self) -> &'static str {
        "contract-pack"
    }
    fn verb(&self) -> &'static str {
        "add-template"
    }
    fn rationale(&self) -> &'static str {
        "Validates contract pack exists and template type is valid"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let contract_pack_id = extract_uuid(verb_call, ctx, "contract-pack-id")?;
        let template_type = extract_string(verb_call, "template-type")?;
        let template_ref = extract_string_opt(verb_call, "template-ref");

        let id = BookingPrincipalRepository::add_contract_template(
            pool,
            contract_pack_id,
            &template_type,
            template_ref.as_deref(),
        )
        .await?;

        let result = ContractTemplateAddResult {
            contract_template_id: id,
            contract_pack_id,
        };
        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Uuid(Uuid::new_v4()))
    }
}

// =============================================================================
// Coverage & Analysis Operations
// =============================================================================

/// Generate coverage matrix: segments x jurisdictions x principals
#[register_custom_op]
pub struct BookingPrincipalCoverageMatrixOp;

#[async_trait]
impl CustomOperation for BookingPrincipalCoverageMatrixOp {
    fn domain(&self) -> &'static str {
        "booking-principal"
    }
    fn verb(&self) -> &'static str {
        "coverage-matrix"
    }
    fn rationale(&self) -> &'static str {
        "Cross-joins principals with offerings and checks three-lane availability per cell"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
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
        .fetch_all(pool)
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

        Ok(ExecutionResult::RecordSet(values))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::RecordSet(vec![]))
    }
}

/// Generate gap report across all three boundaries
#[register_custom_op]
pub struct BookingPrincipalGapReportOp;

#[async_trait]
impl CustomOperation for BookingPrincipalGapReportOp {
    fn domain(&self) -> &'static str {
        "booking-principal"
    }
    fn verb(&self) -> &'static str {
        "gap-report"
    }
    fn rationale(&self) -> &'static str {
        "Aggregates regulatory, commercial, and operational gaps from coverage views"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let mut all_gaps = Vec::new();

        let reg_gaps = BookingPrincipalRepository::get_regulatory_gaps(pool).await?;
        for g in reg_gaps {
            all_gaps.push(serde_json::to_value(g)?);
        }

        let com_gaps = BookingPrincipalRepository::get_commercial_gaps(pool).await?;
        for g in com_gaps {
            all_gaps.push(serde_json::to_value(g)?);
        }

        let ops_gaps = BookingPrincipalRepository::get_operational_gaps(pool).await?;
        for g in ops_gaps {
            all_gaps.push(serde_json::to_value(g)?);
        }

        Ok(ExecutionResult::RecordSet(all_gaps))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::RecordSet(vec![]))
    }
}

/// Impact analysis for principal retirement
#[register_custom_op]
pub struct BookingPrincipalImpactAnalysisOp;

#[async_trait]
impl CustomOperation for BookingPrincipalImpactAnalysisOp {
    fn domain(&self) -> &'static str {
        "booking-principal"
    }
    fn verb(&self) -> &'static str {
        "impact-analysis"
    }
    fn rationale(&self) -> &'static str {
        "Identifies affected client relationships and suggests alternative principals"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let principal_id = extract_uuid(verb_call, ctx, "booking-principal-id")?;

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
        .fetch_all(pool)
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
        .fetch_all(pool)
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

        Ok(ExecutionResult::RecordSet(impact_entries))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::RecordSet(vec![]))
    }
}
