//! Capital structure verbs (8 plugin verbs) — YAML-first re-implementation of
//! `capital.*` from `rust/config/verbs/capital.yaml`.
//!
//! Ops:
//! - `capital.transfer` — atomic transfer of units between holdings + paired
//!   transfer_out / transfer_in movement records
//! - `capital.reconcile` — verify SUM(holdings) = issued_shares, aggregate
//!   voting + economic percentages per shareholder
//! - `capital.get-ownership-chain` — recursive CTE over
//!   `entity_relationships` with multiplicative cumulative percentages
//! - `capital.issue.initial` — first issuance event + supply row for a share
//!   class (rejects if a prior EFFECTIVE event exists)
//! - `capital.issue.new` — subsequent issuance with running supply rollup
//! - `capital.split` — SERIALIZABLE stock split with advisory lock + idempotency
//!   key, adjusting supply, holdings, and dilution instruments atomically
//! - `capital.buyback` — move units from outstanding to treasury
//! - `capital.cancel` — permanent issued-supply reduction
//!
//! `capital.share-class.create` (`ShareClassCreate`), `capital.share-class.get-supply`
//! (`ShareClassGetSupply`), and `capital.issue-shares` (`IssueShares`) were
//! deleted 2026-08-19 (EOP-PLAN capital/share-class consolidation): all three
//! referenced schema that never existed live (`share_classes.authorized_shares`,
//! `share_classes.issued_shares`, `fn_share_class_supply_at()`) and would have
//! errored at runtime; none were reachable anyway (no Domain Pack ever owned
//! `capital.`). The create step is now `share-class.create`
//! (`config/verbs/registry/share-class.yaml`, CRUD, extended with
//! issuer-entity-id/instrument-kind/votes-per-unit/economic-per-unit); the
//! issue step is `capital.issue.initial`/`capital.issue.new` below, the only
//! two members of the original family confirmed to write real columns.
//!
//! `capital.cancel-shares` (`CancelShares`), `capital.cap-table` (`CapTable`),
//! and `capital.holders` (`Holders`) were deleted 2026-08-19 (EOP-PLAN
//! share-register board design pass): `cancel-shares` was broken
//! (nonexistent `share_classes.issued_shares`) and a straight duplicate of
//! `capital.cancel`, which already does the same job correctly.
//! `cap-table`/`holders` were broken (`fn_holder_control_position()` never
//! existed) **and**, fixing them properly would mean re-deriving
//! voting/economic control from company-share data in parallel to the
//! `ob-poc-kyc-substrate` determination engine (`FundControlStrategy`/
//! `ControlProngStrategy`/`EdgeKind`) — the ratified control_interest
//! register. A share register's job is bookkeeping (`reconcile` — does our
//! own ledger add up), not a second way to answer "who controls this
//! entity."

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use chrono::NaiveDate;
use serde_json::{json, Value};
use sqlx::Row;
use std::collections::HashMap;
use uuid::Uuid;

use dsl_runtime::TransactionScope;
use dsl_runtime::{
    self, json_extract_int, json_extract_string, json_extract_string_opt, json_extract_uuid,
    json_get_required_uuid,
};
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::SemOsVerbOp;

// ============================================================================
// Row Structs (replacing anonymous tuples for FromRow)
// ============================================================================

#[derive(Debug)]
struct ShareholderInfo {
    entity_id: Uuid,
    total_units: rust_decimal::Decimal,
    total_voting_rights: rust_decimal::Decimal,
    share_classes: Vec<Value>,
}

// ============================================================================
// capital.transfer
// ============================================================================

pub struct Transfer;

