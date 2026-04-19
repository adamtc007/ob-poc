//! Outreach Operations
//!
//! Plugin handlers for counterparty outreach tracking.
//! Most operations use CRUD via YAML. These handlers implement
//! complex logic for response recording and overdue tracking.

use anyhow::Result;
use async_trait::async_trait;
use dsl_runtime_macros::register_custom_op;
use serde_json::json;
use uuid::Uuid;

use super::{CustomOperation, ExecutionResult};
use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::executor::ExecutionContext;

#[cfg(feature = "database")]
use sqlx::PgPool;

// ── Shared _impl functions (called by both execute and execute_json) ──

#[cfg(feature = "database")]
async fn outreach_record_response_impl(
    request_id: Uuid,
    response_type: &str,
    document_id: Option<Uuid>,
    notes: Option<&str>,
    pool: &PgPool,
) -> Result<serde_json::Value> {
    use sqlx::Row;

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
    .bind(response_type)
    .bind(document_id)
    .bind(notes)
    .fetch_one(pool)
    .await?;

    Ok(json!({
        "request_id": row.get::<Uuid, _>("request_id"),
        "target_entity_id": row.get::<Uuid, _>("target_entity_id"),
        "request_type": row.get::<String, _>("request_type"),
        "response_type": response_type,
        "status": "responded"
    }))
}

#[cfg(feature = "database")]
async fn outreach_list_overdue_impl(
    days_overdue: i32,
    pool: &PgPool,
) -> Result<Vec<serde_json::Value>> {
    use sqlx::Row;

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
    .fetch_all(pool)
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
                "days_past_deadline": r.get::<Option<i32>, _>("days_past_deadline")
            })
        })
        .collect();

    Ok(results)
}

// =============================================================================
// RECORD RESPONSE
// =============================================================================

/// Record response to an outreach request
///
/// Rationale: Updates multiple fields and may trigger follow-up actions
#[register_custom_op]
pub struct OutreachRecordResponseOp;

#[async_trait]
impl CustomOperation for OutreachRecordResponseOp {
    fn domain(&self) -> &'static str {
        "research.outreach"
    }

    fn verb(&self) -> &'static str {
        "record-response"
    }

    fn rationale(&self) -> &'static str {
        "Updates request status, records response details, and links documents"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let request_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "request-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing request-id argument"))?;

        let response_type = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "response-type")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing response-type argument"))?;

        let document_id: Option<Uuid> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "document-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            });

        let notes = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "notes")
            .and_then(|a| a.value.as_string());

        let result =
            outreach_record_response_impl(request_id, response_type, document_id, notes, pool)
                .await?;

        Ok(ExecutionResult::Record(result))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(json!({"status": "responded"})))
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use super::helpers::{
            json_extract_string, json_extract_string_opt, json_extract_uuid, json_extract_uuid_opt,
        };

        let request_id = json_extract_uuid(args, ctx, "request-id")?;
        let response_type = json_extract_string(args, "response-type")?;
        let document_id = json_extract_uuid_opt(args, ctx, "document-id");
        let notes = json_extract_string_opt(args, "notes");

        let result = outreach_record_response_impl(
            request_id,
            &response_type,
            document_id,
            notes.as_deref(),
            pool,
        )
        .await?;

        Ok(dsl_runtime::VerbExecutionOutcome::Record(result))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

// =============================================================================
// LIST OVERDUE
// =============================================================================

/// List overdue outreach requests
///
/// Rationale: Calculates overdue based on deadline_date and current date
#[register_custom_op]
pub struct OutreachListOverdueOp;

#[async_trait]
impl CustomOperation for OutreachListOverdueOp {
    fn domain(&self) -> &'static str {
        "research.outreach"
    }

    fn verb(&self) -> &'static str {
        "list-overdue"
    }

    fn rationale(&self) -> &'static str {
        "Calculates days overdue and returns filtered list"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let days_overdue = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "days-overdue")
            .and_then(|a| a.value.as_integer())
            .unwrap_or(0) as i32;

        let results = outreach_list_overdue_impl(days_overdue, pool).await?;
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

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        _ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use super::helpers::json_extract_int_opt;

        let days_overdue = json_extract_int_opt(args, "days-overdue").unwrap_or(0) as i32;
        let results = outreach_list_overdue_impl(days_overdue, pool).await?;
        Ok(dsl_runtime::VerbExecutionOutcome::RecordSet(
            results,
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handler_metadata() {
        let record = OutreachRecordResponseOp;
        assert_eq!(record.domain(), "research.outreach");
        assert_eq!(record.verb(), "record-response");

        let overdue = OutreachListOverdueOp;
        assert_eq!(overdue.domain(), "research.outreach");
        assert_eq!(overdue.verb(), "list-overdue");
    }
}
