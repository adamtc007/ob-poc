//! Service-resource pipeline verbs — SemOS-side YAML-first re-implementation.
//!
//! 16 verbs spanning 7 domains (service-intent, discovery, attributes,
//! provisioning, readiness, pipeline, service-resource). Every op
//! dispatches to [`ServicePipelineService::dispatch_service_pipeline_verb`],
//! which orchestrates SRDEF engines in ob-poc. The service returns
//! `VerbExecutionOutcome` directly — no bindings, no wrapping. YAML
//! contracts in `config/verbs/service-{intent,pipeline,resource,availability}.yaml`.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::Value;
use uuid::Uuid;

use dsl_runtime::service_traits::ServicePipelineService;
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::SemOsVerbOp;

macro_rules! service_pipeline_op {
    ($struct:ident, $domain:literal, $verb:literal) => {
        pub struct $struct;

        #[async_trait]
        impl SemOsVerbOp for $struct {
            fn fqn(&self) -> &str {
                concat!($domain, ".", $verb)
            }
            async fn execute(
                &self,
                args: &serde_json::Value,
                ctx: &mut VerbExecutionContext,
                scope: &mut dyn TransactionScope,
            ) -> Result<VerbExecutionOutcome> {
                let service = ctx.service::<dyn ServicePipelineService>()?;
                service
                    .dispatch_service_pipeline_verb(scope.pool(), $domain, $verb, args)
                    .await
            }
        }
    };
}

async fn dispatch_service_pipeline_verb(
    args: &Value,
    ctx: &mut VerbExecutionContext,
    scope: &mut dyn TransactionScope,
    domain: &str,
    verb: &str,
) -> Result<VerbExecutionOutcome> {
    let service = ctx.service::<dyn ServicePipelineService>()?;
    service
        .dispatch_service_pipeline_verb(scope.pool(), domain, verb, args)
        .await
}

fn arg_uuid(args: &Value, name: &str) -> Option<Uuid> {
    args.get(name)
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
}

async fn resolve_cbu_id(
    args: &Value,
    scope: &mut dyn TransactionScope,
    domain: &str,
    verb: &str,
) -> Result<Option<Uuid>> {
    if let Some(cbu_id) = arg_uuid(args, "cbu-id") {
        return Ok(Some(cbu_id));
    }

    if (domain, verb) == ("service-intent", "supersede") {
        let Some(intent_id) = arg_uuid(args, "intent-id") else {
            return Ok(None);
        };
        let cbu_id = sqlx::query_scalar(
            r#"SELECT cbu_id FROM "ob-poc".service_intents WHERE intent_id = $1"#,
        )
        .bind(intent_id)
        .fetch_optional(scope.executor())
        .await?;
        return Ok(cbu_id);
    }

    Ok(None)
}

async fn set_cbu_discovery_state(
    cbu_id: Uuid,
    state: &str,
    ctx: &mut VerbExecutionContext,
    scope: &mut dyn TransactionScope,
    reason: &str,
) -> Result<()> {
    let affected = sqlx::query(
        r#"
        UPDATE "ob-poc".cbus
        SET cbu_discovery_state = $2,
            updated_at = NOW()
        WHERE cbu_id = $1
        "#,
    )
    .bind(cbu_id)
    .bind(state)
    .execute(scope.executor())
    .await?
    .rows_affected();

    if affected == 0 {
        return Err(anyhow!("CBU not found: {cbu_id}"));
    }

    let to_node = format!("cbu_discovery_state:{}", state.to_ascii_lowercase());
    dsl_runtime::domain_ops::helpers::emit_pending_state_advance(
        ctx,
        cbu_id,
        &to_node,
        "cbu/discovery-state",
        reason,
    );
    Ok(())
}

fn readiness_target_from_outcome(outcome: &VerbExecutionOutcome) -> &'static str {
    match outcome {
        VerbExecutionOutcome::Record(record) => {
            let blocked = record.get("blocked").and_then(|v| v.as_i64()).unwrap_or(0);
            if blocked == 0 {
                "READY"
            } else {
                "BLOCKED"
            }
        }
        _ => "BLOCKED",
    }
}

