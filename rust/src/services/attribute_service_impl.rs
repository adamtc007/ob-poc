//! ob-poc impl of [`dsl_runtime::service_traits::AttributeService`].
//!
//! Single-method dispatch for the 16 attribute / document / derivation
//! verbs that operate on the SemOS-first attribute lifecycle. Bridge
//! stays in ob-poc because every snapshot write goes through
//! `crate::sem_reg::store::SnapshotStore` + `crate::sem_reg::types::*`
//! + `crate::sem_reg::derivation_spec::*` — multi-consumer surfaces
//! with no dsl-runtime analogue.
//!
//! `define`, `define-internal`, and `define-derived` produce post-
//! execution bindings (`@attribute → UUID`) returned in
//! [`AttributeDispatchOutcome::bindings`] for the consumer wrapper to
//! apply via `ctx.bind`.

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use sem_os_core::attribute_def::{AttributeDataType, AttributeDefBody, AttributeSource};
use sem_os_core::principal::Principal;
use sem_os_core::types::{AttributeVisibility, EvidenceGrade};
use serde_json::{json, Value};
use sqlx::{PgPool, Postgres, Row, Transaction};
use uuid::Uuid;

use dsl_runtime::execution::VerbExecutionOutcome;
use dsl_runtime::service_traits::{AttributeDispatchOutcome, AttributeService};

use crate::sem_reg::derivation_spec::{
    DerivationExpression, DerivationInput, DerivationSpecBody, FreshnessRule, NullSemantics,
};
use crate::sem_reg::store::SnapshotStore;
use crate::sem_reg::types::{
    ChangeType, GovernanceTier, ObjectType, SnapshotMeta, SnapshotRow, SnapshotStatus, TrustClass,
};
use crate::services::attribute_identity_service::AttributeIdentityService;

const EXT_AUDIT_USER: &str = "audit_user";

pub struct ObPocAttributeService;

impl ObPocAttributeService {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ObPocAttributeService {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AttributeService for ObPocAttributeService {
    async fn dispatch_attribute_verb(
        &self,
        pool: &PgPool,
        domain: &str,
        verb_name: &str,
        args: &Value,
        principal: &Principal,
    ) -> Result<AttributeDispatchOutcome> {
        match (domain, verb_name) {
            ("attribute", "list-sources") => attribute_list_sources(pool, args).await,
            ("attribute", "list-sinks") => attribute_list_sinks(pool, args).await,
            ("attribute", "trace-lineage") => attribute_trace_lineage(pool, args).await,
            ("attribute", "list-by-document") => attribute_list_by_document(pool, args).await,
            ("attribute", "check-coverage") => attribute_check_coverage(pool, args).await,
            ("document", "list-attributes") => attribute_list_by_document(pool, args).await,
            ("document", "check-extraction-coverage") => {
                document_check_extraction_coverage(pool, args).await
            }
            ("attribute", "define") => attribute_define(pool, args, principal).await,
            ("attribute", "define-internal") => {
                attribute_define_internal(pool, args, principal).await
            }
            ("attribute", "update-internal") => {
                attribute_update_internal(pool, args, principal).await
            }
            ("attribute", "define-derived") => {
                attribute_define_derived(pool, args, principal).await
            }
            ("attribute", "set-evidence-grade") => attribute_set_evidence_grade(pool, args).await,
            ("attribute", "deprecate") => attribute_deprecate(pool, args).await,
            ("attribute", "inspect") => attribute_inspect(pool, args).await,
            ("derivation", "recompute-stale") => derivation_recompute_stale(pool, args).await,
            ("attribute", "bridge-to-semos") => {
                attribute_bridge_to_semos(pool, args, principal).await
            }
            (d, v) => Err(anyhow!("unknown attribute verb: {d}.{v}")),
        }
    }
}

// ── arg helpers ───────────────────────────────────────────────────────────────

fn arg_string(args: &Value, name: &str) -> Result<String> {
    args.get(name)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow!("Missing {name} argument"))
}

fn arg_string_opt(args: &Value, name: &str) -> Option<String> {
    args.get(name).and_then(|v| v.as_str()).map(|s| s.to_string())
}

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

fn arg_int_opt(args: &Value, name: &str) -> Option<i64> {
    args.get(name).and_then(|v| match v {
        Value::Number(n) => n.as_i64(),
        Value::String(s) => s.parse().ok(),
        _ => None,
    })
}

fn parse_json_arg(args: &Value, name: &str) -> Result<Option<Value>> {
    match args.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => {
            let value = serde_json::from_str(s)
                .with_context(|| format!("Failed to parse JSON argument {}", name))?;
            Ok(Some(value))
        }
        Some(v) => Ok(Some(v.clone())),
    }
}

fn audit_user_for(principal: &Principal, fallback: &str) -> String {
    if !principal.actor_id.is_empty() && principal.actor_id != "system" {
        principal.actor_id.clone()
    } else {
        fallback.to_string()
    }
}

fn record(value: Value) -> AttributeDispatchOutcome {
    AttributeDispatchOutcome {
        outcome: VerbExecutionOutcome::Record(value),
        bindings: Vec::new(),
    }
}

fn uuid_with_binding(uuid: Uuid, bind_name: &str) -> AttributeDispatchOutcome {
    AttributeDispatchOutcome {
        outcome: VerbExecutionOutcome::Uuid(uuid),
        bindings: vec![(bind_name.to_string(), uuid)],
    }
}

// ── shared snapshot/registry helpers ──────────────────────────────────────────

#[derive(Debug, Clone)]
struct AttributeSnapshotContext {
    registry_uuid: Uuid,
    registry_id: String,
    fqn: String,
    active_snapshot: Option<SnapshotRow>,
}

fn normalize_attribute_id(raw_id: &str, domain: Option<&str>) -> String {
    if raw_id.contains('.') || domain.is_none() {
        raw_id.to_string()
    } else {
        format!("{}.{}", domain.unwrap_or_default(), raw_id)
    }
}

fn parse_evidence_grade(raw: Option<String>, default: EvidenceGrade) -> Result<EvidenceGrade> {
    match raw {
        None => Ok(default),
        Some(value) => value
            .parse::<EvidenceGrade>()
            .map_err(|_| anyhow!("Invalid evidence-grade '{}'", value)),
    }
}

fn parse_data_type(value_type: &str) -> Result<AttributeDataType> {
    AttributeDataType::from_pg_check_value(value_type)
        .ok_or_else(|| anyhow!("Unsupported attribute value-type '{}'", value_type))
}

fn parse_null_semantics(raw: Option<String>) -> Result<NullSemantics> {
    match raw.as_deref().unwrap_or("propagate") {
        "propagate" => Ok(NullSemantics::Propagate),
        "skip" => Ok(NullSemantics::Skip),
        "error" => Ok(NullSemantics::Error),
        other => Err(anyhow!("Unsupported null-semantics '{}'", other)),
    }
}

fn effective_description(display_name: &str, semos_description: Option<String>) -> String {
    semos_description.unwrap_or_else(|| display_name.to_string())
}

