//! Deal Record Operations (28 plugin verbs) — YAML-first re-implementation of
//! `deal.*` from `rust/config/verbs/deal.yaml`.
//!
//! Deal Record is the commercial origination hub that links Sales through contracting,
//! onboarding, servicing, and billing in a closed loop.
//!
//! # Ops
//!
//! CRUD:
//! - `deal.create` — Create a new deal record (PROSPECT status)
//! - `deal.search` — Full-text search by deal name or reference
//! - `deal.update` — Update deal fields (name, owner, revenue, notes)
//! - `deal.update-status` — State machine transition with KYC gating
//! - `deal.cancel` — Cancel a deal (only from pre-active states)
//!
//! Participants:
//! - `deal.add-participant` — Upsert participant with one-primary-per-deal enforcement
//! - `deal.remove-participant` — Remove participant (optional role filter)
//!
//! Contracts:
//! - `deal.add-contract` — Link contract (requires KYC clearance)
//! - `deal.remove-contract` — Unlink contract (validates no dependent rate cards)
//!
//! Products:
//! - `deal.add-product` — Add product to deal commercial scope
//! - `deal.update-product-status` — Update product status with AGREED timestamp
//! - `deal.remove-product` — Soft-delete (sets status REMOVED)
//!
//! Rate Cards:
//! - `deal.create-rate-card` — Create DRAFT rate card for product
//! - `deal.add-rate-card-line` — Add fee line (DRAFT/PROPOSED only)
//! - `deal.update-rate-card-line` — Modify line values (non-AGREED only)
//! - `deal.remove-rate-card-line` — Delete line (validates no billing deps)
//! - `deal.propose-rate-card` — Transition to PROPOSED (requires lines)
//! - `deal.counter-rate-card` — Clone with COUNTER_OFFERED, mark original SUPERSEDED
//! - `deal.agree-rate-card` — Transition to AGREED (from PROPOSED/COUNTER_OFFERED)
//!
//! SLA & Documents:
//! - `deal.add-sla` — Create SLA record with optional contract/product/service
//! - `deal.add-document` — Link document to deal
//! - `deal.update-document-status` — Update document status
//!
//! UBO & Onboarding:
//! - `deal.add-ubo-assessment` — Link entity UBO assessment
//! - `deal.update-ubo-assessment` — Update assessment status / risk rating
//! - `deal.request-onboarding` — Create single onboarding request (validates prereqs)
//! - `deal.request-onboarding-batch` — Batch insert onboarding requests
//! - `deal.update-onboarding-status` — Update request; auto-transition deal to ACTIVE when all complete
//!
//! Summary:
//! - `deal.read-summary` — Composite summary with counts

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use dsl_runtime::domain_ops::helpers::{
    json_extract_bool_opt, json_extract_string, json_extract_string_opt, json_extract_uuid,
    json_extract_uuid_opt,
};
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::SemOsVerbOp;

// =============================================================================
// Result Types
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DealCreateResult {
    pub deal_id: Uuid,
    pub deal_name: String,
    pub deal_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DealStatusUpdateResult {
    pub deal_id: Uuid,
    pub old_status: String,
    pub new_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchOnboardingResult {
    pub request_ids: Vec<Uuid>,
    pub count: i32,
}

// =============================================================================
// State Machine Validation
// =============================================================================

/// Valid deal status transitions
fn is_valid_deal_status_transition(from: &str, to: &str) -> bool {
    matches!(
        (from, to),
        ("PROSPECT", "QUALIFYING")
            | ("PROSPECT", "CANCELLED")
            | ("QUALIFYING", "NEGOTIATING")
            | ("QUALIFYING", "CANCELLED")
            | ("NEGOTIATING", "KYC_CLEARANCE")
            | ("NEGOTIATING", "QUALIFYING")
            | ("NEGOTIATING", "CANCELLED")
            | ("KYC_CLEARANCE", "CONTRACTED")
            | ("KYC_CLEARANCE", "NEGOTIATING")
            | ("KYC_CLEARANCE", "CANCELLED")
            | ("CONTRACTED", "ONBOARDING")
            | ("CONTRACTED", "CANCELLED")
            | ("ONBOARDING", "ACTIVE")
            | ("ONBOARDING", "CANCELLED")
            | ("ACTIVE", "WINDING_DOWN")
            | ("WINDING_DOWN", "OFFBOARDED")
    )
}

async fn deal_has_group_approved_kyc_clearance(
    scope: &mut dyn TransactionScope,
    deal_id: Uuid,
) -> Result<bool> {
    let has_approved_case: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1
            FROM "ob-poc".deals d
            JOIN "ob-poc".cases c
              ON c.client_group_id = d.primary_client_group_id
            WHERE d.deal_id = $1
              AND status = 'APPROVED'
        )
        "#,
    )
    .bind(deal_id)
    .fetch_one(scope.executor())
    .await?;

    Ok(has_approved_case)
}

async fn deal_controls_cbu(
    scope: &mut dyn TransactionScope,
    deal_id: Uuid,
    cbu_id: Uuid,
) -> Result<bool> {
    let is_controlled: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1
            FROM "ob-poc".deals d
            JOIN "ob-poc".client_group_entity cge
              ON cge.group_id = d.primary_client_group_id
            WHERE d.deal_id = $1
              AND cge.cbu_id = $2
              AND cge.membership_type <> 'historical'
        )
        "#,
    )
    .bind(deal_id)
    .bind(cbu_id)
    .fetch_one(scope.executor())
    .await?;

    Ok(is_controlled)
}

// =============================================================================
// deal.create
// =============================================================================

/// Create a new deal record.
pub struct Create;

#[async_trait]
impl SemOsVerbOp for Create {
    fn fqn(&self) -> &str {
        "deal.create"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let deal_name = json_extract_string(args, "deal-name")?;
        let primary_client_group_id = json_extract_uuid(args, ctx, "primary-client-group-id")?;
        let deal_reference = json_extract_string_opt(args, "deal-reference");
        let sales_owner = json_extract_string_opt(args, "sales-owner");
        let sales_team = json_extract_string_opt(args, "sales-team");
        let estimated_revenue: Option<f64> = args.get("estimated-revenue").and_then(|v| v.as_f64());
        let currency_code = json_extract_string_opt(args, "currency-code").unwrap_or("USD".into());
        let notes = json_extract_string_opt(args, "notes");

        let deal_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO "ob-poc".deals (
                deal_name, primary_client_group_id, deal_reference,
                sales_owner, sales_team, estimated_revenue, currency_code, notes
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING deal_id
            "#,
        )
        .bind(&deal_name)
        .bind(primary_client_group_id)
        .bind(&deal_reference)
        .bind(&sales_owner)
        .bind(&sales_team)
        .bind(estimated_revenue)
        .bind(&currency_code)
        .bind(&notes)
        .fetch_one(scope.executor())
        .await?;

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".deal_events (deal_id, event_type, subject_type, subject_id, new_value, actor)
            VALUES ($1, 'DEAL_CREATED', 'DEAL', $1, 'PROSPECT', $2)
            "#,
        )
        .bind(deal_id)
        .bind(&sales_owner)
        .execute(scope.executor())
        .await?;

        // Phase C.3 rollout: new deal enters at PROSPECT state.
        dsl_runtime::domain_ops::helpers::emit_pending_state_advance(
            ctx,
            deal_id,
            "deal:prospect",
            "deal/lifecycle",
            "deal.create — new deal at PROSPECT",
        );

