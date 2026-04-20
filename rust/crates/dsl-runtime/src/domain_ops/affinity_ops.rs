//! Affinity navigation CustomOps — 6 `registry.*` verbs.
//!
//! Implements the YAML contracts in
//! `rust/config/verbs/sem-reg/registry.yaml` for:
//!  - registry.verbs-for-table       (AffinityVerbsForTableOp)
//!  - registry.verbs-for-attribute   (AffinityVerbsForAttributeOp)
//!  - registry.data-for-verb         (AffinityDataForVerbOp)
//!  - registry.adjacent-verbs        (AffinityAdjacentVerbsOp)
//!  - registry.governance-gaps       (AffinityGovernanceGapsOp)
//!  - registry.discover-dsl          (AffinityDiscoverDslOp)
//!
//! All six ops query the active `AffinityGraph` (loaded from
//! `sem_reg.snapshots` via [`affinity_graph_cache`]); the discover-dsl
//! op additionally resolves CCIR context via the
//! [`SemOsContextResolver`] trait to scope candidate verbs.

use anyhow::Result;
use async_trait::async_trait;
use dsl_runtime_macros::register_custom_op;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashSet;
use uuid::Uuid;

use sem_os_core::abac::ActorContext;
use sem_os_core::affinity::{discover_dsl, AffinityKind, DataRef, TableRef};
use sem_os_core::context_resolution::{
    ContextResolutionRequest, DiscoveryContext, EvidenceMode, SubjectRef,
};
use sem_os_core::principal::Principal;
use sem_os_core::types::Classification;
use sem_os_core::verb_contract::VerbContractBody;

use crate::custom_op::CustomOperation;
use crate::domain_ops::affinity_graph_cache::load_affinity_graph_cached;
use crate::domain_ops::helpers::{
    json_extract_int_opt, json_extract_string, json_extract_string_opt,
};
use crate::execution::{VerbExecutionContext, VerbExecutionOutcome};
use crate::service_traits::SemOsContextResolver;

// ── Result types ──────────────────────────────────────────────────────────────

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

// ── Filter enums (parsed from YAML string args) ───────────────────────────────

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

// ── Shared loaders ────────────────────────────────────────────────────────────