#[allow(clippy::too_many_arguments)]
fn build_attribute_def_body(
    semantic_id: &str,
    display_name: &str,
    description: String,
    domain: String,
    value_type: &str,
    evidence_grade: EvidenceGrade,
    derived: bool,
    category: Option<String>,
    validation_rules: Option<Value>,
    applicability: Option<Value>,
    derivation_spec_fqn: Option<String>,
    visibility: Option<AttributeVisibility>,
) -> Result<AttributeDefBody> {
    Ok(AttributeDefBody {
        fqn: semantic_id.to_string(),
        name: display_name.to_string(),
        description,
        domain,
        data_type: parse_data_type(value_type)?,
        evidence_grade,
        source: Some(AttributeSource {
            producing_verb: None,
            schema: Some("ob-poc".to_string()),
            table: Some("attribute_registry".to_string()),
            column: Some(semantic_id.to_string()),
            derived,
        }),
        constraints: None,
        sinks: Vec::new(),
        category,
        validation_rules,
        applicability,
        is_required: None,
        default_value: None,
        group_id: None,
        is_derived: Some(derived),
        derivation_spec_fqn,
        visibility,
    })
}

#[allow(clippy::too_many_arguments)]
fn build_derivation_spec_body(
    semantic_id: &str,
    display_name: &str,
    description: String,
    evidence_grade: EvidenceGrade,
    function_name: &str,
    inputs_json: Value,
    null_semantics: NullSemantics,
    freshness_seconds: Option<i64>,
) -> Result<DerivationSpecBody> {
    let inputs: Vec<DerivationInput> = serde_json::from_value(inputs_json)
        .context("derivation-inputs must be a JSON array of DerivationInput objects")?;
    let output_attribute_fqn = if semantic_id.ends_with("_value") {
        semantic_id.to_string()
    } else {
        format!("{semantic_id}_value")
    };
    Ok(DerivationSpecBody {
        fqn: semantic_id.to_string(),
        name: display_name.to_string(),
        description,
        output_attribute_fqn,
        inputs,
        expression: DerivationExpression::FunctionRef {
            ref_name: function_name.to_string(),
        },
        null_semantics,
        freshness_rule: freshness_seconds.map(|seconds| FreshnessRule {
            max_age_seconds: seconds.max(0) as u64,
        }),
        security_inheritance: Default::default(),
        evidence_grade,
        tests: Vec::new(),
    })
}

async fn patch_attribute_semos_metadata(
    tx: &mut Transaction<'_, Postgres>,
    semantic_id: &str,
    semos_patch: Value,
) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE "ob-poc".attribute_registry
        SET metadata = jsonb_set(
                COALESCE(metadata, '{}'::jsonb),
                '{sem_os}',
                COALESCE(metadata->'sem_os', '{}'::jsonb) || $2::jsonb,
                true
            ),
            updated_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(semantic_id)
    .bind(semos_patch)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn sync_attribute_registry_governance(
    tx: &mut Transaction<'_, Postgres>,
    semantic_id: &str,
    sem_reg_snapshot_id: Option<Uuid>,
    is_derived: bool,
    derivation_spec_fqn: Option<&str>,
    evidence_grade: EvidenceGrade,
) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE "ob-poc".attribute_registry
        SET sem_reg_snapshot_id = COALESCE($2, sem_reg_snapshot_id),
            is_derived = $3,
            derivation_spec_fqn = $4,
            evidence_grade = $5,
            updated_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(semantic_id)
    .bind(sem_reg_snapshot_id)
    .bind(is_derived)
    .bind(derivation_spec_fqn)
    .bind(evidence_grade.to_string())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn materialize_to_store(
    tx: &mut Transaction<'_, Postgres>,
    semantic_id: &str,
    uuid: Uuid,
    snapshot_id: Uuid,
    definition: &Value,
    is_derived: bool,
    derivation_spec_fqn: Option<&str>,
) -> Result<Uuid> {
    let name = definition
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or(semantic_id);
    let domain = definition.get("domain").and_then(|v| v.as_str());
    let data_type = definition
        .get("data_type")
        .and_then(|v| v.as_str())
        .unwrap_or("text");
    let evidence_grade = definition
        .get("evidence_grade")
        .and_then(|v| v.as_str())
        .unwrap_or("none");
    let category = definition.get("category").and_then(|v| v.as_str());
    let validation_rules = definition.get("validation_rules").cloned();
    let applicability = definition.get("applicability").cloned();
    let visibility = definition
        .get("visibility")
        .and_then(|v| v.as_str())
        .unwrap_or("external");

    let semos_metadata = json!({
        "snapshot_id": snapshot_id,
        "object_id": uuid,
        "attribute_fqn": semantic_id,
    });

    let row = sqlx::query(
        r#"
        INSERT INTO "ob-poc".attribute_registry (
            id, uuid, display_name, category, value_type, domain,
            validation_rules, applicability, evidence_grade,
            is_derived, derivation_spec_fqn, sem_reg_snapshot_id,
            metadata, visibility
        )
        VALUES (
            $1, $2, $3, $4, $5, $6,
            COALESCE($7, '{}'::jsonb),
            COALESCE($8, '{}'::jsonb),
            $9,
            $10,
            $11,
            $12,
            jsonb_build_object('sem_os', $13::jsonb),
            $14
        )
        ON CONFLICT (id) DO UPDATE SET
            display_name = EXCLUDED.display_name,
            category = EXCLUDED.category,
            value_type = EXCLUDED.value_type,
            domain = EXCLUDED.domain,
            validation_rules = COALESCE(EXCLUDED.validation_rules, "ob-poc".attribute_registry.validation_rules),
            applicability = COALESCE(EXCLUDED.applicability, "ob-poc".attribute_registry.applicability),
            evidence_grade = EXCLUDED.evidence_grade,
            is_derived = EXCLUDED.is_derived,
            derivation_spec_fqn = EXCLUDED.derivation_spec_fqn,
            sem_reg_snapshot_id = EXCLUDED.sem_reg_snapshot_id,
            metadata = jsonb_set(
                COALESCE("ob-poc".attribute_registry.metadata, '{}'::jsonb),
                '{sem_os}',
                COALESCE("ob-poc".attribute_registry.metadata->'sem_os', '{}'::jsonb) || $13::jsonb,
                true
            ),
            visibility = EXCLUDED.visibility,
            updated_at = NOW()
        RETURNING uuid
        "#,
    )
    .bind(semantic_id)
    .bind(uuid)
    .bind(name)
    .bind(category)
    .bind(data_type)
    .bind(domain)
    .bind(validation_rules)
    .bind(applicability)
    .bind(evidence_grade)
    .bind(is_derived)
    .bind(derivation_spec_fqn)
    .bind(snapshot_id)
    .bind(semos_metadata)
    .bind(visibility)
    .fetch_one(&mut **tx)
    .await?;

    Ok(row.get("uuid"))
}

async fn load_active_attribute_context(
    pool: &PgPool,
    reference: &str,
) -> Result<AttributeSnapshotContext> {
    let identity_service = AttributeIdentityService::new(pool.clone());
    let identity = identity_service
        .resolve_reference(reference)
        .await?
        .ok_or_else(|| anyhow!("Attribute '{}' not found", reference))?;
    let registry_uuid = identity
        .runtime_uuid()
        .ok_or_else(|| anyhow!("Attribute '{}' has no operational registry UUID", reference))?;
    let registry_id = identity
        .registry_id
        .clone()
        .or_else(|| identity.semos_attribute_fqn.clone())
        .or_else(|| identity.attribute_fqn.clone())
        .unwrap_or_else(|| reference.to_string());
    let fqn = identity
        .semos_attribute_fqn
        .clone()
        .or_else(|| identity.attribute_fqn.clone())
        .unwrap_or_else(|| registry_id.clone());
    let active_snapshot =
        SnapshotStore::resolve_active(pool, ObjectType::AttributeDef, registry_uuid)
            .await?
            .or(SnapshotStore::find_active_by_definition_field(
                pool,
                ObjectType::AttributeDef,
                "fqn",
                &fqn,
            )
            .await?);
    Ok(AttributeSnapshotContext {
        registry_uuid,
        registry_id,
        fqn,
        active_snapshot,
    })
}

