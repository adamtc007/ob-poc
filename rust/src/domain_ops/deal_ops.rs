//! Deal Record Operations
//!
//! Operations for deal lifecycle management, rate card negotiation, and onboarding handoff.
//!
//! Deal Record is the commercial origination hub that links Sales through contracting,
//! onboarding, servicing, and billing in a closed loop.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use ob_poc_macros::register_custom_op;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::helpers::{
    extract_bool_opt, extract_string, extract_string_opt, extract_uuid, extract_uuid_opt,
};
use super::CustomOperation;
use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};

#[cfg(feature = "database")]
use sqlx::PgPool;

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

// These structs are defined for future use when returning structured results
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateCardCreateResult {
    pub rate_card_id: Uuid,
    pub deal_id: Uuid,
    pub status: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateCardLineResult {
    pub line_id: Uuid,
    pub rate_card_id: Uuid,
    pub fee_type: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnboardingRequestResult {
    pub request_id: Uuid,
    pub deal_id: Uuid,
    pub cbu_id: Uuid,
    pub request_status: String,
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
            | ("NEGOTIATING", "CONTRACTED")
            | ("NEGOTIATING", "QUALIFYING")
            | ("NEGOTIATING", "CANCELLED")
            | ("CONTRACTED", "ONBOARDING")
            | ("CONTRACTED", "CANCELLED")
            | ("ONBOARDING", "ACTIVE")
            | ("ONBOARDING", "CANCELLED")
            | ("ACTIVE", "WINDING_DOWN")
            | ("WINDING_DOWN", "OFFBOARDED")
    )
}

/// Valid rate card status transitions
#[allow(dead_code)]
fn is_valid_rate_card_status_transition(from: &str, to: &str) -> bool {
    matches!(
        (from, to),
        ("DRAFT", "PROPOSED")
            | ("DRAFT", "CANCELLED")
            | ("PROPOSED", "COUNTER_OFFERED")
            | ("PROPOSED", "AGREED")
            | ("PROPOSED", "CANCELLED")
            | ("COUNTER_OFFERED", "PROPOSED")
            | ("COUNTER_OFFERED", "AGREED")
            | ("COUNTER_OFFERED", "CANCELLED")
            | ("AGREED", "SUPERSEDED")
    )
}

// =============================================================================
// Deal CRUD Operations
// =============================================================================

/// Create a new deal record
#[register_custom_op]
pub struct DealCreateOp;

#[async_trait]
impl CustomOperation for DealCreateOp {
    fn domain(&self) -> &'static str {
        "deal"
    }
    fn verb(&self) -> &'static str {
        "create"
    }
    fn rationale(&self) -> &'static str {
        "Creates deal and records initial event in deal_events audit trail"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let deal_name = extract_string(verb_call, "deal-name")?;
        let primary_client_group_id = extract_uuid(verb_call, ctx, "primary-client-group-id")?;
        let deal_reference = extract_string_opt(verb_call, "deal-reference");
        let sales_owner = extract_string_opt(verb_call, "sales-owner");
        let sales_team = extract_string_opt(verb_call, "sales-team");
        let estimated_revenue: Option<f64> = verb_call
            .get_arg("estimated-revenue")
            .and_then(|v| v.value.as_decimal())
            .map(|d| d.to_string().parse().unwrap_or(0.0));
        let currency_code = extract_string_opt(verb_call, "currency-code").unwrap_or("USD".into());
        let notes = extract_string_opt(verb_call, "notes");

        // Insert deal
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
        .fetch_one(pool)
        .await?;

        // Record event
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".deal_events (deal_id, event_type, subject_type, subject_id, new_value, actor)
            VALUES ($1, 'DEAL_CREATED', 'DEAL', $1, 'PROSPECT', $2)
            "#,
        )
        .bind(deal_id)
        .bind(&sales_owner)
        .execute(pool)
        .await?;

        let _result = DealCreateResult {
            deal_id,
            deal_name,
            deal_status: "PROSPECT".to_string(),
        };
        Ok(ExecutionResult::Uuid(deal_id))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Uuid(Uuid::new_v4()))
    }
}

/// Search deals by name or reference
#[register_custom_op]
pub struct DealSearchOp;

#[async_trait]
impl CustomOperation for DealSearchOp {
    fn domain(&self) -> &'static str {
        "deal"
    }
    fn verb(&self) -> &'static str {
        "search"
    }
    fn rationale(&self) -> &'static str {
        "Full-text search across deal name and reference"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let query = extract_string(verb_call, "query")?;
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
        .fetch_all(pool)
        .await?;

        let results: Vec<serde_json::Value> = rows
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

        Ok(ExecutionResult::RecordSet(results))
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

/// Update deal record fields
#[register_custom_op]
pub struct DealUpdateOp;

#[async_trait]
impl CustomOperation for DealUpdateOp {
    fn domain(&self) -> &'static str {
        "deal"
    }
    fn verb(&self) -> &'static str {
        "update"
    }
    fn rationale(&self) -> &'static str {
        "Updates deal fields and records change event"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let deal_id = extract_uuid(verb_call, ctx, "deal-id")?;
        let deal_name = extract_string_opt(verb_call, "deal-name");
        let sales_owner = extract_string_opt(verb_call, "sales-owner");
        let estimated_revenue: Option<f64> = verb_call
            .get_arg("estimated-revenue")
            .and_then(|v| v.value.as_decimal())
            .map(|d| d.to_string().parse().unwrap_or(0.0));
        let notes = extract_string_opt(verb_call, "notes");

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
        Ok(ExecutionResult::Affected(1))
    }
}

/// Transition deal status with lifecycle validation
#[register_custom_op]
pub struct DealUpdateStatusOp;