#[async_trait]
impl SemOsVerbOp for Transfer {
    fn fqn(&self) -> &str {
        "capital.transfer"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let share_class_id = json_get_required_uuid(args, "share-class-id")?;
        let from_entity_id = json_get_required_uuid(args, "from-entity-id")?;
        let to_entity_id = json_get_required_uuid(args, "to-entity-id")?;
        let units: rust_decimal::Decimal = json_extract_string(args, "units")?
            .parse()
            .map_err(|_| anyhow!("units must be a decimal number"))?;
        let transfer_date = json_extract_string(args, "transfer-date")?;
        let reference = json_extract_string(args, "reference")?;
        let price_per_unit: Option<rust_decimal::Decimal> =
            json_extract_string_opt(args, "price-per-unit").and_then(|s| s.parse().ok());

        let source_holding: Option<(Uuid, rust_decimal::Decimal)> = sqlx::query_as(
            r#"
            SELECT id, units FROM "ob-poc".holdings
            WHERE share_class_id = $1 AND investor_entity_id = $2 AND status = 'active'
            "#,
        )
        .bind(share_class_id)
        .bind(from_entity_id)
        .fetch_optional(scope.executor())
        .await?;

        let (source_holding_id, source_units) =
            source_holding.ok_or_else(|| anyhow!("Source holding not found or inactive"))?;

        if source_units < units {
            return Err(anyhow!(
                "Insufficient units: have {}, trying to transfer {}",
                source_units,
                units
            ));
        }

        let target_holding: Option<(Uuid, rust_decimal::Decimal)> = sqlx::query_as(
            r#"
            SELECT id, units FROM "ob-poc".holdings
            WHERE share_class_id = $1 AND investor_entity_id = $2 AND status = 'active'
            "#,
        )
        .bind(share_class_id)
        .bind(to_entity_id)
        .fetch_optional(scope.executor())
        .await?;

        let target_holding_id = if let Some((id, _)) = target_holding {
            id
        } else {
            let new_id: (Uuid,) = sqlx::query_as(
                r#"
                INSERT INTO "ob-poc".holdings (share_class_id, investor_entity_id, units, status)
                VALUES ($1, $2, 0, 'active')
                RETURNING id
                "#,
            )
            .bind(share_class_id)
            .bind(to_entity_id)
            .fetch_one(scope.executor())
            .await?;
            new_id.0
        };

        sqlx::query(
            r#"UPDATE "ob-poc".holdings SET units = units - $1, updated_at = now() WHERE id = $2"#,
        )
        .bind(units)
        .bind(source_holding_id)
        .execute(scope.executor())
        .await?;

        sqlx::query(
            r#"UPDATE "ob-poc".holdings SET units = units + $1, updated_at = now() WHERE id = $2"#,
        )
        .bind(units)
        .bind(target_holding_id)
        .execute(scope.executor())
        .await?;

        let amount = price_per_unit.map(|p| p * units);
        let transfer_out_id: (Uuid,) = sqlx::query_as(
            r#"
            INSERT INTO "ob-poc".movements (
                holding_id, movement_type, units, price_per_unit, amount,
                currency, trade_date, status, reference
            )
            VALUES ($1, 'transfer_out', $2, $3, $4, 'USD', $5::date, 'settled', $6)
            RETURNING id
            "#,
        )
        .bind(source_holding_id)
        .bind(units)
        .bind(price_per_unit)
        .bind(amount)
        .bind(&transfer_date)
        .bind(&reference)
        .fetch_one(scope.executor())
        .await?;

        let _transfer_in_id: (Uuid,) = sqlx::query_as(
            r#"
            INSERT INTO "ob-poc".movements (
                holding_id, movement_type, units, price_per_unit, amount,
                currency, trade_date, status, reference
            )
            VALUES ($1, 'transfer_in', $2, $3, $4, 'USD', $5::date, 'settled', $6)
            RETURNING id
            "#,
        )
        .bind(target_holding_id)
        .bind(units)
        .bind(price_per_unit)
        .bind(amount)
        .bind(&transfer_date)
        .bind(&reference)
        .fetch_one(scope.executor())
        .await?;

        dsl_runtime::emit_pending_state_advance_batch(
            ctx,
            &[
                dsl_runtime::StateTransitionInput {
                    entity_id: from_entity_id,
                    to_node: "capital:transferred_out",
                    slot_path: "capital/holdings",
                    reason: "capital.transfer (source)",
                },
                dsl_runtime::StateTransitionInput {
                    entity_id: to_entity_id,
                    to_node: "capital:transferred_in",
                    slot_path: "capital/holdings",
                    reason: "capital.transfer (target)",
                },
            ],
        );

        Ok(VerbExecutionOutcome::Uuid(transfer_out_id.0))
    }
}

// ============================================================================
// capital.reconcile
// ============================================================================

pub struct Reconcile;

#[async_trait]
impl SemOsVerbOp for Reconcile {
    fn fqn(&self) -> &str {
        "capital.reconcile"
    }

