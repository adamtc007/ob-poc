//! State reducer custom operations.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use dsl_runtime_macros::register_custom_op;
use serde_json::json;
use sqlx::PgPool;

use crate::custom_op::CustomOperation;
use crate::domain_ops::helpers::{
    json_extract_string, json_extract_string_opt, json_extract_uuid, json_extract_uuid_opt,
};
use crate::execution::{VerbExecutionContext, VerbExecutionOutcome};
use crate::state_reducer::{
    handle_state_blocked_why, handle_state_check_consistency, handle_state_derive,
    handle_state_derive_all, handle_state_diagnose, handle_state_list_overrides,
    handle_state_override, handle_state_revoke_override, load_builtin_state_machine,
};

fn parse_optional_datetime(value: Option<String>) -> Result<Option<DateTime<Utc>>> {
    match value {
        Some(value) => Ok(Some(value.parse::<DateTime<Utc>>()?)),
        None => Ok(None),
    }
}

#[register_custom_op]
pub struct StateDeriveOp;

#[async_trait]
impl CustomOperation for StateDeriveOp {
    fn domain(&self) -> &'static str {
        "state"
    }

    fn verb(&self) -> &'static str {
        "derive"
    }

    fn rationale(&self) -> &'static str {
        "Reducer-backed state derivation custom operation"
    }

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let entity_id = json_extract_uuid(args, ctx, "entity-id")?;
        let slot_path = json_extract_string(args, "slot-path")?;
        let case_id = json_extract_uuid_opt(args, ctx, "case-id");
        let machine_name = json_extract_string_opt(args, "state-machine")
            .unwrap_or_else(|| "entity_kyc_lifecycle".to_string());
        let state_machine =
            load_builtin_state_machine(&machine_name).map_err(|err| anyhow!(err.to_string()))?;
        let result =
            handle_state_derive(pool, cbu_id, entity_id, &slot_path, case_id, &state_machine)
                .await?;
        Ok(VerbExecutionOutcome::Record(serde_json::to_value(result)?))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

#[register_custom_op]
pub struct StateDiagnoseOp;

#[async_trait]
impl CustomOperation for StateDiagnoseOp {
    fn domain(&self) -> &'static str {
        "state"
    }

    fn verb(&self) -> &'static str {
        "diagnose"
    }

    fn rationale(&self) -> &'static str {
        "Reducer-backed state trace custom operation"
    }

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let entity_id = json_extract_uuid(args, ctx, "entity-id")?;
        let slot_path = json_extract_string(args, "slot-path")?;
        let case_id = json_extract_uuid_opt(args, ctx, "case-id");
        let machine_name = json_extract_string_opt(args, "state-machine")
            .unwrap_or_else(|| "entity_kyc_lifecycle".to_string());
        let state_machine =
            load_builtin_state_machine(&machine_name).map_err(|err| anyhow!(err.to_string()))?;
        let result =
            handle_state_diagnose(pool, cbu_id, entity_id, &slot_path, case_id, &state_machine)
                .await?;
        Ok(VerbExecutionOutcome::Record(serde_json::to_value(result)?))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

#[register_custom_op]
pub struct StateDeriveAllOp;

#[async_trait]
impl CustomOperation for StateDeriveAllOp {
    fn domain(&self) -> &'static str {
        "state"
    }

    fn verb(&self) -> &'static str {
        "derive-all"
    }

    fn rationale(&self) -> &'static str {
        "Reducer scan over all discovered slots"
    }

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let case_id = json_extract_uuid_opt(args, ctx, "case-id");
        let machine_name = json_extract_string_opt(args, "state-machine")
            .unwrap_or_else(|| "entity_kyc_lifecycle".to_string());
        let state_machine =
            load_builtin_state_machine(&machine_name).map_err(|err| anyhow!(err.to_string()))?;
        let result = handle_state_derive_all(pool, cbu_id, case_id, &state_machine).await?;
        Ok(VerbExecutionOutcome::RecordSet(
            result
                .into_iter()
                .map(serde_json::to_value)
                .collect::<Result<Vec<_>, _>>()?,
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

#[register_custom_op]
pub struct StateBlockedWhyOp;

#[async_trait]
impl CustomOperation for StateBlockedWhyOp {
    fn domain(&self) -> &'static str {
        "state"
    }

    fn verb(&self) -> &'static str {
        "blocked-why"
    }

    fn rationale(&self) -> &'static str {
        "Reducer blocked-why custom operation"
    }

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let entity_id = json_extract_uuid(args, ctx, "entity-id")?;
        let slot_path = json_extract_string(args, "slot-path")?;
        let verb = json_extract_string(args, "verb")?;
        let case_id = json_extract_uuid_opt(args, ctx, "case-id");
        let machine_name = json_extract_string_opt(args, "state-machine")
            .unwrap_or_else(|| "entity_kyc_lifecycle".to_string());
        let state_machine =
            load_builtin_state_machine(&machine_name).map_err(|err| anyhow!(err.to_string()))?;
        let result = handle_state_blocked_why(
            pool,
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

    fn is_migrated(&self) -> bool {
        true
    }
}