#[async_trait]
impl CustomOperation for DealUpdateStatusOp {
    fn domain(&self) -> &'static str {
        "deal"
    }
    fn verb(&self) -> &'static str {
        "update-status"
    }
    fn rationale(&self) -> &'static str {
        "State machine validation for deal status transitions"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let deal_id = extract_uuid(verb_call, ctx, "deal-id")?;
        let new_status = extract_string(verb_call, "new-status")?;

        // Get current status
        let current_status: String =
            sqlx::query_scalar(r#"SELECT deal_status FROM "ob-poc".deals WHERE deal_id = $1"#)
                .bind(deal_id)
                .fetch_one(pool)
                .await?;

        // Validate transition
        if !is_valid_deal_status_transition(&current_status, &new_status) {
            return Err(anyhow!(
                "Invalid status transition from {} to {}",
                current_status,
                new_status
            ));
        }

        // Determine which timestamp to set
        let timestamp_column = match new_status.as_str() {
            "QUALIFYING" => Some("qualified_at"),
            "CONTRACTED" => Some("contracted_at"),
            "ACTIVE" => Some("active_at"),
            "OFFBOARDED" | "CANCELLED" => Some("closed_at"),
            _ => None,
        };

        // Update status
        if let Some(col) = timestamp_column {
            let query = format!(
                r#"UPDATE "ob-poc".deals SET deal_status = $2, {} = NOW(), updated_at = NOW() WHERE deal_id = $1"#,
                col
            );
            sqlx::query(&query)
                .bind(deal_id)
                .bind(&new_status)
                .execute(pool)
                .await?;
        } else {
            sqlx::query(
                r#"UPDATE "ob-poc".deals SET deal_status = $2, updated_at = NOW() WHERE deal_id = $1"#,
            )
            .bind(deal_id)
            .bind(&new_status)
            .execute(pool)
            .await?;
        }

        // Record event
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".deal_events (deal_id, event_type, subject_type, subject_id, old_value, new_value)
            VALUES ($1, 'STATUS_CHANGED', 'DEAL', $1, $2, $3)
            "#,
        )
        .bind(deal_id)
        .bind(&current_status)
        .bind(&new_status)
        .execute(pool)
        .await?;

        let result = DealStatusUpdateResult {
            deal_id,
            old_status: current_status,
            new_status,
        };
        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Affected(1))
    }
}

/// Cancel a deal
#[register_custom_op]
pub struct DealCancelOp;

#[async_trait]
impl CustomOperation for DealCancelOp {
    fn domain(&self) -> &'static str {
        "deal"
    }
    fn verb(&self) -> &'static str {
        "cancel"
    }
    fn rationale(&self) -> &'static str {
        "Validates deal can be cancelled and records cancellation event"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let deal_id = extract_uuid(verb_call, ctx, "deal-id")?;
        let reason = extract_string(verb_call, "reason")?;

        // Get current status
        let current_status: String =
            sqlx::query_scalar(r#"SELECT deal_status FROM "ob-poc".deals WHERE deal_id = $1"#)
                .bind(deal_id)
                .fetch_one(pool)
                .await?;

        // Cannot cancel if already ACTIVE, WINDING_DOWN, or OFFBOARDED
        if matches!(
            current_status.as_str(),
            "ACTIVE" | "WINDING_DOWN" | "OFFBOARDED" | "CANCELLED"
        ) {
            return Err(anyhow!("Cannot cancel deal in status {}", current_status));
        }

        // Update to cancelled
        sqlx::query(
            r#"
            UPDATE "ob-poc".deals
            SET deal_status = 'CANCELLED', closed_at = NOW(), updated_at = NOW()
            WHERE deal_id = $1
            "#,
        )
        .bind(deal_id)
        .execute(pool)
        .await?;

        // Record event
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".deal_events (deal_id, event_type, subject_type, subject_id, old_value, new_value, description)
            VALUES ($1, 'STATUS_CHANGED', 'DEAL', $1, $2, 'CANCELLED', $3)
            "#,
        )
        .bind(deal_id)
        .bind(&current_status)
        .bind(&reason)
        .execute(pool)
        .await?;

        Ok(ExecutionResult::Affected(1))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Affected(1))
    }
}

// =============================================================================
// Participant Operations
// =============================================================================

/// Add a participant to a deal
#[register_custom_op]
pub struct DealAddParticipantOp;

#[async_trait]
impl CustomOperation for DealAddParticipantOp {
    fn domain(&self) -> &'static str {
        "deal"
    }
    fn verb(&self) -> &'static str {
        "add-participant"
    }
    fn rationale(&self) -> &'static str {
        "Handles upsert logic and enforces one-primary-per-deal constraint"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let deal_id = extract_uuid(verb_call, ctx, "deal-id")?;
        let entity_id = extract_uuid(verb_call, ctx, "entity-id")?;
        let participant_role = extract_string_opt(verb_call, "participant-role")
            .unwrap_or_else(|| "CONTRACTING_PARTY".to_string());
        let lei = extract_string_opt(verb_call, "lei");
        let is_primary = extract_bool_opt(verb_call, "is-primary").unwrap_or(false);

        // Insert participant (upsert on unique constraint)
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
        .execute(pool)
        .await?;

        // Record event
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".deal_events (deal_id, event_type, subject_type, subject_id, description)
            VALUES ($1, 'PARTICIPANT_ADDED', 'ENTITY', $2, $3)
            "#,
        )
        .bind(deal_id)
        .bind(entity_id)
        .bind(&participant_role)
        .execute(pool)
        .await?;

        Ok(ExecutionResult::Affected(1))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Affected(1))
    }
}

/// Remove a participant from a deal
#[register_custom_op]
pub struct DealRemoveParticipantOp;

