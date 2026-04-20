//! Schema domain CustomOps — 13 `schema.*` verbs.
//!
//! Implements the YAML contracts in
//! `rust/config/verbs/sem-reg/schema.yaml`:
//!
//! Structure semantics (5 verbs, route via SchemaIntrospectionAccess trait):
//!  - schema.domain.describe          → SchemaDomainDescribeOp
//!  - schema.entity.describe          → SchemaEntityDescribeOp
//!  - schema.entity.list-fields       → SchemaEntityListFieldsOp
//!  - schema.entity.list-relationships → SchemaEntityListRelationshipsOp
//!  - schema.entity.list-verbs        → SchemaEntityListVerbsOp
//!
//! Schema introspection / extraction (5 verbs, route via StewardshipDispatch
//! cascade — slice #7 trick — to the `db_introspect` MCP tool):
//!  - schema.introspect               → SchemaIntrospectOp
//!  - schema.extract-attributes       → SchemaExtractAttributesOp
//!  - schema.extract-verbs            → SchemaExtractVerbsOp
//!  - schema.extract-entities         → SchemaExtractEntitiesOp
//!  - schema.cross-reference          → SchemaCrossReferenceOp
//!
//! Diagram generation (3 verbs, direct sqlx + sem_os_core::diagram +
//! relocated affinity_graph_cache):
//!  - schema.generate-erd             → SchemaGenerateErdOp
//!  - schema.generate-verb-flow       → SchemaGenerateVerbFlowOp
//!  - schema.generate-discovery-map   → SchemaGenerateDiscoveryMapOp
//!
//! Allowed in BOTH Research and Governed AgentModes (read-only).

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use dsl_runtime_macros::register_custom_op;
use serde_json::{json, Value};
use sqlx::PgPool;
use std::collections::HashSet;

use sem_os_core::affinity::{match_intent, AffinityGraph, DataRef};
use sem_os_core::diagram::{
    enrichment::build_diagram_model,
    mermaid,
    model::{ColumnInput, ForeignKeyInput, RenderOptions, TableInput},
};
use sem_os_core::verb_contract::VerbContractBody;

use crate::custom_op::CustomOperation;
use crate::domain_ops::affinity_graph_cache::load_affinity_graph_cached;
use crate::domain_ops::helpers::{
    json_extract_bool_opt, json_extract_int_opt, json_extract_string, json_extract_string_opt,
};
use crate::execution::{VerbExecutionContext, VerbExecutionOutcome};
use crate::service_traits::{SchemaIntrospectionAccess, StewardshipDispatch};

// ── Helpers shared across structure-semantics verbs ───────────────────────────

