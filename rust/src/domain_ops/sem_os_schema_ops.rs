//! Schema domain CustomOps (Spec §C.2 — 5 verbs).
//!
//! Schema introspection and extraction verbs.
//! Allowed in BOTH Research and Governed AgentModes (read-only).

use anyhow::Result;
use async_trait::async_trait;
use serde_json::json;
use std::collections::BTreeSet;

use dsl_runtime_macros::register_custom_op;

use super::CustomOperation;
use crate::dsl_v2::verb_registry::registry;
use crate::ontology::{ontology, SearchKeyDef};

#[cfg(feature = "database")]
use sqlx::PgPool;

#[cfg(feature = "database")]
use {
    dsl_runtime::domain_ops::affinity_graph_cache::load_affinity_graph_cached,
    crate::sem_reg::{entity_type_def::EntityTypeDefBody, store::SnapshotStore, types::ObjectType},
    sem_os_core::affinity::{match_intent, AffinityGraph, DataRef},
    sem_os_core::diagram::{
        enrichment::build_diagram_model,
        mermaid,
        model::{ColumnInput, ForeignKeyInput, RenderOptions, TableInput},
    },
    sem_os_core::verb_contract::VerbContractBody,
};

// ── Typed result types ────────────────────────────────────────────

#[derive(Debug, serde::Serialize)]
struct GenerateErdResult {
    schema: String,
    format: String,
    table_count: usize,
    show_affinity_kind: bool,
    group_by: String,
    diagram: String,
}

#[derive(Debug, serde::Serialize)]
struct GenerateVerbFlowResult {
    mode: String,
    verb_fqn: String,
    domain: Option<String>,
    depth: u32,
    format: String,
    diagram: String,
}

#[derive(Debug, serde::Serialize)]
struct GenerateDiscoveryMapResult {
    domain: Option<String>,
    cluster_by: String,
    format: String,
    utterance_count: usize,
    intent_count: usize,
    diagram: String,
}