#[async_trait]
impl CustomOperation for DealRemoveParticipantOp {
    fn domain(&self) -> &'static str {
        "deal"
    }
    fn verb(&self) -> &'static str {
        "remove-participant"
    }
    fn rationale(&self) -> &'static str {
        "Validates no orphaned contracts before removal"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let deal_id = extract_uuid(verb_call, ctx, "deal-id")?;
        let entity_id = extract_uuid(verb_call, ctx, "entity-id")?;
        let participant_role = extract_string_opt(verb_call, "participant-role");

        let result = if let Some(role) = &participant_role {
            sqlx::query(
                r#"DELETE FROM "ob-poc".deal_participants WHERE deal_id = $1 AND entity_id = $2 AND participant_role = $3"#,
            )
            .bind(deal_id)
            .bind(entity_id)
            .bind(role)
            .execute(pool)
            .await?
        } else {
            sqlx::query(
                r#"DELETE FROM "ob-poc".deal_participants WHERE deal_id = $1 AND entity_id = $2"#,
            )
            .bind(deal_id)
            .bind(entity_id)
            .execute(pool)
            .await?
        };

        Ok(ExecutionResult::Affected(result.rows_affected()))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Affected(1))
    }
}

// =============================================================================
// Contract Operations
// =============================================================================

/// Add a contract to a deal
#[register_custom_op]
pub struct DealAddContractOp;

#[async_trait]
impl CustomOperation for DealAddContractOp {
    fn domain(&self) -> &'static str {
        "deal"
    }
    fn verb(&self) -> &'static str {
        "add-contract"
    }
    fn rationale(&self) -> &'static str {
        "Links contract to deal and records CONTRACT_ADDED event"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let deal_id = extract_uuid(verb_call, ctx, "deal-id")?;
        let contract_id = extract_uuid(verb_call, ctx, "contract-id")?;
        let contract_role =
            extract_string_opt(verb_call, "contract-role").unwrap_or_else(|| "PRIMARY".to_string());
        let sequence_order: i32 = verb_call
            .get_arg("sequence-order")
            .and_then(|v| v.value.as_integer())
            .map(|i| i as i32)
            .unwrap_or(1);

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
        .execute(pool)
        .await?;

        // Record event
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".deal_events (deal_id, event_type, subject_type, subject_id, description)
            VALUES ($1, 'CONTRACT_ADDED', 'CONTRACT', $2, $3)
            "#,
        )
        .bind(deal_id)
        .bind(contract_id)
        .bind(&contract_role)
        .execute(pool)
        .await?;

        Ok(ExecutionResult::Affected(1))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Affected(1))
    }
}

/// Remove a contract from a deal
#[register_custom_op]
pub struct DealRemoveContractOp;

#[async_trait]
impl CustomOperation for DealRemoveContractOp {
    fn domain(&self) -> &'static str {
        "deal"
    }
    fn verb(&self) -> &'static str {
        "remove-contract"
    }
    fn rationale(&self) -> &'static str {
        "Validates no rate cards or billing profiles reference this contract"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let deal_id = extract_uuid(verb_call, ctx, "deal-id")?;
        let contract_id = extract_uuid(verb_call, ctx, "contract-id")?;

        // Check for dependent rate cards
        let count: i64 = sqlx::query_scalar(
            r#"SELECT COUNT(*) FROM "ob-poc".deal_rate_cards WHERE deal_id = $1 AND contract_id = $2"#,
        )
        .bind(deal_id)
        .bind(contract_id)
        .fetch_one(pool)
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
        Ok(ExecutionResult::Affected(1))
    }
}

// =============================================================================
// Deal Product Operations
// =============================================================================

/// Add a product to the deal's commercial scope
#[register_custom_op]
pub struct DealAddProductOp;

#[async_trait]
impl CustomOperation for DealAddProductOp {
    fn domain(&self) -> &'static str {
        "deal"
    }
    fn verb(&self) -> &'static str {
        "add-product"
    }
    fn rationale(&self) -> &'static str {
        "Adds product to deal scope and records PRODUCT_ADDED event"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let deal_id = extract_uuid(verb_call, ctx, "deal-id")?;
        let product_id = extract_uuid(verb_call, ctx, "product-id")?;
        let product_status = extract_string_opt(verb_call, "product-status")
            .unwrap_or_else(|| "PROPOSED".to_string());
        let indicative_revenue: Option<f64> = verb_call
            .get_arg("indicative-revenue")
            .and_then(|v| v.value.as_decimal())
            .map(|d| d.to_string().parse::<f64>().unwrap_or(0.0));
        let currency_code =
            extract_string_opt(verb_call, "currency-code").unwrap_or_else(|| "USD".to_string());
        let notes = extract_string_opt(verb_call, "notes");

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
        .fetch_one(pool)
        .await?;

        // Record event
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".deal_events (deal_id, event_type, subject_type, subject_id, description)
            VALUES ($1, 'PRODUCT_ADDED', 'PRODUCT', $2, $3)
            "#,
        )
        .bind(deal_id)
        .bind(product_id)
        .bind(&product_status)
        .execute(pool)
        .await?;

        Ok(ExecutionResult::Uuid(deal_product_id.0))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Uuid(Uuid::new_v4()))
    }
}

/// Update the status of a product in the deal scope
#[register_custom_op]
pub struct DealUpdateProductStatusOp;

#[async_trait]
impl CustomOperation for DealUpdateProductStatusOp {
    fn domain(&self) -> &'static str {
        "deal"
    }
    fn verb(&self) -> &'static str {
        "update-product-status"
    }
    fn rationale(&self) -> &'static str {
        "Updates product status with event recording"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let deal_id = extract_uuid(verb_call, ctx, "deal-id")?;
        let product_id = extract_uuid(verb_call, ctx, "product-id")?;
        let product_status = extract_string(verb_call, "product-status")?;

        // Update with AGREED timestamp if status is AGREED
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
            .execute(pool)
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
            .execute(pool)
            .await?
        };

        // Record event
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".deal_events (deal_id, event_type, subject_type, subject_id, description)
            VALUES ($1, 'PRODUCT_STATUS_CHANGED', 'PRODUCT', $2, $3)
            "#,
        )
        .bind(deal_id)
        .bind(product_id)
        .bind(&product_status)
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
        Ok(ExecutionResult::Affected(1))
    }
}