async fn load_active_derivation_snapshot(
    pool: &PgPool,
    fqn: &str,
) -> Result<Option<SnapshotRow>> {
    SnapshotStore::find_active_by_definition_field(pool, ObjectType::DerivationSpec, "fqn", fqn)
        .await
}

fn next_meta_from_predecessor(
    predecessor: Option<&SnapshotRow>,
    object_type: ObjectType,
    object_id: Uuid,
    created_by: &str,
    change_type: ChangeType,
    change_rationale: Option<String>,
    status: SnapshotStatus,
) -> SnapshotMeta {
    let mut meta = SnapshotMeta::new_operational(object_type, object_id, created_by.to_string());
    meta.change_type = change_type;
    meta.change_rationale = change_rationale;
    meta.status = status;
    if let Some(pred) = predecessor {
        meta.version_major = pred.version_major;
        meta.version_minor = pred.version_minor + 1;
        meta.predecessor_id = Some(pred.snapshot_id);
    }
    meta
}

async fn publish_snapshot_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    meta: &SnapshotMeta,
    definition: &Value,
) -> Result<Uuid> {
    if let Some(predecessor_id) = meta.predecessor_id {
        let affected = sqlx::query(
            r#"
            UPDATE sem_reg.snapshots
            SET effective_until = NOW()
            WHERE snapshot_id = $1 AND effective_until IS NULL
            "#,
        )
        .bind(predecessor_id)
        .execute(&mut **tx)
        .await?
        .rows_affected();
        if affected == 0 {
            return Err(anyhow!(
                "Predecessor snapshot {} not found or already superseded",
                predecessor_id
            ));
        }
    }

    let security_label = serde_json::to_value(&meta.security_label)?;
    let snapshot_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO sem_reg.snapshots (
            snapshot_set_id, object_type, object_id,
            version_major, version_minor, status,
            governance_tier, trust_class, security_label,
            predecessor_id, change_type, change_rationale,
            created_by, approved_by, definition
        ) VALUES (
            NULL, $1::sem_reg.object_type, $2,
            $3, $4, $5::sem_reg.snapshot_status,
            $6::sem_reg.governance_tier, $7::sem_reg.trust_class, $8,
            $9, $10::sem_reg.change_type, $11,
            $12, $13, $14
        )
        RETURNING snapshot_id
        "#,
    )
    .bind(meta.object_type.as_ref())
    .bind(meta.object_id)
    .bind(meta.version_major)
    .bind(meta.version_minor)
    .bind(meta.status.as_ref())
    .bind(meta.governance_tier.as_ref())
    .bind(meta.trust_class.as_ref())
    .bind(security_label)
    .bind(meta.predecessor_id)
    .bind(meta.change_type.as_ref())
    .bind(&meta.change_rationale)
    .bind(&meta.created_by)
    .bind(&meta.approved_by)
    .bind(definition)
    .fetch_one(&mut **tx)
    .await?;

    if matches!(meta.object_type, ObjectType::DerivationSpec) && meta.predecessor_id.is_some() {
        if let Some(spec_fqn) = definition.get("fqn").and_then(|value| value.as_str()) {
            let _: i64 = sqlx::query_scalar(
                r#"SELECT COALESCE("ob-poc".propagate_spec_staleness($1, $2), 0)"#,
            )
            .bind(spec_fqn)
            .bind(snapshot_id)
            .fetch_one(&mut **tx)
            .await?;
        }
    }

    Ok(snapshot_id)
}

fn hashes_match(snapshot: Option<&SnapshotRow>, definition: &Value) -> bool {
    snapshot
        .map(|existing| {
            crate::sem_reg::ids::definition_hash(&existing.definition)
                == crate::sem_reg::ids::definition_hash(definition)
        })
        .unwrap_or(false)
}

// ── attribute.list-sources ────────────────────────────────────────────────────

async fn attribute_list_sources(pool: &PgPool, args: &Value) -> Result<AttributeDispatchOutcome> {
    let attr_id = arg_string(args, "attribute")?;
    let rows = sqlx::query!(
        r#"
        SELECT dt.type_code, dt.display_name, dt.category,
               dal.extraction_method, dal.is_authoritative, dal.proof_strength,
               dal.extraction_confidence_default
        FROM "ob-poc".document_attribute_links dal
        JOIN "ob-poc".document_types dt ON dt.type_id = dal.document_type_id
        JOIN "ob-poc".attribute_registry ar ON ar.uuid = dal.attribute_id
        WHERE ar.id = $1 AND dal.direction IN ('SOURCE', 'BOTH')
        ORDER BY dal.is_authoritative DESC,
                 CASE dal.proof_strength
                     WHEN 'PRIMARY' THEN 1
                     WHEN 'SECONDARY' THEN 2
                     WHEN 'SUPPORTING' THEN 3
                     ELSE 4
                 END,
                 dt.type_code
        "#,
        attr_id
    )
    .fetch_all(pool)
    .await?;
    let sources: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "document_type": r.type_code,
                "display_name": r.display_name,
                "category": r.category,
                "extraction_method": r.extraction_method,
                "is_authoritative": r.is_authoritative,
                "proof_strength": r.proof_strength,
                "confidence_default": r.extraction_confidence_default
            })
        })
        .collect();
    Ok(record(json!({
        "attribute": attr_id,
        "source_count": sources.len(),
        "sources": sources
    })))
}

// ── attribute.list-sinks ──────────────────────────────────────────────────────

async fn attribute_list_sinks(pool: &PgPool, args: &Value) -> Result<AttributeDispatchOutcome> {
    let attr_id = arg_string(args, "attribute")?;
    let rows = sqlx::query!(
        r#"
        SELECT dt.type_code, dt.display_name, dt.category, dal.proof_strength
        FROM "ob-poc".document_attribute_links dal
        JOIN "ob-poc".document_types dt ON dt.type_id = dal.document_type_id
        JOIN "ob-poc".attribute_registry ar ON ar.uuid = dal.attribute_id
        WHERE ar.id = $1 AND dal.direction IN ('SINK', 'BOTH')
        ORDER BY dt.type_code
        "#,
        attr_id
    )
    .fetch_all(pool)
    .await?;
    let sinks: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "document_type": r.type_code,
                "display_name": r.display_name,
                "category": r.category,
                "proof_strength": r.proof_strength
            })
        })
        .collect();
    Ok(record(json!({
        "attribute": attr_id,
        "sink_count": sinks.len(),
        "sinks": sinks
    })))
}

// ── attribute.trace-lineage ───────────────────────────────────────────────────