fn get_first_string_arg_json(args: &serde_json::Value, names: &[&str]) -> Option<String> {
    names
        .iter()
        .find_map(|name| super::helpers::json_extract_string_opt(args, name))
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

fn resolve_structure_entity_arg_json(args: &serde_json::Value) -> Option<String> {
    let raw = get_first_string_arg_json(
        args,
        &[
            "entity-type",
            "entity_type",
            "entity",
            "domain",
            "domain-name",
        ],
    )?;
    let normalized = normalize_structure_target(&raw);
    Some(ontology().resolve_alias(&normalized).to_string())
}

fn summarize_entity_relationships(entity_type: &str) -> Vec<serde_json::Value> {
    let ontology = ontology();
    let entity = ontology.get_entity(entity_type);
    let category = entity.map(|def| def.category.as_str());

    ontology
        .taxonomy()
        .relationships()
        .iter()
        .filter(|rel| {
            rel.parent == entity_type
                || rel.child == entity_type
                || category == Some(rel.parent.as_str())
                || category == Some(rel.child.as_str())
        })
        .map(|rel| {
            json!({
                "parent": rel.parent,
                "child": rel.child,
                "fk_arg": rel.fk_arg,
                "description": rel.description,
            })
        })
        .collect()
}

fn summarize_domain_verbs(domain: &str) -> Vec<serde_json::Value> {
    let mut verbs: Vec<_> = registry()
        .verbs_for_domain(domain)
        .into_iter()
        .map(|verb| {
            let required_args: Vec<&str> = verb.required_arg_names();
            let optional_args: Vec<&str> = verb.optional_arg_names();
            json!({
                "verb_fqn": verb.full_name(),
                "description": verb.description,
                "required_args": required_args,
                "optional_args": optional_args,
            })
        })
        .collect();
    verbs.sort_by(|a, b| a["verb_fqn"].as_str().cmp(&b["verb_fqn"].as_str()));
    verbs
}

fn summarize_entity_fields_from_ontology(entity_type: &str) -> Vec<serde_json::Value> {
    let ontology = ontology();
    let Some(entity) = ontology.get_entity(entity_type) else {
        return Vec::new();
    };

    let mut seen = BTreeSet::new();
    let mut fields = Vec::new();

    if seen.insert(entity.db.pk.clone()) {
        fields.push(json!({
            "name": entity.db.pk,
            "required": true,
            "source": "ontology.db.pk",
        }));
    }

    for key in &entity.search_keys {
        match key {
            SearchKeyDef::Single { column, .. } => {
                if seen.insert(column.clone()) {
                    fields.push(json!({
                        "name": column,
                        "required": false,
                        "source": "ontology.search_key",
                    }));
                }
            }
            SearchKeyDef::Composite { columns, .. } => {
                for column in columns {
                    if seen.insert(column.clone()) {
                        fields.push(json!({
                            "name": column,
                            "required": false,
                            "source": "ontology.search_key",
                        }));
                    }
                }
            }
        }
    }

    fields
}

#[cfg(feature = "database")]
async fn load_entity_type_body(
    pool: &PgPool,
    entity_type: &str,
) -> Result<Option<EntityTypeDefBody>> {
    let candidates = [entity_type.to_string(), format!("entity.{entity_type}")];
    for candidate in candidates {
        if let Some(row) = SnapshotStore::find_active_by_definition_field(
            pool,
            ObjectType::EntityTypeDef,
            "fqn",
            &candidate,
        )
        .await?
        {
            if let Ok(body) = serde_json::from_value::<EntityTypeDefBody>(row.definition.clone()) {
                return Ok(Some(body));
            }
        }
    }
    Ok(None)
}

#[cfg(feature = "database")]
async fn describe_entity_structure(entity_type: &str, pool: &PgPool) -> Result<serde_json::Value> {
    let ontology = ontology();
    let ontology_entity = ontology.get_entity(entity_type);
    let snapshot_body = load_entity_type_body(pool, entity_type).await?;

    let fields = if let Some(body) = &snapshot_body {
        body.required_attributes
            .iter()
            .map(|fqn| json!({"name": fqn, "required": true, "source": "sem_reg.required_attribute"}))
            .chain(body.optional_attributes.iter().map(|fqn| {
                json!({"name": fqn, "required": false, "source": "sem_reg.optional_attribute"})
            }))
            .collect::<Vec<_>>()
    } else {
        summarize_entity_fields_from_ontology(entity_type)
    };

    let relationships = summarize_entity_relationships(entity_type);
    let verbs = summarize_domain_verbs(entity_type);

    Ok(json!({
        "entity_type": entity_type,
        "description": snapshot_body
            .as_ref()
            .map(|body| body.description.clone())
            .or_else(|| ontology_entity.map(|entity| entity.description.clone())),
        "domain": snapshot_body
            .as_ref()
            .map(|body| body.domain.clone())
            .unwrap_or_else(|| entity_type.to_string()),
        "db_table": snapshot_body
            .as_ref()
            .and_then(|body| body.db_table.as_ref().map(|table| json!(table)))
            .or_else(|| ontology_entity.map(|entity| json!({
                "schema": entity.db.schema,
                "table": entity.db.table,
                "primary_key": entity.db.pk,
            }))),
        "field_count": fields.len(),
        "relationship_count": relationships.len(),
        "verb_count": verbs.len(),
        "fields": fields,
        "relationships": relationships,
        "verbs": verbs,
        "source": if snapshot_body.is_some() { "sem_reg+ontology+runtime_registry" } else { "ontology+runtime_registry" },
    }))
}

#[cfg(not(feature = "database"))]
fn describe_entity_structure(entity_type: &str) -> serde_json::Value {
    let ontology = ontology();
    let ontology_entity = ontology.get_entity(entity_type);
    let fields = summarize_entity_fields_from_ontology(entity_type);
    let relationships = summarize_entity_relationships(entity_type);
    let verbs = summarize_domain_verbs(entity_type);

    json!({
        "entity_type": entity_type,
        "description": ontology_entity.map(|entity| entity.description.clone()),
        "domain": entity_type,
        "db_table": ontology_entity.map(|entity| json!({
            "schema": entity.db.schema,
            "table": entity.db.table,
            "primary_key": entity.db.pk,
        })),
        "field_count": fields.len(),
        "relationship_count": relationships.len(),
        "verb_count": verbs.len(),
        "fields": fields,
        "relationships": relationships,
        "verbs": verbs,
        "source": "ontology+runtime_registry",
    })
}

// ── Structure Semantics Verbs ──────────────────────────────────

/// Describe the structure semantics of a domain.
#[register_custom_op]
pub struct SchemaDomainDescribeOp;

#[async_trait]
impl CustomOperation for SchemaDomainDescribeOp {
    fn domain(&self) -> &'static str {
        "schema"
    }
    fn verb(&self) -> &'static str {
        "domain.describe"
    }
    fn rationale(&self) -> &'static str {
        "Summarizes a data-management domain from ontology and runtime verb metadata"
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        _ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        let entity_type = resolve_structure_entity_arg_json(args).ok_or_else(|| {
            anyhow::anyhow!("schema.domain.describe requires :domain or :entity-type")
        })?;
        Ok(dsl_runtime::VerbExecutionOutcome::Record(
            describe_entity_structure(&entity_type, pool).await?,
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

/// Describe the structure semantics of an entity type.
#[register_custom_op]
pub struct SchemaEntityDescribeOp;

#[async_trait]
impl CustomOperation for SchemaEntityDescribeOp {
    fn domain(&self) -> &'static str {
        "schema"
    }
    fn verb(&self) -> &'static str {
        "entity.describe"
    }
    fn rationale(&self) -> &'static str {
        "Returns fields, relationships, and verbs for an entity type from registry and DSL metadata"
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        _ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        let entity_type = resolve_structure_entity_arg_json(args)
            .ok_or_else(|| anyhow::anyhow!("schema.entity.describe requires :entity-type"))?;
        Ok(dsl_runtime::VerbExecutionOutcome::Record(
            describe_entity_structure(&entity_type, pool).await?,
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

/// List fields for an entity type.
#[register_custom_op]
pub struct SchemaEntityListFieldsOp;

#[async_trait]
impl CustomOperation for SchemaEntityListFieldsOp {
    fn domain(&self) -> &'static str {
        "schema"
    }
    fn verb(&self) -> &'static str {
        "entity.list-fields"
    }
    fn rationale(&self) -> &'static str {
        "Lists structure fields from SemReg entity definitions or ontology fallbacks"
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        _ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        let entity_type = resolve_structure_entity_arg_json(args)
            .ok_or_else(|| anyhow::anyhow!("schema.entity.list-fields requires :entity-type"))?;
        let record = describe_entity_structure(&entity_type, pool).await?;
        Ok(dsl_runtime::VerbExecutionOutcome::Record(json!({
            "entity_type": entity_type,
            "fields": record["fields"].clone(),
            "field_count": record["field_count"].clone(),
            "source": record["source"].clone(),
        })))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

/// List relationships for an entity type.
#[register_custom_op]
pub struct SchemaEntityListRelationshipsOp;

#[async_trait]
impl CustomOperation for SchemaEntityListRelationshipsOp {
    fn domain(&self) -> &'static str {
        "schema"
    }
    fn verb(&self) -> &'static str {
        "entity.list-relationships"
    }
    fn rationale(&self) -> &'static str {
        "Lists ontology-backed relationships relevant to an entity type"
    }
    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        _ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        let entity_type = resolve_structure_entity_arg_json(args).ok_or_else(|| {
            anyhow::anyhow!("schema.entity.list-relationships requires :entity-type")
        })?;
        let record = describe_entity_structure(&entity_type, pool).await?;
        Ok(dsl_runtime::VerbExecutionOutcome::Record(json!({
            "entity_type": entity_type,
            "relationships": record["relationships"].clone(),
            "relationship_count": record["relationship_count"].clone(),
            "source": record["source"].clone(),
        })))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

