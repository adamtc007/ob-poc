//! RFI (Request for Information) Operations
//!
//! These operations manage document request workflows by working with
//! the existing kyc.doc_requests table, generating requests based on
//! threshold requirements.

use anyhow::Result;
use async_trait::async_trait;

use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::custom_ops::CustomOperation;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};

#[cfg(feature = "database")]
use sqlx::PgPool;

/// Generate doc_requests based on threshold requirements for a case
///
/// Rationale: Calls the SQL function that evaluates threshold requirements
/// and creates doc_requests for missing documents.
pub struct RfiGenerateOp;

#[async_trait]
impl CustomOperation for RfiGenerateOp {
    fn domain(&self) -> &'static str {
        "rfi"
    }
    fn verb(&self) -> &'static str {
        "generate"
    }
    fn rationale(&self) -> &'static str {
        "Calls SQL function to generate doc_requests from threshold requirements"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use serde_json::json;
        use uuid::Uuid;

        // Get case ID (required)
        let case_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "case-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing case-id argument"))?;

        // Get optional batch reference
        let batch_ref = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "batch-reference")
            .and_then(|a| a.value.as_string());

        // Call the SQL function to generate doc_requests
        let result = sqlx::query!(
            r#"SELECT batch_id, requests_created, entities_processed
               FROM kyc.generate_doc_requests_from_threshold($1, $2)"#,
            case_id,
            batch_ref
        )
        .fetch_one(pool)
        .await?;

        let batch_id = result.batch_id.unwrap_or_else(Uuid::new_v4);
        ctx.bind("batch", batch_id);

        let output = json!({
            "batch_id": batch_id,
            "case_id": case_id,
            "requests_created": result.requests_created,
            "entities_processed": result.entities_processed
        });

        Ok(ExecutionResult::Record(output))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::json!({
            "batch_id": uuid::Uuid::new_v4(),
            "requests_created": 0
        })))
    }
}

/// Check document completion status for a case
///
/// Rationale: Aggregates doc_request status across all workstreams in a case.
pub struct RfiCheckCompletionOp;

#[async_trait]
impl CustomOperation for RfiCheckCompletionOp {
    fn domain(&self) -> &'static str {
        "rfi"
    }
    fn verb(&self) -> &'static str {
        "check-completion"
    }
    fn rationale(&self) -> &'static str {
        "Aggregates doc_request completion status for a case"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use serde_json::json;
        use uuid::Uuid;

        let case_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "case-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing case-id argument"))?;

        let result = sqlx::query!(
            r#"SELECT total_requests, pending_requests, received_requests,
                      verified_requests, mandatory_pending, all_mandatory_complete
               FROM kyc.check_case_doc_completion($1)"#,
            case_id
        )
        .fetch_one(pool)
        .await?;

        let output = json!({
            "case_id": case_id,
            "total_requests": result.total_requests,
            "pending_requests": result.pending_requests,
            "received_requests": result.received_requests,
            "verified_requests": result.verified_requests,
            "mandatory_pending": result.mandatory_pending,
            "all_mandatory_complete": result.all_mandatory_complete
        });

        Ok(ExecutionResult::Record(output))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::json!({
            "all_mandatory_complete": false
        })))
    }
}

/// List doc_requests for a case with their status
///
/// Rationale: Joins workstreams to get all doc_requests for a case.
pub struct RfiListByCaseOp;

#[async_trait]
impl CustomOperation for RfiListByCaseOp {
    fn domain(&self) -> &'static str {
        "rfi"
    }
    fn verb(&self) -> &'static str {
        "list-by-case"
    }
    fn rationale(&self) -> &'static str {
        "Lists all doc_requests across workstreams for a case"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use serde_json::json;
        use uuid::Uuid;

        let case_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "case-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing case-id argument"))?;

        // Optional status filter
        let status_filter = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "status")
            .and_then(|a| a.value.as_string());

        let requests = sqlx::query!(
            r#"SELECT dr.request_id, dr.workstream_id, dr.doc_type, dr.status,
                      dr.is_mandatory, dr.due_date, dr.received_at, dr.verified_at,
                      dr.batch_reference, e.name as entity_name, e.entity_id
               FROM kyc.doc_requests dr
               JOIN kyc.entity_workstreams w ON w.workstream_id = dr.workstream_id
               JOIN "ob-poc".entities e ON e.entity_id = w.entity_id
               WHERE w.case_id = $1
               AND ($2::text IS NULL OR dr.status = $2)
               ORDER BY dr.is_mandatory DESC, dr.due_date ASC NULLS LAST"#,
            case_id,
            status_filter
        )
        .fetch_all(pool)
        .await?;

        let request_list: Vec<serde_json::Value> = requests
            .iter()
            .map(|r| {
                json!({
                    "request_id": r.request_id,
                    "workstream_id": r.workstream_id,
                    "entity_id": r.entity_id,
                    "entity_name": r.entity_name,
                    "doc_type": r.doc_type,
                    "status": r.status,
                    "is_mandatory": r.is_mandatory,
                    "due_date": r.due_date,
                    "received_at": r.received_at,
                    "verified_at": r.verified_at,
                    "batch_reference": r.batch_reference
                })
            })
            .collect();

        Ok(ExecutionResult::RecordSet(request_list))
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
