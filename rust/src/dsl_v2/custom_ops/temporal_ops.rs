//! Temporal Query Operations
//!
//! Point-in-time queries for regulatory lookback.
//! Answers: "What did the structure look like on date X?"
//!
//! Uses SQL functions from migrations/005_temporal_query_layer.sql

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use chrono::NaiveDate;
use serde_json::{json, Value};
use uuid::Uuid;

use super::helpers::{extract_string_opt, extract_uuid};
use super::CustomOperation;
use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};

#[cfg(feature = "database")]
use sqlx::PgPool;

// ============================================================================
// Helper: Parse date from args or default to today
// ============================================================================

fn get_date_arg(verb_call: &VerbCall, arg_name: &str) -> Result<NaiveDate> {
    if let Some(val) = extract_string_opt(verb_call, arg_name) {
        if val == "today" {
            return Ok(chrono::Utc::now().date_naive());
        }
        return NaiveDate::parse_from_str(&val, "%Y-%m-%d").map_err(|e| {
            anyhow!(
                "Invalid date format for {}: {} (expected YYYY-MM-DD)",
                arg_name,
                e
            )
        });
    }
    // Default to today
    Ok(chrono::Utc::now().date_naive())
}

fn get_optional_date_arg(verb_call: &VerbCall, arg_name: &str) -> Result<Option<NaiveDate>> {
    if let Some(val) = extract_string_opt(verb_call, arg_name) {
        let date = NaiveDate::parse_from_str(&val, "%Y-%m-%d")
            .map_err(|e| anyhow!("Invalid date format for {}: {}", arg_name, e))?;
        return Ok(Some(date));
    }
    Ok(None)
}

fn get_decimal_arg(verb_call: &VerbCall, arg_name: &str, default: f64) -> f64 {
    if let Some(val) = extract_string_opt(verb_call, arg_name) {
        return val.parse().unwrap_or(default);
    }
    default
}

fn get_int_arg(verb_call: &VerbCall, arg_name: &str, default: i32) -> i32 {
    if let Some(val) = extract_string_opt(verb_call, arg_name) {
        return val.parse().unwrap_or(default);
    }
    default
}

// ============================================================================
// temporal.ownership-as-of
// ============================================================================

pub struct TemporalOwnershipAsOfOp;

