//! Evidence custom operations for KYC UBO evidence management.
//!
//! Operations for managing evidence records in `"ob-poc".kyc_ubo_evidence` that
//! support the UBO registry lifecycle. Evidence entries track documents
//! and other artifacts required to prove beneficial ownership determinations.
//!
//! ## State Machine
//!
//! ```text
//! REQUIRED → RECEIVED → VERIFIED
//!                ↓
//!             REJECTED → (re-link → RECEIVED)
//!
//! Any non-terminal → WAIVED
//! ```
//!
//! ## Operations
//!
//! - `evidence.require` - Create new evidence requirement
//! - `evidence.link` - Link a document to evidence, status → RECEIVED
//! - `evidence.verify` - QA approves evidence, status → VERIFIED
//! - `evidence.reject` - QA rejects evidence, clears document link
//! - `evidence.waive` - Waive evidence requirement with authority

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use dsl_runtime_macros::register_custom_op;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::helpers::{
    extract_string, extract_string_opt, extract_uuid, json_extract_string, json_extract_string_opt,
    json_extract_uuid,
};
use super::CustomOperation;
use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};

#[cfg(feature = "database")]
use sqlx::PgPool;

// =============================================================================
// Result Types
// =============================================================================

/// Result of creating a new evidence requirement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceRequireResult {
    pub evidence_id: Uuid,
    pub registry_id: Uuid,
    pub evidence_type: String,
    pub status: String,
}

/// Result of linking a document to an evidence record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceLinkResult {
    pub evidence_id: Uuid,
    pub document_id: Uuid,
    pub previous_status: String,
    pub new_status: String,
}

/// Result of verifying an evidence record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceVerifyResult {
    pub evidence_id: Uuid,
    pub verified_by: String,
    pub verified_at: chrono::DateTime<chrono::Utc>,
}

/// Result of rejecting an evidence record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceRejectResult {
    pub evidence_id: Uuid,
    pub reason: String,
    pub previous_document_id: Option<Uuid>,
}

/// Result of waiving an evidence requirement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceWaiveResult {
    pub evidence_id: Uuid,
    pub reason: String,
    pub waived_by: String,
}

// =============================================================================
// Helpers
// =============================================================================

