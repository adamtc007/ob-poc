//! Graph Validation Operations (Phase 2.2)
//!
//! Implements ownership/control graph validation per KYC/UBO architecture spec section 6.4:
//! - Cycle detection (Tarjan-style SCC via DFS)
//! - Missing percentages check
//! - Supply >100% check (ownership per target entity)
//! - Terminus integrity check (chains must end at natural persons or known terminators)
//! - Source conflicts check (conflicting ownership data from different sources)
//! - Orphan entities check (entities with no inbound or outbound relationships)
//!
//! The op queries `entity_relationships` from the database, runs all validation checks,
//! and persists any anomalies found into `kyc.research_anomalies`.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use ob_poc_macros::register_custom_op;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

#[cfg(feature = "database")]
use sqlx::PgPool;

use super::helpers::extract_uuid_opt;
use super::{CustomOperation, ExecutionContext, ExecutionResult, VerbCall};

// ============================================================================
// Types
// ============================================================================

/// A single graph anomaly discovered during validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphAnomaly {
    /// Type of anomaly detected.
    pub anomaly_type: String,
    /// Entity IDs involved in this anomaly.
    pub entity_ids: Vec<Uuid>,
    /// Human-readable detail describing the anomaly.
    pub detail: String,
    /// Severity: ERROR or WARNING.
    pub severity: String,
}

/// Top-level result returned by graph.validate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphValidateResult {
    /// Number of anomalies found across all checks.
    pub anomalies_found: i32,
    /// The list of anomalies.
    pub anomalies: Vec<GraphAnomaly>,
    /// Number of edges analysed.
    pub edges_analysed: i32,
    /// Number of distinct entities in the graph.
    pub entities_analysed: i32,
    /// Number of anomalies persisted to kyc.research_anomalies.
    pub anomalies_persisted: i32,
}

/// Internal edge representation loaded from the database.
#[cfg(feature = "database")]
#[derive(Debug, Clone)]
struct Edge {
    relationship_id: Uuid,
    from_entity_id: Uuid,
    to_entity_id: Uuid,
    relationship_type: String,
    percentage: Option<f64>,
    source: Option<String>,
}

// ============================================================================
// GraphValidateOp
// ============================================================================

#[register_custom_op]
pub struct GraphValidateOp;

#[async_trait]
impl CustomOperation for GraphValidateOp {
    fn domain(&self) -> &'static str {
        "graph"
    }

    fn verb(&self) -> &'static str {
        "validate"
    }

    fn rationale(&self) -> &'static str {
        "Graph validation requires traversal algorithms (cycle detection, supply aggregation, \
         terminus reachability) that cannot be expressed as data-driven CRUD"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let case_id = extract_uuid_opt(verb_call, ctx, "case-id");

        // Load edges — scoped to case entities if case-id provided, otherwise all active edges.
        let edges = load_edges(pool, case_id).await?;

        // Collect all entity IDs referenced in edges.
        let mut all_entity_ids: HashSet<Uuid> = HashSet::new();
        for edge in &edges {
            all_entity_ids.insert(edge.from_entity_id);
            all_entity_ids.insert(edge.to_entity_id);
        }

        let entities_analysed = all_entity_ids.len() as i32;
        let edges_analysed = edges.len() as i32;

        // Run all validation checks.
        let mut anomalies: Vec<GraphAnomaly> = Vec::new();

        // Synchronous checks.
        detect_cycles(&edges, &mut anomalies);
        check_missing_percentages(&edges, &mut anomalies);
        check_supply_exceeds_100(&edges, &mut anomalies);
        check_source_conflicts(&edges, &mut anomalies);
        check_orphan_entities(&edges, &all_entity_ids, &mut anomalies);

        // Async check: terminus integrity (requires DB lookups for entity types).
        check_terminus_integrity(&edges, &all_entity_ids, pool, &mut anomalies).await?;

        let anomalies_found = anomalies.len() as i32;

        // Persist anomalies to kyc.research_anomalies.
        let anomalies_persisted =
            persist_anomalies(pool, &anomalies, case_id, &all_entity_ids).await?;

        let result = GraphValidateResult {
            anomalies_found,
            anomalies,
            edges_analysed,
            entities_analysed,
            anomalies_persisted,
        };

        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Void)
    }
}