/// List verbs for an entity type's domain.
#[register_custom_op]
pub struct SchemaEntityListVerbsOp;

#[async_trait]
impl CustomOperation for SchemaEntityListVerbsOp {
    fn domain(&self) -> &'static str {
        "schema"
    }
    fn verb(&self) -> &'static str {
        "entity.list-verbs"
    }
    fn rationale(&self) -> &'static str {
        "Lists runtime verb contracts for the entity type's DSL domain"
    }
    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        _ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        let entity_type = resolve_structure_entity_arg_json(args)
            .ok_or_else(|| anyhow::anyhow!("schema.entity.list-verbs requires :entity-type"))?;
        let record = describe_entity_structure(&entity_type, pool).await?;
        Ok(dsl_runtime::VerbExecutionOutcome::Record(json!({
            "entity_type": entity_type,
            "verbs": record["verbs"].clone(),
            "verb_count": record["verb_count"].clone(),
            "source": record["source"].clone(),
        })))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

// ── Schema Introspection ──────────────────────────────────────────

/// Introspect database schema via information_schema.
#[register_custom_op]
pub struct SchemaIntrospectOp;

#[async_trait]
impl CustomOperation for SchemaIntrospectOp {
    fn domain(&self) -> &'static str {
        "schema"
    }
    fn verb(&self) -> &'static str {
        "introspect"
    }
    fn rationale(&self) -> &'static str {
        "Delegates to db_introspect MCP tool"
    }
    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        super::sem_os_helpers::delegate_to_tool_json(pool, ctx, args, "db_introspect").await
    }
    fn is_migrated(&self) -> bool {
        true
    }
}