        Ok(VerbExecutionOutcome::Uuid(deal_id))
    }
}

// =============================================================================
// deal.search
// =============================================================================

/// Search deals by name or reference.
pub struct Search;

#[async_trait]
impl SemOsVerbOp for Search {
    fn fqn(&self) -> &str {
        "deal.search"
    }

    async fn execute(
        &self,
        args: &Value,
        _ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let query = json_extract_string(args, "query")?;
        let pattern = format!("%{}%", query);

        let rows: Vec<(Uuid, String, Option<String>, String, Option<String>)> = sqlx::query_as(
            r#"
            SELECT deal_id, deal_name, deal_reference, deal_status, sales_owner
            FROM "ob-poc".deals
            WHERE deal_name ILIKE $1 OR deal_reference ILIKE $1
            ORDER BY opened_at DESC
            LIMIT 50
            "#,
        )
        .bind(&pattern)
        .fetch_all(scope.executor())
        .await?;

        let results: Vec<Value> = rows
            .into_iter()
            .map(|(deal_id, deal_name, deal_ref, status, owner)| {
                serde_json::json!({
                    "deal_id": deal_id,
                    "deal_name": deal_name,
                    "deal_reference": deal_ref,
                    "deal_status": status,
                    "sales_owner": owner
                })
            })
            .collect();

        Ok(VerbExecutionOutcome::RecordSet(results))
    }
}

// =============================================================================
// deal.update
// =============================================================================

/// Update deal record fields.
pub struct Update;

#[async_trait]
impl SemOsVerbOp for Update {
    fn fqn(&self) -> &str {
        "deal.update"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let deal_id = json_extract_uuid(args, ctx, "deal-id")?;
        let deal_name = json_extract_string_opt(args, "deal-name");
        let sales_owner = json_extract_string_opt(args, "sales-owner");
        let estimated_revenue: Option<f64> = args.get("estimated-revenue").and_then(|v| v.as_f64());
        let notes = json_extract_string_opt(args, "notes");

        let result = sqlx::query(
            r#"
            UPDATE "ob-poc".deals
            SET
                deal_name = COALESCE($2, deal_name),
                sales_owner = COALESCE($3, sales_owner),
                estimated_revenue = COALESCE($4, estimated_revenue),
                notes = COALESCE($5, notes),
                updated_at = NOW()
            WHERE deal_id = $1
            "#,
        )
        .bind(deal_id)
        .bind(&deal_name)
        .bind(&sales_owner)
        .bind(estimated_revenue)
        .bind(&notes)
        .execute(scope.executor())
        .await?;

        Ok(VerbExecutionOutcome::Affected(result.rows_affected()))
    }
}

// =============================================================================
// deal.update-status
// =============================================================================

/// Transition deal status with lifecycle validation.
pub struct UpdateStatus;

#[async_trait]
impl SemOsVerbOp for UpdateStatus {
    fn fqn(&self) -> &str {
        "deal.update-status"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let deal_id = json_extract_uuid(args, ctx, "deal-id")?;
        let new_status = json_extract_string(args, "new-status")?;

        let current_status: String =
            sqlx::query_scalar(r#"SELECT deal_status FROM "ob-poc".deals WHERE deal_id = $1"#)
                .bind(deal_id)
                .fetch_one(scope.executor())
                .await?;

        if !is_valid_deal_status_transition(&current_status, &new_status) {
            return Err(anyhow!(
                "Invalid status transition from {} to {}",
                current_status,
                new_status
            ));
        }

        if new_status == "CONTRACTED"
            && !deal_has_group_approved_kyc_clearance(scope, deal_id).await?
        {
            return Err(anyhow!(
                "Deal cannot move to CONTRACTED until the primary client group has APPROVED KYC clearance"
            ));
        }

        let timestamp_column = match new_status.as_str() {
            "QUALIFYING" => Some("qualified_at"),
            "CONTRACTED" => Some("contracted_at"),
            "ACTIVE" => Some("active_at"),
            "OFFBOARDED" | "CANCELLED" => Some("closed_at"),
            _ => None,
        };

        if let Some(col) = timestamp_column {
            let query = format!(
                r#"UPDATE "ob-poc".deals SET deal_status = $2, {} = NOW(), updated_at = NOW() WHERE deal_id = $1"#,
                col
            );
            sqlx::query(&query)
                .bind(deal_id)
                .bind(&new_status)
                .execute(scope.executor())
                .await?;
        } else {
            sqlx::query(
                r#"UPDATE "ob-poc".deals SET deal_status = $2, updated_at = NOW() WHERE deal_id = $1"#,
            )
            .bind(deal_id)
            .bind(&new_status)
            .execute(scope.executor())
            .await?;
        }

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".deal_events (deal_id, event_type, subject_type, subject_id, old_value, new_value)
            VALUES ($1, 'STATUS_CHANGED', 'DEAL', $1, $2, $3)
            "#,
        )
        .bind(deal_id)
        .bind(&current_status)
        .bind(&new_status)
        .execute(scope.executor())
        .await?;

        // Phase C.3 rollout: deal.update-status is the primary
        // state-transition verb for the deal lifecycle. Validator above
        // (`is_valid_deal_status_transition` + KYC gate) guarantees a
        // genuine state advance by the time we reach this line.
        let to_node = format!("deal:{}", new_status.to_lowercase());
        let reason = format!("deal.update-status — {} → {}", current_status, new_status);
        dsl_runtime::domain_ops::helpers::emit_pending_state_advance(
            ctx,
            deal_id,
            &to_node,
            "deal/lifecycle",
            &reason,
        );

        let result = DealStatusUpdateResult {
            deal_id,
            old_status: current_status,
            new_status,
        };
        Ok(VerbExecutionOutcome::Record(serde_json::to_value(result)?))
    }
}

// =============================================================================
// deal.cancel
// =============================================================================

/// Cancel a deal.
pub struct Cancel;

#[async_trait]
impl SemOsVerbOp for Cancel {
    fn fqn(&self) -> &str {
        "deal.cancel"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let deal_id = json_extract_uuid(args, ctx, "deal-id")?;
        let reason = json_extract_string(args, "reason")?;

        let current_status: String =
            sqlx::query_scalar(r#"SELECT deal_status FROM "ob-poc".deals WHERE deal_id = $1"#)
                .bind(deal_id)
                .fetch_one(scope.executor())
                .await?;

        if matches!(
            current_status.as_str(),
            "ACTIVE" | "WINDING_DOWN" | "OFFBOARDED" | "CANCELLED"
        ) {
            return Err(anyhow!("Cannot cancel deal in status {}", current_status));
        }

        sqlx::query(
            r#"
            UPDATE "ob-poc".deals
            SET deal_status = 'CANCELLED', closed_at = NOW(), updated_at = NOW()
            WHERE deal_id = $1
            "#,
        )
        .bind(deal_id)
        .execute(scope.executor())
        .await?;

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".deal_events (deal_id, event_type, subject_type, subject_id, old_value, new_value, description)
            VALUES ($1, 'STATUS_CHANGED', 'DEAL', $1, $2, 'CANCELLED', $3)
            "#,
        )
        .bind(deal_id)
        .bind(&current_status)
        .bind(&reason)
        .execute(scope.executor())
        .await?;

        // Phase C.3 rollout: deal cancellation is a terminal state
        // advance. Reason string captures the operator-supplied reason
        // so narration can surface it downstream.
        let advance_reason = format!("deal.cancel — {} → CANCELLED ({})", current_status, reason);
        dsl_runtime::domain_ops::helpers::emit_pending_state_advance(
            ctx,
            deal_id,
            "deal:cancelled",
            "deal/lifecycle",
            &advance_reason,
        );

        Ok(VerbExecutionOutcome::Affected(1))
    }
}