async fn attribute_trace_lineage(pool: &PgPool, args: &Value) -> Result<AttributeDispatchOutcome> {
    let attr_id = arg_string(args, "attribute")?;
    let attr = sqlx::query(
        r#"
        SELECT id, display_name, category, value_type, domain
        FROM "ob-poc".attribute_registry
        WHERE id = $1
        "#,
    )
    .bind(&attr_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| anyhow!("Attribute not found: {attr_id}"))?;

    let sources = sqlx::query!(
        r#"
        SELECT dt.type_code, dt.display_name as doc_name, dt.category,
               dal.extraction_method, dal.is_authoritative, dal.proof_strength,
               dal.extraction_confidence_default
        FROM "ob-poc".document_attribute_links dal
        JOIN "ob-poc".document_types dt ON dt.type_id = dal.document_type_id
        JOIN "ob-poc".attribute_registry ar ON ar.uuid = dal.attribute_id
        WHERE ar.id = $1 AND dal.direction IN ('SOURCE', 'BOTH')
        ORDER BY dal.is_authoritative DESC, dal.proof_strength
        "#,
        attr_id
    )
    .fetch_all(pool)
    .await?;

    let sinks = sqlx::query!(
        r#"
        SELECT dt.type_code, dt.display_name as doc_name, dt.category, dal.proof_strength
        FROM "ob-poc".document_attribute_links dal
        JOIN "ob-poc".document_types dt ON dt.type_id = dal.document_type_id
        JOIN "ob-poc".attribute_registry ar ON ar.uuid = dal.attribute_id
        WHERE ar.id = $1 AND dal.direction IN ('SINK', 'BOTH')
        ORDER BY dt.type_code
        "#,
        attr_id
    )
    .fetch_all(pool)
    .await?;

    let resources = sqlx::query!(
        r#"
        SELECT srt.resource_code, srt.name as resource_name, rar.is_mandatory
        FROM "ob-poc".resource_attribute_requirements rar
        JOIN "ob-poc".service_resource_types srt ON srt.resource_id = rar.resource_id
        JOIN "ob-poc".attribute_registry ar ON ar.uuid = rar.attribute_id
        WHERE ar.id = $1
        ORDER BY rar.is_mandatory DESC, srt.resource_code
        "#,
        attr_id
    )
    .fetch_all(pool)
    .await?;

    let sources_json: Vec<Value> = sources
        .iter()
        .map(|s| {
            json!({
                "document_type": s.type_code,
                "display_name": s.doc_name,
                "category": s.category,
                "extraction_method": s.extraction_method,
                "is_authoritative": s.is_authoritative,
                "proof_strength": s.proof_strength,
                "confidence_default": s.extraction_confidence_default
            })
        })
        .collect();
    let sinks_json: Vec<Value> = sinks
        .iter()
        .map(|s| {
            json!({
                "document_type": s.type_code,
                "display_name": s.doc_name,
                "category": s.category,
                "proof_strength": s.proof_strength
            })
        })
        .collect();
    let resources_json: Vec<Value> = resources
        .iter()
        .map(|r| {
            json!({
                "resource_code": r.resource_code,
                "resource_name": r.resource_name,
                "is_mandatory": r.is_mandatory
            })
        })
        .collect();

    let has_authoritative_source = sources.iter().any(|s| s.is_authoritative.unwrap_or(false));
    let primary_sources = sources
        .iter()
        .filter(|s| s.proof_strength.as_deref() == Some("PRIMARY"))
        .count();

    Ok(record(json!({
        "attribute": {
            "id": attr.get::<String, _>("id"),
            "display_name": attr.get::<String, _>("display_name"),
            "category": attr.get::<String, _>("category"),
            "value_type": attr.get::<String, _>("value_type"),
            "domain": attr.get::<Option<String>, _>("domain")
        },
        "sources": {
            "count": sources_json.len(),
            "has_authoritative": has_authoritative_source,
            "primary_count": primary_sources,
            "documents": sources_json
        },
        "sinks": {
            "count": sinks_json.len(),
            "documents": sinks_json
        },
        "required_by_resources": {
            "count": resources_json.len(),
            "resources": resources_json
        },
        "coverage_status": if has_authoritative_source || primary_sources > 0 {
            "GOOD"
        } else if !sources_json.is_empty() {
            "PARTIAL"
        } else {
            "NO_SOURCE"
        }
    })))
}

// ── attribute.list-by-document (also: document.list-attributes) ───────────────

async fn attribute_list_by_document(
    pool: &PgPool,
    args: &Value,
) -> Result<AttributeDispatchOutcome> {
    let doc_type = arg_string(args, "document-type")?;
    let direction_filter = arg_string_opt(args, "direction");
    let rows = sqlx::query!(
        r#"
        SELECT ar.id as attr_id, ar.display_name, ar.category, ar.value_type,
               dal.direction, dal.extraction_method, dal.is_authoritative,
               dal.proof_strength, dal.extraction_confidence_default
        FROM "ob-poc".document_attribute_links dal
        JOIN "ob-poc".document_types dt ON dt.type_id = dal.document_type_id
        JOIN "ob-poc".attribute_registry ar ON ar.uuid = dal.attribute_id
        WHERE dt.type_code = $1 AND ($2::text IS NULL OR dal.direction = $2)
        ORDER BY dal.direction, ar.category, ar.id
        "#,
        doc_type,
        direction_filter
    )
    .fetch_all(pool)
    .await?;
    let attributes: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "attribute_id": r.attr_id,
                "display_name": r.display_name,
                "category": r.category,
                "value_type": r.value_type,
                "direction": r.direction,
                "extraction_method": r.extraction_method,
                "is_authoritative": r.is_authoritative,
                "proof_strength": r.proof_strength,
                "confidence_default": r.extraction_confidence_default
            })
        })
        .collect();
    let source_count = rows
        .iter()
        .filter(|r| r.direction == "SOURCE" || r.direction == "BOTH")
        .count();
    let sink_count = rows
        .iter()
        .filter(|r| r.direction == "SINK" || r.direction == "BOTH")
        .count();
    Ok(record(json!({
        "document_type": doc_type,
        "attribute_count": attributes.len(),
        "source_count": source_count,
        "sink_count": sink_count,
        "attributes": attributes
    })))
}

// ── attribute.check-coverage ──────────────────────────────────────────────────

async fn attribute_check_coverage(
    pool: &PgPool,
    args: &Value,
) -> Result<AttributeDispatchOutcome> {
    let doc_type = arg_string(args, "document-type")?;
    let doc = sqlx::query!(
        r#"
        SELECT type_id, type_code, display_name, required_attributes
        FROM "ob-poc".document_types
        WHERE type_code = $1
        "#,
        doc_type
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| anyhow!("Document type not found: {doc_type}"))?;

    let linked_attrs = sqlx::query!(
        r#"
        SELECT ar.id as attr_id
        FROM "ob-poc".document_attribute_links dal
        JOIN "ob-poc".attribute_registry ar ON ar.uuid = dal.attribute_id
        WHERE dal.document_type_id = $1 AND dal.direction IN ('SOURCE', 'BOTH')
        "#,
        doc.type_id
    )
    .fetch_all(pool)
    .await?;

    let linked_set: std::collections::HashSet<String> =
        linked_attrs.iter().map(|r| r.attr_id.clone()).collect();
    let required: Value = doc.required_attributes.unwrap_or_else(|| json!({}));
    let mut mandatory_missing = Vec::new();
    let mut mandatory_covered = Vec::new();
    if let Some(mandatory) = required.get("mandatory").and_then(|m| m.as_array()) {
        for attr in mandatory {
            if let Some(attr_id) = attr.as_str() {
                if linked_set.contains(attr_id) {
                    mandatory_covered.push(attr_id.to_string());
                } else {
                    mandatory_missing.push(attr_id.to_string());
                }
            }
        }
    }
    let total_mandatory = mandatory_covered.len() + mandatory_missing.len();
    let coverage_pct = if total_mandatory > 0 {
        (mandatory_covered.len() as f64 / total_mandatory as f64) * 100.0
    } else {
        100.0
    };
    Ok(record(json!({
        "document_type": doc_type,
        "display_name": doc.display_name,
        "linked_attributes": linked_set.len(),
        "mandatory_coverage": {
            "total": total_mandatory,
            "covered": mandatory_covered.len(),
            "missing": mandatory_missing.len(),
            "coverage_percentage": format!("{:.1}%", coverage_pct),
            "covered_list": mandatory_covered,
            "missing_list": mandatory_missing
        },
        "status": if mandatory_missing.is_empty() { "COMPLETE" } else { "INCOMPLETE" }
    })))
}