    async fn execute(
        &self,
        args: &Value,
        _ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let entity_id = json_get_required_uuid(args, "entity-id")?;

        // Real columns (fixed 2026-08-19, EOP-PLAN share-register board
        // design pass): `share_classes.issued_shares`/`.voting_rights_per_share`
        // never existed live. Issued supply lives in `share_class_supply`
        // (latest row per class, LATERAL join); voting weight lives in
        // `share_classes.votes_per_unit` (nullable -- defaults to 1, same
        // default `capital.share-class.create` used before it was
        // deleted). `lifecycle_status <> 'LIQUIDATED'` replaces the
        // deprecated `status` column as the "still a going concern" filter
        // -- no verb writes `status` anymore after this pass.
        let share_classes: Vec<(Uuid, String, rust_decimal::Decimal, rust_decimal::Decimal)> =
            sqlx::query_as(
                r#"
                SELECT sc.id, sc.name,
                       COALESCE(scs.issued_units, 0),
                       COALESCE(sc.votes_per_unit, 1)
                FROM "ob-poc".share_classes sc
                LEFT JOIN LATERAL (
                    SELECT issued_units FROM "ob-poc".share_class_supply
                    WHERE share_class_id = sc.id
                    ORDER BY as_of_date DESC
                    LIMIT 1
                ) scs ON true
                WHERE sc.issuer_entity_id = $1 AND sc.lifecycle_status <> 'LIQUIDATED'
                "#,
            )
            .bind(entity_id)
            .fetch_all(scope.executor())
            .await?;

        let mut total_issued: rust_decimal::Decimal = rust_decimal::Decimal::ZERO;
        let mut total_allocated: rust_decimal::Decimal = rust_decimal::Decimal::ZERO;
        let mut shareholders_map: HashMap<Uuid, ShareholderInfo> = HashMap::new();

        for (class_id, class_name, issued, voting_per_share) in &share_classes {
            total_issued += issued;

            // usage_type != 'TA': this reconciles the share register's own
            // cap-table holdings, not TA/investor-linked holdings (those
            // are KYC-domain-owned, reconciled via their own investor
            // register lifecycle).
            let holdings: Vec<(Uuid, rust_decimal::Decimal)> = sqlx::query_as(
                r#"
                SELECT investor_entity_id, units
                FROM "ob-poc".holdings
                WHERE share_class_id = $1 AND status = 'active' AND units > 0
                  AND usage_type IS DISTINCT FROM 'TA'
                "#,
            )
            .bind(class_id)
            .fetch_all(scope.executor())
            .await?;

            for (investor_id, units) in holdings {
                total_allocated += units;
                let voting_rights = units * voting_per_share;

                let info = shareholders_map
                    .entry(investor_id)
                    .or_insert(ShareholderInfo {
                        entity_id: investor_id,
                        total_units: rust_decimal::Decimal::ZERO,
                        total_voting_rights: rust_decimal::Decimal::ZERO,
                        share_classes: vec![],
                    });
                info.total_units += units;
                info.total_voting_rights += voting_rights;
                info.share_classes.push(json!({
                    "class_id": class_id,
                    "class_name": class_name,
                    "units": units.to_string(),
                    "voting_rights": voting_rights.to_string()
                }));
            }
        }

        let total_issued_dec = total_issued;
        let total_voting: rust_decimal::Decimal = share_classes
            .iter()
            .map(|(_, _, issued, voting)| issued * voting)
            .sum();

        let shareholders: Vec<Value> = shareholders_map
            .values()
            .map(|info| {
                let ownership_pct = if total_issued_dec > rust_decimal::Decimal::ZERO {
                    (info.total_units / total_issued_dec * rust_decimal::Decimal::from(100))
                        .round_dp(4)
                } else {
                    rust_decimal::Decimal::ZERO
                };
                let voting_pct = if total_voting > rust_decimal::Decimal::ZERO {
                    (info.total_voting_rights / total_voting * rust_decimal::Decimal::from(100))
                        .round_dp(4)
                } else {
                    rust_decimal::Decimal::ZERO
                };

                json!({
                    "entity_id": info.entity_id,
                    "total_units": info.total_units.to_string(),
                    "ownership_pct": ownership_pct.to_string(),
                    "voting_pct": voting_pct.to_string(),
                    "share_classes": info.share_classes
                })
            })
            .collect();

        let unallocated = total_issued_dec - total_allocated;
        let is_reconciled = unallocated == rust_decimal::Decimal::ZERO;

        Ok(VerbExecutionOutcome::Record(json!({
            "is_reconciled": is_reconciled,
            "issued_shares": total_issued.to_string(),
            "allocated_shares": total_allocated.to_string(),
            "unallocated_shares": unallocated.to_string(),
            "shareholders": shareholders
        })))
    }
}

// ============================================================================
// capital.get-ownership-chain
// ============================================================================

pub struct GetOwnershipChain;

#[async_trait]
impl SemOsVerbOp for GetOwnershipChain {
    fn fqn(&self) -> &str {
        "capital.get-ownership-chain"
    }

