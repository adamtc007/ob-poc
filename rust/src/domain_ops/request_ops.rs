//! Outstanding Request Operations (11 plugin verbs) — `request.*`, `document.*`
//! lifecycle ops for fire-and-forget request tracking, auto-fulfillment, and
//! BPMN signal routing.
//!
//! Phase 5c-migrate Phase B Pattern B slice #76: ported from
//! `CustomOperation` + `inventory::collect!` to `SemOsVerbOp`. Stays in
//! `ob-poc::domain_ops::request_ops` because the ops bridge to
//! `crate::bpmn_integration::{client, correlation}` — upstream of
//! `sem_os_postgres`.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use chrono::{Duration, NaiveDate, Utc};
use sem_os_postgres::ops::SemOsVerbOp;
use serde_json::json;
use uuid::Uuid;

use dsl_runtime::domain_ops::helpers::{
    json_extract_bool_opt, json_extract_int_opt, json_extract_string, json_extract_string_opt,
    json_extract_uuid, json_extract_uuid_opt,
};
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

#[cfg(feature = "database")]
use sqlx::PgPool;

// ═══════════════════════════════════════════════════════════════════════════════
// Shared Types
// ═══════════════════════════════════════════════════════════════════════════════

/// Resolved document subject with all linked IDs
#[derive(Debug, Default)]
pub struct DocumentSubject {
    pub subject_type: String,
    pub subject_id: Uuid,
    pub workstream_id: Option<Uuid>,
    pub case_id: Option<Uuid>,
    pub cbu_id: Option<Uuid>,
    pub entity_id: Option<Uuid>,
}