// ── document.check-extraction-coverage ────────────────────────────────────────

async fn document_check_extraction_coverage(
    pool: &PgPool,
    args: &Value,
) -> Result<AttributeDispatchOutcome> {
    let entity_id = arg_uuid(args, "entity-id")?;
    let cbu_id = arg_uuid_opt(args, "cbu-id");
    let documents = sqlx::query!(
        r#"
        SELECT dc.doc_id, dt.type_code, dt.display_name
        FROM "ob-poc".document_catalog dc
        JOIN "ob-poc".document_types dt ON dt.type_id = dc.document_type_id
        WHERE dc.entity_id = $1
        AND ($2::uuid IS NULL OR dc.cbu_id = $2)
        "#,
        entity_id,
        cbu_id
    )
    .fetch_all(pool)
    .await?;
    let doc_type_codes: Vec<String> = documents.iter().map(|d| d.type_code.clone()).collect();
    let sourceable_attrs = if !doc_type_codes.is_empty() {
        sqlx::query!(
            r#"
            SELECT DISTINCT ar.id as attr_id, ar.display_name, ar.category,
                   dal.is_authoritative, dal.proof_strength
            FROM "ob-poc".document_attribute_links dal
            JOIN "ob-poc".document_types dt ON dt.type_id = dal.document_type_id
            JOIN "ob-poc".attribute_registry ar ON ar.uuid = dal.attribute_id
            WHERE dt.type_code = ANY($1) AND dal.direction IN ('SOURCE', 'BOTH')
            ORDER BY ar.category, ar.id
            "#,
            &doc_type_codes
        )
        .fetch_all(pool)
        .await?
    } else {
        vec![]
    };
    let entity = sqlx::query!(
        r#"
        SELECT e.entity_id, et.type_code as entity_type
        FROM "ob-poc".entities e
        JOIN "ob-poc".entity_types et ON et.entity_type_id = e.entity_type_id
        WHERE e.entity_id = $1
          AND e.deleted_at IS NULL
        "#,
        entity_id
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| anyhow!("Entity not found: {entity_id}"))?;
    let available_docs: Vec<Value> = documents
        .iter()
        .map(|d| {
            json!({
                "doc_id": d.doc_id,
                "type_code": d.type_code,
                "display_name": d.display_name
            })
        })
        .collect();
    let sourceable: Vec<Value> = sourceable_attrs
        .iter()
        .map(|a| {
            json!({
                "attribute_id": a.attr_id,
                "display_name": a.display_name,
                "category": a.category,
                "is_authoritative": a.is_authoritative,
                "proof_strength": a.proof_strength
            })
        })
        .collect();
    let has_authoritative = sourceable_attrs
        .iter()
        .any(|a| a.is_authoritative.unwrap_or(false));
    Ok(record(json!({
        "entity_id": entity_id,
        "entity_type": entity.entity_type,
        "available_documents": {
            "count": available_docs.len(),
            "documents": available_docs
        },
        "sourceable_attributes": {
            "count": sourceable.len(),
            "has_authoritative": has_authoritative,
            "attributes": sourceable
        },
        "coverage_summary": {
            "document_count": documents.len(),
            "attribute_count": sourceable_attrs.len(),
            "has_identity_docs": doc_type_codes.iter().any(|c| c.contains("PASSPORT") || c.contains("NATIONAL_ID")),
            "has_address_proof": doc_type_codes.iter().any(|c| c.contains("UTILITY") || c.contains("BANK_STATEMENT"))
        }
    })))
}

// ── attribute.define ──────────────────────────────────────────────────────────

async fn attribute_define(
    pool: &PgPool,
    args: &Value,
    principal: &Principal,
) -> Result<AttributeDispatchOutcome> {
    let raw_id = arg_string(args, "id")?;
    let display_name = arg_string(args, "display-name")?;
    let category = arg_string(args, "category")?;
    let value_type = arg_string(args, "value-type")?;
    let domain = arg_string_opt(args, "domain");
    let semantic_id = normalize_attribute_id(&raw_id, domain.as_deref());
    let description = effective_description(
        &display_name,
        arg_string_opt(args, "semos-description"),
    );
    let evidence_grade =
        parse_evidence_grade(arg_string_opt(args, "evidence-grade"), EvidenceGrade::None)?;
    let validation_rules = parse_json_arg(args, "validation-rules")?;
    let applicability = parse_json_arg(args, "applicability")?;

    let audit_user = audit_user_for(principal, "attribute.define");
    let object_id = crate::sem_reg::ids::object_id_for(ObjectType::AttributeDef, &semantic_id);

    let body = build_attribute_def_body(
        &semantic_id,
        &display_name,
        description,
        domain.clone().unwrap_or_else(|| {
            semantic_id
                .split('.')
                .next()
                .unwrap_or("attribute")
                .to_string()
        }),
        &value_type,
        evidence_grade,
        false,
        Some(category),
        validation_rules,
        applicability,
        None,
        None,
    )?;
    let definition = serde_json::to_value(&body)?;

    let predecessor =
        SnapshotStore::resolve_active(pool, ObjectType::AttributeDef, object_id).await?;
    let mut tx = pool.begin().await?;
    let snapshot_id = if hashes_match(predecessor.as_ref(), &definition) {
        predecessor
            .as_ref()
            .map(|row| row.snapshot_id)
            .ok_or_else(|| {
                anyhow!(
                    "Failed to resolve existing AttributeDef snapshot for {}",
                    semantic_id
                )
            })?
    } else {
        let meta = next_meta_from_predecessor(
            predecessor.as_ref(),
            ObjectType::AttributeDef,
            object_id,
            &audit_user,
            if predecessor.is_some() {
                ChangeType::NonBreaking
            } else {
                ChangeType::Created
            },
            None,
            SnapshotStatus::Active,
        );
        publish_snapshot_in_tx(&mut tx, &meta, &definition).await?
    };
    let registry_uuid = materialize_to_store(
        &mut tx,
        &semantic_id,
        object_id,
        snapshot_id,
        &definition,
        false,
        None,
    )
    .await?;
    tx.commit().await?;
    Ok(uuid_with_binding(registry_uuid, "attribute"))
}

// ── attribute.define-internal ─────────────────────────────────────────────────

