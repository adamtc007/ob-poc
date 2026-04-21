//! Lifecycle resource instance verbs (12 plugin verbs — 6 canonical
//! + 6 `service-resource.*-lifecycle` compat aliases). YAML-first
//! re-implementation of `lifecycle.*` from
//! `rust/config/verbs/lifecycle.yaml` (plus the 6 compat aliases
//! carried from `service-resource.yaml`).
//!
//! Canonical ops:
//! - `lifecycle.provision` — upsert `cbu_lifecycle_instances` with
//!   context validation
//! - `lifecycle.analyze-gaps` — query `v_cbu_lifecycle_gaps` view
//! - `lifecycle.check-readiness` — gaps → blocking vs warning
//! - `lifecycle.discover` — tree-walk lifecycles + resource types
//! - `lifecycle.generate-plan` — synthesize provisioning DSL from gaps
//! - `lifecycle.execute-plan` — dry-run-aware DSL execution stub
//!
//! The 6 `service-resource.*-lifecycle` variants are pure delegation
//! wrappers kept for backwards-compatibility with legacy verb calls.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::{json, Value};
use uuid::Uuid;

use dsl_runtime::domain_ops::helpers::{
    json_extract_bool_opt, json_extract_string, json_extract_string_opt, json_extract_uuid,
    json_extract_uuid_opt,
};
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::SemOsVerbOp;

// ---------------------------------------------------------------------------
// Shared loader helpers (used by canonical ops + compat aliases)
// ---------------------------------------------------------------------------

async fn do_provision(
    args: &Value,
    ctx: &mut VerbExecutionContext,
    scope: &mut dyn TransactionScope,
) -> Result<VerbExecutionOutcome> {
    let cbu_id: Uuid = json_extract_uuid(args, ctx, "cbu-id")?;
    let resource_type_code = json_extract_string(args, "resource-type")?;

    let rt: Option<(Uuid, bool, bool, bool)> = sqlx::query_as(
        r#"SELECT resource_type_id, per_market, per_currency, per_counterparty
           FROM "ob-poc".lifecycle_resource_types WHERE code = $1"#,
    )
    .bind(&resource_type_code)
    .fetch_optional(scope.executor())
    .await?;
    let (resource_type_id, per_market, _per_currency, per_counterparty) =
        rt.ok_or_else(|| anyhow!("Unknown lifecycle resource type: {}", resource_type_code))?;

    let market_id: Option<Uuid> = json_extract_uuid_opt(args, ctx, "market");
    let market_id: Option<Uuid> = if market_id.is_none() {
        if let Some(mic) = json_extract_string_opt(args, "market") {
            sqlx::query_scalar(r#"SELECT market_id FROM "ob-poc".markets WHERE mic = $1"#)
                .bind(mic)
                .fetch_optional(scope.executor())
                .await?
        } else {
            None
        }
    } else {
        market_id
    };

    let currency: Option<String> = json_extract_string_opt(args, "currency");
    let counterparty_id: Option<Uuid> = json_extract_uuid_opt(args, ctx, "counterparty");

    if per_market && market_id.is_none() {
        return Err(anyhow!(
            "market is required for resource type {}",
            resource_type_code
        ));
    }
    if per_counterparty && counterparty_id.is_none() {
        return Err(anyhow!(
            "counterparty is required for resource type {}",
            resource_type_code
        ));
    }

    let provider_code = json_extract_string_opt(args, "provider");
    let provider_account = json_extract_string_opt(args, "provider-account");
    let provider_bic = json_extract_string_opt(args, "provider-bic");
    let config: Option<Value> = args
        .get("config")
        .and_then(|v| if v.is_object() { Some(v.clone()) } else { None });

    let context_suffix = market_id
        .map(|m| m.to_string())
        .or_else(|| counterparty_id.map(|c| c.to_string()))
        .or_else(|| currency.clone())
        .unwrap_or_else(|| "default".to_string());
    let instance_url = format!(
        "cbu:{}/lifecycle/{}/{}",
        cbu_id, resource_type_code, context_suffix
    );

    let instance_identifier = json_extract_string_opt(args, "instance-identifier");
    let instance_id = Uuid::new_v4();

    let (result_id,): (Uuid,) = sqlx::query_as(
        r#"WITH ins AS (
            INSERT INTO "ob-poc".cbu_lifecycle_instances
            (instance_id, cbu_id, resource_type_id, instance_identifier, instance_url,
             market_id, currency, counterparty_entity_id, status,
             provider_code, provider_account, provider_bic, config, provisioned_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'PROVISIONED', $9, $10, $11, $12, NOW())
            ON CONFLICT (instance_url) DO UPDATE SET
                provider_code = COALESCE(EXCLUDED.provider_code, cbu_lifecycle_instances.provider_code),
                provider_account = COALESCE(EXCLUDED.provider_account, cbu_lifecycle_instances.provider_account),
                provider_bic = COALESCE(EXCLUDED.provider_bic, cbu_lifecycle_instances.provider_bic),
                config = COALESCE(EXCLUDED.config, cbu_lifecycle_instances.config),
                updated_at = NOW()
            RETURNING instance_id
        )
        SELECT instance_id FROM ins
        UNION ALL
        SELECT instance_id FROM "ob-poc".cbu_lifecycle_instances
        WHERE instance_url = $5
        AND NOT EXISTS (SELECT 1 FROM ins)
        LIMIT 1"#,
    )
    .bind(instance_id)
    .bind(cbu_id)
    .bind(resource_type_id)
    .bind(&instance_identifier)
    .bind(&instance_url)
    .bind(market_id)
    .bind(&currency)
    .bind(counterparty_id)
    .bind(&provider_code)
    .bind(&provider_account)
    .bind(&provider_bic)
    .bind(&config)
    .fetch_one(scope.executor())
    .await?;

    ctx.bind("instance", result_id);
    Ok(VerbExecutionOutcome::Record(json!({
        "instance_id": result_id,
        "instance_url": instance_url,
        "status": "PROVISIONED"
    })))
}

