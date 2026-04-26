//! Evidence domain verbs (10 plugin verbs = 5 canonical +
//! 5 compatibility aliases) — YAML-first re-implementation of
//! `rust/config/verbs/evidence.yaml`.
//!
//! State machine: REQUIRED → RECEIVED → VERIFIED; reject clears
//! the document link and returns to REQUIRED-like; any state →
//! WAIVED. Shared runner functions + thin alias ops.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::{json, Value};
use uuid::Uuid;

use dsl_runtime::domain_ops::helpers::{
    json_extract_string, json_extract_string_opt, json_extract_uuid,
};
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::SemOsVerbOp;

async fn fetch_status(scope: &mut dyn TransactionScope, evidence_id: Uuid) -> Result<String> {
    let row: Option<(String,)> =
        sqlx::query_as(r#"SELECT status FROM "ob-poc".kyc_ubo_evidence WHERE evidence_id = $1"#)
            .bind(evidence_id)
            .fetch_optional(scope.executor())
            .await?;
    row.map(|(s,)| s)
        .ok_or_else(|| anyhow!("Evidence record not found: {}", evidence_id))
}

async fn do_require(
    args: &Value,
    ctx: &mut VerbExecutionContext,
    scope: &mut dyn TransactionScope,
) -> Result<VerbExecutionOutcome> {
    let registry_id = json_extract_uuid(args, ctx, "registry-id")?;
    let evidence_type = json_extract_string(args, "evidence-type")?;
    let description = json_extract_string_opt(args, "description");
    let doc_type = json_extract_string_opt(args, "doc-type");

    let evidence_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO "ob-poc".kyc_ubo_evidence
            (registry_id, evidence_type, description, doc_type, status)
        VALUES ($1, $2, $3, $4, 'REQUIRED')
        RETURNING evidence_id
        "#,
    )
    .bind(registry_id)
    .bind(&evidence_type)
    .bind(&description)
    .bind(&doc_type)
    .fetch_one(scope.executor())
    .await?;

    ctx.bind("evidence", evidence_id);
    Ok(VerbExecutionOutcome::Record(json!({
        "evidence_id": evidence_id,
        "registry_id": registry_id,
        "evidence_type": evidence_type,
        "status": "REQUIRED",
    })))
}

async fn do_link(
    args: &Value,
    ctx: &mut VerbExecutionContext,
    scope: &mut dyn TransactionScope,
) -> Result<VerbExecutionOutcome> {
    let evidence_id = json_extract_uuid(args, ctx, "evidence-id")?;
    let document_id = json_extract_uuid(args, ctx, "document-id")?;
    let current_status = fetch_status(scope, evidence_id).await?;
    if current_status != "REQUIRED" && current_status != "REJECTED" {
        return Err(anyhow!(
            "Cannot link document: evidence is in status '{}'. Only REQUIRED or REJECTED can have documents linked.",
            current_status
        ));
    }
    sqlx::query(
        r#"
        UPDATE "ob-poc".kyc_ubo_evidence
        SET document_id = $2, status = 'RECEIVED', updated_at = NOW()
        WHERE evidence_id = $1
        "#,
    )
    .bind(evidence_id)
    .bind(document_id)
    .execute(scope.executor())
    .await?;
    Ok(VerbExecutionOutcome::Record(json!({
        "evidence_id": evidence_id,
        "document_id": document_id,
        "previous_status": current_status,
        "new_status": "RECEIVED",
    })))
}

async fn do_verify(
    args: &Value,
    ctx: &mut VerbExecutionContext,
    scope: &mut dyn TransactionScope,
) -> Result<VerbExecutionOutcome> {
    let evidence_id = json_extract_uuid(args, ctx, "evidence-id")?;
    let verified_by = json_extract_string(args, "verified-by")?;
    let notes = json_extract_string_opt(args, "notes");
    let current_status = fetch_status(scope, evidence_id).await?;
    if current_status != "RECEIVED" {
        return Err(anyhow!(
            "Cannot verify: evidence is in status '{}'. Only RECEIVED can be verified.",
            current_status
        ));
    }
    let verified_at: chrono::DateTime<chrono::Utc> = sqlx::query_scalar(
        r#"
        UPDATE "ob-poc".kyc_ubo_evidence
        SET status = 'VERIFIED', verified_at = NOW(), verified_by = $2,
            notes = COALESCE($3, notes), updated_at = NOW()
        WHERE evidence_id = $1
        RETURNING verified_at
        "#,
    )
    .bind(evidence_id)
    .bind(&verified_by)
    .bind(&notes)
    .fetch_one(scope.executor())
    .await?;
    Ok(VerbExecutionOutcome::Record(json!({
        "evidence_id": evidence_id,
        "verified_by": verified_by,
        "verified_at": verified_at,
    })))
}

