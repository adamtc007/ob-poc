//! Schema diagram-generation verbs (3) — physical schema + AffinityGraph
//! → Mermaid.
//!
//! `generate-erd` walks `information_schema` for tables/columns/PKs/FKs
//! and renders via `sem_os_core::diagram::{enrichment, mermaid, model}`.
//! `generate-verb-flow` renders a verb-centric or domain-centric flow
//! over `AffinityGraph`. `generate-discovery-map` composes verb
//! invocation phrases → intent matches → mermaid discovery map.
//!
//! The `AffinityGraph` is loaded via the shared
//! `dsl-runtime::domain_ops::affinity_graph_cache::load_affinity_graph_cached`
//! (same reuse pattern as slice #9). The `information_schema`
//! queries and `sem_reg.snapshots` read run on `scope.executor()` so
//! they participate in the Sequencer-owned txn.

use std::collections::HashSet;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::{json, Value};

use sem_os_core::affinity::{match_intent, AffinityGraph, DataRef};
use sem_os_core::diagram::{
    enrichment::build_diagram_model,
    mermaid,
    model::{ColumnInput, ForeignKeyInput, RenderOptions, TableInput},
};
use sem_os_core::verb_contract::VerbContractBody;

use dsl_runtime::domain_ops::affinity_graph_cache::load_affinity_graph_cached;
use dsl_runtime::domain_ops::helpers::{
    json_extract_bool_opt, json_extract_int_opt, json_extract_string, json_extract_string_opt,
};
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use crate::ops::SemOsVerbOp;

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
    scope: &mut dyn TransactionScope,
    schema_name: &str,
) -> Result<Vec<TableInput>> {
    let table_rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT table_schema, table_name FROM information_schema.tables \
         WHERE table_type = 'BASE TABLE' AND table_schema = $1 \
         ORDER BY table_name",
    )
    .bind(schema_name)
    .fetch_all(scope.executor())
    .await?;

    let mut tables = Vec::new();
    for (schema, tname) in table_rows {
        let col_rows: Vec<(String, String, String)> = sqlx::query_as(
            "SELECT column_name, data_type, is_nullable FROM information_schema.columns \
             WHERE table_schema = $1 AND table_name = $2 \
             ORDER BY ordinal_position",
        )
        .bind(&schema)
        .bind(&tname)
        .fetch_all(scope.executor())
        .await?;

        let columns: Vec<ColumnInput> = col_rows
            .into_iter()
            .map(|(name, sql_type, is_nullable)| ColumnInput {
                name,
                sql_type,
                is_nullable: is_nullable.eq_ignore_ascii_case("YES"),
            })
            .collect();

        let pk_rows: Vec<(String,)> = sqlx::query_as(
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
        .fetch_all(scope.executor())
        .await?;
        let primary_keys: Vec<String> = pk_rows.into_iter().map(|(c,)| c).collect();

        let fk_rows: Vec<(String, String, String, String)> = sqlx::query_as(
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
        .fetch_all(scope.executor())
        .await?;
        let foreign_keys: Vec<ForeignKeyInput> = fk_rows
            .into_iter()
            .map(
                |(from_column, target_schema, target_table, target_column)| ForeignKeyInput {
                    from_column,
                    target_schema,
                    target_table,
                    target_column,
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

async fn load_active_verb_contracts(
    scope: &mut dyn TransactionScope,
) -> Result<Vec<VerbContractBody>> {
    let rows: Vec<(Value,)> = sqlx::query_as(
        "SELECT definition FROM sem_reg.snapshots \
         WHERE status = 'active' AND object_type::text = 'verb_contract'",
    )
    .fetch_all(scope.executor())
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

pub struct SchemaGenerateErd;

#[async_trait]
impl SemOsVerbOp for SchemaGenerateErd {
    fn fqn(&self) -> &str {
        "schema.generate-erd"
    }
    async fn execute(
        &self,
        args: &Value,
        _ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
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
        let group_by =
            json_extract_string_opt(args, "group-by").unwrap_or_else(|| "schema".to_string());
        let max_tables = json_extract_int_opt(args, "max-tables").unwrap_or(50) as usize;
        let requested_tables = extract_string_list_arg(args, "tables");

        let mut tables = extract_tables_from_schema(scope, &schema_name).await?;
        if let Some(wanted) = requested_tables {
            let wanted_set: HashSet<&str> = wanted.iter().map(String::as_str).collect();
            tables.retain(|t| wanted_set.contains(t.table_name.as_str()));
        }
        let graph = if enrich {
            load_affinity_graph_cached(scope.pool()).await?
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
}

// ── schema.generate-verb-flow ─────────────────────────────────────────────────

pub struct SchemaGenerateVerbFlow;

#[async_trait]
impl SemOsVerbOp for SchemaGenerateVerbFlow {
    fn fqn(&self) -> &str {
        "schema.generate-verb-flow"
    }
    async fn execute(
        &self,
        args: &Value,
        _ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
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

        let graph = load_affinity_graph_cached(scope.pool()).await?;
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
}

// ── schema.generate-discovery-map ─────────────────────────────────────────────

pub struct SchemaGenerateDiscoveryMap;

#[async_trait]
impl SemOsVerbOp for SchemaGenerateDiscoveryMap {
    fn fqn(&self) -> &str {
        "schema.generate-discovery-map"
    }
    async fn execute(
        &self,
        args: &Value,
        _ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let domain = json_extract_string_opt(args, "domain");
        let format =
            json_extract_string_opt(args, "format").unwrap_or_else(|| "mermaid".to_string());
        if format != "mermaid" {
            return Err(anyhow!(
                "schema.generate-discovery-map: unsupported format '{format}'"
            ));
        }
        let cluster_by =
            json_extract_string_opt(args, "cluster-by").unwrap_or_else(|| "verb".to_string());

        let graph = load_affinity_graph_cached(scope.pool()).await?;
        let verbs = load_active_verb_contracts(scope).await?;

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
}
