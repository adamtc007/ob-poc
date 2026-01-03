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