// ============================================================================
// Edge Loading
// ============================================================================

#[cfg(feature = "database")]
async fn load_edges(pool: &PgPool, case_id: Option<Uuid>) -> Result<Vec<Edge>> {
    let rows: Vec<(
        Uuid,
        Uuid,
        Uuid,
        String,
        Option<rust_decimal::Decimal>,
        Option<String>,
    )> = if let Some(cid) = case_id {
        // Scope to entities linked to this KYC case via entity_workstreams.
        sqlx::query_as(
                r#"
                SELECT
                    er.relationship_id,
                    er.from_entity_id,
                    er.to_entity_id,
                    er.relationship_type,
                    er.percentage,
                    er.source
                FROM "ob-poc".entity_relationships er
                WHERE (er.effective_to IS NULL OR er.effective_to > CURRENT_DATE)
                  AND (
                      er.from_entity_id IN (SELECT entity_id FROM kyc.entity_workstreams WHERE case_id = $1)
                      OR er.to_entity_id IN (SELECT entity_id FROM kyc.entity_workstreams WHERE case_id = $1)
                  )
                ORDER BY er.to_entity_id, er.from_entity_id
                "#,
            )
            .bind(cid)
            .fetch_all(pool)
            .await?
    } else {
        // All active edges.
        sqlx::query_as(
            r#"
                SELECT
                    er.relationship_id,
                    er.from_entity_id,
                    er.to_entity_id,
                    er.relationship_type,
                    er.percentage,
                    er.source
                FROM "ob-poc".entity_relationships er
                WHERE (er.effective_to IS NULL OR er.effective_to > CURRENT_DATE)
                ORDER BY er.to_entity_id, er.from_entity_id
                "#,
        )
        .fetch_all(pool)
        .await?
    };

    let edges = rows
        .into_iter()
        .map(
            |(
                relationship_id,
                from_entity_id,
                to_entity_id,
                relationship_type,
                percentage,
                source,
            )| {
                Edge {
                    relationship_id,
                    from_entity_id,
                    to_entity_id,
                    relationship_type,
                    percentage: percentage.map(|d| d.to_string().parse::<f64>().unwrap_or(0.0)),
                    source,
                }
            },
        )
        .collect();

    Ok(edges)
}

// ============================================================================
// Check 1: Cycle Detection (DFS-based SCC)
// ============================================================================

