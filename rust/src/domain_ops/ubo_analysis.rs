//! UBO (Ultimate Beneficial Owner) Analysis Operations
//!
//! Enhanced UBO operations for discovery, chain tracing, snapshots, and comparisons.
//! These extend the basic UBO CRUD operations with complex graph traversal and analysis.

use anyhow::Result;
use async_trait::async_trait;
use dsl_runtime_macros::register_custom_op;

use super::helpers::{
    extract_uuid, extract_uuid_opt, json_extract_cbu_id, json_extract_string_opt, json_extract_uuid,
};
use crate::domain_ops::CustomOperation;
use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};

#[cfg(feature = "database")]
use sqlx::PgPool;

/// Calculate UBO (Ultimate Beneficial Ownership) chain
///
/// Rationale: Requires recursive graph traversal through ownership chains
/// to identify beneficial owners above the specified threshold.
#[register_custom_op]
pub struct UboCalculateOp;

#[cfg(feature = "database")]
async fn ubo_calculate_impl(
    cbu_id: uuid::Uuid,
    threshold: f64,
    pool: &PgPool,
) -> Result<ExecutionResult> {
    use serde_json::json;

    let cbu_entity: Option<(uuid::Uuid,)> = sqlx::query_as(
        r#"
        SELECT e.entity_id
        FROM "ob-poc".cbu_entity_roles cer
        JOIN "ob-poc".entities e ON e.entity_id = cer.entity_id
        JOIN "ob-poc".roles r ON r.role_id = cer.role_id
        WHERE cer.cbu_id = $1
          AND e.deleted_at IS NULL
          AND r.name IN ('Primary Entity', 'Main Entity', 'Client')
        LIMIT 1
        "#,
    )
    .bind(cbu_id)
    .fetch_optional(pool)
    .await?;

    let target_entity_id = match cbu_entity {
        Some((entity_id,)) => entity_id,
        None => return Ok(ExecutionResult::RecordSet(vec![])),
    };

    let ubos: Vec<(uuid::Uuid, Option<rust_decimal::Decimal>)> = sqlx::query_as(
        r#"
        WITH RECURSIVE ownership_chain AS (
            SELECT
                r.from_entity_id as entity_id,
                r.percentage as ownership_percent,
                ARRAY[r.from_entity_id] as path,
                1 as depth
            FROM "ob-poc".entity_relationships r
            WHERE r.to_entity_id = $1
            AND r.relationship_type = 'ownership'
            AND r.ownership_type IN ('DIRECT', 'BENEFICIAL', 'direct', 'beneficial')
            AND (r.effective_to IS NULL OR r.effective_to > CURRENT_DATE)

            UNION ALL

            SELECT
                r2.from_entity_id as entity_id,
                (oc.ownership_percent * r2.percentage / 100)::numeric(5,2) as ownership_percent,
                oc.path || r2.from_entity_id,
                oc.depth + 1
            FROM ownership_chain oc
            JOIN "ob-poc".entity_relationships r2 ON r2.to_entity_id = oc.entity_id
            WHERE oc.depth < 10
            AND r2.relationship_type = 'ownership'
            AND NOT r2.from_entity_id = ANY(oc.path)
            AND (r2.effective_to IS NULL OR r2.effective_to > CURRENT_DATE)
        )
        SELECT
            entity_id,
            SUM(ownership_percent) as total_ownership
        FROM ownership_chain
        GROUP BY entity_id
        HAVING SUM(ownership_percent) >= $2
        ORDER BY total_ownership DESC
        "#,
    )
    .bind(target_entity_id)
    .bind(sqlx::types::BigDecimal::try_from(threshold).ok())
    .fetch_all(pool)
    .await?;

    let ubo_list: Vec<serde_json::Value> = ubos
        .iter()
        .map(|(entity_id, total_ownership)| {
            json!({
                "entity_id": entity_id,
                "ownership_percent": total_ownership
            })
        })
        .collect();

    Ok(ExecutionResult::RecordSet(ubo_list))
}

#[async_trait]
impl CustomOperation for UboCalculateOp {
    fn domain(&self) -> &'static str {
        "ubo"
    }
    fn verb(&self) -> &'static str {
        "calculate"
    }
    fn rationale(&self) -> &'static str {
        "Requires recursive graph traversal through ownership hierarchy"
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        let cbu_id = json_extract_cbu_id(args, ctx)?;
        let threshold = json_extract_string_opt(args, "threshold")
            .as_deref()
            .and_then(|value| value.parse::<f64>().ok())
            .unwrap_or(25.0);

        match ubo_calculate_impl(cbu_id, threshold, pool).await? {
            ExecutionResult::RecordSet(rows) => Ok(
                dsl_runtime::VerbExecutionOutcome::RecordSet(rows),
            ),
            _ => unreachable!(),
        }
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