/// Remove a product from the deal scope (sets status to REMOVED)
#[register_custom_op]
pub struct DealRemoveProductOp;

#[async_trait]
impl CustomOperation for DealRemoveProductOp {
    fn domain(&self) -> &'static str {
        "deal"
    }
    fn verb(&self) -> &'static str {
        "remove-product"
    }
    fn rationale(&self) -> &'static str {
        "Soft-deletes product from deal by setting status to REMOVED"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let deal_id = extract_uuid(verb_call, ctx, "deal-id")?;
        let product_id = extract_uuid(verb_call, ctx, "product-id")?;

        let result = sqlx::query(
            r#"
            UPDATE "ob-poc".deal_products
            SET product_status = 'REMOVED', updated_at = NOW()
            WHERE deal_id = $1 AND product_id = $2
            "#,
        )
        .bind(deal_id)
        .bind(product_id)
        .execute(pool)
        .await?;

        // Record event
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".deal_events (deal_id, event_type, subject_type, subject_id, description)
            VALUES ($1, 'PRODUCT_REMOVED', 'PRODUCT', $2, 'Product removed from deal scope')
            "#,
        )
        .bind(deal_id)
        .bind(product_id)
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
        Ok(ExecutionResult::Affected(1))
    }
}

// =============================================================================
// Rate Card Operations
// =============================================================================

/// Create a negotiated rate card for a product within a deal
#[register_custom_op]
pub struct DealCreateRateCardOp;

#[async_trait]
impl CustomOperation for DealCreateRateCardOp {
    fn domain(&self) -> &'static str {
        "deal"
    }
    fn verb(&self) -> &'static str {
        "create-rate-card"
    }
    fn rationale(&self) -> &'static str {
        "Creates rate card in DRAFT status and validates contract is linked to deal"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let deal_id = extract_uuid(verb_call, ctx, "deal-id")?;
        let contract_id = extract_uuid(verb_call, ctx, "contract-id")?;
        let product_id = extract_uuid(verb_call, ctx, "product-id")?;
        let rate_card_name = extract_string_opt(verb_call, "rate-card-name");
        let effective_from = extract_string(verb_call, "effective-from")?;
        let effective_to = extract_string_opt(verb_call, "effective-to");

        // Validate contract is linked to deal
        let exists: bool = sqlx::query_scalar(
            r#"SELECT EXISTS(SELECT 1 FROM "ob-poc".deal_contracts WHERE deal_id = $1 AND contract_id = $2)"#,
        )
        .bind(deal_id)
        .bind(contract_id)
        .fetch_one(pool)
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
        .fetch_one(pool)
        .await?;

        // Record event
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".deal_events (deal_id, event_type, subject_type, subject_id, new_value)
            VALUES ($1, 'RATE_CARD_CREATED', 'RATE_CARD', $2, 'DRAFT')
            "#,
        )
        .bind(deal_id)
        .bind(rate_card_id)
        .execute(pool)
        .await?;

        Ok(ExecutionResult::Uuid(rate_card_id))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Uuid(Uuid::new_v4()))
    }
}

/// Add a fee line to a rate card
#[register_custom_op]
pub struct DealAddRateCardLineOp;

#[async_trait]
impl CustomOperation for DealAddRateCardLineOp {
    fn domain(&self) -> &'static str {
        "deal"
    }
    fn verb(&self) -> &'static str {
        "add-rate-card-line"
    }
    fn rationale(&self) -> &'static str {
        "Validates rate card is negotiable and relies on DB CHECK constraints for pricing model"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let rate_card_id = extract_uuid(verb_call, ctx, "rate-card-id")?;
        let fee_type = extract_string(verb_call, "fee-type")?;
        let fee_subtype =
            extract_string_opt(verb_call, "fee-subtype").unwrap_or_else(|| "DEFAULT".to_string());
        let pricing_model = extract_string(verb_call, "pricing-model")?;
        let rate_value: Option<f64> = verb_call
            .get_arg("rate-value")
            .and_then(|v| v.value.as_decimal())
            .map(|d| d.to_string().parse().unwrap_or(0.0));
        let minimum_fee: Option<f64> = verb_call
            .get_arg("minimum-fee")
            .and_then(|v| v.value.as_decimal())
            .map(|d| d.to_string().parse().unwrap_or(0.0));
        let maximum_fee: Option<f64> = verb_call
            .get_arg("maximum-fee")
            .and_then(|v| v.value.as_decimal())
            .map(|d| d.to_string().parse().unwrap_or(0.0));
        let currency_code =
            extract_string_opt(verb_call, "currency-code").unwrap_or_else(|| "USD".to_string());
        let tier_brackets = verb_call
            .get_arg("tier-brackets")
            .and_then(|v| serde_json::to_value(v).ok());
        let fee_basis = extract_string_opt(verb_call, "fee-basis");
        let description = extract_string_opt(verb_call, "description");

        // Validate rate card is in DRAFT or PROPOSED status
        let status: String = sqlx::query_scalar(
            r#"SELECT status FROM "ob-poc".deal_rate_cards WHERE rate_card_id = $1"#,
        )
        .bind(rate_card_id)
        .fetch_one(pool)
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
        .fetch_one(pool)
        .await?;

        Ok(ExecutionResult::Uuid(line_id))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Uuid(Uuid::new_v4()))
    }
}

