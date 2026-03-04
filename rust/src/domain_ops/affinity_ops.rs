//! Affinity navigation CustomOps — 6 registry verbs that query the AffinityGraph.
//!
//! These ops load active snapshots from `sem_reg.snapshots`, build the in-memory
//! `AffinityGraph`, then run the requested query and return typed results.
//!
//! Verb mappings:
//!  - `registry.verbs-for-table`     → AffinityVerbsForTableOp
//!  - `registry.verbs-for-attribute` → AffinityVerbsForAttributeOp
//!  - `registry.data-for-verb`       → AffinityDataForVerbOp
//!  - `registry.adjacent-verbs`      → AffinityAdjacentVerbsOp
//!  - `registry.governance-gaps`     → AffinityGovernanceGapsOp
//!  - `registry.discover-dsl`        → AffinityDiscoverDslOp (Phase 2 stub)

use anyhow::Result;
use async_trait::async_trait;
use ob_poc_macros::register_custom_op;
use serde::{Deserialize, Serialize};

use super::sem_reg_helpers::{get_int_arg, get_string_arg};
use super::{CustomOperation, ExecutionContext, ExecutionResult, VerbCall};

#[cfg(feature = "database")]
use {
    crate::sem_reg::types::{pg_rows_to_snapshot_rows, PgSnapshotRow},
    sem_os_core::affinity::{AffinityGraph, DataRef, TableRef},
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoverDslResult {
    pub status: String,
    pub message: String,
}

// ── Shared helper ─────────────────────────────────────────────────────────────

#[cfg(feature = "database")]
async fn load_affinity_graph(pool: &PgPool) -> Result<AffinityGraph> {
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
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let schema_name = get_string_arg(verb_call, "schema-name")
            .ok_or_else(|| anyhow::anyhow!("registry.verbs-for-table: schema-name is required"))?;
        let table_name = get_string_arg(verb_call, "table-name")
            .ok_or_else(|| anyhow::anyhow!("registry.verbs-for-table: table-name is required"))?;

        let graph = load_affinity_graph(pool).await?;
        let verb_affinities = graph.verbs_for_table(&schema_name, &table_name);

        let verbs: Vec<VerbAffinityRow> = verb_affinities
            .into_iter()
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
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let attr_fqn = get_string_arg(verb_call, "attribute-fqn").ok_or_else(|| {
            anyhow::anyhow!("registry.verbs-for-attribute: attribute-fqn is required")
        })?;

        let graph = load_affinity_graph(pool).await?;
        let verb_affinities = graph.verbs_for_attribute(&attr_fqn);

        let verbs: Vec<VerbAffinityRow> = verb_affinities
            .into_iter()
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
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let verb_fqn = get_string_arg(verb_call, "verb-fqn")
            .ok_or_else(|| anyhow::anyhow!("registry.data-for-verb: verb-fqn is required"))?;
        let depth = get_int_arg(verb_call, "depth")
            .map(|d| d as u32)
            .unwrap_or(1);

        let graph = load_affinity_graph(pool).await?;
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
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let verb_fqn = get_string_arg(verb_call, "verb-fqn")
            .ok_or_else(|| anyhow::anyhow!("registry.adjacent-verbs: verb-fqn is required"))?;

        let graph = load_affinity_graph(pool).await?;
        let adjacent = graph.adjacent_verbs(&verb_fqn);

        let entries: Vec<AdjacentVerbEntry> = adjacent
            .into_iter()
            .map(|(adj_fqn, shared_refs)| AdjacentVerbEntry {
                verb_fqn: adj_fqn,
                shared_data: shared_refs.into_iter().map(|r| r.index_key()).collect(),
            })
            .collect();

        let result = AdjacentVerbsResult {
            verb_fqn,
            adjacent: entries,
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
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let schema_filter = get_string_arg(verb_call, "schema-name");

        // Build the affinity graph
        let graph = load_affinity_graph(pool).await?;

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

        let orphan_tables: Vec<String> = graph
            .orphan_tables(&known_table_refs)
            .into_iter()
            .map(|t| t.key())
            .collect();

        let orphan_verbs = graph.orphan_verbs();
        let write_only = graph.write_only_attributes();
        let read_before_write = graph.read_before_write_attributes();

        let result = GovernanceGapsResult {
            orphan_tables,
            orphan_verbs,
            write_only_attributes: write_only,
            read_before_write_attributes: read_before_write,
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
            "registry.governance-gaps requires database feature"
        ))
    }
}

// ── AffinityDiscoverDslOp ─────────────────────────────────────────────────────

/// Phase 2 stub: synthesis of a verb chain from a natural-language intent.
/// Returns NotImplemented until Phase 2 is built.
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
        "Phase 2 stub: utterance → verb chain synthesis via AffinityGraph (not yet implemented)"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let result = DiscoverDslResult {
            status: "not_implemented".to_owned(),
            message: "registry.discover-dsl is a Phase 2 feature — DSL chain synthesis from utterances is not yet available".to_owned(),
        };
        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        let result = DiscoverDslResult {
            status: "not_implemented".to_owned(),
            message: "registry.discover-dsl is a Phase 2 feature — not yet available".to_owned(),
        };
        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }
}