fn provisioning_target_from_outcome(outcome: &VerbExecutionOutcome) -> &'static str {
    match outcome {
        VerbExecutionOutcome::Record(record) => {
            let not_ready = record
                .get("not_ready")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            let services_blocked = record
                .get("services_blocked")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            if not_ready == 0 && services_blocked == 0 {
                "READY"
            } else {
                "BLOCKED"
            }
        }
        _ => "BLOCKED",
    }
}

fn pipeline_target_from_outcome(outcome: &VerbExecutionOutcome) -> &'static str {
    match outcome {
        VerbExecutionOutcome::Record(record) => {
            let services_blocked = record
                .get("readiness")
                .and_then(|v| v.get("services_blocked"))
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            if services_blocked == 0 {
                "READY"
            } else {
                "BLOCKED"
            }
        }
        _ => "BLOCKED",
    }
}

macro_rules! service_pipeline_state_op {
    ($struct:ident, $domain:literal, $verb:literal, fixed $state:literal) => {
        pub struct $struct;

        #[async_trait]
        impl SemOsVerbOp for $struct {
            fn fqn(&self) -> &str {
                concat!($domain, ".", $verb)
            }
            async fn execute(
                &self,
                args: &Value,
                ctx: &mut VerbExecutionContext,
                scope: &mut dyn TransactionScope,
            ) -> Result<VerbExecutionOutcome> {
                let cbu_id = resolve_cbu_id(args, scope, $domain, $verb).await?;
                let outcome =
                    dispatch_service_pipeline_verb(args, ctx, scope, $domain, $verb).await?;
                if let Some(cbu_id) = cbu_id {
                    let reason = format!("{}.{} -> {}", $domain, $verb, $state);
                    set_cbu_discovery_state(cbu_id, $state, ctx, scope, &reason).await?;
                }
                Ok(outcome)
            }
        }
    };
}

pub struct ServiceIntentCreate;

#[async_trait]
impl SemOsVerbOp for ServiceIntentCreate {
    fn fqn(&self) -> &str {
        "service-intent.create"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let outcome =
            dispatch_service_pipeline_verb(args, ctx, scope, "service-intent", "create").await?;
        let Some(cbu_id) = arg_uuid(args, "cbu-id") else {
            return Ok(outcome);
        };
        set_cbu_discovery_state(
            cbu_id,
            "PENDING",
            ctx,
            scope,
            "service-intent.create -> PENDING",
        )
        .await?;
        Ok(outcome)
    }
}

service_pipeline_op!(ServiceIntentList, "service-intent", "list");
service_pipeline_state_op!(ServiceIntentSupersede, "service-intent", "supersede", fixed "PENDING");

pub struct DiscoveryRun;

#[async_trait]
impl SemOsVerbOp for DiscoveryRun {
    fn fqn(&self) -> &str {
        "discovery.run"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = resolve_cbu_id(args, scope, "discovery", "run").await?;
        if let Some(cbu_id) = cbu_id {
            set_cbu_discovery_state(
                cbu_id,
                "DISCOVERING",
                ctx,
                scope,
                "discovery.run entry -> DISCOVERING",
            )
            .await?;
        }
        let outcome = dispatch_service_pipeline_verb(args, ctx, scope, "discovery", "run").await?;
        if let Some(cbu_id) = cbu_id {
            set_cbu_discovery_state(cbu_id, "ROLLUP", ctx, scope, "discovery.run -> ROLLUP")
                .await?;
        }
        Ok(outcome)
    }
}

service_pipeline_op!(DiscoveryExplain, "discovery", "explain");
service_pipeline_state_op!(AttributeRollup, "attributes", "rollup", fixed "ROLLUP");
service_pipeline_state_op!(AttributePopulate, "attributes", "populate", fixed "POPULATE");
service_pipeline_op!(AttributeGaps, "attributes", "gaps");
service_pipeline_op!(AttributeSet, "attributes", "set");

pub struct ProvisioningRun;