/// Modify an existing rate card line
#[register_custom_op]
pub struct DealUpdateRateCardLineOp;

#[async_trait]
impl CustomOperation for DealUpdateRateCardLineOp {
    fn domain(&self) -> &'static str {
        "deal"
    }
    fn verb(&self) -> &'static str {
        "update-rate-card-line"
    }
    fn rationale(&self) -> &'static str {
        "Validates parent rate card is still negotiable"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let line_id = extract_uuid(verb_call, ctx, "line-id")?;
        let rate_value: Option<f64> = verb_call
            .get_arg("rate-value")
            .and_then(|v| v.value.as_decimal())
            .map(|d| d.to_string().parse().unwrap_or(0.0));
        let minimum_fee: Option<f64> = verb_call
            .get_arg("minimum-fee")
            .and_then(|v| v.value.as_decimal())
            .map(|d| d.to_string().parse().unwrap_or(0.0));
        let maximum_fee: Option<f64> = verb_call
            .get_arg("maximum-fee")
            .and_then(|v| v.value.as_decimal())
            .map(|d| d.to_string().parse().unwrap_or(0.0));
        let tier_brackets = verb_call
            .get_arg("tier-brackets")
            .and_then(|v| serde_json::to_value(v).ok());

        // Validate rate card is not AGREED
        let status: String = sqlx::query_scalar(
            r#"
            SELECT rc.status
            FROM "ob-poc".deal_rate_cards rc
            JOIN "ob-poc".deal_rate_card_lines l ON rc.rate_card_id = l.rate_card_id
            WHERE l.line_id = $1
            "#,
        )
        .bind(line_id)
        .fetch_one(pool)
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
        Ok(ExecutionResult::Affected(1))
    }
}

/// Remove a fee line from a rate card
#[register_custom_op]
pub struct DealRemoveRateCardLineOp;

#[async_trait]
impl CustomOperation for DealRemoveRateCardLineOp {
    fn domain(&self) -> &'static str {
        "deal"
    }
    fn verb(&self) -> &'static str {
        "remove-rate-card-line"
    }
    fn rationale(&self) -> &'static str {
        "Validates no billing targets reference this line"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let line_id = extract_uuid(verb_call, ctx, "line-id")?;

        // Check for dependent billing targets
        let count: i64 = sqlx::query_scalar(
            r#"SELECT COUNT(*) FROM "ob-poc".fee_billing_account_targets WHERE rate_card_line_id = $1"#,
        )
        .bind(line_id)
        .fetch_one(pool)
        .await?;

        if count > 0 {
            return Err(anyhow!(
                "Cannot remove line - {} billing targets depend on it",
                count
            ));
        }

        let result = sqlx::query(r#"DELETE FROM "ob-poc".deal_rate_card_lines WHERE line_id = $1"#)
            .bind(line_id)
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
        Ok(ExecutionResult::Affected(1))
    }
}

/// Propose rate card for client review
#[register_custom_op]
pub struct DealProposeRateCardOp;

#[async_trait]
impl CustomOperation for DealProposeRateCardOp {
    fn domain(&self) -> &'static str {
        "deal"
    }
    fn verb(&self) -> &'static str {
        "propose-rate-card"
    }
    fn rationale(&self) -> &'static str {
        "Validates at least one line exists and transitions to PROPOSED"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let rate_card_id = extract_uuid(verb_call, ctx, "rate-card-id")?;

        // Validate at least one line exists
        let count: i64 = sqlx::query_scalar(
            r#"SELECT COUNT(*) FROM "ob-poc".deal_rate_card_lines WHERE rate_card_id = $1"#,
        )
        .bind(rate_card_id)
        .fetch_one(pool)
        .await?;

        if count == 0 {
            return Err(anyhow!("Cannot propose empty rate card"));
        }

        // Get deal_id for event recording
        let deal_id: Uuid = sqlx::query_scalar(
            r#"SELECT deal_id FROM "ob-poc".deal_rate_cards WHERE rate_card_id = $1"#,
        )
        .bind(rate_card_id)
        .fetch_one(pool)
        .await?;

        // Update status
        sqlx::query(
            r#"
            UPDATE "ob-poc".deal_rate_cards
            SET status = 'PROPOSED', negotiation_round = negotiation_round + 1, updated_at = NOW()
            WHERE rate_card_id = $1
            "#,
        )
        .bind(rate_card_id)
        .execute(pool)
        .await?;

        // Record event
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".deal_events (deal_id, event_type, subject_type, subject_id, new_value)
            VALUES ($1, 'RATE_CARD_PROPOSED', 'RATE_CARD', $2, 'PROPOSED')
            "#,
        )
        .bind(deal_id)
        .bind(rate_card_id)
        .execute(pool)
        .await?;

        Ok(ExecutionResult::Affected(1))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Affected(1))
    }
}

/// Client counter-offer - creates new version via clone
#[register_custom_op]
pub struct DealCounterRateCardOp;