    async fn execute(
        &self,
        args: &Value,
        _ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let entity_id = json_get_required_uuid(args, "entity-id")?;
        let min_pct: rust_decimal::Decimal = json_extract_string_opt(args, "min-ownership-pct")
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(|| rust_decimal::Decimal::new(1, 2));

        let chains: Vec<(Uuid, String, String, rust_decimal::Decimal, i32)> = sqlx::query_as(
            r#"
            WITH RECURSIVE ownership_chain AS (
                SELECT
                    er.from_entity_id as owner_id,
                    e.name as owner_name,
                    ARRAY[er.from_entity_id]::uuid[] as path,
                    COALESCE(er.percentage, 0)::numeric as cumulative_pct,
                    1 as depth
                FROM "ob-poc".entity_relationships er
                JOIN "ob-poc".entities e ON er.from_entity_id = e.entity_id
                WHERE er.to_entity_id = $1
                  AND e.deleted_at IS NULL
                  AND er.relationship_type = 'ownership'
                  AND (er.effective_to IS NULL OR er.effective_to > CURRENT_DATE)
                  AND COALESCE(er.percentage, 0) >= $2

                UNION ALL

                SELECT
                    er.from_entity_id,
                    e.name,
                    oc.path || er.from_entity_id,
                    (oc.cumulative_pct * COALESCE(er.percentage, 0) / 100)::numeric,
                    oc.depth + 1
                FROM ownership_chain oc
                JOIN "ob-poc".entity_relationships er ON er.to_entity_id = oc.owner_id
                JOIN "ob-poc".entities e ON er.from_entity_id = e.entity_id
                WHERE er.relationship_type = 'ownership'
                  AND e.deleted_at IS NULL
                  AND (er.effective_to IS NULL OR er.effective_to > CURRENT_DATE)
                  AND oc.depth < 10
                  AND NOT (er.from_entity_id = ANY(oc.path))
                  AND (oc.cumulative_pct * COALESCE(er.percentage, 0) / 100) >= $2
            )
            SELECT owner_id, owner_name, array_to_string(path, '->'), cumulative_pct, depth
            FROM ownership_chain
            ORDER BY cumulative_pct DESC, depth
            "#,
        )
        .bind(entity_id)
        .bind(min_pct)
        .fetch_all(scope.executor())
        .await?;

        let chain_data: Vec<Value> = chains
            .iter()
            .map(|(owner_id, owner_name, path, pct, depth)| {
                json!({
                    "owner_entity_id": owner_id,
                    "owner_name": owner_name,
                    "path": path,
                    "cumulative_ownership_pct": pct.to_string(),
                    "chain_depth": depth
                })
            })
            .collect();

        Ok(VerbExecutionOutcome::RecordSet(chain_data))
    }
}


// ============================================================================
// capital.issue.initial
// ============================================================================

pub struct IssueInitial;

#[async_trait]
impl SemOsVerbOp for IssueInitial {
    fn fqn(&self) -> &str {
        "capital.issue.initial"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let share_class_id = json_extract_uuid(args, ctx, "share-class-id")?;
        let units: rust_decimal::Decimal = json_extract_string(args, "units")?
            .parse()
            .map_err(|_| anyhow!("units must be a decimal number"))?;
        let price_per_unit: Option<rust_decimal::Decimal> =
            json_extract_string_opt(args, "price-per-unit").and_then(|s| s.parse().ok());
        let effective_date: NaiveDate = json_extract_string_opt(args, "effective-date")
            .and_then(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok())
            .unwrap_or_else(|| chrono::Utc::now().date_naive());
        let board_resolution_ref = json_extract_string_opt(args, "board-resolution-ref");

        let issuer_entity_id: Uuid = sqlx::query_scalar(
            r#"SELECT issuer_entity_id FROM "ob-poc".share_classes WHERE id = $1"#,
        )
        .bind(share_class_id)
        .fetch_optional(scope.executor())
        .await?
        .ok_or_else(|| anyhow!("Share class {} not found", share_class_id))?;

        let prior_exists: bool = sqlx::query_scalar(
            r#"SELECT EXISTS(SELECT 1 FROM "ob-poc".issuance_events WHERE share_class_id = $1 AND status = 'EFFECTIVE')"#,
        )
        .bind(share_class_id)
        .fetch_one(scope.executor())
        .await?;

        if prior_exists {
            return Err(anyhow!(
                "Share class {} already has issuance events. Use capital.issue.new for subsequent issues.",
                share_class_id
            ));
        }

        let event_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO "ob-poc".issuance_events (
                share_class_id, issuer_entity_id, event_type, units_delta,
                price_per_unit, effective_date, board_resolution_ref, status
            ) VALUES ($1, $2, 'INITIAL_ISSUE', $3, $4, $5, $6, 'EFFECTIVE')
            RETURNING event_id
            "#,
        )
        .bind(share_class_id)
        .bind(issuer_entity_id)
        .bind(units)
        .bind(price_per_unit)
        .bind(effective_date)
        .bind(&board_resolution_ref)
        .fetch_one(scope.executor())
        .await?;

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".share_class_supply (
                share_class_id, issued_units, outstanding_units, as_of_date, as_of_event_id
            ) VALUES ($1, $2, $2, $3, $4)
            ON CONFLICT (share_class_id, as_of_date) DO UPDATE SET
                issued_units = $2,
                outstanding_units = $2,
                as_of_event_id = $4,
                updated_at = now()
            "#,
        )
        .bind(share_class_id)
        .bind(units)
        .bind(effective_date)
        .bind(event_id)
        .execute(scope.executor())
        .await?;

        dsl_runtime::emit_pending_state_advance(
            ctx,
            issuer_entity_id,
            "capital:issue_initial",
            "capital/issuance",
            "capital.issue.initial",
        );

        Ok(VerbExecutionOutcome::Uuid(event_id))
    }
}