// =============================================================================
// deal.add-participant
// =============================================================================

/// Add a participant to a deal.
pub struct AddParticipant;

#[async_trait]
impl SemOsVerbOp for AddParticipant {
    fn fqn(&self) -> &str {
        "deal.add-participant"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let deal_id = json_extract_uuid(args, ctx, "deal-id")?;
        let entity_id = json_extract_uuid(args, ctx, "entity-id")?;
        let participant_role = json_extract_string_opt(args, "participant-role")
            .unwrap_or_else(|| "CONTRACTING_PARTY".to_string());
        let lei = json_extract_string_opt(args, "lei");
        let is_primary = json_extract_bool_opt(args, "is-primary").unwrap_or(false);

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".deal_participants (deal_id, entity_id, participant_role, lei, is_primary)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (deal_id, entity_id, participant_role)
            DO UPDATE SET lei = EXCLUDED.lei, is_primary = EXCLUDED.is_primary
            "#,
        )
        .bind(deal_id)
        .bind(entity_id)
        .bind(&participant_role)
        .bind(&lei)
        .bind(is_primary)
        .execute(scope.executor())
        .await?;

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".deal_events (deal_id, event_type, subject_type, subject_id, description)
            VALUES ($1, 'PARTICIPANT_ADDED', 'ENTITY', $2, $3)
            "#,
        )
        .bind(deal_id)
        .bind(entity_id)
        .bind(&participant_role)
        .execute(scope.executor())
        .await?;

        Ok(VerbExecutionOutcome::Affected(1))
    }
}

// =============================================================================
// deal.remove-participant
// =============================================================================

/// Remove a participant from a deal.
pub struct RemoveParticipant;

#[async_trait]
impl SemOsVerbOp for RemoveParticipant {
    fn fqn(&self) -> &str {
        "deal.remove-participant"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let deal_id = json_extract_uuid(args, ctx, "deal-id")?;
        let entity_id = json_extract_uuid(args, ctx, "entity-id")?;
        let participant_role = json_extract_string_opt(args, "participant-role");

        let result = if let Some(role) = &participant_role {
            sqlx::query(
                r#"DELETE FROM "ob-poc".deal_participants WHERE deal_id = $1 AND entity_id = $2 AND participant_role = $3"#,
            )
            .bind(deal_id)
            .bind(entity_id)
            .bind(role)
            .execute(scope.executor())
            .await?
        } else {
            sqlx::query(
                r#"DELETE FROM "ob-poc".deal_participants WHERE deal_id = $1 AND entity_id = $2"#,
            )
            .bind(deal_id)
            .bind(entity_id)
            .execute(scope.executor())
            .await?
        };

        Ok(VerbExecutionOutcome::Affected(result.rows_affected()))
    }
}

// =============================================================================
// deal.add-contract
// =============================================================================

/// Add a contract to a deal.
pub struct AddContract;

#[async_trait]
impl SemOsVerbOp for AddContract {
    fn fqn(&self) -> &str {
        "deal.add-contract"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let deal_id = json_extract_uuid(args, ctx, "deal-id")?;
        let contract_id = json_extract_uuid(args, ctx, "contract-id")?;
        let contract_role =
            json_extract_string_opt(args, "contract-role").unwrap_or_else(|| "PRIMARY".to_string());
        let sequence_order: i32 = args
            .get("sequence-order")
            .and_then(|v| v.as_i64())
            .map(|i| i as i32)
            .unwrap_or(1);

        if !deal_has_group_approved_kyc_clearance(scope, deal_id).await? {
            return Err(anyhow!(
                "Cannot link contract to deal until the primary client group has APPROVED KYC clearance"
            ));
        }

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".deal_contracts (deal_id, contract_id, contract_role, sequence_order)
            VALUES ($1, $2, $3, $4)
            "#,
        )
        .bind(deal_id)
        .bind(contract_id)
        .bind(&contract_role)
        .bind(sequence_order)
        .execute(scope.executor())
        .await?;

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".deal_events (deal_id, event_type, subject_type, subject_id, description)
            VALUES ($1, 'CONTRACT_ADDED', 'CONTRACT', $2, $3)
            "#,
        )
        .bind(deal_id)
        .bind(contract_id)
        .bind(&contract_role)
        .execute(scope.executor())
        .await?;

        Ok(VerbExecutionOutcome::Affected(1))
    }
}

// =============================================================================
// deal.remove-contract
// =============================================================================

/// Remove a contract from a deal.
pub struct RemoveContract;

#[async_trait]
impl SemOsVerbOp for RemoveContract {
    fn fqn(&self) -> &str {
        "deal.remove-contract"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let deal_id = json_extract_uuid(args, ctx, "deal-id")?;
        let contract_id = json_extract_uuid(args, ctx, "contract-id")?;

        let count: i64 = sqlx::query_scalar(
            r#"SELECT COUNT(*) FROM "ob-poc".deal_rate_cards WHERE deal_id = $1 AND contract_id = $2"#,
        )
        .bind(deal_id)
        .bind(contract_id)
        .fetch_one(scope.executor())
        .await?;

        if count > 0 {
            return Err(anyhow!(
                "Cannot remove contract - {} rate cards depend on it",
                count
            ));
        }

        let result = sqlx::query(
            r#"DELETE FROM "ob-poc".deal_contracts WHERE deal_id = $1 AND contract_id = $2"#,
        )
        .bind(deal_id)
        .bind(contract_id)
        .execute(scope.executor())
        .await?;

        Ok(VerbExecutionOutcome::Affected(result.rows_affected()))
    }
}

// =============================================================================
// deal.add-product
// =============================================================================

/// Add a product to the deal's commercial scope.
pub struct AddProduct;

#[async_trait]
impl SemOsVerbOp for AddProduct {
    fn fqn(&self) -> &str {
        "deal.add-product"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let deal_id = json_extract_uuid(args, ctx, "deal-id")?;
        let product_id = json_extract_uuid(args, ctx, "product-id")?;
        let product_status = json_extract_string_opt(args, "product-status")
            .unwrap_or_else(|| "PROPOSED".to_string());
        let indicative_revenue: Option<f64> =
            args.get("indicative-revenue").and_then(|v| v.as_f64());
        let currency_code =
            json_extract_string_opt(args, "currency-code").unwrap_or_else(|| "USD".to_string());
        let notes = json_extract_string_opt(args, "notes");

        let deal_product_id: (Uuid,) = sqlx::query_as(
            r#"
            INSERT INTO "ob-poc".deal_products
                (deal_id, product_id, product_status, indicative_revenue, currency_code, notes, added_at)
            VALUES ($1, $2, $3, $4, $5, $6, NOW())
            ON CONFLICT (deal_id, product_id) DO UPDATE SET
                product_status = EXCLUDED.product_status,
                indicative_revenue = COALESCE(EXCLUDED.indicative_revenue, "ob-poc".deal_products.indicative_revenue),
                currency_code = EXCLUDED.currency_code,
                notes = COALESCE(EXCLUDED.notes, "ob-poc".deal_products.notes),
                updated_at = NOW()
            RETURNING deal_product_id
            "#,
        )
        .bind(deal_id)
        .bind(product_id)
        .bind(&product_status)
        .bind(indicative_revenue)
        .bind(&currency_code)
        .bind(&notes)
        .fetch_one(scope.executor())
        .await?;

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".deal_events (deal_id, event_type, subject_type, subject_id, description)
            VALUES ($1, 'PRODUCT_ADDED', 'PRODUCT', $2, $3)
            "#,
        )
        .bind(deal_id)
        .bind(product_id)
        .bind(&product_status)
        .execute(scope.executor())
        .await?;