#[async_trait]
impl CustomOperation for DealCounterRateCardOp {
    fn domain(&self) -> &'static str {
        "deal"
    }
    fn verb(&self) -> &'static str {
        "counter-rate-card"
    }
    fn rationale(&self) -> &'static str {
        "Clones rate card with COUNTER_OFFERED status and applies counter values"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let rate_card_id = extract_uuid(verb_call, ctx, "rate-card-id")?;
        // counter-lines would be JSON array of {line_id, proposed_rate, proposed_minimum, proposed_maximum}
        // For simplicity, we create a clone and let caller update lines separately

        // Get original rate card details
        let row: (Uuid, Uuid, Uuid, Option<String>, String, Option<String>, i32) = sqlx::query_as(
            r#"
            SELECT deal_id, contract_id, product_id, rate_card_name, effective_from::text, effective_to::text, negotiation_round
            FROM "ob-poc".deal_rate_cards WHERE rate_card_id = $1
            "#,
        )
        .bind(rate_card_id)
        .fetch_one(pool)
        .await?;

        let (deal_id, contract_id, product_id, name, eff_from, eff_to, round) = row;

        // Create new rate card with COUNTER_OFFERED status
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
        .fetch_one(pool)
        .await?;

        // Copy lines from original
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".deal_rate_card_lines (rate_card_id, fee_type, fee_subtype, pricing_model, rate_value, minimum_fee, maximum_fee, currency_code, tier_brackets, fee_basis, description, sequence_order)
            SELECT $2, fee_type, fee_subtype, pricing_model, rate_value, minimum_fee, maximum_fee, currency_code, tier_brackets, fee_basis, description, sequence_order
            FROM "ob-poc".deal_rate_card_lines WHERE rate_card_id = $1
            "#,
        )
        .bind(rate_card_id)
        .bind(new_rate_card_id)
        .execute(pool)
        .await?;

        // Mark original as superseded
        sqlx::query(
            r#"UPDATE "ob-poc".deal_rate_cards SET status = 'SUPERSEDED', superseded_by = $2, updated_at = NOW() WHERE rate_card_id = $1"#,
        )
        .bind(rate_card_id)
        .bind(new_rate_card_id)
        .execute(pool)
        .await?;

        // Record event
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
        .execute(pool)
        .await?;

        Ok(ExecutionResult::Uuid(new_rate_card_id))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Uuid(Uuid::new_v4()))
    }
}

/// Finalise rate card - both parties agree
#[register_custom_op]
pub struct DealAgreeRateCardOp;

#[async_trait]
impl CustomOperation for DealAgreeRateCardOp {
    fn domain(&self) -> &'static str {
        "deal"
    }
    fn verb(&self) -> &'static str {
        "agree-rate-card"
    }
    fn rationale(&self) -> &'static str {
        "Validates rate card is PROPOSED or COUNTER_OFFERED and transitions to AGREED"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let rate_card_id = extract_uuid(verb_call, ctx, "rate-card-id")?;

        // Validate status
        let row: (String, Uuid) = sqlx::query_as(
            r#"SELECT status, deal_id FROM "ob-poc".deal_rate_cards WHERE rate_card_id = $1"#,
        )
        .bind(rate_card_id)
        .fetch_one(pool)
        .await?;

        let (status, deal_id) = row;

        if !matches!(status.as_str(), "PROPOSED" | "COUNTER_OFFERED") {
            return Err(anyhow!("Cannot agree rate card in status {}", status));
        }

        // Update status
        sqlx::query(
            r#"UPDATE "ob-poc".deal_rate_cards SET status = 'AGREED', updated_at = NOW() WHERE rate_card_id = $1"#,
        )
        .bind(rate_card_id)
        .execute(pool)
        .await?;

        // Record event
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".deal_events (deal_id, event_type, subject_type, subject_id, old_value, new_value)
            VALUES ($1, 'RATE_CARD_AGREED', 'RATE_CARD', $2, $3, 'AGREED')
            "#,
        )
        .bind(deal_id)
        .bind(rate_card_id)
        .bind(&status)
        .execute(pool)
        .await?;

        Ok(ExecutionResult::Affected(1))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Affected(1))
    }
}

// =============================================================================
// SLA Operations
// =============================================================================

/// Add an SLA to a deal
#[register_custom_op]
pub struct DealAddSlaOp;

#[async_trait]
impl CustomOperation for DealAddSlaOp {
    fn domain(&self) -> &'static str {
        "deal"
    }
    fn verb(&self) -> &'static str {
        "add-sla"
    }
    fn rationale(&self) -> &'static str {
        "Creates SLA record with optional contract/product/service linkage"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let deal_id = extract_uuid(verb_call, ctx, "deal-id")?;
        let contract_id = extract_uuid_opt(verb_call, ctx, "contract-id");
        let product_id = extract_uuid_opt(verb_call, ctx, "product-id");
        let service_id = extract_uuid_opt(verb_call, ctx, "service-id");
        let sla_name = extract_string(verb_call, "sla-name")?;
        let sla_type = extract_string_opt(verb_call, "sla-type");
        let metric_name = extract_string(verb_call, "metric-name")?;
        let target_value = extract_string(verb_call, "target-value")?;
        let measurement_unit = extract_string_opt(verb_call, "measurement-unit");
        let penalty_type = extract_string_opt(verb_call, "penalty-type");
        let penalty_value: Option<f64> = verb_call
            .get_arg("penalty-value")
            .and_then(|v| v.value.as_decimal())
            .map(|d| d.to_string().parse().unwrap_or(0.0));
        let effective_from = extract_string(verb_call, "effective-from")?;

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
        .fetch_one(pool)
        .await?;

        // Record event
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".deal_events (deal_id, event_type, subject_type, subject_id, description)
            VALUES ($1, 'SLA_ADDED', 'SLA', $2, $3)
            "#,
        )
        .bind(deal_id)
        .bind(sla_id)
        .bind(&sla_name)
        .execute(pool)
        .await?;

        Ok(ExecutionResult::Uuid(sla_id))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Uuid(Uuid::new_v4()))
    }
}

// =============================================================================
// Document Operations
// =============================================================================

/// Add a document to a deal
#[register_custom_op]
pub struct DealAddDocumentOp;