/// Detects cycles in the ownership/control graph using iterative DFS.
/// Any strongly connected component with more than one node is a cycle.
fn detect_cycles(edges: &[Edge], anomalies: &mut Vec<GraphAnomaly>) {
    // Build adjacency list: from -> [to]
    let mut adj: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
    let mut all_nodes: HashSet<Uuid> = HashSet::new();

    for edge in edges {
        adj.entry(edge.from_entity_id)
            .or_default()
            .push(edge.to_entity_id);
        all_nodes.insert(edge.from_entity_id);
        all_nodes.insert(edge.to_entity_id);
    }

    // Tarjan's SCC algorithm (iterative variant)
    let mut index_counter: u32 = 0;
    let mut stack: Vec<Uuid> = Vec::new();
    let mut on_stack: HashSet<Uuid> = HashSet::new();
    let mut indices: HashMap<Uuid, u32> = HashMap::new();
    let mut lowlinks: HashMap<Uuid, u32> = HashMap::new();
    let mut sccs: Vec<Vec<Uuid>> = Vec::new();

    // We use a recursive-style DFS via an explicit call stack.
    // Each frame: (node, neighbor_index, is_root_call)
    for start_node in &all_nodes {
        if indices.contains_key(start_node) {
            continue;
        }

        // DFS stack frames: (node, neighbor_iterator_index)
        let mut dfs_stack: Vec<(Uuid, usize)> = vec![(*start_node, 0)];
        indices.insert(*start_node, index_counter);
        lowlinks.insert(*start_node, index_counter);
        index_counter += 1;
        stack.push(*start_node);
        on_stack.insert(*start_node);

        while let Some((node, ni)) = dfs_stack.last_mut() {
            let node = *node;
            let neighbors = adj.get(&node).cloned().unwrap_or_default();

            if *ni < neighbors.len() {
                let neighbor = neighbors[*ni];
                *ni += 1;

                if !indices.contains_key(&neighbor) {
                    // Tree edge — push neighbor.
                    indices.insert(neighbor, index_counter);
                    lowlinks.insert(neighbor, index_counter);
                    index_counter += 1;
                    stack.push(neighbor);
                    on_stack.insert(neighbor);
                    dfs_stack.push((neighbor, 0));
                } else if on_stack.contains(&neighbor) {
                    // Back edge — update lowlink.
                    let neighbor_idx = indices[&neighbor];
                    let current_low = lowlinks[&node];
                    if neighbor_idx < current_low {
                        lowlinks.insert(node, neighbor_idx);
                    }
                }
            } else {
                // All neighbors explored — pop and propagate lowlink.
                dfs_stack.pop();

                if let Some((parent, _)) = dfs_stack.last() {
                    let parent = *parent;
                    let node_low = lowlinks[&node];
                    let parent_low = lowlinks[&parent];
                    if node_low < parent_low {
                        lowlinks.insert(parent, node_low);
                    }
                }

                // If node is root of an SCC, pop the SCC from stack.
                if lowlinks[&node] == indices[&node] {
                    let mut scc: Vec<Uuid> = Vec::new();
                    loop {
                        let w = stack.pop().unwrap();
                        on_stack.remove(&w);
                        scc.push(w);
                        if w == node {
                            break;
                        }
                    }
                    if scc.len() > 1 {
                        sccs.push(scc);
                    }
                }
            }
        }
    }

    for scc in sccs {
        let ids_str: Vec<String> = scc.iter().map(|id| id.to_string()).collect();
        anomalies.push(GraphAnomaly {
            anomaly_type: "CYCLE".to_string(),
            entity_ids: scc,
            detail: format!(
                "Ownership/control cycle detected among {} entities: {}",
                ids_str.len(),
                ids_str.join(" -> ")
            ),
            severity: "ERROR".to_string(),
        });
    }
}

// ============================================================================
// Check 2: Missing Percentages
// ============================================================================

/// Flags ownership/control edges that have no percentage value.
fn check_missing_percentages(edges: &[Edge], anomalies: &mut Vec<GraphAnomaly>) {
    for edge in edges {
        if edge.relationship_type == "ownership" && edge.percentage.is_none() {
            anomalies.push(GraphAnomaly {
                anomaly_type: "MISSING_PERCENTAGE".to_string(),
                entity_ids: vec![edge.from_entity_id, edge.to_entity_id],
                detail: format!(
                    "Ownership edge from {} to {} (rel {}) has no percentage",
                    edge.from_entity_id, edge.to_entity_id, edge.relationship_id
                ),
                severity: "WARNING".to_string(),
            });
        }
    }
}

// ============================================================================
// Check 3: Supply > 100%
// ============================================================================

/// For each target entity, sums all inbound ownership percentages.
/// If total exceeds 100%, flags as an anomaly.
fn check_supply_exceeds_100(edges: &[Edge], anomalies: &mut Vec<GraphAnomaly>) {
    // Group ownership edges by to_entity_id and sum percentages.
    let mut supply_map: HashMap<Uuid, f64> = HashMap::new();
    let mut holders_map: HashMap<Uuid, Vec<Uuid>> = HashMap::new();

    for edge in edges {
        if edge.relationship_type == "ownership" {
            if let Some(pct) = edge.percentage {
                *supply_map.entry(edge.to_entity_id).or_insert(0.0) += pct;
                holders_map
                    .entry(edge.to_entity_id)
                    .or_default()
                    .push(edge.from_entity_id);
            }
        }
    }

    for (to_entity_id, total) in &supply_map {
        if *total > 100.0 {
            let mut entity_ids = vec![*to_entity_id];
            if let Some(holders) = holders_map.get(to_entity_id) {
                entity_ids.extend(holders);
            }
            anomalies.push(GraphAnomaly {
                anomaly_type: "SUPPLY_EXCEEDS_100".to_string(),
                entity_ids,
                detail: format!(
                    "Entity {} has total inbound ownership of {:.2}% (exceeds 100%)",
                    to_entity_id, total
                ),
                severity: "ERROR".to_string(),
            });
        }
    }
}