async fn do_reject(
    args: &Value,
    ctx: &mut VerbExecutionContext,
    scope: &mut dyn TransactionScope,
) -> Result<VerbExecutionOutcome> {
    let evidence_id = json_extract_uuid(args, ctx, "evidence-id")?;
    let reason = json_extract_string(args, "reason")?;
    let current_status = fetch_status(scope, evidence_id).await?;
    if current_status != "RECEIVED" {
        return Err(anyhow!(
            "Cannot reject: evidence is in status '{}'. Only RECEIVED can be rejected.",
            current_status
        ));
    }
    let previous_document_id: Option<Uuid> = sqlx::query_scalar(
        r#"SELECT document_id FROM "ob-poc".kyc_ubo_evidence WHERE evidence_id = $1"#,
    )
    .bind(evidence_id)
    .fetch_one(scope.executor())
    .await?;
    sqlx::query(
        r#"
        UPDATE "ob-poc".kyc_ubo_evidence
        SET status = 'REJECTED', document_id = NULL, notes = $2, updated_at = NOW()
        WHERE evidence_id = $1
        "#,
    )
    .bind(evidence_id)
    .bind(&reason)
    .execute(scope.executor())
    .await?;
    Ok(VerbExecutionOutcome::Record(json!({
        "evidence_id": evidence_id,
        "reason": reason,
        "previous_document_id": previous_document_id,
    })))
}

async fn do_waive(
    args: &Value,
    ctx: &mut VerbExecutionContext,
    scope: &mut dyn TransactionScope,
) -> Result<VerbExecutionOutcome> {
    let evidence_id = json_extract_uuid(args, ctx, "evidence-id")?;
    let reason = json_extract_string(args, "reason")?;
    let authority = json_extract_string(args, "authority")?;
    let _ = fetch_status(scope, evidence_id).await?;
    sqlx::query(
        r#"
        UPDATE "ob-poc".kyc_ubo_evidence
        SET status = 'WAIVED', waived_reason = $2, waived_by = $3, updated_at = NOW()
        WHERE evidence_id = $1
        "#,
    )
    .bind(evidence_id)
    .bind(&reason)
    .bind(&authority)
    .execute(scope.executor())
    .await?;
    Ok(VerbExecutionOutcome::Record(json!({
        "evidence_id": evidence_id,
        "reason": reason,
        "waived_by": authority,
    })))
}

// ── Canonical ops ─────────────────────────────────────────────────────────────

macro_rules! simple_evidence_op {
    ($struct:ident, $fqn:literal, $runner:ident) => {
        pub struct $struct;

        #[async_trait]
        impl SemOsVerbOp for $struct {
            fn fqn(&self) -> &str {
                $fqn
            }
            async fn execute(
                &self,
                args: &Value,
                ctx: &mut VerbExecutionContext,
                scope: &mut dyn TransactionScope,
            ) -> Result<VerbExecutionOutcome> {
                $runner(args, ctx, scope).await
            }
        }
    };
}

// YAML-mastered FQNs (config/verbs/kyc/evidence.yaml). The earlier
// `evidence.{require,link,verify,reject,waive}` canonicals were
// Rust-only orphans and have been removed.
simple_evidence_op!(CreateRequirement, "evidence.create-requirement", do_require);
simple_evidence_op!(AttachDocument, "evidence.attach-document", do_link);
simple_evidence_op!(MarkVerified, "evidence.mark-verified", do_verify);
simple_evidence_op!(MarkRejected, "evidence.mark-rejected", do_reject);
simple_evidence_op!(MarkWaived, "evidence.mark-waived", do_waive);