async fn attribute_define_internal(
    pool: &PgPool,
    args: &Value,
    principal: &Principal,
) -> Result<AttributeDispatchOutcome> {
    let raw_id = arg_string(args, "id")?;
    let display_name = arg_string(args, "display-name")?;
    let category = arg_string(args, "category")?;
    let value_type = arg_string(args, "value-type")?;
    let domain = arg_string_opt(args, "domain");
    let semantic_id = normalize_attribute_id(&raw_id, domain.as_deref());
    let description = effective_description(
        &display_name,
        arg_string_opt(args, "semos-description"),
    );
    let validation_rules = parse_json_arg(args, "validation-rules")?;
    let applicability = parse_json_arg(args, "applicability")?;

    let audit_user = audit_user_for(principal, "attribute.define-internal");
    let object_id = crate::sem_reg::ids::object_id_for(ObjectType::AttributeDef, &semantic_id);

    let body = build_attribute_def_body(
        &semantic_id,
        &display_name,
        description,
        domain.clone().unwrap_or_else(|| {
            semantic_id
                .split('.')
                .next()
                .unwrap_or("attribute")
                .to_string()
        }),
        &value_type,
        EvidenceGrade::Prohibited,
        false,
        Some(category),
        validation_rules,
        applicability,
        None,
        Some(AttributeVisibility::Internal),
    )?;
    let definition = serde_json::to_value(&body)?;
    let predecessor =
        SnapshotStore::resolve_active(pool, ObjectType::AttributeDef, object_id).await?;
    let mut tx = pool.begin().await?;
    let snapshot_id = if hashes_match(predecessor.as_ref(), &definition) {
        predecessor
            .as_ref()
            .map(|row| row.snapshot_id)
            .ok_or_else(|| {
                anyhow!(
                    "Failed to resolve existing AttributeDef snapshot for {}",
                    semantic_id
                )
            })?
    } else {
        let mut meta = next_meta_from_predecessor(
            predecessor.as_ref(),
            ObjectType::AttributeDef,
            object_id,
            &audit_user,
            if predecessor.is_some() {
                ChangeType::NonBreaking
            } else {
                ChangeType::Created
            },
            None,
            SnapshotStatus::Active,
        );
        meta.governance_tier = GovernanceTier::Operational;
        meta.trust_class = TrustClass::Convenience;
        meta.approved_by = Some("auto".to_string());
        publish_snapshot_in_tx(&mut tx, &meta, &definition).await?
    };
    let registry_uuid = materialize_to_store(
        &mut tx,
        &semantic_id,
        object_id,
        snapshot_id,
        &definition,
        false,
        None,
    )
    .await?;
    tx.commit().await?;
    Ok(uuid_with_binding(registry_uuid, "attribute"))
}

// ── attribute.update-internal ─────────────────────────────────────────────────

async fn attribute_update_internal(
    pool: &PgPool,
    args: &Value,
    principal: &Principal,
) -> Result<AttributeDispatchOutcome> {
    let id = arg_string(args, "id")?;
    let audit_user = audit_user_for(principal, "attribute.update-internal");
    let attr_ctx = load_active_attribute_context(pool, &id).await?;
    let active_snapshot = attr_ctx
        .active_snapshot
        .as_ref()
        .ok_or_else(|| anyhow!("No active SemOS snapshot for attribute '{}'", id))?;
    let mut body: AttributeDefBody = serde_json::from_value(active_snapshot.definition.clone())?;
    let vis = body.visibility.unwrap_or(AttributeVisibility::External);
    if vis != AttributeVisibility::Internal {
        return Err(anyhow!(
            "Cannot update external attributes via update-internal — use the changeset path"
        ));
    }
    if let Ok(Some(val)) = parse_json_arg(args, "validation-rules") {
        body.validation_rules = Some(val);
    }
    if let Ok(Some(val)) = parse_json_arg(args, "applicability") {
        body.applicability = Some(val);
    }
    if let Some(v) = arg_string_opt(args, "is-required") {
        body.is_required = Some(v.parse::<bool>().unwrap_or(false));
    }
    if let Some(v) = arg_string_opt(args, "default-value") {
        body.default_value = Some(v);
    }
    if let Some(v) = arg_string_opt(args, "group-id") {
        body.group_id = Some(v);
    }
    let definition = serde_json::to_value(&body)?;
    if hashes_match(Some(active_snapshot), &definition) {
        return Ok(record(json!({
            "status": "unchanged",
            "attribute_id": attr_ctx.registry_id,
            "snapshot_id": active_snapshot.snapshot_id,
        })));
    }
    let mut meta = next_meta_from_predecessor(
        Some(active_snapshot),
        ObjectType::AttributeDef,
        attr_ctx.registry_uuid,
        &audit_user,
        ChangeType::NonBreaking,
        Some("Internal attribute metadata update".to_string()),
        SnapshotStatus::Active,
    );
    meta.governance_tier = GovernanceTier::Operational;
    meta.trust_class = TrustClass::Convenience;
    meta.approved_by = Some("auto".to_string());
    let mut tx = pool.begin().await?;
    let snapshot_id = publish_snapshot_in_tx(&mut tx, &meta, &definition).await?;
    let registry_uuid = materialize_to_store(
        &mut tx,
        &attr_ctx.registry_id,
        attr_ctx.registry_uuid,
        snapshot_id,
        &definition,
        body.is_derived.unwrap_or(false),
        body.derivation_spec_fqn.as_deref(),
    )
    .await?;
    tx.commit().await?;
    Ok(record(json!({
        "status": "updated",
        "attribute_id": attr_ctx.registry_id,
        "snapshot_id": snapshot_id,
        "registry_uuid": registry_uuid,
    })))
}

// ── attribute.define-derived ──────────────────────────────────────────────────

async fn attribute_define_derived(
    pool: &PgPool,
    args: &Value,
    principal: &Principal,
) -> Result<AttributeDispatchOutcome> {
    let raw_id = arg_string(args, "id")?;
    let domain = Some(arg_string(args, "domain")?);
    let semantic_id = normalize_attribute_id(&raw_id, domain.as_deref());
    let display_name = arg_string(args, "display-name")?;
    let category = arg_string(args, "category")?;
    let value_type = arg_string(args, "value-type")?;
    let description = effective_description(
        &display_name,
        arg_string_opt(args, "semos-description"),
    );
    let evidence_grade = parse_evidence_grade(
        arg_string_opt(args, "evidence-grade"),
        EvidenceGrade::Prohibited,
    )?;
    let derivation_function = arg_string(args, "derivation-function")?;
    let derivation_inputs = parse_json_arg(args, "derivation-inputs")?
        .ok_or_else(|| anyhow!("Missing derivation-inputs argument"))?;
    let null_semantics = parse_null_semantics(arg_string_opt(args, "null-semantics"))?;
    let freshness_seconds = arg_int_opt(args, "freshness-seconds");

    let audit_user = audit_user_for(principal, "attribute.define-derived");
    let object_id = crate::sem_reg::ids::object_id_for(ObjectType::AttributeDef, &semantic_id);
    let derivation_object_id =
        crate::sem_reg::ids::object_id_for(ObjectType::DerivationSpec, &semantic_id);

    let attr_body = build_attribute_def_body(
        &semantic_id,
        &display_name,
        description.clone(),
        domain.clone().unwrap_or_else(|| "attribute".to_string()),
        &value_type,
        evidence_grade,
        true,
        Some(category),
        None,
        None,
        Some(semantic_id.clone()),
        None,
    )?;
    let derivation_body = build_derivation_spec_body(
        &semantic_id,
        &display_name,
        description,
        evidence_grade,
        &derivation_function,
        derivation_inputs,
        null_semantics,
        freshness_seconds,
    )?;
    let attr_definition = serde_json::to_value(&attr_body)?;
    let derivation_definition = serde_json::to_value(&derivation_body)?;

    let attr_predecessor =
        SnapshotStore::resolve_active(pool, ObjectType::AttributeDef, object_id).await?;
    let derivation_predecessor =
        SnapshotStore::resolve_active(pool, ObjectType::DerivationSpec, derivation_object_id)
            .await?;

    let mut tx = pool.begin().await?;

    let attr_snapshot_id = if hashes_match(attr_predecessor.as_ref(), &attr_definition)
        && hashes_match(derivation_predecessor.as_ref(), &derivation_definition)
    {
        attr_predecessor
            .as_ref()
            .map(|row| row.snapshot_id)
            .ok_or_else(|| anyhow!("Missing existing AttributeDef snapshot for {}", semantic_id))?
    } else {
        let attr_meta = next_meta_from_predecessor(
            attr_predecessor.as_ref(),
            ObjectType::AttributeDef,
            object_id,
            &audit_user,
            if attr_predecessor.is_some() {
                ChangeType::NonBreaking
            } else {
                ChangeType::Created
            },
            None,
            SnapshotStatus::Active,
        );
        publish_snapshot_in_tx(&mut tx, &attr_meta, &attr_definition).await?
    };

    let derivation_snapshot_id = if hashes_match(attr_predecessor.as_ref(), &attr_definition)
        && hashes_match(derivation_predecessor.as_ref(), &derivation_definition)
    {
        derivation_predecessor
            .as_ref()
            .map(|row| row.snapshot_id)
            .ok_or_else(|| {
                anyhow!("Missing existing DerivationSpec snapshot for {}", semantic_id)
            })?
    } else {
        let derivation_meta = next_meta_from_predecessor(
            derivation_predecessor.as_ref(),
            ObjectType::DerivationSpec,
            derivation_object_id,
            &audit_user,
            if derivation_predecessor.is_some() {
                ChangeType::NonBreaking
            } else {
                ChangeType::Created
            },
            None,
            SnapshotStatus::Active,
        );
        publish_snapshot_in_tx(&mut tx, &derivation_meta, &derivation_definition).await?
    };
    if let Some(previous) = derivation_predecessor.as_ref() {
        let _affected: i64 = sqlx::query_scalar(
            r#"SELECT COALESCE("ob-poc".propagate_spec_staleness($1, $2), 0)"#,
        )
        .bind(&semantic_id)
        .bind(derivation_snapshot_id)
        .fetch_one(&mut *tx)
        .await?;
        let _ = previous.snapshot_id;
    }
    let registry_uuid = materialize_to_store(
        &mut tx,
        &semantic_id,
        object_id,
        attr_snapshot_id,
        &attr_definition,
        true,
        Some(&semantic_id),
    )
    .await?;
    tx.commit().await?;
    Ok(uuid_with_binding(registry_uuid, "attribute"))
}