#[register_custom_op]
pub struct StateCheckConsistencyOp;

#[async_trait]
impl CustomOperation for StateCheckConsistencyOp {
    fn domain(&self) -> &'static str {
        "state"
    }

    fn verb(&self) -> &'static str {
        "check-consistency"
    }

    fn rationale(&self) -> &'static str {
        "Reducer consistency scan across discovered slots"
    }

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let case_id = json_extract_uuid_opt(args, ctx, "case-id");
        let machine_name = json_extract_string_opt(args, "state-machine")
            .unwrap_or_else(|| "entity_kyc_lifecycle".to_string());
        let state_machine =
            load_builtin_state_machine(&machine_name).map_err(|err| anyhow!(err.to_string()))?;
        let result = handle_state_check_consistency(pool, cbu_id, case_id, &state_machine).await?;
        Ok(VerbExecutionOutcome::RecordSet(
            result
                .into_iter()
                .map(serde_json::to_value)
                .collect::<Result<Vec<_>, _>>()?,
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

#[register_custom_op]
pub struct StateOverrideOp;

#[async_trait]
impl CustomOperation for StateOverrideOp {
    fn domain(&self) -> &'static str {
        "state"
    }

    fn verb(&self) -> &'static str {
        "override"
    }

    fn rationale(&self) -> &'static str {
        "Reducer override write operation"
    }

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        pool: &PgPool,
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
        let machine_name = json_extract_string_opt(args, "state-machine")
            .unwrap_or_else(|| constellation_type.clone());
        let expires_at = parse_optional_datetime(json_extract_string_opt(args, "expires-at"))?;
        let conditions = json_extract_string_opt(args, "conditions");
        let state_machine =
            load_builtin_state_machine(&machine_name).map_err(|err| anyhow!(err.to_string()))?;
        let result = handle_state_override(
            pool,
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

    fn is_migrated(&self) -> bool {
        true
    }
}

#[register_custom_op]
pub struct StateRevokeOverrideOp;

#[async_trait]
impl CustomOperation for StateRevokeOverrideOp {
    fn domain(&self) -> &'static str {
        "state"
    }

    fn verb(&self) -> &'static str {
        "revoke-override"
    }

    fn rationale(&self) -> &'static str {
        "Reducer override revocation operation"
    }

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let override_id = json_extract_uuid(args, ctx, "override-id")?;
        let revoked_by = json_extract_string_opt(args, "revoked-by")
            .or_else(|| Some(ctx.principal.actor_id.clone()))
            .unwrap_or_else(|| "dsl_executor".to_string());
        let reason = json_extract_string(args, "reason")?;
        handle_state_revoke_override(pool, override_id, &revoked_by, &reason).await?;
        Ok(VerbExecutionOutcome::Record(json!({
            "override_id": override_id,
            "revoked": true,
        })))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

#[register_custom_op]
pub struct StateListOverridesOp;

#[async_trait]
impl CustomOperation for StateListOverridesOp {
    fn domain(&self) -> &'static str {
        "state"
    }

    fn verb(&self) -> &'static str {
        "list-overrides"
    }

    fn rationale(&self) -> &'static str {
        "List reducer overrides for a CBU"
    }

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let result = handle_state_list_overrides(pool, cbu_id).await?;
        Ok(VerbExecutionOutcome::RecordSet(
            result
                .into_iter()
                .map(serde_json::to_value)
                .collect::<Result<Vec<_>, _>>()?,
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}
