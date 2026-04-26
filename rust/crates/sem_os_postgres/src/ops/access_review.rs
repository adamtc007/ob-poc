//! Access-review automation verbs (8 plugin verbs) — YAML-first
//! re-implementation of `rust/config/verbs/access-review.yaml`.
//!
//! Slice #10 pattern: `sqlx::query!` macros rewritten as runtime
//! `sqlx::query` / `sqlx::query_as` / `sqlx::query_scalar` (cache-free).
//! Legacy per-op `pool.begin()` transactions replaced by the
//! Sequencer-owned scope so campaign stats + item updates are atomic
//! under one txn.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::{json, Value};
use uuid::Uuid;

use dsl_runtime::domain_ops::helpers::{
    json_extract_string, json_extract_string_list, json_extract_string_opt, json_extract_uuid,
};
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::SemOsVerbOp;

const DEFAULT_ATTESTATION_TEXT: &str = r#"I hereby attest that I have reviewed the access rights listed in this campaign and confirm that:
1. Each confirmed access is necessary for the user's current role
2. Each confirmed access is appropriate given the user's legal authority
3. I have revoked or flagged any access that is no longer required
4. To the best of my knowledge, the reviewed access rights comply with applicable policies and regulations"#;

// ── access-review.populate-campaign ───────────────────────────────────────────

pub struct PopulateCampaign;

#[async_trait]
impl SemOsVerbOp for PopulateCampaign {
    fn fqn(&self) -> &str {
        "access-review.populate-campaign"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let campaign_id = json_extract_uuid(args, ctx, "campaign")?;

        let campaign: Option<(String, Option<Value>)> = sqlx::query_as(
            r#"SELECT scope_type, scope_filter
               FROM "ob-poc".access_review_campaigns
               WHERE campaign_id = $1"#,
        )
        .bind(campaign_id)
        .fetch_optional(scope.executor())
        .await?;
        let (scope_type, _scope_filter) = campaign.ok_or_else(|| anyhow!("Campaign not found"))?;

        sqlx::query(
            r#"UPDATE "ob-poc".access_review_campaigns SET status = 'POPULATING' WHERE campaign_id = $1"#,
        )
        .bind(campaign_id)
        .execute(scope.executor())
        .await?;

        let inserted = sqlx::query(
            r#"
            INSERT INTO "ob-poc".access_review_items (
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
                (t.team_type IN ('board', 'investment-committee', 'conducting-officers')
                    AND m.legal_appointment_id IS NULL),
                (c.last_login_at IS NOT NULL AND c.last_login_at < CURRENT_DATE - 90),
                (c.last_login_at IS NULL AND m.created_at < CURRENT_DATE - 30),
                CASE
                    WHEN c.last_login_at IS NOT NULL AND c.last_login_at < CURRENT_DATE - 90 THEN 'REVOKE'
                    WHEN c.last_login_at IS NULL AND m.created_at < CURRENT_DATE - 30 THEN 'REVIEW'
                    WHEN t.team_type IN ('board', 'investment-committee', 'conducting-officers')
                         AND m.legal_appointment_id IS NULL THEN 'REVIEW'
                    ELSE 'CONFIRM'
                END,
                CASE
                    WHEN c.last_login_at IS NOT NULL AND c.last_login_at < CURRENT_DATE - 90 THEN 70
                    WHEN t.team_type IN ('board', 'investment-committee', 'conducting-officers')
                         AND m.legal_appointment_id IS NULL THEN 80
                    WHEN c.last_login_at IS NULL AND m.created_at < CURRENT_DATE - 30 THEN 40
                    ELSE 10
                END,
                'PENDING'
            FROM "ob-poc".memberships m
            JOIN "ob-poc".teams t ON m.team_id = t.team_id
            JOIN "ob-poc".clients c ON m.user_id = c.client_id
            WHERE m.effective_to IS NULL
              AND t.is_active = true
              AND (
                $2 = 'ALL'
                OR ($2 = 'GOVERNANCE_ONLY'
                    AND t.team_type IN ('board', 'investment-committee', 'conducting-officers', 'executive'))
              )
            ON CONFLICT DO NOTHING
            "#,
        )
        .bind(campaign_id)
        .bind(&scope_type)
        .execute(scope.executor())
        .await?;
        let total_items = inserted.rows_affected() as i32;

        let (flagged,): (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*)
            FROM "ob-poc".access_review_items
            WHERE campaign_id = $1
              AND (flag_no_legal_link = true OR flag_dormant_account = true OR flag_never_logged_in = true)
            "#,
        )
        .bind(campaign_id)
        .fetch_one(scope.executor())
        .await?;

        sqlx::query(
            r#"
            UPDATE "ob-poc".access_review_campaigns
            SET total_items = $2, pending_items = $2, status = 'READY'
            WHERE campaign_id = $1
            "#,
        )
        .bind(campaign_id)
        .bind(total_items)
        .execute(scope.executor())
        .await?;

        Ok(VerbExecutionOutcome::Record(json!({
            "campaign_id": campaign_id,
            "total_items": total_items,
            "flagged_items": flagged,
        })))
    }
}