// ============================================================================
// capital.issue.new
// ============================================================================

pub struct IssueNew;

#[async_trait]
impl SemOsVerbOp for IssueNew {
    fn fqn(&self) -> &str {
        "capital.issue.new"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let share_class_id = json_extract_uuid(args, ctx, "share-class-id")?;
        let units: rust_decimal::Decimal = json_extract_string(args, "units")?
            .parse()
            .map_err(|_| anyhow!("units must be a decimal number"))?;
        let price_per_unit: rust_decimal::Decimal = json_extract_string(args, "price-per-unit")?
            .parse()
            .map_err(|_| anyhow!("price-per-unit must be a decimal number"))?;
        let effective_date: NaiveDate = json_extract_string_opt(args, "effective-date")
            .and_then(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok())
            .unwrap_or_else(|| chrono::Utc::now().date_naive());
        let board_resolution_ref = json_extract_string_opt(args, "board-resolution-ref");

        let share_info: Option<(Uuid, Option<rust_decimal::Decimal>)> = sqlx::query_as(
            r#"
            SELECT sc.issuer_entity_id, scs.issued_units
            FROM "ob-poc".share_classes sc
            LEFT JOIN "ob-poc".share_class_supply scs ON scs.share_class_id = sc.id
            WHERE sc.id = $1
            ORDER BY scs.as_of_date DESC NULLS LAST
            LIMIT 1
            "#,
        )
        .bind(share_class_id)
        .fetch_optional(scope.executor())
        .await?;

        let (issuer_entity_id, current_issued) =
            share_info.ok_or_else(|| anyhow!("Share class {} not found", share_class_id))?;

        let new_issued = current_issued.unwrap_or(rust_decimal::Decimal::ZERO) + units;

        let event_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO "ob-poc".issuance_events (
                share_class_id, issuer_entity_id, event_type, units_delta,
                price_per_unit, effective_date, board_resolution_ref, status
            ) VALUES ($1, $2, 'NEW_ISSUE', $3, $4, $5, $6, 'EFFECTIVE')
            RETURNING event_id
            "#,
        )
        .bind(share_class_id)
        .bind(issuer_entity_id)
        .bind(units)
        .bind(price_per_unit)
        .bind(effective_date)
        .bind(&board_resolution_ref)
        .fetch_one(scope.executor())
        .await?;

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".share_class_supply (
                share_class_id, issued_units, outstanding_units, as_of_date, as_of_event_id
            ) VALUES ($1, $2, $2, $3, $4)
            ON CONFLICT (share_class_id, as_of_date) DO UPDATE SET
                issued_units = $2,
                outstanding_units = $2,
                as_of_event_id = $4,
                updated_at = now()
            "#,
        )
        .bind(share_class_id)
        .bind(new_issued)
        .bind(effective_date)
        .bind(event_id)
        .execute(scope.executor())
        .await?;

        dsl_runtime::emit_pending_state_advance(
            ctx,
            issuer_entity_id,
            "capital:issue_new",
            "capital/issuance",
            "capital.issue.new",
        );

        Ok(VerbExecutionOutcome::Uuid(event_id))
    }
}

// ============================================================================
// capital.split
// ============================================================================

