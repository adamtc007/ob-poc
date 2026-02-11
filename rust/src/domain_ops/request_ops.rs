//! Outstanding Request Operations
//!
//! Fire-and-forget request operations for document requests, verifications, etc.
//! These operations create requests that are tracked asynchronously and can
//! auto-fulfill when matching responses arrive.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use chrono::{Duration, NaiveDate, Utc};
use ob_poc_macros::register_custom_op;
use serde_json::json;
use uuid::Uuid;

use super::helpers::{extract_uuid, extract_uuid_opt};
use super::CustomOperation;
use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};

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
#[register_custom_op]
pub struct RequestCreateOp;

#[async_trait]
impl CustomOperation for RequestCreateOp {
    fn domain(&self) -> &'static str {
        "request"
    }

    fn verb(&self) -> &'static str {
        "create"
    }

    fn rationale(&self) -> &'static str {
        "Creates outstanding request with computed defaults from request_types config"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        // Extract required args
        let subject_type = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "subject-type")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow!("subject-type is required"))?;

        let subject_id = extract_uuid(verb_call, ctx, "subject-id")?;

        let request_type = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "type")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow!("type is required"))?;

        let request_subtype = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "subtype")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow!("subtype is required"))?;

        // Get defaults from request_types config
        let config = sqlx::query!(
            r#"
            SELECT default_due_days, default_grace_days, blocks_by_default, max_reminders
            FROM ob_ref.request_types
            WHERE request_type = $1 AND request_subtype = $2
            "#,
            request_type,
            request_subtype
        )
        .fetch_optional(pool)
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
        let due_in_days = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "due-in-days")
            .and_then(|a| a.value.as_integer())
            .unwrap_or(default_due_days as i64);

        let due_date: NaiveDate = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "due-date")
            .and_then(|a| a.value.as_string())
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
            .unwrap_or_else(|| {
                (Utc::now() + Duration::days(due_in_days))
                    .naive_utc()
                    .date()
            });

        let blocks = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "blocks")
            .and_then(|a| a.value.as_boolean())
            .unwrap_or(blocks_by_default);

        let blocker_message = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "message")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string());

        let requested_from_label = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "from")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string());

        let requested_from_entity_id = extract_uuid_opt(verb_call, ctx, "from-entity");

        let request_details: serde_json::Value = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "details")
            .and_then(|a| {
                if let Some(map) = a.value.as_map() {
                    let json_map: serde_json::Map<String, serde_json::Value> = map
                        .iter()
                        .filter_map(|(k, v)| {
                            v.as_string()
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
        let linked = derive_linked_ids(subject_type, subject_id, pool).await?;

        // Create the request
        let row = sqlx::query!(
            r#"
            INSERT INTO kyc.outstanding_requests (
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
        .fetch_one(pool)
        .await?;

        // If blocking and attached to workstream, update workstream status
        if blocks {
            if let Some(ws_id) = linked.workstream_id {
                let blocker_msg = blocker_message
                    .clone()
                    .unwrap_or_else(|| format!("Awaiting {} {}", request_type, request_subtype));

                sqlx::query!(
                    r#"
                    UPDATE kyc.entity_workstreams
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
                .execute(pool)
                .await?;
            }
        }

        Ok(ExecutionResult::Record(json!({
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

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("Database feature required"))
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Request Overdue Operation
// ═══════════════════════════════════════════════════════════════════════════════

/// List overdue requests
#[register_custom_op]
pub struct RequestOverdueOp;

#[async_trait]
impl CustomOperation for RequestOverdueOp {
    fn domain(&self) -> &'static str {
        "request"
    }

    fn verb(&self) -> &'static str {
        "overdue"
    }

    fn rationale(&self) -> &'static str {
        "Queries overdue requests with optional grace period consideration"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let case_id = extract_uuid_opt(verb_call, ctx, "case-id");

        let cbu_id = extract_uuid_opt(verb_call, ctx, "cbu-id");

        let include_grace = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "include-grace-period")
            .and_then(|a| a.value.as_boolean())
            .unwrap_or(false);

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
            FROM kyc.outstanding_requests
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
        .fetch_all(pool)
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

        Ok(ExecutionResult::RecordSet(results))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("Database feature required"))
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Request Fulfill Operation
// ═══════════════════════════════════════════════════════════════════════════════

/// Mark request as fulfilled
#[register_custom_op]
pub struct RequestFulfillOp;

#[async_trait]
impl CustomOperation for RequestFulfillOp {
    fn domain(&self) -> &'static str {
        "request"
    }

    fn verb(&self) -> &'static str {
        "fulfill"
    }

    fn rationale(&self) -> &'static str {
        "Fulfills request and potentially unblocks workstream"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let request_id = extract_uuid(verb_call, ctx, "request-id")?;

        let fulfillment_type = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "fulfillment-type")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string());

        let reference_id = extract_uuid_opt(verb_call, ctx, "reference-id");

        let reference_type = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "reference-type")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string());

        let notes = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "notes")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string());

        // Update the request
        let updated = sqlx::query!(
            r#"
            UPDATE kyc.outstanding_requests
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
        .fetch_optional(pool)
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
                workstream_unblocked = try_unblock_workstream(ws_id, pool).await?;
            }
        }

        Ok(ExecutionResult::Record(json!({
            "request_id": request_id,
            "status": "FULFILLED",
            "workstream_unblocked": workstream_unblocked
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("Database feature required"))
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Request Cancel Operation
// ═══════════════════════════════════════════════════════════════════════════════

/// Cancel a pending request
#[register_custom_op]
pub struct RequestCancelOp;

#[async_trait]
impl CustomOperation for RequestCancelOp {
    fn domain(&self) -> &'static str {
        "request"
    }

    fn verb(&self) -> &'static str {
        "cancel"
    }

    fn rationale(&self) -> &'static str {
        "Cancels request and potentially unblocks workstream"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let request_id = extract_uuid(verb_call, ctx, "request-id")?;

        let reason = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "reason")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow!("reason is required"))?;

        let updated = sqlx::query!(
            r#"
            UPDATE kyc.outstanding_requests
            SET status = 'CANCELLED',
                status_reason = $2
            WHERE request_id = $1 AND status = 'PENDING'
            RETURNING workstream_id, blocks_subject, case_id
            "#,
            request_id,
            reason
        )
        .fetch_optional(pool)
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
                try_unblock_workstream(ws_id, pool).await?;
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
            pool,
        )
        .await;

        Ok(ExecutionResult::Affected(1))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("Database feature required"))
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Request Extend Operation
// ═══════════════════════════════════════════════════════════════════════════════