// ── access-review.launch-campaign ─────────────────────────────────────────────

pub struct LaunchCampaign;

#[async_trait]
impl SemOsVerbOp for LaunchCampaign {
    fn fqn(&self) -> &str {
        "access-review.launch-campaign"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let campaign_id = json_extract_uuid(args, ctx, "campaign")?;
        let row: Option<(Option<String>, Option<i32>)> = sqlx::query_as(
            r#"
            UPDATE "ob-poc".access_review_campaigns
            SET status = 'ACTIVE', launched_at = NOW()
            WHERE campaign_id = $1 AND status IN ('DRAFT', 'READY')
            RETURNING name, total_items
            "#,
        )
        .bind(campaign_id)
        .fetch_optional(scope.executor())
        .await?;

        match row {
            Some((name, total_items)) => Ok(VerbExecutionOutcome::Record(json!({
                "campaign_id": campaign_id,
                "name": name,
                "total_items": total_items,
                "status": "ACTIVE",
            }))),
            None => Err(anyhow!("Campaign not found or not in launchable state")),
        }
    }
}

// ── access-review.revoke-access ───────────────────────────────────────────────

pub struct RevokeAccess;

#[async_trait]
impl SemOsVerbOp for RevokeAccess {
    fn fqn(&self) -> &str {
        "access-review.revoke-access"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let item_id = json_extract_uuid(args, ctx, "item")?;
        let reason = json_extract_string(args, "reason")?;

        let item: Option<(Uuid, Uuid)> = sqlx::query_as(
            r#"SELECT membership_id, campaign_id FROM "ob-poc".access_review_items WHERE item_id = $1"#,
        )
        .bind(item_id)
        .fetch_optional(scope.executor())
        .await?;
        let (membership_id, campaign_id) = item.ok_or_else(|| anyhow!("Review item not found"))?;

        sqlx::query(
            r#"UPDATE "ob-poc".memberships SET effective_to = CURRENT_DATE WHERE membership_id = $1"#,
        )
        .bind(membership_id)
        .execute(scope.executor())
        .await?;

        sqlx::query(
            r#"
            UPDATE "ob-poc".access_review_items
            SET status = 'REVOKED', reviewed_at = NOW(), reviewer_notes = $2
            WHERE item_id = $1
            "#,
        )
        .bind(item_id)
        .bind(&reason)
        .execute(scope.executor())
        .await?;

        sqlx::query(
            r#"
            UPDATE "ob-poc".access_review_campaigns
            SET revoked_items = COALESCE(revoked_items, 0) + 1,
                reviewed_items = COALESCE(reviewed_items, 0) + 1,
                pending_items = GREATEST(COALESCE(pending_items, 0) - 1, 0)
            WHERE campaign_id = $1
            "#,
        )
        .bind(campaign_id)
        .execute(scope.executor())
        .await?;

        Ok(VerbExecutionOutcome::Record(json!({
            "item_id": item_id,
            "membership_id": membership_id,
            "status": "REVOKED",
        })))
    }
}

// ── access-review.bulk-confirm ────────────────────────────────────────────────

pub struct BulkConfirm;