/// Stock split with full transactional safety.
///
/// Safety features:
/// 1. SERIALIZABLE isolation — prevents concurrent reads seeing stale data
/// 2. Advisory lock on share_class_id — serializes all splits on same class
/// 3. Idempotency key — prevents duplicate application on retry
/// 4. Single transaction — all-or-nothing for holdings, supply, and dilution instruments
pub struct Split;

#[async_trait]
impl SemOsVerbOp for Split {
    fn fqn(&self) -> &str {
        "capital.split"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let share_class_id = json_extract_uuid(args, ctx, "share-class-id")?;
        let ratio_from: i32 = json_extract_int(args, "ratio-from")? as i32;
        let ratio_to: i32 = json_extract_int(args, "ratio-to")? as i32;
        let effective_date: NaiveDate = json_extract_string_opt(args, "effective-date")
            .and_then(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok())
            .unwrap_or_else(|| chrono::Utc::now().date_naive());
        let record_date: Option<NaiveDate> = json_extract_string_opt(args, "record-date")
            .and_then(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok());

        if ratio_from <= 0 || ratio_to <= 0 {
            return Err(anyhow!("ratio-from and ratio-to must be positive"));
        }

        let idempotency_key = format!(
            "split:{}:{}:{}:{}",
            share_class_id, ratio_from, ratio_to, effective_date
        );

        // Fixed 2026-08-19 (EOP-PLAN share-register board design pass):
        // Postgres requires SET TRANSACTION ISOLATION LEVEL to be the first
        // statement of the transaction -- it was previously issued after
        // the idempotency-check and issuer-lookup SELECTs below, so every
        // call (not just idempotent retries) errored with "SET TRANSACTION
        // ISOLATION LEVEL must be called before any query". Must run first.
        sqlx::query("SET TRANSACTION ISOLATION LEVEL SERIALIZABLE")
            .execute(scope.executor())
            .await?;

        let existing: Option<Uuid> = sqlx::query_scalar(
            r#"SELECT event_id FROM "ob-poc".issuance_events WHERE idempotency_key = $1"#,
        )
        .bind(&idempotency_key)
        .fetch_optional(scope.executor())
        .await?;

        if let Some(event_id) = existing {
            return Ok(VerbExecutionOutcome::Uuid(event_id));
        }

        let issuer_entity_id: Uuid = sqlx::query_scalar(
            r#"SELECT issuer_entity_id FROM "ob-poc".share_classes WHERE id = $1"#,
        )
        .bind(share_class_id)
        .fetch_optional(scope.executor())
        .await?
        .ok_or_else(|| anyhow!("Share class {} not found", share_class_id))?;

        // Fixed 2026-08-19 (EOP-PLAN share-register board design pass):
        // "ob-poc".uuid_to_lock_id() never existed live -- this call
        // would have errored every time. pg_advisory_xact_lock takes a
        // plain bigint; XOR-fold the UUID's two 64-bit halves in Rust
        // instead of calling out to a nonexistent SQL function.
        let uuid_bits = share_class_id.as_u128();
        let lock_id = (uuid_bits as i64) ^ ((uuid_bits >> 64) as i64);
        sqlx::query("SELECT pg_advisory_xact_lock($1)")
            .bind(lock_id)
            .execute(scope.executor())
            .await?;

        let current_issued: Option<rust_decimal::Decimal> = sqlx::query_scalar(
            r#"
            SELECT issued_units FROM "ob-poc".share_class_supply
            WHERE share_class_id = $1
            ORDER BY as_of_date DESC LIMIT 1
            "#,
        )
        .bind(share_class_id)
        .fetch_optional(scope.executor())
        .await?;

        let issued = current_issued
            .filter(|&units| units != rust_decimal::Decimal::ZERO)
            .ok_or_else(|| anyhow!("Cannot split share class with no issued units"))?;

        let multiplier =
            rust_decimal::Decimal::from(ratio_to) / rust_decimal::Decimal::from(ratio_from);

        let new_issued = issued * multiplier;
        let units_delta = new_issued - issued;

        let event_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO "ob-poc".issuance_events (
                share_class_id, issuer_entity_id, event_type, units_delta,
                ratio_from, ratio_to, effective_date, record_date, status,
                idempotency_key
            ) VALUES ($1, $2, 'STOCK_SPLIT', $3, $4, $5, $6, $7, 'EFFECTIVE', $8)
            RETURNING event_id
            "#,
        )
        .bind(share_class_id)
        .bind(issuer_entity_id)
        .bind(units_delta)
        .bind(ratio_from)
        .bind(ratio_to)
        .bind(effective_date)
        .bind(record_date)
        .bind(&idempotency_key)
        .fetch_one(scope.executor())
        .await?;