// ── Schema Extraction ─────────────────────────────────────────────

/// Extract attribute definitions from database schema.
#[register_custom_op]
pub struct SchemaExtractAttributesOp;

#[async_trait]
impl CustomOperation for SchemaExtractAttributesOp {
    fn domain(&self) -> &'static str {
        "schema"
    }
    fn verb(&self) -> &'static str {
        "extract-attributes"
    }
    fn rationale(&self) -> &'static str {
        "Scanner-based attribute extraction from schema columns"
    }
    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        super::sem_os_helpers::delegate_to_tool_json(pool, ctx, args, "db_introspect").await
    }
    fn is_migrated(&self) -> bool {
        true
    }
}

/// Extract verb contracts from YAML configuration.
#[register_custom_op]
pub struct SchemaExtractVerbsOp;

#[async_trait]
impl CustomOperation for SchemaExtractVerbsOp {
    fn domain(&self) -> &'static str {
        "schema"
    }
    fn verb(&self) -> &'static str {
        "extract-verbs"
    }
    fn rationale(&self) -> &'static str {
        "Scanner-based verb extraction from YAML config"
    }
    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        super::sem_os_helpers::delegate_to_tool_json(pool, ctx, args, "db_introspect").await
    }
    fn is_migrated(&self) -> bool {
        true
    }
}

/// Extract entity type definitions from schema.
#[register_custom_op]
pub struct SchemaExtractEntitiesOp;

#[async_trait]
impl CustomOperation for SchemaExtractEntitiesOp {
    fn domain(&self) -> &'static str {
        "schema"
    }
    fn verb(&self) -> &'static str {
        "extract-entities"
    }
    fn rationale(&self) -> &'static str {
        "Scanner-based entity type extraction from schema tables"
    }
    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        super::sem_os_helpers::delegate_to_tool_json(pool, ctx, args, "db_introspect").await
    }
    fn is_migrated(&self) -> bool {
        true
    }
}

/// Cross-reference schema against registry snapshots for drift detection.
#[register_custom_op]
pub struct SchemaCrossReferenceOp;

#[async_trait]
impl CustomOperation for SchemaCrossReferenceOp {
    fn domain(&self) -> &'static str {
        "schema"
    }
    fn verb(&self) -> &'static str {
        "cross-reference"
    }
    fn rationale(&self) -> &'static str {
        "Scanner drift detection: compare schema vs registry snapshots"
    }
    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        super::sem_os_helpers::delegate_to_tool_json(pool, ctx, args, "db_introspect").await
    }
    fn is_migrated(&self) -> bool {
        true
    }
}

// ── Diagram generation ops ────────────────────────────────────────────────────

#[cfg(feature = "database")]
fn get_string_list_arg_json(args: &serde_json::Value, name: &str) -> Option<Vec<String>> {
    let v = args.get(name)?;
    if let Some(s) = v.as_str() {
        return Some(vec![s.to_string()]);
    }
    v.as_array().map(|list| {
        list.iter()
            .filter_map(|node| node.as_str().map(ToOwned::to_owned))
            .collect()
    })
}

#[cfg(feature = "database")]
async fn load_active_verb_contracts(pool: &PgPool) -> anyhow::Result<Vec<VerbContractBody>> {
    let rows = sqlx::query_as::<_, (serde_json::Value,)>(
        "SELECT definition \
         FROM sem_reg.snapshots \
         WHERE status = 'active' AND object_type::text = 'verb_contract'",
    )
    .fetch_all(pool)
    .await?;

    let mut verbs = Vec::new();
    for (definition,) in rows {
        if let Ok(verb) = serde_json::from_value::<VerbContractBody>(definition) {
            verbs.push(verb);
        }
    }
    Ok(verbs)
}

