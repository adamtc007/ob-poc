//! Temporal-query verbs (8 plugin verbs) — SemOS-side YAML-first
//! re-implementation of the plugin subset of
//! `rust/config/verbs/temporal.yaml`.
//!
//! Point-in-time queries for regulatory lookback ("what did the
//! structure look like on date X?"). Wraps the SQL functions
//! defined in `migrations/005_temporal_query_layer.sql`.

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use chrono::NaiveDate;
use serde_json::{json, Value};
use uuid::Uuid;

use dsl_runtime::domain_ops::helpers::{json_extract_string_opt, json_extract_uuid};
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::SemOsVerbOp;

fn get_date_arg(args: &Value, name: &str) -> Result<NaiveDate> {
    if let Some(val) = json_extract_string_opt(args, name) {
        if val == "today" {
            return Ok(chrono::Utc::now().date_naive());
        }
        return NaiveDate::parse_from_str(&val, "%Y-%m-%d").map_err(|e| {
            anyhow!(
                "Invalid date format for {}: {} (expected YYYY-MM-DD)",
                name,
                e
            )
        });
    }
    Ok(chrono::Utc::now().date_naive())
}

fn get_optional_date_arg(args: &Value, name: &str) -> Result<Option<NaiveDate>> {
    json_extract_string_opt(args, name)
        .map(|s| {
            NaiveDate::parse_from_str(&s, "%Y-%m-%d")
                .map_err(|e| anyhow!("Invalid date format for {}: {}", name, e))
        })
        .transpose()
}

