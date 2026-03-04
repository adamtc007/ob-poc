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
    crate::sem_reg::types::{pg_rows_to_snapshot_rows, PgSnapshotRow},
    sem_os_core::affinity::AffinityGraph,
    sem_os_core::diagram::{
        enrichment::build_diagram_model,
        mermaid,
        model::{ColumnInput, ForeignKeyInput, RenderOptions, TableInput},
    },
};

// ── Typed result types ────────────────────────────────────────────

#[derive(Debug, serde::Serialize)]
struct GenerateErdResult {
    schema: String,
    format: String,
    table_count: usize,
    diagram: String,
}

#[derive(Debug, serde::Serialize)]
struct GenerateVerbFlowResult {
    verb_fqn: String,
    depth: u32,
    format: String,
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

/// Shared helper: load AffinityGraph from active snapshots.
#[cfg(feature = "database")]
async fn load_affinity_graph_for_schema(pool: &PgPool) -> anyhow::Result<AffinityGraph> {
    let pg_rows = sqlx::query_as::<_, PgSnapshotRow>(
        "SELECT snapshot_id, snapshot_set_id, object_type, object_id, \
         version_major, version_minor, status, governance_tier, trust_class, \
         security_label, effective_from, effective_until, predecessor_id, \
         change_type, change_rationale, created_by, approved_by, definition, \
         created_at \
         FROM sem_reg.snapshots WHERE status = 'active'",
    )
    .fetch_all(pool)
    .await?;
    let snapshots = pg_rows_to_snapshot_rows(pg_rows)?;
    Ok(AffinityGraph::build(&snapshots))
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
        let show_verb_surface = get_bool_arg(verb_call, "show-verb-surface").unwrap_or(true);
        let domain_filter = get_string_arg(verb_call, "domain");
        let max_tables = get_int_arg(verb_call, "max-tables").unwrap_or(50) as usize;

        let tables = extract_tables_from_schema(pool, &schema_name).await?;
        let graph = load_affinity_graph_for_schema(pool).await?;

        let options = RenderOptions {
            schema_filter: Some(vec![schema_name.clone()]),
            domain_filter,
            include_columns: true,
            show_governance: false,
            show_verb_surface,
            max_tables: Some(max_tables),
            format: Some("mermaid".to_string()),
        };
        let model = build_diagram_model(&tables, &graph, &options);
        let mermaid_output = mermaid::render_erd(&model, &options);

        let result = GenerateErdResult {
            schema: schema_name,
            format: "mermaid".to_string(),
            table_count: model.entities.len(),
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
        let verb_fqn = get_string_arg(verb_call, "verb-fqn")
            .ok_or_else(|| anyhow::anyhow!("schema.generate-verb-flow: verb-fqn is required"))?;
        let depth = get_int_arg(verb_call, "depth").unwrap_or(1) as u32;

        let graph = load_affinity_graph_for_schema(pool).await?;
        let mermaid_output = mermaid::render_verb_flow(&verb_fqn, &graph, depth);

        let result = GenerateVerbFlowResult {
            verb_fqn,
            depth,
            format: "mermaid".to_string(),
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
        Err(anyhow::anyhow!(
            "schema.generate-verb-flow requires database"
        ))
    }
}

/// Generate a domain discovery map (Phase 2 stub).
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
        "Discovery map generation deferred to Phase 2"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::to_value(
            serde_json::json!({
                "status": "not_implemented",
                "message": "schema.generate-discovery-map is planned for Phase 2.",
            }),
        )?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::to_value(
            serde_json::json!({
                "status": "not_implemented",
                "message": "schema.generate-discovery-map is planned for Phase 2.",
            }),
        )?))
    }
}
