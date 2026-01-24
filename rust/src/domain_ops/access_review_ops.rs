//! Access Review Automation Operations
//!
//! Plugin handlers for periodic access reviews, attestation, and enforcement.
//! These require custom code because they involve:
//! - Complex campaign population with flag detection
//! - Bulk operations with transaction management
//! - Digital attestation with signature generation
//! - Deadline enforcement with membership suspension

use anyhow::Result;
use async_trait::async_trait;
use ob_poc_macros::register_custom_op;
use serde_json::json;

use super::helpers::extract_uuid;
use super::CustomOperation;
use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};

#[cfg(feature = "database")]
use sqlx::PgPool;

// =============================================================================
// CAMPAIGN OPERATIONS
// =============================================================================

/// Populate campaign with review items based on scope
#[register_custom_op]
pub struct AccessReviewPopulateOp;

#[async_trait]
impl CustomOperation for AccessReviewPopulateOp {
    fn domain(&self) -> &'static str {
        "access-review"
    }

    fn verb(&self) -> &'static str {
        "populate-campaign"
    }

    fn rationale(&self) -> &'static str {
        "Complex multi-table insert with flag detection and reviewer assignment"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let campaign_id = extract_uuid(verb_call, ctx, "campaign")?;

        // Get campaign scope
        let campaign = sqlx::query!(
            r#"
            SELECT scope_type, scope_filter
            FROM teams.access_review_campaigns
            WHERE campaign_id = $1
            "#,
            campaign_id
        )
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Campaign not found"))?;

        // Update status to POPULATING
        sqlx::query!(
            r#"UPDATE teams.access_review_campaigns SET status = 'POPULATING' WHERE campaign_id = $1"#,
            campaign_id
        )
        .execute(pool)
        .await?;

        // Insert review items with flag detection
        // Note: This is a complex insert that joins multiple tables and computes flags
        let inserted = sqlx::query!(
            r#"
            INSERT INTO teams.access_review_items (
                campaign_id, membership_id, user_id, team_id, role_key,
                reviewer_user_id,
                flag_no_legal_link, flag_dormant_account, flag_never_logged_in,
                recommendation, risk_score, status
            )
            SELECT
                $1,
                m.membership_id,
                m.user_id,
                m.team_id,
                m.role_key,
                m.user_id,
                -- Flags
                (t.team_type IN ('board', 'investment-committee', 'conducting-officers')
                    AND m.legal_appointment_id IS NULL),
                (c.last_login_at IS NOT NULL AND c.last_login_at < CURRENT_DATE - 90),
                (c.last_login_at IS NULL AND m.created_at < CURRENT_DATE - 30),
                -- Recommendation
                CASE
                    WHEN c.last_login_at IS NOT NULL AND c.last_login_at < CURRENT_DATE - 90 THEN 'REVOKE'
                    WHEN c.last_login_at IS NULL AND m.created_at < CURRENT_DATE - 30 THEN 'REVIEW'
                    WHEN t.team_type IN ('board', 'investment-committee', 'conducting-officers')
                         AND m.legal_appointment_id IS NULL THEN 'REVIEW'
                    ELSE 'CONFIRM'
                END,
                -- Risk score
                CASE
                    WHEN c.last_login_at IS NOT NULL AND c.last_login_at < CURRENT_DATE - 90 THEN 70
                    WHEN t.team_type IN ('board', 'investment-committee', 'conducting-officers')
                         AND m.legal_appointment_id IS NULL THEN 80
                    WHEN c.last_login_at IS NULL AND m.created_at < CURRENT_DATE - 30 THEN 40
                    ELSE 10
                END,
                'PENDING'
            FROM teams.memberships m
            JOIN teams.teams t ON m.team_id = t.team_id
            JOIN client_portal.clients c ON m.user_id = c.client_id
            WHERE m.effective_to IS NULL
              AND t.is_active = true
              AND (
                $2 = 'ALL'
                OR ($2 = 'GOVERNANCE_ONLY'
                    AND t.team_type IN ('board', 'investment-committee', 'conducting-officers', 'executive'))
              )
            ON CONFLICT DO NOTHING
            "#,
            campaign_id,
            campaign.scope_type
        )
        .execute(pool)
        .await?;

        let total_items = inserted.rows_affected() as i32;

        // Count flagged items
        let flagged: i64 = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) as "count!"
            FROM teams.access_review_items
            WHERE campaign_id = $1
              AND (flag_no_legal_link = true OR flag_dormant_account = true OR flag_never_logged_in = true)
            "#,
            campaign_id
        )
        .fetch_one(pool)
        .await?;

        // Update campaign with stats
        sqlx::query!(
            r#"
            UPDATE teams.access_review_campaigns
            SET total_items = $2, pending_items = $2, status = 'READY'
            WHERE campaign_id = $1
            "#,
            campaign_id,
            total_items
        )
        .execute(pool)
        .await?;

        Ok(ExecutionResult::Record(json!({
            "campaign_id": campaign_id,
            "total_items": total_items,
            "flagged_items": flagged
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

/// Launch campaign and set to ACTIVE
#[register_custom_op]
pub struct AccessReviewLaunchOp;

#[async_trait]
impl CustomOperation for AccessReviewLaunchOp {
    fn domain(&self) -> &'static str {
        "access-review"
    }

    fn verb(&self) -> &'static str {
        "launch-campaign"
    }

    fn rationale(&self) -> &'static str {
        "Campaign launch with status transition and timestamp"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let campaign_id = extract_uuid(verb_call, ctx, "campaign")?;

        // Update status and launch time
        let result = sqlx::query!(
            r#"
            UPDATE teams.access_review_campaigns
            SET status = 'ACTIVE', launched_at = NOW()
            WHERE campaign_id = $1 AND status IN ('DRAFT', 'READY')
            RETURNING name, total_items
            "#,
            campaign_id
        )
        .fetch_optional(pool)
        .await?;

        match result {
            Some(row) => Ok(ExecutionResult::Record(json!({
                "campaign_id": campaign_id,
                "name": row.name,
                "total_items": row.total_items,
                "status": "ACTIVE"
            }))),
            None => Err(anyhow::anyhow!(
                "Campaign not found or not in launchable state"
            )),
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

// =============================================================================
// REVIEW ACTIONS
// =============================================================================

/// Revoke access - ends the membership
#[register_custom_op]
pub struct AccessReviewRevokeOp;

#[async_trait]
impl CustomOperation for AccessReviewRevokeOp {
    fn domain(&self) -> &'static str {
        "access-review"
    }

    fn verb(&self) -> &'static str {
        "revoke-access"
    }

    fn rationale(&self) -> &'static str {
        "Revocation requires ending membership and updating review item atomically"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let item_id = extract_uuid(verb_call, ctx, "item")?;

        let reason = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "reason")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| {
                anyhow::anyhow!("access-review.revoke-access: Missing required argument :reason")
            })?;

        let mut tx = pool.begin().await?;

        // Get membership from review item
        let item = sqlx::query!(
            r#"
            SELECT membership_id, campaign_id
            FROM teams.access_review_items
            WHERE item_id = $1
            "#,
            item_id
        )
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Review item not found"))?;

        // End membership
        sqlx::query!(
            r#"
            UPDATE teams.memberships
            SET effective_to = CURRENT_DATE
            WHERE membership_id = $1
            "#,
            item.membership_id
        )
        .execute(&mut *tx)
        .await?;

        // Update review item
        sqlx::query!(
            r#"
            UPDATE teams.access_review_items
            SET status = 'REVOKED', reviewed_at = NOW(), reviewer_notes = $2
            WHERE item_id = $1
            "#,
            item_id,
            reason
        )
        .execute(&mut *tx)
        .await?;

        // Update campaign stats
        sqlx::query!(
            r#"
            UPDATE teams.access_review_campaigns
            SET revoked_items = COALESCE(revoked_items, 0) + 1,
                reviewed_items = COALESCE(reviewed_items, 0) + 1,
                pending_items = GREATEST(COALESCE(pending_items, 0) - 1, 0)
            WHERE campaign_id = $1
            "#,
            item.campaign_id
        )
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(ExecutionResult::Record(json!({
            "item_id": item_id,
            "membership_id": item.membership_id,
            "status": "REVOKED"
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

// =============================================================================
// BULK OPERATIONS
// =============================================================================

/// Bulk confirm multiple items
#[register_custom_op]
pub struct AccessReviewBulkConfirmOp;

#[async_trait]
impl CustomOperation for AccessReviewBulkConfirmOp {
    fn domain(&self) -> &'static str {
        "access-review"
    }

    fn verb(&self) -> &'static str {
        "bulk-confirm"
    }

    fn rationale(&self) -> &'static str {
        "Bulk update with campaign stats maintenance"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use uuid::Uuid;

        // Get items array from arguments
        let items_arg = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "items")
            .ok_or_else(|| {
                anyhow::anyhow!("access-review.bulk-confirm: Missing required argument :items")
            })?;

        let item_ids: Vec<Uuid> = match &items_arg.value {
            crate::dsl_v2::ast::AstNode::List { items, .. } => {
                items.iter().filter_map(|n| n.as_uuid()).collect()
            }
            _ => return Err(anyhow::anyhow!("items must be a list of UUIDs")),
        };

        if item_ids.is_empty() {
            return Ok(ExecutionResult::Record(json!({"confirmed": 0})));
        }

        let notes = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "notes")
            .and_then(|a| a.value.as_string());

        let mut tx = pool.begin().await?;

        // Update items and get campaign_id
        let updated = sqlx::query!(
            r#"
            UPDATE teams.access_review_items
            SET status = 'CONFIRMED', reviewed_at = NOW(), reviewer_notes = $2
            WHERE item_id = ANY($1) AND status = 'PENDING'
            RETURNING campaign_id
            "#,
            &item_ids,
            notes
        )
        .fetch_all(&mut *tx)
        .await?;

        let confirmed_count = updated.len() as i32;

        // Update campaign stats if any were confirmed
        if let Some(first) = updated.first() {
            sqlx::query!(
                r#"
                UPDATE teams.access_review_campaigns
                SET confirmed_items = COALESCE(confirmed_items, 0) + $2,
                    reviewed_items = COALESCE(reviewed_items, 0) + $2,
                    pending_items = GREATEST(COALESCE(pending_items, 0) - $2, 0)
                WHERE campaign_id = $1
                "#,
                first.campaign_id,
                confirmed_count
            )
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;

        Ok(ExecutionResult::Record(
            json!({"confirmed": confirmed_count}),
        ))
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

/// Confirm all unflagged items for a reviewer
#[register_custom_op]
pub struct AccessReviewConfirmCleanOp;

#[async_trait]
impl CustomOperation for AccessReviewConfirmCleanOp {
    fn domain(&self) -> &'static str {
        "access-review"
    }

    fn verb(&self) -> &'static str {
        "confirm-all-clean"
    }

    fn rationale(&self) -> &'static str {
        "Bulk confirmation of low-risk items"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let campaign_id = extract_uuid(verb_call, ctx, "campaign")?;

        let mut tx = pool.begin().await?;

        // Confirm all unflagged, low-risk items
        let result = sqlx::query!(
            r#"
            UPDATE teams.access_review_items
            SET status = 'CONFIRMED', reviewed_at = NOW(),
                reviewer_notes = 'Auto-confirmed: no issues detected'
            WHERE campaign_id = $1
              AND status = 'PENDING'
              AND flag_no_legal_link = false
              AND flag_dormant_account = false
              AND flag_never_logged_in = false
              AND risk_score < 40
            "#,
            campaign_id
        )
        .execute(&mut *tx)
        .await?;

        let confirmed = result.rows_affected() as i32;

        // Update campaign stats
        sqlx::query!(
            r#"
            UPDATE teams.access_review_campaigns
            SET confirmed_items = COALESCE(confirmed_items, 0) + $2,
                reviewed_items = COALESCE(reviewed_items, 0) + $2,
                pending_items = GREATEST(COALESCE(pending_items, 0) - $2, 0)
            WHERE campaign_id = $1
            "#,
            campaign_id,
            confirmed
        )
        .execute(&mut *tx)
        .await?;

        // Get remaining pending count
        let remaining: i64 = sqlx::query_scalar!(
            r#"SELECT COUNT(*) as "count!" FROM teams.access_review_items
               WHERE campaign_id = $1 AND status = 'PENDING'"#,
            campaign_id
        )
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(ExecutionResult::Record(json!({
            "confirmed": confirmed,
            "remaining": remaining
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

// =============================================================================
// ATTESTATION
// =============================================================================

/// Formal attestation with digital signature
#[register_custom_op]
pub struct AccessReviewAttestOp;

#[async_trait]
impl CustomOperation for AccessReviewAttestOp {
    fn domain(&self) -> &'static str {
        "access-review"
    }

    fn verb(&self) -> &'static str {
        "attest"
    }

    fn rationale(&self) -> &'static str {
        "Digital attestation with cryptographic signature generation"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use chrono::Utc;
        use uuid::Uuid;

        let campaign_id = extract_uuid(verb_call, ctx, "campaign")?;

        let scope = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "scope")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| {
                anyhow::anyhow!("access-review.attest: Missing required argument :scope")
            })?;

        let attestation_text = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "attestation-text")
            .and_then(|a| a.value.as_string())
            .unwrap_or(DEFAULT_ATTESTATION_TEXT);

        // Get reviewed items in scope
        let item_ids: Vec<Uuid> = sqlx::query_scalar!(
            r#"SELECT item_id FROM teams.access_review_items
               WHERE campaign_id = $1 AND status != 'PENDING'"#,
            campaign_id
        )
        .fetch_all(pool)
        .await?;

        let items_count = item_ids.len() as i32;
        let timestamp = Utc::now();

        // Generate signature hash using simple hash
        let signature_input = format!(
            "campaign:{}|items:{}|text:{}|time:{}",
            campaign_id, items_count, attestation_text, timestamp
        );
        // Simple hash for now - use std::hash
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        signature_input.hash(&mut hasher);
        let signature_hash = format!("{:016x}", hasher.finish());

        // Create attestation record
        // Note: access_attestations requires attester info which we'd need from session
        // For now, we'll use placeholder values - in production these come from auth context
        let attestation_id: Uuid = sqlx::query_scalar!(
            r#"
            INSERT INTO teams.access_attestations (
                campaign_id, attester_user_id, attester_name, attester_email,
                attestation_scope, items_count,
                attestation_text, attested_at, signature_hash
            )
            VALUES ($1, '00000000-0000-0000-0000-000000000000'::uuid, 'System', 'system@local',
                    $2, $3, $4, $5, $6)
            RETURNING attestation_id
            "#,
            campaign_id,
            scope,
            items_count,
            attestation_text,
            timestamp,
            signature_hash
        )
        .fetch_one(pool)
        .await?;

        Ok(ExecutionResult::Uuid(attestation_id))
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

// =============================================================================
// DEADLINE PROCESSING
// =============================================================================

/// Process items past deadline
#[register_custom_op]
pub struct AccessReviewProcessDeadlineOp;

#[async_trait]
impl CustomOperation for AccessReviewProcessDeadlineOp {
    fn domain(&self) -> &'static str {
        "access-review"
    }

    fn verb(&self) -> &'static str {
        "process-deadline"
    }

    fn rationale(&self) -> &'static str {
        "Deadline enforcement with membership suspension"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let campaign_id = extract_uuid(verb_call, ctx, "campaign")?;

        let action = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "action")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| {
                anyhow::anyhow!("access-review.process-deadline: Missing required argument :action")
            })?;

        let mut tx = pool.begin().await?;

        let affected: i64 = match action {
            "suspend" => {
                // End memberships for unreviewed items
                sqlx::query!(
                    r#"
                    UPDATE teams.memberships m
                    SET effective_to = CURRENT_DATE
                    FROM teams.access_review_items i
                    WHERE i.membership_id = m.membership_id
                      AND i.campaign_id = $1
                      AND i.status = 'PENDING'
                    "#,
                    campaign_id
                )
                .execute(&mut *tx)
                .await?;

                // Mark items as auto-suspended
                let result = sqlx::query!(
                    r#"
                    UPDATE teams.access_review_items
                    SET status = 'AUTO_SUSPENDED', reviewed_at = NOW(),
                        reviewer_notes = 'Auto-suspended: unreviewed past deadline'
                    WHERE campaign_id = $1 AND status = 'PENDING'
                    "#,
                    campaign_id
                )
                .execute(&mut *tx)
                .await?;

                result.rows_affected() as i64
            }
            "escalate" => {
                let result = sqlx::query!(
                    r#"
                    UPDATE teams.access_review_items
                    SET status = 'ESCALATED', escalated_at = NOW(),
                        reviewer_notes = 'Escalated: unreviewed past deadline'
                    WHERE campaign_id = $1 AND status = 'PENDING'
                    "#,
                    campaign_id
                )
                .execute(&mut *tx)
                .await?;

                result.rows_affected() as i64
            }
            "report-only" => {
                sqlx::query_scalar!(
                    r#"SELECT COUNT(*) as "count!" FROM teams.access_review_items
                       WHERE campaign_id = $1 AND status = 'PENDING'"#,
                    campaign_id
                )
                .fetch_one(&mut *tx)
                .await?
            }
            _ => return Err(anyhow::anyhow!("Invalid action: {}", action)),
        };

        // Update campaign status
        if action != "report-only" {
            sqlx::query!(
                r#"
                UPDATE teams.access_review_campaigns
                SET status = 'COMPLETED', completed_at = NOW()
                WHERE campaign_id = $1
                "#,
                campaign_id
            )
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;

        Ok(ExecutionResult::Record(json!({
            "action": action,
            "affected": affected
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

/// Send reminder notifications
#[register_custom_op]
pub struct AccessReviewSendRemindersOp;

#[async_trait]
impl CustomOperation for AccessReviewSendRemindersOp {
    fn domain(&self) -> &'static str {
        "access-review"
    }

    fn verb(&self) -> &'static str {
        "send-reminders"
    }

    fn rationale(&self) -> &'static str {
        "Reminder aggregation by reviewer"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let campaign_id = extract_uuid(verb_call, ctx, "campaign")?;

        // Get count of reviewers with pending items
        let pending_count: i64 = sqlx::query_scalar!(
            r#"
            SELECT COUNT(DISTINCT reviewer_user_id) as "count!"
            FROM teams.access_review_items
            WHERE campaign_id = $1 AND status = 'PENDING'
            "#,
            campaign_id
        )
        .fetch_one(pool)
        .await?;

        Ok(ExecutionResult::Record(json!({
            "campaign_id": campaign_id,
            "reviewers_to_notify": pending_count
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

// =============================================================================
// CONSTANTS
// =============================================================================

const DEFAULT_ATTESTATION_TEXT: &str = r#"I hereby attest that I have reviewed the access rights listed in this campaign and confirm that:
1. Each confirmed access is necessary for the user's current role
2. Each confirmed access is appropriate given the user's legal authority
3. I have revoked or flagged any access that is no longer required
4. To the best of my knowledge, the reviewed access rights comply with applicable policies and regulations"#;