#[async_trait]
impl CustomOperation for TemporalOwnershipAsOfOp {
    fn domain(&self) -> &'static str {
        "temporal"
    }

    fn verb(&self) -> &'static str {
        "ownership-as-of"
    }

    fn rationale(&self) -> &'static str {
        "Point-in-time ownership query using SQL function with temporal filtering"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let entity_id = extract_uuid(verb_call, ctx, "entity-id")?;
        let as_of_date = get_date_arg(verb_call, "as-of-date")?;

        let rows: Vec<(
            Uuid,
            Uuid,
            String,
            Uuid,
            String,
            Option<sqlx::types::BigDecimal>,
            Option<String>,
            Option<NaiveDate>,
            Option<NaiveDate>,
        )> = sqlx::query_as(r#"SELECT * FROM "ob-poc".ownership_as_of($1, $2)"#)
            .bind(entity_id)
            .bind(as_of_date)
            .fetch_all(pool)
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

        Ok(ExecutionResult::Record(json!({
            "entity_id": entity_id.to_string(),
            "as_of_date": as_of_date.to_string(),
            "count": results.len(),
            "relationships": results,
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("Database feature required"))
    }
}

// ============================================================================
// temporal.ubo-chain-as-of
// ============================================================================

pub struct TemporalUboChainAsOfOp;

#[async_trait]
impl CustomOperation for TemporalUboChainAsOfOp {
    fn domain(&self) -> &'static str {
        "temporal"
    }

    fn verb(&self) -> &'static str {
        "ubo-chain-as-of"
    }

    fn rationale(&self) -> &'static str {
        "Point-in-time UBO chain tracing with recursive CTE and temporal filtering"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let entity_id = extract_uuid(verb_call, ctx, "entity-id")?;
        let as_of_date = get_date_arg(verb_call, "as-of-date")?;
        let threshold = get_decimal_arg(verb_call, "threshold", 25.0);

        let rows: Vec<(
            Vec<Uuid>,
            Vec<String>,
            Uuid,
            String,
            String,
            sqlx::types::BigDecimal,
            i32,
        )> = sqlx::query_as(r#"SELECT * FROM "ob-poc".ubo_chain_as_of($1, $2, $3)"#)
            .bind(entity_id)
            .bind(as_of_date)
            .bind(sqlx::types::BigDecimal::try_from(threshold).unwrap_or_default())
            .fetch_all(pool)
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

        Ok(ExecutionResult::Record(json!({
            "entity_id": entity_id.to_string(),
            "as_of_date": as_of_date.to_string(),
            "threshold": threshold,
            "count": chains.len(),
            "ubo_chains": chains,
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("Database feature required"))
    }
}

// ============================================================================
// temporal.cbu-relationships-as-of
// ============================================================================

pub struct TemporalCbuRelationshipsAsOfOp;

#[async_trait]
impl CustomOperation for TemporalCbuRelationshipsAsOfOp {
    fn domain(&self) -> &'static str {
        "temporal"
    }

    fn verb(&self) -> &'static str {
        "cbu-relationships-as-of"
    }

    fn rationale(&self) -> &'static str {
        "Point-in-time query of all CBU relationships for regulatory lookback"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let cbu_id = extract_uuid(verb_call, ctx, "cbu-id")?;
        let as_of_date = get_date_arg(verb_call, "as-of-date")?;

        let rows: Vec<(
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
        )> = sqlx::query_as(r#"SELECT * FROM "ob-poc".cbu_relationships_as_of($1, $2)"#)
            .bind(cbu_id)
            .bind(as_of_date)
            .fetch_all(pool)
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

        Ok(ExecutionResult::Record(json!({
            "cbu_id": cbu_id.to_string(),
            "as_of_date": as_of_date.to_string(),
            "count": results.len(),
            "relationships": results,
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("Database feature required"))
    }
}

// ============================================================================
// temporal.cbu-roles-as-of
// ============================================================================

pub struct TemporalCbuRolesAsOfOp;

#[async_trait]
impl CustomOperation for TemporalCbuRolesAsOfOp {
    fn domain(&self) -> &'static str {
        "temporal"
    }

    fn verb(&self) -> &'static str {
        "cbu-roles-as-of"
    }

    fn rationale(&self) -> &'static str {
        "Point-in-time query of CBU entity roles"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let cbu_id = extract_uuid(verb_call, ctx, "cbu-id")?;
        let as_of_date = get_date_arg(verb_call, "as-of-date")?;

        let rows: Vec<(
            Uuid,
            String,
            String,
            String,
            Option<NaiveDate>,
            Option<NaiveDate>,
        )> = sqlx::query_as(r#"SELECT * FROM "ob-poc".cbu_roles_as_of($1, $2)"#)
            .bind(cbu_id)
            .bind(as_of_date)
            .fetch_all(pool)
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

        Ok(ExecutionResult::Record(json!({
            "cbu_id": cbu_id.to_string(),
            "as_of_date": as_of_date.to_string(),
            "count": results.len(),
            "roles": results,
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("Database feature required"))
    }
}

// ============================================================================
// temporal.cbu-state-at-approval
// ============================================================================

pub struct TemporalCbuStateAtApprovalOp;

#[async_trait]
impl CustomOperation for TemporalCbuStateAtApprovalOp {
    fn domain(&self) -> &'static str {
        "temporal"
    }

    fn verb(&self) -> &'static str {
        "cbu-state-at-approval"
    }

    fn rationale(&self) -> &'static str {
        "Get CBU state at KYC case approval - answers 'what did we know when we approved?'"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let cbu_id = extract_uuid(verb_call, ctx, "cbu-id")?;

        let rows: Vec<(
            Uuid,
            chrono::DateTime<chrono::Utc>,
            Uuid,
            String,
            String,
            Option<Uuid>,
            Option<sqlx::types::BigDecimal>,
        )> = sqlx::query_as(r#"SELECT * FROM "ob-poc".cbu_state_at_approval($1)"#)
            .bind(cbu_id)
            .fetch_all(pool)
            .await?;

        if rows.is_empty() {
            return Ok(ExecutionResult::Record(json!({
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

        Ok(ExecutionResult::Record(json!({
            "cbu_id": cbu_id.to_string(),
            "case_id": case_id.to_string(),
            "approved_at": approved_at.to_rfc3339(),
            "count": entities.len(),
            "state_at_approval": entities,
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("Database feature required"))
    }
}

// ============================================================================
// temporal.relationship-history
// ============================================================================

pub struct TemporalRelationshipHistoryOp;

#[async_trait]
impl CustomOperation for TemporalRelationshipHistoryOp {
    fn domain(&self) -> &'static str {
        "temporal"
    }

    fn verb(&self) -> &'static str {
        "relationship-history"
    }

    fn rationale(&self) -> &'static str {
        "Query audit trail of changes to a specific relationship"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let relationship_id = extract_uuid(verb_call, ctx, "relationship-id")?;
        let limit = get_int_arg(verb_call, "limit", 50);

        let rows: Vec<(
            Uuid,
            String,
            chrono::DateTime<chrono::Utc>,
            Option<sqlx::types::BigDecimal>,
            Option<String>,
            Option<NaiveDate>,
            Option<NaiveDate>,
        )> = sqlx::query_as(
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
        .fetch_all(pool)
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

        Ok(ExecutionResult::Record(json!({
            "relationship_id": relationship_id.to_string(),
            "count": history.len(),
            "history": history,
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("Database feature required"))
    }
}

// ============================================================================
// temporal.entity-history
// ============================================================================

pub struct TemporalEntityHistoryOp;

#[async_trait]
impl CustomOperation for TemporalEntityHistoryOp {
    fn domain(&self) -> &'static str {
        "temporal"
    }

    fn verb(&self) -> &'static str {
        "entity-history"
    }

    fn rationale(&self) -> &'static str {
        "Query all relationship changes involving an entity"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let entity_id = extract_uuid(verb_call, ctx, "entity-id")?;
        let from_date = get_optional_date_arg(verb_call, "from-date")?;
        let to_date = get_optional_date_arg(verb_call, "to-date")?;
        let limit = get_int_arg(verb_call, "limit", 100);

        let rows: Vec<(
            Uuid,
            Uuid,
            String,
            Uuid,
            Uuid,
            String,
            chrono::DateTime<chrono::Utc>,
            Option<sqlx::types::BigDecimal>,
        )> = sqlx::query_as(
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
        .fetch_all(pool)
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

        Ok(ExecutionResult::Record(json!({
            "entity_id": entity_id.to_string(),
            "from_date": from_date.map(|d| d.to_string()),
            "to_date": to_date.map(|d| d.to_string()),
            "count": history.len(),
            "history": history,
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("Database feature required"))
    }
}

// ============================================================================
// temporal.compare-ownership
// ============================================================================

pub struct TemporalCompareOwnershipOp;

#[async_trait]
impl CustomOperation for TemporalCompareOwnershipOp {
    fn domain(&self) -> &'static str {
        "temporal"
    }

    fn verb(&self) -> &'static str {
        "compare-ownership"
    }

    fn rationale(&self) -> &'static str {
        "Compare ownership structure between two dates to identify changes"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let entity_id = extract_uuid(verb_call, ctx, "entity-id")?;

        let date_a = get_optional_date_arg(verb_call, "date-a")?
            .ok_or_else(|| anyhow!("date-a is required"))?;
        let date_b = get_optional_date_arg(verb_call, "date-b")?
            .ok_or_else(|| anyhow!("date-b is required"))?;

        // Get ownership at date A
        let rows_a: Vec<(Uuid, Uuid, Option<sqlx::types::BigDecimal>, Option<String>)> =
            sqlx::query_as(
                r#"
            SELECT relationship_id, from_entity_id, percentage, ownership_type
            FROM "ob-poc".ownership_as_of($1, $2)
            "#,
            )
            .bind(entity_id)
            .bind(date_a)
            .fetch_all(pool)
            .await?;

        // Get ownership at date B
        let rows_b: Vec<(Uuid, Uuid, Option<sqlx::types::BigDecimal>, Option<String>)> =
            sqlx::query_as(
                r#"
            SELECT relationship_id, from_entity_id, percentage, ownership_type
            FROM "ob-poc".ownership_as_of($1, $2)
            "#,
            )
            .bind(entity_id)
            .bind(date_b)
            .fetch_all(pool)
            .await?;

        // Build sets for comparison
        use std::collections::HashMap;
        let set_a: HashMap<Uuid, _> = rows_a.iter().map(|r| (r.0, r)).collect();
        let set_b: HashMap<Uuid, _> = rows_b.iter().map(|r| (r.0, r)).collect();

        let mut added = Vec::new();
        let mut removed = Vec::new();
        let mut changed = Vec::new();

        // Find added (in B but not in A)
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

        // Find removed (in A but not in B)
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

        // Find changed (in both but different)
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

        Ok(ExecutionResult::Record(json!({
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

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("Database feature required"))
    }
}
