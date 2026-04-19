//! Custom operations for the `remediation` domain.
//!
//! Manages remediation event lifecycle — tracking resolution of cross-workspace
//! state drift from shared attribute supersession.

use anyhow::Result;
use async_trait::async_trait;
use dsl_runtime_macros::register_custom_op;

use super::helpers::{
    extract_string, extract_string_opt, extract_uuid, extract_uuid_opt, json_extract_string,
    json_extract_string_opt, json_extract_uuid, json_extract_uuid_opt,
};
use super::{CustomOperation, ExecutionContext, ExecutionResult, VerbCall};

#[cfg(feature = "database")]
use sqlx::PgPool;

// ── list-open ────────────────────────────────────────────────────────

#[register_custom_op]
pub struct RemediationListOpenOp;

#[cfg(feature = "database")]
async fn remediation_list_open_impl(
    entity_id: Option<uuid::Uuid>,
    workspace: Option<String>,
    pool: &PgPool,
) -> Result<serde_json::Value> {
    use crate::cross_workspace::remediation;

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



    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        let entity_id = json_extract_uuid_opt(args, ctx, "entity-id");
        let workspace = json_extract_string_opt(args, "workspace");
        let records = remediation_list_open_impl(entity_id, workspace, pool).await?;
        match records {
            serde_json::Value::Array(items) => Ok(
                dsl_runtime::VerbExecutionOutcome::RecordSet(items),
            ),
            _ => Ok(dsl_runtime::VerbExecutionOutcome::RecordSet(
                Vec::new(),
            )),
        }
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

impl RemediationListOpenOp {
    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let entity_id = extract_uuid_opt(verb_call, ctx, "entity-id");
        let workspace = extract_string_opt(verb_call, "workspace");
        let records = remediation_list_open_impl(entity_id, workspace, pool).await?;
        match records {
            serde_json::Value::Array(items) => Ok(ExecutionResult::RecordSet(items)),
            _ => Ok(ExecutionResult::RecordSet(Vec::new())),
        }
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

#[cfg(feature = "database")]
async fn remediation_defer_impl(
    remediation_id: uuid::Uuid,
    reason: String,
    pool: &PgPool,
) -> Result<serde_json::Value> {
    use crate::cross_workspace::remediation;

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



    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        let remediation_id = json_extract_uuid(args, ctx, "remediation-id")?;
        let reason = json_extract_string(args, "reason")?;
        Ok(dsl_runtime::VerbExecutionOutcome::Record(
            remediation_defer_impl(remediation_id, reason, pool).await?,
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

impl RemediationDeferOp {
    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let remediation_id = extract_uuid(verb_call, ctx, "remediation-id")?;
        let reason = extract_string(verb_call, "reason")?;
        Ok(ExecutionResult::Record(
            remediation_defer_impl(remediation_id, reason, pool).await?,
        ))
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

#[cfg(feature = "database")]
async fn remediation_revoke_deferral_impl(
    remediation_id: uuid::Uuid,
    pool: &PgPool,
) -> Result<serde_json::Value> {
    use crate::cross_workspace::remediation;

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



    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        let remediation_id = json_extract_uuid(args, ctx, "remediation-id")?;
        Ok(dsl_runtime::VerbExecutionOutcome::Record(
            remediation_revoke_deferral_impl(remediation_id, pool).await?,
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

impl RemediationRevokeDeferralOp {
    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let remediation_id = extract_uuid(verb_call, ctx, "remediation-id")?;
        Ok(ExecutionResult::Record(
            remediation_revoke_deferral_impl(remediation_id, pool).await?,
        ))
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

#[cfg(feature = "database")]
async fn remediation_confirm_external_impl(
    remediation_id: uuid::Uuid,
    provider_ref: String,
    pool: &PgPool,
) -> Result<serde_json::Value> {
    use crate::cross_workspace::remediation;

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



    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        let remediation_id = json_extract_uuid(args, ctx, "remediation-id")?;
        let provider_ref = json_extract_string(args, "provider-ref")?;
        Ok(dsl_runtime::VerbExecutionOutcome::Record(
            remediation_confirm_external_impl(remediation_id, provider_ref, pool).await?,
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

impl RemediationConfirmExternalOp {
    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let remediation_id = extract_uuid(verb_call, ctx, "remediation-id")?;
        let provider_ref = extract_string(verb_call, "provider-ref")?;
        Ok(ExecutionResult::Record(
            remediation_confirm_external_impl(remediation_id, provider_ref, pool).await?,
        ))
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
