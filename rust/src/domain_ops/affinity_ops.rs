//! Affinity navigation CustomOps — 6 registry verbs that query the AffinityGraph.
//!
//! These ops load the in-memory `AffinityGraph`, run the requested query,
//! and return typed results.
//!
//! Verb mappings:
//!  - `registry.verbs-for-table`     → AffinityVerbsForTableOp
//!  - `registry.verbs-for-attribute` → AffinityVerbsForAttributeOp
//!  - `registry.data-for-verb`       → AffinityDataForVerbOp
//!  - `registry.adjacent-verbs`      → AffinityAdjacentVerbsOp
//!  - `registry.governance-gaps`     → AffinityGovernanceGapsOp
//!  - `registry.discover-dsl`        → AffinityDiscoverDslOp

use anyhow::Result;
use async_trait::async_trait;
use ob_poc_macros::register_custom_op;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

use super::sem_os_helpers::{get_bool_arg, get_int_arg, get_string_arg};
use super::{CustomOperation, ExecutionContext, ExecutionResult, VerbCall};

#[cfg(feature = "database")]
use crate::sem_reg::{ActorContext, Classification};
#[cfg(feature = "database")]
use {
    crate::domain_ops::affinity_graph_cache::load_affinity_graph_cached,
    sem_os_core::affinity::{discover_dsl, AffinityGraph, AffinityKind, DataRef, TableRef},
    sem_os_core::context_resolution::{
        ContextResolutionRequest, DiscoveryContext, EvidenceMode, SubjectRef,
    },
    sem_os_core::verb_contract::VerbContractBody,
    sqlx::PgPool,
};

