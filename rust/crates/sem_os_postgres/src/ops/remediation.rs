//! Remediation verbs — SemOS-side YAML-first re-implementation.
//!
//! Four `remediation.*` verbs tracking resolution of cross-workspace
//! state drift from shared attribute supersession. All delegate to
//! `dsl-runtime::cross_workspace::remediation::*` helpers which
//! currently take `&PgPool` — `scope.pool()` is forwarded
//! transitionally.

use anyhow::Result;
use async_trait::async_trait;
use serde::Serialize;
use uuid::Uuid;

use dsl_runtime::cross_workspace::remediation;
use dsl_runtime::domain_ops::helpers::{
    self, json_extract_string, json_extract_string_opt, json_extract_uuid, json_extract_uuid_opt,
};
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::SemOsVerbOp;

pub struct ListOpen;

#[async_trait]
impl SemOsVerbOp for ListOpen {
    fn fqn(&self) -> &str {
        "remediation.list-open"
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let entity_id = json_extract_uuid_opt(args, ctx, "entity-id");
        let workspace = json_extract_string_opt(args, "workspace");
        let events = remediation::list_open(scope.pool(), entity_id, workspace.as_deref()).await?;
        let value = serde_json::to_value(events)?;
        match value {
            serde_json::Value::Array(items) => Ok(VerbExecutionOutcome::RecordSet(items)),
            _ => Ok(VerbExecutionOutcome::RecordSet(Vec::new())),
        }
    }
}

#[derive(Serialize)]
struct DeferResult {
    remediation_id: Uuid,
    status: &'static str,
    reason: String,
}

pub struct Defer;

#[async_trait]
impl SemOsVerbOp for Defer {
    fn fqn(&self) -> &str {
        "remediation.defer"
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let remediation_id = json_extract_uuid(args, ctx, "remediation-id")?;
        let reason = json_extract_string(args, "reason")?;
        remediation::defer(scope.pool(), remediation_id, &reason, None).await?;
        helpers::emit_pending_state_advance(
            ctx,
            remediation_id,
            "remediation:deferred",
            "remediation/lifecycle",
            "remediation.defer",
        );
        Ok(VerbExecutionOutcome::Record(serde_json::to_value(
            DeferResult {
                remediation_id,
                status: "deferred",
                reason,
            },
        )?))
    }
}

#[derive(Serialize)]
struct RevokeResult {
    remediation_id: Uuid,
    status: &'static str,
}

pub struct RevokeDeferral;

#[async_trait]
impl SemOsVerbOp for RevokeDeferral {
    fn fqn(&self) -> &str {
        "remediation.revoke-deferral"
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let remediation_id = json_extract_uuid(args, ctx, "remediation-id")?;
        remediation::revoke_deferral(scope.pool(), remediation_id).await?;
        helpers::emit_pending_state_advance(
            ctx,
            remediation_id,
            "remediation:detection_active",
            "remediation/lifecycle",
            "remediation.revoke-deferral",
        );
        Ok(VerbExecutionOutcome::Record(serde_json::to_value(
            RevokeResult {
                remediation_id,
                status: "detected",
            },
        )?))
    }
}

#[derive(Serialize)]
struct ConfirmResult {
    remediation_id: Uuid,
    provider_ref: String,
    status: &'static str,
}

pub struct ConfirmExternalCorrection;

#[async_trait]
impl SemOsVerbOp for ConfirmExternalCorrection {
    fn fqn(&self) -> &str {
        "remediation.confirm-external-correction"
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let remediation_id = json_extract_uuid(args, ctx, "remediation-id")?;
        let provider_ref = json_extract_string(args, "provider-ref")?;
        remediation::mark_resolved(scope.pool(), remediation_id, None).await?;
        helpers::emit_pending_state_advance(
            ctx,
            remediation_id,
            "remediation:resolved",
            "remediation/lifecycle",
            "remediation.confirm-external-correction",
        );
        Ok(VerbExecutionOutcome::Record(serde_json::to_value(
            ConfirmResult {
                remediation_id,
                provider_ref,
                status: "resolved",
            },
        )?))
    }
}