#[cfg(feature = "database")]
fn render_domain_verb_flow(domain: &str, graph: &AffinityGraph, depth: u32) -> String {
    let mut out = String::new();
    out.push_str("graph LR\n");

    let mut verbs: Vec<String> = graph
        .verb_to_data
        .keys()
        .filter(|v| v.starts_with(&format!("{domain}.")))
        .cloned()
        .collect();
    verbs.sort();

    for verb in &verbs {
        let verb_id = mermaid::sanitize_id(verb);
        out.push_str(&format!("    {verb_id}[\"{verb}\"]\n"));
        for da in graph.data_for_verb(verb) {
            let data_key = da.data_ref.index_key();
            let data_id = mermaid::sanitize_id(&data_key);
            let label = match da.data_ref {
                DataRef::Table(ref t) => format!("{}:{}", t.schema, t.table),
                DataRef::Column(ref c) => format!("{}:{}.{}", c.schema, c.table, c.column),
                DataRef::EntityType { ref fqn } => format!("entity:{fqn}"),
                DataRef::Attribute { ref fqn } => format!("attr:{fqn}"),
            };
            out.push_str(&format!("    {data_id}[(\"{label}\")]\n"));
            out.push_str(&format!("    {verb_id} --> {data_id}\n"));
        }
    }

    if depth > 1 {
        for verb in &verbs {
            let verb_id = mermaid::sanitize_id(verb);
            for (adj, _) in graph.adjacent_verbs(verb) {
                if adj.starts_with(&format!("{domain}.")) {
                    let adj_id = mermaid::sanitize_id(&adj);
                    out.push_str(&format!("    {verb_id} -.-> {adj_id}\n"));
                }
            }
        }
    }

    out
}

/// Shared helper: query physical tables from information_schema for a given schema.
#[cfg(feature = "database")]
async fn extract_tables_from_schema(
    pool: &PgPool,
    schema_name: &str,
) -> anyhow::Result<Vec<TableInput>> {
    // Fetch tables
    let table_rows = sqlx::query_as::<_, (String, String)>(
        "SELECT table_schema, table_name \
         FROM information_schema.tables \
         WHERE table_type = 'BASE TABLE' AND table_schema = $1 \
         ORDER BY table_name",
    )
    .bind(schema_name)
    .fetch_all(pool)
    .await?;

    let mut tables = Vec::new();
    for (schema, tname) in table_rows {
        // Fetch columns
        let col_rows = sqlx::query_as::<_, (String, String, String)>(
            "SELECT column_name, data_type, is_nullable \
             FROM information_schema.columns \
             WHERE table_schema = $1 AND table_name = $2 \
             ORDER BY ordinal_position",
        )
        .bind(&schema)
        .bind(&tname)
        .fetch_all(pool)
        .await?;

        let columns: Vec<ColumnInput> = col_rows
            .into_iter()
            .map(|(col_name, data_type, is_nullable)| ColumnInput {
                name: col_name,
                sql_type: data_type,
                is_nullable: is_nullable.to_uppercase() == "YES",
            })
            .collect();

        // Fetch primary keys
        let pk_rows = sqlx::query_as::<_, (String,)>(
            "SELECT kcu.column_name \
             FROM information_schema.table_constraints tc \
             JOIN information_schema.key_column_usage kcu \
               ON tc.constraint_name = kcu.constraint_name \
               AND tc.table_schema = kcu.table_schema \
             WHERE tc.constraint_type = 'PRIMARY KEY' \
               AND tc.table_schema = $1 AND tc.table_name = $2 \
             ORDER BY kcu.ordinal_position",
        )
        .bind(&schema)
        .bind(&tname)
        .fetch_all(pool)
        .await?;
        let primary_keys: Vec<String> = pk_rows.into_iter().map(|(c,)| c).collect();

        // Fetch foreign keys
        let fk_rows = sqlx::query_as::<_, (String, String, String, String)>(
            "SELECT kcu.column_name, ccu.table_schema, ccu.table_name, ccu.column_name \
             FROM information_schema.table_constraints tc \
             JOIN information_schema.key_column_usage kcu \
               ON tc.constraint_name = kcu.constraint_name \
               AND tc.table_schema = kcu.table_schema \
             JOIN information_schema.constraint_column_usage ccu \
               ON ccu.constraint_name = tc.constraint_name \
             WHERE tc.constraint_type = 'FOREIGN KEY' \
               AND tc.table_schema = $1 AND tc.table_name = $2",
        )
        .bind(&schema)
        .bind(&tname)
        .fetch_all(pool)
        .await?;
        let foreign_keys: Vec<ForeignKeyInput> = fk_rows
            .into_iter()
            .map(
                |(from_col, tgt_schema, tgt_table, tgt_col)| ForeignKeyInput {
                    from_column: from_col,
                    target_schema: tgt_schema,
                    target_table: tgt_table,
                    target_column: tgt_col,
                },
            )
            .collect();

        tables.push(TableInput {
            schema,
            table_name: tname,
            columns,
            primary_keys,
            foreign_keys,
        });
    }
    Ok(tables)
}

