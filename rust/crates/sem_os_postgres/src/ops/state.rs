//! State reducer verbs (8 plugin verbs) — SemOS-side YAML-first
//! re-implementation of the plugin subset of
//! `rust/config/verbs/state.yaml`.
//!
//! Every op loads a built-in state machine via
//! `dsl_runtime::state_reducer::load_builtin_state_machine` then
//! dispatches to the matching `handle_state_*` helper. The
//! reducer helpers still take `&PgPool` — transitional
//! `scope.pool().clone()` pattern (same as slice #13). They
//! open their own connection; commit/rollback on the scope does
//! not affect them.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::{json, Value};

use dsl_runtime::domain_ops::helpers::{
    json_extract_string, json_extract_string_opt, json_extract_uuid, json_extract_uuid_opt,
};
use dsl_runtime::state_reducer::{
    handle_state_blocked_why, handle_state_check_consistency, handle_state_derive,
    handle_state_derive_all, handle_state_diagnose, handle_state_list_overrides,
    handle_state_override, handle_state_revoke_override, load_builtin_state_machine,
};
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::SemOsVerbOp;

fn parse_optional_datetime(value: Option<String>) -> Result<Option<DateTime<Utc>>> {
    match value {
        Some(v) => Ok(Some(v.parse::<DateTime<Utc>>()?)),
        None => Ok(None),
    }
}

fn load_machine(
    args: &Value,
    default: &str,
) -> Result<dsl_runtime::state_reducer::ValidatedStateMachine> {
    let name =
        json_extract_string_opt(args, "state-machine").unwrap_or_else(|| default.to_string());
    load_builtin_state_machine(&name).map_err(|err| anyhow!(err.to_string()))
}

// ── state.derive ──────────────────────────────────────────────────────────────

pub struct Derive;

#[async_trait]
impl SemOsVerbOp for Derive {
    fn fqn(&self) -> &str {
        "state.derive"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let entity_id = json_extract_uuid(args, ctx, "entity-id")?;
        let slot_path = json_extract_string(args, "slot-path")?;
        let case_id = json_extract_uuid_opt(args, ctx, "case-id");
        let state_machine = load_machine(args, "entity_kyc_lifecycle")?;
        let result = handle_state_derive(
            scope.pool(),
            cbu_id,
            entity_id,
            &slot_path,
            case_id,
            &state_machine,
        )
        .await?;
        Ok(VerbExecutionOutcome::Record(serde_json::to_value(result)?))
    }
}

// ── state.diagnose ────────────────────────────────────────────────────────────

pub struct Diagnose;

#[async_trait]
impl SemOsVerbOp for Diagnose {
    fn fqn(&self) -> &str {
        "state.diagnose"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let entity_id = json_extract_uuid(args, ctx, "entity-id")?;
        let slot_path = json_extract_string(args, "slot-path")?;
        let case_id = json_extract_uuid_opt(args, ctx, "case-id");
        let state_machine = load_machine(args, "entity_kyc_lifecycle")?;
        let result = handle_state_diagnose(
            scope.pool(),
            cbu_id,
            entity_id,
            &slot_path,
            case_id,
            &state_machine,
        )
        .await?;
        Ok(VerbExecutionOutcome::Record(serde_json::to_value(result)?))
    }
}

// ── state.derive-all ──────────────────────────────────────────────────────────

pub struct DeriveAll;

#[async_trait]
impl SemOsVerbOp for DeriveAll {
    fn fqn(&self) -> &str {
        "state.derive-all"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let case_id = json_extract_uuid_opt(args, ctx, "case-id");
        let state_machine = load_machine(args, "entity_kyc_lifecycle")?;
        let result = handle_state_derive_all(scope.pool(), cbu_id, case_id, &state_machine).await?;
        let rows = result
            .into_iter()
            .map(serde_json::to_value)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(VerbExecutionOutcome::RecordSet(rows))
    }
}

// ── state.blocked-why ─────────────────────────────────────────────────────────

pub struct BlockedWhy;

