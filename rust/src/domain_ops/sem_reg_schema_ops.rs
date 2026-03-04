//! Schema domain CustomOps (Spec §C.2 — 5 verbs).
//!
//! Schema introspection and extraction verbs.
//! Allowed in BOTH Research and Governed AgentModes (read-only).

use anyhow::Result;
use async_trait::async_trait;

use ob_poc_macros::register_custom_op;

use super::sem_reg_helpers::{delegate_to_tool, get_bool_arg, get_int_arg, get_string_arg};
use super::{CustomOperation, ExecutionContext, ExecutionResult, VerbCall};

#[cfg(feature = "database")]
use sqlx::PgPool;

#[cfg(feature = "database")]
use {
    crate::domain_ops::affinity_graph_cache::load_affinity_graph_cached,
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
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        delegate_to_tool(pool, ctx, verb_call, "db_introspect").await
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("schema.introspect requires database"))
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
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        // Uses db_introspect as the underlying tool, with attribute extraction logic
        delegate_to_tool(pool, ctx, verb_call, "db_introspect").await
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "schema.extract-attributes requires database"
        ))
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
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        delegate_to_tool(pool, ctx, verb_call, "db_introspect").await
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("schema.extract-verbs requires database"))
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
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        delegate_to_tool(pool, ctx, verb_call, "db_introspect").await
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("schema.extract-entities requires database"))
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
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        delegate_to_tool(pool, ctx, verb_call, "db_introspect").await
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("schema.cross-reference requires database"))
    }
}

// ── Diagram generation ops ────────────────────────────────────────────────────

#[cfg(feature = "database")]
fn get_string_list_arg(verb_call: &VerbCall, name: &str) -> Option<Vec<String>> {
    verb_call.arguments.iter().find_map(|a| {
        if a.key != name {
            return None;
        }
        if let Some(s) = a.value.as_string() {
            return Some(vec![s.to_string()]);
        }
        a.value.as_list().map(|list| {
            list.iter()
                .filter_map(|node| node.as_string().map(ToOwned::to_owned))
                .collect()
        })
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
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let schema_name = get_string_arg(verb_call, "schema-name")
            .ok_or_else(|| anyhow::anyhow!("schema.generate-erd: schema-name is required"))?;
        let format = get_string_arg(verb_call, "format").unwrap_or_else(|| "mermaid".to_string());
        if format != "mermaid" {
            return Err(anyhow::anyhow!(
                "schema.generate-erd: unsupported format '{}', only 'mermaid' is currently supported",
                format
            ));
        }
        let show_verb_surface = get_bool_arg(verb_call, "show-verb-surface").unwrap_or(true);
        let show_affinity_kind = get_bool_arg(verb_call, "show-affinity-kind").unwrap_or(false);
        let enrich = get_bool_arg(verb_call, "enrich").unwrap_or(true);
        let domain_filter = get_string_arg(verb_call, "domain");
        let group_by =
            get_string_arg(verb_call, "group-by").unwrap_or_else(|| "schema".to_string());
        let max_tables = get_int_arg(verb_call, "max-tables").unwrap_or(50) as usize;
        let requested_tables = get_string_list_arg(verb_call, "tables");

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
        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("schema.generate-erd requires database"))
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
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let format = get_string_arg(verb_call, "format").unwrap_or_else(|| "mermaid".to_string());
        if format != "mermaid" {
            return Err(anyhow::anyhow!(
                "schema.generate-verb-flow: unsupported format '{}', only 'mermaid' is currently supported",
                format
            ));
        }
        let verb_fqn = get_string_arg(verb_call, "verb-fqn");
        let domain = get_string_arg(verb_call, "domain");
        if verb_fqn.is_none() && domain.is_none() {
            return Err(anyhow::anyhow!(
                "schema.generate-verb-flow: either verb-fqn or domain is required"
            ));
        }
        let depth = get_int_arg(verb_call, "depth").unwrap_or(2) as u32;

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
        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "schema.generate-verb-flow requires database"
        ))
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
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let domain = get_string_arg(verb_call, "domain");
        let format = get_string_arg(verb_call, "format").unwrap_or_else(|| "mermaid".to_string());
        if format != "mermaid" {
            return Err(anyhow::anyhow!(
                "schema.generate-discovery-map: unsupported format '{}', only 'mermaid' is currently supported",
                format
            ));
        }
        let cluster_by =
            get_string_arg(verb_call, "cluster-by").unwrap_or_else(|| "verb".to_string());
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
        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "schema.generate-discovery-map requires database"
        ))
    }
}
