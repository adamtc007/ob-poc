//! Outreach domain verbs (2 plugin verbs) — SemOS-side YAML-first
//! re-implementation of the plugin subset of
//! `rust/config/verbs/research/outreach.yaml`.
//!
//! `research.outreach.record-response` closes a counterparty
//! outreach request with a response type + optional document /
//! notes. `research.outreach.list-overdue` lists requests whose
//! deadline has slipped past the configured threshold. Every other
//! verb in the domain is `behavior: crud` and served by the CRUD
//! executor — no op needed here.

use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use sqlx::Row;
use uuid::Uuid;

use dsl_runtime::domain_ops::helpers::{
    json_extract_int_opt, json_extract_string, json_extract_string_opt, json_extract_uuid,
    json_extract_uuid_opt,
};
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::SemOsVerbOp;

// ── research.outreach.record-response ─────────────────────────────────────────

pub struct RecordResponse;

#[async_trait]
impl SemOsVerbOp for RecordResponse {
    fn fqn(&self) -> &str {
        "research.outreach.record-response"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let request_id = json_extract_uuid(args, ctx, "request-id")?;
        let response_type = json_extract_string(args, "response-type")?;
        let document_id = json_extract_uuid_opt(args, ctx, "document-id");
        let notes = json_extract_string_opt(args, "notes");

        let row = sqlx::query(
            r#"
            UPDATE "ob-poc".outreach_requests
            SET
                status = 'RESPONDED',
                response_type = $2,
                response_received_at = NOW(),
                response_document_id = $3,
                resolution_notes = COALESCE($4, resolution_notes),
                updated_at = NOW()
            WHERE request_id = $1
            RETURNING request_id, target_entity_id, request_type
            "#,
        )
        .bind(request_id)
        .bind(&response_type)
        .bind(document_id)
        .bind(notes.as_deref())
        .fetch_one(scope.executor())
        .await?;

        Ok(VerbExecutionOutcome::Record(json!({
            "request_id": row.get::<Uuid, _>("request_id"),
            "target_entity_id": row.get::<Uuid, _>("target_entity_id"),
            "request_type": row.get::<String, _>("request_type"),
            "response_type": response_type,
            "status": "responded",
        })))
    }
}

// ── research.outreach.list-overdue ────────────────────────────────────────────

pub struct ListOverdue;

#[async_trait]
impl SemOsVerbOp for ListOverdue {
    fn fqn(&self) -> &str {
        "research.outreach.list-overdue"
    }
    async fn execute(
        &self,
        args: &Value,
        _ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let days_overdue = json_extract_int_opt(args, "days-overdue").unwrap_or(0) as i32;

        let rows = sqlx::query(
            r#"
            SELECT
                r.request_id,
                r.target_entity_id,
                e.name AS entity_name,
                r.request_type,
                r.recipient_name,
                r.recipient_email,
                r.status,
                r.deadline_date,
                r.sent_at,
                (CURRENT_DATE - r.deadline_date) AS days_past_deadline
            FROM "ob-poc".outreach_requests r
            JOIN "ob-poc".entities e ON e.entity_id = r.target_entity_id
            WHERE r.status IN ('SENT', 'REMINDED', 'PENDING')
              AND e.deleted_at IS NULL
              AND r.deadline_date IS NOT NULL
              AND r.deadline_date < CURRENT_DATE - $1::integer
            ORDER BY r.deadline_date ASC
            "#,
        )
        .bind(days_overdue)
        .fetch_all(scope.executor())
        .await?;

        let results: Vec<_> = rows
            .iter()
            .map(|r| {
                json!({
                    "request_id": r.get::<Uuid, _>("request_id"),
                    "target_entity_id": r.get::<Uuid, _>("target_entity_id"),
                    "entity_name": r.get::<Option<String>, _>("entity_name"),
                    "request_type": r.get::<String, _>("request_type"),
                    "recipient_name": r.get::<Option<String>, _>("recipient_name"),
                    "recipient_email": r.get::<Option<String>, _>("recipient_email"),
                    "status": r.get::<String, _>("status"),
                    "deadline_date": r.get::<Option<chrono::NaiveDate>, _>("deadline_date"),
                    "sent_at": r.get::<Option<chrono::DateTime<chrono::Utc>>, _>("sent_at"),
                    "days_past_deadline": r.get::<Option<i32>, _>("days_past_deadline"),
                })
            })
            .collect();

        Ok(VerbExecutionOutcome::RecordSet(results))
    }
}