#[async_trait]
impl SemOsVerbOp for BlockedWhy {
    fn fqn(&self) -> &str {
        "state.blocked-why"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let entity_id = json_extract_uuid(args, ctx, "entity-id")?;
        let slot_path = json_extract_string(args, "slot-path")?;
        let verb = json_extract_string(args, "verb")?;
        let case_id = json_extract_uuid_opt(args, ctx, "case-id");
        let state_machine = load_machine(args, "entity_kyc_lifecycle")?;
        let result = handle_state_blocked_why(
            scope.pool(),
            cbu_id,
            entity_id,
            &slot_path,
            &verb,
            case_id,
            &state_machine,
        )
        .await?;
        Ok(VerbExecutionOutcome::Record(serde_json::to_value(result)?))
    }
}

// ── state.check-consistency ───────────────────────────────────────────────────

pub struct CheckConsistency;

#[async_trait]
impl SemOsVerbOp for CheckConsistency {
    fn fqn(&self) -> &str {
        "state.check-consistency"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let case_id = json_extract_uuid_opt(args, ctx, "case-id");
        let state_machine = load_machine(args, "entity_kyc_lifecycle")?;
        let result =
            handle_state_check_consistency(scope.pool(), cbu_id, case_id, &state_machine).await?;
        let rows = result
            .into_iter()
            .map(serde_json::to_value)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(VerbExecutionOutcome::RecordSet(rows))
    }
}

// ── state.override ────────────────────────────────────────────────────────────

pub struct Override;

#[async_trait]
impl SemOsVerbOp for Override {
    fn fqn(&self) -> &str {
        "state.override"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let entity_id = json_extract_uuid(args, ctx, "entity-id")?;
        let slot_path = json_extract_string(args, "slot-path")?;
        let override_state = json_extract_string(args, "override-state")?;
        let justification = json_extract_string(args, "justification")?;
        let authority = json_extract_string(args, "authority")?;
        let case_id = json_extract_uuid_opt(args, ctx, "case-id");
        let constellation_type = json_extract_string_opt(args, "constellation-type")
            .unwrap_or_else(|| "entity_kyc_lifecycle".to_string());
        let expires_at = parse_optional_datetime(json_extract_string_opt(args, "expires-at"))?;
        let conditions = json_extract_string_opt(args, "conditions");
        let machine_name = json_extract_string_opt(args, "state-machine")
            .unwrap_or_else(|| constellation_type.clone());
        let state_machine =
            load_builtin_state_machine(&machine_name).map_err(|err| anyhow!(err.to_string()))?;
        let result = handle_state_override(
            scope.pool(),
            cbu_id,
            case_id,
            &constellation_type,
            &slot_path,
            entity_id,
            &override_state,
            &justification,
            &authority,
            expires_at,
            conditions.as_deref(),
            &state_machine,
        )
        .await?;
        Ok(VerbExecutionOutcome::Record(serde_json::to_value(result)?))
    }
}

// ── state.revoke-override ─────────────────────────────────────────────────────

pub struct RevokeOverride;

#[async_trait]
impl SemOsVerbOp for RevokeOverride {
    fn fqn(&self) -> &str {
        "state.revoke-override"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let override_id = json_extract_uuid(args, ctx, "override-id")?;
        let revoked_by = json_extract_string_opt(args, "revoked-by")
            .unwrap_or_else(|| ctx.principal.actor_id.clone());
        let reason = json_extract_string(args, "reason")?;
        handle_state_revoke_override(scope.pool(), override_id, &revoked_by, &reason).await?;
        Ok(VerbExecutionOutcome::Record(json!({
            "override_id": override_id,
            "revoked": true,
        })))
    }
}

// ── state.list-overrides ──────────────────────────────────────────────────────

pub struct ListOverrides;

#[async_trait]
impl SemOsVerbOp for ListOverrides {
    fn fqn(&self) -> &str {
        "state.list-overrides"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let result = handle_state_list_overrides(scope.pool(), cbu_id).await?;
        let rows = result
            .into_iter()
            .map(serde_json::to_value)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(VerbExecutionOutcome::RecordSet(rows))
    }
}