/// Fetch the current status of an evidence record. Returns an error if not found.
#[cfg(feature = "database")]
async fn fetch_evidence_status(pool: &PgPool, evidence_id: Uuid) -> Result<String> {
    let row: Option<(String,)> =
        sqlx::query_as(r#"SELECT status FROM "ob-poc".kyc_ubo_evidence WHERE evidence_id = $1"#)
            .bind(evidence_id)
            .fetch_optional(pool)
            .await?;

    row.map(|(s,)| s)
        .ok_or_else(|| anyhow!("Evidence record not found: {}", evidence_id))
}

#[cfg(feature = "database")]
async fn evidence_require_impl(
    registry_id: Uuid,
    evidence_type: String,
    description: Option<String>,
    doc_type: Option<String>,
    pool: &PgPool,
) -> Result<EvidenceRequireResult> {
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
    .fetch_one(pool)
    .await?;

    Ok(EvidenceRequireResult {
        evidence_id,
        registry_id,
        evidence_type,
        status: "REQUIRED".to_string(),
    })
}

#[cfg(feature = "database")]
async fn evidence_link_impl(
    evidence_id: Uuid,
    document_id: Uuid,
    pool: &PgPool,
) -> Result<EvidenceLinkResult> {
    let current_status = fetch_evidence_status(pool, evidence_id).await?;

    if current_status != "REQUIRED" && current_status != "REJECTED" {
        return Err(anyhow!(
            "Cannot link document: evidence is in status '{}'. \
             Only REQUIRED or REJECTED evidence can have documents linked.",
            current_status
        ));
    }

    sqlx::query(
        r#"
        UPDATE "ob-poc".kyc_ubo_evidence
        SET document_id = $2,
            status = 'RECEIVED',
            updated_at = NOW()
        WHERE evidence_id = $1
        "#,
    )
    .bind(evidence_id)
    .bind(document_id)
    .execute(pool)
    .await?;

    Ok(EvidenceLinkResult {
        evidence_id,
        document_id,
        previous_status: current_status,
        new_status: "RECEIVED".to_string(),
    })
}

#[cfg(feature = "database")]
async fn evidence_verify_impl(
    evidence_id: Uuid,
    verified_by: String,
    notes: Option<String>,
    pool: &PgPool,
) -> Result<EvidenceVerifyResult> {
    let current_status = fetch_evidence_status(pool, evidence_id).await?;

    if current_status != "RECEIVED" {
        return Err(anyhow!(
            "Cannot verify: evidence is in status '{}'. \
             Only RECEIVED evidence can be verified.",
            current_status
        ));
    }

    let verified_at: chrono::DateTime<chrono::Utc> = sqlx::query_scalar(
        r#"
        UPDATE "ob-poc".kyc_ubo_evidence
        SET status = 'VERIFIED',
            verified_at = NOW(),
            verified_by = $2,
            notes = COALESCE($3, notes),
            updated_at = NOW()
        WHERE evidence_id = $1
        RETURNING verified_at
        "#,
    )
    .bind(evidence_id)
    .bind(&verified_by)
    .bind(&notes)
    .fetch_one(pool)
    .await?;

    Ok(EvidenceVerifyResult {
        evidence_id,
        verified_by,
        verified_at,
    })
}

#[cfg(feature = "database")]
async fn evidence_reject_impl(
    evidence_id: Uuid,
    reason: String,
    pool: &PgPool,
) -> Result<EvidenceRejectResult> {
    let current_status = fetch_evidence_status(pool, evidence_id).await?;

    if current_status != "RECEIVED" {
        return Err(anyhow!(
            "Cannot reject: evidence is in status '{}'. \
             Only RECEIVED evidence can be rejected.",
            current_status
        ));
    }

    let previous_document_id: Option<Uuid> = sqlx::query_scalar(
        r#"SELECT document_id FROM "ob-poc".kyc_ubo_evidence WHERE evidence_id = $1"#,
    )
    .bind(evidence_id)
    .fetch_one(pool)
    .await?;

    sqlx::query(
        r#"
        UPDATE "ob-poc".kyc_ubo_evidence
        SET status = 'REJECTED',
            document_id = NULL,
            notes = $2,
            updated_at = NOW()
        WHERE evidence_id = $1
        "#,
    )
    .bind(evidence_id)
    .bind(&reason)
    .execute(pool)
    .await?;

    Ok(EvidenceRejectResult {
        evidence_id,
        reason,
        previous_document_id,
    })
}

#[cfg(feature = "database")]
async fn evidence_waive_impl(
    evidence_id: Uuid,
    reason: String,
    authority: String,
    pool: &PgPool,
) -> Result<EvidenceWaiveResult> {
    let _current_status = fetch_evidence_status(pool, evidence_id).await?;

    sqlx::query(
        r#"
        UPDATE "ob-poc".kyc_ubo_evidence
        SET status = 'WAIVED',
            waived_reason = $2,
            waived_by = $3,
            updated_at = NOW()
        WHERE evidence_id = $1
        "#,
    )
    .bind(evidence_id)
    .bind(&reason)
    .bind(&authority)
    .execute(pool)
    .await?;

    Ok(EvidenceWaiveResult {
        evidence_id,
        reason,
        waived_by: authority,
    })
}

// =============================================================================
// EvidenceRequireOp — INSERT new evidence requirement
// =============================================================================

/// Creates a new evidence requirement with status REQUIRED.
///
/// Inserts a row into `"ob-poc".kyc_ubo_evidence` linked to the given UBO registry
/// entry. The evidence starts in REQUIRED status and must be fulfilled by
/// linking a document (evidence.link) before it can be verified.
#[register_custom_op]
pub struct EvidenceRequireOp;

#[async_trait]
impl CustomOperation for EvidenceRequireOp {
    fn domain(&self) -> &'static str {
        "evidence"
    }

    fn verb(&self) -> &'static str {
        "require"
    }

    fn rationale(&self) -> &'static str {
        "Creates evidence requirement linked to UBO registry with initial REQUIRED status"
    }


    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        let registry_id = json_extract_uuid(args, ctx, "registry-id")?;
        let evidence_type = json_extract_string(args, "evidence-type")?;
        let description = json_extract_string_opt(args, "description");
        let doc_type = json_extract_string_opt(args, "doc-type");
        let result =
            evidence_require_impl(registry_id, evidence_type, description, doc_type, pool).await?;
        ctx.bind("evidence", result.evidence_id);
        Ok(dsl_runtime::VerbExecutionOutcome::Record(
            serde_json::to_value(result)?,
        ))
    }


    fn is_migrated(&self) -> bool {
        true
    }
}

impl EvidenceRequireOp {
    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let registry_id = extract_uuid(verb_call, ctx, "registry-id")?;
        let evidence_type = extract_string(verb_call, "evidence-type")?;
        let description = extract_string_opt(verb_call, "description");
        let doc_type = extract_string_opt(verb_call, "doc-type");
        let result =
            evidence_require_impl(registry_id, evidence_type, description, doc_type, pool).await?;
        ctx.bind("evidence", result.evidence_id);
        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
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

// =============================================================================
// EvidenceLinkOp — Link document, status → RECEIVED
// =============================================================================

/// Links a document to an evidence record and sets status to RECEIVED.
///
/// The evidence must be in REQUIRED or REJECTED status. Linking after
/// rejection allows re-submission of a replacement document.
#[register_custom_op]
pub struct EvidenceLinkOp;

#[async_trait]
impl CustomOperation for EvidenceLinkOp {
    fn domain(&self) -> &'static str {
        "evidence"
    }