        sqlx::query(
            r#"
            UPDATE "ob-poc".share_class_supply
            SET issued_units = issued_units * $2,
                outstanding_units = outstanding_units * $2,
                treasury_units = COALESCE(treasury_units, 0) * $2,
                reserved_units = COALESCE(reserved_units, 0) * $2,
                as_of_event_id = $3,
                updated_at = now()
            WHERE share_class_id = $1
              AND as_of_date = (SELECT MAX(as_of_date) FROM "ob-poc".share_class_supply WHERE share_class_id = $1)
            "#,
        )
        .bind(share_class_id)
        .bind(multiplier)
        .bind(event_id)
        .execute(scope.executor())
        .await?;

        sqlx::query(
            r#"
            UPDATE "ob-poc".holdings
            SET units = units * $2,
                cost_basis = CASE WHEN cost_basis IS NOT NULL THEN cost_basis / $2 ELSE NULL END,
                updated_at = now()
            WHERE share_class_id = $1 AND status = 'active'
            "#,
        )
        .bind(share_class_id)
        .bind(multiplier)
        .execute(scope.executor())
        .await?;

        sqlx::query(
            r#"
            UPDATE "ob-poc".dilution_instruments
            SET conversion_ratio = conversion_ratio * $2,
                exercise_price = CASE WHEN exercise_price IS NOT NULL THEN exercise_price / $2 ELSE NULL END,
                updated_at = now()
            WHERE converts_to_share_class_id = $1 AND status = 'ACTIVE'
            "#,
        )
        .bind(share_class_id)
        .bind(multiplier)
        .execute(scope.executor())
        .await?;

        dsl_runtime::emit_pending_state_advance(
            ctx,
            issuer_entity_id,
            "capital:split_executed",
            "capital/issuance",
            "capital.split",
        );

        Ok(VerbExecutionOutcome::Uuid(event_id))
    }
}

// ============================================================================
// capital.buyback
// ============================================================================

pub struct Buyback;

#[async_trait]
impl SemOsVerbOp for Buyback {
    fn fqn(&self) -> &str {
        "capital.buyback"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let share_class_id = json_extract_uuid(args, ctx, "share-class-id")?;
        let units: rust_decimal::Decimal = json_extract_string(args, "units")?
            .parse()
            .map_err(|_| anyhow!("units must be a decimal number"))?;
        let price_per_unit: rust_decimal::Decimal = json_extract_string(args, "price-per-unit")?
            .parse()
            .map_err(|_| anyhow!("price-per-unit must be a decimal number"))?;
        let effective_date: NaiveDate = json_extract_string_opt(args, "effective-date")
            .and_then(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok())
            .unwrap_or_else(|| chrono::Utc::now().date_naive());

        let issuer_entity_id: Uuid = sqlx::query_scalar(
            r#"SELECT issuer_entity_id FROM "ob-poc".share_classes WHERE id = $1"#,
        )
        .bind(share_class_id)
        .fetch_optional(scope.executor())
        .await?
        .ok_or_else(|| anyhow!("Share class {} not found", share_class_id))?;

        let total_amount = units * price_per_unit;

        let event_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO "ob-poc".issuance_events (
                share_class_id, issuer_entity_id, event_type, units_delta,
                price_per_unit, total_amount, effective_date, status
            ) VALUES ($1, $2, 'BUYBACK', $3, $4, $5, $6, 'EFFECTIVE')
            RETURNING event_id
            "#,
        )
        .bind(share_class_id)
        .bind(issuer_entity_id)
        .bind(units)
        .bind(price_per_unit)
        .bind(total_amount)
        .bind(effective_date)
        .fetch_one(scope.executor())
        .await?;

        sqlx::query(
            r#"
            UPDATE "ob-poc".share_class_supply
            SET outstanding_units = outstanding_units - $2,
                treasury_units = treasury_units + $2,
                as_of_event_id = $3,
                updated_at = now()
            WHERE share_class_id = $1
              AND as_of_date = (SELECT MAX(as_of_date) FROM "ob-poc".share_class_supply WHERE share_class_id = $1)
            "#,
        )
        .bind(share_class_id)
        .bind(units)
        .bind(event_id)
        .execute(scope.executor())
        .await?;

        dsl_runtime::emit_pending_state_advance(
            ctx,
            issuer_entity_id,
            "capital:treasury_acquired",
            "capital/issuance",
            "capital.buyback",
        );

        Ok(VerbExecutionOutcome::Uuid(event_id))
    }
}

// ============================================================================
// capital.cancel
// ============================================================================