/// Linked IDs derived from a subject
#[derive(Debug, Default)]
pub struct LinkedIds {
    pub workstream_id: Option<Uuid>,
    pub case_id: Option<Uuid>,
    pub cbu_id: Option<Uuid>,
    pub entity_id: Option<Uuid>,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Request Create Operation
// ═══════════════════════════════════════════════════════════════════════════════

/// Create an outstanding request (generic)
pub struct RequestCreate;

#[async_trait]
impl SemOsVerbOp for RequestCreate {
    fn fqn(&self) -> &str {
        "request.create"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let pool = scope.pool().clone();

        // Extract required args
        let subject_type = json_extract_string(args, "subject-type")?;

        let subject_id = json_extract_uuid(args, ctx, "subject-id")?;

        let request_type = json_extract_string(args, "type")?;

        let request_subtype = json_extract_string(args, "subtype")?;

        // Get defaults from request_types config
        let config = sqlx::query!(
            r#"
            SELECT default_due_days, default_grace_days, blocks_by_default, max_reminders
            FROM "ob-poc".request_types
            WHERE request_type = $1 AND request_subtype = $2
            "#,
            request_type,
            request_subtype
        )
        .fetch_optional(&pool)
        .await?;

        let default_due_days = config
            .as_ref()
            .and_then(|c| c.default_due_days)
            .unwrap_or(7);
        let default_grace_days = config
            .as_ref()
            .and_then(|c| c.default_grace_days)
            .unwrap_or(3);
        let blocks_by_default = config
            .as_ref()
            .and_then(|c| c.blocks_by_default)
            .unwrap_or(true);
        let max_reminders = config.as_ref().and_then(|c| c.max_reminders).unwrap_or(3);

        // Optional args with defaults
        let due_in_days =
            json_extract_int_opt(args, "due-in-days").unwrap_or(default_due_days as i64);

        let due_date: NaiveDate = json_extract_string_opt(args, "due-date")
            .and_then(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok())
            .unwrap_or_else(|| {
                (Utc::now() + Duration::days(due_in_days))
                    .naive_utc()
                    .date()
            });

        let blocks = json_extract_bool_opt(args, "blocks").unwrap_or(blocks_by_default);

        let blocker_message = json_extract_string_opt(args, "message");

        let requested_from_label = json_extract_string_opt(args, "from");

        let requested_from_entity_id = json_extract_uuid_opt(args, ctx, "from-entity");

        let request_details: serde_json::Value = args
            .get("details")
            .and_then(|v| {
                if let Some(map) = v.as_object() {
                    let json_map: serde_json::Map<String, serde_json::Value> = map
                        .iter()
                        .filter_map(|(k, v)| {
                            v.as_str()
                                .map(|s| (k.clone(), serde_json::Value::String(s.to_string())))
                        })
                        .collect();
                    Some(serde_json::Value::Object(json_map))
                } else {
                    None
                }
            })
            .unwrap_or(json!({}));

        // Derive linked IDs based on subject_type
        let linked = derive_linked_ids(&subject_type, subject_id, &pool).await?;

        // Create the request
        let row = sqlx::query!(
            r#"
            INSERT INTO "ob-poc".outstanding_requests (
                subject_type, subject_id,
                workstream_id, case_id, cbu_id, entity_id,
                request_type, request_subtype, request_details,
                requested_from_type, requested_from_entity_id, requested_from_label,
                requested_by_agent,
                due_date, grace_period_days, max_reminders,
                blocks_subject, blocker_message,
                created_by_verb
            ) VALUES (
                $1, $2,
                $3, $4, $5, $6,
                $7, $8, $9,
                'CLIENT', $10, $11,
                $12,
                $13, $14, $15,
                $16, $17,
                'request.create'
            )
            RETURNING request_id
            "#,
            subject_type,
            subject_id,
            linked.workstream_id,
            linked.case_id,
            linked.cbu_id,
            linked.entity_id,
            request_type,
            request_subtype,
            request_details,
            requested_from_entity_id,
            requested_from_label,
            true, // requested_by_agent
            due_date,
            default_grace_days,
            max_reminders,
            blocks,
            blocker_message,
        )
        .fetch_one(&pool)
        .await?;

        // If blocking and attached to workstream, update workstream status
        if blocks {
            if let Some(ws_id) = linked.workstream_id {
                let blocker_msg = blocker_message
                    .clone()
                    .unwrap_or_else(|| format!("Awaiting {} {}", request_type, request_subtype));

                sqlx::query!(
                    r#"
                    UPDATE "ob-poc".entity_workstreams
                    SET status = 'BLOCKED',
                        blocker_type = $2,
                        blocker_request_id = $3,
                        blocker_message = $4,
                        blocked_at = NOW()
                    WHERE workstream_id = $1 AND status != 'BLOCKED'
                    "#,
                    ws_id,
                    format!("AWAITING_{}", request_type),
                    row.request_id,
                    blocker_msg
                )
                .execute(&pool)
                .await?;
            }
        }

        Ok(VerbExecutionOutcome::Record(json!({
            "request_id": row.request_id,
            "request_type": request_type,
            "request_subtype": request_subtype,
            "status": "PENDING",
            "due_date": due_date.to_string(),
            "blocks_subject": blocks,
            "subject": {
                "type": subject_type,
                "id": subject_id
            }
        })))
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Request Overdue Operation
// ═══════════════════════════════════════════════════════════════════════════════

/// List overdue requests
pub struct RequestOverdue;

#[async_trait]
impl SemOsVerbOp for RequestOverdue {
    fn fqn(&self) -> &str {
        "request.overdue"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let pool = scope.pool().clone();

        let case_id = json_extract_uuid_opt(args, ctx, "case-id");

        let cbu_id = json_extract_uuid_opt(args, ctx, "cbu-id");

        let include_grace = json_extract_bool_opt(args, "include-grace-period").unwrap_or(false);

        let rows = sqlx::query!(
            r#"
            SELECT
                request_id,
                request_type,
                request_subtype,
                subject_type,
                subject_id,
                workstream_id,
                case_id,
                cbu_id,
                requested_from_label,
                requested_at,
                due_date,
                grace_period_days,
                status,
                reminder_count,
                escalation_level,
                blocker_message,
                CURRENT_DATE - due_date as days_overdue
            FROM "ob-poc".outstanding_requests
            WHERE status = 'PENDING'
              AND ($1::uuid IS NULL OR case_id = $1)
              AND ($2::uuid IS NULL OR cbu_id = $2)
              AND (
                  CASE WHEN $3 THEN
                      due_date + (grace_period_days || ' days')::interval < CURRENT_DATE
                  ELSE
                      due_date < CURRENT_DATE
                  END
              )
            ORDER BY due_date ASC
            "#,
            case_id,
            cbu_id,
            include_grace
        )
        .fetch_all(&pool)
        .await?;

        let results: Vec<serde_json::Value> = rows
            .into_iter()
            .map(|r| {
                json!({
                    "request_id": r.request_id,
                    "type": r.request_type,
                    "subtype": r.request_subtype,
                    "subject_type": r.subject_type,
                    "subject_id": r.subject_id,
                    "workstream_id": r.workstream_id,
                    "case_id": r.case_id,
                    "cbu_id": r.cbu_id,
                    "from": r.requested_from_label,
                    "requested_at": r.requested_at,
                    "due_date": r.due_date,
                    "days_overdue": r.days_overdue,
                    "grace_period_days": r.grace_period_days,
                    "reminder_count": r.reminder_count,
                    "escalation_level": r.escalation_level,
                    "blocker_message": r.blocker_message
                })
            })
            .collect();

        Ok(VerbExecutionOutcome::RecordSet(results))
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Request Fulfill Operation
// ═══════════════════════════════════════════════════════════════════════════════

/// Mark request as fulfilled
pub struct RequestFulfill;

#[async_trait]
impl SemOsVerbOp for RequestFulfill {
    fn fqn(&self) -> &str {
        "request.fulfill"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let pool = scope.pool().clone();

        let request_id = json_extract_uuid(args, ctx, "request-id")?;

        let fulfillment_type = json_extract_string_opt(args, "fulfillment-type");

        let reference_id = json_extract_uuid_opt(args, ctx, "reference-id");

        let reference_type = json_extract_string_opt(args, "reference-type");

        let notes = json_extract_string_opt(args, "notes");

        // Update the request
        let updated = sqlx::query!(
            r#"
            UPDATE "ob-poc".outstanding_requests
            SET status = 'FULFILLED',
                fulfilled_at = NOW(),
                fulfillment_type = COALESCE($2, 'MANUAL_ENTRY'),
                fulfillment_reference_id = $3,
                fulfillment_reference_type = $4,
                fulfillment_notes = $5
            WHERE request_id = $1 AND status = 'PENDING'
            RETURNING workstream_id, blocks_subject
            "#,
            request_id,
            fulfillment_type,
            reference_id,
            reference_type,
            notes
        )
        .fetch_optional(&pool)
        .await?;

        let Some(row) = updated else {
            return Err(anyhow!(
                "Request {} not found or not in PENDING status",
                request_id
            ));
        };

        // Try to unblock workstream if this was blocking
        let mut workstream_unblocked = false;
        if row.blocks_subject.unwrap_or(false) {
            if let Some(ws_id) = row.workstream_id {
                workstream_unblocked = try_unblock_workstream(ws_id, &pool).await?;
            }
        }

        Ok(VerbExecutionOutcome::Record(json!({
            "request_id": request_id,
            "status": "FULFILLED",
            "workstream_unblocked": workstream_unblocked
        })))
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Request Cancel Operation
// ═══════════════════════════════════════════════════════════════════════════════

/// Cancel a pending request
pub struct RequestCancel;

#[async_trait]
impl SemOsVerbOp for RequestCancel {
    fn fqn(&self) -> &str {
        "request.cancel"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let pool = scope.pool().clone();

        let request_id = json_extract_uuid(args, ctx, "request-id")?;

        let reason = json_extract_string(args, "reason")?;

        let updated = sqlx::query!(
            r#"
            UPDATE "ob-poc".outstanding_requests
            SET status = 'CANCELLED',
                status_reason = $2
            WHERE request_id = $1 AND status = 'PENDING'
            RETURNING workstream_id, blocks_subject, case_id
            "#,
            request_id,
            reason
        )
        .fetch_optional(&pool)
        .await?;

        let Some(row) = updated else {
            return Err(anyhow!(
                "Request {} not found or not in PENDING status",
                request_id
            ));
        };

        // Try to unblock workstream
        if row.blocks_subject.unwrap_or(false) {
            if let Some(ws_id) = row.workstream_id {
                try_unblock_workstream(ws_id, &pool).await?;
            }
        }

        // Best-effort BPMN signal for active workflow correlation
        try_send_bpmn_signal(
            row.case_id,
            "request_cancelled",
            &json!({
                "request_id": request_id,
                "reason": reason,
            }),
            &pool,
        )
        .await;

        Ok(VerbExecutionOutcome::Affected(1))
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Request Extend Operation
// ═══════════════════════════════════════════════════════════════════════════════

/// Extend request due date
pub struct RequestExtend;

#[async_trait]
impl SemOsVerbOp for RequestExtend {
    fn fqn(&self) -> &str {
        "request.extend"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let pool = scope.pool().clone();

        let request_id = json_extract_uuid(args, ctx, "request-id")?;

        let reason = json_extract_string(args, "reason")?;

        // Get new due date from either days or explicit date
        let days = json_extract_int_opt(args, "days");

        let new_due_date = json_extract_string_opt(args, "new-due-date")
            .and_then(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok());

        if days.is_none() && new_due_date.is_none() {
            return Err(anyhow!("Either days or new-due-date is required"));
        }

        // Calculate new due date
        let new_date = match (new_due_date, days) {
            (Some(date), _) => date,
            (None, Some(days_val)) => {
                // Get current due date and add days
                let current = sqlx::query_scalar!(
                    r#"SELECT due_date FROM "ob-poc".outstanding_requests WHERE request_id = $1"#,
                    request_id
                )
                .fetch_optional(&pool)
                .await?
                .flatten()
                .ok_or_else(|| anyhow!("Request {} not found", request_id))?;

                current
                    + Duration::days(days_val)
                        .to_std()
                        .ok()
                        .and_then(|d| chrono::Duration::from_std(d).ok())
                        .unwrap_or_else(|| chrono::Duration::days(days_val))
            }
            (None, None) => {
                // This case is already handled above with early return
                unreachable!("Either days or new-due-date is required - checked above")
            }
        };

        // Fetch case_id for BPMN signal routing
        let case_id = sqlx::query_scalar!(
            r#"SELECT case_id FROM "ob-poc".outstanding_requests WHERE request_id = $1"#,
            request_id
        )
        .fetch_optional(&pool)
        .await?
        .flatten();

        // Update with extension logged in communication_log
        let extension_log = json!({
            "timestamp": Utc::now(),
            "type": "EXTENSION",
            "reason": reason,
            "new_due_date": new_date.to_string()
        });

        let result = sqlx::query!(
            r#"
            UPDATE "ob-poc".outstanding_requests
            SET due_date = $2,
                communication_log = communication_log || $3::jsonb
            WHERE request_id = $1 AND status = 'PENDING'
            "#,
            request_id,
            new_date,
            extension_log
        )
        .execute(&pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(anyhow!(
                "Request {} not found or not in PENDING status",
                request_id
            ));
        }

        // Best-effort BPMN signal for active workflow correlation
        try_send_bpmn_signal(
            case_id,
            "request_extended",
            &json!({
                "request_id": request_id,
                "new_due_date": new_date.to_string(),
                "reason": reason,
            }),
            &pool,
        )
        .await;

        Ok(VerbExecutionOutcome::Affected(1))
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Request Remind Operation
// ═══════════════════════════════════════════════════════════════════════════════

/// Send reminder for pending request
pub struct RequestRemind;

#[async_trait]
impl SemOsVerbOp for RequestRemind {
    fn fqn(&self) -> &str {
        "request.remind"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let pool = scope.pool().clone();

        let request_id = json_extract_uuid(args, ctx, "request-id")?;

        let channel =
            json_extract_string_opt(args, "channel").unwrap_or_else(|| "EMAIL".to_string());

        let message = json_extract_string_opt(args, "message");

        // Check if we can send another reminder
        let current = sqlx::query!(
            r#"
            SELECT reminder_count, max_reminders, last_reminder_at, case_id
            FROM "ob-poc".outstanding_requests
            WHERE request_id = $1 AND status = 'PENDING'
            "#,
            request_id
        )
        .fetch_optional(&pool)
        .await?
        .ok_or_else(|| anyhow!("Request {} not found or not PENDING", request_id))?;

        if current.reminder_count.unwrap_or(0) >= current.max_reminders.unwrap_or(3) {
            return Err(anyhow!(
                "Maximum reminders ({}) already sent",
                current.max_reminders.unwrap_or(3)
            ));
        }

        let reminder_log = json!({
            "timestamp": Utc::now(),
            "type": "REMINDER",
            "channel": channel,
            "message": message,
            "triggered_by": "DSL"
        });

        sqlx::query!(
            r#"
            UPDATE "ob-poc".outstanding_requests
            SET last_reminder_at = NOW(),
                reminder_count = COALESCE(reminder_count, 0) + 1,
                communication_log = communication_log || $2::jsonb
            WHERE request_id = $1
            "#,
            request_id,
            reminder_log
        )
        .execute(&pool)
        .await?;

        // Best-effort BPMN signal for active workflow correlation
        try_send_bpmn_signal(
            current.case_id,
            "request_reminded",
            &json!({
                "request_id": request_id,
                "channel": channel,
                "reminder_count": current.reminder_count.unwrap_or(0) + 1,
            }),
            &pool,
        )
        .await;

        Ok(VerbExecutionOutcome::Affected(1))
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Request Escalate Operation
// ═══════════════════════════════════════════════════════════════════════════════

/// Escalate overdue request
pub struct RequestEscalate;

#[async_trait]
impl SemOsVerbOp for RequestEscalate {
    fn fqn(&self) -> &str {
        "request.escalate"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let pool = scope.pool().clone();

        let request_id = json_extract_uuid(args, ctx, "request-id")?;

        let escalate_to = json_extract_uuid_opt(args, ctx, "escalate-to");

        let reason = json_extract_string_opt(args, "reason");

        // Fetch case_id before update for BPMN signal routing
        let case_id = sqlx::query_scalar!(
            r#"SELECT case_id FROM "ob-poc".outstanding_requests WHERE request_id = $1"#,
            request_id
        )
        .fetch_optional(&pool)
        .await?
        .flatten();

        let result = sqlx::query!(
            r#"
            UPDATE "ob-poc".outstanding_requests
            SET status = 'ESCALATED',
                escalated_at = NOW(),
                escalation_level = COALESCE(escalation_level, 0) + 1,
                escalated_to_user_id = $2,
                escalation_reason = $3
            WHERE request_id = $1 AND status IN ('PENDING', 'ESCALATED')
            "#,
            request_id,
            escalate_to,
            reason
        )
        .execute(&pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(anyhow!(
                "Request {} not found or in terminal status",
                request_id
            ));
        }

        // Best-effort BPMN signal for active workflow correlation
        try_send_bpmn_signal(
            case_id,
            "request_escalated",
            &json!({
                "request_id": request_id,
                "escalate_to": escalate_to,
                "reason": reason,
            }),
            &pool,
        )
        .await;

        Ok(VerbExecutionOutcome::Affected(1))
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Request Waive Operation
// ═══════════════════════════════════════════════════════════════════════════════

/// Waive a request requirement
pub struct RequestWaive;

#[async_trait]
impl SemOsVerbOp for RequestWaive {
    fn fqn(&self) -> &str {
        "request.waive"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let pool = scope.pool().clone();

        let request_id = json_extract_uuid(args, ctx, "request-id")?;

        let reason = json_extract_string(args, "reason")?;

        let approved_by = json_extract_uuid(args, ctx, "approved-by")?;

        let updated = sqlx::query!(
            r#"
            UPDATE "ob-poc".outstanding_requests
            SET status = 'WAIVED',
                fulfilled_at = NOW(),
                fulfillment_type = 'WAIVER',
                fulfillment_notes = $2,
                fulfilled_by_user_id = $3
            WHERE request_id = $1 AND status = 'PENDING'
            RETURNING workstream_id, blocks_subject
            "#,
            request_id,
            reason,
            approved_by
        )
        .fetch_optional(&pool)
        .await?;

        let Some(row) = updated else {
            return Err(anyhow!(
                "Request {} not found or not in PENDING status",
                request_id
            ));
        };

        // Try to unblock workstream
        if row.blocks_subject.unwrap_or(false) {
            if let Some(ws_id) = row.workstream_id {
                try_unblock_workstream(ws_id, &pool).await?;
            }
        }

        Ok(VerbExecutionOutcome::Affected(1))
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Document Request Operation (convenience wrapper)
// ═══════════════════════════════════════════════════════════════════════════════

/// Request a document (creates outstanding request, fire-and-forget)
pub struct DocumentRequest;

#[async_trait]
impl SemOsVerbOp for DocumentRequest {
    fn fqn(&self) -> &str {
        "document.request"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let pool = scope.pool().clone();

        let doc_type = json_extract_string(args, "type")?;

        // Resolve subject (workstream > entity > case)
        let subject = resolve_document_subject(args, ctx, &pool).await?;

        // Get defaults from request_types
        let config = sqlx::query!(
            r#"
            SELECT default_due_days, default_grace_days, blocks_by_default, max_reminders, description
            FROM "ob-poc".request_types
            WHERE request_type = 'DOCUMENT' AND request_subtype = $1
            "#,
            doc_type
        )
        .fetch_optional(&pool)
        .await?;

        let default_due_days = config
            .as_ref()
            .and_then(|c| c.default_due_days)
            .unwrap_or(7);
        let default_grace_days = config
            .as_ref()
            .and_then(|c| c.default_grace_days)
            .unwrap_or(3);
        let blocks_by_default = config
            .as_ref()
            .and_then(|c| c.blocks_by_default)
            .unwrap_or(true);
        let max_reminders = config.as_ref().and_then(|c| c.max_reminders).unwrap_or(3);

        let due_in_days =
            json_extract_int_opt(args, "due-in-days").unwrap_or(default_due_days as i64);

        let due_date = (Utc::now() + Duration::days(due_in_days))
            .naive_utc()
            .date();

        let requested_from =
            json_extract_string_opt(args, "from").unwrap_or_else(|| "client".to_string());

        let notes = json_extract_string_opt(args, "notes");

        let blocker_message = format!(
            "Awaiting {} from {}",
            humanize_doc_type(&doc_type),
            requested_from
        );

        // Create the request
        let row = sqlx::query!(
            r#"
            INSERT INTO "ob-poc".outstanding_requests (
                subject_type, subject_id,
                workstream_id, case_id, cbu_id, entity_id,
                request_type, request_subtype, request_details,
                requested_from_type, requested_from_label,
                requested_by_agent,
                due_date, grace_period_days, max_reminders,
                blocks_subject, blocker_message,
                created_by_verb
            ) VALUES (
                $1, $2,
                $3, $4, $5, $6,
                'DOCUMENT', $7, $8,
                'CLIENT', $9,
                TRUE,
                $10, $11, $12,
                $13, $14,
                'document.request'
            )
            RETURNING request_id
            "#,
            subject.subject_type,
            subject.subject_id,
            subject.workstream_id,
            subject.case_id,
            subject.cbu_id,
            subject.entity_id,
            doc_type,
            json!({"notes": notes}),
            requested_from,
            due_date,
            default_grace_days,
            max_reminders,
            blocks_by_default,
            blocker_message,
        )
        .fetch_one(&pool)
        .await?;

        // If blocking and attached to workstream, update workstream status
        if blocks_by_default {
            if let Some(ws_id) = subject.workstream_id {
                sqlx::query!(
                    r#"
                    UPDATE "ob-poc".entity_workstreams
                    SET status = 'BLOCKED',
                        blocker_type = 'AWAITING_DOCUMENT',
                        blocker_request_id = $2,
                        blocker_message = $3,
                        blocked_at = NOW()
                    WHERE workstream_id = $1 AND status != 'BLOCKED'
                    "#,
                    ws_id,
                    row.request_id,
                    blocker_message
                )
                .execute(&pool)
                .await?;
            }
        }

        Ok(VerbExecutionOutcome::Record(json!({
            "request_id": row.request_id,
            "request_type": "DOCUMENT",
            "request_subtype": doc_type,
            "status": "PENDING",
            "due_date": due_date.to_string(),
            "blocks_subject": blocks_by_default,
            "subject": {
                "type": subject.subject_type,
                "id": subject.subject_id
            }
        })))
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Document Upload Operation (auto-fulfillment)
// ═══════════════════════════════════════════════════════════════════════════════

/// Upload a document (auto-fulfills matching outstanding request)
pub struct DocumentUpload;

#[async_trait]
impl SemOsVerbOp for DocumentUpload {
    fn fqn(&self) -> &str {
        "document.upload"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let pool = scope.pool().clone();

        let doc_type = json_extract_string(args, "type")?;

        let file_path = json_extract_string(args, "file-path")?;

        let notes = json_extract_string_opt(args, "notes");

        // Resolve subject
        let subject = resolve_document_subject(args, ctx, &pool).await?;

        // Store the document in catalog
        let document_id = sqlx::query_scalar!(
            r#"
            INSERT INTO "ob-poc".document_catalog (
                cbu_id,
                document_type_code,
                document_name,
                storage_key,
                status,
                metadata
            ) VALUES (
                $1,
                $2,
                $3,
                $4,
                'active',
                $5
            )
            RETURNING doc_id
            "#,
            subject.cbu_id,
            doc_type,
            format!("{} - {}", doc_type, file_path),
            file_path,
            json!({"notes": notes, "uploaded_via": "document.upload"})
        )
        .fetch_one(&pool)
        .await?;

        // Try to find and fulfill matching pending request
        let fulfilled_request = sqlx::query!(
            r#"
            UPDATE "ob-poc".outstanding_requests
            SET status = 'FULFILLED',
                fulfilled_at = NOW(),
                fulfillment_type = 'DOCUMENT_UPLOAD',
                fulfillment_reference_type = 'DOCUMENT',
                fulfillment_reference_id = $3
            WHERE request_id = (
                SELECT request_id
                FROM "ob-poc".outstanding_requests
                WHERE request_type = 'DOCUMENT'
                  AND request_subtype = $2
                  AND status = 'PENDING'
                  AND (
                    (workstream_id = $1 AND $1 IS NOT NULL)
                    OR (entity_id = $4 AND $4 IS NOT NULL AND workstream_id IS NULL)
                    OR (case_id = $5 AND $5 IS NOT NULL AND workstream_id IS NULL AND entity_id IS NULL)
                  )
                ORDER BY requested_at ASC
                LIMIT 1
                FOR UPDATE SKIP LOCKED
            )
            RETURNING request_id, workstream_id, blocks_subject
            "#,
            subject.workstream_id,
            doc_type,
            document_id,
            subject.entity_id,
            subject.case_id
        )
        .fetch_optional(&pool)
        .await?;

        let mut workstream_unblocked = false;

        // If we fulfilled a request that was blocking a workstream, try to unblock
        if let Some(ref req) = fulfilled_request {
            if req.blocks_subject.unwrap_or(false) {
                if let Some(ws_id) = req.workstream_id {
                    workstream_unblocked = try_unblock_workstream(ws_id, &pool).await?;
                }
            }
        }

        Ok(VerbExecutionOutcome::Record(json!({
            "document_id": document_id,
            "document_type": doc_type,
            "fulfilled_request_id": fulfilled_request.as_ref().map(|r| r.request_id),
            "workstream_unblocked": workstream_unblocked,
            "subject": {
                "type": subject.subject_type,
                "id": subject.subject_id
            }
        })))
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Document Waive Request Operation
// ═══════════════════════════════════════════════════════════════════════════════

/// Waive document requirement (for outstanding requests)
pub struct DocumentWaive;

#[async_trait]
impl SemOsVerbOp for DocumentWaive {
    fn fqn(&self) -> &str {
        "document.waive-request"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let pool = scope.pool().clone();

        let workstream_id = json_extract_uuid(args, ctx, "workstream-id")?;

        let doc_type = json_extract_string(args, "type")?;

        let reason = json_extract_string(args, "reason")?;

        let approved_by = json_extract_uuid(args, ctx, "approved-by")?;

        // Find and waive matching request
        let updated = sqlx::query!(
            r#"
            UPDATE "ob-poc".outstanding_requests
            SET status = 'WAIVED',
                fulfilled_at = NOW(),
                fulfillment_type = 'WAIVER',
                fulfillment_notes = $3,
                fulfilled_by_user_id = $4
            WHERE request_type = 'DOCUMENT'
              AND request_subtype = $2
              AND workstream_id = $1
              AND status = 'PENDING'
            RETURNING request_id, blocks_subject
            "#,
            workstream_id,
            doc_type,
            reason,
            approved_by
        )
        .fetch_optional(&pool)
        .await?;

        let Some(row) = updated else {
            return Err(anyhow!(
                "No pending DOCUMENT request for type {} on workstream {}",
                doc_type,
                workstream_id
            ));
        };

        // Try to unblock workstream
        if row.blocks_subject.unwrap_or(false) {
            try_unblock_workstream(workstream_id, &pool).await?;
        }

        Ok(VerbExecutionOutcome::Affected(1))
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Helper Functions
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(feature = "database")]
async fn derive_linked_ids(
    subject_type: &str,
    subject_id: Uuid,
    pool: &PgPool,
) -> Result<LinkedIds> {
    match subject_type {
        "WORKSTREAM" => {
            let row = sqlx::query!(
                r#"
                SELECT w.workstream_id, w.entity_id, c.case_id, c.cbu_id
                FROM "ob-poc".entity_workstreams w
                JOIN "ob-poc".cases c ON w.case_id = c.case_id
                WHERE w.workstream_id = $1
                "#,
                subject_id
            )
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| anyhow!("Workstream {} not found", subject_id))?;

            Ok(LinkedIds {
                workstream_id: Some(row.workstream_id),
                case_id: Some(row.case_id),
                cbu_id: Some(row.cbu_id),
                entity_id: Some(row.entity_id),
            })
        }
        "KYC_CASE" => {
            let row = sqlx::query!(
                r#"SELECT case_id, cbu_id FROM "ob-poc".cases WHERE case_id = $1"#,
                subject_id
            )
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| anyhow!("Case {} not found", subject_id))?;

            Ok(LinkedIds {
                workstream_id: None,
                case_id: Some(row.case_id),
                cbu_id: Some(row.cbu_id),
                entity_id: None,
            })
        }
        "ENTITY" => Ok(LinkedIds {
            entity_id: Some(subject_id),
            ..Default::default()
        }),
        "CBU" => Ok(LinkedIds {
            cbu_id: Some(subject_id),
            ..Default::default()
        }),
        _ => Err(anyhow!("Unknown subject_type: {}", subject_type)),
    }
}

#[cfg(feature = "database")]
async fn resolve_document_subject(
    args: &serde_json::Value,
    ctx: &VerbExecutionContext,
    pool: &PgPool,
) -> Result<DocumentSubject> {
    // Try workstream first
    if let Some(ws_id) = json_extract_uuid_opt(args, ctx, "workstream-id") {
        let row = sqlx::query!(
            r#"
            SELECT w.workstream_id, w.entity_id, c.case_id, c.cbu_id
            FROM "ob-poc".entity_workstreams w
            JOIN "ob-poc".cases c ON w.case_id = c.case_id
            WHERE w.workstream_id = $1
            "#,
            ws_id
        )
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| anyhow!("Workstream {} not found", ws_id))?;

        return Ok(DocumentSubject {
            subject_type: "WORKSTREAM".to_string(),
            subject_id: ws_id,
            workstream_id: Some(row.workstream_id),
            case_id: Some(row.case_id),
            cbu_id: Some(row.cbu_id),
            entity_id: Some(row.entity_id),
        });
    }

    // Try entity
    if let Some(entity_id) = json_extract_uuid_opt(args, ctx, "entity-id") {
        return Ok(DocumentSubject {
            subject_type: "ENTITY".to_string(),
            subject_id: entity_id,
            entity_id: Some(entity_id),
            ..Default::default()
        });
    }

    // Try case
    if let Some(case_id) = json_extract_uuid_opt(args, ctx, "case-id") {
        let row = sqlx::query!(
            r#"SELECT case_id, cbu_id FROM "ob-poc".cases WHERE case_id = $1"#,
            case_id
        )
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| anyhow!("Case {} not found", case_id))?;

        return Ok(DocumentSubject {
            subject_type: "KYC_CASE".to_string(),
            subject_id: case_id,
            case_id: Some(row.case_id),
            cbu_id: Some(row.cbu_id),
            ..Default::default()
        });
    }

    Err(anyhow!(
        "One of workstream-id, entity-id, or case-id is required"
    ))
}

#[cfg(feature = "database")]
async fn try_unblock_workstream(workstream_id: Uuid, pool: &PgPool) -> Result<bool> {
    // Check if there are any remaining blocking requests
    let remaining_blockers = sqlx::query_scalar!(
        r#"
        SELECT COUNT(*) as "count!"
        FROM "ob-poc".outstanding_requests
        WHERE workstream_id = $1
          AND status = 'PENDING'
          AND blocks_subject = TRUE
        "#,
        workstream_id
    )
    .fetch_one(pool)
    .await?;

    if remaining_blockers == 0 {
        // No more blockers, unblock workstream
        let result = sqlx::query!(
            r#"
            UPDATE "ob-poc".entity_workstreams
            SET status = CASE
                    WHEN status = 'BLOCKED' THEN 'COLLECT'
                    ELSE status
                END,
                blocker_type = NULL,
                blocker_request_id = NULL,
                blocker_message = NULL
            WHERE workstream_id = $1 AND status = 'BLOCKED'
            "#,
            workstream_id
        )
        .execute(pool)
        .await?;

        return Ok(result.rows_affected() > 0);
    }

    Ok(false)
}

/// Best-effort BPMN signal routing for lifecycle request operations.
///
/// Phase F.1c (2026-04-22, Pattern B §3.4): the direct gRPC call was
/// removed. This helper now:
///
/// 1. Reads the active BPMN correlation for the case (DB read, fine
///    inside the ambient txn).
/// 2. Inserts a `bpmn_signal` row into `public.outbox` (DB write, same
///    txn). The existing `BpmnSignalConsumer` drainer (registered
///    alongside `MaintenanceSpawnConsumer` in ob-poc-web::main) performs
///    the actual gRPC call post-commit.
/// 3. Appends the audit entry to `outstanding_requests.communication_log`
///    (DB write, same txn).
///
/// Net effect: zero gRPC/HTTP inside the verb body. If the outer txn
/// rolls back, the outbox row is gone with it — no orphaned BPMN
/// signals. Same atomicity + idempotency contract as `BpmnSignal::execute`.
#[cfg(feature = "database")]
async fn try_send_bpmn_signal(
    case_id: Option<Uuid>,
    signal_name: &str,
    payload: &serde_json::Value,
    pool: &PgPool,
) {
    use crate::bpmn_integration::correlation::CorrelationStore;

    let Some(case_id) = case_id else {
        return; // No case context, skip BPMN lookup
    };

    let store = CorrelationStore::new(pool.clone());
    let correlation = match store
        .find_active_by_domain_key("kyc-open-case", &case_id.to_string())
        .await
    {
        Ok(Some(c)) => c,
        Ok(None) => {
            tracing::debug!(
                case_id = %case_id,
                signal = signal_name,
                "No active BPMN correlation for case, skipping signal"
            );
            return;
        }
        Err(e) => {
            tracing::warn!(
                case_id = %case_id,
                signal = signal_name,
                error = %e,
                "Failed to query BPMN correlation, skipping signal"
            );
            return;
        }
    };

    // Defer the actual signal via public.outbox — same pattern as
    // `BpmnSignal::execute` uses. Idempotency key collapses duplicate
    // signals from a single txn.
    let outbox_id = uuid::Uuid::new_v4();
    let trace_id = uuid::Uuid::new_v4();
    let payload_bytes = serde_json::to_vec(payload).unwrap_or_default();

    let mut hasher = blake3::Hasher::new();
    hasher.update(signal_name.as_bytes());
    hasher.update(b"\x00");
    hasher.update(&payload_bytes);
    let payload_hash = hasher.finalize().to_hex().to_string();
    let idempotency_key = format!(
        "bpmn_signal:{}:{}:{}",
        correlation.process_instance_id,
        signal_name,
        &payload_hash[..16]
    );

    let outbox_payload = serde_json::json!({
        "instance_id": correlation.process_instance_id,
        "message_name": signal_name,
        "payload": serde_json::to_string(payload).unwrap_or_default(),
    });

    if let Err(e) = sqlx::query(
        r#"
        INSERT INTO public.outbox
            (id, trace_id, envelope_version, effect_kind, payload, idempotency_key, status)
        VALUES
            ($1, $2, $3, $4, $5, $6, 'pending')
        ON CONFLICT (idempotency_key, effect_kind) DO NOTHING
        "#,
    )
    .bind(outbox_id)
    .bind(trace_id)
    .bind(1i16)
    .bind("bpmn_signal")
    .bind(&outbox_payload)
    .bind(&idempotency_key)
    .execute(pool)
    .await
    {
        tracing::warn!(
            case_id = %case_id,
            signal = signal_name,
            error = %e,
            "Failed to queue bpmn_signal in public.outbox (non-blocking)"
        );
    } else {
        tracing::info!(
            case_id = %case_id,
            process_instance_id = %correlation.process_instance_id,
            signal = signal_name,
            %idempotency_key,
            "bpmn.signal queued to public.outbox for request lifecycle event"
        );
    }

    // Record the signal in the communication_log for audit regardless
    // of outbox status. This is the same pre-F.1c audit trail, now
    // reflecting "queued" instead of "sent" — the drainer updates the
    // outbox row when the actual gRPC call completes.
    let signal_log = serde_json::json!({
        "timestamp": chrono::Utc::now(),
        "type": "BPMN_SIGNAL_QUEUED",
        "signal_name": signal_name,
        "process_instance_id": correlation.process_instance_id,
        "correlation_id": correlation.correlation_id,
        "outbox_id": outbox_id,
        "idempotency_key": idempotency_key,
        "payload": payload,
    });

    let _ = sqlx::query!(
        r#"
        UPDATE "ob-poc".outstanding_requests
        SET communication_log = communication_log || $2::jsonb
        WHERE request_id = (
            SELECT request_id FROM "ob-poc".outstanding_requests
            WHERE case_id = $1 AND status = 'PENDING'
            LIMIT 1
        )
        "#,
        case_id,
        signal_log,
    )
    .execute(pool)
    .await;
}

fn humanize_doc_type(doc_type: &str) -> String {
    doc_type
        .replace('_', " ")
        .to_lowercase()
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().chain(chars).collect(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