/// Extend request due date
#[register_custom_op]
pub struct RequestExtendOp;

#[async_trait]
impl CustomOperation for RequestExtendOp {
    fn domain(&self) -> &'static str {
        "request"
    }

    fn verb(&self) -> &'static str {
        "extend"
    }

    fn rationale(&self) -> &'static str {
        "Extends due date with audit trail"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let request_id = extract_uuid(verb_call, ctx, "request-id")?;

        let reason = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "reason")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow!("reason is required"))?;

        // Get new due date from either days or explicit date
        let days = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "days")
            .and_then(|a| a.value.as_integer());

        let new_due_date = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "new-due-date")
            .and_then(|a| a.value.as_string())
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());

        if days.is_none() && new_due_date.is_none() {
            return Err(anyhow!("Either days or new-due-date is required"));
        }

        // Calculate new due date
        let new_date = match (new_due_date, days) {
            (Some(date), _) => date,
            (None, Some(days_val)) => {
                // Get current due date and add days
                let current = sqlx::query_scalar!(
                    r#"SELECT due_date FROM kyc.outstanding_requests WHERE request_id = $1"#,
                    request_id
                )
                .fetch_optional(pool)
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
            r#"SELECT case_id FROM kyc.outstanding_requests WHERE request_id = $1"#,
            request_id
        )
        .fetch_optional(pool)
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
            UPDATE kyc.outstanding_requests
            SET due_date = $2,
                communication_log = communication_log || $3::jsonb
            WHERE request_id = $1 AND status = 'PENDING'
            "#,
            request_id,
            new_date,
            extension_log
        )
        .execute(pool)
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
            pool,
        )
        .await;

        Ok(ExecutionResult::Affected(1))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("Database feature required"))
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Request Remind Operation
// ═══════════════════════════════════════════════════════════════════════════════

/// Send reminder for pending request
#[register_custom_op]
pub struct RequestRemindOp;

