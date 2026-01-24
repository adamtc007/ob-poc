//! Outreach Operations
//!
//! Plugin handlers for counterparty outreach tracking.
//! Most operations use CRUD via YAML. These handlers implement
//! complex logic for response recording and overdue tracking.

use anyhow::Result;
use async_trait::async_trait;
use ob_poc_macros::register_custom_op;
use serde_json::json;
use uuid::Uuid;

use super::{CustomOperation, ExecutionResult};
use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::executor::ExecutionContext;

#[cfg(feature = "database")]
use sqlx::PgPool;

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

        // Update the request
        let result = sqlx::query!(
            r#"
            UPDATE kyc.outreach_requests
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
            request_id,
            response_type,
            document_id,
            notes
        )
        .fetch_one(pool)
        .await?;

        Ok(ExecutionResult::Record(json!({
            "request_id": result.request_id,
            "target_entity_id": result.target_entity_id,
            "request_type": result.request_type,
            "response_type": response_type,
            "status": "responded"
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(json!({"status": "responded"})))
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

        let requests = sqlx::query!(
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
            FROM kyc.outreach_requests r
            JOIN "ob-poc".entities e ON e.entity_id = r.target_entity_id
            WHERE r.status IN ('SENT', 'REMINDED', 'PENDING')
              AND r.deadline_date IS NOT NULL
              AND r.deadline_date < CURRENT_DATE - $1::integer
            ORDER BY r.deadline_date ASC
            "#,
            days_overdue
        )
        .fetch_all(pool)
        .await?;

        let results: Vec<_> = requests
            .iter()
            .map(|r| {
                json!({
                    "request_id": r.request_id,
                    "target_entity_id": r.target_entity_id,
                    "entity_name": r.entity_name,
                    "request_type": r.request_type,
                    "recipient_name": r.recipient_name,
                    "recipient_email": r.recipient_email,
                    "status": r.status,
                    "deadline_date": r.deadline_date,
                    "sent_at": r.sent_at,
                    "days_past_deadline": r.days_past_deadline
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