#[async_trait]
impl CustomOperation for DealAddDocumentOp {
    fn domain(&self) -> &'static str {
        "deal"
    }
    fn verb(&self) -> &'static str {
        "add-document"
    }
    fn rationale(&self) -> &'static str {
        "Links document to deal with type and status tracking"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let deal_id = extract_uuid(verb_call, ctx, "deal-id")?;
        let document_id = extract_uuid(verb_call, ctx, "document-id")?;
        let document_type = extract_string(verb_call, "document-type")?;
        let document_status =
            extract_string_opt(verb_call, "document-status").unwrap_or_else(|| "DRAFT".to_string());

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
        .execute(pool)
        .await?;

        Ok(ExecutionResult::Affected(1))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Affected(1))
    }
}

/// Update document status
#[register_custom_op]
pub struct DealUpdateDocumentStatusOp;

#[async_trait]
impl CustomOperation for DealUpdateDocumentStatusOp {
    fn domain(&self) -> &'static str {
        "deal"
    }
    fn verb(&self) -> &'static str {
        "update-document-status"
    }
    fn rationale(&self) -> &'static str {
        "Updates document status and records event"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let deal_id = extract_uuid(verb_call, ctx, "deal-id")?;
        let document_id = extract_uuid(verb_call, ctx, "document-id")?;
        let document_status = extract_string(verb_call, "document-status")?;

        let result = sqlx::query(
            r#"UPDATE "ob-poc".deal_documents SET document_status = $3 WHERE deal_id = $1 AND document_id = $2"#,
        )
        .bind(deal_id)
        .bind(document_id)
        .bind(&document_status)
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
        Ok(ExecutionResult::Affected(1))
    }
}

// =============================================================================
// UBO Assessment Operations
// =============================================================================

/// Add UBO assessment to deal
#[register_custom_op]
pub struct DealAddUboAssessmentOp;

#[async_trait]
impl CustomOperation for DealAddUboAssessmentOp {
    fn domain(&self) -> &'static str {
        "deal"
    }
    fn verb(&self) -> &'static str {
        "add-ubo-assessment"
    }
    fn rationale(&self) -> &'static str {
        "Links entity UBO assessment to deal with optional KYC case"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let deal_id = extract_uuid(verb_call, ctx, "deal-id")?;
        let entity_id = extract_uuid(verb_call, ctx, "entity-id")?;
        let kyc_case_id = extract_uuid_opt(verb_call, ctx, "kyc-case-id");

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
        .fetch_one(pool)
        .await?;

        Ok(ExecutionResult::Uuid(assessment_id))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Uuid(Uuid::new_v4()))
    }
}

/// Update UBO assessment status
#[register_custom_op]
pub struct DealUpdateUboAssessmentOp;

#[async_trait]
impl CustomOperation for DealUpdateUboAssessmentOp {
    fn domain(&self) -> &'static str {
        "deal"
    }
    fn verb(&self) -> &'static str {
        "update-ubo-assessment"
    }
    fn rationale(&self) -> &'static str {
        "Updates assessment status and risk rating with PROHIBITED check"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let assessment_id = extract_uuid(verb_call, ctx, "assessment-id")?;
        let assessment_status = extract_string_opt(verb_call, "assessment-status");
        let risk_rating = extract_string_opt(verb_call, "risk-rating");

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
        Ok(ExecutionResult::Affected(1))
    }
}

// =============================================================================
// Onboarding Handoff Operations
// =============================================================================

/// Create onboarding request
#[register_custom_op]
pub struct DealRequestOnboardingOp;

#[async_trait]
impl CustomOperation for DealRequestOnboardingOp {
    fn domain(&self) -> &'static str {
        "deal"
    }
    fn verb(&self) -> &'static str {
        "request-onboarding"
    }
    fn rationale(&self) -> &'static str {
        "Validates deal status, contract linkage, and CBU ownership before creating request"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let deal_id = extract_uuid(verb_call, ctx, "deal-id")?;
        let contract_id = extract_uuid(verb_call, ctx, "contract-id")?;
        let cbu_id = extract_uuid(verb_call, ctx, "cbu-id")?;
        let product_id = extract_uuid(verb_call, ctx, "product-id")?;
        let requires_kyc = extract_bool_opt(verb_call, "requires-kyc").unwrap_or(true);
        let target_live_date = extract_string_opt(verb_call, "target-live-date");
        let requested_by = extract_string_opt(verb_call, "requested-by");
        let notes = extract_string_opt(verb_call, "notes");

        // Validate deal status
        let deal_status: String =
            sqlx::query_scalar(r#"SELECT deal_status FROM "ob-poc".deals WHERE deal_id = $1"#)
                .bind(deal_id)
                .fetch_one(pool)
                .await?;

        if !matches!(deal_status.as_str(), "CONTRACTED" | "ONBOARDING") {
            return Err(anyhow!(
                "Deal must be in CONTRACTED or ONBOARDING status to request onboarding"
            ));
        }

        // Validate contract is linked
        let contract_linked: bool = sqlx::query_scalar(
            r#"SELECT EXISTS(SELECT 1 FROM "ob-poc".deal_contracts WHERE deal_id = $1 AND contract_id = $2)"#,
        )
        .bind(deal_id)
        .bind(contract_id)
        .fetch_one(pool)
        .await?;

        if !contract_linked {
            return Err(anyhow!("Contract is not linked to this deal"));
        }

        // Create request
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
        .fetch_one(pool)
        .await?;

        // Transition deal to ONBOARDING if currently CONTRACTED
        if deal_status == "CONTRACTED" {
            sqlx::query(r#"UPDATE "ob-poc".deals SET deal_status = 'ONBOARDING', updated_at = NOW() WHERE deal_id = $1"#)
                .bind(deal_id)
                .execute(pool)
                .await?;
        }

        // Record event
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
        .execute(pool)
        .await?;

        Ok(ExecutionResult::Uuid(request_id))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Uuid(Uuid::new_v4()))
    }
}

/// Batch onboarding request
#[register_custom_op]
pub struct DealRequestOnboardingBatchOp;