        Ok(VerbExecutionOutcome::Uuid(deal_product_id.0))
    }
}

// =============================================================================
// deal.update-product-status
// =============================================================================

/// Update the status of a product in the deal scope.
pub struct UpdateProductStatus;

#[async_trait]
impl SemOsVerbOp for UpdateProductStatus {
    fn fqn(&self) -> &str {
        "deal.update-product-status"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let deal_id = json_extract_uuid(args, ctx, "deal-id")?;
        let product_id = json_extract_uuid(args, ctx, "product-id")?;
        let product_status = json_extract_string(args, "product-status")?;

        let result = if product_status == "AGREED" {
            sqlx::query(
                r#"
                UPDATE "ob-poc".deal_products
                SET product_status = $3, agreed_at = NOW(), updated_at = NOW()
                WHERE deal_id = $1 AND product_id = $2
                "#,
            )
            .bind(deal_id)
            .bind(product_id)
            .bind(&product_status)
            .execute(scope.executor())
            .await?
        } else {
            sqlx::query(
                r#"
                UPDATE "ob-poc".deal_products
                SET product_status = $3, updated_at = NOW()
                WHERE deal_id = $1 AND product_id = $2
                "#,
            )
            .bind(deal_id)
            .bind(product_id)
            .bind(&product_status)
            .execute(scope.executor())
            .await?
        };

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".deal_events (deal_id, event_type, subject_type, subject_id, description)
            VALUES ($1, 'PRODUCT_STATUS_CHANGED', 'PRODUCT', $2, $3)
            "#,
        )
        .bind(deal_id)
        .bind(product_id)
        .bind(&product_status)
        .execute(scope.executor())
        .await?;

        Ok(VerbExecutionOutcome::Affected(result.rows_affected()))
    }
}

// =============================================================================
// deal.remove-product
// =============================================================================

/// Remove a product from the deal scope (soft-delete via status=REMOVED).
pub struct RemoveProduct;

#[async_trait]
impl SemOsVerbOp for RemoveProduct {
    fn fqn(&self) -> &str {
        "deal.remove-product"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let deal_id = json_extract_uuid(args, ctx, "deal-id")?;
        let product_id = json_extract_uuid(args, ctx, "product-id")?;

        let result = sqlx::query(
            r#"
            UPDATE "ob-poc".deal_products
            SET product_status = 'REMOVED', updated_at = NOW()
            WHERE deal_id = $1 AND product_id = $2
            "#,
        )
        .bind(deal_id)
        .bind(product_id)
        .execute(scope.executor())
        .await?;

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".deal_events (deal_id, event_type, subject_type, subject_id, description)
            VALUES ($1, 'PRODUCT_REMOVED', 'PRODUCT', $2, 'Product removed from deal scope')
            "#,
        )
        .bind(deal_id)
        .bind(product_id)
        .execute(scope.executor())
        .await?;

        Ok(VerbExecutionOutcome::Affected(result.rows_affected()))
    }
}

// =============================================================================
// deal.create-rate-card
// =============================================================================

/// Create a negotiated rate card for a product within a deal.
pub struct CreateRateCard;

#[async_trait]
impl SemOsVerbOp for CreateRateCard {
    fn fqn(&self) -> &str {
        "deal.create-rate-card"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let deal_id = json_extract_uuid(args, ctx, "deal-id")?;
        let contract_id = json_extract_uuid(args, ctx, "contract-id")?;
        let product_id = json_extract_uuid(args, ctx, "product-id")?;
        let rate_card_name = json_extract_string_opt(args, "rate-card-name");
        let effective_from = json_extract_string(args, "effective-from")?;
        let effective_to = json_extract_string_opt(args, "effective-to");

        let exists: bool = sqlx::query_scalar(
            r#"SELECT EXISTS(SELECT 1 FROM "ob-poc".deal_contracts WHERE deal_id = $1 AND contract_id = $2)"#,
        )
        .bind(deal_id)
        .bind(contract_id)
        .fetch_one(scope.executor())
        .await?;

        if !exists {
            return Err(anyhow!("Contract is not linked to this deal"));
        }

        let rate_card_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO "ob-poc".deal_rate_cards (deal_id, contract_id, product_id, rate_card_name, effective_from, effective_to)
            VALUES ($1, $2, $3, $4, $5::date, $6::date)
            RETURNING rate_card_id
            "#,
        )
        .bind(deal_id)
        .bind(contract_id)
        .bind(product_id)
        .bind(&rate_card_name)
        .bind(&effective_from)
        .bind(&effective_to)
        .fetch_one(scope.executor())
        .await?;

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".deal_events (deal_id, event_type, subject_type, subject_id, new_value)
            VALUES ($1, 'RATE_CARD_CREATED', 'RATE_CARD', $2, 'DRAFT')
            "#,
        )
        .bind(deal_id)
        .bind(rate_card_id)
        .execute(scope.executor())
        .await?;

        Ok(VerbExecutionOutcome::Uuid(rate_card_id))
    }
}

// =============================================================================
// deal.add-rate-card-line
// =============================================================================

/// Add a fee line to a rate card.
pub struct AddRateCardLine;

#[async_trait]
impl SemOsVerbOp for AddRateCardLine {
    fn fqn(&self) -> &str {
        "deal.add-rate-card-line"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let rate_card_id = json_extract_uuid(args, ctx, "rate-card-id")?;
        let fee_type = json_extract_string(args, "fee-type")?;
        let fee_subtype =
            json_extract_string_opt(args, "fee-subtype").unwrap_or_else(|| "DEFAULT".to_string());
        let pricing_model = json_extract_string(args, "pricing-model")?;
        let rate_value: Option<f64> = args.get("rate-value").and_then(|v| v.as_f64());
        let minimum_fee: Option<f64> = args.get("minimum-fee").and_then(|v| v.as_f64());
        let maximum_fee: Option<f64> = args.get("maximum-fee").and_then(|v| v.as_f64());
        let currency_code =
            json_extract_string_opt(args, "currency-code").unwrap_or_else(|| "USD".to_string());
        let tier_brackets = args.get("tier-brackets").cloned();
        let fee_basis = json_extract_string_opt(args, "fee-basis");
        let description = json_extract_string_opt(args, "description");

        let status: String = sqlx::query_scalar(
            r#"SELECT status FROM "ob-poc".deal_rate_cards WHERE rate_card_id = $1"#,
        )
        .bind(rate_card_id)
        .fetch_one(scope.executor())
        .await?;

        if !matches!(status.as_str(), "DRAFT" | "PROPOSED") {
            return Err(anyhow!("Cannot modify rate card in status {}", status));
        }

        let line_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO "ob-poc".deal_rate_card_lines (
                rate_card_id, fee_type, fee_subtype, pricing_model,
                rate_value, minimum_fee, maximum_fee, currency_code,
                tier_brackets, fee_basis, description
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            RETURNING line_id
            "#,
        )
        .bind(rate_card_id)
        .bind(&fee_type)
        .bind(&fee_subtype)
        .bind(&pricing_model)
        .bind(rate_value)
        .bind(minimum_fee)
        .bind(maximum_fee)
        .bind(&currency_code)
        .bind(&tier_brackets)
        .bind(&fee_basis)
        .bind(&description)
        .fetch_one(scope.executor())
        .await?;

        Ok(VerbExecutionOutcome::Uuid(line_id))
    }
}