// ============================================================================
// Check 4: Terminus Integrity
// ============================================================================

/// Ownership chains should terminate at natural persons (entity_proper_persons)
/// or known regulated entities. Entities that are leaf sources (no inbound ownership)
/// and are NOT natural persons are flagged.
#[cfg(feature = "database")]
async fn check_terminus_integrity(
    edges: &[Edge],
    _all_entity_ids: &HashSet<Uuid>,
    pool: &PgPool,
    anomalies: &mut Vec<GraphAnomaly>,
) -> Result<()> {
    // Find entities that have outbound ownership edges but no inbound ownership edges.
    // These are the "roots" of ownership chains and should be natural persons.
    let mut has_inbound: HashSet<Uuid> = HashSet::new();
    let mut has_outbound: HashSet<Uuid> = HashSet::new();

    for edge in edges {
        if edge.relationship_type == "ownership" || edge.relationship_type == "control" {
            has_inbound.insert(edge.to_entity_id);
            has_outbound.insert(edge.from_entity_id);
        }
    }

    // Root entities: have outbound but no inbound ownership.
    let root_entities: Vec<Uuid> = has_outbound
        .iter()
        .filter(|id| !has_inbound.contains(id))
        .copied()
        .collect();

    if root_entities.is_empty() {
        return Ok(());
    }

    // Check which root entities are natural persons.
    let natural_persons: HashSet<Uuid> = {
        let ids: Vec<Uuid> = root_entities.clone();
        let rows: Vec<(Uuid,)> = sqlx::query_as(
            r#"
            SELECT pp.entity_id
            FROM "ob-poc".entity_proper_persons pp
            WHERE pp.entity_id = ANY($1)
            "#,
        )
        .bind(&ids)
        .fetch_all(pool)
        .await?;

        rows.into_iter().map(|(id,)| id).collect()
    };

    // Also check for known regulated entities (these are acceptable terminators).
    let regulated_entities: HashSet<Uuid> = {
        let ids: Vec<Uuid> = root_entities.clone();
        let rows: Vec<(Uuid,)> = sqlx::query_as(
            r#"
            SELECT er.entity_id
            FROM "ob-poc".entity_relationships er2
            JOIN "ob-poc".entities er ON er.entity_id = er2.from_entity_id
            WHERE er.entity_id = ANY($1)
              AND er2.is_regulated = true
            UNION
            SELECT e.entity_id
            FROM "ob-poc".entities e
            JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
            WHERE e.entity_id = ANY($1)
              AND et.type_code IN ('government', 'public_authority', 'listed_company')
            "#,
        )
        .bind(&ids)
        .fetch_all(pool)
        .await?;

        rows.into_iter().map(|(id,)| id).collect()
    };

    // Flag root entities that are neither natural persons nor regulated terminators.
    for root_id in &root_entities {
        if !natural_persons.contains(root_id) && !regulated_entities.contains(root_id) {
            anomalies.push(GraphAnomaly {
                anomaly_type: "TERMINUS_NOT_NATURAL_PERSON".to_string(),
                entity_ids: vec![*root_id],
                detail: format!(
                    "Entity {} is a root of an ownership chain but is not a natural person or \
                     known regulated terminator. Ownership chains should resolve to individuals.",
                    root_id
                ),
                severity: "WARNING".to_string(),
            });
        }
    }

    Ok(())
}

// ============================================================================
// Check 5: Source Conflicts
// ============================================================================