impl UboCalculateOp {

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let cbu_id = extract_uuid(verb_call, ctx, "cbu-id")?;
        let threshold: f64 = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "threshold")
            .and_then(|a| a.value.as_decimal())
            .map(|d| d.to_string().parse().unwrap_or(25.0))
            .unwrap_or(25.0);

        ubo_calculate_impl(cbu_id, threshold, pool).await
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

/// Discover potential UBOs from document extraction or registry lookup
///
/// Rationale: Orchestrates document extraction and registry lookups to identify
/// potential beneficial owners, creating preliminary ownership records.
#[register_custom_op]
pub struct UboTraceChainsOp;

#[cfg(feature = "database")]
async fn ubo_trace_chains_impl(
    cbu_id: uuid::Uuid,
    target_entity_id: Option<uuid::Uuid>,
    threshold: f64,
    as_of_date: chrono::NaiveDate,
    pool: &PgPool,
) -> Result<ExecutionResult> {
    use serde_json::json;

    let threshold_bd: sqlx::types::BigDecimal = threshold
        .to_string()
        .parse()
        .unwrap_or_else(|_| sqlx::types::BigDecimal::from(25));

    type TraceChainRow = (
        uuid::Uuid,
        Option<uuid::Uuid>,
        Option<String>,
        Option<Vec<uuid::Uuid>>,
        Option<Vec<String>>,
        Option<Vec<Option<rust_decimal::Decimal>>>,
        Option<rust_decimal::Decimal>,
        Option<i32>,
        Option<bool>,
        Option<Vec<String>>,
        Option<bool>,
    );

    let chains: Vec<TraceChainRow> = sqlx::query_as(
        r#"SELECT chain_id, ubo_person_id, ubo_name,
                  path_entities, path_names, ownership_percentages,
                  effective_ownership, chain_depth, is_complete,
                  relationship_types, has_control_path
           FROM "ob-poc".compute_ownership_chains($1, $2, 10, $4)
           WHERE effective_ownership >= $3 OR has_control_path = true
           ORDER BY effective_ownership DESC NULLS LAST"#,
    )
    .bind(cbu_id)
    .bind(target_entity_id)
    .bind(threshold_bd)
    .bind(as_of_date)
    .fetch_all(pool)
    .await?;

    let chain_list: Vec<serde_json::Value> = chains
        .iter()
        .map(
            |(
                chain_id,
                ubo_person_id,
                ubo_name,
                path_entities,
                path_names,
                ownership_percentages,
                effective_ownership,
                chain_depth,
                is_complete,
                relationship_types,
                has_control_path,
            )| {
            json!({
                "chain_id": chain_id,
                "ubo_person_id": ubo_person_id,
                "ubo_name": ubo_name,
                "path_entities": path_entities,
                "path_names": path_names,
                "ownership_percentages": ownership_percentages,
                "effective_ownership": effective_ownership,
                "chain_depth": chain_depth,
                "is_complete": is_complete,
                "relationship_types": relationship_types,
                "has_control_path": has_control_path,
                "ubo_type": if *has_control_path == Some(true) && effective_ownership.is_none() {
                    "CONTROL"
                } else if *has_control_path == Some(true) {
                    "OWNERSHIP_AND_CONTROL"
                } else {
                    "OWNERSHIP"
                }
            })
        },
        )
        .collect();

    let ownership_chains: Vec<&serde_json::Value> = chain_list
        .iter()
        .filter(|c| c.get("ubo_type").and_then(|v| v.as_str()) != Some("CONTROL"))
        .collect();

    let control_chains: Vec<&serde_json::Value> = chain_list
        .iter()
        .filter(|c| {
            let ubo_type = c.get("ubo_type").and_then(|v| v.as_str());
            ubo_type == Some("CONTROL") || ubo_type == Some("OWNERSHIP_AND_CONTROL")
        })
        .collect();

    let result = json!({
        "cbu_id": cbu_id,
        "target_entity_id": target_entity_id,
        "threshold": threshold,
        "as_of_date": as_of_date.to_string(),
        "chain_count": chain_list.len(),
        "chains": chain_list,
        "ownership_chain_count": ownership_chains.len(),
        "control_chain_count": control_chains.len(),
        "includes_control_relationships": true
    });

    Ok(ExecutionResult::Record(result))
}

