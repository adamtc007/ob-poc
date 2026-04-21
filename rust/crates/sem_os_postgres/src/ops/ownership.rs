//! Ownership computation + reconciliation (8 plugin verbs) —
//! YAML-first re-implementation of `ownership.*` from
//! `rust/config/verbs/ownership.yaml`.
//!
//! Ops:
//! - `ownership.compute` — invokes `fn_derive_ownership_snapshots`
//! - `ownership.snapshot.list` — list snapshots with optional source /
//!   min-pct filters
//! - `ownership.list-control-positions` — dispatch
//!   `fn_holder_control_position` with basis
//! - `ownership.find-controller` — filter control-position rows to
//!   entities with control / significant influence / board rights
//! - `ownership.reconcile` — compare per-entity snapshots across two
//!   derivation sources, persist a run + findings, return run_id
//! - `ownership.reconcile.findings` — list findings for a run (severity +
//!   resolution-status filters)
//! - `ownership.analyze-gaps` — sum register-derived percentages, flag
//!   gap from 100%
//! - `ownership.trace-chain` — recursive CTE across
//!   `entity_relationships` ownership edges, return top-5 paths

use anyhow::Result;
use async_trait::async_trait;
use chrono::NaiveDate;
use serde_json::{json, Value};
use sqlx::Row;
use std::collections::HashMap;
use uuid::Uuid;

use dsl_runtime::domain_ops::helpers::{
    json_extract_int_opt, json_extract_string_opt, json_extract_uuid,
};
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::SemOsVerbOp;

fn ownership_date_arg(args: &Value, arg_name: &str) -> NaiveDate {
    json_extract_string_opt(args, arg_name)
        .as_deref()
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
        .unwrap_or_else(|| chrono::Utc::now().date_naive())
}

pub struct Compute;