async fn do_analyze_gaps(
    args: &Value,
    ctx: &mut VerbExecutionContext,
    scope: &mut dyn TransactionScope,
) -> Result<VerbExecutionOutcome> {
    let cbu_id: Uuid = json_extract_uuid(args, ctx, "cbu-id")?;

    type GapRow = (
        Uuid, String, String, Option<String>, Option<String>,
        String, String, bool, String, String,
        Option<String>, Option<String>, bool, bool, bool,
    );
    let gaps: Vec<GapRow> = sqlx::query_as(
        r#"SELECT
            cbu_id, cbu_name, instrument_class, market, counterparty_name,
            lifecycle_code, lifecycle_name, is_mandatory,
            missing_resource_code, missing_resource_name, provisioning_verb,
            location_type, per_market, per_currency, per_counterparty
           FROM "ob-poc".v_cbu_lifecycle_gaps
           WHERE cbu_id = $1
           ORDER BY instrument_class, lifecycle_code, missing_resource_code"#,
    )
    .bind(cbu_id)
    .fetch_all(scope.executor())
    .await?;

    let out: Vec<Value> = gaps
        .into_iter()
        .map(|g| json!({
            "cbu_id": g.0,
            "cbu_name": g.1,
            "instrument_class": g.2,
            "market": g.3,
            "counterparty_name": g.4,
            "lifecycle_code": g.5,
            "lifecycle_name": g.6,
            "is_mandatory": g.7,
            "missing_resource_code": g.8,
            "missing_resource_name": g.9,
            "provisioning_verb": g.10,
            "location_type": g.11,
            "per_market": g.12,
            "per_currency": g.13,
            "per_counterparty": g.14
        }))
        .collect();
    Ok(VerbExecutionOutcome::RecordSet(out))
}

async fn do_check_readiness(
    args: &Value,
    ctx: &mut VerbExecutionContext,
    scope: &mut dyn TransactionScope,
) -> Result<VerbExecutionOutcome> {
    let cbu_id: Uuid = json_extract_uuid(args, ctx, "cbu-id")?;
    let instrument_class = json_extract_string(args, "instrument-class")?;
    let market: Option<String> = json_extract_string_opt(args, "market");

    let gaps: Vec<(String, String, bool, String)> = sqlx::query_as(
        r#"SELECT lifecycle_code, missing_resource_code, is_mandatory, missing_resource_name
           FROM "ob-poc".v_cbu_lifecycle_gaps
           WHERE cbu_id = $1
             AND instrument_class = $2
             AND ($3::text IS NULL OR market = $3)"#,
    )
    .bind(cbu_id)
    .bind(&instrument_class)
    .bind(market.as_deref())
    .fetch_all(scope.executor())
    .await?;

    let blocking_gaps: Vec<Value> = gaps
        .iter()
        .filter(|g| g.2)
        .map(|g| json!({"lifecycle": g.0, "resource_code": g.1, "resource_name": g.3}))
        .collect();
    let warnings: Vec<Value> = gaps
        .iter()
        .filter(|g| !g.2)
        .map(|g| json!({"lifecycle": g.0, "resource_code": g.1, "resource_name": g.3}))
        .collect();

    Ok(VerbExecutionOutcome::Record(json!({
        "ready": blocking_gaps.is_empty(),
        "instrument_class": instrument_class,
        "market": market,
        "blocking_gaps": blocking_gaps,
        "warnings": warnings
    })))
}