// ── Shared result types ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbAffinityRow {
    pub verb_fqn: String,
    pub affinity_kind: String,
    pub provenance: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbsForTableResult {
    pub schema_name: String,
    pub table_name: String,
    pub verbs: Vec<VerbAffinityRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbsForAttributeResult {
    pub attribute_fqn: String,
    pub verbs: Vec<VerbAffinityRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataAffinityRow {
    pub data_ref_kind: String,
    pub data_ref_key: String,
    pub affinity_kind: String,
    pub provenance: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataForVerbResult {
    pub verb_fqn: String,
    pub depth: u32,
    pub data_assets: Vec<DataAffinityRow>,
    /// Transitive reach at depth > 1
    pub footprint: Option<DataFootprintSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataFootprintSummary {
    pub tables: Vec<String>,
    pub columns: Vec<String>,
    pub attributes: Vec<String>,
    pub entity_types: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdjacentVerbEntry {
    pub verb_fqn: String,
    pub shared_data: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdjacentVerbsResult {
    pub verb_fqn: String,
    pub adjacent: Vec<AdjacentVerbEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceGapsResult {
    pub orphan_tables: Vec<String>,
    pub orphan_verbs: Vec<String>,
    pub write_only_attributes: Vec<String>,
    pub read_before_write_attributes: Vec<String>,
}

// ── Shared helpers ────────────────────────────────────────────────────────────

#[cfg(feature = "database")]
async fn load_affinity_graph(_ctx: &ExecutionContext, pool: &PgPool) -> Result<AffinityGraph> {
    load_affinity_graph_cached(pool).await
}

#[cfg(feature = "database")]
async fn load_active_verb_contracts(pool: &PgPool) -> Result<Vec<VerbContractBody>> {
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
fn to_sem_os_actor(actor: &ActorContext) -> sem_os_core::abac::ActorContext {
    let json = serde_json::to_value(actor).expect("ActorContext serializes");
    serde_json::from_value(json).expect("ActorContext round-trips")
}

#[cfg(feature = "database")]
async fn resolve_context_via_sem_os(
    pool: &PgPool,
    actor: &ActorContext,
    request: ContextResolutionRequest,
) -> sem_os_core::service::Result<sem_os_core::context_resolution::ContextResolutionResponse> {
    let principal =
        sem_os_core::principal::Principal::in_process(&actor.actor_id, actor.roles.clone());
    let service = crate::sem_reg::agent::mcp_tools::build_sem_os_service(pool);
    service.resolve_context(&principal, request).await
}

#[derive(Copy, Clone, Eq, PartialEq)]
enum AttributeDirection {
    Produces,
    Consumes,
    Both,
}

impl AttributeDirection {
    fn from_arg(value: Option<String>) -> Result<Self> {
        match value.as_deref().map(str::to_lowercase).as_deref() {
            None | Some("both") => Ok(Self::Both),
            Some("produces") => Ok(Self::Produces),
            Some("consumes") => Ok(Self::Consumes),
            Some(other) => Err(anyhow::anyhow!(
                "registry.verbs-for-attribute: direction must be one of produces|consumes|both, got '{other}'"
            )),
        }
    }

    fn include(self, kind: &AffinityKind) -> bool {
        matches!(
            (self, kind),
            (Self::Both, _)
                | (Self::Produces, AffinityKind::ProducesAttribute)
                | (Self::Consumes, AffinityKind::ConsumesAttribute { .. })
        )
    }
}

#[derive(Copy, Clone, Eq, PartialEq)]
enum GapType {
    OrphanTables,
    OrphanVerbs,
    WriteOnly,
    ReadBeforeWrite,
    All,
}

impl GapType {
    fn from_arg(value: Option<String>) -> Result<Self> {
        match value.as_deref().map(str::to_lowercase).as_deref() {
            None | Some("all") => Ok(Self::All),
            Some("orphan_tables") => Ok(Self::OrphanTables),
            Some("orphan_verbs") => Ok(Self::OrphanVerbs),
            Some("write_only") => Ok(Self::WriteOnly),
            Some("read_before_write") => Ok(Self::ReadBeforeWrite),
            Some(other) => Err(anyhow::anyhow!(
                "registry.governance-gaps: gap-type must be orphan_tables|orphan_verbs|write_only|read_before_write|all, got '{other}'"
            )),
        }
    }
}

// ── AffinityVerbsForTableOp ───────────────────────────────────────────────────

/// Find all verbs that read, write, or reference a database table.
#[register_custom_op]
pub struct AffinityVerbsForTableOp;

#[async_trait]
impl CustomOperation for AffinityVerbsForTableOp {
    fn domain(&self) -> &'static str {
        "registry"
    }
    fn verb(&self) -> &'static str {
        "verbs-for-table"
    }
    fn rationale(&self) -> &'static str {
        "Queries the AffinityGraph for all verbs touching a physical database table"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let schema_name = get_string_arg(verb_call, "schema-name")
            .ok_or_else(|| anyhow::anyhow!("registry.verbs-for-table: schema-name is required"))?;
        let table_name = get_string_arg(verb_call, "table-name")
            .ok_or_else(|| anyhow::anyhow!("registry.verbs-for-table: table-name is required"))?;
        let include_lookups = get_bool_arg(verb_call, "include-lookups").unwrap_or(true);

        let graph = load_affinity_graph(ctx, pool).await?;
        let verb_affinities = graph.verbs_for_table(&schema_name, &table_name);

        let verbs: Vec<VerbAffinityRow> = verb_affinities
            .into_iter()
            .filter(|va| {
                include_lookups || !matches!(va.affinity_kind, AffinityKind::ArgLookup { .. })
            })
            .map(|va| VerbAffinityRow {
                verb_fqn: va.verb_fqn,
                affinity_kind: format!("{:?}", va.affinity_kind),
                provenance: format!("{:?}", va.provenance),
            })
            .collect();

        let result = VerbsForTableResult {
            schema_name,
            table_name,
            verbs,
        };

        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        _ctx: &mut sem_os_core::execution::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<sem_os_core::execution::VerbExecutionOutcome> {
        use super::helpers::{json_extract_bool_opt, json_extract_string};
        let schema_name = json_extract_string(args, "schema-name")?;
        let table_name = json_extract_string(args, "table-name")?;
        let include_lookups = json_extract_bool_opt(args, "include-lookups").unwrap_or(true);

        let graph = load_affinity_graph_cached(pool).await?;
        let verb_affinities = graph.verbs_for_table(&schema_name, &table_name);
        let verbs: Vec<VerbAffinityRow> = verb_affinities
            .into_iter()
            .filter(|va| {
                include_lookups || !matches!(va.affinity_kind, AffinityKind::ArgLookup { .. })
            })
            .map(|va| VerbAffinityRow {
                verb_fqn: va.verb_fqn,
                affinity_kind: format!("{:?}", va.affinity_kind),
                provenance: format!("{:?}", va.provenance),
            })
            .collect();
        let result = VerbsForTableResult {
            schema_name,
            table_name,
            verbs,
        };
        Ok(sem_os_core::execution::VerbExecutionOutcome::Record(
            serde_json::to_value(result)?,
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "registry.verbs-for-table requires database feature"
        ))
    }
}

// ── AffinityVerbsForAttributeOp ───────────────────────────────────────────────

/// Find all verbs that produce or consume a given attribute.
#[register_custom_op]
pub struct AffinityVerbsForAttributeOp;

#[async_trait]
impl CustomOperation for AffinityVerbsForAttributeOp {
    fn domain(&self) -> &'static str {
        "registry"
    }
    fn verb(&self) -> &'static str {
        "verbs-for-attribute"
    }
    fn rationale(&self) -> &'static str {
        "Queries the AffinityGraph for all verbs producing or consuming a given attribute"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let attr_fqn = get_string_arg(verb_call, "attribute-fqn")
            .or_else(|| get_string_arg(verb_call, "attr-fqn"))
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "registry.verbs-for-attribute: attribute-fqn (or legacy attr-fqn) is required"
                )
            })?;
        let direction = AttributeDirection::from_arg(get_string_arg(verb_call, "direction"))?;

        let graph = load_affinity_graph(ctx, pool).await?;
        let verb_affinities = graph
            .verbs_for_attribute(&attr_fqn)
            .into_iter()
            .filter(|va| direction.include(&va.affinity_kind));

        let verbs: Vec<VerbAffinityRow> = verb_affinities
            .map(|va| VerbAffinityRow {
                verb_fqn: va.verb_fqn,
                affinity_kind: format!("{:?}", va.affinity_kind),
                provenance: format!("{:?}", va.provenance),
            })
            .collect();

        let result = VerbsForAttributeResult {
            attribute_fqn: attr_fqn,
            verbs,
        };

        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        _ctx: &mut sem_os_core::execution::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<sem_os_core::execution::VerbExecutionOutcome> {
        use super::helpers::{json_extract_string, json_extract_string_opt};
        let attr_fqn = json_extract_string(args, "attribute-fqn")
            .or_else(|_| json_extract_string(args, "attr-fqn"))?;
        let direction = AttributeDirection::from_arg(json_extract_string_opt(args, "direction"))?;

        let graph = load_affinity_graph_cached(pool).await?;
        let verbs: Vec<VerbAffinityRow> = graph
            .verbs_for_attribute(&attr_fqn)
            .into_iter()
            .filter(|va| direction.include(&va.affinity_kind))
            .map(|va| VerbAffinityRow {
                verb_fqn: va.verb_fqn,
                affinity_kind: format!("{:?}", va.affinity_kind),
                provenance: format!("{:?}", va.provenance),
            })
            .collect();
        let result = VerbsForAttributeResult {
            attribute_fqn: attr_fqn,
            verbs,
        };
        Ok(sem_os_core::execution::VerbExecutionOutcome::Record(
            serde_json::to_value(result)?,
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "registry.verbs-for-attribute requires database feature"
        ))
    }
}

// ── AffinityDataForVerbOp ─────────────────────────────────────────────────────

/// Find all data assets a verb touches. Optionally computes the transitive
/// data footprint up to `depth` hops.
#[register_custom_op]
pub struct AffinityDataForVerbOp;

#[async_trait]
impl CustomOperation for AffinityDataForVerbOp {
    fn domain(&self) -> &'static str {
        "registry"
    }
    fn verb(&self) -> &'static str {
        "data-for-verb"
    }
    fn rationale(&self) -> &'static str {
        "Queries the AffinityGraph for all data assets touched by a verb, with optional transitive footprint"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let verb_fqn = get_string_arg(verb_call, "verb-fqn")
            .ok_or_else(|| anyhow::anyhow!("registry.data-for-verb: verb-fqn is required"))?;
        let depth = get_int_arg(verb_call, "depth")
            .map(|d| d as u32)
            .unwrap_or(1);

        let graph = load_affinity_graph(ctx, pool).await?;
        let data_assets = graph.data_for_verb(&verb_fqn);

        let asset_rows: Vec<DataAffinityRow> = data_assets
            .into_iter()
            .map(|da| {
                let data_ref_kind = match &da.data_ref {
                    DataRef::Table(_) => "table",
                    DataRef::Column(_) => "column",
                    DataRef::EntityType { .. } => "entity_type",
                    DataRef::Attribute { .. } => "attribute",
                }
                .to_owned();
                DataAffinityRow {
                    data_ref_kind,
                    data_ref_key: da.data_ref.index_key(),
                    affinity_kind: format!("{:?}", da.affinity_kind),
                    provenance: format!("{:?}", da.provenance),
                }
            })
            .collect();

        let footprint = if depth > 1 {
            let fp = graph.data_footprint(&verb_fqn, depth);
            Some(DataFootprintSummary {
                tables: fp.tables.values().map(|t| t.key()).collect(),
                columns: fp.columns.keys().cloned().collect(),
                attributes: fp.attributes.into_iter().collect(),
                entity_types: fp.entity_types.into_iter().collect(),
            })
        } else {
            None
        };

        let result = DataForVerbResult {
            verb_fqn,
            depth,
            data_assets: asset_rows,
            footprint,
        };

        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        _ctx: &mut sem_os_core::execution::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<sem_os_core::execution::VerbExecutionOutcome> {
        use super::helpers::{json_extract_int_opt, json_extract_string};
        let verb_fqn = json_extract_string(args, "verb-fqn")?;
        let depth = json_extract_int_opt(args, "depth")
            .map(|d| d as u32)
            .unwrap_or(1);

        let graph = load_affinity_graph_cached(pool).await?;
        let data_assets = graph.data_for_verb(&verb_fqn);
        let asset_rows: Vec<DataAffinityRow> = data_assets
            .into_iter()
            .map(|da| {
                let data_ref_kind = match &da.data_ref {
                    DataRef::Table(_) => "table",
                    DataRef::Column(_) => "column",
                    DataRef::EntityType { .. } => "entity_type",
                    DataRef::Attribute { .. } => "attribute",
                }
                .to_owned();
                DataAffinityRow {
                    data_ref_kind,
                    data_ref_key: da.data_ref.index_key(),
                    affinity_kind: format!("{:?}", da.affinity_kind),
                    provenance: format!("{:?}", da.provenance),
                }
            })
            .collect();
        let footprint = if depth > 1 {
            let fp = graph.data_footprint(&verb_fqn, depth);
            Some(DataFootprintSummary {
                tables: fp.tables.values().map(|t| t.key()).collect(),
                columns: fp.columns.keys().cloned().collect(),
                attributes: fp.attributes.into_iter().collect(),
                entity_types: fp.entity_types.into_iter().collect(),
            })
        } else {
            None
        };
        let result = DataForVerbResult {
            verb_fqn,
            depth,
            data_assets: asset_rows,
            footprint,
        };
        Ok(sem_os_core::execution::VerbExecutionOutcome::Record(
            serde_json::to_value(result)?,
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "registry.data-for-verb requires database feature"
        ))
    }
}

