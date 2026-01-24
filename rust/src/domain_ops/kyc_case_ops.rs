//! KYC Case Operations
//!
//! Plugin operations for KYC case state queries with domain-embedded requests.
//! Requests appear as child nodes of workstreams, not as separate lists.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use ob_poc_macros::register_custom_op;
use serde_json::{json, Value};
use sqlx::PgPool;

use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};

use super::helpers::extract_uuid;
use super::CustomOperation;

// ============================================================================
// KycCaseStateOp - Returns case with workstreams and embedded awaiting requests
// ============================================================================

/// Returns the full KYC case state with workstreams containing embedded
/// `awaiting` arrays. This is the domain-coherent view where requests are
/// child nodes of workstreams, not a separate list.
#[register_custom_op]
pub struct KycCaseStateOp;

#[async_trait]
impl CustomOperation for KycCaseStateOp {
    fn domain(&self) -> &'static str {
        "kyc-case"
    }

    fn verb(&self) -> &'static str {
        "state"
    }

    fn rationale(&self) -> &'static str {
        "Returns case with workstreams and embedded awaiting requests - requires complex join query"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let case_id = extract_uuid(verb_call, ctx, "case-id")?;

        // Load case with CBU info
        let case_row = sqlx::query!(
            r#"
            SELECT
                c.case_id,
                c.status,
                c.risk_rating,
                c.case_type,
                c.escalation_level,
                c.opened_at,
                c.sla_deadline,
                c.notes,
                cb.cbu_id,
                cb.name as cbu_name,
                cb.client_type
            FROM kyc.cases c
            JOIN "ob-poc".cbus cb ON c.cbu_id = cb.cbu_id
            WHERE c.case_id = $1
            "#,
            case_id
        )
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| anyhow!("Case not found: {}", case_id))?;

        // Load workstreams with entity info
        let workstreams = sqlx::query!(
            r#"
            SELECT
                w.workstream_id,
                w.entity_id,
                w.status,
                w.discovery_reason,
                w.is_ubo,
                w.ownership_percentage,
                w.risk_rating as ws_risk_rating,
                w.requires_enhanced_dd,
                w.blocker_type,
                w.blocker_message,
                e.name as entity_name
            FROM kyc.entity_workstreams w
            JOIN "ob-poc".entities e ON w.entity_id = e.entity_id
            WHERE w.case_id = $1
            ORDER BY w.created_at
            "#,
            case_id
        )
        .fetch_all(pool)
        .await?;

        // For each workstream, get entity role and awaiting requests
        let mut workstream_states = Vec::new();
        let mut total_awaiting = 0;
        let mut overdue_count = 0;

        for ws in &workstreams {
            // Get entity role in CBU context
            let role: Option<String> = sqlx::query_scalar!(
                r#"
                SELECT r.name
                FROM "ob-poc".cbu_entity_roles cer
                JOIN "ob-poc".roles r ON cer.role_id = r.role_id
                WHERE cer.entity_id = $1 AND cer.cbu_id = $2
                LIMIT 1
                "#,
                ws.entity_id,
                case_row.cbu_id
            )
            .fetch_optional(pool)
            .await?;

            // Get awaiting requests for this workstream
            let awaiting_rows = sqlx::query!(
                r#"
                SELECT
                    request_id,
                    request_type,
                    request_subtype,
                    requested_from_label,
                    requested_at,
                    due_date,
                    reminder_count,
                    escalation_level,
                    status,
                    blocker_message,
                    CURRENT_DATE - due_date as days_overdue
                FROM kyc.outstanding_requests
                WHERE workstream_id = $1 AND status IN ('PENDING', 'ESCALATED')
                ORDER BY due_date ASC
                "#,
                ws.workstream_id
            )
            .fetch_all(pool)
            .await?;

            let awaiting_nodes: Vec<Value> = awaiting_rows
                .iter()
                .map(|r| {
                    let days_over = r.days_overdue.unwrap_or(0).max(0);
                    let is_overdue = days_over > 0;

                    if is_overdue {
                        overdue_count += 1;
                    }

                    let mut actions = vec!["remind", "extend"];
                    if is_overdue {
                        actions.push("escalate");
                    }
                    actions.push("waive");

                    json!({
                        "request_id": r.request_id,
                        "type": r.request_type,
                        "subtype": r.request_subtype,
                        "from": r.requested_from_label,
                        "requested_at": r.requested_at,
                        "due_date": r.due_date,
                        "days_overdue": days_over,
                        "overdue": is_overdue,
                        "reminder_count": r.reminder_count,
                        "escalated": r.escalation_level.unwrap_or(0) > 0,
                        "actions": actions
                    })
                })
                .collect();

            total_awaiting += awaiting_nodes.len();

            workstream_states.push(json!({
                "workstream_id": ws.workstream_id,
                "entity": {
                    "entity_id": ws.entity_id,
                    "name": ws.entity_name,
                    "role": role
                },
                "status": ws.status,
                "is_ubo": ws.is_ubo,
                "ownership_percentage": ws.ownership_percentage,
                "risk_rating": ws.ws_risk_rating,
                "requires_enhanced_dd": ws.requires_enhanced_dd,
                "blocker_type": ws.blocker_type,
                "blocker_message": ws.blocker_message,
                "awaiting": awaiting_nodes
            }));
        }

        // Build summary
        let complete_count = workstream_states
            .iter()
            .filter(|w| w["status"] == "COMPLETE")
            .count();
        let blocked_count = workstream_states
            .iter()
            .filter(|w| w["status"] == "BLOCKED")
            .count();
        let in_progress_count = workstream_states
            .iter()
            .filter(|w| {
                let s = w["status"].as_str().unwrap_or("");
                s != "COMPLETE" && s != "BLOCKED" && s != "PENDING"
            })
            .count();

        // Build attention items from overdue/blocked workstreams
        let empty_vec = vec![];
        let attention: Vec<Value> = workstream_states
            .iter()
            .flat_map(|ws| {
                let awaiting = ws["awaiting"].as_array().unwrap_or(&empty_vec);
                awaiting
                    .iter()
                    .filter(|r| r["overdue"].as_bool().unwrap_or(false))
                    .map(|r| {
                        let days = r["days_overdue"].as_i64().unwrap_or(0);
                        let priority = if days > 7 { "HIGH" } else { "MEDIUM" };

                        json!({
                            "workstream": ws["workstream_id"],
                            "entity": ws["entity"]["name"],
                            "issue": format!("{} overdue {} days",
                                r["subtype"].as_str().unwrap_or("Request"),
                                days),
                            "priority": priority,
                            "actions": r["actions"]
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .collect();

        let result = json!({
            "case_id": case_row.case_id,
            "cbu": {
                "cbu_id": case_row.cbu_id,
                "name": case_row.cbu_name,
                "client_type": case_row.client_type
            },
            "status": case_row.status,
            "risk_rating": case_row.risk_rating,
            "case_type": case_row.case_type,
            "escalation_level": case_row.escalation_level,
            "opened_at": case_row.opened_at,
            "sla_deadline": case_row.sla_deadline,
            "workstreams": workstream_states,
            "summary": {
                "total_workstreams": workstream_states.len(),
                "complete": complete_count,
                "in_progress": in_progress_count,
                "blocked": blocked_count,
                "total_awaiting": total_awaiting,
                "overdue": overdue_count
            },
            "attention": attention
        });

        Ok(ExecutionResult::Record(result))
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

// ============================================================================
// WorkstreamStateOp - Returns single workstream with embedded awaiting
// ============================================================================

/// Returns a single workstream with its embedded awaiting requests.
#[register_custom_op]
pub struct WorkstreamStateOp;

#[async_trait]
impl CustomOperation for WorkstreamStateOp {
    fn domain(&self) -> &'static str {
        "entity-workstream"
    }

    fn verb(&self) -> &'static str {
        "state"
    }

    fn rationale(&self) -> &'static str {
        "Returns workstream with embedded awaiting requests, checks, and documents"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let workstream_id = extract_uuid(verb_call, ctx, "workstream-id")?;

        // Load workstream with entity info
        let ws = sqlx::query!(
            r#"
            SELECT
                w.workstream_id,
                w.case_id,
                w.entity_id,
                w.status,
                w.discovery_reason,
                w.is_ubo,
                w.ownership_percentage,
                w.risk_rating,
                w.requires_enhanced_dd,
                w.blocker_type,
                w.blocker_message,
                w.created_at,
                w.completed_at,
                e.name as entity_name,
                c.cbu_id
            FROM kyc.entity_workstreams w
            JOIN "ob-poc".entities e ON w.entity_id = e.entity_id
            JOIN kyc.cases c ON w.case_id = c.case_id
            WHERE w.workstream_id = $1
            "#,
            workstream_id
        )
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| anyhow!("Workstream not found: {}", workstream_id))?;

        // Get entity role
        let role: Option<String> = sqlx::query_scalar!(
            r#"
            SELECT r.name
            FROM "ob-poc".cbu_entity_roles cer
            JOIN "ob-poc".roles r ON cer.role_id = r.role_id
            WHERE cer.entity_id = $1 AND cer.cbu_id = $2
            LIMIT 1
            "#,
            ws.entity_id,
            ws.cbu_id
        )
        .fetch_optional(pool)
        .await?;

        // Get awaiting requests
        let awaiting_rows = sqlx::query!(
            r#"
            SELECT
                request_id,
                request_type,
                request_subtype,
                requested_from_label,
                requested_at,
                due_date,
                reminder_count,
                escalation_level,
                status,
                CURRENT_DATE - due_date as days_overdue
            FROM kyc.outstanding_requests
            WHERE workstream_id = $1 AND status IN ('PENDING', 'ESCALATED')
            ORDER BY due_date ASC
            "#,
            workstream_id
        )
        .fetch_all(pool)
        .await?;

        let awaiting: Vec<Value> = awaiting_rows
            .iter()
            .map(|r| {
                let days_over = r.days_overdue.unwrap_or(0).max(0);
                let is_overdue = days_over > 0;
                let mut actions = vec!["remind", "extend"];
                if is_overdue {
                    actions.push("escalate");
                }
                actions.push("waive");

                json!({
                    "request_id": r.request_id,
                    "type": r.request_type,
                    "subtype": r.request_subtype,
                    "from": r.requested_from_label,
                    "requested_at": r.requested_at,
                    "due_date": r.due_date,
                    "days_overdue": days_over,
                    "overdue": is_overdue,
                    "reminder_count": r.reminder_count,
                    "escalated": r.escalation_level.unwrap_or(0) > 0,
                    "actions": actions
                })
            })
            .collect();

        // Get completed screenings
        let screenings = sqlx::query!(
            r#"
            SELECT screening_type, status, completed_at
            FROM kyc.screenings
            WHERE workstream_id = $1 AND status NOT IN ('PENDING', 'RUNNING')
            ORDER BY completed_at DESC
            "#,
            workstream_id
        )
        .fetch_all(pool)
        .await?;

        let checks_complete: Vec<Value> = screenings
            .iter()
            .map(|s| {
                json!({
                    "type": s.screening_type,
                    "result": s.status,
                    "date": s.completed_at
                })
            })
            .collect();

        // Get received documents
        let documents = sqlx::query!(
            r#"
            SELECT d.document_type_code, d.doc_id, d.created_at
            FROM "ob-poc".document_catalog d
            JOIN kyc.outstanding_requests r ON d.doc_id = r.fulfillment_reference_id
            WHERE r.workstream_id = $1 AND r.status = 'FULFILLED'
            ORDER BY d.created_at DESC
            "#,
            workstream_id
        )
        .fetch_all(pool)
        .await?;

        let documents_received: Vec<Value> = documents
            .iter()
            .map(|d| {
                json!({
                    "type": d.document_type_code,
                    "document_id": d.doc_id,
                    "date": d.created_at
                })
            })
            .collect();

        let result = json!({
            "workstream_id": ws.workstream_id,
            "case_id": ws.case_id,
            "entity": {
                "entity_id": ws.entity_id,
                "name": ws.entity_name,
                "role": role
            },
            "status": ws.status,
            "is_ubo": ws.is_ubo,
            "ownership_percentage": ws.ownership_percentage,
            "risk_rating": ws.risk_rating,
            "requires_enhanced_dd": ws.requires_enhanced_dd,
            "blocker_type": ws.blocker_type,
            "blocker_message": ws.blocker_message,
            "awaiting": awaiting,
            "checks_complete": checks_complete,
            "documents_received": documents_received
        });

        Ok(ExecutionResult::Record(result))
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