async fn load_active_verb_contracts(pool: &PgPool) -> Result<Vec<VerbContractBody>> {
    let rows = sqlx::query_as::<_, (serde_json::Value,)>(
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

fn map_verb_affinity(va: sem_os_core::affinity::VerbAffinity) -> VerbAffinityRow {
    VerbAffinityRow {
        verb_fqn: va.verb_fqn,
        affinity_kind: format!("{:?}", va.affinity_kind),
        provenance: format!("{:?}", va.provenance),
    }
}

fn map_data_affinity(da: sem_os_core::affinity::DataAffinity) -> DataAffinityRow {
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
}

// ── registry.verbs-for-table ──────────────────────────────────────────────────

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

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let schema_name = json_extract_string(args, "schema-name")?;
        let table_name = json_extract_string(args, "table-name")?;
        let include_lookups =
            crate::domain_ops::helpers::json_extract_bool_opt(args, "include-lookups")
                .unwrap_or(true);

        let graph = load_affinity_graph_cached(pool).await?;
        let verbs = graph
            .verbs_for_table(&schema_name, &table_name)
            .into_iter()
            .filter(|va| {
                include_lookups || !matches!(va.affinity_kind, AffinityKind::ArgLookup { .. })
            })
            .map(map_verb_affinity)
            .collect();

        Ok(VerbExecutionOutcome::Record(serde_json::to_value(
            VerbsForTableResult {
                schema_name,
                table_name,
                verbs,
            },
        )?))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

// ── registry.verbs-for-attribute ──────────────────────────────────────────────

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

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        // YAML contract names the arg `attribute-fqn`; legacy callers used `attr-fqn`.
        let attribute_fqn = json_extract_string(args, "attribute-fqn")
            .or_else(|_| json_extract_string(args, "attr-fqn"))?;
        let direction = AttributeDirection::from_arg(json_extract_string_opt(args, "direction"))?;

        let graph = load_affinity_graph_cached(pool).await?;
        let verbs = graph
            .verbs_for_attribute(&attribute_fqn)
            .into_iter()
            .filter(|va| direction.include(&va.affinity_kind))
            .map(map_verb_affinity)
            .collect();

        Ok(VerbExecutionOutcome::Record(serde_json::to_value(
            VerbsForAttributeResult {
                attribute_fqn,
                verbs,
            },
        )?))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

// ── registry.data-for-verb ────────────────────────────────────────────────────

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

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let verb_fqn = json_extract_string(args, "verb-fqn")?;
        let depth = json_extract_int_opt(args, "depth")
            .map(|d| d as u32)
            .unwrap_or(1);

        let graph = load_affinity_graph_cached(pool).await?;
        let data_assets = graph
            .data_for_verb(&verb_fqn)
            .into_iter()
            .map(map_data_affinity)
            .collect();

        let footprint = (depth > 1).then(|| {
            let fp = graph.data_footprint(&verb_fqn, depth);
            DataFootprintSummary {
                tables: fp.tables.values().map(|t| t.key()).collect(),
                columns: fp.columns.keys().cloned().collect(),
                attributes: fp.attributes.into_iter().collect(),
                entity_types: fp.entity_types.into_iter().collect(),
            }
        });

        Ok(VerbExecutionOutcome::Record(serde_json::to_value(
            DataForVerbResult {
                verb_fqn,
                depth,
                data_assets,
                footprint,
            },
        )?))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

// ── registry.adjacent-verbs ───────────────────────────────────────────────────

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

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let verb_fqn = json_extract_string(args, "verb-fqn")?;
        let min_overlap = json_extract_int_opt(args, "min-overlap")
            .unwrap_or(1)
            .max(1) as usize;

        let graph = load_affinity_graph_cached(pool).await?;
        let adjacent = graph
            .adjacent_verbs(&verb_fqn)
            .into_iter()
            .map(|(adj_fqn, shared_refs)| AdjacentVerbEntry {
                verb_fqn: adj_fqn,
                shared_data: shared_refs.into_iter().map(|r| r.index_key()).collect(),
            })
            .filter(|row| row.shared_data.len() >= min_overlap)
            .collect();

        Ok(VerbExecutionOutcome::Record(serde_json::to_value(
            AdjacentVerbsResult {
                verb_fqn,
                adjacent,
            },
        )?))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

// ── registry.governance-gaps ──────────────────────────────────────────────────

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

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let schema_filter = json_extract_string_opt(args, "schema-name");
        let gap_type = GapType::from_arg(json_extract_string_opt(args, "gap-type"))?;

        let graph = load_affinity_graph_cached(pool).await?;

        // Pull the physical table list from information_schema for orphan-table detection.
        let known_tables: Vec<(String, String)> = match schema_filter.as_deref() {
            Some(schema) => {
                sqlx::query_as(
                    "SELECT table_schema, table_name FROM information_schema.tables \
                     WHERE table_type = 'BASE TABLE' AND table_schema = $1 \
                     ORDER BY table_schema, table_name",
                )
                .bind(schema)
                .fetch_all(pool)
                .await?
            }
            None => {
                sqlx::query_as(
                    "SELECT table_schema, table_name FROM information_schema.tables \
                     WHERE table_type = 'BASE TABLE' \
                       AND table_schema NOT IN ('pg_catalog','information_schema','pg_toast') \
                     ORDER BY table_schema, table_name",
                )
                .fetch_all(pool)
                .await?
            }
        };
        let known_table_refs: Vec<TableRef> = known_tables
            .into_iter()
            .map(|(s, t)| TableRef::new(s, t))
            .collect();

        let result = GovernanceGapsResult {
            orphan_tables: gap_includes(&gap_type, GapType::OrphanTables).then(|| {
                graph
                    .orphan_tables(&known_table_refs)
                    .into_iter()
                    .map(|t| t.key())
                    .collect()
            }).unwrap_or_default(),
            orphan_verbs: gap_includes(&gap_type, GapType::OrphanVerbs)
                .then(|| graph.orphan_verbs())
                .unwrap_or_default(),
            write_only_attributes: gap_includes(&gap_type, GapType::WriteOnly)
                .then(|| graph.write_only_attributes())
                .unwrap_or_default(),
            read_before_write_attributes: gap_includes(&gap_type, GapType::ReadBeforeWrite)
                .then(|| graph.read_before_write_attributes())
                .unwrap_or_default(),
        };

        Ok(VerbExecutionOutcome::Record(serde_json::to_value(result)?))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

fn gap_includes(filter: &GapType, kind: GapType) -> bool {
    matches!(filter, GapType::All) || *filter == kind
}

// ── registry.discover-dsl ─────────────────────────────────────────────────────

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

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        // YAML contract names the arg `utterance`; legacy callers used `intent`.
        let utterance = json_extract_string(args, "utterance")
            .or_else(|_| json_extract_string(args, "intent"))?;
        let max_chain_length = json_extract_int_opt(args, "max-chain-length")
            .map(|v| v.max(1) as usize)
            .unwrap_or(5);
        let subject_uuid = json_extract_string_opt(args, "subject-id")
            .as_deref()
            .map(Uuid::parse_str)
            .transpose()
            .map_err(|e| anyhow::anyhow!("invalid subject-id: {e}"))?;

        let graph = load_affinity_graph_cached(pool).await?;
        let verb_contracts = load_active_verb_contracts(pool).await?;

        let (allowed_verbs, policy_check) = match subject_uuid {
            Some(subject_id) => resolve_ccir_scope(ctx, subject_id, &utterance).await,
            None => (None, None),
        };

        let result = discover_dsl(
            &utterance,
            &graph,
            &verb_contracts,
            subject_uuid,
            max_chain_length,
            allowed_verbs.as_ref(),
            policy_check,
        );
        Ok(VerbExecutionOutcome::Record(serde_json::to_value(result)?))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

/// Resolve the CCIR-allowed verb set for a subject, using the
/// `SemOsContextResolver` trait. Returns `(allowed_verbs, policy_check_marker)`.
async fn resolve_ccir_scope(
    ctx: &VerbExecutionContext,
    subject_id: Uuid,
    utterance: &str,
) -> (Option<HashSet<String>>, Option<String>) {
    let actor = ActorContext {
        actor_id: ctx.principal.actor_id.clone(),
        roles: vec!["analyst".to_owned()],
        department: Some("registry".to_owned()),
        clearance: Some(Classification::Internal),
        jurisdictions: vec![],
    };
    let request = ContextResolutionRequest {
        subject: SubjectRef::EntityId(subject_id),
        intent_summary: None,
        raw_utterance: Some(utterance.to_owned()),
        actor,
        goals: vec!["dsl_discovery".to_owned()],
        constraints: Default::default(),
        evidence_mode: EvidenceMode::Normal,
        point_in_time: None,
        entity_kind: None,
        entity_confidence: None,
        discovery: DiscoveryContext::default(),
    };

    let resolver = match ctx.service::<dyn SemOsContextResolver>() {
        Ok(r) => r,
        Err(e) => return (None, Some(format!("ccir_unavailable:{e}"))),
    };
    let principal = Principal::in_process(&ctx.principal.actor_id, vec!["analyst".to_owned()]);
    match resolver.resolve_context(&principal, request).await {
        Ok(resp) => {
            let allowed: HashSet<String> =
                resp.candidate_verbs.into_iter().map(|v| v.fqn).collect();
            let marker = format!(
                "ccir_applied:candidates={},confidence={:.2}",
                allowed.len(),
                resp.confidence
            );
            (Some(allowed), Some(marker))
        }
        Err(err) => (None, Some(format!("ccir_failed:{err}"))),
    }
}