fn get_decimal_arg(args: &Value, name: &str, default: f64) -> f64 {
    json_extract_string_opt(args, name)
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn get_int_arg(args: &Value, name: &str, default: i32) -> i32 {
    json_extract_string_opt(args, name)
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

// ── temporal.ownership-as-of ──────────────────────────────────────────────────

pub struct OwnershipAsOf;

#[async_trait]
impl SemOsVerbOp for OwnershipAsOf {
    fn fqn(&self) -> &str {
        "temporal.ownership-as-of"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let entity_id = json_extract_uuid(args, ctx, "entity-id")?;
        let as_of_date = get_date_arg(args, "as-of-date")?;

        type Row = (
            Uuid,
            Uuid,
            String,
            Uuid,
            String,
            Option<sqlx::types::BigDecimal>,
            Option<String>,
            Option<NaiveDate>,
            Option<NaiveDate>,
        );

        let rows: Vec<Row> = sqlx::query_as(r#"SELECT * FROM "ob-poc".ownership_as_of($1, $2)"#)
            .bind(entity_id)
            .bind(as_of_date)
            .fetch_all(scope.executor())
            .await?;

        let results: Vec<Value> = rows
            .iter()
            .map(|row| {
                json!({
                    "relationship_id": row.0.to_string(),
                    "from_entity_id": row.1.to_string(),
                    "from_entity_name": row.2,
                    "to_entity_id": row.3.to_string(),
                    "to_entity_name": row.4,
                    "percentage": row.5.as_ref().map(|d| d.to_string()),
                    "ownership_type": row.6,
                    "effective_from": row.7.map(|d| d.to_string()),
                    "effective_to": row.8.map(|d| d.to_string()),
                })
            })
            .collect();

        Ok(VerbExecutionOutcome::Record(json!({
            "entity_id": entity_id.to_string(),
            "as_of_date": as_of_date.to_string(),
            "count": results.len(),
            "relationships": results,
        })))
    }
}

// ── temporal.ubo-chain-as-of ──────────────────────────────────────────────────

pub struct UboChainAsOf;

#[async_trait]
impl SemOsVerbOp for UboChainAsOf {
    fn fqn(&self) -> &str {
        "temporal.ubo-chain-as-of"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let entity_id = json_extract_uuid(args, ctx, "entity-id")?;
        let as_of_date = get_date_arg(args, "as-of-date")?;
        let threshold = get_decimal_arg(args, "threshold", 25.0);

        type Row = (
            Vec<Uuid>,
            Vec<String>,
            Uuid,
            String,
            String,
            sqlx::types::BigDecimal,
            i32,
        );

        let rows: Vec<Row> = sqlx::query_as(r#"SELECT * FROM "ob-poc".ubo_chain_as_of($1, $2, $3)"#)
            .bind(entity_id)
            .bind(as_of_date)
            .bind(sqlx::types::BigDecimal::try_from(threshold).unwrap_or_default())
            .fetch_all(scope.executor())
            .await?;

        let chains: Vec<Value> = rows
            .iter()
            .map(|row| {
                json!({
                    "chain_path": row.0.iter().map(|u| u.to_string()).collect::<Vec<_>>(),
                    "chain_names": row.1,
                    "ultimate_owner_id": row.2.to_string(),
                    "ultimate_owner_name": row.3,
                    "ultimate_owner_type": row.4,
                    "effective_percentage": row.5.to_string(),
                    "chain_length": row.6,
                })
            })
            .collect();

        Ok(VerbExecutionOutcome::Record(json!({
            "entity_id": entity_id.to_string(),
            "as_of_date": as_of_date.to_string(),
            "threshold": threshold,
            "count": chains.len(),
            "ubo_chains": chains,
        })))
    }
}

// ── temporal.cbu-relationships-as-of ──────────────────────────────────────────

pub struct CbuRelationshipsAsOf;

#[async_trait]
impl SemOsVerbOp for CbuRelationshipsAsOf {
    fn fqn(&self) -> &str {
        "temporal.cbu-relationships-as-of"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let as_of_date = get_date_arg(args, "as-of-date")?;

        type Row = (
            Uuid,
            Uuid,
            String,
            Uuid,
            String,
            String,
            Option<sqlx::types::BigDecimal>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<NaiveDate>,
            Option<NaiveDate>,
        );

        let rows: Vec<Row> =
            sqlx::query_as(r#"SELECT * FROM "ob-poc".cbu_relationships_as_of($1, $2)"#)
                .bind(cbu_id)
                .bind(as_of_date)
                .fetch_all(scope.executor())
                .await?;

        let results: Vec<Value> = rows
            .iter()
            .map(|row| {
                json!({
                    "relationship_id": row.0.to_string(),
                    "from_entity_id": row.1.to_string(),
                    "from_entity_name": row.2,
                    "to_entity_id": row.3.to_string(),
                    "to_entity_name": row.4,
                    "relationship_type": row.5,
                    "percentage": row.6.as_ref().map(|d| d.to_string()),
                    "ownership_type": row.7,
                    "control_type": row.8,
                    "trust_role": row.9,
                    "effective_from": row.10.map(|d| d.to_string()),
                    "effective_to": row.11.map(|d| d.to_string()),
                })
            })
            .collect();

        Ok(VerbExecutionOutcome::Record(json!({
            "cbu_id": cbu_id.to_string(),
            "as_of_date": as_of_date.to_string(),
            "count": results.len(),
            "relationships": results,
        })))
    }
}

// ── temporal.cbu-roles-as-of ──────────────────────────────────────────────────

pub struct CbuRolesAsOf;

#[async_trait]
impl SemOsVerbOp for CbuRolesAsOf {
    fn fqn(&self) -> &str {
        "temporal.cbu-roles-as-of"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let as_of_date = get_date_arg(args, "as-of-date")?;

        type Row = (
            Uuid,
            String,
            String,
            String,
            Option<NaiveDate>,
            Option<NaiveDate>,
        );

        let rows: Vec<Row> = sqlx::query_as(r#"SELECT * FROM "ob-poc".cbu_roles_as_of($1, $2)"#)
            .bind(cbu_id)
            .bind(as_of_date)
            .fetch_all(scope.executor())
            .await?;

        let results: Vec<Value> = rows
            .iter()
            .map(|row| {
                json!({
                    "entity_id": row.0.to_string(),
                    "entity_name": row.1,
                    "entity_type": row.2,
                    "role_name": row.3,
                    "effective_from": row.4.map(|d| d.to_string()),
                    "effective_to": row.5.map(|d| d.to_string()),
                })
            })
            .collect();

        Ok(VerbExecutionOutcome::Record(json!({
            "cbu_id": cbu_id.to_string(),
            "as_of_date": as_of_date.to_string(),
            "count": results.len(),
            "roles": results,
        })))
    }
}

// ── temporal.cbu-state-at-approval ────────────────────────────────────────────

pub struct CbuStateAtApproval;

#[async_trait]
impl SemOsVerbOp for CbuStateAtApproval {
    fn fqn(&self) -> &str {
        "temporal.cbu-state-at-approval"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;

        type Row = (
            Uuid,
            chrono::DateTime<chrono::Utc>,
            Uuid,
            String,
            String,
            Option<Uuid>,
            Option<sqlx::types::BigDecimal>,
        );

        let rows: Vec<Row> = sqlx::query_as(r#"SELECT * FROM "ob-poc".cbu_state_at_approval($1)"#)
            .bind(cbu_id)
            .fetch_all(scope.executor())
            .await?;

        if rows.is_empty() {
            return Ok(VerbExecutionOutcome::Record(json!({
                "cbu_id": cbu_id.to_string(),
                "error": "No approved KYC case found for this CBU",
            })));
        }

        let case_id = rows[0].0;
        let approved_at = rows[0].1;
        let entities: Vec<Value> = rows
            .iter()
            .map(|row| {
                json!({
                    "entity_id": row.2.to_string(),
                    "entity_name": row.3,
                    "role_name": row.4,
                    "ownership_from": row.5.map(|u| u.to_string()),
                    "ownership_percentage": row.6.as_ref().map(|d| d.to_string()),
                })
            })
            .collect();

        Ok(VerbExecutionOutcome::Record(json!({
            "cbu_id": cbu_id.to_string(),
            "case_id": case_id.to_string(),
            "approved_at": approved_at.to_rfc3339(),
            "count": entities.len(),
            "state_at_approval": entities,
        })))
    }
}

// ── temporal.relationship-history ─────────────────────────────────────────────

pub struct RelationshipHistory;

#[async_trait]
impl SemOsVerbOp for RelationshipHistory {
    fn fqn(&self) -> &str {
        "temporal.relationship-history"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let relationship_id = json_extract_uuid(args, ctx, "relationship-id")?;
        let limit = get_int_arg(args, "limit", 50);

        type Row = (
            Uuid,
            String,
            chrono::DateTime<chrono::Utc>,
            Option<sqlx::types::BigDecimal>,
            Option<String>,
            Option<NaiveDate>,
            Option<NaiveDate>,
        );

        let rows: Vec<Row> = sqlx::query_as(
            r#"
            SELECT
                history_id,
                operation,
                changed_at,
                percentage,
                ownership_type,
                effective_from,
                effective_to
            FROM "ob-poc".entity_relationships_history
            WHERE relationship_id = $1
            ORDER BY changed_at DESC
            LIMIT $2
            "#,
        )
        .bind(relationship_id)
        .bind(limit)
        .fetch_all(scope.executor())
        .await?;

        let history: Vec<Value> = rows
            .iter()
            .map(|row| {
                json!({
                    "history_id": row.0.to_string(),
                    "operation": row.1,
                    "changed_at": row.2.to_rfc3339(),
                    "percentage": row.3.as_ref().map(|d| d.to_string()),
                    "ownership_type": row.4,
                    "effective_from": row.5.map(|d| d.to_string()),
                    "effective_to": row.6.map(|d| d.to_string()),
                })
            })
            .collect();

        Ok(VerbExecutionOutcome::Record(json!({
            "relationship_id": relationship_id.to_string(),
            "count": history.len(),
            "history": history,
        })))
    }
}

// ── temporal.entity-history ───────────────────────────────────────────────────

pub struct EntityHistory;

#[async_trait]
impl SemOsVerbOp for EntityHistory {
    fn fqn(&self) -> &str {
        "temporal.entity-history"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let entity_id = json_extract_uuid(args, ctx, "entity-id")?;
        let from_date = get_optional_date_arg(args, "from-date")?;
        let to_date = get_optional_date_arg(args, "to-date")?;
        let limit = get_int_arg(args, "limit", 100);

        type Row = (
            Uuid,
            Uuid,
            String,
            Uuid,
            Uuid,
            String,
            chrono::DateTime<chrono::Utc>,
            Option<sqlx::types::BigDecimal>,
        );

        let rows: Vec<Row> = sqlx::query_as(
            r#"
            SELECT
                h.history_id,
                h.relationship_id,
                h.relationship_type,
                h.from_entity_id,
                h.to_entity_id,
                h.operation,
                h.changed_at,
                h.percentage
            FROM "ob-poc".entity_relationships_history h
            WHERE (h.from_entity_id = $1 OR h.to_entity_id = $1)
              AND ($2::date IS NULL OR h.changed_at::date >= $2)
              AND ($3::date IS NULL OR h.changed_at::date <= $3)
            ORDER BY h.changed_at DESC
            LIMIT $4
            "#,
        )
        .bind(entity_id)
        .bind(from_date)
        .bind(to_date)
        .bind(limit)
        .fetch_all(scope.executor())
        .await?;

        let history: Vec<Value> = rows
            .iter()
            .map(|row| {
                json!({
                    "history_id": row.0.to_string(),
                    "relationship_id": row.1.to_string(),
                    "relationship_type": row.2,
                    "from_entity_id": row.3.to_string(),
                    "to_entity_id": row.4.to_string(),
                    "operation": row.5,
                    "changed_at": row.6.to_rfc3339(),
                    "percentage": row.7.as_ref().map(|d| d.to_string()),
                })
            })
            .collect();

        Ok(VerbExecutionOutcome::Record(json!({
            "entity_id": entity_id.to_string(),
            "from_date": from_date.map(|d| d.to_string()),
            "to_date": to_date.map(|d| d.to_string()),
            "count": history.len(),
            "history": history,
        })))
    }
}

// ── temporal.compare-ownership ────────────────────────────────────────────────

pub struct CompareOwnership;

#[async_trait]
impl SemOsVerbOp for CompareOwnership {
    fn fqn(&self) -> &str {
        "temporal.compare-ownership"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let entity_id = json_extract_uuid(args, ctx, "entity-id")?;
        let date_a = get_optional_date_arg(args, "date-a")?
            .ok_or_else(|| anyhow!("date-a is required"))?;
        let date_b = get_optional_date_arg(args, "date-b")?
            .ok_or_else(|| anyhow!("date-b is required"))?;

        type OwnershipRow = (Uuid, Uuid, Option<sqlx::types::BigDecimal>, Option<String>);

        let rows_a: Vec<OwnershipRow> = sqlx::query_as(
            r#"
            SELECT relationship_id, from_entity_id, percentage, ownership_type
            FROM "ob-poc".ownership_as_of($1, $2)
            "#,
        )
        .bind(entity_id)
        .bind(date_a)
        .fetch_all(scope.executor())
        .await?;

        let rows_b: Vec<OwnershipRow> = sqlx::query_as(
            r#"
            SELECT relationship_id, from_entity_id, percentage, ownership_type
            FROM "ob-poc".ownership_as_of($1, $2)
            "#,
        )
        .bind(entity_id)
        .bind(date_b)
        .fetch_all(scope.executor())
        .await?;

        let set_a: HashMap<Uuid, &OwnershipRow> = rows_a.iter().map(|r| (r.0, r)).collect();
        let set_b: HashMap<Uuid, &OwnershipRow> = rows_b.iter().map(|r| (r.0, r)).collect();

        let mut added = Vec::new();
        let mut removed = Vec::new();
        let mut changed = Vec::new();

        for (id, row) in &set_b {
            if !set_a.contains_key(id) {
                added.push(json!({
                    "relationship_id": id.to_string(),
                    "from_entity_id": row.1.to_string(),
                    "percentage": row.2.as_ref().map(|d| d.to_string()),
                    "ownership_type": row.3,
                }));
            }
        }
        for (id, row) in &set_a {
            if !set_b.contains_key(id) {
                removed.push(json!({
                    "relationship_id": id.to_string(),
                    "from_entity_id": row.1.to_string(),
                    "percentage": row.2.as_ref().map(|d| d.to_string()),
                    "ownership_type": row.3,
                }));
            }
        }
        for (id, row_a) in &set_a {
            if let Some(row_b) = set_b.get(id) {
                let pct_a = row_a.2.as_ref().map(|d| d.to_string());
                let pct_b = row_b.2.as_ref().map(|d| d.to_string());
                if pct_a != pct_b || row_a.3 != row_b.3 {
                    changed.push(json!({
                        "relationship_id": id.to_string(),
                        "from_entity_id": row_a.1.to_string(),
                        "percentage_before": pct_a,
                        "percentage_after": pct_b,
                        "ownership_type_before": row_a.3,
                        "ownership_type_after": row_b.3,
                    }));
                }
            }
        }

        Ok(VerbExecutionOutcome::Record(json!({
            "entity_id": entity_id.to_string(),
            "date_a": date_a.to_string(),
            "date_b": date_b.to_string(),
            "summary": {
                "added": added.len(),
                "removed": removed.len(),
                "changed": changed.len(),
            },
            "added": added,
            "removed": removed,
            "changed": changed,
        })))
    }
}