    fn verb(&self) -> &'static str {
        "link"
    }

    fn rationale(&self) -> &'static str {
        "Validates status allows linking and atomically updates document_id and status"
    }


    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        let evidence_id = json_extract_uuid(args, ctx, "evidence-id")?;
        let document_id = json_extract_uuid(args, ctx, "document-id")?;
        let result = evidence_link_impl(evidence_id, document_id, pool).await?;
        Ok(dsl_runtime::VerbExecutionOutcome::Record(
            serde_json::to_value(result)?,
        ))
    }


    fn is_migrated(&self) -> bool {
        true
    }
}

impl EvidenceLinkOp {
    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let evidence_id = extract_uuid(verb_call, ctx, "evidence-id")?;
        let document_id = extract_uuid(verb_call, ctx, "document-id")?;
        let result = evidence_link_impl(evidence_id, document_id, pool).await?;
        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
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

// =============================================================================
// EvidenceVerifyOp — status → VERIFIED
// =============================================================================

/// Verifies an evidence record, setting status to VERIFIED.
///
/// The evidence must be in RECEIVED status (a document must be linked).
/// Records the verifier identity and timestamp.
#[register_custom_op]
pub struct EvidenceVerifyOp;

#[async_trait]
impl CustomOperation for EvidenceVerifyOp {
    fn domain(&self) -> &'static str {
        "evidence"
    }

    fn verb(&self) -> &'static str {
        "verify"
    }

    fn rationale(&self) -> &'static str {
        "Validates RECEIVED status before transitioning to VERIFIED with verifier attribution"
    }


    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        let evidence_id = json_extract_uuid(args, ctx, "evidence-id")?;
        let verified_by = json_extract_string(args, "verified-by")?;
        let notes = json_extract_string_opt(args, "notes");
        let result = evidence_verify_impl(evidence_id, verified_by, notes, pool).await?;
        Ok(dsl_runtime::VerbExecutionOutcome::Record(
            serde_json::to_value(result)?,
        ))
    }


    fn is_migrated(&self) -> bool {
        true
    }
}

impl EvidenceVerifyOp {
    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let evidence_id = extract_uuid(verb_call, ctx, "evidence-id")?;
        let verified_by = extract_string(verb_call, "verified-by")?;
        let notes = extract_string_opt(verb_call, "notes");
        let result = evidence_verify_impl(evidence_id, verified_by, notes, pool).await?;
        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
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

// =============================================================================
// EvidenceRejectOp — status → REJECTED, clears document_id
// =============================================================================

/// Rejects an evidence record, clearing the linked document so it can be re-linked.
///
/// The evidence must be in RECEIVED status. After rejection, the evidence
/// returns to a state where a new document can be linked via evidence.link.
#[register_custom_op]
pub struct EvidenceRejectOp;

#[async_trait]
impl CustomOperation for EvidenceRejectOp {
    fn domain(&self) -> &'static str {
        "evidence"
    }

    fn verb(&self) -> &'static str {
        "reject"
    }

    fn rationale(&self) -> &'static str {
        "Validates RECEIVED status, clears document_id to allow re-linking after rejection"
    }


    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        let evidence_id = json_extract_uuid(args, ctx, "evidence-id")?;
        let reason = json_extract_string(args, "reason")?;
        let result = evidence_reject_impl(evidence_id, reason, pool).await?;
        Ok(dsl_runtime::VerbExecutionOutcome::Record(
            serde_json::to_value(result)?,
        ))
    }


    fn is_migrated(&self) -> bool {
        true
    }
}

impl EvidenceRejectOp {
    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let evidence_id = extract_uuid(verb_call, ctx, "evidence-id")?;
        let reason = extract_string(verb_call, "reason")?;
        let result = evidence_reject_impl(evidence_id, reason, pool).await?;
        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
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

// =============================================================================
// EvidenceWaiveOp — status → WAIVED
// =============================================================================

/// Waives an evidence requirement with reason and authority.
///
/// Any evidence status can transition to WAIVED. Records the authority
/// who approved the waiver and the reason for audit purposes.
#[register_custom_op]
pub struct EvidenceWaiveOp;

#[async_trait]
impl CustomOperation for EvidenceWaiveOp {
    fn domain(&self) -> &'static str {
        "evidence"
    }