#[async_trait]
impl SemOsVerbOp for Compute {
    fn fqn(&self) -> &str {
        "ownership.compute"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let issuer_entity_id = json_extract_uuid(args, ctx, "issuer-entity-id")?;
        let as_of = ownership_date_arg(args, "as-of");
        let count: i32 =
            sqlx::query_scalar(r#"SELECT "ob-poc".fn_derive_ownership_snapshots($1, $2)"#)
                .bind(issuer_entity_id)
                .bind(as_of)
                .fetch_one(scope.executor())
                .await?;
        Ok(VerbExecutionOutcome::Record(json!({
            "issuer_entity_id": issuer_entity_id,
            "as_of_date": as_of.to_string(),
            "snapshots_created": count
        })))
    }
}

pub struct SnapshotList;

#[async_trait]
impl SemOsVerbOp for SnapshotList {
    fn fqn(&self) -> &str {
        "ownership.snapshot.list"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let issuer_entity_id = json_extract_uuid(args, ctx, "issuer-entity-id")?;
        let as_of = ownership_date_arg(args, "as-of");
        let derived_from = json_extract_string_opt(args, "derived-from")
            .unwrap_or_else(|| "ALL".to_string());
        let min_pct: Option<rust_decimal::Decimal> =
            json_extract_string_opt(args, "min-pct").and_then(|s| s.parse().ok());
        let basis = json_extract_string_opt(args, "basis").unwrap_or_else(|| "VOTES".to_string());

        type Row14 = (
            Uuid, Uuid, Uuid, Option<Uuid>, NaiveDate, String,
            Option<rust_decimal::Decimal>, Option<rust_decimal::Decimal>,
            Option<rust_decimal::Decimal>, Option<rust_decimal::Decimal>,
            String, bool, bool, String,
        );
        let snapshots: Vec<Row14> = if derived_from == "ALL" {
            sqlx::query_as(
                r#"
                SELECT
                    os.snapshot_id, os.issuer_entity_id, os.owner_entity_id, os.share_class_id,
                    os.as_of_date, os.basis, os.units, os.percentage, os.percentage_min, os.percentage_max,
                    os.derived_from, os.is_direct, os.is_aggregated, os.confidence
                FROM "ob-poc".ownership_snapshots os
                WHERE os.issuer_entity_id = $1
                  AND os.as_of_date = $2
                  AND os.basis = $3
                  AND os.superseded_at IS NULL
                  AND ($4::numeric IS NULL OR os.percentage >= $4)
                ORDER BY os.percentage DESC NULLS LAST
                "#,
            )
            .bind(issuer_entity_id)
            .bind(as_of)
            .bind(&basis)
            .bind(min_pct)
            .fetch_all(scope.executor())
            .await?
        } else {
            sqlx::query_as(
                r#"
                SELECT
                    os.snapshot_id, os.issuer_entity_id, os.owner_entity_id, os.share_class_id,
                    os.as_of_date, os.basis, os.units, os.percentage, os.percentage_min, os.percentage_max,
                    os.derived_from, os.is_direct, os.is_aggregated, os.confidence
                FROM "ob-poc".ownership_snapshots os
                WHERE os.issuer_entity_id = $1
                  AND os.as_of_date = $2
                  AND os.basis = $3
                  AND os.derived_from = $4
                  AND os.superseded_at IS NULL
                  AND ($5::numeric IS NULL OR os.percentage >= $5)
                ORDER BY os.percentage DESC NULLS LAST
                "#,
            )
            .bind(issuer_entity_id)
            .bind(as_of)
            .bind(&basis)
            .bind(&derived_from)
            .bind(min_pct)
            .fetch_all(scope.executor())
            .await?
        };

        let mut out: Vec<Value> = Vec::with_capacity(snapshots.len());
        for s in &snapshots {
            let owner_name: Option<String> = sqlx::query_scalar(
                r#"SELECT name FROM "ob-poc".entities
                   WHERE entity_id = $1 AND deleted_at IS NULL"#,
            )
            .bind(s.2)
            .fetch_optional(scope.executor())
            .await?;
            out.push(json!({
                "snapshot_id": s.0,
                "owner_entity_id": s.2,
                "owner_name": owner_name,
                "share_class_id": s.3,
                "as_of_date": s.4.to_string(),
                "basis": s.5,
                "units": s.6.map(|d| d.to_string()),
                "percentage": s.7.map(|d| d.to_string()),
                "percentage_min": s.8.map(|d| d.to_string()),
                "percentage_max": s.9.map(|d| d.to_string()),
                "derived_from": s.10,
                "is_direct": s.11,
                "is_aggregated": s.12,
                "confidence": s.13
            }));
        }
        Ok(VerbExecutionOutcome::RecordSet(out))
    }
}

pub struct ListControlPositions;

#[async_trait]
impl SemOsVerbOp for ListControlPositions {
    fn fqn(&self) -> &str {
        "ownership.list-control-positions"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let issuer_entity_id = json_extract_uuid(args, ctx, "issuer-entity-id")?;
        let as_of = ownership_date_arg(args, "as-of");
        let basis = json_extract_string_opt(args, "basis").unwrap_or_else(|| "VOTES".to_string());
        let rows = sqlx::query(r#"SELECT * FROM "ob-poc".fn_holder_control_position($1, $2, $3)"#)
            .bind(issuer_entity_id)
            .bind(as_of)
            .bind(&basis)
            .fetch_all(scope.executor())
            .await?;
        let data: Vec<Value> = rows
            .iter()
            .map(|row| {
                json!({
                    "issuer_name": row.get::<String, _>("issuer_name"),
                    "holder_entity_id": row.get::<Uuid, _>("holder_entity_id"),
                    "holder_name": row.get::<String, _>("holder_name"),
                    "holder_type": row.get::<String, _>("holder_type"),
                    "units": row.get::<rust_decimal::Decimal, _>("holder_units").to_string(),
                    "votes": row.get::<rust_decimal::Decimal, _>("holder_votes").to_string(),
                    "economic": row.get::<rust_decimal::Decimal, _>("holder_economic").to_string(),
                    "total_issuer_votes": row.get::<rust_decimal::Decimal, _>("total_issuer_votes").to_string(),
                    "total_issuer_economic": row.get::<rust_decimal::Decimal, _>("total_issuer_economic").to_string(),
                    "voting_pct": row.get::<rust_decimal::Decimal, _>("voting_pct").to_string(),
                    "economic_pct": row.get::<rust_decimal::Decimal, _>("economic_pct").to_string(),
                    "control_threshold_pct": row.get::<rust_decimal::Decimal, _>("control_threshold_pct").to_string(),
                    "significant_threshold_pct": row.get::<rust_decimal::Decimal, _>("significant_threshold_pct").to_string(),
                    "has_control": row.get::<bool, _>("has_control"),
                    "has_significant_influence": row.get::<bool, _>("has_significant_influence"),
                    "has_board_rights": row.get::<bool, _>("has_board_rights"),
                    "board_seats": row.get::<i32, _>("board_seats")
                })
            })
            .collect();
        Ok(VerbExecutionOutcome::RecordSet(data))
    }
}

pub struct FindController;

#[async_trait]
impl SemOsVerbOp for FindController {
    fn fqn(&self) -> &str {
        "ownership.find-controller"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let issuer_entity_id = json_extract_uuid(args, ctx, "issuer-entity-id")?;
        let as_of = ownership_date_arg(args, "as-of");
        let rows = sqlx::query(r#"SELECT * FROM "ob-poc".fn_holder_control_position($1, $2, 'VOTES')"#)
            .bind(issuer_entity_id)
            .bind(as_of)
            .fetch_all(scope.executor())
            .await?;
        let controllers: Vec<Value> = rows
            .iter()
            .filter(|row| {
                let has_control: bool = row.get("has_control");
                let has_significant: bool = row.get("has_significant_influence");
                let has_board: bool = row.get("has_board_rights");
                has_control || has_significant || has_board
            })
            .map(|row| {
                let has_control: bool = row.get("has_control");
                let has_significant: bool = row.get("has_significant_influence");
                let has_board: bool = row.get("has_board_rights");
                let mut control_basis = Vec::new();
                if has_control {
                    control_basis.push("VOTING_MAJORITY");
                }
                if has_significant {
                    control_basis.push("SIGNIFICANT_INFLUENCE");
                }
                if has_board {
                    control_basis.push("BOARD_RIGHTS");
                }
                json!({
                    "holder_entity_id": row.get::<Uuid, _>("holder_entity_id"),
                    "holder_name": row.get::<String, _>("holder_name"),
                    "holder_type": row.get::<String, _>("holder_type"),
                    "voting_pct": row.get::<rust_decimal::Decimal, _>("voting_pct").to_string(),
                    "economic_pct": row.get::<rust_decimal::Decimal, _>("economic_pct").to_string(),
                    "has_control": has_control,
                    "has_significant_influence": has_significant,
                    "has_board_rights": has_board,
                    "board_seats": row.get::<i32, _>("board_seats"),
                    "control_basis": control_basis
                })
            })
            .collect();
        Ok(VerbExecutionOutcome::RecordSet(controllers))
    }
}

pub struct Reconcile;

#[async_trait]
impl SemOsVerbOp for Reconcile {
    fn fqn(&self) -> &str {
        "ownership.reconcile"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let issuer_entity_id = json_extract_uuid(args, ctx, "issuer-entity-id")?;
        let as_of = ownership_date_arg(args, "as-of");
        let source_a = json_extract_string_opt(args, "source-a").unwrap_or_else(|| "REGISTER".to_string());
        let source_b = json_extract_string_opt(args, "source-b").unwrap_or_else(|| "BODS".to_string());
        let basis = json_extract_string_opt(args, "basis").unwrap_or_else(|| "VOTES".to_string());
        let tolerance_bps: i32 = json_extract_int_opt(args, "tolerance-bps")
            .map(|i| i as i32)
            .unwrap_or(100);

        let run_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO "ob-poc".ownership_reconciliation_runs (
                issuer_entity_id, as_of_date, basis, source_a, source_b, tolerance_bps, status
            ) VALUES ($1, $2, $3, $4, $5, $6, 'RUNNING')
            RETURNING run_id
            "#,
        )
        .bind(issuer_entity_id)
        .bind(as_of)
        .bind(&basis)
        .bind(&source_a)
        .bind(&source_b)
        .bind(tolerance_bps)
        .fetch_one(scope.executor())
        .await?;