/// Generate an entity-relationship diagram from physical schema + AffinityGraph.
#[register_custom_op]
pub struct SchemaGenerateErdOp;

#[async_trait]
impl CustomOperation for SchemaGenerateErdOp {
    fn domain(&self) -> &'static str {
        "schema"
    }
    fn verb(&self) -> &'static str {
        "generate-erd"
    }
    fn rationale(&self) -> &'static str {
        "ERD generation requires physical schema extraction + AffinityGraph build"
    }
    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        _ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        let schema_name = super::helpers::json_extract_string(args, "schema-name")
            .map_err(|_| anyhow::anyhow!("schema.generate-erd: schema-name is required"))?;
        let format = super::helpers::json_extract_string_opt(args, "format")
            .unwrap_or_else(|| "mermaid".to_string());
        if format != "mermaid" {
            return Err(anyhow::anyhow!(
                "schema.generate-erd: unsupported format '{}', only 'mermaid' is currently supported",
                format
            ));
        }
        let show_verb_surface =
            super::helpers::json_extract_bool_opt(args, "show-verb-surface").unwrap_or(true);
        let show_affinity_kind =
            super::helpers::json_extract_bool_opt(args, "show-affinity-kind").unwrap_or(false);
        let enrich = super::helpers::json_extract_bool_opt(args, "enrich").unwrap_or(true);
        let domain_filter = super::helpers::json_extract_string_opt(args, "domain");
        let group_by = super::helpers::json_extract_string_opt(args, "group-by")
            .unwrap_or_else(|| "schema".to_string());
        let max_tables = super::helpers::json_extract_int_opt(args, "max-tables").unwrap_or(50)
            as usize;
        let requested_tables = get_string_list_arg_json(args, "tables");

        let mut tables = extract_tables_from_schema(pool, &schema_name).await?;
        if let Some(ref wanted) = requested_tables {
            let wanted_set: std::collections::HashSet<&str> =
                wanted.iter().map(String::as_str).collect();
            tables.retain(|t| wanted_set.contains(t.table_name.as_str()));
        }
        let graph = if enrich {
            load_affinity_graph_cached(pool).await?
        } else {
            AffinityGraph::build(&[])
        };

        let options = RenderOptions {
            schema_filter: Some(vec![schema_name.clone()]),
            domain_filter,
            include_columns: true,
            show_governance: false,
            show_verb_surface,
            show_affinity_kind,
            max_tables: Some(max_tables),
            format: Some(format.clone()),
        };
        let model = build_diagram_model(&tables, &graph, &options);
        let mermaid_output = mermaid::render_erd(&model, &options);

        let result = GenerateErdResult {
            schema: schema_name,
            format,
            table_count: model.entities.len(),
            show_affinity_kind,
            group_by,
            diagram: mermaid_output,
        };
        Ok(dsl_runtime::VerbExecutionOutcome::Record(
            serde_json::to_value(result)?,
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

/// Generate a verb-flow diagram showing what data a verb touches.
#[register_custom_op]
pub struct SchemaGenerateVerbFlowOp;

#[async_trait]
impl CustomOperation for SchemaGenerateVerbFlowOp {
    fn domain(&self) -> &'static str {
        "schema"
    }
    fn verb(&self) -> &'static str {
        "generate-verb-flow"
    }
    fn rationale(&self) -> &'static str {
        "Verb-flow diagram requires AffinityGraph for verb→data edge traversal"
    }
    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        _ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        let format = super::helpers::json_extract_string_opt(args, "format")
            .unwrap_or_else(|| "mermaid".to_string());
        if format != "mermaid" {
            return Err(anyhow::anyhow!(
                "schema.generate-verb-flow: unsupported format '{}', only 'mermaid' is currently supported",
                format
            ));
        }
        let verb_fqn = super::helpers::json_extract_string_opt(args, "verb-fqn");
        let domain = super::helpers::json_extract_string_opt(args, "domain");
        if verb_fqn.is_none() && domain.is_none() {
            return Err(anyhow::anyhow!(
                "schema.generate-verb-flow: either verb-fqn or domain is required"
            ));
        }
        let depth = super::helpers::json_extract_int_opt(args, "depth").unwrap_or(2) as u32;

        let graph = load_affinity_graph_cached(pool).await?;
        let (mode, verb_value, diagram) = if let Some(ref vfqn) = verb_fqn {
            (
                "verb".to_string(),
                vfqn.clone(),
                mermaid::render_verb_flow(vfqn, &graph, depth),
            )
        } else {
            let domain_value = domain.clone().unwrap_or_default();
            (
                "domain".to_string(),
                domain_value.clone(),
                render_domain_verb_flow(&domain_value, &graph, depth),
            )
        };

        let result = GenerateVerbFlowResult {
            mode,
            verb_fqn: verb_value,
            domain,
            depth,
            format,
            diagram,
        };
        Ok(dsl_runtime::VerbExecutionOutcome::Record(
            serde_json::to_value(result)?,
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

/// Generate a domain discovery map.
#[register_custom_op]
pub struct SchemaGenerateDiscoveryMapOp;

#[async_trait]
impl CustomOperation for SchemaGenerateDiscoveryMapOp {
    fn domain(&self) -> &'static str {
        "schema"
    }
    fn verb(&self) -> &'static str {
        "generate-discovery-map"
    }
    fn rationale(&self) -> &'static str {
        "Generate utterance -> intent -> verb -> data map from AffinityGraph and verb phrases"
    }
    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        _ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        let domain = super::helpers::json_extract_string_opt(args, "domain");
        let format = super::helpers::json_extract_string_opt(args, "format")
            .unwrap_or_else(|| "mermaid".to_string());
        if format != "mermaid" {
            return Err(anyhow::anyhow!(
                "schema.generate-discovery-map: unsupported format '{}', only 'mermaid' is currently supported",
                format
            ));
        }
        let cluster_by = super::helpers::json_extract_string_opt(args, "cluster-by")
            .unwrap_or_else(|| "verb".to_string());
        let graph = load_affinity_graph_cached(pool).await?;
        let verbs = load_active_verb_contracts(pool).await?;

        let scoped_verbs: Vec<VerbContractBody> = if let Some(ref dom) = domain {
            verbs.into_iter().filter(|v| v.domain == *dom).collect()
        } else {
            verbs
        };

        let mut utterances = Vec::new();
        for verb in &scoped_verbs {
            if verb.invocation_phrases.is_empty() {
                utterances.push(format!("{} {}", verb.domain, verb.action));
            } else {
                utterances.extend(verb.invocation_phrases.iter().take(2).cloned());
            }
        }
        utterances.sort();
        utterances.dedup();
        utterances.truncate(24);

        let mut intent_matches = Vec::new();
        for utterance in &utterances {
            let mut matched = match_intent(utterance, &graph, &scoped_verbs);
            matched.truncate(4);
            intent_matches.extend(matched);
        }
        intent_matches.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.verb.cmp(&b.verb))
        });
        intent_matches.truncate(32);

        let discovery = sem_os_core::affinity::DiscoveryResponse {
            intent_matches: intent_matches.clone(),
            suggested_sequence: Vec::new(),
            disambiguation_needed: Vec::new(),
            governance_context: sem_os_core::affinity::GovernanceContext {
                all_tables_governed: true,
                required_mode: cluster_by.clone(),
                policy_check: None,
            },
        };
        let source_utterance = domain
            .clone()
            .map(|d| format!("domain:{d}"))
            .unwrap_or_else(|| "domain:all".to_owned());
        let diagram = mermaid::render_discovery_map(&source_utterance, &discovery, &graph);

        let result = GenerateDiscoveryMapResult {
            domain,
            cluster_by,
            format,
            utterance_count: utterances.len(),
            intent_count: intent_matches.len(),
            diagram,
        };
        Ok(dsl_runtime::VerbExecutionOutcome::Record(
            serde_json::to_value(result)?,
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}