/// Resolve `entity-type` arg, accepting any of the legacy aliases
/// (`entity_type`, `entity`, `domain`, `domain-name`), normalising
/// plurals to singular, and applying ontology alias resolution.
fn resolve_entity_type(
    args: &Value,
    schema: &dyn SchemaIntrospectionAccess,
) -> Option<String> {
    let raw = ["entity-type", "entity_type", "entity", "domain", "domain-name"]
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

/// Build the unified entity description by merging ontology, sem_reg
/// snapshot, and runtime verb registry. Each consumer op then projects
/// the relevant subset (fields, relationships, verbs, or whole record).
async fn describe_entity(
    schema: &dyn SchemaIntrospectionAccess,
    pool: &PgPool,
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
        .or_else(|| ontology_summary.as_ref().and_then(|s| s.get("description").cloned()))
        .unwrap_or(Value::Null);

    let domain = snapshot
        .as_ref()
        .and_then(|b| b.get("domain").cloned())
        .unwrap_or_else(|| Value::String(entity_type.to_string()));

    let db_table = snapshot
        .as_ref()
        .and_then(|b| b.get("db_table").cloned())
        .or_else(|| ontology_summary.as_ref().and_then(|s| s.get("db_table").cloned()))
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

// ── schema.domain.describe ────────────────────────────────────────────────────

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

    async fn execute_json(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let schema = ctx.service::<dyn SchemaIntrospectionAccess>()?;
        let entity_type = resolve_entity_type(args, schema.as_ref())
            .ok_or_else(|| anyhow!("schema.domain.describe requires :domain or :entity-type"))?;
        let record = describe_entity(schema.as_ref(), pool, &entity_type).await?;
        Ok(VerbExecutionOutcome::Record(record))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

// ── schema.entity.describe ────────────────────────────────────────────────────

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

    async fn execute_json(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let schema = ctx.service::<dyn SchemaIntrospectionAccess>()?;
        let entity_type = resolve_entity_type(args, schema.as_ref())
            .ok_or_else(|| anyhow!("schema.entity.describe requires :entity-type"))?;
        let record = describe_entity(schema.as_ref(), pool, &entity_type).await?;
        Ok(VerbExecutionOutcome::Record(record))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

// ── schema.entity.list-fields ─────────────────────────────────────────────────

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

    async fn execute_json(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let schema = ctx.service::<dyn SchemaIntrospectionAccess>()?;
        let entity_type = resolve_entity_type(args, schema.as_ref())
            .ok_or_else(|| anyhow!("schema.entity.list-fields requires :entity-type"))?;
        let record = describe_entity(schema.as_ref(), pool, &entity_type).await?;
        Ok(VerbExecutionOutcome::Record(json!({
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

// ── schema.entity.list-relationships ──────────────────────────────────────────

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

    async fn execute_json(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let schema = ctx.service::<dyn SchemaIntrospectionAccess>()?;
        let entity_type = resolve_entity_type(args, schema.as_ref())
            .ok_or_else(|| anyhow!("schema.entity.list-relationships requires :entity-type"))?;
        let record = describe_entity(schema.as_ref(), pool, &entity_type).await?;
        Ok(VerbExecutionOutcome::Record(json!({
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

// ── schema.entity.list-verbs ──────────────────────────────────────────────────

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

    async fn execute_json(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let schema = ctx.service::<dyn SchemaIntrospectionAccess>()?;
        let entity_type = resolve_entity_type(args, schema.as_ref())
            .ok_or_else(|| anyhow!("schema.entity.list-verbs requires :entity-type"))?;
        let record = describe_entity(schema.as_ref(), pool, &entity_type).await?;
        Ok(VerbExecutionOutcome::Record(json!({
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

// ── Schema introspection / extraction ─────────────────────────────────────────
// These 5 verbs all delegate to the `db_introspect` MCP tool. The
// StewardshipDispatch cascade routes the call (slice #7 trick).

async fn dispatch_db_introspect(
    ctx: &VerbExecutionContext,
    args: &Value,
) -> Result<VerbExecutionOutcome> {
    let dispatcher = ctx.service::<dyn StewardshipDispatch>()?;
    let outcome = dispatcher
        .dispatch("db_introspect", args, &ctx.principal)
        .await?
        .ok_or_else(|| anyhow!("db_introspect tool not registered"))?;
    if outcome.success {
        Ok(VerbExecutionOutcome::Record(outcome.data))
    } else {
        Err(anyhow!(
            "{}",
            outcome
                .message
                .unwrap_or_else(|| "db_introspect failed".to_string())
        ))
    }
}

macro_rules! introspect_op {
    ($struct:ident, $verb:literal, $rationale:literal) => {
        #[register_custom_op]
        pub struct $struct;

        #[async_trait]
        impl CustomOperation for $struct {
            fn domain(&self) -> &'static str {
                "schema"
            }
            fn verb(&self) -> &'static str {
                $verb
            }
            fn rationale(&self) -> &'static str {
                $rationale
            }
            async fn execute_json(
                &self,
                args: &Value,
                ctx: &mut VerbExecutionContext,
                _pool: &PgPool,
            ) -> Result<VerbExecutionOutcome> {
                dispatch_db_introspect(ctx, args).await
            }
            fn is_migrated(&self) -> bool {
                true
            }
        }
    };
}

introspect_op!(
    SchemaIntrospectOp,
    "introspect",
    "Delegates to db_introspect MCP tool"
);
introspect_op!(
    SchemaExtractAttributesOp,
    "extract-attributes",
    "Scanner-based attribute extraction from schema columns"
);
introspect_op!(
    SchemaExtractVerbsOp,
    "extract-verbs",
    "Scanner-based verb extraction from YAML config"
);
introspect_op!(
    SchemaExtractEntitiesOp,
    "extract-entities",
    "Scanner-based entity type extraction from schema tables"
);
introspect_op!(
    SchemaCrossReferenceOp,
    "cross-reference",
    "Scanner drift detection: compare schema vs registry snapshots"
);

// ── Diagram generation helpers ────────────────────────────────────────────────

fn extract_string_list_arg(args: &Value, name: &str) -> Option<Vec<String>> {
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

async fn extract_tables_from_schema(
    pool: &PgPool,
    schema_name: &str,
) -> Result<Vec<TableInput>> {
    let table_rows = sqlx::query_as::<_, (String, String)>(
        "SELECT table_schema, table_name FROM information_schema.tables \
         WHERE table_type = 'BASE TABLE' AND table_schema = $1 \
         ORDER BY table_name",
    )
    .bind(schema_name)
    .fetch_all(pool)
    .await?;

    let mut tables = Vec::new();
    for (schema, tname) in table_rows {
        let col_rows = sqlx::query_as::<_, (String, String, String)>(
            "SELECT column_name, data_type, is_nullable FROM information_schema.columns \
             WHERE table_schema = $1 AND table_name = $2 \
             ORDER BY ordinal_position",
        )
        .bind(&schema)
        .bind(&tname)
        .fetch_all(pool)
        .await?;

        let columns: Vec<ColumnInput> = col_rows
            .into_iter()
            .map(|(name, sql_type, is_nullable)| ColumnInput {
                name,
                sql_type,
                is_nullable: is_nullable.eq_ignore_ascii_case("YES"),
            })
            .collect();

        let pk_rows = sqlx::query_as::<_, (String,)>(
            "SELECT kcu.column_name FROM information_schema.table_constraints tc \
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
            .map(|(from_column, target_schema, target_table, target_column)| ForeignKeyInput {
                from_column,
                target_schema,
                target_table,
                target_column,
            })
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

async fn load_active_verb_contracts(pool: &PgPool) -> Result<Vec<VerbContractBody>> {
    let rows = sqlx::query_as::<_, (Value,)>(
        "SELECT definition FROM sem_reg.snapshots \
         WHERE status = 'active' AND object_type::text = 'verb_contract'",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .filter_map(|(def,)| serde_json::from_value::<VerbContractBody>(def).ok())
        .collect())
}

fn render_domain_verb_flow(domain: &str, graph: &AffinityGraph, depth: u32) -> String {
    let mut out = String::from("graph LR\n");
    let mut verbs: Vec<&String> = graph
        .verb_to_data
        .keys()
        .filter(|v| v.starts_with(&format!("{domain}.")))
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

// ── schema.generate-erd ───────────────────────────────────────────────────────

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

    async fn execute_json(
        &self,
        args: &Value,
        _ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let schema_name = json_extract_string(args, "schema-name")?;
        let format =
            json_extract_string_opt(args, "format").unwrap_or_else(|| "mermaid".to_string());
        if format != "mermaid" {
            return Err(anyhow!(
                "schema.generate-erd: unsupported format '{format}', only 'mermaid' is supported"
            ));
        }
        let show_verb_surface = json_extract_bool_opt(args, "show-verb-surface").unwrap_or(true);
        let show_affinity_kind = json_extract_bool_opt(args, "show-affinity-kind").unwrap_or(false);
        let enrich = json_extract_bool_opt(args, "enrich").unwrap_or(true);
        let domain_filter = json_extract_string_opt(args, "domain");
        let group_by = json_extract_string_opt(args, "group-by")
            .unwrap_or_else(|| "schema".to_string());
        let max_tables = json_extract_int_opt(args, "max-tables").unwrap_or(50) as usize;
        let requested_tables = extract_string_list_arg(args, "tables");

        let mut tables = extract_tables_from_schema(pool, &schema_name).await?;
        if let Some(wanted) = requested_tables {
            let wanted_set: HashSet<&str> = wanted.iter().map(String::as_str).collect();
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
        let diagram = mermaid::render_erd(&model, &options);

        Ok(VerbExecutionOutcome::Record(json!({
            "schema": schema_name,
            "format": format,
            "table_count": model.entities.len(),
            "show_affinity_kind": show_affinity_kind,
            "group_by": group_by,
            "diagram": diagram,
        })))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

// ── schema.generate-verb-flow ─────────────────────────────────────────────────

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

    async fn execute_json(
        &self,
        args: &Value,
        _ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let format =
            json_extract_string_opt(args, "format").unwrap_or_else(|| "mermaid".to_string());
        if format != "mermaid" {
            return Err(anyhow!(
                "schema.generate-verb-flow: unsupported format '{format}'"
            ));
        }
        let verb_fqn = json_extract_string_opt(args, "verb-fqn");
        let domain = json_extract_string_opt(args, "domain");
        if verb_fqn.is_none() && domain.is_none() {
            return Err(anyhow!(
                "schema.generate-verb-flow: either verb-fqn or domain is required"
            ));
        }
        let depth = json_extract_int_opt(args, "depth").unwrap_or(2) as u32;

        let graph = load_affinity_graph_cached(pool).await?;
        let (mode, verb_value, diagram) = match verb_fqn.as_ref() {
            Some(vfqn) => (
                "verb".to_string(),
                vfqn.clone(),
                mermaid::render_verb_flow(vfqn, &graph, depth),
            ),
            None => {
                let domain_value = domain.clone().unwrap_or_default();
                (
                    "domain".to_string(),
                    domain_value.clone(),
                    render_domain_verb_flow(&domain_value, &graph, depth),
                )
            }
        };

        Ok(VerbExecutionOutcome::Record(json!({
            "mode": mode,
            "verb_fqn": verb_value,
            "domain": domain,
            "depth": depth,
            "format": format,
            "diagram": diagram,
        })))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

// ── schema.generate-discovery-map ─────────────────────────────────────────────

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

    async fn execute_json(
        &self,
        args: &Value,
        _ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let domain = json_extract_string_opt(args, "domain");
        let format =
            json_extract_string_opt(args, "format").unwrap_or_else(|| "mermaid".to_string());
        if format != "mermaid" {
            return Err(anyhow!(
                "schema.generate-discovery-map: unsupported format '{format}'"
            ));
        }
        let cluster_by = json_extract_string_opt(args, "cluster-by")
            .unwrap_or_else(|| "verb".to_string());

        let graph = load_affinity_graph_cached(pool).await?;
        let verbs = load_active_verb_contracts(pool).await?;

        let scoped_verbs: Vec<VerbContractBody> = match domain.as_ref() {
            Some(dom) => verbs.into_iter().filter(|v| v.domain == *dom).collect(),
            None => verbs,
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
            .as_ref()
            .map(|d| format!("domain:{d}"))
            .unwrap_or_else(|| "domain:all".to_owned());
        let diagram = mermaid::render_discovery_map(&source_utterance, &discovery, &graph);

        Ok(VerbExecutionOutcome::Record(json!({
            "domain": domain,
            "cluster_by": cluster_by,
            "format": format,
            "utterance_count": utterances.len(),
            "intent_count": intent_matches.len(),
            "diagram": diagram,
        })))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}