async fn do_discover(
    args: &Value,
    _ctx: &mut VerbExecutionContext,
    scope: &mut dyn TransactionScope,
) -> Result<VerbExecutionOutcome> {
    let instrument_class = json_extract_string(args, "instrument-class")?;
    let include_optional = json_extract_bool_opt(args, "include-optional").unwrap_or(false);

    let lifecycles: Vec<(String, String, bool, bool)> = sqlx::query_as(
        r#"SELECT l.code, l.name, il.is_mandatory, il.requires_isda
           FROM "ob-poc".lifecycles l
           JOIN "ob-poc".instrument_lifecycles il ON il.lifecycle_id = l.lifecycle_id
           JOIN "ob-poc".instrument_classes ic ON ic.class_id = il.instrument_class_id
           WHERE ic.code = $1
             AND il.is_active = true
             AND l.is_active = true
             AND ($2 OR il.is_mandatory = true)
           ORDER BY il.display_order"#,
    )
    .bind(&instrument_class)
    .bind(include_optional)
    .fetch_all(scope.executor())
    .await?;

    let requires_isda = lifecycles.iter().any(|(_, _, _, r)| *r);

    let mut mandatory = Vec::new();
    let mut optional = Vec::new();
    for (code, name, is_mandatory, _) in &lifecycles {
        let resources: Vec<(String, String, bool)> = sqlx::query_as(
            r#"SELECT lrt.code, lrt.name, lrc.is_required
               FROM "ob-poc".lifecycle_resource_types lrt
               JOIN "ob-poc".lifecycle_resource_capabilities lrc
                 ON lrc.resource_type_id = lrt.resource_type_id
               JOIN "ob-poc".lifecycles l ON l.lifecycle_id = lrc.lifecycle_id
               WHERE l.code = $1
                 AND lrt.is_active = true
                 AND lrc.is_active = true
               ORDER BY lrc.priority"#,
        )
        .bind(code)
        .fetch_all(scope.executor())
        .await?;

        let entry = json!({
            "code": code,
            "name": name,
            "resources": resources.iter().map(|(c, n, r)| {
                json!({"code": c, "name": n, "required": r})
            }).collect::<Vec<_>>()
        });
        if *is_mandatory {
            mandatory.push(entry);
        } else {
            optional.push(entry);
        }
    }

    Ok(VerbExecutionOutcome::Record(json!({
        "instrument_class": instrument_class,
        "requires_isda": requires_isda,
        "mandatory_lifecycles": mandatory,
        "optional_lifecycles": optional
    })))
}

async fn do_generate_plan(
    args: &Value,
    ctx: &mut VerbExecutionContext,
    scope: &mut dyn TransactionScope,
) -> Result<VerbExecutionOutcome> {
    let cbu_id: Uuid = json_extract_uuid(args, ctx, "cbu-id")?;
    let user_responses: Value = args
        .get("user-responses")
        .and_then(|v| if v.is_object() { Some(v.clone()) } else { None })
        .unwrap_or_else(|| json!({}));

    type GapRow = (
        String, Option<String>, Option<String>, String, bool,
        String, String, Option<String>, Option<String>, bool, bool, bool,
    );
    let gaps: Vec<GapRow> = sqlx::query_as(
        r#"SELECT instrument_class, market, counterparty_name, lifecycle_code,
                  is_mandatory, missing_resource_code, missing_resource_name,
                  provisioning_verb, location_type, per_market, per_currency, per_counterparty
           FROM "ob-poc".v_cbu_lifecycle_gaps
           WHERE cbu_id = $1"#,
    )
    .bind(cbu_id)
    .fetch_all(scope.executor())
    .await?;

    let mut dsl_statements = Vec::new();
    let mut pending_prompts = Vec::new();

    for gap in gaps {
        let resource_code = &gap.5;
        if let Some(verb) = &gap.7 {
            let has_response = user_responses.get(resource_code).is_some();
            if has_response {
                let mut stmt = format!(
                    "({} :cbu-id \"{}\" :resource-type \"{}\"",
                    verb, cbu_id, resource_code
                );
                if let Some(market) = &gap.1 {
                    stmt.push_str(&format!(" :market \"{}\"", market));
                }
                if let Some(counterparty) = &gap.2 {
                    stmt.push_str(&format!(" :counterparty \"{}\"", counterparty));
                }
                if let Some(resp) = user_responses.get(resource_code) {
                    if let Some(provider) = resp.get("provider").and_then(|v| v.as_str()) {
                        stmt.push_str(&format!(" :provider \"{}\"", provider));
                    }
                    if let Some(bic) = resp.get("bic").and_then(|v| v.as_str()) {
                        stmt.push_str(&format!(" :provider-bic \"{}\"", bic));
                    }
                }
                stmt.push(')');
                dsl_statements.push(stmt);
            } else {
                pending_prompts.push(json!({
                    "resource_code": resource_code,
                    "resource_name": gap.6,
                    "instrument_class": gap.0,
                    "market": gap.1,
                    "counterparty": gap.2,
                    "location_type": gap.8,
                    "prompt": format!(
                        "For {} in {}: Which provider for {}?",
                        gap.0,
                        gap.1.as_deref().or(gap.2.as_deref()).unwrap_or("your setup"),
                        gap.6
                    )
                }));
            }
        }
    }

    Ok(VerbExecutionOutcome::Record(json!({
        "dsl_statements": dsl_statements,
        "pending_prompts": pending_prompts,
        "ready_to_execute": pending_prompts.is_empty()
    })))
}