// ── AffinityAdjacentVerbsOp ───────────────────────────────────────────────────

/// Find verbs sharing data dependencies with a given verb.
#[register_custom_op]
pub struct AffinityAdjacentVerbsOp;

#[async_trait]
impl CustomOperation for AffinityAdjacentVerbsOp {
    fn domain(&self) -> &'static str {
        "registry"
    }
    fn verb(&self) -> &'static str {
        "adjacent-verbs"
    }
    fn rationale(&self) -> &'static str {
        "Queries the AffinityGraph for verbs sharing data dependencies with a given verb"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let verb_fqn = get_string_arg(verb_call, "verb-fqn")
            .ok_or_else(|| anyhow::anyhow!("registry.adjacent-verbs: verb-fqn is required"))?;
        let min_overlap = get_int_arg(verb_call, "min-overlap").unwrap_or(1).max(1) as usize;

        let graph = load_affinity_graph(ctx, pool).await?;
        let adjacent = graph.adjacent_verbs(&verb_fqn);

        let entries: Vec<AdjacentVerbEntry> = adjacent
            .into_iter()
            .map(|(adj_fqn, shared_refs)| AdjacentVerbEntry {
                verb_fqn: adj_fqn,
                shared_data: shared_refs.into_iter().map(|r| r.index_key()).collect(),
            })
            .filter(|row| row.shared_data.len() >= min_overlap)
            .collect();

        let result = AdjacentVerbsResult {
            verb_fqn,
            adjacent: entries,
        };

        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        _ctx: &mut sem_os_core::execution::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<sem_os_core::execution::VerbExecutionOutcome> {
        use super::helpers::{json_extract_int_opt, json_extract_string};
        let verb_fqn = json_extract_string(args, "verb-fqn")?;
        let min_overlap = json_extract_int_opt(args, "min-overlap")
            .unwrap_or(1)
            .max(1) as usize;

        let graph = load_affinity_graph_cached(pool).await?;
        let adjacent = graph.adjacent_verbs(&verb_fqn);
        let entries: Vec<AdjacentVerbEntry> = adjacent
            .into_iter()
            .map(|(adj_fqn, shared_refs)| AdjacentVerbEntry {
                verb_fqn: adj_fqn,
                shared_data: shared_refs.into_iter().map(|r| r.index_key()).collect(),
            })
            .filter(|row| row.shared_data.len() >= min_overlap)
            .collect();
        let result = AdjacentVerbsResult {
            verb_fqn,
            adjacent: entries,
        };
        Ok(sem_os_core::execution::VerbExecutionOutcome::Record(
            serde_json::to_value(result)?,
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "registry.adjacent-verbs requires database feature"
        ))
    }
}

