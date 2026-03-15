//! State reducer custom operations.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use ob_poc_macros::register_custom_op;
use serde_json::json;

use super::helpers::{extract_string, extract_string_opt, extract_uuid, extract_uuid_opt};
use super::{CustomOperation, ExecutionContext, ExecutionResult, VerbCall};
use crate::sem_reg::reducer::{
    handle_state_blocked_why, handle_state_check_consistency, handle_state_derive,
    handle_state_derive_all, handle_state_diagnose, handle_state_list_overrides,
    handle_state_override, handle_state_revoke_override, load_builtin_state_machine,
};

#[cfg(feature = "database")]
use sqlx::PgPool;

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

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let cbu_id = extract_uuid(verb_call, ctx, "cbu-id")?;
        let entity_id = extract_uuid(verb_call, ctx, "entity-id")?;
        let slot_path = extract_string(verb_call, "slot-path")?;
        let case_id = extract_uuid_opt(verb_call, ctx, "case-id");
        let machine_name = extract_string_opt(verb_call, "state-machine")
            .unwrap_or_else(|| "entity_kyc_lifecycle".to_string());
        let state_machine =
            load_builtin_state_machine(&machine_name).map_err(|err| anyhow!(err.to_string()))?;
        let result =
            handle_state_derive(pool, cbu_id, entity_id, &slot_path, case_id, &state_machine)
                .await?;
        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("state.derive requires database"))
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

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let cbu_id = extract_uuid(verb_call, ctx, "cbu-id")?;
        let entity_id = extract_uuid(verb_call, ctx, "entity-id")?;
        let slot_path = extract_string(verb_call, "slot-path")?;
        let case_id = extract_uuid_opt(verb_call, ctx, "case-id");
        let machine_name = extract_string_opt(verb_call, "state-machine")
            .unwrap_or_else(|| "entity_kyc_lifecycle".to_string());
        let state_machine =
            load_builtin_state_machine(&machine_name).map_err(|err| anyhow!(err.to_string()))?;
        let result =
            handle_state_diagnose(pool, cbu_id, entity_id, &slot_path, case_id, &state_machine)
                .await?;
        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("state.diagnose requires database"))
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

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let cbu_id = extract_uuid(verb_call, ctx, "cbu-id")?;
        let case_id = extract_uuid_opt(verb_call, ctx, "case-id");
        let machine_name = extract_string_opt(verb_call, "state-machine")
            .unwrap_or_else(|| "entity_kyc_lifecycle".to_string());
        let state_machine =
            load_builtin_state_machine(&machine_name).map_err(|err| anyhow!(err.to_string()))?;
        let result = handle_state_derive_all(pool, cbu_id, case_id, &state_machine).await?;
        Ok(ExecutionResult::RecordSet(
            result
                .into_iter()
                .map(serde_json::to_value)
                .collect::<Result<Vec<_>, _>>()?,
        ))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("state.derive-all requires database"))
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

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let cbu_id = extract_uuid(verb_call, ctx, "cbu-id")?;
        let entity_id = extract_uuid(verb_call, ctx, "entity-id")?;
        let slot_path = extract_string(verb_call, "slot-path")?;
        let verb = extract_string(verb_call, "verb")?;
        let case_id = extract_uuid_opt(verb_call, ctx, "case-id");
        let machine_name = extract_string_opt(verb_call, "state-machine")
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
        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("state.blocked-why requires database"))
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

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let cbu_id = extract_uuid(verb_call, ctx, "cbu-id")?;
        let case_id = extract_uuid_opt(verb_call, ctx, "case-id");
        let machine_name = extract_string_opt(verb_call, "state-machine")
            .unwrap_or_else(|| "entity_kyc_lifecycle".to_string());
        let state_machine =
            load_builtin_state_machine(&machine_name).map_err(|err| anyhow!(err.to_string()))?;
        let result = handle_state_check_consistency(pool, cbu_id, case_id, &state_machine).await?;
        Ok(ExecutionResult::RecordSet(
            result
                .into_iter()
                .map(serde_json::to_value)
                .collect::<Result<Vec<_>, _>>()?,
        ))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("state.check-consistency requires database"))
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

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let cbu_id = extract_uuid(verb_call, ctx, "cbu-id")?;
        let entity_id = extract_uuid(verb_call, ctx, "entity-id")?;
        let slot_path = extract_string(verb_call, "slot-path")?;
        let override_state = extract_string(verb_call, "override-state")?;
        let justification = extract_string(verb_call, "justification")?;
        let authority = extract_string(verb_call, "authority")?;
        let case_id = extract_uuid_opt(verb_call, ctx, "case-id");
        let constellation_type = extract_string_opt(verb_call, "constellation-type")
            .unwrap_or_else(|| "entity_kyc_lifecycle".to_string());
        let machine_name = extract_string_opt(verb_call, "state-machine")
            .unwrap_or_else(|| constellation_type.clone());
        let expires_at = parse_optional_datetime(extract_string_opt(verb_call, "expires-at"))?;
        let conditions = extract_string_opt(verb_call, "conditions");
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
        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("state.override requires database"))
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

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let override_id = extract_uuid(verb_call, ctx, "override-id")?;
        let revoked_by = extract_string_opt(verb_call, "revoked-by")
            .or_else(|| ctx.audit_user.clone())
            .unwrap_or_else(|| "dsl_executor".to_string());
        let reason = extract_string(verb_call, "reason")?;
        handle_state_revoke_override(pool, override_id, &revoked_by, &reason).await?;
        Ok(ExecutionResult::Record(json!({
            "override_id": override_id,
            "revoked": true,
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("state.revoke-override requires database"))
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

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let cbu_id = extract_uuid(verb_call, ctx, "cbu-id")?;
        let result = handle_state_list_overrides(pool, cbu_id).await?;
        Ok(ExecutionResult::RecordSet(
            result
                .into_iter()
                .map(serde_json::to_value)
                .collect::<Result<Vec<_>, _>>()?,
        ))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("state.list-overrides requires database"))
    }
}