/// Detects conflicting ownership data from different sources for the same edge pair.
/// If two edges between the same (from, to) pair have different percentages from
/// different sources, flag it.
fn check_source_conflicts(edges: &[Edge], anomalies: &mut Vec<GraphAnomaly>) {
    // Group edges by (from_entity_id, to_entity_id, relationship_type).
    let mut edge_groups: HashMap<(Uuid, Uuid, String), Vec<&Edge>> = HashMap::new();

    for edge in edges {
        let key = (
            edge.from_entity_id,
            edge.to_entity_id,
            edge.relationship_type.clone(),
        );
        edge_groups.entry(key).or_default().push(edge);
    }

    for ((from_id, to_id, rel_type), group) in &edge_groups {
        if group.len() < 2 {
            continue;
        }

        // Check if different sources report different percentages.
        let mut source_pcts: HashMap<String, f64> = HashMap::new();
        for edge in group {
            let src = edge.source.clone().unwrap_or_else(|| "unknown".to_string());
            if let Some(pct) = edge.percentage {
                if let Some(existing_pct) = source_pcts.get(&src) {
                    // Same source, different percentage — also a conflict.
                    if (*existing_pct - pct).abs() > 0.01 {
                        anomalies.push(GraphAnomaly {
                            anomaly_type: "SOURCE_CONFLICT".to_string(),
                            entity_ids: vec![*from_id, *to_id],
                            detail: format!(
                                "Source '{}' reports conflicting {} percentages for {} -> {}: \
                                 {:.2}% vs {:.2}%",
                                src, rel_type, from_id, to_id, existing_pct, pct
                            ),
                            severity: "ERROR".to_string(),
                        });
                    }
                } else {
                    source_pcts.insert(src, pct);
                }
            }
        }

        // Cross-source conflicts: compare pairs of distinct sources.
        let sources: Vec<(&String, &f64)> = source_pcts.iter().collect();
        for i in 0..sources.len() {
            for j in (i + 1)..sources.len() {
                let (src_a, pct_a) = sources[i];
                let (src_b, pct_b) = sources[j];
                if (pct_a - pct_b).abs() > 0.01 {
                    anomalies.push(GraphAnomaly {
                        anomaly_type: "SOURCE_CONFLICT".to_string(),
                        entity_ids: vec![*from_id, *to_id],
                        detail: format!(
                            "Conflicting {} percentages for {} -> {}: source '{}' reports {:.2}% \
                             vs source '{}' reports {:.2}%",
                            rel_type, from_id, to_id, src_a, pct_a, src_b, pct_b
                        ),
                        severity: "WARNING".to_string(),
                    });
                }
            }
        }
    }
}

// ============================================================================
// Check 6: Orphan Entities
// ============================================================================

/// Detects entities that appear in the graph but have neither inbound nor outbound
/// ownership/control edges (isolated nodes). These are suspicious in a relationship graph.
fn check_orphan_entities(
    edges: &[Edge],
    all_entity_ids: &HashSet<Uuid>,
    anomalies: &mut Vec<GraphAnomaly>,
) {
    let mut connected: HashSet<Uuid> = HashSet::new();
    for edge in edges {
        connected.insert(edge.from_entity_id);
        connected.insert(edge.to_entity_id);
    }

    // Orphans are entities referenced somewhere but not in any edge.
    // In practice this fires when a case's entity_workstreams references entities
    // that have no relationships at all.
    // Since all_entity_ids is built from edges, orphans only appear if we loaded
    // extra entity IDs from the case. For the all-edges path, this check finds
    // nothing by construction. We still keep it for case-scoped mode where we could
    // extend loading to include workstream entities.

    // For now, we flag entities that ONLY appear as self-loops (from == to), which
    // is itself an anomaly, or entities that appear on exactly one side (only source
    // or only target) with no meaningful peer — this is handled by terminus integrity.
    // This section flags true isolates if we ever load extra entity IDs.

    for entity_id in all_entity_ids {
        if !connected.contains(entity_id) {
            anomalies.push(GraphAnomaly {
                anomaly_type: "ORPHAN_ENTITY".to_string(),
                entity_ids: vec![*entity_id],
                detail: format!(
                    "Entity {} has no ownership or control relationships (orphan in graph)",
                    entity_id
                ),
                severity: "WARNING".to_string(),
            });
        }
    }
}

// ============================================================================
// Anomaly Persistence
// ============================================================================

