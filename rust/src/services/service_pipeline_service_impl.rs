//! ob-poc impl of [`dsl_runtime::service_traits::ServicePipelineService`].
//!
//! Single-method dispatch for all 16 verbs across the
//! intent → discovery → attribute → provisioning → readiness pipeline.
//! Bridge stays in ob-poc because it consumes
//! `crate::service_resources::*` (engines, orchestrators, SRDEF
//! registry loader) — multi-consumer modules that have no dsl-runtime
//! analogue.
//!
//! Dispatch table is a verbatim port of the previous
//! `execute_json` bodies from the relocated
//! `dsl-runtime::domain_ops::service_pipeline_ops`.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::{json, Value};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use dsl_runtime::execution::VerbExecutionOutcome;
use dsl_runtime::service_traits::ServicePipelineService;

use crate::service_resources::{
    load_and_sync_srdefs, load_srdefs_from_config, run_discovery_pipeline,
    run_provisioning_pipeline, AttributeRollupEngine, AttributeSource, NewServiceIntent,
    PopulationEngine, ReadinessEngine, ServiceResourcePipelineService, SetCbuAttrValue,
};

pub struct ObPocServicePipelineService;

impl ObPocServicePipelineService {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ObPocServicePipelineService {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ServicePipelineService for ObPocServicePipelineService {
    async fn dispatch_service_pipeline_verb(
        &self,
        pool: &PgPool,
        domain: &str,
        verb_name: &str,
        args: &Value,
    ) -> Result<VerbExecutionOutcome> {
        match (domain, verb_name) {
            ("service-intent", "create") => service_intent_create(pool, args).await,
            ("service-intent", "list") => service_intent_list(pool, args).await,
            ("service-intent", "supersede") => service_intent_supersede(pool, args).await,
            ("discovery", "run") => discovery_run(pool, args).await,
            ("discovery", "explain") => discovery_explain(pool, args).await,
            ("attributes", "rollup") => attribute_rollup(pool, args).await,
            ("attributes", "populate") => attribute_populate(pool, args).await,
            ("attributes", "gaps") => attribute_gaps(pool, args).await,
            ("attributes", "set") => attribute_set(pool, args).await,
            ("provisioning", "run") => provisioning_run(pool, args).await,
            ("provisioning", "status") => provisioning_status(pool, args).await,
            ("readiness", "compute") => readiness_compute(pool, args).await,
            ("readiness", "explain") => readiness_explain(pool, args).await,
            ("pipeline", "full") => pipeline_full(pool, args).await,
            ("service-resource", "check-attribute-gaps") => {
                service_resource_check_attribute_gaps(pool).await
            }
            ("service-resource", "sync-definitions") => {
                service_resource_sync_definitions(pool).await
            }
            (d, v) => Err(anyhow!("unknown service-pipeline verb: {d}.{v}")),
        }
    }
}

// ── Argument helpers ──────────────────────────────────────────────────────────

fn arg_uuid(args: &Value, name: &str) -> Result<Uuid> {
    args.get(name)
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
        .ok_or_else(|| anyhow!("Missing or invalid {name} argument"))
}

fn arg_uuid_opt(args: &Value, name: &str) -> Option<Uuid> {
    args.get(name)
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
}

fn arg_string(args: &Value, name: &str) -> Result<String> {
    args.get(name)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow!("Missing {name} argument"))
}

fn arg_string_opt(args: &Value, name: &str) -> Option<String> {
    args.get(name)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

// ── service-intent.create ─────────────────────────────────────────────────────

async fn service_intent_create(pool: &PgPool, args: &Value) -> Result<VerbExecutionOutcome> {
    let cbu_id = arg_uuid(args, "cbu-id")?;
    let product_id = arg_uuid(args, "product-id")?;
    let service_id = arg_uuid(args, "service-id")?;
    let options = args.get("options").cloned();
    let service = ServiceResourcePipelineService::new(pool.clone());
    let input = NewServiceIntent {
        cbu_id,
        product_id,
        service_id,
        options,
        created_by: None,
    };
    let intent_id = service.create_service_intent(&input).await?;
    Ok(VerbExecutionOutcome::Uuid(intent_id))
}

// ── service-intent.list ───────────────────────────────────────────────────────

async fn service_intent_list(pool: &PgPool, args: &Value) -> Result<VerbExecutionOutcome> {
    let cbu_id = arg_uuid(args, "cbu-id")?;
    let service = ServiceResourcePipelineService::new(pool.clone());
    let intents = service.get_service_intents(cbu_id).await?;
    Ok(VerbExecutionOutcome::RecordSet(
        intents
            .iter()
            .map(|i| {
                json!({
                    "intent_id": i.intent_id,
                    "cbu_id": i.cbu_id,
                    "product_id": i.product_id,
                    "service_id": i.service_id,
                    "options": i.options,
                    "status": i.status,
                    "created_at": i.created_at.map(|dt| dt.to_rfc3339()),
                })
            })
            .collect(),
    ))
}

// ── service-intent.supersede ──────────────────────────────────────────────────

async fn service_intent_supersede(pool: &PgPool, args: &Value) -> Result<VerbExecutionOutcome> {
    let intent_id = arg_uuid(args, "intent-id")?;
    let options = args
        .get("options")
        .cloned()
        .ok_or_else(|| anyhow!("Missing options argument"))?;
    let existing: (Uuid, Uuid, Uuid) = sqlx::query_as(
        r#"SELECT cbu_id, product_id, service_id FROM "ob-poc".service_intents WHERE intent_id = $1"#,
    )
    .bind(intent_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| anyhow!("Intent not found: {intent_id}"))?;
    sqlx::query(
        r#"UPDATE "ob-poc".service_intents SET status = 'superseded' WHERE intent_id = $1"#,
    )
    .bind(intent_id)
    .execute(pool)
    .await?;
    let new_id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO "ob-poc".service_intents (cbu_id, product_id, service_id, options)
           VALUES ($1, $2, $3, $4) RETURNING intent_id"#,
    )
    .bind(existing.0)
    .bind(existing.1)
    .bind(existing.2)
    .bind(&options)
    .fetch_one(pool)
    .await?;
    Ok(VerbExecutionOutcome::Uuid(new_id))
}

// ── discovery.run ─────────────────────────────────────────────────────────────

async fn discovery_run(pool: &PgPool, args: &Value) -> Result<VerbExecutionOutcome> {
    let cbu_id = arg_uuid(args, "cbu-id")?;
    let registry = load_srdefs_from_config().unwrap_or_default();
    let result = run_discovery_pipeline(pool, &registry, cbu_id).await?;
    Ok(VerbExecutionOutcome::Record(json!({
        "cbu_id": result.cbu_id,
        "srdefs_discovered": result.srdefs_discovered,
        "attrs_rolled_up": result.attrs_rolled_up,
        "attrs_populated": result.attrs_populated,
        "attrs_missing": result.attrs_missing,
    })))
}

// ── discovery.explain ─────────────────────────────────────────────────────────

#[derive(sqlx::FromRow)]
struct DiscoveryReasonRow {
    srdef_id: String,
    service_id: Uuid,
    trigger_type: String,
    reason_detail: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
}

async fn discovery_explain(pool: &PgPool, args: &Value) -> Result<VerbExecutionOutcome> {
    let cbu_id = arg_uuid(args, "cbu-id")?;
    let srdef_filter = arg_string_opt(args, "srdef-id");
    let reasons: Vec<DiscoveryReasonRow> = if let Some(srdef_id) = srdef_filter {
        sqlx::query_as(
            r#"SELECT srdef_id, service_id, trigger_type, reason_detail, discovered_at as created_at
               FROM "ob-poc".srdef_discovery_reasons
               WHERE cbu_id = $1 AND srdef_id = $2 ORDER BY discovered_at DESC"#,
        )
        .bind(cbu_id)
        .bind(srdef_id)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as(
            r#"SELECT srdef_id, service_id, trigger_type, reason_detail, discovered_at as created_at
               FROM "ob-poc".srdef_discovery_reasons
               WHERE cbu_id = $1 ORDER BY srdef_id, discovered_at DESC"#,
        )
        .bind(cbu_id)
        .fetch_all(pool)
        .await?
    };
    Ok(VerbExecutionOutcome::RecordSet(
        reasons
            .iter()
            .map(|r| {
                json!({
                    "srdef_id": r.srdef_id,
                    "service_id": r.service_id,
                    "trigger_type": r.trigger_type,
                    "reason_detail": r.reason_detail,
                    "created_at": r.created_at.to_rfc3339(),
                })
            })
            .collect(),
    ))
}

// ── attributes.rollup ─────────────────────────────────────────────────────────

async fn attribute_rollup(pool: &PgPool, args: &Value) -> Result<VerbExecutionOutcome> {
    let cbu_id = arg_uuid(args, "cbu-id")?;
    let engine = AttributeRollupEngine::new(pool);
    let result = engine.rollup_for_cbu(cbu_id).await?;
    Ok(VerbExecutionOutcome::Record(json!({
        "total_attributes": result.total_attributes,
        "required_count": result.required_count,
        "optional_count": result.optional_count,
        "conflict_count": result.conflict_count,
    })))
}

// ── attributes.populate ───────────────────────────────────────────────────────

async fn attribute_populate(pool: &PgPool, args: &Value) -> Result<VerbExecutionOutcome> {
    let cbu_id = arg_uuid(args, "cbu-id")?;
    let engine = PopulationEngine::new(pool);
    let result = engine.populate_for_cbu(cbu_id).await?;
    Ok(VerbExecutionOutcome::Record(json!({
        "populated": result.populated,
        "already_populated": result.already_populated,
        "still_missing": result.still_missing,
    })))
}

// ── attributes.gaps ───────────────────────────────────────────────────────────

#[derive(sqlx::FromRow)]
struct AttrGapRow {
    attr_id: Uuid,
    attr_code: String,
    attr_name: String,
    attr_category: String,
    #[allow(dead_code)]
    has_value: bool,
}

async fn attribute_gaps(pool: &PgPool, args: &Value) -> Result<VerbExecutionOutcome> {
    let cbu_id = arg_uuid(args, "cbu-id")?;
    let gaps: Vec<AttrGapRow> = sqlx::query_as(
        r#"SELECT attr_id, attr_code, attr_name, attr_category, has_value
           FROM "ob-poc".v_cbu_attr_gaps
           WHERE cbu_id = $1 AND NOT has_value
           ORDER BY attr_category, attr_name"#,
    )
    .bind(cbu_id)
    .fetch_all(pool)
    .await?;
    Ok(VerbExecutionOutcome::RecordSet(
        gaps.iter()
            .map(|g| {
                json!({
                    "attr_id": g.attr_id,
                    "attr_code": g.attr_code,
                    "attr_name": g.attr_name,
                    "attr_category": g.attr_category,
                })
            })
            .collect(),
    ))
}

// ── attributes.set ────────────────────────────────────────────────────────────

async fn attribute_set(pool: &PgPool, args: &Value) -> Result<VerbExecutionOutcome> {
    let cbu_id = arg_uuid(args, "cbu-id")?;
    let attr_id = arg_uuid(args, "attr-id")?;
    let value = arg_string(args, "value")?;
    let service = ServiceResourcePipelineService::new(pool.clone());
    let input = SetCbuAttrValue {
        cbu_id,
        attr_id,
        value: Value::String(value),
        source: AttributeSource::Manual,
        evidence_refs: None,
        explain_refs: None,
    };
    service.set_cbu_attr_value(&input).await?;
    Ok(VerbExecutionOutcome::Affected(1))
}

// ── provisioning.run ──────────────────────────────────────────────────────────

async fn provisioning_run(pool: &PgPool, args: &Value) -> Result<VerbExecutionOutcome> {
    let cbu_id = arg_uuid(args, "cbu-id")?;
    let registry = load_srdefs_from_config().unwrap_or_default();
    let result = run_provisioning_pipeline(pool, &registry, cbu_id).await?;
    Ok(VerbExecutionOutcome::Record(json!({
        "cbu_id": result.cbu_id,
        "requests_created": result.requests_created,
        "already_active": result.already_active,
        "not_ready": result.not_ready,
        "services_ready": result.services_ready,
        "services_partial": result.services_partial,
        "services_blocked": result.services_blocked,
    })))
}

// ── provisioning.status ───────────────────────────────────────────────────────

async fn provisioning_status(pool: &PgPool, args: &Value) -> Result<VerbExecutionOutcome> {
    let request_id = arg_uuid(args, "request-id")?;
    let row = sqlx::query(
        r#"SELECT pr.request_id, pr.cbu_id, pr.srdef_id, pr.status, pr.requested_at,
                  pe.kind as event_kind, pe.occurred_at as event_at
           FROM "ob-poc".provisioning_requests pr
           LEFT JOIN "ob-poc".provisioning_events pe ON pr.request_id = pe.request_id
           WHERE pr.request_id = $1
           ORDER BY pe.occurred_at DESC NULLS LAST LIMIT 1"#,
    )
    .bind(request_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| anyhow!("Request not found: {request_id}"))?;
    Ok(VerbExecutionOutcome::Record(json!({
        "request_id": row.get::<Uuid, _>("request_id"),
        "cbu_id": row.get::<Uuid, _>("cbu_id"),
        "srdef_id": row.get::<String, _>("srdef_id"),
        "status": row.get::<String, _>("status"),
        "requested_at": row.get::<Option<chrono::DateTime<chrono::Utc>>, _>("requested_at"),
        "latest_event": row.get::<Option<String>, _>("event_kind"),
        "event_at": row.get::<Option<chrono::DateTime<chrono::Utc>>, _>("event_at"),
    })))
}

// ── readiness.compute ─────────────────────────────────────────────────────────

async fn readiness_compute(pool: &PgPool, args: &Value) -> Result<VerbExecutionOutcome> {
    let cbu_id = arg_uuid(args, "cbu-id")?;
    let registry = load_srdefs_from_config().unwrap_or_default();
    let engine = ReadinessEngine::new(pool, &registry);
    let result = engine.compute_for_cbu(cbu_id).await?;
    Ok(VerbExecutionOutcome::Record(json!({
        "total_services": result.total_services,
        "ready": result.ready,
        "partial": result.partial,
        "blocked": result.blocked,
    })))
}

// ── readiness.explain ─────────────────────────────────────────────────────────

async fn readiness_explain(pool: &PgPool, args: &Value) -> Result<VerbExecutionOutcome> {
    let cbu_id = arg_uuid(args, "cbu-id")?;
    let service_filter = arg_uuid_opt(args, "service-id");
    let service = ServiceResourcePipelineService::new(pool.clone());
    let readiness = service.get_service_readiness(cbu_id).await?;
    let blocking: Vec<_> = readiness
        .into_iter()
        .filter(|r| service_filter.is_none_or(|sid| r.service_id == sid))
        .filter(|r| r.status != "ready")
        .map(|r| {
            json!({
                "service_id": r.service_id,
                "product_id": r.product_id,
                "status": r.status,
                "blocking_reasons": r.blocking_reasons,
            })
        })
        .collect();
    Ok(VerbExecutionOutcome::RecordSet(blocking))
}

// ── pipeline.full ─────────────────────────────────────────────────────────────

async fn pipeline_full(pool: &PgPool, args: &Value) -> Result<VerbExecutionOutcome> {
    let cbu_id = arg_uuid(args, "cbu-id")?;
    let registry = load_srdefs_from_config().unwrap_or_default();
    let discovery = run_discovery_pipeline(pool, &registry, cbu_id).await?;
    let provisioning = run_provisioning_pipeline(pool, &registry, cbu_id).await?;
    Ok(VerbExecutionOutcome::Record(json!({
        "cbu_id": cbu_id,
        "discovery": {
            "srdefs_discovered": discovery.srdefs_discovered,
            "attrs_rolled_up": discovery.attrs_rolled_up,
            "attrs_populated": discovery.attrs_populated,
            "attrs_missing": discovery.attrs_missing,
        },
        "provisioning": {
            "requests_created": provisioning.requests_created,
            "already_active": provisioning.already_active,
            "not_ready": provisioning.not_ready,
        },
        "readiness": {
            "services_ready": provisioning.services_ready,
            "services_partial": provisioning.services_partial,
            "services_blocked": provisioning.services_blocked,
        },
    })))
}

// ── service-resource.check-attribute-gaps ─────────────────────────────────────

async fn service_resource_check_attribute_gaps(pool: &PgPool) -> Result<VerbExecutionOutcome> {
    let registry = load_srdefs_from_config().unwrap_or_default();
    let mut gap_rows: Vec<Value> = Vec::new();
    for (srdef_id, srdef) in &registry.srdefs {
        for attr in &srdef.attributes {
            let in_registry: bool = sqlx::query_scalar(
                r#"SELECT EXISTS(SELECT 1 FROM "ob-poc".attribute_registry WHERE id = $1)"#,
            )
            .bind(&attr.attr_id)
            .fetch_one(pool)
            .await
            .unwrap_or(false);
            let in_semos: bool = sqlx::query_scalar(
                r#"SELECT EXISTS(SELECT 1 FROM sem_reg.v_active_attribute_defs WHERE semantic_id = $1)"#,
            )
            .bind(&attr.attr_id)
            .fetch_one(pool)
            .await
            .unwrap_or(false);
            let status = match (in_registry, in_semos) {
                (true, true) => "ok",
                (true, false) => "ungoverned",
                (false, _) => "missing",
            };
            gap_rows.push(json!({
                "srdef_id": srdef_id,
                "attribute_fqn": attr.attr_id,
                "status": status,
            }));
        }
    }
    Ok(VerbExecutionOutcome::RecordSet(gap_rows))
}

// ── service-resource.sync-definitions ─────────────────────────────────────────

async fn service_resource_sync_definitions(pool: &PgPool) -> Result<VerbExecutionOutcome> {
    let (_registry, sync_result) = load_and_sync_srdefs(pool).await?;
    Ok(VerbExecutionOutcome::Record(json!({
        "inserted": sync_result.inserted,
        "updated": sync_result.updated,
        "errors": sync_result.errors,
    })))
}
