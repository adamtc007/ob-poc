//! Ownership Computation & Reconciliation Operations
//!
//! Plugin handlers for ownership snapshots, control positions, and reconciliation
//! against external sources (BODS, GLEIF).
//!
//! ## Key Tables
//! - kyc.ownership_snapshots
//! - kyc.special_rights
//! - kyc.ownership_reconciliation_runs
//! - kyc.ownership_reconciliation_findings
//!
//! ## Key SQL Functions
//! - kyc.fn_holder_control_position()
//! - kyc.fn_derive_ownership_snapshots()

use anyhow::Result;
use async_trait::async_trait;
use chrono::NaiveDate;
use serde_json::json;
use uuid::Uuid;

#[cfg(feature = "database")]
use sqlx::PgPool;

use super::helpers::{extract_int_opt, extract_string_opt, extract_uuid};
use super::{CustomOperation, ExecutionContext, ExecutionResult, VerbCall};

// ============================================================================
// Ownership Computation
// ============================================================================

/// Derive ownership snapshots from register holdings
pub struct OwnershipComputeOp;

#[async_trait]
impl CustomOperation for OwnershipComputeOp {
    fn domain(&self) -> &'static str {
        "ownership"
    }

    fn verb(&self) -> &'static str {
        "compute"
    }

    fn rationale(&self) -> &'static str {
        "Ownership computation aggregates holdings and calls SQL function for snapshot creation"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let issuer_entity_id = extract_uuid(verb_call, ctx, "issuer-entity-id")?;
        let as_of: NaiveDate = verb_call
            .get_arg("as-of")
            .and_then(|a| a.value.as_string())
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
            .unwrap_or_else(|| chrono::Utc::now().date_naive());
        let _basis = extract_string_opt(verb_call, "basis").unwrap_or_else(|| "VOTES".to_string());

        // Call the derivation function
        let count: i32 = sqlx::query_scalar(r#"SELECT kyc.fn_derive_ownership_snapshots($1, $2)"#)
            .bind(issuer_entity_id)
            .bind(as_of)
            .fetch_one(pool)
            .await?;

        tracing::info!(
            "ownership.compute: derived {} snapshots for issuer {} as-of {}",
            count,
            issuer_entity_id,
            as_of
        );

        Ok(ExecutionResult::Record(json!({
            "issuer_entity_id": issuer_entity_id,
            "as_of_date": as_of.to_string(),
            "snapshots_created": count
        })))
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

/// List ownership snapshots for an issuer
pub struct OwnershipSnapshotListOp;

#[async_trait]
impl CustomOperation for OwnershipSnapshotListOp {
    fn domain(&self) -> &'static str {
        "ownership"
    }

    fn verb(&self) -> &'static str {
        "snapshot.list"
    }

    fn rationale(&self) -> &'static str {
        "Snapshot listing with filtering by source and minimum percentage"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let issuer_entity_id = extract_uuid(verb_call, ctx, "issuer-entity-id")?;
        let as_of: NaiveDate = verb_call
            .get_arg("as-of")
            .and_then(|a| a.value.as_string())
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
            .unwrap_or_else(|| chrono::Utc::now().date_naive());
        let derived_from =
            extract_string_opt(verb_call, "derived-from").unwrap_or_else(|| "ALL".to_string());
        let min_pct: Option<rust_decimal::Decimal> = verb_call
            .get_arg("min-pct")
            .and_then(|a| a.value.as_string())
            .and_then(|s| s.parse().ok());
        let basis = extract_string_opt(verb_call, "basis").unwrap_or_else(|| "VOTES".to_string());

        let snapshots: Vec<(
            Uuid,
            Uuid,
            Uuid,
            Option<Uuid>,
            NaiveDate,
            String,
            Option<rust_decimal::Decimal>,
            Option<rust_decimal::Decimal>,
            Option<rust_decimal::Decimal>,
            Option<rust_decimal::Decimal>,
            String,
            bool,
            bool,
            String,
        )> = if derived_from == "ALL" {
            sqlx::query_as(
                r#"
                SELECT
                    os.snapshot_id, os.issuer_entity_id, os.owner_entity_id, os.share_class_id,
                    os.as_of_date, os.basis, os.units, os.percentage, os.percentage_min, os.percentage_max,
                    os.derived_from, os.is_direct, os.is_aggregated, os.confidence
                FROM kyc.ownership_snapshots os
                WHERE os.issuer_entity_id = $1
                  AND os.as_of_date = $2
                  AND os.basis = $3
                  AND os.superseded_at IS NULL
                  AND ($4::numeric IS NULL OR os.percentage >= $4)
                ORDER BY os.percentage DESC NULLS LAST
                "#
            )
            .bind(issuer_entity_id)
            .bind(as_of)
            .bind(&basis)
            .bind(min_pct)
            .fetch_all(pool)
            .await?
        } else {
            sqlx::query_as(
                r#"
                SELECT
                    os.snapshot_id, os.issuer_entity_id, os.owner_entity_id, os.share_class_id,
                    os.as_of_date, os.basis, os.units, os.percentage, os.percentage_min, os.percentage_max,
                    os.derived_from, os.is_direct, os.is_aggregated, os.confidence
                FROM kyc.ownership_snapshots os
                WHERE os.issuer_entity_id = $1
                  AND os.as_of_date = $2
                  AND os.basis = $3
                  AND os.derived_from = $4
                  AND os.superseded_at IS NULL
                  AND ($5::numeric IS NULL OR os.percentage >= $5)
                ORDER BY os.percentage DESC NULLS LAST
                "#
            )
            .bind(issuer_entity_id)
            .bind(as_of)
            .bind(&basis)
            .bind(&derived_from)
            .bind(min_pct)
            .fetch_all(pool)
            .await?
        };

        // Get owner names
        let snapshot_data: Vec<serde_json::Value> =
            futures::future::try_join_all(snapshots.iter().map(|s| async {
                let owner_name: Option<String> = sqlx::query_scalar(
                    r#"SELECT name FROM "ob-poc".entities WHERE entity_id = $1"#,
                )
                .bind(s.2)
                .fetch_optional(pool)
                .await?;

                Ok::<_, anyhow::Error>(json!({
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
                }))
            }))
            .await?;

        Ok(ExecutionResult::RecordSet(snapshot_data))
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