#[async_trait]
impl SemOsVerbOp for ProvisioningRun {
    fn fqn(&self) -> &str {
        "provisioning.run"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = resolve_cbu_id(args, scope, "provisioning", "run").await?;
        if let Some(cbu_id) = cbu_id {
            set_cbu_discovery_state(
                cbu_id,
                "PROVISION",
                ctx,
                scope,
                "provisioning.run entry -> PROVISION",
            )
            .await?;
        }
        let outcome =
            dispatch_service_pipeline_verb(args, ctx, scope, "provisioning", "run").await?;
        if let Some(cbu_id) = cbu_id {
            let target = provisioning_target_from_outcome(&outcome);
            let reason = format!("provisioning.run -> {target}");
            set_cbu_discovery_state(cbu_id, target, ctx, scope, &reason).await?;
        }
        Ok(outcome)
    }
}

service_pipeline_op!(ProvisioningStatus, "provisioning", "status");

pub struct ReadinessCompute;

#[async_trait]
impl SemOsVerbOp for ReadinessCompute {
    fn fqn(&self) -> &str {
        "readiness.compute"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = resolve_cbu_id(args, scope, "readiness", "compute").await?;
        let outcome =
            dispatch_service_pipeline_verb(args, ctx, scope, "readiness", "compute").await?;
        if let Some(cbu_id) = cbu_id {
            let target = readiness_target_from_outcome(&outcome);
            let reason = format!("readiness.compute -> {target}");
            set_cbu_discovery_state(cbu_id, target, ctx, scope, &reason).await?;
        }
        Ok(outcome)
    }
}

service_pipeline_op!(ReadinessExplain, "readiness", "explain");

pub struct PipelineFull;

#[async_trait]
impl SemOsVerbOp for PipelineFull {
    fn fqn(&self) -> &str {
        "pipeline.full"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = resolve_cbu_id(args, scope, "pipeline", "full").await?;
        if let Some(cbu_id) = cbu_id {
            set_cbu_discovery_state(
                cbu_id,
                "DISCOVERING",
                ctx,
                scope,
                "pipeline.full entry -> DISCOVERING",
            )
            .await?;
        }
        let outcome = dispatch_service_pipeline_verb(args, ctx, scope, "pipeline", "full").await?;
        if let Some(cbu_id) = cbu_id {
            let target = pipeline_target_from_outcome(&outcome);
            let reason = format!("pipeline.full -> {target}");
            set_cbu_discovery_state(cbu_id, target, ctx, scope, &reason).await?;
        }
        Ok(outcome)
    }
}

service_pipeline_op!(
    ServiceResourceCheckAttributeGaps,
    "service-resource",
    "check-attribute-gaps"
);
service_pipeline_op!(
    ServiceResourceSyncDefinitions,
    "service-resource",
    "sync-definitions"
);

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn readiness_outcome_maps_to_discovery_state() {
        assert_eq!(
            readiness_target_from_outcome(&VerbExecutionOutcome::Record(json!({"blocked": 0}))),
            "READY"
        );
        assert_eq!(
            readiness_target_from_outcome(&VerbExecutionOutcome::Record(json!({"blocked": 2}))),
            "BLOCKED"
        );
    }

    #[test]
    fn provisioning_outcome_maps_to_discovery_state() {
        assert_eq!(
            provisioning_target_from_outcome(&VerbExecutionOutcome::Record(json!({
                "not_ready": 0,
                "services_blocked": 0
            }))),
            "READY"
        );
        assert_eq!(
            provisioning_target_from_outcome(&VerbExecutionOutcome::Record(json!({
                "not_ready": 1,
                "services_blocked": 0
            }))),
            "BLOCKED"
        );
    }

    #[test]
    fn pipeline_outcome_maps_to_discovery_state() {
        assert_eq!(
            pipeline_target_from_outcome(&VerbExecutionOutcome::Record(json!({
                "readiness": {"services_blocked": 0}
            }))),
            "READY"
        );
        assert_eq!(
            pipeline_target_from_outcome(&VerbExecutionOutcome::Record(json!({
                "readiness": {"services_blocked": 1}
            }))),
            "BLOCKED"
        );
    }
}
