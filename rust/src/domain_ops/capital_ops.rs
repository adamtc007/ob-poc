//! Capital Structure Operations
//!
//! Plugin handlers for corporate capital structure management.
//! Extends Clearstream-style registry with corporate share semantics.
//!
//! ## Rationale
//! These operations require custom code because they involve:
//! - Multi-table transactions (share classes + holdings + movements)
//! - Reconciliation logic (SUM(holdings) = issued_shares)
//! - Ownership chain traversal with multiplicative percentages

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::json;
use uuid::Uuid;

#[cfg(feature = "database")]
use sqlx::PgPool;

use super::helpers::get_required_uuid;
use super::{CustomOperation, ExecutionContext, ExecutionResult, VerbCall};

/// Transfer shares between shareholders
pub struct CapitalTransferOp;

#[async_trait]
impl CustomOperation for CapitalTransferOp {
    fn domain(&self) -> &'static str {
        "capital"
    }

    fn verb(&self) -> &'static str {
        "transfer"
    }

    fn rationale(&self) -> &'static str {
        "Share transfers require atomic updates to multiple holdings and movement records"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let share_class_id = get_required_uuid(verb_call, "share-class-id")?;
        let from_entity_id = get_required_uuid(verb_call, "from-entity-id")?;
        let to_entity_id = get_required_uuid(verb_call, "to-entity-id")?;
        let units: rust_decimal::Decimal = verb_call
            .get_arg("units")
            .and_then(|a| a.value.as_string())
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| anyhow!("units is required"))?;
        let transfer_date = verb_call
            .get_arg("transfer-date")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow!("transfer-date is required"))?;
        let reference = verb_call
            .get_arg("reference")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow!("reference is required"))?;
        let price_per_unit: Option<rust_decimal::Decimal> = verb_call
            .get_arg("price-per-unit")
            .and_then(|a| a.value.as_string())
            .and_then(|s| s.parse().ok());

        // Start transaction
        let mut tx = pool.begin().await?;

        // 1. Verify source holding has enough units
        let source_holding: Option<(Uuid, rust_decimal::Decimal)> = sqlx::query_as(
            r#"
            SELECT id, units FROM kyc.holdings
            WHERE share_class_id = $1 AND investor_entity_id = $2 AND status = 'active'
            "#,
        )
        .bind(share_class_id)
        .bind(from_entity_id)
        .fetch_optional(&mut *tx)
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

        // 2. Get or create target holding
        let target_holding: Option<(Uuid, rust_decimal::Decimal)> = sqlx::query_as(
            r#"
            SELECT id, units FROM kyc.holdings
            WHERE share_class_id = $1 AND investor_entity_id = $2 AND status = 'active'
            "#,
        )
        .bind(share_class_id)
        .bind(to_entity_id)
        .fetch_optional(&mut *tx)
        .await?;

        let target_holding_id = if let Some((id, _)) = target_holding {
            id
        } else {
            // Create new holding for recipient
            let new_id: (Uuid,) = sqlx::query_as(
                r#"
                INSERT INTO kyc.holdings (share_class_id, investor_entity_id, units, status)
                VALUES ($1, $2, 0, 'active')
                RETURNING id
                "#,
            )
            .bind(share_class_id)
            .bind(to_entity_id)
            .fetch_one(&mut *tx)
            .await?;
            new_id.0
        };

        // 3. Update source holding
        sqlx::query(
            r#"
            UPDATE kyc.holdings SET units = units - $1, updated_at = now()
            WHERE id = $2
            "#,
        )
        .bind(units)
        .bind(source_holding_id)
        .execute(&mut *tx)
        .await?;

        // 4. Update target holding
        sqlx::query(
            r#"
            UPDATE kyc.holdings SET units = units + $1, updated_at = now()
            WHERE id = $2
            "#,
        )
        .bind(units)
        .bind(target_holding_id)
        .execute(&mut *tx)
        .await?;

        // 5. Record transfer-out movement
        let amount = price_per_unit.map(|p| p * units);
        let transfer_out_id: (Uuid,) = sqlx::query_as(
            r#"
            INSERT INTO kyc.movements (
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
        .bind(transfer_date)
        .bind(reference)
        .fetch_one(&mut *tx)
        .await?;

        // 6. Record transfer-in movement
        let _transfer_in_id: (Uuid,) = sqlx::query_as(
            r#"
            INSERT INTO kyc.movements (
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
        .bind(transfer_date)
        .bind(reference)
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(ExecutionResult::Uuid(transfer_out_id.0))
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

/// Reconcile capital structure - verify SUM(holdings) = issued_shares
pub struct CapitalReconcileOp;

#[async_trait]
impl CustomOperation for CapitalReconcileOp {
    fn domain(&self) -> &'static str {
        "capital"
    }

    fn verb(&self) -> &'static str {
        "reconcile"
    }

    fn rationale(&self) -> &'static str {
        "Reconciliation requires aggregation across holdings and computation of ownership/voting percentages"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let entity_id = get_required_uuid(verb_call, "entity-id")?;

        // Get all share classes issued by this entity
        let share_classes: Vec<(Uuid, String, i64, rust_decimal::Decimal)> = sqlx::query_as(
            r#"
            SELECT id, name, issued_shares, voting_rights_per_share
            FROM kyc.share_classes
            WHERE issuer_entity_id = $1 AND status = 'active'
            "#,
        )
        .bind(entity_id)
        .fetch_all(pool)
        .await?;

        let mut total_issued: i64 = 0;
        let mut total_allocated: rust_decimal::Decimal = rust_decimal::Decimal::ZERO;
        let mut shareholders_map: std::collections::HashMap<Uuid, ShareholderInfo> =
            std::collections::HashMap::new();

        for (class_id, class_name, issued, voting_per_share) in &share_classes {
            total_issued += issued;

            // Get holdings for this share class
            let holdings: Vec<(Uuid, rust_decimal::Decimal)> = sqlx::query_as(
                r#"
                SELECT investor_entity_id, units
                FROM kyc.holdings
                WHERE share_class_id = $1 AND status = 'active' AND units > 0
                "#,
            )
            .bind(class_id)
            .fetch_all(pool)
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

        // Calculate percentages
        let total_issued_dec = rust_decimal::Decimal::from(total_issued);
        let total_voting: rust_decimal::Decimal = share_classes
            .iter()
            .map(|(_, _, issued, voting)| rust_decimal::Decimal::from(*issued) * voting)
            .sum();

        let shareholders: Vec<serde_json::Value> = shareholders_map
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

        Ok(ExecutionResult::Record(json!({
            "is_reconciled": is_reconciled,
            "issued_shares": total_issued,
            "allocated_shares": total_allocated.to_string(),
            "unallocated_shares": unallocated.to_string(),
            "shareholders": shareholders
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

#[derive(Debug)]
struct ShareholderInfo {
    entity_id: Uuid,
    total_units: rust_decimal::Decimal,
    total_voting_rights: rust_decimal::Decimal,
    share_classes: Vec<serde_json::Value>,
}

/// Get ownership chain with multiplicative percentages
pub struct CapitalOwnershipChainOp;

#[async_trait]
impl CustomOperation for CapitalOwnershipChainOp {
    fn domain(&self) -> &'static str {
        "capital"
    }

    fn verb(&self) -> &'static str {
        "get-ownership-chain"
    }

    fn rationale(&self) -> &'static str {
        "Ownership chain traversal requires recursive graph walking with multiplicative percentage calculation"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let entity_id = get_required_uuid(verb_call, "entity-id")?;
        let min_pct: rust_decimal::Decimal = verb_call
            .get_arg("min-ownership-pct")
            .and_then(|a| a.value.as_string())
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(|| rust_decimal::Decimal::new(1, 2)); // 0.01 = 1%

        // Use recursive CTE to trace ownership chains
        let chains: Vec<(Uuid, String, String, rust_decimal::Decimal, i32)> = sqlx::query_as(
            r#"
            WITH RECURSIVE ownership_chain AS (
                -- Base case: direct owners of the target entity
                SELECT
                    er.from_entity_id as owner_id,
                    e.name as owner_name,
                    ARRAY[er.from_entity_id]::uuid[] as path,
                    COALESCE(er.percentage, 0)::numeric as cumulative_pct,
                    1 as depth
                FROM "ob-poc".entity_relationships er
                JOIN "ob-poc".entities e ON er.from_entity_id = e.entity_id
                WHERE er.to_entity_id = $1
                  AND er.relationship_type = 'ownership'
                  AND (er.effective_to IS NULL OR er.effective_to > CURRENT_DATE)
                  AND COALESCE(er.percentage, 0) >= $2

                UNION ALL

                -- Recursive case: owners of owners
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
        .fetch_all(pool)
        .await?;

        let chain_data: Vec<serde_json::Value> = chains
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

        Ok(ExecutionResult::RecordSet(chain_data))
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

/// Issue additional shares
pub struct CapitalIssueSharesOp;

#[async_trait]
impl CustomOperation for CapitalIssueSharesOp {
    fn domain(&self) -> &'static str {
        "capital"
    }

    fn verb(&self) -> &'static str {
        "issue-shares"
    }

    fn rationale(&self) -> &'static str {
        "Share issuance requires validation against authorized shares and audit trail"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let share_class_id = get_required_uuid(verb_call, "share-class-id")?;
        let additional_shares: i64 = verb_call
            .get_arg("additional-shares")
            .and_then(|a| a.value.as_string())
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| anyhow!("additional-shares is required"))?;
        let _issue_date = verb_call
            .get_arg("issue-date")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow!("issue-date is required"))?;
        let _reason = verb_call
            .get_arg("reason")
            .and_then(|a| a.value.as_string());

        // Get current share class info
        let share_class: Option<(i64, Option<i64>)> = sqlx::query_as(
            r#"
            SELECT issued_shares, authorized_shares
            FROM kyc.share_classes
            WHERE id = $1
            "#,
        )
        .bind(share_class_id)
        .fetch_optional(pool)
        .await?;

        let (current_issued, authorized) =
            share_class.ok_or_else(|| anyhow!("Share class not found"))?;

        let new_issued = current_issued + additional_shares;

        // Validate against authorized if set
        if let Some(auth) = authorized {
            if new_issued > auth {
                return Err(anyhow!(
                    "Cannot issue {} shares: would exceed authorized {} (current: {})",
                    additional_shares,
                    auth,
                    current_issued
                ));
            }
        }

        // Update issued shares
        let result = sqlx::query(
            r#"
            UPDATE kyc.share_classes
            SET issued_shares = $1, updated_at = now()
            WHERE id = $2
            "#,
        )
        .bind(new_issued)
        .bind(share_class_id)
        .execute(pool)
        .await?;

        Ok(ExecutionResult::Affected(result.rows_affected()))
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

/// Cancel/buyback shares
pub struct CapitalCancelSharesOp;

#[async_trait]
impl CustomOperation for CapitalCancelSharesOp {
    fn domain(&self) -> &'static str {
        "capital"
    }

    fn verb(&self) -> &'static str {
        "cancel-shares"
    }

    fn rationale(&self) -> &'static str {
        "Share cancellation requires validation that cancelled <= unallocated and audit trail"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let share_class_id = get_required_uuid(verb_call, "share-class-id")?;
        let shares_to_cancel: i64 = verb_call
            .get_arg("shares-to-cancel")
            .and_then(|a| a.value.as_string())
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| anyhow!("shares-to-cancel is required"))?;
        let _cancel_date = verb_call
            .get_arg("cancel-date")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow!("cancel-date is required"))?;
        let _reason = verb_call
            .get_arg("reason")
            .and_then(|a| a.value.as_string());

        // Get current info
        let share_class: Option<(i64,)> = sqlx::query_as(
            r#"
            SELECT issued_shares
            FROM kyc.share_classes
            WHERE id = $1
            "#,
        )
        .bind(share_class_id)
        .fetch_optional(pool)
        .await?;

        let (current_issued,) = share_class.ok_or_else(|| anyhow!("Share class not found"))?;

        // Get total allocated
        let allocated: (rust_decimal::Decimal,) = sqlx::query_as(
            r#"
            SELECT COALESCE(SUM(units), 0)
            FROM kyc.holdings
            WHERE share_class_id = $1 AND status = 'active'
            "#,
        )
        .bind(share_class_id)
        .fetch_one(pool)
        .await?;

        let allocated_i64: i64 = allocated
            .0
            .to_string()
            .parse()
            .unwrap_or(current_issued + 1);
        let unallocated = current_issued - allocated_i64;

        if shares_to_cancel > unallocated {
            return Err(anyhow!(
                "Cannot cancel {} shares: only {} unallocated (issued: {}, allocated: {})",
                shares_to_cancel,
                unallocated,
                current_issued,
                allocated_i64
            ));
        }

        let new_issued = current_issued - shares_to_cancel;

        // Update issued shares
        let result = sqlx::query(
            r#"
            UPDATE kyc.share_classes
            SET issued_shares = $1, updated_at = now()
            WHERE id = $2
            "#,
        )
        .bind(new_issued)
        .bind(share_class_id)
        .execute(pool)
        .await?;

        Ok(ExecutionResult::Affected(result.rows_affected()))
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
// New Operations for Capital Structure & Ownership Model (Migration 013)
// ============================================================================

use super::helpers::{extract_string, extract_string_opt, extract_uuid, extract_uuid_opt};
use chrono::NaiveDate;

/// Create a new share class for an issuer
pub struct CapitalShareClassCreateOp;

#[async_trait]
impl CustomOperation for CapitalShareClassCreateOp {
    fn domain(&self) -> &'static str {
        "capital"
    }

    fn verb(&self) -> &'static str {
        "share-class.create"
    }

    fn rationale(&self) -> &'static str {
        "Share class creation requires multi-table setup: share_classes, identifiers, supply"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let issuer_entity_id = extract_uuid(verb_call, ctx, "issuer-entity-id")?;
        let name = extract_string(verb_call, "name")?;
        let instrument_kind = extract_string(verb_call, "instrument-kind")?;

        let votes_per_unit: rust_decimal::Decimal = verb_call
            .get_arg("votes-per-unit")
            .and_then(|a| a.value.as_string())
            .and_then(|s| s.parse().ok())
            .unwrap_or(rust_decimal::Decimal::ONE);

        let economic_per_unit: rust_decimal::Decimal = verb_call
            .get_arg("economic-per-unit")
            .and_then(|a| a.value.as_string())
            .and_then(|s| s.parse().ok())
            .unwrap_or(rust_decimal::Decimal::ONE);

        let currency =
            extract_string_opt(verb_call, "currency").unwrap_or_else(|| "EUR".to_string());

        let authorized_units: Option<rust_decimal::Decimal> = verb_call
            .get_arg("authorized-units")
            .and_then(|a| a.value.as_string())
            .and_then(|s| s.parse().ok());

        let cbu_id = extract_uuid_opt(verb_call, ctx, "cbu-id");

        // Validate issuer exists
        let issuer_exists: bool = sqlx::query_scalar(
            r#"SELECT EXISTS(SELECT 1 FROM "ob-poc".entities WHERE entity_id = $1)"#,
        )
        .bind(issuer_entity_id)
        .fetch_one(pool)
        .await?;

        if !issuer_exists {
            return Err(anyhow!("Issuer entity {} not found", issuer_entity_id));
        }

        let mut tx = pool.begin().await?;

        // Insert share class
        let share_class_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO kyc.share_classes (
                issuer_entity_id, cbu_id, name, instrument_kind,
                votes_per_unit, economic_per_unit, currency,
                authorized_shares, class_category
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'CORPORATE')
            RETURNING id
            "#,
        )
        .bind(issuer_entity_id)
        .bind(cbu_id)
        .bind(&name)
        .bind(&instrument_kind)
        .bind(votes_per_unit)
        .bind(economic_per_unit)
        .bind(&currency)
        .bind(authorized_units)
        .fetch_one(&mut *tx)
        .await?;

        // Auto-generate INTERNAL identifier
        let internal_ref = format!("SC-{}", &share_class_id.to_string()[..8]);
        sqlx::query(
            r#"
            INSERT INTO kyc.share_class_identifiers (
                share_class_id, scheme_code, identifier_value, is_primary
            ) VALUES ($1, 'INTERNAL', $2, true)
            "#,
        )
        .bind(share_class_id)
        .bind(&internal_ref)
        .execute(&mut *tx)
        .await?;

        // Initialize supply at zero
        sqlx::query(
            r#"
            INSERT INTO kyc.share_class_supply (
                share_class_id, authorized_units, issued_units, outstanding_units, as_of_date
            ) VALUES ($1, $2, 0, 0, CURRENT_DATE)
            "#,
        )
        .bind(share_class_id)
        .bind(authorized_units)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        // Bind if :as specified
        if let Some(ref binding) = verb_call.binding {
            ctx.bind(binding, share_class_id);
        }

        tracing::info!(
            "capital.share-class.create: {} ({}) for issuer {}, votes_per_unit={}",
            name,
            share_class_id,
            issuer_entity_id,
            votes_per_unit
        );

        Ok(ExecutionResult::Uuid(share_class_id))
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

/// Get current supply state for a share class
pub struct CapitalShareClassGetSupplyOp;

#[async_trait]
impl CustomOperation for CapitalShareClassGetSupplyOp {
    fn domain(&self) -> &'static str {
        "capital"
    }

    fn verb(&self) -> &'static str {
        "share-class.get-supply"
    }

    fn rationale(&self) -> &'static str {
        "Supply computation uses SQL function for as-of date handling"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let share_class_id = extract_uuid(verb_call, ctx, "share-class-id")?;
        let as_of: NaiveDate = verb_call
            .get_arg("as-of")
            .and_then(|a| a.value.as_string())
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
            .unwrap_or_else(|| chrono::Utc::now().date_naive());

        let supply: Option<(
            Uuid,
            Option<rust_decimal::Decimal>,
            rust_decimal::Decimal,
            rust_decimal::Decimal,
            rust_decimal::Decimal,
            rust_decimal::Decimal,
            rust_decimal::Decimal,
            NaiveDate,
        )> = sqlx::query_as(r#"SELECT * FROM kyc.fn_share_class_supply_at($1, $2)"#)
            .bind(share_class_id)
            .bind(as_of)
            .fetch_optional(pool)
            .await?;

        match supply {
            Some((_, authorized, issued, outstanding, treasury, votes, economic, date)) => {
                Ok(ExecutionResult::Record(json!({
                    "share_class_id": share_class_id,
                    "authorized_units": authorized.map(|d| d.to_string()),
                    "issued_units": issued.to_string(),
                    "outstanding_units": outstanding.to_string(),
                    "treasury_units": treasury.to_string(),
                    "total_votes": votes.to_string(),
                    "total_economic": economic.to_string(),
                    "as_of_date": date.to_string()
                })))
            }
            None => Err(anyhow!("Share class {} not found", share_class_id)),
        }
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

/// Initial issuance of shares (incorporation/fund launch)
pub struct CapitalIssueInitialOp;

#[async_trait]
impl CustomOperation for CapitalIssueInitialOp {
    fn domain(&self) -> &'static str {
        "capital"
    }

    fn verb(&self) -> &'static str {
        "issue.initial"
    }

    fn rationale(&self) -> &'static str {
        "Initial issuance creates event record and updates supply atomically"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let share_class_id = extract_uuid(verb_call, ctx, "share-class-id")?;
        let units: rust_decimal::Decimal = verb_call
            .get_arg("units")
            .and_then(|a| a.value.as_string())
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| anyhow!("units is required"))?;
        let price_per_unit: Option<rust_decimal::Decimal> = verb_call
            .get_arg("price-per-unit")
            .and_then(|a| a.value.as_string())
            .and_then(|s| s.parse().ok());
        let effective_date: NaiveDate = verb_call
            .get_arg("effective-date")
            .and_then(|a| a.value.as_string())
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
            .unwrap_or_else(|| chrono::Utc::now().date_naive());
        let board_resolution_ref = extract_string_opt(verb_call, "board-resolution-ref");

        // Get issuer from share class
        let issuer_entity_id: Uuid =
            sqlx::query_scalar(r#"SELECT issuer_entity_id FROM kyc.share_classes WHERE id = $1"#)
                .bind(share_class_id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| anyhow!("Share class {} not found", share_class_id))?;

        // Check no prior issuance exists
        let prior_exists: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM kyc.issuance_events
                WHERE share_class_id = $1 AND status = 'EFFECTIVE'
            )
            "#,
        )
        .bind(share_class_id)
        .fetch_one(pool)
        .await?;

        if prior_exists {
            return Err(anyhow!(
                "Share class {} already has issuance events. Use capital.issue.new for subsequent issues.",
                share_class_id
            ));
        }

        let mut tx = pool.begin().await?;

        // Insert event
        let event_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO kyc.issuance_events (
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
        .fetch_one(&mut *tx)
        .await?;

        // Update or insert supply
        sqlx::query(
            r#"
            INSERT INTO kyc.share_class_supply (
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
        .execute(&mut *tx)
        .await?;

        // Also update share_classes.issued_shares for compatibility
        sqlx::query(r#"UPDATE kyc.share_classes SET issued_shares = $2 WHERE id = $1"#)
            .bind(share_class_id)
            .bind(units)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;

        if let Some(ref binding) = verb_call.binding {
            ctx.bind(binding, event_id);
        }

        tracing::info!(
            "capital.issue.initial: {} units of {} at {:?}/unit",
            units,
            share_class_id,
            price_per_unit
        );

        Ok(ExecutionResult::Uuid(event_id))
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

/// Subsequent issuance (capital raise)
pub struct CapitalIssueNewOp;

#[async_trait]
impl CustomOperation for CapitalIssueNewOp {
    fn domain(&self) -> &'static str {
        "capital"
    }

    fn verb(&self) -> &'static str {
        "issue.new"
    }

    fn rationale(&self) -> &'static str {
        "New issuance adds to existing supply with event audit trail"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let share_class_id = extract_uuid(verb_call, ctx, "share-class-id")?;
        let units: rust_decimal::Decimal = verb_call
            .get_arg("units")
            .and_then(|a| a.value.as_string())
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| anyhow!("units is required"))?;
        let price_per_unit: rust_decimal::Decimal = verb_call
            .get_arg("price-per-unit")
            .and_then(|a| a.value.as_string())
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| anyhow!("price-per-unit is required"))?;
        let effective_date: NaiveDate = verb_call
            .get_arg("effective-date")
            .and_then(|a| a.value.as_string())
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
            .unwrap_or_else(|| chrono::Utc::now().date_naive());
        let board_resolution_ref = extract_string_opt(verb_call, "board-resolution-ref");

        // Get issuer and current supply
        let share_info: Option<(Uuid, Option<rust_decimal::Decimal>)> = sqlx::query_as(
            r#"
            SELECT sc.issuer_entity_id, scs.issued_units
            FROM kyc.share_classes sc
            LEFT JOIN kyc.share_class_supply scs ON scs.share_class_id = sc.id
            WHERE sc.id = $1
            ORDER BY scs.as_of_date DESC NULLS LAST
            LIMIT 1
            "#,
        )
        .bind(share_class_id)
        .fetch_optional(pool)
        .await?;

        let (issuer_entity_id, current_issued) =
            share_info.ok_or_else(|| anyhow!("Share class {} not found", share_class_id))?;

        let new_issued = current_issued.unwrap_or(rust_decimal::Decimal::ZERO) + units;

        let mut tx = pool.begin().await?;

        // Insert event
        let event_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO kyc.issuance_events (
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
        .fetch_one(&mut *tx)
        .await?;

        // Update supply
        sqlx::query(
            r#"
            INSERT INTO kyc.share_class_supply (
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
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        if let Some(ref binding) = verb_call.binding {
            ctx.bind(binding, event_id);
        }

        Ok(ExecutionResult::Uuid(event_id))
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

/// Stock split with full transactional safety
///
/// Safety features:
/// 1. SERIALIZABLE isolation - prevents concurrent reads seeing stale data
/// 2. Advisory lock on share_class_id - serializes all splits on same class
/// 3. Idempotency key - prevents duplicate application on retry
/// 4. Single transaction - all-or-nothing for holdings, supply, and dilution instruments
pub struct CapitalSplitOp;

#[async_trait]
impl CustomOperation for CapitalSplitOp {
    fn domain(&self) -> &'static str {
        "capital"
    }

    fn verb(&self) -> &'static str {
        "split"
    }

    fn rationale(&self) -> &'static str {
        "Stock splits require ratio-based supply adjustment and holdings update"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let share_class_id = extract_uuid(verb_call, ctx, "share-class-id")?;
        let ratio_from: i32 = verb_call
            .get_arg("ratio-from")
            .and_then(|a| a.value.as_integer())
            .map(|i| i as i32)
            .ok_or_else(|| anyhow!("ratio-from is required"))?;
        let ratio_to: i32 = verb_call
            .get_arg("ratio-to")
            .and_then(|a| a.value.as_integer())
            .map(|i| i as i32)
            .ok_or_else(|| anyhow!("ratio-to is required"))?;
        let effective_date: NaiveDate = verb_call
            .get_arg("effective-date")
            .and_then(|a| a.value.as_string())
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
            .unwrap_or_else(|| chrono::Utc::now().date_naive());
        let record_date: Option<NaiveDate> = verb_call
            .get_arg("record-date")
            .and_then(|a| a.value.as_string())
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());

        if ratio_from <= 0 || ratio_to <= 0 {
            return Err(anyhow!("ratio-from and ratio-to must be positive"));
        }

        // Generate idempotency key from inputs
        let idempotency_key = format!(
            "split:{}:{}:{}:{}",
            share_class_id, ratio_from, ratio_to, effective_date
        );

        // Check for existing operation (idempotent)
        let existing: Option<Uuid> = sqlx::query_scalar(
            r#"SELECT event_id FROM kyc.issuance_events WHERE idempotency_key = $1"#,
        )
        .bind(&idempotency_key)
        .fetch_optional(pool)
        .await?;

        if let Some(event_id) = existing {
            tracing::info!(
                "capital.split: Returning existing event {} (idempotent)",
                event_id
            );
            if let Some(ref binding) = verb_call.binding {
                ctx.bind(binding, event_id);
            }
            return Ok(ExecutionResult::Uuid(event_id));
        }

        // Get issuer and validate share class exists
        let issuer_entity_id: Uuid =
            sqlx::query_scalar(r#"SELECT issuer_entity_id FROM kyc.share_classes WHERE id = $1"#)
                .bind(share_class_id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| anyhow!("Share class {} not found", share_class_id))?;

        // Start SERIALIZABLE transaction
        let mut tx = pool.begin().await?;
        sqlx::query("SET TRANSACTION ISOLATION LEVEL SERIALIZABLE")
            .execute(&mut *tx)
            .await?;

        // Advisory lock on share class (prevents concurrent splits)
        let lock_id: i64 = sqlx::query_scalar(r#"SELECT kyc.uuid_to_lock_id($1)"#)
            .bind(share_class_id)
            .fetch_one(&mut *tx)
            .await?;
        sqlx::query("SELECT pg_advisory_xact_lock($1)")
            .bind(lock_id)
            .execute(&mut *tx)
            .await?;

        // Validate current state
        let current_issued: Option<rust_decimal::Decimal> = sqlx::query_scalar(
            r#"
            SELECT issued_units FROM kyc.share_class_supply
            WHERE share_class_id = $1
            ORDER BY as_of_date DESC
            LIMIT 1
            "#,
        )
        .bind(share_class_id)
        .fetch_optional(&mut *tx)
        .await?;

        let issued = current_issued
            .filter(|&units| units != rust_decimal::Decimal::ZERO)
            .ok_or_else(|| anyhow!("Cannot split share class with no issued units"))?;

        let multiplier =
            rust_decimal::Decimal::from(ratio_to) / rust_decimal::Decimal::from(ratio_from);

        // Calculate units delta for the event (for audit purposes)
        let new_issued = issued * multiplier;
        let units_delta = new_issued - issued;

        // 1. Insert split event with idempotency key
        let event_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO kyc.issuance_events (
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
        .fetch_one(&mut *tx)
        .await?;

        // 2. Update supply (multiply all unit counts)
        let supply_updated = sqlx::query(
            r#"
            UPDATE kyc.share_class_supply
            SET issued_units = issued_units * $2,
                outstanding_units = outstanding_units * $2,
                treasury_units = COALESCE(treasury_units, 0) * $2,
                reserved_units = COALESCE(reserved_units, 0) * $2,
                as_of_event_id = $3,
                updated_at = now()
            WHERE share_class_id = $1
              AND as_of_date = (SELECT MAX(as_of_date) FROM kyc.share_class_supply WHERE share_class_id = $1)
            "#,
        )
        .bind(share_class_id)
        .bind(multiplier)
        .bind(event_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();

        // 3. Bulk update ALL holdings for this share class
        let holdings_updated = sqlx::query(
            r#"
            UPDATE kyc.holdings
            SET units = units * $2,
                cost_basis = CASE WHEN cost_basis IS NOT NULL
                             THEN cost_basis / $2 ELSE NULL END,
                updated_at = now()
            WHERE share_class_id = $1 AND status = 'active'
            "#,
        )
        .bind(share_class_id)
        .bind(multiplier)
        .execute(&mut *tx)
        .await?
        .rows_affected();

        // 4. Bulk update ALL dilution instruments converting to this share class
        let instruments_updated = sqlx::query(
            r#"
            UPDATE kyc.dilution_instruments
            SET conversion_ratio = conversion_ratio * $2,
                exercise_price = CASE WHEN exercise_price IS NOT NULL
                                 THEN exercise_price / $2 ELSE NULL END,
                updated_at = now()
            WHERE converts_to_share_class_id = $1
              AND status = 'ACTIVE'
            "#,
        )
        .bind(share_class_id)
        .bind(multiplier)
        .execute(&mut *tx)
        .await?
        .rows_affected();

        // Commit (releases advisory lock)
        tx.commit().await?;

        if let Some(ref binding) = verb_call.binding {
            ctx.bind(binding, event_id);
        }

        tracing::info!(
            "capital.split: {}:{} on {} - {} supply rows, {} holdings, {} instruments adjusted",
            ratio_from,
            ratio_to,
            share_class_id,
            supply_updated,
            holdings_updated,
            instruments_updated
        );

        Ok(ExecutionResult::Uuid(event_id))
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

/// Share buyback into treasury
pub struct CapitalBuybackOp;

#[async_trait]
impl CustomOperation for CapitalBuybackOp {
    fn domain(&self) -> &'static str {
        "capital"
    }

    fn verb(&self) -> &'static str {
        "buyback"
    }

    fn rationale(&self) -> &'static str {
        "Buyback moves shares to treasury with event audit trail"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let share_class_id = extract_uuid(verb_call, ctx, "share-class-id")?;
        let units: rust_decimal::Decimal = verb_call
            .get_arg("units")
            .and_then(|a| a.value.as_string())
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| anyhow!("units is required"))?;
        let price_per_unit: rust_decimal::Decimal = verb_call
            .get_arg("price-per-unit")
            .and_then(|a| a.value.as_string())
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| anyhow!("price-per-unit is required"))?;
        let effective_date: NaiveDate = verb_call
            .get_arg("effective-date")
            .and_then(|a| a.value.as_string())
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
            .unwrap_or_else(|| chrono::Utc::now().date_naive());

        let issuer_entity_id: Uuid =
            sqlx::query_scalar(r#"SELECT issuer_entity_id FROM kyc.share_classes WHERE id = $1"#)
                .bind(share_class_id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| anyhow!("Share class {} not found", share_class_id))?;

        let total_amount = units * price_per_unit;

        let mut tx = pool.begin().await?;

        let event_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO kyc.issuance_events (
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
        .fetch_one(&mut *tx)
        .await?;

        // Update supply - move to treasury
        sqlx::query(
            r#"
            UPDATE kyc.share_class_supply
            SET outstanding_units = outstanding_units - $2,
                treasury_units = treasury_units + $2,
                as_of_event_id = $3,
                updated_at = now()
            WHERE share_class_id = $1
              AND as_of_date = (SELECT MAX(as_of_date) FROM kyc.share_class_supply WHERE share_class_id = $1)
            "#
        )
        .bind(share_class_id)
        .bind(units)
        .bind(event_id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        if let Some(ref binding) = verb_call.binding {
            ctx.bind(binding, event_id);
        }

        Ok(ExecutionResult::Uuid(event_id))
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

/// Permanent share cancellation
pub struct CapitalCancelOp;

#[async_trait]
impl CustomOperation for CapitalCancelOp {
    fn domain(&self) -> &'static str {
        "capital"
    }

    fn verb(&self) -> &'static str {
        "cancel"
    }

    fn rationale(&self) -> &'static str {
        "Share cancellation reduces issued supply permanently"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let share_class_id = extract_uuid(verb_call, ctx, "share-class-id")?;
        let units: rust_decimal::Decimal = verb_call
            .get_arg("units")
            .and_then(|a| a.value.as_string())
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| anyhow!("units is required"))?;
        let effective_date: NaiveDate = verb_call
            .get_arg("effective-date")
            .and_then(|a| a.value.as_string())
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
            .unwrap_or_else(|| chrono::Utc::now().date_naive());
        let reason = extract_string_opt(verb_call, "reason");

        let issuer_entity_id: Uuid =
            sqlx::query_scalar(r#"SELECT issuer_entity_id FROM kyc.share_classes WHERE id = $1"#)
                .bind(share_class_id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| anyhow!("Share class {} not found", share_class_id))?;

        let mut tx = pool.begin().await?;

        let event_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO kyc.issuance_events (
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
        .fetch_one(&mut *tx)
        .await?;

        // Update supply - reduce issued and outstanding
        sqlx::query(
            r#"
            UPDATE kyc.share_class_supply
            SET issued_units = issued_units - $2,
                outstanding_units = outstanding_units - $2,
                as_of_event_id = $3,
                updated_at = now()
            WHERE share_class_id = $1
              AND as_of_date = (SELECT MAX(as_of_date) FROM kyc.share_class_supply WHERE share_class_id = $1)
            "#
        )
        .bind(share_class_id)
        .bind(units)
        .bind(event_id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        if let Some(ref binding) = verb_call.binding {
            ctx.bind(binding, event_id);
        }

        Ok(ExecutionResult::Uuid(event_id))
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

/// Get full cap table for an issuer
pub struct CapitalCapTableOp;

#[async_trait]
impl CustomOperation for CapitalCapTableOp {
    fn domain(&self) -> &'static str {
        "capital"
    }

    fn verb(&self) -> &'static str {
        "cap-table"
    }

    fn rationale(&self) -> &'static str {
        "Cap table aggregates across share classes with control computation"
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
        let basis =
            extract_string_opt(verb_call, "basis").unwrap_or_else(|| "OUTSTANDING".to_string());

        // Get issuer name
        let issuer_name: String =
            sqlx::query_scalar(r#"SELECT name FROM "ob-poc".entities WHERE entity_id = $1"#)
                .bind(issuer_entity_id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| anyhow!("Issuer entity {} not found", issuer_entity_id))?;

        // Get share classes
        let share_classes: Vec<(Uuid, String, String, rust_decimal::Decimal, rust_decimal::Decimal)> = sqlx::query_as(
            r#"
            SELECT sc.id, sc.name, COALESCE(sc.instrument_kind, 'FUND_UNIT'),
                   COALESCE(scs.issued_units, sc.issued_shares, 0),
                   COALESCE(sc.votes_per_unit, sc.voting_rights_per_share, 1)
            FROM kyc.share_classes sc
            LEFT JOIN kyc.share_class_supply scs ON scs.share_class_id = sc.id
                AND scs.as_of_date = (SELECT MAX(as_of_date) FROM kyc.share_class_supply WHERE share_class_id = sc.id AND as_of_date <= $2)
            WHERE sc.issuer_entity_id = $1
            "#
        )
        .bind(issuer_entity_id)
        .bind(as_of)
        .fetch_all(pool)
        .await?;

        // Get holders using the SQL function - use query() with manual extraction
        // since SQLx FromRow only supports tuples up to ~16 elements
        use sqlx::Row;
        let holder_rows =
            sqlx::query(r#"SELECT * FROM kyc.fn_holder_control_position($1, $2, $3)"#)
                .bind(issuer_entity_id)
                .bind(as_of)
                .bind(&basis)
                .fetch_all(pool)
                .await?;

        // Extract holder data from rows
        struct HolderData {
            holder_entity_id: Uuid,
            holder_name: String,
            holder_type: String,
            holder_units: rust_decimal::Decimal,
            holder_votes: rust_decimal::Decimal,
            holder_economic: rust_decimal::Decimal,
            voting_pct: rust_decimal::Decimal,
            economic_pct: rust_decimal::Decimal,
            has_control: bool,
            has_significant_influence: bool,
            has_board_rights: bool,
            board_seats: i32,
        }

        let holders: Vec<HolderData> = holder_rows
            .iter()
            .map(|row| HolderData {
                holder_entity_id: row.get("holder_entity_id"),
                holder_name: row.get("holder_name"),
                holder_type: row.get("holder_type"),
                holder_units: row.get("holder_units"),
                holder_votes: row.get("holder_votes"),
                holder_economic: row.get("holder_economic"),
                voting_pct: row.get("voting_pct"),
                economic_pct: row.get("economic_pct"),
                has_control: row.get("has_control"),
                has_significant_influence: row.get("has_significant_influence"),
                has_board_rights: row.get("has_board_rights"),
                board_seats: row.get("board_seats"),
            })
            .collect();

        let total_votes: rust_decimal::Decimal = share_classes
            .iter()
            .map(|(_, _, _, issued, votes_per)| issued * votes_per)
            .sum();
        let total_economic: rust_decimal::Decimal = share_classes
            .iter()
            .map(|(_, _, _, issued, _)| *issued)
            .sum();

        let share_class_data: Vec<serde_json::Value> = share_classes.iter()
            .map(|(id, name, kind, issued, votes_per)| {
                let class_votes = issued * votes_per;
                json!({
                    "share_class_id": id,
                    "name": name,
                    "instrument_kind": kind,
                    "issued_units": issued.to_string(),
                    "votes_per_unit": votes_per.to_string(),
                    "total_votes": class_votes.to_string(),
                    "voting_weight_pct": if total_votes > rust_decimal::Decimal::ZERO {
                        (class_votes / total_votes * rust_decimal::Decimal::from(100)).round_dp(2).to_string()
                    } else { "0".to_string() }
                })
            })
            .collect();

        let holder_data: Vec<serde_json::Value> = holders
            .iter()
            .map(|h| {
                json!({
                    "holder_entity_id": h.holder_entity_id,
                    "holder_name": h.holder_name,
                    "holder_type": h.holder_type,
                    "units": h.holder_units.to_string(),
                    "votes": h.holder_votes.to_string(),
                    "economic": h.holder_economic.to_string(),
                    "voting_pct": h.voting_pct.to_string(),
                    "economic_pct": h.economic_pct.to_string(),
                    "has_control": h.has_control,
                    "has_significant_influence": h.has_significant_influence,
                    "has_board_rights": h.has_board_rights,
                    "board_seats": h.board_seats
                })
            })
            .collect();

        Ok(ExecutionResult::Record(json!({
            "issuer_entity_id": issuer_entity_id,
            "issuer_name": issuer_name,
            "as_of_date": as_of.to_string(),
            "basis": basis,
            "share_classes": share_class_data,
            "holders": holder_data,
            "total_votes": total_votes.to_string(),
            "total_economic": total_economic.to_string()
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

/// List all holders for an issuer with ownership percentages
pub struct CapitalHoldersOp;

#[async_trait]
impl CustomOperation for CapitalHoldersOp {
    fn domain(&self) -> &'static str {
        "capital"
    }

    fn verb(&self) -> &'static str {
        "holders"
    }

    fn rationale(&self) -> &'static str {
        "Holder listing uses control position function for computed fields"
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
        let min_pct: Option<rust_decimal::Decimal> = verb_call
            .get_arg("min-pct")
            .and_then(|a| a.value.as_string())
            .and_then(|s| s.parse().ok());

        // Use query() with manual extraction since SQLx FromRow only supports tuples up to ~16 elements
        use sqlx::Row;
        let holder_rows =
            sqlx::query(r#"SELECT * FROM kyc.fn_holder_control_position($1, $2, 'VOTES')"#)
                .bind(issuer_entity_id)
                .bind(as_of)
                .fetch_all(pool)
                .await?;

        let filtered: Vec<serde_json::Value> = holder_rows
            .iter()
            .filter(|row| {
                let voting_pct: rust_decimal::Decimal = row.get("voting_pct");
                min_pct.is_none_or(|min| voting_pct >= min)
            })
            .map(|row| {
                json!({
                    "holder_entity_id": row.get::<Uuid, _>("holder_entity_id"),
                    "holder_name": row.get::<String, _>("holder_name"),
                    "holder_type": row.get::<String, _>("holder_type"),
                    "units": row.get::<rust_decimal::Decimal, _>("holder_units").to_string(),
                    "votes": row.get::<rust_decimal::Decimal, _>("holder_votes").to_string(),
                    "economic": row.get::<rust_decimal::Decimal, _>("holder_economic").to_string(),
                    "voting_pct": row.get::<rust_decimal::Decimal, _>("voting_pct").to_string(),
                    "economic_pct": row.get::<rust_decimal::Decimal, _>("economic_pct").to_string(),
                    "has_control": row.get::<bool, _>("has_control"),
                    "has_significant_influence": row.get::<bool, _>("has_significant_influence"),
                    "has_board_rights": row.get::<bool, _>("has_board_rights"),
                    "board_seats": row.get::<i32, _>("board_seats")
                })
            })
            .collect();

        Ok(ExecutionResult::RecordSet(filtered))
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