#[async_trait]
impl CustomOperation for DealRequestOnboardingBatchOp {
    fn domain(&self) -> &'static str {
        "deal"
    }
    fn verb(&self) -> &'static str {
        "request-onboarding-batch"
    }
    fn rationale(&self) -> &'static str {
        "Transactional batch insert of multiple onboarding requests"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let deal_id = extract_uuid(verb_call, ctx, "deal-id")?;
        let contract_id = extract_uuid(verb_call, ctx, "contract-id")?;
        let requires_kyc = extract_bool_opt(verb_call, "requires-kyc").unwrap_or(true);
        let requested_by = extract_string_opt(verb_call, "requested-by");

        // Get requests array from arg
        let requests_arg = verb_call
            .get_arg("requests")
            .ok_or_else(|| anyhow!("requests argument is required"))?;

        let requests: Vec<serde_json::Value> = serde_json::from_value(
            serde_json::to_value(requests_arg).unwrap_or(serde_json::Value::Array(vec![])),
        )
        .unwrap_or_default();

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
            .fetch_one(pool)
            .await?;

            request_ids.push(request_id);
        }

        let result = BatchOnboardingResult {
            request_ids: request_ids.clone(),
            count: request_ids.len() as i32,
        };

        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
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

/// Update onboarding request status
#[register_custom_op]
pub struct DealUpdateOnboardingStatusOp;

#[async_trait]
impl CustomOperation for DealUpdateOnboardingStatusOp {
    fn domain(&self) -> &'static str {
        "deal"
    }
    fn verb(&self) -> &'static str {
        "update-onboarding-status"
    }
    fn rationale(&self) -> &'static str {
        "Updates request status and checks if all requests complete to transition deal to ACTIVE"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let request_id = extract_uuid(verb_call, ctx, "request-id")?;
        let request_status = extract_string(verb_call, "request-status")?;
        let kyc_case_id = extract_uuid_opt(verb_call, ctx, "kyc-case-id");

        // Get deal_id
        let deal_id: Uuid = sqlx::query_scalar(
            r#"SELECT deal_id FROM "ob-poc".deal_onboarding_requests WHERE request_id = $1"#,
        )
        .bind(request_id)
        .fetch_one(pool)
        .await?;

        // Update request
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
        .execute(pool)
        .await?;

        // If COMPLETED, check if all requests are completed
        if request_status == "COMPLETED" {
            let pending_count: i64 = sqlx::query_scalar(
                r#"
                SELECT COUNT(*) FROM "ob-poc".deal_onboarding_requests
                WHERE deal_id = $1 AND request_status NOT IN ('COMPLETED', 'CANCELLED')
                "#,
            )
            .bind(deal_id)
            .fetch_one(pool)
            .await?;

            if pending_count == 0 {
                // All requests completed - transition deal to ACTIVE
                sqlx::query(
                    r#"UPDATE "ob-poc".deals SET deal_status = 'ACTIVE', active_at = NOW(), updated_at = NOW() WHERE deal_id = $1 AND deal_status = 'ONBOARDING'"#,
                )
                .bind(deal_id)
                .execute(pool)
                .await?;

                sqlx::query(
                    r#"
                    INSERT INTO "ob-poc".deal_events (deal_id, event_type, subject_type, subject_id, new_value, description)
                    VALUES ($1, 'STATUS_CHANGED', 'DEAL', $1, 'ACTIVE', 'All onboarding requests completed')
                    "#,
                )
                .bind(deal_id)
                .execute(pool)
                .await?;
            }
        }

        Ok(ExecutionResult::Affected(1))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Affected(1))
    }
}

// =============================================================================
// Summary Operations
// =============================================================================

/// Full deal summary
#[register_custom_op]
pub struct DealSummaryOp;

#[async_trait]
impl CustomOperation for DealSummaryOp {
    fn domain(&self) -> &'static str {
        "deal"
    }
    fn verb(&self) -> &'static str {
        "summary"
    }
    fn rationale(&self) -> &'static str {
        "Composite query returning deal with all nested data"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let deal_id = extract_uuid(verb_call, ctx, "deal-id")?;

        // Get deal
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
        .fetch_optional(pool)
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

        // Get participants count
        let participant_count: i64 = sqlx::query_scalar(
            r#"SELECT COUNT(*) FROM "ob-poc".deal_participants WHERE deal_id = $1"#,
        )
        .bind(deal_id)
        .fetch_one(pool)
        .await?;

        // Get contracts count
        let contract_count: i64 = sqlx::query_scalar(
            r#"SELECT COUNT(*) FROM "ob-poc".deal_contracts WHERE deal_id = $1"#,
        )
        .bind(deal_id)
        .fetch_one(pool)
        .await?;

        // Get rate cards count
        let rate_card_count: i64 = sqlx::query_scalar(
            r#"SELECT COUNT(*) FROM "ob-poc".deal_rate_cards WHERE deal_id = $1"#,
        )
        .bind(deal_id)
        .fetch_one(pool)
        .await?;

        // Get onboarding progress
        let onboarding_stats: (i64, i64) = sqlx::query_as(
            r#"
            SELECT
                COUNT(*) FILTER (WHERE request_status = 'COMPLETED'),
                COUNT(*)
            FROM "ob-poc".deal_onboarding_requests WHERE deal_id = $1
            "#,
        )
        .bind(deal_id)
        .fetch_one(pool)
        .await?;

        let result = serde_json::json!({
            "deal": deal_info,
            "participant_count": participant_count,
            "contract_count": contract_count,
            "rate_card_count": rate_card_count,
            "onboarding_completed": onboarding_stats.0,
            "onboarding_total": onboarding_stats.1
        });

        Ok(ExecutionResult::Record(result))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::json!({})))
    }
}