        let snapshots_a: Vec<(Uuid, rust_decimal::Decimal)> = sqlx::query_as(
            r#"
            SELECT owner_entity_id, COALESCE(percentage, 0)
            FROM "ob-poc".ownership_snapshots
            WHERE issuer_entity_id = $1
              AND as_of_date = $2
              AND derived_from = $3
              AND basis = $4
              AND superseded_at IS NULL
            "#,
        )
        .bind(issuer_entity_id)
        .bind(as_of)
        .bind(&source_a)
        .bind(&basis)
        .fetch_all(scope.executor())
        .await?;

        let snapshots_b: Vec<(Uuid, rust_decimal::Decimal)> = sqlx::query_as(
            r#"
            SELECT owner_entity_id, COALESCE(percentage, (COALESCE(percentage_min, 0) + COALESCE(percentage_max, 0)) / 2)
            FROM "ob-poc".ownership_snapshots
            WHERE issuer_entity_id = $1
              AND as_of_date = $2
              AND derived_from = $3
              AND basis = $4
              AND superseded_at IS NULL
            "#,
        )
        .bind(issuer_entity_id)
        .bind(as_of)
        .bind(&source_b)
        .bind(&basis)
        .fetch_all(scope.executor())
        .await?;

        let map_a: HashMap<Uuid, rust_decimal::Decimal> = snapshots_a.into_iter().collect();
        let map_b: HashMap<Uuid, rust_decimal::Decimal> = snapshots_b.into_iter().collect();
        let mut matched = 0;
        let mut mismatched = 0;
        let mut missing_in_a = 0;
        let mut missing_in_b = 0;