// =============================================================================
// deal.update-rate-card-line
// =============================================================================

/// Modify an existing rate card line.
pub struct UpdateRateCardLine;

#[async_trait]
impl SemOsVerbOp for UpdateRateCardLine {
    fn fqn(&self) -> &str {
        "deal.update-rate-card-line"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let line_id = json_extract_uuid(args, ctx, "line-id")?;
        let rate_value: Option<f64> = args.get("rate-value").and_then(|v| v.as_f64());
        let minimum_fee: Option<f64> = args.get("minimum-fee").and_then(|v| v.as_f64());
        let maximum_fee: Option<f64> = args.get("maximum-fee").and_then(|v| v.as_f64());
        let tier_brackets = args.get("tier-brackets").cloned();

        let status: String = sqlx::query_scalar(
            r#"
            SELECT rc.status
            FROM "ob-poc".deal_rate_cards rc
            JOIN "ob-poc".deal_rate_card_lines l ON rc.rate_card_id = l.rate_card_id
            WHERE l.line_id = $1
            "#,
        )
        .bind(line_id)
        .fetch_one(scope.executor())
        .await?;

        if status == "AGREED" {
            return Err(anyhow!("Cannot modify lines on an AGREED rate card"));
        }

        let result = sqlx::query(
            r#"
            UPDATE "ob-poc".deal_rate_card_lines
            SET
                rate_value = COALESCE($2, rate_value),
                minimum_fee = COALESCE($3, minimum_fee),
                maximum_fee = COALESCE($4, maximum_fee),
                tier_brackets = COALESCE($5, tier_brackets)
            WHERE line_id = $1
            "#,
        )
        .bind(line_id)
        .bind(rate_value)
        .bind(minimum_fee)
        .bind(maximum_fee)
        .bind(&tier_brackets)
        .execute(scope.executor())
        .await?;

        Ok(VerbExecutionOutcome::Affected(result.rows_affected()))
    }
}

// =============================================================================
// deal.remove-rate-card-line
// =============================================================================

/// Remove a fee line from a rate card.
pub struct RemoveRateCardLine;

#[async_trait]
impl SemOsVerbOp for RemoveRateCardLine {
    fn fqn(&self) -> &str {
        "deal.remove-rate-card-line"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let line_id = json_extract_uuid(args, ctx, "line-id")?;

        let count: i64 = sqlx::query_scalar(
            r#"SELECT COUNT(*) FROM "ob-poc".fee_billing_account_targets WHERE rate_card_line_id = $1"#,
        )
        .bind(line_id)
        .fetch_one(scope.executor())
        .await?;

        if count > 0 {
            return Err(anyhow!(
                "Cannot remove line - {} billing targets depend on it",
                count
            ));
        }

        let result = sqlx::query(r#"DELETE FROM "ob-poc".deal_rate_card_lines WHERE line_id = $1"#)
            .bind(line_id)
            .execute(scope.executor())
            .await?;

        Ok(VerbExecutionOutcome::Affected(result.rows_affected()))
    }
}

// =============================================================================
// deal.propose-rate-card
// =============================================================================

/// Propose rate card for client review.
pub struct ProposeRateCard;

#[async_trait]
impl SemOsVerbOp for ProposeRateCard {
    fn fqn(&self) -> &str {
        "deal.propose-rate-card"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let rate_card_id = json_extract_uuid(args, ctx, "rate-card-id")?;

        let count: i64 = sqlx::query_scalar(
            r#"SELECT COUNT(*) FROM "ob-poc".deal_rate_card_lines WHERE rate_card_id = $1"#,
        )
        .bind(rate_card_id)
        .fetch_one(scope.executor())
        .await?;

        if count == 0 {
            return Err(anyhow!("Cannot propose empty rate card"));
        }

        let deal_id: Uuid = sqlx::query_scalar(
            r#"SELECT deal_id FROM "ob-poc".deal_rate_cards WHERE rate_card_id = $1"#,
        )
        .bind(rate_card_id)
        .fetch_one(scope.executor())
        .await?;

        sqlx::query(
            r#"
            UPDATE "ob-poc".deal_rate_cards
            SET status = 'PROPOSED', negotiation_round = negotiation_round + 1, updated_at = NOW()
            WHERE rate_card_id = $1
            "#,
        )
        .bind(rate_card_id)
        .execute(scope.executor())
        .await?;

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".deal_events (deal_id, event_type, subject_type, subject_id, new_value)
            VALUES ($1, 'RATE_CARD_PROPOSED', 'RATE_CARD', $2, 'PROPOSED')
            "#,
        )
        .bind(deal_id)
        .bind(rate_card_id)
        .execute(scope.executor())
        .await?;

        Ok(VerbExecutionOutcome::Affected(1))
    }
}

// =============================================================================
// deal.counter-rate-card
// =============================================================================

/// Client counter-offer — clones rate card with COUNTER_OFFERED status.
pub struct CounterRateCard;

#[async_trait]
impl SemOsVerbOp for CounterRateCard {
    fn fqn(&self) -> &str {
        "deal.counter-rate-card"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let rate_card_id = json_extract_uuid(args, ctx, "rate-card-id")?;

        let row: (Uuid, Uuid, Uuid, Option<String>, String, Option<String>, i32) = sqlx::query_as(
            r#"
            SELECT deal_id, contract_id, product_id, rate_card_name, effective_from::text, effective_to::text, negotiation_round
            FROM "ob-poc".deal_rate_cards WHERE rate_card_id = $1
            "#,
        )
        .bind(rate_card_id)
        .fetch_one(scope.executor())
        .await?;

        let (deal_id, contract_id, product_id, name, eff_from, eff_to, round) = row;

        let new_rate_card_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO "ob-poc".deal_rate_cards (deal_id, contract_id, product_id, rate_card_name, effective_from, effective_to, status, negotiation_round)
            VALUES ($1, $2, $3, $4, $5::date, $6::date, 'COUNTER_OFFERED', $7)
            RETURNING rate_card_id
            "#,
        )
        .bind(deal_id)
        .bind(contract_id)
        .bind(product_id)
        .bind(&name)
        .bind(&eff_from)
        .bind(&eff_to)
        .bind(round + 1)
        .fetch_one(scope.executor())
        .await?;

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".deal_rate_card_lines (rate_card_id, fee_type, fee_subtype, pricing_model, rate_value, minimum_fee, maximum_fee, currency_code, tier_brackets, fee_basis, description, sequence_order)
            SELECT $2, fee_type, fee_subtype, pricing_model, rate_value, minimum_fee, maximum_fee, currency_code, tier_brackets, fee_basis, description, sequence_order
            FROM "ob-poc".deal_rate_card_lines WHERE rate_card_id = $1
            "#,
        )
        .bind(rate_card_id)
        .bind(new_rate_card_id)
        .execute(scope.executor())
        .await?;

        sqlx::query(
            r#"UPDATE "ob-poc".deal_rate_cards SET status = 'SUPERSEDED', superseded_by = $2, updated_at = NOW() WHERE rate_card_id = $1"#,
        )
        .bind(rate_card_id)
        .bind(new_rate_card_id)
        .execute(scope.executor())
        .await?;

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".deal_events (deal_id, event_type, subject_type, subject_id, old_value, new_value, description)
            VALUES ($1, 'RATE_CARD_COUNTERED', 'RATE_CARD', $2, $3::text, $4::text, 'Counter-offer created')
            "#,
        )
        .bind(deal_id)
        .bind(new_rate_card_id)
        .bind(rate_card_id.to_string())
        .bind(new_rate_card_id.to_string())
        .execute(scope.executor())
        .await?;

        Ok(VerbExecutionOutcome::Uuid(new_rate_card_id))
    }
}

