//! Custom operations for the `remediation` domain.
//!
//! Manages remediation event lifecycle — tracking resolution of cross-workspace
//! state drift from shared attribute supersession.
//!
//! Relocated from ob-poc to dsl-runtime in Phase 5a composite-blocker #2
//! (2026-04-20). All four ops call `dsl-runtime::cross_workspace::remediation`
//! which was relocated alongside this file.

use anyhow::Result;
use async_trait::async_trait;
use dsl_runtime_macros::register_custom_op;
use sqlx::PgPool;

use crate::cross_workspace::remediation;
use crate::custom_op::CustomOperation;
use crate::domain_ops::helpers::{
    json_extract_string, json_extract_string_opt, json_extract_uuid, json_extract_uuid_opt,
};
use crate::execution::{VerbExecutionContext, VerbExecutionOutcome};

// ── list-open ────────────────────────────────────────────────────────

#[register_custom_op]
pub struct RemediationListOpenOp;

async fn remediation_list_open_impl(
    entity_id: Option<uuid::Uuid>,
    workspace: Option<String>,
    pool: &PgPool,
) -> Result<serde_json::Value> {
    let events = remediation::list_open(pool, entity_id, workspace.as_deref()).await?;
    Ok(serde_json::to_value(events)?)
}

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

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let entity_id = json_extract_uuid_opt(args, ctx, "entity-id");
        let workspace = json_extract_string_opt(args, "workspace");
        let records = remediation_list_open_impl(entity_id, workspace, pool).await?;
        match records {
            serde_json::Value::Array(items) => Ok(VerbExecutionOutcome::RecordSet(items)),
            _ => Ok(VerbExecutionOutcome::RecordSet(Vec::new())),
        }
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

// ── defer ────────────────────────────────────────────────────────────

#[register_custom_op]
pub struct RemediationDeferOp;

async fn remediation_defer_impl(
    remediation_id: uuid::Uuid,
    reason: String,
    pool: &PgPool,
) -> Result<serde_json::Value> {
    remediation::defer(pool, remediation_id, &reason, None).await?;

    #[derive(serde::Serialize)]
    struct DeferResult {
        remediation_id: uuid::Uuid,
        status: &'static str,
        reason: String,
    }

    Ok(serde_json::to_value(DeferResult {
        remediation_id,
        status: "deferred",
        reason,
    })?)
}

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

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let remediation_id = json_extract_uuid(args, ctx, "remediation-id")?;
        let reason = json_extract_string(args, "reason")?;
        Ok(VerbExecutionOutcome::Record(
            remediation_defer_impl(remediation_id, reason, pool).await?,
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

// ── revoke-deferral ──────────────────────────────────────────────────

#[register_custom_op]
pub struct RemediationRevokeDeferralOp;

async fn remediation_revoke_deferral_impl(
    remediation_id: uuid::Uuid,
    pool: &PgPool,
) -> Result<serde_json::Value> {
    remediation::revoke_deferral(pool, remediation_id).await?;

    #[derive(serde::Serialize)]
    struct RevokeResult {
        remediation_id: uuid::Uuid,
        status: &'static str,
    }

    Ok(serde_json::to_value(RevokeResult {
        remediation_id,
        status: "detected",
    })?)
}

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

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let remediation_id = json_extract_uuid(args, ctx, "remediation-id")?;
        Ok(VerbExecutionOutcome::Record(
            remediation_revoke_deferral_impl(remediation_id, pool).await?,
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

// ── confirm-external-correction ──────────────────────────────────────

#[register_custom_op]
pub struct RemediationConfirmExternalOp;

async fn remediation_confirm_external_impl(
    remediation_id: uuid::Uuid,
    provider_ref: String,
    pool: &PgPool,
) -> Result<serde_json::Value> {
    remediation::mark_resolved(pool, remediation_id, None).await?;

    #[derive(serde::Serialize)]
    struct ConfirmResult {
        remediation_id: uuid::Uuid,
        provider_ref: String,
        status: &'static str,
    }

    Ok(serde_json::to_value(ConfirmResult {
        remediation_id,
        provider_ref,
        status: "resolved",
    })?)
}

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

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let remediation_id = json_extract_uuid(args, ctx, "remediation-id")?;
        let provider_ref = json_extract_string(args, "provider-ref")?;
        Ok(VerbExecutionOutcome::Record(
            remediation_confirm_external_impl(remediation_id, provider_ref, pool).await?,
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}
