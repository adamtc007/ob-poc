//! Evidence custom operations for KYC UBO evidence management.
//!
//! Operations for managing evidence records in `kyc.ubo_evidence` that
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
use ob_poc_macros::register_custom_op;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::helpers::{extract_string, extract_string_opt, extract_uuid};
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
        sqlx::query_as(r#"SELECT status FROM kyc.ubo_evidence WHERE evidence_id = $1"#)
            .bind(evidence_id)
            .fetch_optional(pool)
            .await?;

    row.map(|(s,)| s)
        .ok_or_else(|| anyhow!("Evidence record not found: {}", evidence_id))
}

// =============================================================================
// EvidenceRequireOp — INSERT new evidence requirement
// =============================================================================

/// Creates a new evidence requirement with status REQUIRED.
///
/// Inserts a row into `kyc.ubo_evidence` linked to the given UBO registry
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

        let evidence_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO kyc.ubo_evidence
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

        ctx.bind("evidence", evidence_id);

        let result = EvidenceRequireResult {
            evidence_id,
            registry_id,
            evidence_type,
            status: "REQUIRED".to_string(),
        };
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
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let evidence_id = extract_uuid(verb_call, ctx, "evidence-id")?;
        let document_id = extract_uuid(verb_call, ctx, "document-id")?;

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
            UPDATE kyc.ubo_evidence
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

        let result = EvidenceLinkResult {
            evidence_id,
            document_id,
            previous_status: current_status,
            new_status: "RECEIVED".to_string(),
        };
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
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let evidence_id = extract_uuid(verb_call, ctx, "evidence-id")?;
        let verified_by = extract_string(verb_call, "verified-by")?;
        let notes = extract_string_opt(verb_call, "notes");

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
            UPDATE kyc.ubo_evidence
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

        let result = EvidenceVerifyResult {
            evidence_id,
            verified_by,
            verified_at,
        };
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
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let evidence_id = extract_uuid(verb_call, ctx, "evidence-id")?;
        let reason = extract_string(verb_call, "reason")?;

        let current_status = fetch_evidence_status(pool, evidence_id).await?;

        if current_status != "RECEIVED" {
            return Err(anyhow!(
                "Cannot reject: evidence is in status '{}'. \
                 Only RECEIVED evidence can be rejected.",
                current_status
            ));
        }

        // Fetch the current document_id before clearing it
        let previous_document_id: Option<Uuid> = sqlx::query_scalar(
            r#"SELECT document_id FROM kyc.ubo_evidence WHERE evidence_id = $1"#,
        )
        .bind(evidence_id)
        .fetch_one(pool)
        .await?;

        sqlx::query(
            r#"
            UPDATE kyc.ubo_evidence
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

        let result = EvidenceRejectResult {
            evidence_id,
            reason,
            previous_document_id,
        };
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
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let evidence_id = extract_uuid(verb_call, ctx, "evidence-id")?;
        let reason = extract_string(verb_call, "reason")?;
        let authority = extract_string(verb_call, "authority")?;

        // Verify the evidence record exists (fetch_evidence_status validates existence)
        let _current_status = fetch_evidence_status(pool, evidence_id).await?;

        sqlx::query(
            r#"
            UPDATE kyc.ubo_evidence
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

        let result = EvidenceWaiveResult {
            evidence_id,
            reason,
            waived_by: authority,
        };
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