#[async_trait]
impl CustomOperation for RequestRemindOp {
    fn domain(&self) -> &'static str {
        "request"
    }

    fn verb(&self) -> &'static str {
        "remind"
    }

    fn rationale(&self) -> &'static str {
        "Records reminder with rate limiting"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let request_id = extract_uuid(verb_call, ctx, "request-id")?;

        let channel = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "channel")
            .and_then(|a| a.value.as_string())
            .unwrap_or("EMAIL");

        let message = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "message")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string());

        // Check if we can send another reminder
        let current = sqlx::query!(
            r#"
            SELECT reminder_count, max_reminders, last_reminder_at, case_id
            FROM kyc.outstanding_requests
            WHERE request_id = $1 AND status = 'PENDING'
            "#,
            request_id
        )
        .fetch_optional(pool)
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
            UPDATE kyc.outstanding_requests
            SET last_reminder_at = NOW(),
                reminder_count = COALESCE(reminder_count, 0) + 1,
                communication_log = communication_log || $2::jsonb
            WHERE request_id = $1
            "#,
            request_id,
            reminder_log
        )
        .execute(pool)
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
            pool,
        )
        .await;

        Ok(ExecutionResult::Affected(1))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("Database feature required"))
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Request Escalate Operation
// ═══════════════════════════════════════════════════════════════════════════════

/// Escalate overdue request
#[register_custom_op]
pub struct RequestEscalateOp;

#[async_trait]
impl CustomOperation for RequestEscalateOp {
    fn domain(&self) -> &'static str {
        "request"
    }

    fn verb(&self) -> &'static str {
        "escalate"
    }

    fn rationale(&self) -> &'static str {
        "Escalates request with level tracking"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let request_id = extract_uuid(verb_call, ctx, "request-id")?;

        let escalate_to = extract_uuid_opt(verb_call, ctx, "escalate-to");

        let reason = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "reason")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string());

        // Fetch case_id before update for BPMN signal routing
        let case_id = sqlx::query_scalar!(
            r#"SELECT case_id FROM kyc.outstanding_requests WHERE request_id = $1"#,
            request_id
        )
        .fetch_optional(pool)
        .await?
        .flatten();

        let result = sqlx::query!(
            r#"
            UPDATE kyc.outstanding_requests
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
        .execute(pool)
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
            pool,
        )
        .await;

        Ok(ExecutionResult::Affected(1))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("Database feature required"))
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Request Waive Operation
// ═══════════════════════════════════════════════════════════════════════════════

/// Waive a request requirement
#[register_custom_op]
pub struct RequestWaiveOp;