        for (entity_id, pct_a) in &map_a {
            if let Some(pct_b) = map_b.get(entity_id) {
                let delta_bps = ((pct_a - pct_b).abs() * rust_decimal::Decimal::from(10000))
                    .to_string()
                    .parse::<i32>()
                    .unwrap_or(0);
                let (finding_type, severity) = if delta_bps <= tolerance_bps {
                    matched += 1;
                    ("MATCH", "INFO")
                } else {
                    mismatched += 1;
                    let sev = if delta_bps > 500 { "ERROR" } else { "WARN" };
                    ("MISMATCH", sev)
                };
                sqlx::query(
                    r#"
                    INSERT INTO "ob-poc".ownership_reconciliation_findings (
                        run_id, owner_entity_id, source_a_pct, source_b_pct, delta_bps,
                        finding_type, severity
                    ) VALUES ($1, $2, $3, $4, $5, $6, $7)
                    "#,
                )
                .bind(run_id)
                .bind(entity_id)
                .bind(pct_a)
                .bind(pct_b)
                .bind(delta_bps)
                .bind(finding_type)
                .bind(severity)
                .execute(scope.executor())
                .await?;
            } else {
                missing_in_b += 1;
                sqlx::query(
                    r#"
                    INSERT INTO "ob-poc".ownership_reconciliation_findings (
                        run_id, owner_entity_id, source_a_pct, finding_type, severity
                    ) VALUES ($1, $2, $3, 'MISSING_IN_EXTERNAL', 'WARN')
                    "#,
                )
                .bind(run_id)
                .bind(entity_id)
                .bind(pct_a)
                .execute(scope.executor())
                .await?;
            }
        }
        for (entity_id, pct_b) in &map_b {
            if !map_a.contains_key(entity_id) {
                missing_in_a += 1;
                sqlx::query(
                    r#"
                    INSERT INTO "ob-poc".ownership_reconciliation_findings (
                        run_id, owner_entity_id, source_b_pct, finding_type, severity
                    ) VALUES ($1, $2, $3, 'MISSING_IN_REGISTER', 'ERROR')
                    "#,
                )
                .bind(run_id)
                .bind(entity_id)
                .bind(pct_b)
                .execute(scope.executor())
                .await?;
            }
        }

        let total_entities = (map_a.len() + map_b.len()) as i32;
        sqlx::query(
            r#"
            UPDATE "ob-poc".ownership_reconciliation_runs
            SET status = 'COMPLETED',
                completed_at = now(),
                total_entities = $2,
                matched_count = $3,
                mismatched_count = $4,
                missing_in_a_count = $5,
                missing_in_b_count = $6
            WHERE run_id = $1
            "#,
        )
        .bind(run_id)
        .bind(total_entities)
        .bind(matched)
        .bind(mismatched)
        .bind(missing_in_a)
        .bind(missing_in_b)
        .execute(scope.executor())
        .await?;