#[async_trait]
impl SemOsVerbOp for BulkConfirm {
    fn fqn(&self) -> &str {
        "access-review.bulk-confirm"
    }
    async fn execute(
        &self,
        args: &Value,
        _ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let item_ids: Vec<Uuid> = json_extract_string_list(args, "items")?
            .into_iter()
            .map(|s| s.parse())
            .collect::<std::result::Result<Vec<_>, _>>()?;
        if item_ids.is_empty() {
            return Ok(VerbExecutionOutcome::Record(json!({ "confirmed": 0 })));
        }
        let notes = json_extract_string_opt(args, "notes");

        let updated: Vec<(Uuid,)> = sqlx::query_as(
            r#"
            UPDATE "ob-poc".access_review_items
            SET status = 'CONFIRMED', reviewed_at = NOW(), reviewer_notes = $2
            WHERE item_id = ANY($1) AND status = 'PENDING'
            RETURNING campaign_id
            "#,
        )
        .bind(&item_ids)
        .bind(&notes)
        .fetch_all(scope.executor())
        .await?;
        let confirmed_count = updated.len() as i32;

        if let Some((first_campaign,)) = updated.first() {
            sqlx::query(
                r#"
                UPDATE "ob-poc".access_review_campaigns
                SET confirmed_items = COALESCE(confirmed_items, 0) + $2,
                    reviewed_items = COALESCE(reviewed_items, 0) + $2,
                    pending_items = GREATEST(COALESCE(pending_items, 0) - $2, 0)
                WHERE campaign_id = $1
                "#,
            )
            .bind(first_campaign)
            .bind(confirmed_count)
            .execute(scope.executor())
            .await?;
        }

        Ok(VerbExecutionOutcome::Record(
            json!({ "confirmed": confirmed_count }),
        ))
    }
}

// ── access-review.confirm-all-clean ───────────────────────────────────────────

pub struct ConfirmAllClean;

#[async_trait]
impl SemOsVerbOp for ConfirmAllClean {
    fn fqn(&self) -> &str {
        "access-review.confirm-all-clean"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let campaign_id = json_extract_uuid(args, ctx, "campaign")?;

        let result = sqlx::query(
            r#"
            UPDATE "ob-poc".access_review_items
            SET status = 'CONFIRMED', reviewed_at = NOW(),
                reviewer_notes = 'Auto-confirmed: no issues detected'
            WHERE campaign_id = $1
              AND status = 'PENDING'
              AND flag_no_legal_link = false
              AND flag_dormant_account = false
              AND flag_never_logged_in = false
              AND risk_score < 40
            "#,
        )
        .bind(campaign_id)
        .execute(scope.executor())
        .await?;
        let confirmed = result.rows_affected() as i32;

        sqlx::query(
            r#"
            UPDATE "ob-poc".access_review_campaigns
            SET confirmed_items = COALESCE(confirmed_items, 0) + $2,
                reviewed_items = COALESCE(reviewed_items, 0) + $2,
                pending_items = GREATEST(COALESCE(pending_items, 0) - $2, 0)
            WHERE campaign_id = $1
            "#,
        )
        .bind(campaign_id)
        .bind(confirmed)
        .execute(scope.executor())
        .await?;

        let (remaining,): (i64,) = sqlx::query_as(
            r#"SELECT COUNT(*) FROM "ob-poc".access_review_items
               WHERE campaign_id = $1 AND status = 'PENDING'"#,
        )
        .bind(campaign_id)
        .fetch_one(scope.executor())
        .await?;

        Ok(VerbExecutionOutcome::Record(json!({
            "confirmed": confirmed,
            "remaining": remaining,
        })))
    }
}

// ── access-review.attest ──────────────────────────────────────────────────────

pub struct Attest;

#[async_trait]
impl SemOsVerbOp for Attest {
    fn fqn(&self) -> &str {
        "access-review.attest"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        use std::hash::{Hash, Hasher};
        let campaign_id = json_extract_uuid(args, ctx, "campaign")?;
        let scope_arg = json_extract_string(args, "scope")?;
        let attestation_text = json_extract_string_opt(args, "attestation-text")
            .unwrap_or_else(|| DEFAULT_ATTESTATION_TEXT.to_string());

        let item_ids: Vec<Uuid> = sqlx::query_scalar(
            r#"SELECT item_id FROM "ob-poc".access_review_items
               WHERE campaign_id = $1 AND status != 'PENDING'"#,
        )
        .bind(campaign_id)
        .fetch_all(scope.executor())
        .await?;
        let items_count = item_ids.len() as i32;
        let timestamp = chrono::Utc::now();
        let signature_input = format!(
            "campaign:{}|items:{}|text:{}|time:{}",
            campaign_id, items_count, attestation_text, timestamp
        );
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        signature_input.hash(&mut hasher);
        let signature_hash = format!("{:016x}", hasher.finish());

        let attestation_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO "ob-poc".access_attestations (
                campaign_id, attester_user_id, attester_name, attester_email,
                attestation_scope, items_count,
                attestation_text, attested_at, signature_hash
            )
            VALUES ($1, '00000000-0000-0000-0000-000000000000'::uuid, 'System', 'system@local',
                    $2, $3, $4, $5, $6)
            RETURNING attestation_id
            "#,
        )
        .bind(campaign_id)
        .bind(&scope_arg)
        .bind(items_count)
        .bind(&attestation_text)
        .bind(timestamp)
        .bind(&signature_hash)
        .fetch_one(scope.executor())
        .await?;