#[async_trait]
impl CustomOperation for RequestWaiveOp {
    fn domain(&self) -> &'static str {
        "request"
    }

    fn verb(&self) -> &'static str {
        "waive"
    }

    fn rationale(&self) -> &'static str {
        "Waives request with approval tracking and unblocks workstream"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let request_id = extract_uuid(verb_call, ctx, "request-id")?;

        let reason = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "reason")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow!("reason is required"))?;

        let approved_by = extract_uuid(verb_call, ctx, "approved-by")?;

        let updated = sqlx::query!(
            r#"
            UPDATE kyc.outstanding_requests
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
        .fetch_optional(pool)
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
                try_unblock_workstream(ws_id, pool).await?;
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
        Err(anyhow!("Database feature required"))
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Document Request Operation (convenience wrapper)
// ═══════════════════════════════════════════════════════════════════════════════

/// Request a document (creates outstanding request, fire-and-forget)
#[register_custom_op]
pub struct DocumentRequestOp;

#[async_trait]
impl CustomOperation for DocumentRequestOp {
    fn domain(&self) -> &'static str {
        "document"
    }

    fn verb(&self) -> &'static str {
        "request"
    }

    fn rationale(&self) -> &'static str {
        "Creates DOCUMENT type outstanding request with computed defaults"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let doc_type = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "type")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow!("type is required"))?;

        // Resolve subject (workstream > entity > case)
        let subject = resolve_document_subject(verb_call, ctx, pool).await?;

        // Get defaults from request_types
        let config = sqlx::query!(
            r#"
            SELECT default_due_days, default_grace_days, blocks_by_default, max_reminders, description
            FROM ob_ref.request_types
            WHERE request_type = 'DOCUMENT' AND request_subtype = $1
            "#,
            doc_type
        )
        .fetch_optional(pool)
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

        let due_in_days = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "due-in-days")
            .and_then(|a| a.value.as_integer())
            .unwrap_or(default_due_days as i64);

        let due_date = (Utc::now() + Duration::days(due_in_days))
            .naive_utc()
            .date();

        let requested_from = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "from")
            .and_then(|a| a.value.as_string())
            .unwrap_or("client");

        let notes = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "notes")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string());

        let blocker_message = format!(
            "Awaiting {} from {}",
            humanize_doc_type(doc_type),
            requested_from
        );

        // Create the request
        let row = sqlx::query!(
            r#"
            INSERT INTO kyc.outstanding_requests (
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
        .fetch_one(pool)
        .await?;

        // If blocking and attached to workstream, update workstream status
        if blocks_by_default {
            if let Some(ws_id) = subject.workstream_id {
                sqlx::query!(
                    r#"
                    UPDATE kyc.entity_workstreams
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
                .execute(pool)
                .await?;
            }
        }

        Ok(ExecutionResult::Record(json!({
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

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("Database feature required"))
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Document Upload Operation (auto-fulfillment)
// ═══════════════════════════════════════════════════════════════════════════════

/// Upload a document (auto-fulfills matching outstanding request)
#[register_custom_op]
pub struct DocumentUploadOp;

#[async_trait]
impl CustomOperation for DocumentUploadOp {
    fn domain(&self) -> &'static str {
        "document"
    }

    fn verb(&self) -> &'static str {
        "upload"
    }

    fn rationale(&self) -> &'static str {
        "Catalogs document and auto-fulfills matching pending request"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let doc_type = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "type")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow!("type is required"))?;

        let file_path = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "file-path")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow!("file-path is required"))?;

        let notes = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "notes")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string());

        // Resolve subject
        let subject = resolve_document_subject(verb_call, ctx, pool).await?;

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
        .fetch_one(pool)
        .await?;

        // Try to find and fulfill matching pending request
        let fulfilled_request = sqlx::query!(
            r#"
            UPDATE kyc.outstanding_requests
            SET status = 'FULFILLED',
                fulfilled_at = NOW(),
                fulfillment_type = 'DOCUMENT_UPLOAD',
                fulfillment_reference_type = 'DOCUMENT',
                fulfillment_reference_id = $3
            WHERE request_id = (
                SELECT request_id
                FROM kyc.outstanding_requests
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
        .fetch_optional(pool)
        .await?;

        let mut workstream_unblocked = false;

        // If we fulfilled a request that was blocking a workstream, try to unblock
        if let Some(ref req) = fulfilled_request {
            if req.blocks_subject.unwrap_or(false) {
                if let Some(ws_id) = req.workstream_id {
                    workstream_unblocked = try_unblock_workstream(ws_id, pool).await?;
                }
            }
        }

        Ok(ExecutionResult::Record(json!({
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

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("Database feature required"))
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Document Waive Request Operation
// ═══════════════════════════════════════════════════════════════════════════════

/// Waive document requirement (for outstanding requests)
#[register_custom_op]
pub struct DocumentWaiveOp;

#[async_trait]
impl CustomOperation for DocumentWaiveOp {
    fn domain(&self) -> &'static str {
        "document"
    }

    fn verb(&self) -> &'static str {
        "waive-request"
    }

    fn rationale(&self) -> &'static str {
        "Waives document request by type for a workstream"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let workstream_id = extract_uuid(verb_call, ctx, "workstream-id")?;

        let doc_type = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "type")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow!("type is required"))?;

        let reason = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "reason")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow!("reason is required"))?;

        let approved_by = extract_uuid(verb_call, ctx, "approved-by")?;

        // Find and waive matching request
        let updated = sqlx::query!(
            r#"
            UPDATE kyc.outstanding_requests
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
        .fetch_optional(pool)
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
            try_unblock_workstream(workstream_id, pool).await?;
        }

        Ok(ExecutionResult::Affected(1))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("Database feature required"))
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
                FROM kyc.entity_workstreams w
                JOIN kyc.cases c ON w.case_id = c.case_id
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
                r#"SELECT case_id, cbu_id FROM kyc.cases WHERE case_id = $1"#,
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
    verb_call: &VerbCall,
    ctx: &ExecutionContext,
    pool: &PgPool,
) -> Result<DocumentSubject> {
    // Try workstream first
    if let Some(ws_id) = extract_uuid_opt(verb_call, ctx, "workstream-id") {
        let row = sqlx::query!(
            r#"
            SELECT w.workstream_id, w.entity_id, c.case_id, c.cbu_id
            FROM kyc.entity_workstreams w
            JOIN kyc.cases c ON w.case_id = c.case_id
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
    if let Some(entity_id) = extract_uuid_opt(verb_call, ctx, "entity-id") {
        return Ok(DocumentSubject {
            subject_type: "ENTITY".to_string(),
            subject_id: entity_id,
            entity_id: Some(entity_id),
            ..Default::default()
        });
    }

    // Try case
    if let Some(case_id) = extract_uuid_opt(verb_call, ctx, "case-id") {
        let row = sqlx::query!(
            r#"SELECT case_id, cbu_id FROM kyc.cases WHERE case_id = $1"#,
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
        FROM kyc.outstanding_requests
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
            UPDATE kyc.entity_workstreams
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

/// Lazy BPMN gRPC client — created once from `BPMN_LITE_GRPC_URL` env var.
/// Returns `None` when the env var is not set (BPMN integration disabled).
#[cfg(feature = "database")]
fn bpmn_client() -> Option<&'static crate::bpmn_integration::client::BpmnLiteConnection> {
    use std::sync::OnceLock;
    static CLIENT: OnceLock<Option<crate::bpmn_integration::client::BpmnLiteConnection>> =
        OnceLock::new();
    CLIENT
        .get_or_init(|| {
            if std::env::var("BPMN_LITE_GRPC_URL").is_err() {
                tracing::debug!("BPMN_LITE_GRPC_URL not set — signal routing disabled");
                return None;
            }
            match crate::bpmn_integration::client::BpmnLiteConnection::from_env() {
                Ok(conn) => Some(conn),
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to create BPMN client for signal routing");
                    None
                }
            }
        })
        .as_ref()
}

/// Best-effort BPMN signal routing for lifecycle request operations.
///
/// When a request is reminded/cancelled/escalated/extended, check if the
/// associated case has an active BPMN correlation. If so, send a signal to
/// the BPMN process alongside the legacy `outstanding_requests` DB update.
///
/// This is additive — the legacy path always runs. BPMN signaling is best-effort:
/// if the BPMN infrastructure is unavailable or the case has no active correlation,
/// the function logs and returns without error.
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

    // Send gRPC signal to BPMN-Lite — best-effort, never fails the main operation.
    if let Some(client) = bpmn_client() {
        let payload_bytes = serde_json::to_vec(payload).unwrap_or_default();
        match client
            .signal(
                correlation.process_instance_id,
                signal_name,
                Some(&payload_bytes),
            )
            .await
        {
            Ok(()) => {
                tracing::info!(
                    case_id = %case_id,
                    process_instance_id = %correlation.process_instance_id,
                    signal = signal_name,
                    "BPMN signal sent for lifecycle event"
                );
            }
            Err(e) => {
                tracing::warn!(
                    case_id = %case_id,
                    process_instance_id = %correlation.process_instance_id,
                    signal = signal_name,
                    error = %e,
                    "BPMN signal failed (non-blocking)"
                );
            }
        }
    } else {
        tracing::debug!(
            case_id = %case_id,
            process_instance_id = %correlation.process_instance_id,
            signal = signal_name,
            "BPMN client not available, skipping signal"
        );
    }

    // Record the signal in the communication_log for audit regardless of send result
    let signal_log = serde_json::json!({
        "timestamp": chrono::Utc::now(),
        "type": "BPMN_SIGNAL",
        "signal_name": signal_name,
        "process_instance_id": correlation.process_instance_id,
        "correlation_id": correlation.correlation_id,
        "payload": payload,
    });

    // Best-effort audit — fire and forget
    let _ = sqlx::query!(
        r#"
        UPDATE kyc.outstanding_requests
        SET communication_log = communication_log || $2::jsonb
        WHERE request_id = (
            SELECT request_id FROM kyc.outstanding_requests
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