        ctx.bind("ownership_reconciliation_run", run_id);
        Ok(VerbExecutionOutcome::Uuid(run_id))
    }
}

pub struct ReconcileFindings;

#[async_trait]
impl SemOsVerbOp for ReconcileFindings {
    fn fqn(&self) -> &str {
        "ownership.reconcile.findings"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let run_id = json_extract_uuid(args, ctx, "run-id")?;
        let severity = json_extract_string_opt(args, "severity").unwrap_or_else(|| "ALL".to_string());
        let status = json_extract_string_opt(args, "status").unwrap_or_else(|| "OPEN".to_string());

        type Row11 = (
            Uuid, Uuid, Uuid,
            Option<rust_decimal::Decimal>, Option<rust_decimal::Decimal>,
            Option<i32>, String, Option<String>, String,
            Option<String>, Option<String>,
        );
        let findings: Vec<Row11> = if severity == "ALL" && status == "ALL" {
            sqlx::query_as(
                r#"
                SELECT f.finding_id, f.run_id, f.owner_entity_id,
                       f.source_a_pct, f.source_b_pct, f.delta_bps,
                       f.finding_type, f.severity, f.resolution_status,
                       f.resolution_notes, f.resolved_by
                FROM "ob-poc".ownership_reconciliation_findings f
                WHERE f.run_id = $1
                ORDER BY f.delta_bps DESC NULLS LAST
                "#,
            )
            .bind(run_id)
            .fetch_all(scope.executor())
            .await?
        } else if severity == "ALL" {
            sqlx::query_as(
                r#"
                SELECT f.finding_id, f.run_id, f.owner_entity_id,
                       f.source_a_pct, f.source_b_pct, f.delta_bps,
                       f.finding_type, f.severity, f.resolution_status,
                       f.resolution_notes, f.resolved_by
                FROM "ob-poc".ownership_reconciliation_findings f
                WHERE f.run_id = $1 AND f.resolution_status = $2
                ORDER BY f.delta_bps DESC NULLS LAST
                "#,
            )
            .bind(run_id)
            .bind(&status)
            .fetch_all(scope.executor())
            .await?
        } else if status == "ALL" {
            sqlx::query_as(
                r#"
                SELECT f.finding_id, f.run_id, f.owner_entity_id,
                       f.source_a_pct, f.source_b_pct, f.delta_bps,
                       f.finding_type, f.severity, f.resolution_status,
                       f.resolution_notes, f.resolved_by
                FROM "ob-poc".ownership_reconciliation_findings f
                WHERE f.run_id = $1 AND f.severity = $2
                ORDER BY f.delta_bps DESC NULLS LAST
                "#,
            )
            .bind(run_id)
            .bind(&severity)
            .fetch_all(scope.executor())
            .await?
        } else {
            sqlx::query_as(
                r#"
                SELECT f.finding_id, f.run_id, f.owner_entity_id,
                       f.source_a_pct, f.source_b_pct, f.delta_bps,
                       f.finding_type, f.severity, f.resolution_status,
                       f.resolution_notes, f.resolved_by
                FROM "ob-poc".ownership_reconciliation_findings f
                WHERE f.run_id = $1 AND f.severity = $2 AND f.resolution_status = $3
                ORDER BY f.delta_bps DESC NULLS LAST
                "#,
            )
            .bind(run_id)
            .bind(&severity)
            .bind(&status)
            .fetch_all(scope.executor())
            .await?
        };

        let mut out: Vec<Value> = Vec::with_capacity(findings.len());
        for f in &findings {
            let owner_name: Option<String> = sqlx::query_scalar(
                r#"SELECT name FROM "ob-poc".entities
                   WHERE entity_id = $1 AND deleted_at IS NULL"#,
            )
            .bind(f.2)
            .fetch_optional(scope.executor())
            .await?;
            out.push(json!({
                "finding_id": f.0,
                "run_id": f.1,
                "owner_entity_id": f.2,
                "owner_name": owner_name,
                "source_a_pct": f.3.map(|d| d.to_string()),
                "source_b_pct": f.4.map(|d| d.to_string()),
                "delta_bps": f.5,
                "finding_type": f.6,
                "severity": f.7,
                "resolution_status": f.8,
                "resolution_notes": f.9,
                "resolved_by": f.10
            }));
        }
        Ok(VerbExecutionOutcome::RecordSet(out))
    }
}

pub struct AnalyzeGaps;

#[async_trait]
impl SemOsVerbOp for AnalyzeGaps {
    fn fqn(&self) -> &str {
        "ownership.analyze-gaps"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let issuer_entity_id = json_extract_uuid(args, ctx, "issuer-entity-id")?;
        let as_of = ownership_date_arg(args, "as-of");
        let total_pct: rust_decimal::Decimal = sqlx::query_scalar(
            r#"
            SELECT COALESCE(SUM(percentage), 0)
            FROM "ob-poc".ownership_snapshots
            WHERE issuer_entity_id = $1
              AND as_of_date = $2
              AND derived_from = 'REGISTER'
              AND basis = 'VOTES'
              AND superseded_at IS NULL
            "#,
        )
        .bind(issuer_entity_id)
        .bind(as_of)
        .fetch_one(scope.executor())
        .await?;
        let gap_pct = rust_decimal::Decimal::from(100) - total_pct;
        let is_reconciled = gap_pct.abs() < rust_decimal::Decimal::new(1, 2);
        Ok(VerbExecutionOutcome::Record(json!({
            "issuer_entity_id": issuer_entity_id,
            "as_of_date": as_of.to_string(),
            "total_ownership_pct": total_pct.to_string(),
            "gap_pct": gap_pct.to_string(),
            "is_reconciled": is_reconciled
        })))
    }
}

pub struct TraceChain;

#[async_trait]
impl SemOsVerbOp for TraceChain {
    fn fqn(&self) -> &str {
        "ownership.trace-chain"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let from_entity_id = json_extract_uuid(args, ctx, "from-entity-id")?;
        let to_entity_id = json_extract_uuid(args, ctx, "to-entity-id")?;
        let max_depth: i32 = json_extract_int_opt(args, "max-depth")
            .map(|i| i as i32)
            .unwrap_or(10);
        let paths: Vec<(String, rust_decimal::Decimal, i32)> = sqlx::query_as(
            r#"
            WITH RECURSIVE ownership_chain AS (
                SELECT
                    er.from_entity_id,
                    er.to_entity_id,
                    ARRAY[er.from_entity_id, er.to_entity_id]::uuid[] AS path,
                    COALESCE(er.percentage, 0)::numeric AS cumulative_pct,
                    1 AS depth
                FROM "ob-poc".entity_relationships er
                WHERE er.from_entity_id = $1
                  AND er.relationship_type = 'ownership'
                  AND (er.effective_to IS NULL OR er.effective_to > CURRENT_DATE)
                UNION ALL
                SELECT
                    oc.from_entity_id,
                    er.to_entity_id,
                    oc.path || er.to_entity_id,
                    (oc.cumulative_pct * COALESCE(er.percentage, 0) / 100)::numeric,
                    oc.depth + 1
                FROM ownership_chain oc
                JOIN "ob-poc".entity_relationships er ON er.from_entity_id = oc.to_entity_id
                WHERE er.relationship_type = 'ownership'
                  AND (er.effective_to IS NULL OR er.effective_to > CURRENT_DATE)
                  AND oc.depth < $3
                  AND NOT (er.to_entity_id = ANY(oc.path))
            )
            SELECT
                array_to_string(path, ' -> '),
                cumulative_pct,
                depth
            FROM ownership_chain
            WHERE to_entity_id = $2
            ORDER BY cumulative_pct DESC
            LIMIT 5
            "#,
        )
        .bind(from_entity_id)
        .bind(to_entity_id)
        .bind(max_depth)
        .fetch_all(scope.executor())
        .await?;
        let path_exists = !paths.is_empty();
        let best_path = paths.first();
        Ok(VerbExecutionOutcome::Record(json!({
            "from_entity_id": from_entity_id,
            "to_entity_id": to_entity_id,
            "path_exists": path_exists,
            "path": best_path.map(|(p, _, _)| p.clone()),
            "cumulative_pct": best_path.map(|(_, pct, _)| pct.to_string()),
            "depth": best_path.map(|(_, _, d)| d),
            "all_paths": paths.iter().map(|(p, pct, d)| json!({
                "path": p,
                "cumulative_pct": pct.to_string(),
                "depth": d
            })).collect::<Vec<_>>()
        })))
    }
}
