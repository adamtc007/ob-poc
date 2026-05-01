//! `red-flag.*` plugin verbs from `rust/config/verbs/kyc/red-flag.yaml`.
//!
//! Currently a single op:
//! - `red-flag.escalate` — bumps the red flag's status and propagates the
//!   escalation to a linked entity_workstream (status → ENHANCED_DD) when
//!   one is attached. Mirrors the M-014 transition rules; the case-level
//!   side of the cascade (case → REFERRED) requires a `cases.status` enum
//!   extension and is deferred to a dedicated verb.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::{json, Value};
use uuid::Uuid;

use dsl_runtime::domain_ops::helpers::{json_extract_string, json_extract_uuid};
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::SemOsVerbOp;

pub struct Escalate;

#[async_trait]
impl SemOsVerbOp for Escalate {
    fn fqn(&self) -> &str {
        "red-flag.escalate"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let red_flag_id = json_extract_uuid(args, ctx, "red-flag-id")?;
        let reason = json_extract_string(args, "reason")?;

        let row: Option<(String, Option<Uuid>, Uuid)> = sqlx::query_as(
            r#"SELECT status, workstream_id, case_id
               FROM "ob-poc".red_flags
               WHERE red_flag_id = $1"#,
        )
        .bind(red_flag_id)
        .fetch_optional(scope.executor())
        .await?;
        let (current_status, workstream_id, case_id) =
            row.ok_or_else(|| anyhow!("Red flag not found: {}", red_flag_id))?;

        if !matches!(current_status.as_str(), "OPEN" | "UNDER_REVIEW") {
            return Err(anyhow!(
                "Red flag {} is in status '{}'; only OPEN or UNDER_REVIEW flags can be escalated",
                red_flag_id,
                current_status
            ));
        }

        let note = format!("[ESCALATED] {}", reason);
        let affected = sqlx::query(
            r#"UPDATE "ob-poc".red_flags
               SET status = 'UNDER_REVIEW',
                   reviewed_at = NOW(),
                   resolution_notes = COALESCE(resolution_notes || E'\n' || $2, $2)
               WHERE red_flag_id = $1"#,
        )
        .bind(red_flag_id)
        .bind(&note)
        .execute(scope.executor())
        .await?
        .rows_affected();

        let mut workstream_escalated = false;
        if let Some(wsid) = workstream_id {
            let ws_updated = sqlx::query(
                r#"UPDATE "ob-poc".entity_workstreams
                   SET status = 'ENHANCED_DD',
                       requires_enhanced_dd = true,
                       updated_at = NOW()
                   WHERE workstream_id = $1
                     AND status NOT IN ('COMPLETE', 'BLOCKED', 'PROHIBITED')"#,
            )
            .bind(wsid)
            .execute(scope.executor())
            .await?
            .rows_affected();
            workstream_escalated = ws_updated > 0;
        }

        let to_node = if workstream_escalated {
            "red-flag:escalated-enhanced-dd"
        } else {
            "red-flag:escalated"
        };
        let advance_reason = format!(
            "red-flag.escalate — {} → UNDER_REVIEW (workstream_escalated={})",
            current_status, workstream_escalated
        );
        dsl_runtime::domain_ops::helpers::emit_pending_state_advance(
            ctx,
            red_flag_id,
            to_node,
            "kyc-case/red-flag",
            &advance_reason,
        );

        Ok(VerbExecutionOutcome::Record(json!({
            "red_flag_id": red_flag_id,
            "case_id": case_id,
            "previous_status": current_status,
            "new_status": "UNDER_REVIEW",
            "workstream_escalated": workstream_escalated,
            "rows_affected": affected,
        })))
    }
}
