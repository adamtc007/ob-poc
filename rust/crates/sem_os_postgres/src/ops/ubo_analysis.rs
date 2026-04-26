//! UBO-analysis verbs (3 plugin verbs) — SemOS-side YAML-first
//! re-implementation of the plugin subset of
//! `rust/config/verbs/ubo.yaml`.
//!
//! - `ubo.calculate` — bounded recursive ownership chain through
//!   `entity_relationships`, filtered by threshold.
//! - `ubo.trace-chains` — delegates to the
//!   `compute_ownership_chains` SQL function for full chain
//!   traversal with control-path annotation.
//! - `ubo.list-owners` — temporal-aware owners list for an entity.

use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use uuid::Uuid;

use dsl_runtime::domain_ops::helpers::{
    json_extract_cbu_id, json_extract_string_opt, json_extract_uuid, json_extract_uuid_opt,
};
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::SemOsVerbOp;

// ── ubo.calculate ─────────────────────────────────────────────────────────────

pub struct Calculate;

#[async_trait]
impl SemOsVerbOp for Calculate {
    fn fqn(&self) -> &str {
        "ubo.calculate"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_cbu_id(args, ctx)?;
        let threshold: f64 = json_extract_string_opt(args, "threshold")
            .as_deref()
            .and_then(|v| v.parse().ok())
            .unwrap_or(25.0);

        let cbu_entity: Option<(Uuid,)> = sqlx::query_as(
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
        .fetch_optional(scope.executor())
        .await?;

        let target_entity_id = match cbu_entity {
            Some((entity_id,)) => entity_id,
            None => return Ok(VerbExecutionOutcome::RecordSet(vec![])),
        };

        let ubos: Vec<(Uuid, Option<rust_decimal::Decimal>)> = sqlx::query_as(
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
        .fetch_all(scope.executor())
        .await?;

        let ubo_list: Vec<Value> = ubos
            .iter()
            .map(|(entity_id, total)| {
                json!({
                    "entity_id": entity_id,
                    "ownership_percent": total,
                })
            })
            .collect();
        Ok(VerbExecutionOutcome::RecordSet(ubo_list))
    }
}

// ── ubo.trace-chains ──────────────────────────────────────────────────────────

pub struct TraceChains;

#[async_trait]
impl SemOsVerbOp for TraceChains {
    fn fqn(&self) -> &str {
        "ubo.trace-chains"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_cbu_id(args, ctx)?;
        let target_entity_id = json_extract_uuid_opt(args, ctx, "target-entity-id");
        let threshold: f64 = json_extract_string_opt(args, "threshold")
            .as_deref()
            .and_then(|v| v.parse().ok())
            .unwrap_or(25.0);
        let as_of_date = json_extract_string_opt(args, "as-of-date")
            .as_deref()
            .and_then(|v| chrono::NaiveDate::parse_from_str(v, "%Y-%m-%d").ok())
            .unwrap_or_else(|| chrono::Utc::now().date_naive());

        let threshold_bd: sqlx::types::BigDecimal = threshold
            .to_string()
            .parse()
            .unwrap_or_else(|_| sqlx::types::BigDecimal::from(25));

        type TraceChainRow = (
            Uuid,
            Option<Uuid>,
            Option<String>,
            Option<Vec<Uuid>>,
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
        .fetch_all(scope.executor())
        .await?;

        let chain_list: Vec<Value> = chains
            .iter()
            .map(|(chain_id, ubo_person_id, ubo_name, path_entities, path_names, ownership_pcts, effective_ownership, chain_depth, is_complete, relationship_types, has_control_path)| {
                json!({
                    "chain_id": chain_id,
                    "ubo_person_id": ubo_person_id,
                    "ubo_name": ubo_name,
                    "path_entities": path_entities,
                    "path_names": path_names,
                    "ownership_percentages": ownership_pcts,
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
                    },
                })
            })
            .collect();

        let ownership_chains = chain_list
            .iter()
            .filter(|c| c.get("ubo_type").and_then(|v| v.as_str()) != Some("CONTROL"))
            .count();
        let control_chains = chain_list
            .iter()
            .filter(|c| {
                let t = c.get("ubo_type").and_then(|v| v.as_str());
                t == Some("CONTROL") || t == Some("OWNERSHIP_AND_CONTROL")
            })
            .count();

        Ok(VerbExecutionOutcome::Record(json!({
            "cbu_id": cbu_id,
            "target_entity_id": target_entity_id,
            "threshold": threshold,
            "as_of_date": as_of_date.to_string(),
            "chain_count": chain_list.len(),
            "chains": chain_list,
            "ownership_chain_count": ownership_chains,
            "control_chain_count": control_chains,
            "includes_control_relationships": true,
        })))
    }
}

// ── ubo.list-owners ───────────────────────────────────────────────────────────

pub struct ListOwners;

#[async_trait]
impl SemOsVerbOp for ListOwners {
    fn fqn(&self) -> &str {
        "ubo.list-owners"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let entity_id = json_extract_uuid(args, ctx, "entity-id")?;
        let as_of_date = json_extract_string_opt(args, "as-of-date")
            .as_deref()
            .and_then(|v| chrono::NaiveDate::parse_from_str(v, "%Y-%m-%d").ok())
            .unwrap_or_else(|| chrono::Utc::now().date_naive());

        type OwnerRow = (
            Uuid,
            Uuid,
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
        .fetch_all(scope.executor())
        .await?;

        let owner_list: Vec<Value> = owners
            .iter()
            .map(
                |(
                    rel_id,
                    owner_id,
                    owner_name,
                    owner_type,
                    pct,
                    own_type,
                    eff_from,
                    eff_to,
                    src,
                )| {
                    json!({
                        "relationship_id": rel_id,
                        "owner_entity_id": owner_id,
                        "owner_name": owner_name,
                        "owner_type": owner_type,
                        "percentage": pct,
                        "ownership_type": own_type,
                        "effective_from": eff_from,
                        "effective_to": eff_to,
                        "source": src,
                    })
                },
            )
            .collect();

        Ok(VerbExecutionOutcome::Record(json!({
            "entity_id": entity_id,
            "as_of_date": as_of_date.to_string(),
            "owner_count": owner_list.len(),
            "owners": owner_list,
        })))
    }
}