async fn do_execute_plan(
    args: &Value,
    _ctx: &mut VerbExecutionContext,
    _scope: &mut dyn TransactionScope,
) -> Result<VerbExecutionOutcome> {
    let dry_run = json_extract_bool_opt(args, "dry-run").unwrap_or(false);
    let plan = args
        .get("plan")
        .and_then(|v| if v.is_object() { Some(v.clone()) } else { None })
        .ok_or_else(|| anyhow!("Missing plan argument"))?;
    let statements = plan
        .get("dsl_statements")
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow!("dsl_statements required in plan"))?;

    let mut results = Vec::new();
    for stmt in statements {
        let dsl = stmt.as_str().unwrap_or("");
        if dry_run {
            results.push(json!({
                "statement": dsl,
                "status": "would_execute"
            }));
        } else {
            results.push(json!({
                "statement": dsl,
                "status": "pending_execution",
                "note": "DSL execution integration pending"
            }));
        }
    }
    Ok(VerbExecutionOutcome::Record(json!({
        "dry_run": dry_run,
        "executed": results.len(),
        "results": results
    })))
}

// ---------------------------------------------------------------------------
// Canonical + compat ops
// ---------------------------------------------------------------------------

macro_rules! lifecycle_op {
    ($name:ident, $fqn:expr, $handler:ident) => {
        pub struct $name;

        #[async_trait]
        impl SemOsVerbOp for $name {
            fn fqn(&self) -> &str {
                $fqn
            }
            async fn execute(
                &self,
                args: &Value,
                ctx: &mut VerbExecutionContext,
                scope: &mut dyn TransactionScope,
            ) -> Result<VerbExecutionOutcome> {
                $handler(args, ctx, scope).await
            }
        }
    };
}

lifecycle_op!(Provision, "lifecycle.provision", do_provision);
lifecycle_op!(AnalyzeGaps, "lifecycle.analyze-gaps", do_analyze_gaps);
lifecycle_op!(CheckReadiness, "lifecycle.check-readiness", do_check_readiness);
lifecycle_op!(Discover, "lifecycle.discover", do_discover);
lifecycle_op!(GeneratePlan, "lifecycle.generate-plan", do_generate_plan);
lifecycle_op!(ExecutePlan, "lifecycle.execute-plan", do_execute_plan);

lifecycle_op!(
    ServiceProvisionLifecycle,
    "service-resource.provision-lifecycle",
    do_provision
);
lifecycle_op!(
    ServiceAnalyzeLifecycleGaps,
    "service-resource.analyze-lifecycle-gaps",
    do_analyze_gaps
);
lifecycle_op!(
    ServiceCheckLifecycleReadiness,
    "service-resource.check-lifecycle-readiness",
    do_check_readiness
);
lifecycle_op!(
    ServiceDiscoverLifecycles,
    "service-resource.discover-lifecycles",
    do_discover
);
lifecycle_op!(
    ServiceGenerateLifecyclePlan,
    "service-resource.generate-lifecycle-plan",
    do_generate_plan
);
lifecycle_op!(
    ServiceExecuteLifecyclePlan,
    "service-resource.execute-lifecycle-plan",
    do_execute_plan
);