        Ok(VerbExecutionOutcome::Uuid(attestation_id))
    }
}

// ── access-review.process-deadline ────────────────────────────────────────────

pub struct ProcessDeadline;

#[async_trait]
impl SemOsVerbOp for ProcessDeadline {
    fn fqn(&self) -> &str {
        "access-review.process-deadline"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let campaign_id = json_extract_uuid(args, ctx, "campaign")?;
        let action = json_extract_string(args, "action")?;

        let affected: i64 = match action.as_str() {
            "suspend" => {
                sqlx::query(
                    r#"
                    UPDATE "ob-poc".memberships m
                    SET effective_to = CURRENT_DATE
                    FROM "ob-poc".access_review_items i
                    WHERE i.membership_id = m.membership_id
                      AND i.campaign_id = $1
                      AND i.status = 'PENDING'
                    "#,
                )
                .bind(campaign_id)
                .execute(scope.executor())
                .await?;
                let r = sqlx::query(
                    r#"
                    UPDATE "ob-poc".access_review_items
                    SET status = 'AUTO_SUSPENDED', reviewed_at = NOW(),
                        reviewer_notes = 'Auto-suspended: unreviewed past deadline'
                    WHERE campaign_id = $1 AND status = 'PENDING'
                    "#,
                )
                .bind(campaign_id)
                .execute(scope.executor())
                .await?;
                r.rows_affected() as i64
            }
            "escalate" => {
                let r = sqlx::query(
                    r#"
                    UPDATE "ob-poc".access_review_items
                    SET status = 'ESCALATED', escalated_at = NOW(),
                        reviewer_notes = 'Escalated: unreviewed past deadline'
                    WHERE campaign_id = $1 AND status = 'PENDING'
                    "#,
                )
                .bind(campaign_id)
                .execute(scope.executor())
                .await?;
                r.rows_affected() as i64
            }
            "report-only" => {
                let (c,): (i64,) = sqlx::query_as(
                    r#"SELECT COUNT(*) FROM "ob-poc".access_review_items
                       WHERE campaign_id = $1 AND status = 'PENDING'"#,
                )
                .bind(campaign_id)
                .fetch_one(scope.executor())
                .await?;
                c
            }
            _ => return Err(anyhow!("Invalid action: {}", action)),
        };

        if action != "report-only" {
            sqlx::query(
                r#"
                UPDATE "ob-poc".access_review_campaigns
                SET status = 'COMPLETED', completed_at = NOW()
                WHERE campaign_id = $1
                "#,
            )
            .bind(campaign_id)
            .execute(scope.executor())
            .await?;
        }

        Ok(VerbExecutionOutcome::Record(json!({
            "action": action,
            "affected": affected,
        })))
    }
}

// ── access-review.send-reminders ──────────────────────────────────────────────

pub struct SendReminders;

#[async_trait]
impl SemOsVerbOp for SendReminders {
    fn fqn(&self) -> &str {
        "access-review.send-reminders"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let campaign_id = json_extract_uuid(args, ctx, "campaign")?;
        let (pending_count,): (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(DISTINCT reviewer_user_id)
            FROM "ob-poc".access_review_items
            WHERE campaign_id = $1 AND status = 'PENDING'
            "#,
        )
        .bind(campaign_id)
        .fetch_one(scope.executor())
        .await?;
        Ok(VerbExecutionOutcome::Record(json!({
            "campaign_id": campaign_id,
            "reviewers_to_notify": pending_count,
        })))
    }
}