// ── attribute.set-evidence-grade ──────────────────────────────────────────────

async fn attribute_set_evidence_grade(
    pool: &PgPool,
    args: &Value,
) -> Result<AttributeDispatchOutcome> {
    let reference = arg_string(args, "id")?;
    let new_grade = parse_evidence_grade(
        Some(arg_string(args, "evidence-grade")?),
        EvidenceGrade::None,
    )?;
    let context = load_active_attribute_context(pool, &reference).await?;
    let active = context
        .active_snapshot
        .clone()
        .ok_or_else(|| anyhow!("No active AttributeDef snapshot found for {}", context.fqn))?;
    let mut body: AttributeDefBody = active.parse_definition()?;
    if body.evidence_grade == new_grade {
        return Ok(record(json!({
            "attribute": context.registry_id,
            "snapshot_id": active.snapshot_id,
            "evidence_grade": new_grade.to_string(),
            "updated": false,
        })));
    }

    let mut tx = pool.begin().await?;
    body.evidence_grade = new_grade;
    let definition = serde_json::to_value(&body)?;
    let meta = next_meta_from_predecessor(
        Some(&active),
        ObjectType::AttributeDef,
        context.registry_uuid,
        "attribute.set-evidence-grade",
        ChangeType::NonBreaking,
        Some(format!("evidence_grade -> {}", new_grade)),
        SnapshotStatus::Active,
    );
    let attr_snapshot_id = publish_snapshot_in_tx(&mut tx, &meta, &definition).await?;

    if body.source.as_ref().is_some_and(|source| source.derived) {
        if let Some(derivation_snapshot) =
            load_active_derivation_snapshot(pool, &context.fqn).await?
        {
            let mut derivation_body: DerivationSpecBody = derivation_snapshot.parse_definition()?;
            derivation_body.evidence_grade = new_grade;
            let derivation_definition = serde_json::to_value(&derivation_body)?;
            let derivation_meta = next_meta_from_predecessor(
                Some(&derivation_snapshot),
                ObjectType::DerivationSpec,
                derivation_snapshot.object_id,
                "attribute.set-evidence-grade",
                ChangeType::NonBreaking,
                Some(format!("evidence_grade -> {}", new_grade)),
                SnapshotStatus::Active,
            );
            let derivation_snapshot_id =
                publish_snapshot_in_tx(&mut tx, &derivation_meta, &derivation_definition).await?;
            patch_attribute_semos_metadata(
                &mut tx,
                &context.registry_id,
                json!({ "derivation_snapshot_id": derivation_snapshot_id }),
            )
            .await?;
        }
    }

    patch_attribute_semos_metadata(
        &mut tx,
        &context.registry_id,
        json!({
            "snapshot_id": attr_snapshot_id,
            "evidence_grade": new_grade.to_string(),
        }),
    )
    .await?;
    sync_attribute_registry_governance(
        &mut tx,
        &context.registry_id,
        Some(attr_snapshot_id),
        body.source.as_ref().is_some_and(|source| source.derived),
        if body.source.as_ref().is_some_and(|source| source.derived) {
            Some(context.fqn.as_str())
        } else {
            None
        },
        new_grade,
    )
    .await?;
    tx.commit().await?;

    Ok(record(json!({
        "attribute": context.registry_id,
        "snapshot_id": attr_snapshot_id,
        "evidence_grade": new_grade.to_string(),
        "updated": true,
    })))
}

// ── attribute.deprecate ───────────────────────────────────────────────────────

async fn attribute_deprecate(pool: &PgPool, args: &Value) -> Result<AttributeDispatchOutcome> {
    let reference = arg_string(args, "id")?;
    let reason = arg_string(args, "reason")?;
    let replacement = arg_string_opt(args, "replacement");
    let context = load_active_attribute_context(pool, &reference).await?;
    let active = context
        .active_snapshot
        .clone()
        .ok_or_else(|| anyhow!("No active AttributeDef snapshot found for {}", context.fqn))?;
    let body: AttributeDefBody = active.parse_definition()?;

    let mut tx = pool.begin().await?;
    let meta = next_meta_from_predecessor(
        Some(&active),
        ObjectType::AttributeDef,
        context.registry_uuid,
        "attribute.deprecate",
        ChangeType::Deprecation,
        Some(reason.clone()),
        SnapshotStatus::Deprecated,
    );
    let attr_snapshot_id =
        publish_snapshot_in_tx(&mut tx, &meta, &serde_json::to_value(&body)?).await?;

    if body.source.as_ref().is_some_and(|source| source.derived) {
        if let Some(derivation_snapshot) =
            load_active_derivation_snapshot(pool, &context.fqn).await?
        {
            let derivation_body: DerivationSpecBody = derivation_snapshot.parse_definition()?;
            let derivation_meta = next_meta_from_predecessor(
                Some(&derivation_snapshot),
                ObjectType::DerivationSpec,
                derivation_snapshot.object_id,
                "attribute.deprecate",
                ChangeType::Deprecation,
                Some(reason.clone()),
                SnapshotStatus::Deprecated,
            );
            let derivation_snapshot_id = publish_snapshot_in_tx(
                &mut tx,
                &derivation_meta,
                &serde_json::to_value(&derivation_body)?,
            )
            .await?;
            patch_attribute_semos_metadata(
                &mut tx,
                &context.registry_id,
                json!({ "derivation_snapshot_id": derivation_snapshot_id }),
            )
            .await?;
        }
    }

    patch_attribute_semos_metadata(
        &mut tx,
        &context.registry_id,
        json!({
            "snapshot_id": attr_snapshot_id,
            "deprecated": true,
            "deprecation_reason": reason,
            "replacement": replacement,
        }),
    )
    .await?;
    sync_attribute_registry_governance(
        &mut tx,
        &context.registry_id,
        Some(attr_snapshot_id),
        body.source.as_ref().is_some_and(|source| source.derived),
        if body.source.as_ref().is_some_and(|source| source.derived) {
            Some(context.fqn.as_str())
        } else {
            None
        },
        body.evidence_grade,
    )
    .await?;
    tx.commit().await?;

    Ok(record(json!({
        "attribute": context.registry_id,
        "snapshot_id": attr_snapshot_id,
        "deprecated": true,
    })))
}