// =============================================================================
// deal.agree-rate-card
// =============================================================================

/// Finalise rate card — both parties agree.
pub struct AgreeRateCard;

#[async_trait]
impl SemOsVerbOp for AgreeRateCard {
    fn fqn(&self) -> &str {
        "deal.agree-rate-card"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let rate_card_id = json_extract_uuid(args, ctx, "rate-card-id")?;

        let row: (String, Uuid) = sqlx::query_as(
            r#"SELECT status, deal_id FROM "ob-poc".deal_rate_cards WHERE rate_card_id = $1"#,
        )
        .bind(rate_card_id)
        .fetch_one(scope.executor())
        .await?;

        let (status, deal_id) = row;

        if !matches!(status.as_str(), "PROPOSED" | "COUNTER_OFFERED") {
            return Err(anyhow!("Cannot agree rate card in status {}", status));
        }

        sqlx::query(
            r#"UPDATE "ob-poc".deal_rate_cards SET status = 'AGREED', updated_at = NOW() WHERE rate_card_id = $1"#,
        )
        .bind(rate_card_id)
        .execute(scope.executor())
        .await?;

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".deal_events (deal_id, event_type, subject_type, subject_id, old_value, new_value)
            VALUES ($1, 'RATE_CARD_AGREED', 'RATE_CARD', $2, $3, 'AGREED')
            "#,
        )
        .bind(deal_id)
        .bind(rate_card_id)
        .bind(&status)
        .execute(scope.executor())
        .await?;

        Ok(VerbExecutionOutcome::Affected(1))
    }
}

// =============================================================================
// deal.add-sla
// =============================================================================

/// Add an SLA to a deal.
pub struct AddSla;

#[async_trait]
impl SemOsVerbOp for AddSla {
    fn fqn(&self) -> &str {
        "deal.add-sla"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let deal_id = json_extract_uuid(args, ctx, "deal-id")?;
        let contract_id = json_extract_uuid_opt(args, ctx, "contract-id");
        let product_id = json_extract_uuid_opt(args, ctx, "product-id");
        let service_id = json_extract_uuid_opt(args, ctx, "service-id");
        let sla_name = json_extract_string(args, "sla-name")?;
        let sla_type = json_extract_string_opt(args, "sla-type");
        let metric_name = json_extract_string(args, "metric-name")?;
        let target_value = json_extract_string(args, "target-value")?;
        let measurement_unit = json_extract_string_opt(args, "measurement-unit");
        let penalty_type = json_extract_string_opt(args, "penalty-type");
        let penalty_value: Option<f64> = args.get("penalty-value").and_then(|v| v.as_f64());
        let effective_from = json_extract_string(args, "effective-from")?;

        let sla_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO "ob-poc".deal_slas (
                deal_id, contract_id, product_id, service_id,
                sla_name, sla_type, metric_name, target_value, measurement_unit,
                penalty_type, penalty_value, effective_from
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12::date)
            RETURNING sla_id
            "#,
        )
        .bind(deal_id)
        .bind(contract_id)
        .bind(product_id)
        .bind(service_id)
        .bind(&sla_name)
        .bind(&sla_type)
        .bind(&metric_name)
        .bind(&target_value)
        .bind(&measurement_unit)
        .bind(&penalty_type)
        .bind(penalty_value)
        .bind(&effective_from)
        .fetch_one(scope.executor())
        .await?;

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".deal_events (deal_id, event_type, subject_type, subject_id, description)
            VALUES ($1, 'SLA_ADDED', 'SLA', $2, $3)
            "#,
        )
        .bind(deal_id)
        .bind(sla_id)
        .bind(&sla_name)
        .execute(scope.executor())
        .await?;

        Ok(VerbExecutionOutcome::Uuid(sla_id))
    }
}

// =============================================================================
// deal.add-document
// =============================================================================

/// Add a document to a deal.
pub struct AddDocument;

#[async_trait]
impl SemOsVerbOp for AddDocument {
    fn fqn(&self) -> &str {
        "deal.add-document"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let deal_id = json_extract_uuid(args, ctx, "deal-id")?;
        let document_id = json_extract_uuid(args, ctx, "document-id")?;
        let document_type = json_extract_string(args, "document-type")?;
        let document_status =
            json_extract_string_opt(args, "document-status").unwrap_or_else(|| "DRAFT".to_string());

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".deal_documents (deal_id, document_id, document_type, document_status)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (deal_id, document_id) DO UPDATE SET document_status = EXCLUDED.document_status
            "#,
        )
        .bind(deal_id)
        .bind(document_id)
        .bind(&document_type)
        .bind(&document_status)
        .execute(scope.executor())
        .await?;

        Ok(VerbExecutionOutcome::Affected(1))
    }
}

// =============================================================================
// deal.update-document-status
// =============================================================================

/// Update document status.
pub struct UpdateDocumentStatus;

#[async_trait]
impl SemOsVerbOp for UpdateDocumentStatus {
    fn fqn(&self) -> &str {
        "deal.update-document-status"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let deal_id = json_extract_uuid(args, ctx, "deal-id")?;
        let document_id = json_extract_uuid(args, ctx, "document-id")?;
        let document_status = json_extract_string(args, "document-status")?;

        let result = sqlx::query(
            r#"UPDATE "ob-poc".deal_documents SET document_status = $3 WHERE deal_id = $1 AND document_id = $2"#,
        )
        .bind(deal_id)
        .bind(document_id)
        .bind(&document_status)
        .execute(scope.executor())
        .await?;

        Ok(VerbExecutionOutcome::Affected(result.rows_affected()))
    }
}

// =============================================================================
// deal.add-ubo-assessment
// =============================================================================

/// Add UBO assessment to deal.
pub struct AddUboAssessment;

#[async_trait]
impl SemOsVerbOp for AddUboAssessment {
    fn fqn(&self) -> &str {
        "deal.add-ubo-assessment"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let deal_id = json_extract_uuid(args, ctx, "deal-id")?;
        let entity_id = json_extract_uuid(args, ctx, "entity-id")?;
        let kyc_case_id = json_extract_uuid_opt(args, ctx, "kyc-case-id");

        let assessment_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO "ob-poc".deal_ubo_assessments (deal_id, entity_id, kyc_case_id)
            VALUES ($1, $2, $3)
            RETURNING assessment_id
            "#,
        )
        .bind(deal_id)
        .bind(entity_id)
        .bind(kyc_case_id)
        .fetch_one(scope.executor())
        .await?;

        Ok(VerbExecutionOutcome::Uuid(assessment_id))
    }
}

// =============================================================================
// deal.update-ubo-assessment
// =============================================================================

/// Update UBO assessment status.
pub struct UpdateUboAssessment;

