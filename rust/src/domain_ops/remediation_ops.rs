//! Custom operations for the `remediation` domain.
//!
//! Manages remediation event lifecycle — tracking resolution of cross-workspace
//! state drift from shared attribute supersession.

use anyhow::Result;
use async_trait::async_trait;
use ob_poc_macros::register_custom_op;

use super::helpers::{extract_string, extract_string_opt, extract_uuid, extract_uuid_opt};
use super::{CustomOperation, ExecutionContext, ExecutionResult, VerbCall};

#[cfg(feature = "database")]
use sqlx::PgPool;

// ── list-open ────────────────────────────────────────────────────────

#[register_custom_op]
pub struct RemediationListOpenOp;

#[async_trait]
impl CustomOperation for RemediationListOpenOp {
    fn domain(&self) -> &'static str {
        "remediation"
    }

    fn verb(&self) -> &'static str {
        "list-open"
    }

    fn rationale(&self) -> &'static str {
        "Remediation queries require join with shared_atom_registry for atom paths"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use crate::cross_workspace::remediation;

        let entity_id = extract_uuid_opt(verb_call, ctx, "entity-id");
        let workspace = extract_string_opt(verb_call, "workspace");

        let events = remediation::list_open(pool, entity_id, workspace.as_deref()).await?;
        let records: Vec<serde_json::Value> = events
            .into_iter()
            .map(serde_json::to_value)
            .collect::<Result<_, _>>()?;
        Ok(ExecutionResult::RecordSet(records))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("remediation.list-open requires database"))
    }
}

// ── defer ────────────────────────────────────────────────────────────

#[register_custom_op]
pub struct RemediationDeferOp;

#[async_trait]
impl CustomOperation for RemediationDeferOp {
    fn domain(&self) -> &'static str {
        "remediation"
    }

    fn verb(&self) -> &'static str {
        "defer"
    }

    fn rationale(&self) -> &'static str {
        "Deferral is a compliance-auditable state transition with reason recording"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use crate::cross_workspace::remediation;

        let remediation_id = extract_uuid(verb_call, ctx, "remediation-id")?;
        let reason = extract_string(verb_call, "reason")?;

        remediation::defer(pool, remediation_id, &reason, None).await?;

        #[derive(serde::Serialize)]
        struct DeferResult {
            remediation_id: uuid::Uuid,
            status: &'static str,
            reason: String,
        }

        Ok(ExecutionResult::Record(serde_json::to_value(
            DeferResult {
                remediation_id,
                status: "deferred",
                reason,
            },
        )?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("remediation.defer requires database"))
    }
}

// ── revoke-deferral ──────────────────────────────────────────────────

#[register_custom_op]
pub struct RemediationRevokeDeferralOp;

#[async_trait]
impl CustomOperation for RemediationRevokeDeferralOp {
    fn domain(&self) -> &'static str {
        "remediation"
    }

    fn verb(&self) -> &'static str {
        "revoke-deferral"
    }

    fn rationale(&self) -> &'static str {
        "Re-opens a deferred remediation for replay"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use crate::cross_workspace::remediation;

        let remediation_id = extract_uuid(verb_call, ctx, "remediation-id")?;

        remediation::revoke_deferral(pool, remediation_id).await?;

        #[derive(serde::Serialize)]
        struct RevokeResult {
            remediation_id: uuid::Uuid,
            status: &'static str,
        }

        Ok(ExecutionResult::Record(serde_json::to_value(
            RevokeResult {
                remediation_id,
                status: "detected",
            },
        )?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("remediation.revoke-deferral requires database"))
    }
}

// ── confirm-external-correction ──────────────────────────────────────

#[register_custom_op]
pub struct RemediationConfirmExternalOp;

#[async_trait]
impl CustomOperation for RemediationConfirmExternalOp {
    fn domain(&self) -> &'static str {
        "remediation"
    }

    fn verb(&self) -> &'static str {
        "confirm-external-correction"
    }

    fn rationale(&self) -> &'static str {
        "Records manual provider correction and resolves the remediation"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use crate::cross_workspace::remediation;

        let remediation_id = extract_uuid(verb_call, ctx, "remediation-id")?;
        let provider_ref = extract_string(verb_call, "provider-ref")?;

        // Resolve the escalated remediation
        remediation::mark_resolved(pool, remediation_id, None).await?;

        #[derive(serde::Serialize)]
        struct ConfirmResult {
            remediation_id: uuid::Uuid,
            provider_ref: String,
            status: &'static str,
        }

        Ok(ExecutionResult::Record(serde_json::to_value(
            ConfirmResult {
                remediation_id,
                provider_ref,
                status: "resolved",
            },
        )?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!(
            "remediation.confirm-external-correction requires database"
        ))
    }
}