// ── attribute.inspect ─────────────────────────────────────────────────────────

async fn attribute_inspect(pool: &PgPool, args: &Value) -> Result<AttributeDispatchOutcome> {
    let reference = arg_string(args, "id")?;
    let context = load_active_attribute_context(pool, &reference).await?;
    let row = sqlx::query(
        r#"
        SELECT *
        FROM "ob-poc".v_attribute_registry_reconciled
        WHERE uuid = $1
        "#,
    )
    .bind(context.registry_uuid)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| {
        anyhow!(
            "Attribute '{}' is missing from v_attribute_registry_reconciled",
            context.registry_id
        )
    })?;

    let derivation_definition: Option<Value> = row.get("derivation_definition");
    let derivation = derivation_definition
        .map(|definition| {
            let function = definition
                .get("expression")
                .and_then(|expr| expr.get("ref_name"))
                .and_then(Value::as_str)
                .map(str::to_string);
            json!({
                "function": function,
                "inputs": definition.get("inputs").cloned().unwrap_or(json!([])),
                "null_semantics": definition.get("null_semantics").cloned().unwrap_or(Value::Null),
                "freshness_rule": definition.get("freshness_rule").cloned().unwrap_or(Value::Null),
            })
        })
        .unwrap_or_else(|| json!(null));

    Ok(record(json!({
        "identity": {
            "registry_id": row.get::<String, _>("registry_id"),
            "fqn": row.get::<String, _>("fqn"),
            "uuid": row.get::<Uuid, _>("uuid"),
            "display_name": row.get::<String, _>("display_name"),
        },
        "governance": {
            "snapshot_id": row.get::<Option<Uuid>, _>("attribute_snapshot_id"),
            "version": row.get::<Option<String>, _>("attribute_snapshot_version"),
            "status": row.get::<Option<String>, _>("attribute_snapshot_status"),
            "evidence_grade": row.get::<String, _>("evidence_grade"),
            "governance_tier": row.get::<Option<String>, _>("governance_tier"),
        },
        "definition": {
            "data_type": row.get::<String, _>("value_type"),
            "domain": row.get::<Option<String>, _>("domain"),
            "source": row.get::<Option<Value>, _>("attribute_source"),
            "constraints": row.get::<Option<Value>, _>("attribute_constraints"),
        },
        "derivation": derivation,
        "operational": {
            "active_observations": row.get::<i64, _>("active_observations"),
            "cbu_values": row.get::<i64, _>("cbu_values"),
            "document_sources": row.get::<i64, _>("document_sources"),
        }
    })))
}

// ── derivation.recompute-stale ────────────────────────────────────────────────

async fn derivation_recompute_stale(
    pool: &PgPool,
    args: &Value,
) -> Result<AttributeDispatchOutcome> {
    let limit = arg_int_opt(args, "limit").unwrap_or(100);
    let engine = crate::service_resources::PopulationEngine::new(pool);
    let result = engine.recompute_stale_batch(limit).await?;
    Ok(record(json!({
        "picked": result.picked,
        "recomputed": result.recomputed,
        "skipped_already_current": result.skipped_already_current,
        "still_stale": result.still_stale,
        "failed": result.failed
    })))
}

// ── attribute.bridge-to-semos ─────────────────────────────────────────────────

async fn attribute_bridge_to_semos(
    pool: &PgPool,
    args: &Value,
    principal: &Principal,
) -> Result<AttributeDispatchOutcome> {
    let limit = arg_int_opt(args, "limit").unwrap_or(100);
    let audit_user = audit_user_for(principal, "attribute.bridge-to-semos");

    let rows: Vec<(
        String,
        Uuid,
        String,
        String,
        String,
        Option<String>,
        Option<Value>,
        Option<Value>,
        String,
    )> = sqlx::query_as(
        r#"
        SELECT id, uuid, display_name, category, value_type, domain,
               validation_rules, applicability, evidence_grade
        FROM "ob-poc".attribute_registry
        WHERE sem_reg_snapshot_id IS NULL
        ORDER BY id
        LIMIT $1
        "#,
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;

    let mut bridged = 0u64;
    let mut failed = 0u64;
    let total = rows.len();

    for (
        semantic_id,
        _uuid,
        display_name,
        category,
        value_type,
        domain,
        validation_rules,
        applicability,
        evidence_grade_str,
    ) in &rows
    {
        let evidence_grade = evidence_grade_str
            .parse::<EvidenceGrade>()
            .unwrap_or(EvidenceGrade::None);
        let domain_str = domain.clone().unwrap_or_else(|| {
            semantic_id
                .split('.')
                .next()
                .unwrap_or("attribute")
                .to_string()
        });

        let body = build_attribute_def_body(
            semantic_id,
            display_name,
            display_name.clone(),
            domain_str,
            value_type,
            evidence_grade,
            false,
            Some(category.clone()),
            validation_rules.clone(),
            applicability.clone(),
            None,
            None,
        )?;
        let definition = serde_json::to_value(&body)?;
        let object_id = crate::sem_reg::ids::object_id_for(ObjectType::AttributeDef, semantic_id);
        if SnapshotStore::resolve_active(pool, ObjectType::AttributeDef, object_id)
            .await?
            .is_some()
        {
            continue;
        }

        let mut tx = pool.begin().await?;
        let meta = next_meta_from_predecessor(
            None,
            ObjectType::AttributeDef,
            object_id,
            &audit_user,
            ChangeType::Created,
            None,
            SnapshotStatus::Active,
        );
        match publish_snapshot_in_tx(&mut tx, &meta, &definition).await {
            Ok(snapshot_id) => {
                sqlx::query(
                    r#"UPDATE "ob-poc".attribute_registry SET sem_reg_snapshot_id = $1, updated_at = NOW() WHERE id = $2"#,
                )
                .bind(snapshot_id)
                .bind(semantic_id)
                .execute(&mut *tx)
                .await?;
                tx.commit().await?;
                bridged += 1;
            }
            Err(e) => {
                tx.rollback().await?;
                tracing::warn!(semantic_id = %semantic_id, error = %e, "Failed to bridge attribute to SemOS");
                failed += 1;
            }
        }
    }

    // Silence unused-key lint when no audit_user is consumed (always consumed above)
    let _ = EXT_AUDIT_USER;

    Ok(record(json!({
        "total_candidates": total,
        "bridged": bridged,
        "already_governed": total as u64 - bridged - failed,
        "failed": failed
    })))
}