// ── AffinityGovernanceGapsOp ──────────────────────────────────────────────────

/// Identify governance gaps: tables with no verb affinity, verbs with no data
/// affinity, and attributes that are written-only or read-before-write.
#[register_custom_op]
pub struct AffinityGovernanceGapsOp;

#[async_trait]
impl CustomOperation for AffinityGovernanceGapsOp {
    fn domain(&self) -> &'static str {
        "registry"
    }
    fn verb(&self) -> &'static str {
        "governance-gaps"
    }
    fn rationale(&self) -> &'static str {
        "Identifies ungoverned tables, disconnected verbs, and attribute flow anomalies via the AffinityGraph"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let schema_filter = get_string_arg(verb_call, "schema-name");
        let gap_type = GapType::from_arg(get_string_arg(verb_call, "gap-type"))?;

        // Build the affinity graph
        let graph = load_affinity_graph(ctx, pool).await?;

        // Load physical table list from information_schema
        let known_tables = if let Some(ref schema) = schema_filter {
            sqlx::query_as::<_, (String, String)>(
                "SELECT table_schema, table_name \
                 FROM information_schema.tables \
                 WHERE table_type = 'BASE TABLE' AND table_schema = $1 \
                 ORDER BY table_schema, table_name",
            )
            .bind(schema)
            .fetch_all(pool)
            .await?
        } else {
            sqlx::query_as::<_, (String, String)>(
                "SELECT table_schema, table_name \
                 FROM information_schema.tables \
                 WHERE table_type = 'BASE TABLE' \
                   AND table_schema NOT IN ('pg_catalog','information_schema','pg_toast') \
                 ORDER BY table_schema, table_name",
            )
            .fetch_all(pool)
            .await?
        };

        let known_table_refs: Vec<TableRef> = known_tables
            .into_iter()
            .map(|(s, t)| TableRef::new(s, t))
            .collect();

        let orphan_tables_all: Vec<String> = graph
            .orphan_tables(&known_table_refs)
            .into_iter()
            .map(|t| t.key())
            .collect();
        let orphan_verbs_all = graph.orphan_verbs();
        let write_only_all = graph.write_only_attributes();
        let read_before_write_all = graph.read_before_write_attributes();

        let result = GovernanceGapsResult {
            orphan_tables: if matches!(gap_type, GapType::All | GapType::OrphanTables) {
                orphan_tables_all
            } else {
                Vec::new()
            },
            orphan_verbs: if matches!(gap_type, GapType::All | GapType::OrphanVerbs) {
                orphan_verbs_all
            } else {
                Vec::new()
            },
            write_only_attributes: if matches!(gap_type, GapType::All | GapType::WriteOnly) {
                write_only_all
            } else {
                Vec::new()
            },
            read_before_write_attributes: if matches!(
                gap_type,
                GapType::All | GapType::ReadBeforeWrite
            ) {
                read_before_write_all
            } else {
                Vec::new()
            },
        };

        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        _ctx: &mut sem_os_core::execution::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<sem_os_core::execution::VerbExecutionOutcome> {
        use super::helpers::json_extract_string_opt;
        let schema_filter = json_extract_string_opt(args, "schema-name");
        let gap_type = GapType::from_arg(json_extract_string_opt(args, "gap-type"))?;

        let graph = load_affinity_graph_cached(pool).await?;
        let known_tables = if let Some(ref schema) = schema_filter {
            sqlx::query_as::<_, (String, String)>(
                "SELECT table_schema, table_name \
                 FROM information_schema.tables \
                 WHERE table_type = 'BASE TABLE' AND table_schema = $1 \
                 ORDER BY table_schema, table_name",
            )
            .bind(schema)
            .fetch_all(pool)
            .await?
        } else {
            sqlx::query_as::<_, (String, String)>(
                "SELECT table_schema, table_name \
                 FROM information_schema.tables \
                 WHERE table_type = 'BASE TABLE' \
                   AND table_schema NOT IN ('pg_catalog','information_schema','pg_toast') \
                 ORDER BY table_schema, table_name",
            )
            .fetch_all(pool)
            .await?
        };
        let known_table_refs: Vec<TableRef> = known_tables
            .into_iter()
            .map(|(s, t)| TableRef::new(s, t))
            .collect();
        let orphan_tables_all: Vec<String> = graph
            .orphan_tables(&known_table_refs)
            .into_iter()
            .map(|t| t.key())
            .collect();
        let orphan_verbs_all = graph.orphan_verbs();
        let write_only_all = graph.write_only_attributes();
        let read_before_write_all = graph.read_before_write_attributes();
        let result = GovernanceGapsResult {
            orphan_tables: if matches!(gap_type, GapType::All | GapType::OrphanTables) {
                orphan_tables_all
            } else {
                Vec::new()
            },
            orphan_verbs: if matches!(gap_type, GapType::All | GapType::OrphanVerbs) {
                orphan_verbs_all
            } else {
                Vec::new()
            },
            write_only_attributes: if matches!(gap_type, GapType::All | GapType::WriteOnly) {
                write_only_all
            } else {
                Vec::new()
            },
            read_before_write_attributes: if matches!(
                gap_type,
                GapType::All | GapType::ReadBeforeWrite
            ) {
                read_before_write_all
            } else {
                Vec::new()
            },
        };
        Ok(sem_os_core::execution::VerbExecutionOutcome::Record(
            serde_json::to_value(result)?,
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "registry.governance-gaps requires database feature"
        ))
    }
}