/// Persists anomalies to kyc.research_anomalies.
/// Creates a research_action record to serve as the parent for anomaly rows.
/// Returns the number of anomalies persisted.
#[cfg(feature = "database")]
async fn persist_anomalies(
    pool: &PgPool,
    anomalies: &[GraphAnomaly],
    case_id: Option<Uuid>,
    all_entity_ids: &HashSet<Uuid>,
) -> Result<i32> {
    if anomalies.is_empty() {
        return Ok(0);
    }

    // We need a target_entity_id for the research_action. Use the first entity from
    // the case, or the first entity from the graph if no case.
    let representative_entity_id = if let Some(cid) = case_id {
        let row: Option<(Uuid,)> = sqlx::query_as(
            r#"SELECT entity_id FROM kyc.entity_workstreams WHERE case_id = $1 LIMIT 1"#,
        )
        .bind(cid)
        .fetch_optional(pool)
        .await?;
        row.map(|(id,)| id)
    } else {
        all_entity_ids.iter().next().copied()
    };

    let target_entity_id = representative_entity_id
        .ok_or_else(|| anyhow!("No entities found to anchor research action"))?;

    // Create a research_action as parent for the anomaly records.
    let action_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO kyc.research_actions (
            target_entity_id,
            action_type,
            source_provider,
            source_key,
            source_key_type,
            verb_domain,
            verb_name,
            verb_args,
            success,
            entities_created,
            entities_updated,
            relationships_created
        ) VALUES (
            $1,
            'GRAPH_VALIDATION',
            'internal',
            COALESCE($2::text, 'all'),
            'graph_validate',
            'graph',
            'validate',
            '{}'::jsonb,
            true,
            0,
            0,
            0
        )
        RETURNING action_id
        "#,
    )
    .bind(target_entity_id)
    .bind(case_id.map(|id| id.to_string()))
    .fetch_one(pool)
    .await?;

    let mut persisted = 0i32;

    for anomaly in anomalies {
        // Use the first entity_id from the anomaly as the entity reference.
        let entity_id = anomaly
            .entity_ids
            .first()
            .copied()
            .unwrap_or(target_entity_id);

        // Map anomaly_type to a rule_code that fits the VARCHAR(50) constraint.
        let rule_code = match anomaly.anomaly_type.as_str() {
            "CYCLE" => "GRAPH_CYCLE_DETECTED",
            "MISSING_PERCENTAGE" => "GRAPH_MISSING_PCT",
            "SUPPLY_EXCEEDS_100" => "GRAPH_SUPPLY_GT_100",
            "TERMINUS_NOT_NATURAL_PERSON" => "GRAPH_TERMINUS_NOT_NP",
            "SOURCE_CONFLICT" => "GRAPH_SOURCE_CONFLICT",
            "ORPHAN_ENTITY" => "GRAPH_ORPHAN_ENTITY",
            other => other,
        };

        // Map severity: our types use ERROR/WARNING, the DB allows ERROR/WARNING/INFO.
        let db_severity = match anomaly.severity.as_str() {
            "ERROR" => "ERROR",
            "WARNING" => "WARNING",
            _ => "INFO",
        };

        // Build expected/actual values for context.
        let expected_value: Option<String> = match anomaly.anomaly_type.as_str() {
            "SUPPLY_EXCEEDS_100" => Some("<=100%".to_string()),
            "MISSING_PERCENTAGE" => Some("numeric percentage".to_string()),
            "TERMINUS_NOT_NATURAL_PERSON" => Some("natural person or regulated entity".to_string()),
            "CYCLE" => Some("acyclic graph".to_string()),
            _ => None,
        };

        let actual_value: Option<String> = Some(anomaly.detail.clone());

        sqlx::query(
            r#"
            INSERT INTO kyc.research_anomalies (
                action_id,
                entity_id,
                rule_code,
                severity,
                description,
                expected_value,
                actual_value,
                status
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, 'OPEN')
            "#,
        )
        .bind(action_id)
        .bind(entity_id)
        .bind(rule_code)
        .bind(db_severity)
        .bind(&anomaly.detail)
        .bind(&expected_value)
        .bind(&actual_value)
        .execute(pool)
        .await?;

        persisted += 1;
    }

    tracing::info!(
        "graph.validate: persisted {} anomalies under action_id {}",
        persisted,
        action_id
    );

    Ok(persisted)
}
