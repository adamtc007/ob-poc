//! Schema structure-semantics verbs (5) — project ontology + sem_reg
//! snapshot + runtime verb registry through [`SchemaIntrospectionAccess`].
//!
//! Each verb is a thin projection of the shared `describe_entity` record.
//! Argument name aliases (`entity-type` / `entity_type` / `entity` /
//! `domain` / `domain-name`) and plural→singular normalisation are
//! preserved from the legacy handler.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::{json, Value};

use dsl_runtime::domain_ops::helpers::json_extract_string_opt;
use dsl_runtime::service_traits::SchemaIntrospectionAccess;
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use crate::ops::SemOsVerbOp;

fn resolve_entity_type(args: &Value, schema: &dyn SchemaIntrospectionAccess) -> Option<String> {
    let raw = [
        "entity-type",
        "entity_type",
        "entity",
        "domain",
        "domain-name",
    ]
    .iter()
    .find_map(|name| json_extract_string_opt(args, name))?;
    let normalized = normalize_structure_target(&raw);
    Some(schema.resolve_entity_alias(&normalized))
}

fn normalize_structure_target(raw: &str) -> String {
    let normalized = raw.trim().to_lowercase();
    match normalized.as_str() {
        "documents" => "document".to_string(),
        "products" => "product".to_string(),
        "deals" | "deal record" | "deal records" | "records" => "deal".to_string(),
        "cbus" | "client business units" | "client business unit" => "cbu".to_string(),
        other => other.trim_end_matches('s').to_string(),
    }
}

async fn describe_entity(
    schema: &dyn SchemaIntrospectionAccess,
    pool: &sqlx::PgPool,
    entity_type: &str,
) -> Result<Value> {
    let ontology_summary = schema.ontology_entity_summary(entity_type);
    let snapshot = schema.entity_type_snapshot(pool, entity_type).await?;

    let fields = if let Some(body) = &snapshot {
        let required = body
            .get("required_attributes")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let optional = body
            .get("optional_attributes")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        required
            .into_iter()
            .map(|fqn| json!({"name": fqn, "required": true, "source": "sem_reg.required_attribute"}))
            .chain(optional.into_iter().map(|fqn| {
                json!({"name": fqn, "required": false, "source": "sem_reg.optional_attribute"})
            }))
            .collect::<Vec<_>>()
    } else {
        schema.ontology_entity_fields(entity_type)
    };

    let relationships = schema.ontology_entity_relationships(entity_type);
    let verbs = schema.domain_verbs(entity_type);

    let description = snapshot
        .as_ref()
        .and_then(|b| b.get("description").cloned())
        .or_else(|| {
            ontology_summary
                .as_ref()
                .and_then(|s| s.get("description").cloned())
        })
        .unwrap_or(Value::Null);

    let domain = snapshot
        .as_ref()
        .and_then(|b| b.get("domain").cloned())
        .unwrap_or_else(|| Value::String(entity_type.to_string()));

    let db_table = snapshot
        .as_ref()
        .and_then(|b| b.get("db_table").cloned())
        .or_else(|| {
            ontology_summary
                .as_ref()
                .and_then(|s| s.get("db_table").cloned())
        })
        .unwrap_or(Value::Null);

    let source = if snapshot.is_some() {
        "sem_reg+ontology+runtime_registry"
    } else {
        "ontology+runtime_registry"
    };

    Ok(json!({
        "entity_type": entity_type,
        "description": description,
        "domain": domain,
        "db_table": db_table,
        "field_count": fields.len(),
        "relationship_count": relationships.len(),
        "verb_count": verbs.len(),
        "fields": fields,
        "relationships": relationships,
        "verbs": verbs,
        "source": source,
    }))
}

/// Resolve `SchemaIntrospectionAccess` and produce the full merged
/// record; each op below projects the relevant subset.
async fn load_record(
    args: &Value,
    ctx: &mut VerbExecutionContext,
    scope: &mut dyn TransactionScope,
    missing_msg: &str,
) -> Result<(String, Value)> {
    let schema = ctx.service::<dyn SchemaIntrospectionAccess>()?;
    let entity_type =
        resolve_entity_type(args, schema.as_ref()).ok_or_else(|| anyhow!("{missing_msg}"))?;
    let record = describe_entity(schema.as_ref(), scope.pool(), &entity_type).await?;
    Ok((entity_type, record))
}

// ── schema.domain.describe ────────────────────────────────────────────────────

pub struct SchemaDomainDescribe;

#[async_trait]
impl SemOsVerbOp for SchemaDomainDescribe {
    fn fqn(&self) -> &str {
        "schema.domain.describe"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let (_entity_type, record) = load_record(
            args,
            ctx,
            scope,
            "schema.domain.describe requires :domain or :entity-type",
        )
        .await?;
        Ok(VerbExecutionOutcome::Record(record))
    }
}

// ── schema.entity.describe ────────────────────────────────────────────────────

pub struct SchemaEntityDescribe;

#[async_trait]
impl SemOsVerbOp for SchemaEntityDescribe {
    fn fqn(&self) -> &str {
        "schema.entity.describe"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let (_entity_type, record) = load_record(
            args,
            ctx,
            scope,
            "schema.entity.describe requires :entity-type",
        )
        .await?;
        Ok(VerbExecutionOutcome::Record(record))
    }
}

// ── schema.entity.list-fields ─────────────────────────────────────────────────

pub struct SchemaEntityListFields;

#[async_trait]
impl SemOsVerbOp for SchemaEntityListFields {
    fn fqn(&self) -> &str {
        "schema.entity.list-fields"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let (entity_type, record) = load_record(
            args,
            ctx,
            scope,
            "schema.entity.list-fields requires :entity-type",
        )
        .await?;
        Ok(VerbExecutionOutcome::Record(json!({
            "entity_type": entity_type,
            "fields": record["fields"].clone(),
            "field_count": record["field_count"].clone(),
            "source": record["source"].clone(),
        })))
    }
}

// ── schema.entity.list-relationships ──────────────────────────────────────────

pub struct SchemaEntityListRelationships;

#[async_trait]
impl SemOsVerbOp for SchemaEntityListRelationships {
    fn fqn(&self) -> &str {
        "schema.entity.list-relationships"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let (entity_type, record) = load_record(
            args,
            ctx,
            scope,
            "schema.entity.list-relationships requires :entity-type",
        )
        .await?;
        Ok(VerbExecutionOutcome::Record(json!({
            "entity_type": entity_type,
            "relationships": record["relationships"].clone(),
            "relationship_count": record["relationship_count"].clone(),
            "source": record["source"].clone(),
        })))
    }
}

// ── schema.entity.list-verbs ──────────────────────────────────────────────────

pub struct SchemaEntityListVerbs;

#[async_trait]
impl SemOsVerbOp for SchemaEntityListVerbs {
    fn fqn(&self) -> &str {
        "schema.entity.list-verbs"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let (entity_type, record) = load_record(
            args,
            ctx,
            scope,
            "schema.entity.list-verbs requires :entity-type",
        )
        .await?;
        Ok(VerbExecutionOutcome::Record(json!({
            "entity_type": entity_type,
            "verbs": record["verbs"].clone(),
            "verb_count": record["verb_count"].clone(),
            "source": record["source"].clone(),
        })))
    }
}