// ── AffinityDiscoverDslOp ─────────────────────────────────────────────────────

/// Discover intent and synthesize a safe DSL chain from natural language.
#[register_custom_op]
pub struct AffinityDiscoverDslOp;

#[async_trait]
impl CustomOperation for AffinityDiscoverDslOp {
    fn domain(&self) -> &'static str {
        "registry"
    }
    fn verb(&self) -> &'static str {
        "discover-dsl"
    }
    fn rationale(&self) -> &'static str {
        "Utterance -> intent -> DSL chain synthesis via AffinityGraph with CCIR-aware filtering"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let utterance = get_string_arg(verb_call, "utterance")
            .or_else(|| get_string_arg(verb_call, "intent"))
            .ok_or_else(|| anyhow::anyhow!("registry.discover-dsl: utterance is required"))?;
        let subject_id = get_string_arg(verb_call, "subject-id");
        let max_chain_length = get_int_arg(verb_call, "max-chain-length")
            .map(|v| v.max(1) as usize)
            .unwrap_or(5);

        let subject_uuid = subject_id
            .as_deref()
            .map(Uuid::parse_str)
            .transpose()
            .map_err(|e| anyhow::anyhow!("registry.discover-dsl: invalid subject-id: {e}"))?;

        let graph = load_affinity_graph(ctx, pool).await?;
        let verb_contracts = load_active_verb_contracts(pool).await?;

        let mut allowed_verbs: Option<HashSet<String>> = None;
        let mut policy_check = None;

        if let Some(subject_uuid) = subject_uuid {
            let actor = ActorContext {
                actor_id: ctx
                    .audit_user
                    .clone()
                    .unwrap_or_else(|| "sem-os-discovery".to_owned()),
                roles: vec!["analyst".to_owned()],
                department: Some("registry".to_owned()),
                clearance: Some(Classification::Internal),
                jurisdictions: vec![],
            };
            let request = ContextResolutionRequest {
                subject: SubjectRef::EntityId(subject_uuid),
                intent_summary: None,
                raw_utterance: Some(utterance.clone()),
                actor: to_sem_os_actor(&actor),
                goals: vec!["dsl_discovery".to_owned()],
                constraints: Default::default(),
                evidence_mode: EvidenceMode::Normal,
                point_in_time: None,
                entity_kind: None,
                entity_confidence: None,
                discovery: DiscoveryContext::default(),
            };
            match resolve_context_via_sem_os(pool, &actor, request).await {
                Ok(resp) => {
                    let allowed: HashSet<String> =
                        resp.candidate_verbs.into_iter().map(|v| v.fqn).collect();
                    policy_check = Some(format!(
                        "ccir_applied:candidates={},confidence={:.2}",
                        allowed.len(),
                        resp.confidence
                    ));
                    allowed_verbs = Some(allowed);
                }
                Err(err) => {
                    policy_check = Some(format!("ccir_failed:{err}"));
                }
            }
        }

        let result = discover_dsl(
            &utterance,
            &graph,
            &verb_contracts,
            subject_uuid,
            max_chain_length,
            allowed_verbs.as_ref(),
            policy_check,
        );
        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut sem_os_core::execution::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<sem_os_core::execution::VerbExecutionOutcome> {
        use super::helpers::{json_extract_int_opt, json_extract_string, json_extract_string_opt};
        let utterance = json_extract_string(args, "utterance")
            .or_else(|_| json_extract_string(args, "intent"))?;
        let subject_id = json_extract_string_opt(args, "subject-id");
        let max_chain_length = json_extract_int_opt(args, "max-chain-length")
            .map(|v| v.max(1) as usize)
            .unwrap_or(5);

        let subject_uuid = subject_id
            .as_deref()
            .map(Uuid::parse_str)
            .transpose()
            .map_err(|e| anyhow::anyhow!("invalid subject-id: {e}"))?;

        let graph = load_affinity_graph_cached(pool).await?;
        let verb_contracts = load_active_verb_contracts(pool).await?;

        let mut allowed_verbs: Option<HashSet<String>> = None;
        let mut policy_check = None;

        if let Some(subject_uuid) = subject_uuid {
            let actor_id = ctx.principal.actor_id.clone();
            let actor = ActorContext {
                actor_id: actor_id.clone(),
                roles: vec!["analyst".to_owned()],
                department: Some("registry".to_owned()),
                clearance: Some(Classification::Internal),
                jurisdictions: vec![],
            };
            let request = ContextResolutionRequest {
                subject: SubjectRef::EntityId(subject_uuid),
                intent_summary: None,
                raw_utterance: Some(utterance.clone()),
                actor: to_sem_os_actor(&actor),
                goals: vec!["dsl_discovery".to_owned()],
                constraints: Default::default(),
                evidence_mode: EvidenceMode::Normal,
                point_in_time: None,
                entity_kind: None,
                entity_confidence: None,
                discovery: DiscoveryContext::default(),
            };
            match resolve_context_via_sem_os(pool, &actor, request).await {
                Ok(resp) => {
                    let allowed: HashSet<String> =
                        resp.candidate_verbs.into_iter().map(|v| v.fqn).collect();
                    policy_check = Some(format!(
                        "ccir_applied:candidates={},confidence={:.2}",
                        allowed.len(),
                        resp.confidence
                    ));
                    allowed_verbs = Some(allowed);
                }
                Err(err) => {
                    policy_check = Some(format!("ccir_failed:{err}"));
                }
            }
        }

        let result = discover_dsl(
            &utterance,
            &graph,
            &verb_contracts,
            subject_uuid,
            max_chain_length,
            allowed_verbs.as_ref(),
            policy_check,
        );
        Ok(sem_os_core::execution::VerbExecutionOutcome::Record(
            serde_json::to_value(result)?,
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("registry.discover-dsl requires database"))
    }
}