    fn verb(&self) -> &'static str {
        "waive"
    }

    fn rationale(&self) -> &'static str {
        "Waiver requires authority and reason tracking for audit trail"
    }


    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        let evidence_id = json_extract_uuid(args, ctx, "evidence-id")?;
        let reason = json_extract_string(args, "reason")?;
        let authority = json_extract_string(args, "authority")?;
        let result = evidence_waive_impl(evidence_id, reason, authority, pool).await?;
        Ok(dsl_runtime::VerbExecutionOutcome::Record(
            serde_json::to_value(result)?,
        ))
    }


    fn is_migrated(&self) -> bool {
        true
    }
}

impl EvidenceWaiveOp {
    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let evidence_id = extract_uuid(verb_call, ctx, "evidence-id")?;
        let reason = extract_string(verb_call, "reason")?;
        let authority = extract_string(verb_call, "authority")?;
        let result = evidence_waive_impl(evidence_id, reason, authority, pool).await?;
        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
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

/// Compatibility alias for `evidence.create-requirement`.
#[register_custom_op]
pub struct EvidenceCreateRequirementOp;

#[async_trait]
impl CustomOperation for EvidenceCreateRequirementOp {
    fn domain(&self) -> &'static str {
        "evidence"
    }
    fn verb(&self) -> &'static str {
        "create-requirement"
    }
    fn rationale(&self) -> &'static str {
        "Compatibility alias for evidence.require"
    }


    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        EvidenceRequireOp.execute_json(args, ctx, pool).await
    }


    fn is_migrated(&self) -> bool {
        true
    }
}

impl EvidenceCreateRequirementOp {
    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        EvidenceRequireOp.execute(verb_call, ctx, pool).await
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

/// Compatibility alias for `evidence.attach-document`.
#[register_custom_op]
pub struct EvidenceAttachDocumentOp;

#[async_trait]
impl CustomOperation for EvidenceAttachDocumentOp {
    fn domain(&self) -> &'static str {
        "evidence"
    }
    fn verb(&self) -> &'static str {
        "attach-document"
    }
    fn rationale(&self) -> &'static str {
        "Compatibility alias for evidence.link"
    }


    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        EvidenceLinkOp.execute_json(args, ctx, pool).await
    }


    fn is_migrated(&self) -> bool {
        true
    }
}

impl EvidenceAttachDocumentOp {
    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        EvidenceLinkOp.execute(verb_call, ctx, pool).await
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

/// Compatibility alias for `evidence.mark-verified`.
#[register_custom_op]
pub struct EvidenceMarkVerifiedOp;

#[async_trait]
impl CustomOperation for EvidenceMarkVerifiedOp {
    fn domain(&self) -> &'static str {
        "evidence"
    }
    fn verb(&self) -> &'static str {
        "mark-verified"
    }
    fn rationale(&self) -> &'static str {
        "Compatibility alias for evidence.verify"
    }


    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        EvidenceVerifyOp.execute_json(args, ctx, pool).await
    }


    fn is_migrated(&self) -> bool {
        true
    }
}

impl EvidenceMarkVerifiedOp {
    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        EvidenceVerifyOp.execute(verb_call, ctx, pool).await
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

/// Compatibility alias for `evidence.mark-rejected`.
#[register_custom_op]
pub struct EvidenceMarkRejectedOp;

#[async_trait]
impl CustomOperation for EvidenceMarkRejectedOp {
    fn domain(&self) -> &'static str {
        "evidence"
    }
    fn verb(&self) -> &'static str {
        "mark-rejected"
    }
    fn rationale(&self) -> &'static str {
        "Compatibility alias for evidence.reject"
    }


    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        EvidenceRejectOp.execute_json(args, ctx, pool).await
    }


    fn is_migrated(&self) -> bool {
        true
    }
}

impl EvidenceMarkRejectedOp {
    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        EvidenceRejectOp.execute(verb_call, ctx, pool).await
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

/// Compatibility alias for `evidence.mark-waived`.
#[register_custom_op]
pub struct EvidenceMarkWaivedOp;

#[async_trait]
impl CustomOperation for EvidenceMarkWaivedOp {
    fn domain(&self) -> &'static str {
        "evidence"
    }
    fn verb(&self) -> &'static str {
        "mark-waived"
    }
    fn rationale(&self) -> &'static str {
        "Compatibility alias for evidence.waive"
    }


    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        EvidenceWaiveOp.execute_json(args, ctx, pool).await
    }


    fn is_migrated(&self) -> bool {
        true
    }
}

impl EvidenceMarkWaivedOp {
    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        EvidenceWaiveOp.execute(verb_call, ctx, pool).await
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