#[async_trait]
impl SemOsVerbOp for UpdateUboAssessment {
    fn fqn(&self) -> &str {
        "deal.update-ubo-assessment"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let assessment_id = json_extract_uuid(args, ctx, "assessment-id")?;
        let assessment_status = json_extract_string_opt(args, "assessment-status");
        let risk_rating = json_extract_string_opt(args, "risk-rating");

        let result = sqlx::query(
            r#"
            UPDATE "ob-poc".deal_ubo_assessments
            SET
                assessment_status = COALESCE($2, assessment_status),
                risk_rating = COALESCE($3, risk_rating),
                completed_at = CASE WHEN $2 = 'COMPLETED' THEN NOW() ELSE completed_at END,
                updated_at = NOW()
            WHERE assessment_id = $1
            "#,
        )
        .bind(assessment_id)
        .bind(&assessment_status)
        .bind(&risk_rating)
        .execute(scope.executor())
        .await?;

        Ok(VerbExecutionOutcome::Affected(result.rows_affected()))
    }
}

// =============================================================================
// deal.request-onboarding
// =============================================================================

/// Create onboarding request.
pub struct RequestOnboarding;

#[async_trait]
impl SemOsVerbOp for RequestOnboarding {
    fn fqn(&self) -> &str {
        "deal.request-onboarding"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let deal_id = json_extract_uuid(args, ctx, "deal-id")?;
        let contract_id = json_extract_uuid(args, ctx, "contract-id")?;
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let product_id = json_extract_uuid(args, ctx, "product-id")?;
        let requires_kyc = json_extract_bool_opt(args, "requires-kyc").unwrap_or(true);
        let target_live_date = json_extract_string_opt(args, "target-live-date");
        let requested_by = json_extract_string_opt(args, "requested-by");
        let notes = json_extract_string_opt(args, "notes");

        let deal_status: String =
            sqlx::query_scalar(r#"SELECT deal_status FROM "ob-poc".deals WHERE deal_id = $1"#)
                .bind(deal_id)
                .fetch_one(scope.executor())
                .await?;

        if !matches!(deal_status.as_str(), "CONTRACTED" | "ONBOARDING") {
            return Err(anyhow!(
                "Deal must be in CONTRACTED or ONBOARDING status to request onboarding"
            ));
        }

        let contract_linked: bool = sqlx::query_scalar(
            r#"SELECT EXISTS(SELECT 1 FROM "ob-poc".deal_contracts WHERE deal_id = $1 AND contract_id = $2)"#,
        )
        .bind(deal_id)
        .bind(contract_id)
        .fetch_one(scope.executor())
        .await?;

        if !contract_linked {
            return Err(anyhow!("Contract is not linked to this deal"));
        }

        if !deal_has_group_approved_kyc_clearance(scope, deal_id).await? {
            return Err(anyhow!(
                "Deal onboarding requires APPROVED KYC clearance for the primary client group"
            ));
        }

        if !deal_controls_cbu(scope, deal_id, cbu_id).await? {
            return Err(anyhow!(
                "CBU must already be linked to the deal's primary client group before onboarding can be requested"
            ));
        }

        let request_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO "ob-poc".deal_onboarding_requests (
                deal_id, contract_id, cbu_id, product_id,
                requires_kyc, target_live_date, requested_by, notes
            )
            VALUES ($1, $2, $3, $4, $5, $6::date, $7, $8)
            RETURNING request_id
            "#,
        )
        .bind(deal_id)
        .bind(contract_id)
        .bind(cbu_id)
        .bind(product_id)
        .bind(requires_kyc)
        .bind(&target_live_date)
        .bind(&requested_by)
        .bind(&notes)
        .fetch_one(scope.executor())
        .await?;

        if deal_status == "CONTRACTED" {
            sqlx::query(r#"UPDATE "ob-poc".deals SET deal_status = 'ONBOARDING', updated_at = NOW() WHERE deal_id = $1"#)
                .bind(deal_id)
                .execute(scope.executor())
                .await?;
        }

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".deal_events (deal_id, event_type, subject_type, subject_id, description, actor)
            VALUES ($1, 'ONBOARDING_REQUESTED', 'ONBOARDING_REQUEST', $2, $3, $4)
            "#,
        )
        .bind(deal_id)
        .bind(request_id)
        .bind(format!("CBU {} for product", cbu_id))
        .bind(&requested_by)
        .execute(scope.executor())
        .await?;

        Ok(VerbExecutionOutcome::Uuid(request_id))
    }
}

// =============================================================================
// deal.request-onboarding-batch
// =============================================================================

/// Batch onboarding request.
pub struct RequestOnboardingBatch;