pub struct Cancel;

#[async_trait]
impl SemOsVerbOp for Cancel {
    fn fqn(&self) -> &str {
        "capital.cancel"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let share_class_id = json_extract_uuid(args, ctx, "share-class-id")?;
        let units: rust_decimal::Decimal = json_extract_string(args, "units")?
            .parse()
            .map_err(|_| anyhow!("units must be a decimal number"))?;
        let effective_date: NaiveDate = json_extract_string_opt(args, "effective-date")
            .and_then(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok())
            .unwrap_or_else(|| chrono::Utc::now().date_naive());
        let reason = json_extract_string_opt(args, "reason");

        let issuer_entity_id: Uuid = sqlx::query_scalar(
            r#"SELECT issuer_entity_id FROM "ob-poc".share_classes WHERE id = $1"#,
        )
        .bind(share_class_id)
        .fetch_optional(scope.executor())
        .await?
        .ok_or_else(|| anyhow!("Share class {} not found", share_class_id))?;

        let event_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO "ob-poc".issuance_events (
                share_class_id, issuer_entity_id, event_type, units_delta,
                effective_date, notes, status
            ) VALUES ($1, $2, 'CANCELLATION', $3, $4, $5, 'EFFECTIVE')
            RETURNING event_id
            "#,
        )
        .bind(share_class_id)
        .bind(issuer_entity_id)
        .bind(units)
        .bind(effective_date)
        .bind(&reason)
        .fetch_one(scope.executor())
        .await?;

        sqlx::query(
            r#"
            UPDATE "ob-poc".share_class_supply
            SET issued_units = issued_units - $2,
                outstanding_units = outstanding_units - $2,
                as_of_event_id = $3,
                updated_at = now()
            WHERE share_class_id = $1
              AND as_of_date = (SELECT MAX(as_of_date) FROM "ob-poc".share_class_supply WHERE share_class_id = $1)
            "#,
        )
        .bind(share_class_id)
        .bind(units)
        .bind(event_id)
        .execute(scope.executor())
        .await?;

        dsl_runtime::emit_pending_state_advance(
            ctx,
            issuer_entity_id,
            "capital:issued_cancelled",
            "capital/issuance",
            "capital.cancel",
        );

        Ok(VerbExecutionOutcome::Uuid(event_id))
    }
}

// ============================================================================
// capital.close-holding
// ============================================================================

/// Close a fully-divested holding (EOP-PLAN share-register board design
/// pass, Phase 4). Nothing previously closed a holding once its balance
/// was transferred away or otherwise drained to zero -- it sat 'active'
/// forever, a ghost row for `reconcile`/`list-shareholders`. Guarded to
/// require units = 0; this is the exit move for the new `holding` slot
/// (`share_register_dag.yaml`, scoped to `usage_type != 'TA'` -- the
/// share register's own cap-table holdings, not the KYC-domain
/// investor-register axis on `holding_status`).
pub struct CloseHolding;

#[async_trait]
impl SemOsVerbOp for CloseHolding {
    fn fqn(&self) -> &str {
        "capital.close-holding"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let share_class_id = json_extract_uuid(args, ctx, "share-class-id")?;
        let shareholder_entity_id = json_extract_uuid(args, ctx, "shareholder-entity-id")?;

        let row = sqlx::query(
            r#"
            SELECT id, units, status
            FROM "ob-poc".holdings
            WHERE share_class_id = $1 AND investor_entity_id = $2
            FOR UPDATE
            "#,
        )
        .bind(share_class_id)
        .bind(shareholder_entity_id)
        .fetch_optional(scope.executor())
        .await?
        .ok_or_else(|| anyhow!("Holding not found for entity {}", shareholder_entity_id))?;

        let holding_id: Uuid = row.get("id");
        let units: rust_decimal::Decimal = row.get("units");
        let status: String = row.get("status");

        if status == "closed" {
            return Err(anyhow!("Holding {} is already closed", holding_id));
        }
        if units != rust_decimal::Decimal::ZERO {
            return Err(anyhow!(
                "Cannot close holding {}: {} units still outstanding",
                holding_id,
                units
            ));
        }

        let rows = sqlx::query(
            r#"
            UPDATE "ob-poc".holdings
            SET status = 'closed', updated_at = now()
            WHERE id = $1 AND units = 0
            "#,
        )
        .bind(holding_id)
        .execute(scope.executor())
        .await?
        .rows_affected();
        if rows == 0 {
            return Err(anyhow!("Concurrent modification detected"));
        }

        Ok(VerbExecutionOutcome::Affected(rows))
    }
}