#[async_trait]
impl CustomOperation for UboTraceChainsOp {
    fn domain(&self) -> &'static str {
        "ubo"
    }
    fn verb(&self) -> &'static str {
        "trace-chains"
    }
    fn rationale(&self) -> &'static str {
        "Calls SQL recursive function to compute ownership chains"
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        let cbu_id = json_extract_cbu_id(args, ctx)?;
        let target_entity_id = super::helpers::json_extract_uuid_opt(args, ctx, "target-entity-id");
        let threshold = json_extract_string_opt(args, "threshold")
            .as_deref()
            .and_then(|value| value.parse::<f64>().ok())
            .unwrap_or(25.0);
        let as_of_date = json_extract_string_opt(args, "as-of-date")
            .as_deref()
            .and_then(|value| chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d").ok())
            .unwrap_or_else(|| chrono::Utc::now().date_naive());

        match ubo_trace_chains_impl(cbu_id, target_entity_id, threshold, as_of_date, pool).await? {
            ExecutionResult::Record(value) => {
                Ok(dsl_runtime::VerbExecutionOutcome::Record(value))
            }
            _ => unreachable!(),
        }
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

impl UboTraceChainsOp {

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let cbu_id = extract_uuid(verb_call, ctx, "cbu-id")?;
        let target_entity_id = extract_uuid_opt(verb_call, ctx, "target-entity-id");

        let threshold: f64 = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "threshold")
            .and_then(|a| a.value.as_decimal())
            .map(|d| d.to_string().parse().unwrap_or(25.0))
            .unwrap_or(25.0);

        // Get as-of-date (optional, defaults to today)
        let as_of_date: chrono::NaiveDate = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "as-of-date")
            .and_then(|a| a.value.as_string())
            .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
            .unwrap_or_else(|| chrono::Utc::now().date_naive());

        ubo_trace_chains_impl(cbu_id, target_entity_id, threshold, as_of_date, pool).await
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::json!({
            "chain_count": 0,
            "chains": []
        })))
    }
}

/// List owners of an entity as of a specific date
///
/// Rationale: Returns ownership relationships for an entity with temporal filtering.
/// This is the temporal-aware version of the CRUD list-owners.
#[register_custom_op]
pub struct UboListOwnersOp;

#[cfg(feature = "database")]
async fn ubo_list_owners_impl(
    entity_id: uuid::Uuid,
    as_of_date: chrono::NaiveDate,
    pool: &PgPool,
) -> Result<ExecutionResult> {
    use serde_json::json;

    type OwnerRow = (
        uuid::Uuid,
        uuid::Uuid,
        Option<String>,
        String,
        Option<rust_decimal::Decimal>,
        Option<String>,
        Option<chrono::NaiveDate>,
        Option<chrono::NaiveDate>,
        Option<String>,
    );

    let owners: Vec<OwnerRow> = sqlx::query_as(
        r#"SELECT
            r.relationship_id,
            r.from_entity_id as owner_entity_id,
            e.name as owner_name,
            et.type_code as owner_type,
            r.percentage,
            r.ownership_type,
            r.effective_from,
            r.effective_to,
            r.source
        FROM "ob-poc".entity_relationships r
        JOIN "ob-poc".entities e ON r.from_entity_id = e.entity_id
        JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
        WHERE r.to_entity_id = $1
          AND e.deleted_at IS NULL
          AND r.relationship_type = 'ownership'
          AND (r.effective_from IS NULL OR r.effective_from <= $2)
          AND (r.effective_to IS NULL OR r.effective_to >= $2)
        ORDER BY r.percentage DESC NULLS LAST"#,
    )
    .bind(entity_id)
    .bind(as_of_date)
    .fetch_all(pool)
    .await?;

    let owner_list: Vec<serde_json::Value> = owners
        .iter()
        .map(
            |(
                relationship_id,
                owner_entity_id,
                owner_name,
                owner_type,
                percentage,
                ownership_type,
                effective_from,
                effective_to,
                source,
            )| {
                json!({
                    "relationship_id": relationship_id,
                    "owner_entity_id": owner_entity_id,
                    "owner_name": owner_name,
                    "owner_type": owner_type,
                    "percentage": percentage,
                    "ownership_type": ownership_type,
                    "effective_from": effective_from,
                    "effective_to": effective_to,
                    "source": source
                })
            },
        )
        .collect();

    let result = json!({
        "entity_id": entity_id,
        "as_of_date": as_of_date.to_string(),
        "owner_count": owner_list.len(),
        "owners": owner_list
    });

    Ok(ExecutionResult::Record(result))
}

#[async_trait]
impl CustomOperation for UboListOwnersOp {
    fn domain(&self) -> &'static str {
        "ubo"
    }
    fn verb(&self) -> &'static str {
        "list-owners"
    }
    fn rationale(&self) -> &'static str {
        "Lists ownership relationships with temporal filtering"
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        let entity_id = json_extract_uuid(args, ctx, "entity-id")?;
        let as_of_date = json_extract_string_opt(args, "as-of-date")
            .as_deref()
            .and_then(|value| chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d").ok())
            .unwrap_or_else(|| chrono::Utc::now().date_naive());

        match ubo_list_owners_impl(entity_id, as_of_date, pool).await? {
            ExecutionResult::Record(value) => {
                Ok(dsl_runtime::VerbExecutionOutcome::Record(value))
            }
            _ => unreachable!(),
        }
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

impl UboListOwnersOp {

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let entity_id = extract_uuid(verb_call, ctx, "entity-id")?;
        let as_of_date: chrono::NaiveDate = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "as-of-date")
            .and_then(|a| a.value.as_string())
            .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
            .unwrap_or_else(|| chrono::Utc::now().date_naive());

        ubo_list_owners_impl(entity_id, as_of_date, pool).await
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::json!({
            "owner_count": 0,
            "owners": []
        })))
    }
}