#[async_trait]
impl SemOsVerbOp for RequestOnboardingBatch {
    fn fqn(&self) -> &str {
        "deal.request-onboarding-batch"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let deal_id = json_extract_uuid(args, ctx, "deal-id")?;
        let contract_id = json_extract_uuid(args, ctx, "contract-id")?;
        let requires_kyc = json_extract_bool_opt(args, "requires-kyc").unwrap_or(true);
        let requested_by = json_extract_string_opt(args, "requested-by");

        let deal_status: String =
            sqlx::query_scalar(r#"SELECT deal_status FROM "ob-poc".deals WHERE deal_id = $1"#)
                .bind(deal_id)
                .fetch_one(scope.executor())
                .await?;

        if !matches!(deal_status.as_str(), "CONTRACTED" | "ONBOARDING") {
            return Err(anyhow!(
                "Deal must be in CONTRACTED or ONBOARDING status to request onboarding"
            ));
        }

        let contract_linked: bool = sqlx::query_scalar(
            r#"SELECT EXISTS(SELECT 1 FROM "ob-poc".deal_contracts WHERE deal_id = $1 AND contract_id = $2)"#,
        )
        .bind(deal_id)
        .bind(contract_id)
        .fetch_one(scope.executor())
        .await?;

        if !contract_linked {
            return Err(anyhow!("Contract is not linked to this deal"));
        }

        if !deal_has_group_approved_kyc_clearance(scope, deal_id).await? {
            return Err(anyhow!(
                "Deal onboarding requires APPROVED KYC clearance for the primary client group"
            ));
        }

        let requests_arg = args
            .get("requests")
            .ok_or_else(|| anyhow!("requests argument is required"))?;

        let requests: Vec<Value> = serde_json::from_value(requests_arg.clone()).unwrap_or_default();

        let mut request_ids = Vec::new();

        for req in requests {
            let cbu_id: Uuid = req
                .get("cbu-id")
                .or_else(|| req.get("cbu_id"))
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok())
                .ok_or_else(|| anyhow!("cbu-id required in each request"))?;

            let product_id: Uuid = req
                .get("product-id")
                .or_else(|| req.get("product_id"))
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok())
                .ok_or_else(|| anyhow!("product-id required in each request"))?;

            if !deal_controls_cbu(scope, deal_id, cbu_id).await? {
                return Err(anyhow!(
                    "CBU {} is not linked to the deal's primary client group",
                    cbu_id
                ));
            }

            let target_live_date = req
                .get("target-live-date")
                .or_else(|| req.get("target_live_date"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            let request_id: Uuid = sqlx::query_scalar(
                r#"
                INSERT INTO "ob-poc".deal_onboarding_requests (
                    deal_id, contract_id, cbu_id, product_id, requires_kyc, target_live_date, requested_by
                )
                VALUES ($1, $2, $3, $4, $5, $6::date, $7)
                RETURNING request_id
                "#,
            )
            .bind(deal_id)
            .bind(contract_id)
            .bind(cbu_id)
            .bind(product_id)
            .bind(requires_kyc)
            .bind(&target_live_date)
            .bind(&requested_by)
            .fetch_one(scope.executor())
            .await?;

            request_ids.push(request_id);
        }

        if deal_status == "CONTRACTED" {
            sqlx::query(
                r#"UPDATE "ob-poc".deals SET deal_status = 'ONBOARDING', updated_at = NOW() WHERE deal_id = $1"#,
            )
            .bind(deal_id)
            .execute(scope.executor())
            .await?;
        }

        let result = BatchOnboardingResult {
            request_ids: request_ids.clone(),
            count: request_ids.len() as i32,
        };

        Ok(VerbExecutionOutcome::Record(serde_json::to_value(result)?))
    }
}

// =============================================================================
// deal.update-onboarding-status
// =============================================================================

/// Update onboarding request status.
pub struct UpdateOnboardingStatus;

#[async_trait]
impl SemOsVerbOp for UpdateOnboardingStatus {
    fn fqn(&self) -> &str {
        "deal.update-onboarding-status"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let request_id = json_extract_uuid(args, ctx, "request-id")?;
        let request_status = json_extract_string(args, "request-status")?;
        let kyc_case_id = json_extract_uuid_opt(args, ctx, "kyc-case-id");

        let deal_id: Uuid = sqlx::query_scalar(
            r#"SELECT deal_id FROM "ob-poc".deal_onboarding_requests WHERE request_id = $1"#,
        )
        .bind(request_id)
        .fetch_one(scope.executor())
        .await?;

        sqlx::query(
            r#"
            UPDATE "ob-poc".deal_onboarding_requests
            SET
                request_status = $2,
                kyc_case_id = COALESCE($3, kyc_case_id),
                kyc_cleared_at = CASE WHEN $2 = 'KYC_CLEARED' THEN NOW() ELSE kyc_cleared_at END,
                completed_at = CASE WHEN $2 = 'COMPLETED' THEN NOW() ELSE completed_at END,
                updated_at = NOW()
            WHERE request_id = $1
            "#,
        )
        .bind(request_id)
        .bind(&request_status)
        .bind(kyc_case_id)
        .execute(scope.executor())
        .await?;

        if request_status == "COMPLETED" {
            let pending_count: i64 = sqlx::query_scalar(
                r#"
                SELECT COUNT(*) FROM "ob-poc".deal_onboarding_requests
                WHERE deal_id = $1 AND request_status NOT IN ('COMPLETED', 'CANCELLED')
                "#,
            )
            .bind(deal_id)
            .fetch_one(scope.executor())
            .await?;

            if pending_count == 0 {
                sqlx::query(
                    r#"UPDATE "ob-poc".deals SET deal_status = 'ACTIVE', active_at = NOW(), updated_at = NOW() WHERE deal_id = $1 AND deal_status = 'ONBOARDING'"#,
                )
                .bind(deal_id)
                .execute(scope.executor())
                .await?;

                sqlx::query(
                    r#"
                    INSERT INTO "ob-poc".deal_events (deal_id, event_type, subject_type, subject_id, new_value, description)
                    VALUES ($1, 'STATUS_CHANGED', 'DEAL', $1, 'ACTIVE', 'All onboarding requests completed')
                    "#,
                )
                .bind(deal_id)
                .execute(scope.executor())
                .await?;
            }
        }

        Ok(VerbExecutionOutcome::Affected(1))
    }
}

// =============================================================================
// deal.read-summary
// =============================================================================

/// Full deal summary with counts.
pub struct ReadSummary;

#[async_trait]
impl SemOsVerbOp for ReadSummary {
    fn fqn(&self) -> &str {
        "deal.read-summary"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let deal_id = json_extract_uuid(args, ctx, "deal-id")?;

        let deal: Option<(
            Uuid,
            String,
            Option<String>,
            String,
            Option<String>,
            Option<f64>,
        )> = sqlx::query_as(
            r#"
            SELECT deal_id, deal_name, deal_reference, deal_status, sales_owner, estimated_revenue::float8
            FROM "ob-poc".deals WHERE deal_id = $1
            "#,
        )
        .bind(deal_id)
        .fetch_optional(scope.executor())
        .await?;

        let deal_info = match deal {
            Some((id, name, ref_, status, owner, rev)) => serde_json::json!({
                "deal_id": id,
                "deal_name": name,
                "deal_reference": ref_,
                "deal_status": status,
                "sales_owner": owner,
                "estimated_revenue": rev
            }),
            None => return Err(anyhow!("Deal not found")),
        };

        let participant_count: i64 = sqlx::query_scalar(
            r#"SELECT COUNT(*) FROM "ob-poc".deal_participants WHERE deal_id = $1"#,
        )
        .bind(deal_id)
        .fetch_one(scope.executor())
        .await?;

        let contract_count: i64 = sqlx::query_scalar(
            r#"SELECT COUNT(*) FROM "ob-poc".deal_contracts WHERE deal_id = $1"#,
        )
        .bind(deal_id)
        .fetch_one(scope.executor())
        .await?;

        let rate_card_count: i64 = sqlx::query_scalar(
            r#"SELECT COUNT(*) FROM "ob-poc".deal_rate_cards WHERE deal_id = $1"#,
        )
        .bind(deal_id)
        .fetch_one(scope.executor())
        .await?;

        let onboarding_stats: (i64, i64) = sqlx::query_as(
            r#"
            SELECT
                COUNT(*) FILTER (WHERE request_status = 'COMPLETED'),
                COUNT(*)
            FROM "ob-poc".deal_onboarding_requests WHERE deal_id = $1
            "#,
        )
        .bind(deal_id)
        .fetch_one(scope.executor())
        .await?;

        let result = serde_json::json!({
            "deal": deal_info,
            "participant_count": participant_count,
            "contract_count": contract_count,
            "rate_card_count": rate_card_count,
            "onboarding_completed": onboarding_stats.0,
            "onboarding_total": onboarding_stats.1
        });

        Ok(VerbExecutionOutcome::Record(result))
    }
}

// ---------------------------------------------------------------------------
// deal.update-kyc-clearance
// ---------------------------------------------------------------------------
//
// Preserving update on `deals.kyc_clearance_status` — the parallel
// substate under IN_CLEARANCE that the KYC workspace propagates back
// to the deal when a case approval/rejection lands. Constraint
// `deals_kyc_clearance_status_check` (migration
// `20260429_carrier_08_deals_in_clearance_substates.sql`) caps the
// value to `pending|in_review|approved|rejected`.

const KYC_CLEARANCE_STATUSES: &[&str] = &["pending", "in_review", "approved", "rejected"];

pub struct UpdateKycClearance;

#[async_trait]
impl SemOsVerbOp for UpdateKycClearance {
    fn fqn(&self) -> &str {
        "deal.update-kyc-clearance"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let deal_id = json_extract_uuid(args, ctx, "deal-id")?;
        let target = json_extract_string(args, "kyc-clearance-status")?;

        if !KYC_CLEARANCE_STATUSES.contains(&target.as_str()) {
            return Err(anyhow!(
                "Invalid kyc-clearance-status '{}'. Must be one of: {}",
                target,
                KYC_CLEARANCE_STATUSES.join(", ")
            ));
        }

        let affected = sqlx::query(
            r#"UPDATE "ob-poc".deals
               SET kyc_clearance_status = $2,
                   updated_at = NOW()
               WHERE deal_id = $1"#,
        )
        .bind(deal_id)
        .bind(&target)
        .execute(scope.executor())
        .await?
        .rows_affected();

        if affected == 0 {
            return Err(anyhow!("Deal not found: {}", deal_id));
        }

        Ok(VerbExecutionOutcome::Affected(affected))
    }
}