/// Get control positions for an issuer
pub struct OwnershipControlPositionsOp;

#[async_trait]
impl CustomOperation for OwnershipControlPositionsOp {
    fn domain(&self) -> &'static str {
        "ownership"
    }

    fn verb(&self) -> &'static str {
        "control-positions"
    }

    fn rationale(&self) -> &'static str {
        "Control position computation uses SQL function with basis parameter"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let issuer_entity_id = extract_uuid(verb_call, ctx, "issuer-entity-id")?;
        let as_of: NaiveDate = verb_call
            .get_arg("as-of")
            .and_then(|a| a.value.as_string())
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
            .unwrap_or_else(|| chrono::Utc::now().date_naive());
        let basis = extract_string_opt(verb_call, "basis").unwrap_or_else(|| "VOTES".to_string());

        // Use query() with manual extraction since SQLx FromRow only supports tuples up to ~16 elements
        use sqlx::Row;
        let position_rows =
            sqlx::query(r#"SELECT * FROM kyc.fn_holder_control_position($1, $2, $3)"#)
                .bind(issuer_entity_id)
                .bind(as_of)
                .bind(&basis)
                .fetch_all(pool)
                .await?;

        let position_data: Vec<serde_json::Value> = position_rows
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

        Ok(ExecutionResult::RecordSet(position_data))
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

/// Find who controls an issuer
pub struct OwnershipWhoControlsOp;

#[async_trait]
impl CustomOperation for OwnershipWhoControlsOp {
    fn domain(&self) -> &'static str {
        "ownership"
    }

    fn verb(&self) -> &'static str {
        "who-controls"
    }

    fn rationale(&self) -> &'static str {
        "Who controls query filters to entities with control or board majority"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let issuer_entity_id = extract_uuid(verb_call, ctx, "issuer-entity-id")?;
        let as_of: NaiveDate = verb_call
            .get_arg("as-of")
            .and_then(|a| a.value.as_string())
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
            .unwrap_or_else(|| chrono::Utc::now().date_naive());

        // Use query() with manual extraction since SQLx FromRow only supports tuples up to ~16 elements
        use sqlx::Row;
        let position_rows =
            sqlx::query(r#"SELECT * FROM kyc.fn_holder_control_position($1, $2, 'VOTES')"#)
                .bind(issuer_entity_id)
                .bind(as_of)
                .fetch_all(pool)
                .await?;

        // Filter to those with control, significant influence, or board rights
        let controllers: Vec<serde_json::Value> = position_rows
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

        Ok(ExecutionResult::RecordSet(controllers))
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

// ============================================================================
// Reconciliation
// ============================================================================

/// Compare ownership from different sources
pub struct OwnershipReconcileOp;

#[async_trait]
impl CustomOperation for OwnershipReconcileOp {
    fn domain(&self) -> &'static str {
        "ownership"
    }

    fn verb(&self) -> &'static str {
        "reconcile"
    }

    fn rationale(&self) -> &'static str {
        "Reconciliation compares snapshots from two sources and creates findings"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let issuer_entity_id = extract_uuid(verb_call, ctx, "issuer-entity-id")?;
        let as_of: NaiveDate = verb_call
            .get_arg("as-of")
            .and_then(|a| a.value.as_string())
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
            .unwrap_or_else(|| chrono::Utc::now().date_naive());
        let source_a =
            extract_string_opt(verb_call, "source-a").unwrap_or_else(|| "REGISTER".to_string());
        let source_b =
            extract_string_opt(verb_call, "source-b").unwrap_or_else(|| "BODS".to_string());
        let basis = extract_string_opt(verb_call, "basis").unwrap_or_else(|| "VOTES".to_string());
        let tolerance_bps: i32 = extract_int_opt(verb_call, "tolerance-bps")
            .map(|i| i as i32)
            .unwrap_or(100);

        let mut tx = pool.begin().await?;

        // Create reconciliation run
        let run_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO kyc.ownership_reconciliation_runs (
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
        .fetch_one(&mut *tx)
        .await?;

        // Get snapshots from source A
        let snapshots_a: Vec<(Uuid, rust_decimal::Decimal)> = sqlx::query_as(
            r#"
            SELECT owner_entity_id, COALESCE(percentage, 0)
            FROM kyc.ownership_snapshots
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
        .fetch_all(&mut *tx)
        .await?;

        // Get snapshots from source B
        let snapshots_b: Vec<(Uuid, rust_decimal::Decimal)> = sqlx::query_as(
            r#"
            SELECT owner_entity_id, COALESCE(percentage, (COALESCE(percentage_min, 0) + COALESCE(percentage_max, 0)) / 2)
            FROM kyc.ownership_snapshots
            WHERE issuer_entity_id = $1
              AND as_of_date = $2
              AND derived_from = $3
              AND basis = $4
              AND superseded_at IS NULL
            "#
        )
        .bind(issuer_entity_id)
        .bind(as_of)
        .bind(&source_b)
        .bind(&basis)
        .fetch_all(&mut *tx)
        .await?;

        // Build lookup maps
        use std::collections::HashMap;
        let map_a: HashMap<Uuid, rust_decimal::Decimal> = snapshots_a.into_iter().collect();
        let map_b: HashMap<Uuid, rust_decimal::Decimal> = snapshots_b.into_iter().collect();

        let mut matched = 0;
        let mut mismatched = 0;
        let mut missing_in_a = 0;
        let mut missing_in_b = 0;

        // Compare A against B
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
                    INSERT INTO kyc.ownership_reconciliation_findings (
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
                .execute(&mut *tx)
                .await?;
            } else {
                missing_in_b += 1;
                sqlx::query(
                    r#"
                    INSERT INTO kyc.ownership_reconciliation_findings (
                        run_id, owner_entity_id, source_a_pct, finding_type, severity
                    ) VALUES ($1, $2, $3, 'MISSING_IN_EXTERNAL', 'WARN')
                    "#,
                )
                .bind(run_id)
                .bind(entity_id)
                .bind(pct_a)
                .execute(&mut *tx)
                .await?;
            }
        }

        // Check for entities in B but not A
        for (entity_id, pct_b) in &map_b {
            if !map_a.contains_key(entity_id) {
                missing_in_a += 1;
                sqlx::query(
                    r#"
                    INSERT INTO kyc.ownership_reconciliation_findings (
                        run_id, owner_entity_id, source_b_pct, finding_type, severity
                    ) VALUES ($1, $2, $3, 'MISSING_IN_REGISTER', 'ERROR')
                    "#,
                )
                .bind(run_id)
                .bind(entity_id)
                .bind(pct_b)
                .execute(&mut *tx)
                .await?;
            }
        }

        // Update run status
        let total_entities = (map_a.len() + map_b.len()) as i32;
        sqlx::query(
            r#"
            UPDATE kyc.ownership_reconciliation_runs
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
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        if let Some(ref binding) = verb_call.binding {
            ctx.bind(binding, run_id);
        }

        tracing::info!(
            "ownership.reconcile: {} vs {} for {}: matched={}, mismatched={}, missing_a={}, missing_b={}",
            source_a, source_b, issuer_entity_id, matched, mismatched, missing_in_a, missing_in_b
        );

        Ok(ExecutionResult::Uuid(run_id))
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

/// List findings from a reconciliation run
pub struct OwnershipReconcileFindingsOp;

#[async_trait]
impl CustomOperation for OwnershipReconcileFindingsOp {
    fn domain(&self) -> &'static str {
        "ownership"
    }

    fn verb(&self) -> &'static str {
        "reconcile.findings"
    }

    fn rationale(&self) -> &'static str {
        "Findings listing with filtering by severity and resolution status"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let run_id = extract_uuid(verb_call, ctx, "run-id")?;
        let severity =
            extract_string_opt(verb_call, "severity").unwrap_or_else(|| "ALL".to_string());
        let status = extract_string_opt(verb_call, "status").unwrap_or_else(|| "OPEN".to_string());

        let findings: Vec<(
            Uuid,
            Uuid,
            Uuid,
            Option<rust_decimal::Decimal>,
            Option<rust_decimal::Decimal>,
            Option<i32>,
            String,
            Option<String>,
            String,
            Option<String>,
            Option<String>,
        )> = if severity == "ALL" && status == "ALL" {
            sqlx::query_as(
                r#"
                SELECT f.finding_id, f.run_id, f.owner_entity_id,
                       f.source_a_pct, f.source_b_pct, f.delta_bps,
                       f.finding_type, f.severity, f.resolution_status,
                       f.resolution_notes, f.resolved_by
                FROM kyc.ownership_reconciliation_findings f
                WHERE f.run_id = $1
                ORDER BY f.delta_bps DESC NULLS LAST
                "#,
            )
            .bind(run_id)
            .fetch_all(pool)
            .await?
        } else if severity == "ALL" {
            sqlx::query_as(
                r#"
                SELECT f.finding_id, f.run_id, f.owner_entity_id,
                       f.source_a_pct, f.source_b_pct, f.delta_bps,
                       f.finding_type, f.severity, f.resolution_status,
                       f.resolution_notes, f.resolved_by
                FROM kyc.ownership_reconciliation_findings f
                WHERE f.run_id = $1 AND f.resolution_status = $2
                ORDER BY f.delta_bps DESC NULLS LAST
                "#,
            )
            .bind(run_id)
            .bind(&status)
            .fetch_all(pool)
            .await?
        } else if status == "ALL" {
            sqlx::query_as(
                r#"
                SELECT f.finding_id, f.run_id, f.owner_entity_id,
                       f.source_a_pct, f.source_b_pct, f.delta_bps,
                       f.finding_type, f.severity, f.resolution_status,
                       f.resolution_notes, f.resolved_by
                FROM kyc.ownership_reconciliation_findings f
                WHERE f.run_id = $1 AND f.severity = $2
                ORDER BY f.delta_bps DESC NULLS LAST
                "#,
            )
            .bind(run_id)
            .bind(&severity)
            .fetch_all(pool)
            .await?
        } else {
            sqlx::query_as(
                r#"
                SELECT f.finding_id, f.run_id, f.owner_entity_id,
                       f.source_a_pct, f.source_b_pct, f.delta_bps,
                       f.finding_type, f.severity, f.resolution_status,
                       f.resolution_notes, f.resolved_by
                FROM kyc.ownership_reconciliation_findings f
                WHERE f.run_id = $1 AND f.severity = $2 AND f.resolution_status = $3
                ORDER BY f.delta_bps DESC NULLS LAST
                "#,
            )
            .bind(run_id)
            .bind(&severity)
            .bind(&status)
            .fetch_all(pool)
            .await?
        };

        // Get owner names
        let finding_data: Vec<serde_json::Value> =
            futures::future::try_join_all(findings.iter().map(|f| async {
                let owner_name: Option<String> = sqlx::query_scalar(
                    r#"SELECT name FROM "ob-poc".entities WHERE entity_id = $1"#,
                )
                .bind(f.2)
                .fetch_optional(pool)
                .await?;

                Ok::<_, anyhow::Error>(json!({
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
                }))
            }))
            .await?;

        Ok(ExecutionResult::RecordSet(finding_data))
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

// ============================================================================
// Ownership Analysis
// ============================================================================

/// Analyze ownership gaps for an issuer
pub struct OwnershipAnalyzeGapsOp;

#[async_trait]
impl CustomOperation for OwnershipAnalyzeGapsOp {
    fn domain(&self) -> &'static str {
        "ownership"
    }

    fn verb(&self) -> &'static str {
        "analyze-gaps"
    }

    fn rationale(&self) -> &'static str {
        "Gap analysis checks if ownership sums to 100% and identifies unallocated shares"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let issuer_entity_id = extract_uuid(verb_call, ctx, "issuer-entity-id")?;
        let as_of: NaiveDate = verb_call
            .get_arg("as-of")
            .and_then(|a| a.value.as_string())
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
            .unwrap_or_else(|| chrono::Utc::now().date_naive());

        // Get total ownership from snapshots
        let total_pct: rust_decimal::Decimal = sqlx::query_scalar(
            r#"
            SELECT COALESCE(SUM(percentage), 0)
            FROM kyc.ownership_snapshots
            WHERE issuer_entity_id = $1
              AND as_of_date = $2
              AND derived_from = 'REGISTER'
              AND basis = 'VOTES'
              AND superseded_at IS NULL
            "#,
        )
        .bind(issuer_entity_id)
        .bind(as_of)
        .fetch_one(pool)
        .await?;

        let gap_pct = rust_decimal::Decimal::from(100) - total_pct;
        let is_reconciled = gap_pct.abs() < rust_decimal::Decimal::new(1, 2); // 0.01%

        Ok(ExecutionResult::Record(json!({
            "issuer_entity_id": issuer_entity_id,
            "as_of_date": as_of.to_string(),
            "total_ownership_pct": total_pct.to_string(),
            "gap_pct": gap_pct.to_string(),
            "is_reconciled": is_reconciled
        })))
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

/// Trace ownership chain from one entity to another
pub struct OwnershipTraceChainOp;

#[async_trait]
impl CustomOperation for OwnershipTraceChainOp {
    fn domain(&self) -> &'static str {
        "ownership"
    }

    fn verb(&self) -> &'static str {
        "trace-chain"
    }

    fn rationale(&self) -> &'static str {
        "Chain tracing uses recursive CTE to find paths and compute cumulative ownership"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let from_entity_id = extract_uuid(verb_call, ctx, "from-entity-id")?;
        let to_entity_id = extract_uuid(verb_call, ctx, "to-entity-id")?;
        let max_depth: i32 = extract_int_opt(verb_call, "max-depth")
            .map(|i| i as i32)
            .unwrap_or(10);

        // Use recursive CTE to find path
        let paths: Vec<(String, rust_decimal::Decimal, i32)> = sqlx::query_as(
            r#"
            WITH RECURSIVE ownership_chain AS (
                -- Base case: direct ownership from from_entity to target
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

                -- Recursive case
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
        .fetch_all(pool)
        .await?;

        let path_exists = !paths.is_empty();
        let best_path = paths.first();

        Ok(ExecutionResult::Record(json!({
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

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Void)
    }
}

// ============================================================================
// Registration function for mod.rs
// ============================================================================

pub fn register_ownership_ops(registry: &mut super::CustomOperationRegistry) {
    use std::sync::Arc;

    registry.register(Arc::new(OwnershipComputeOp));
    registry.register(Arc::new(OwnershipSnapshotListOp));
    registry.register(Arc::new(OwnershipControlPositionsOp));
    registry.register(Arc::new(OwnershipWhoControlsOp));
    registry.register(Arc::new(OwnershipReconcileOp));
    registry.register(Arc::new(OwnershipReconcileFindingsOp));
    registry.register(Arc::new(OwnershipAnalyzeGapsOp));
    registry.register(Arc::new(OwnershipTraceChainOp));
}
