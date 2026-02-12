//! Skeleton Build Pipeline Operations
//!
//! Orchestrates the full KYC skeleton build: import-run begin → graph validate →
//! UBO compute chains → coverage compute → outreach plan generate → tollgate evaluate →
//! import-run complete. Each step performs real computation matching the logic in the
//! corresponding individual ops (graph_validate_ops, ubo_compute_ops, coverage_compute_ops,
//! outreach_plan_ops, tollgate_evaluate_ops).

use anyhow::Result;
use async_trait::async_trait;
use ob_poc_macros::register_custom_op;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};

use super::helpers::{extract_string_opt, extract_uuid};
use super::CustomOperation;

#[cfg(feature = "database")]
use sqlx::PgPool;

#[cfg(feature = "database")]
use std::collections::{HashMap, HashSet};

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkeletonBuildResult {
    pub case_id: Uuid,
    pub import_run_id: Uuid,
    pub determination_run_id: Uuid,
    pub anomalies_found: i64,
    pub ubo_candidates_found: i64,
    pub coverage_pct: f64,
    pub outreach_plan_id: Option<Uuid>,
    pub skeleton_ready: bool,
    pub steps_completed: Vec<String>,
}

// ============================================================================
// SkeletonBuildOp
// ============================================================================

#[register_custom_op]
pub struct SkeletonBuildOp;

#[async_trait]
impl CustomOperation for SkeletonBuildOp {
    fn domain(&self) -> &'static str {
        "skeleton"
    }
    fn verb(&self) -> &'static str {
        "build"
    }
    fn rationale(&self) -> &'static str {
        "Orchestrates the full skeleton build pipeline across 7 steps"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let case_id = extract_uuid(verb_call, ctx, "case-id")?;
        let source =
            extract_string_opt(verb_call, "source").unwrap_or_else(|| "MANUAL".to_string());
        let threshold: f64 = extract_string_opt(verb_call, "threshold")
            .and_then(|s| s.parse().ok())
            .unwrap_or(5.0);

        let mut steps_completed = Vec::new();

        // Step 1: Begin import run
        let run_id = Uuid::new_v4();
        sqlx::query(
            r#"INSERT INTO "ob-poc".graph_import_runs
               (run_id, run_kind, source, scope_root_entity_id, status, started_at)
               SELECT $1, 'SKELETON_BUILD', $2, c.client_group_id, 'ACTIVE', NOW()
               FROM kyc.cases c WHERE c.case_id = $3"#,
        )
        .bind(run_id)
        .bind(&source)
        .bind(case_id)
        .execute(pool)
        .await?;

        // Link import run to case
        sqlx::query(
            r#"INSERT INTO kyc.case_import_runs (case_id, run_id)
               VALUES ($1, $2)
               ON CONFLICT DO NOTHING"#,
        )
        .bind(case_id)
        .bind(run_id)
        .execute(pool)
        .await?;
        steps_completed.push("import-run.begin".to_string());

        // Step 2: Graph validate — real cycle detection, supply checks, anomaly persistence
        let anomalies_found = run_graph_validate(pool, case_id).await?;
        steps_completed.push("graph.validate".to_string());

        // Step 3: UBO compute chains — real DFS chain traversal with percentage multiplication
        let (determination_run_id, ubo_candidates_found) =
            run_ubo_compute(pool, case_id, threshold).await?;
        steps_completed.push("ubo.compute-chains".to_string());

        // Step 4: Coverage compute — real 4-prong coverage checks
        let coverage_pct = run_coverage_compute(pool, case_id, determination_run_id).await?;
        steps_completed.push("coverage.compute".to_string());

        // Step 5: Outreach plan generate — real gap-to-doc mapping
        let outreach_plan_id = run_outreach_plan(pool, case_id, determination_run_id).await?;
        steps_completed.push("outreach.plan-generate".to_string());

        // Step 6: Tollgate evaluate (SKELETON_READY) — real gate evaluation
        let skeleton_ready = run_tollgate_evaluate(pool, case_id).await?;
        steps_completed.push("tollgate.evaluate-gate".to_string());

        // Step 7: Complete import run
        sqlx::query(
            r#"UPDATE "ob-poc".graph_import_runs
               SET status = 'COMPLETED', completed_at = NOW()
               WHERE run_id = $1"#,
        )
        .bind(run_id)
        .execute(pool)
        .await?;
        steps_completed.push("import-run.complete".to_string());

        let result = SkeletonBuildResult {
            case_id,
            import_run_id: run_id,
            determination_run_id,
            anomalies_found,
            ubo_candidates_found,
            coverage_pct,
            outreach_plan_id,
            skeleton_ready,
            steps_completed,
        };

        // Bind the result UUID so downstream can reference @skeleton
        ctx.bind("run", run_id);

        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }
}

// ============================================================================
// Step 2: Graph Validate — Cycle detection, supply checks, anomaly persistence
//
// Mirrors the logic in graph_validate_ops.rs:
//   1. Load edges scoped to case entities
//   2. Tarjan-style cycle detection (DFS SCC)
//   3. Missing percentage check on ownership edges
//   4. Supply >100% check per target entity
//   5. Source conflict check (different sources, different percentages)
//   6. Persist anomalies to kyc.research_anomalies
// ============================================================================

/// Internal edge representation for graph validation.
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

/// A single graph anomaly.
#[cfg(feature = "database")]
#[derive(Debug, Clone)]
struct GraphAnomaly {
    anomaly_type: String,
    entity_ids: Vec<Uuid>,
    detail: String,
    severity: String,
}

#[cfg(feature = "database")]
async fn run_graph_validate(pool: &PgPool, case_id: Uuid) -> Result<i64> {
    // 1. Load edges scoped to this case's entity workstreams
    let rows: Vec<(Uuid, Uuid, Uuid, String, Option<rust_decimal::Decimal>, Option<String>)> =
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
        .bind(case_id)
        .fetch_all(pool)
        .await?;

    let edges: Vec<Edge> = rows
        .into_iter()
        .map(|(rid, from_id, to_id, rel_type, pct, src)| Edge {
            relationship_id: rid,
            from_entity_id: from_id,
            to_entity_id: to_id,
            relationship_type: rel_type,
            percentage: pct.map(|d| d.to_string().parse::<f64>().unwrap_or(0.0)),
            source: src,
        })
        .collect();

    let mut all_entity_ids: HashSet<Uuid> = HashSet::new();
    for edge in &edges {
        all_entity_ids.insert(edge.from_entity_id);
        all_entity_ids.insert(edge.to_entity_id);
    }

    let mut anomalies: Vec<GraphAnomaly> = Vec::new();

    // 2. Cycle detection (Tarjan SCC)
    detect_cycles(&edges, &mut anomalies);

    // 3. Missing percentages
    check_missing_percentages(&edges, &mut anomalies);

    // 4. Supply >100%
    check_supply_exceeds_100(&edges, &mut anomalies);

    // 5. Source conflicts
    check_source_conflicts(&edges, &mut anomalies);

    let anomalies_found = anomalies.len() as i64;

    // 6. Persist anomalies
    if !anomalies.is_empty() {
        let representative_entity_id: Option<Uuid> = {
            let row: Option<(Uuid,)> = sqlx::query_as(
                r#"SELECT entity_id FROM kyc.entity_workstreams WHERE case_id = $1 LIMIT 1"#,
            )
            .bind(case_id)
            .fetch_optional(pool)
            .await?;
            row.map(|(id,)| id)
        };

        if let Some(target_entity_id) = representative_entity_id {
            let action_id: Uuid = sqlx::query_scalar(
                r#"
                INSERT INTO kyc.research_actions (
                    target_entity_id, action_type, source_provider, source_key,
                    source_key_type, verb_domain, verb_name, verb_args,
                    success, entities_created, entities_updated, relationships_created
                ) VALUES (
                    $1, 'GRAPH_VALIDATION', 'internal', $2::text,
                    'skeleton_build', 'skeleton', 'build', '{}'::jsonb,
                    true, 0, 0, 0
                )
                RETURNING action_id
                "#,
            )
            .bind(target_entity_id)
            .bind(case_id.to_string())
            .fetch_one(pool)
            .await?;

            for anomaly in &anomalies {
                let entity_id = anomaly
                    .entity_ids
                    .first()
                    .copied()
                    .unwrap_or(target_entity_id);

                let rule_code = match anomaly.anomaly_type.as_str() {
                    "CYCLE" => "GRAPH_CYCLE_DETECTED",
                    "MISSING_PERCENTAGE" => "GRAPH_MISSING_PCT",
                    "SUPPLY_EXCEEDS_100" => "GRAPH_SUPPLY_GT_100",
                    "SOURCE_CONFLICT" => "GRAPH_SOURCE_CONFLICT",
                    other => other,
                };

                let db_severity = match anomaly.severity.as_str() {
                    "ERROR" => "ERROR",
                    "WARNING" => "WARNING",
                    _ => "INFO",
                };

                sqlx::query(
                    r#"
                    INSERT INTO kyc.research_anomalies (
                        action_id, entity_id, rule_code, severity,
                        description, status
                    ) VALUES ($1, $2, $3, $4, $5, 'OPEN')
                    "#,
                )
                .bind(action_id)
                .bind(entity_id)
                .bind(rule_code)
                .bind(db_severity)
                .bind(&anomaly.detail)
                .execute(pool)
                .await?;
            }
        }
    }

    tracing::info!(
        "skeleton.build step 2: graph.validate found {} anomalies across {} edges for case {}",
        anomalies_found,
        edges.len(),
        case_id
    );

    Ok(anomalies_found)
}

/// Cycle detection via iterative Tarjan SCC algorithm.
/// SCCs with more than one node are cycles.
#[cfg(feature = "database")]
fn detect_cycles(edges: &[Edge], anomalies: &mut Vec<GraphAnomaly>) {
    let mut adj: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
    let mut all_nodes: HashSet<Uuid> = HashSet::new();

    for edge in edges {
        adj.entry(edge.from_entity_id)
            .or_default()
            .push(edge.to_entity_id);
        all_nodes.insert(edge.from_entity_id);
        all_nodes.insert(edge.to_entity_id);
    }

    let mut index_counter: u32 = 0;
    let mut stack: Vec<Uuid> = Vec::new();
    let mut on_stack: HashSet<Uuid> = HashSet::new();
    let mut indices: HashMap<Uuid, u32> = HashMap::new();
    let mut lowlinks: HashMap<Uuid, u32> = HashMap::new();

    for start_node in &all_nodes {
        if indices.contains_key(start_node) {
            continue;
        }

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
                    indices.insert(neighbor, index_counter);
                    lowlinks.insert(neighbor, index_counter);
                    index_counter += 1;
                    stack.push(neighbor);
                    on_stack.insert(neighbor);
                    dfs_stack.push((neighbor, 0));
                } else if on_stack.contains(&neighbor) {
                    let neighbor_idx = indices[&neighbor];
                    let current_low = lowlinks[&node];
                    if neighbor_idx < current_low {
                        lowlinks.insert(node, neighbor_idx);
                    }
                }
            } else {
                dfs_stack.pop();

                if let Some((parent, _)) = dfs_stack.last() {
                    let parent = *parent;
                    let node_low = lowlinks[&node];
                    let parent_low = lowlinks[&parent];
                    if node_low < parent_low {
                        lowlinks.insert(parent, node_low);
                    }
                }

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
            }
        }
    }
}

/// Flags ownership edges that have no percentage value.
#[cfg(feature = "database")]
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

/// For each target entity, sums inbound ownership percentages and flags >100%.
#[cfg(feature = "database")]
fn check_supply_exceeds_100(edges: &[Edge], anomalies: &mut Vec<GraphAnomaly>) {
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

/// Detects conflicting ownership data from different sources.
#[cfg(feature = "database")]
fn check_source_conflicts(edges: &[Edge], anomalies: &mut Vec<GraphAnomaly>) {
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

        let mut source_pcts: HashMap<String, f64> = HashMap::new();
        for edge in group {
            let src = edge.source.clone().unwrap_or_else(|| "unknown".to_string());
            if let Some(pct) = edge.percentage {
                if let Some(existing_pct) = source_pcts.get(&src) {
                    if (*existing_pct - pct).abs() > 0.01 {
                        anomalies.push(GraphAnomaly {
                            anomaly_type: "SOURCE_CONFLICT".to_string(),
                            entity_ids: vec![*from_id, *to_id],
                            detail: format!(
                                "Source '{}' reports conflicting {} percentages for {} -> {}: {:.2}% vs {:.2}%",
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

        // Cross-source conflicts
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
                            "Conflicting {} percentages for {} -> {}: source '{}' reports {:.2}% vs source '{}' reports {:.2}%",
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
// Step 3: UBO Compute Chains — DFS traversal with percentage multiplication
//
// Mirrors the logic in ubo_compute_ops.rs:
//   1. Load subject entities from entity_workstreams
//   2. Load all active ownership edges
//   3. Build upward adjacency list
//   4. DFS with cycle detection and depth guard (20 hops)
//   5. Percentage multiplication along chains
//   6. Threshold filter
//   7. Persist to ubo_determination_runs with JSONB snapshots
// ============================================================================

#[cfg(feature = "database")]
async fn run_ubo_compute(pool: &PgPool, case_id: Uuid, threshold: f64) -> Result<(Uuid, i64)> {
    let start = std::time::Instant::now();

    // 1. Load subject entities
    let subject_entities: Vec<(Uuid,)> =
        sqlx::query_as(r#"SELECT entity_id FROM kyc.entity_workstreams WHERE case_id = $1"#)
            .bind(case_id)
            .fetch_all(pool)
            .await?;

    let subject_entity_ids: Vec<Uuid> = subject_entities.iter().map(|(eid,)| *eid).collect();

    if subject_entity_ids.is_empty() {
        // No workstream entities — create an empty determination run
        let run_id: Uuid = sqlx::query_scalar(
            r#"INSERT INTO kyc.ubo_determination_runs (
                   subject_entity_id, case_id, as_of, config_version, threshold_pct,
                   candidates_found, output_snapshot, chains_snapshot,
                   computed_at, computed_by, computation_ms
               ) VALUES (
                   '00000000-0000-0000-0000-000000000000', $1, CURRENT_DATE, 'v1.0', $2,
                   0, '[]'::jsonb, '[]'::jsonb, NOW(), 'skeleton.build', 0
               )
               RETURNING run_id"#,
        )
        .bind(case_id)
        .bind(
            rust_decimal::Decimal::from_f64_retain(threshold)
                .unwrap_or_else(|| rust_decimal::Decimal::new(500, 2)),
        )
        .fetch_one(pool)
        .await?;

        return Ok((run_id, 0));
    }

    // 2. Load all active ownership edges
    let edge_rows: Vec<(Uuid, Uuid, Option<rust_decimal::Decimal>)> = sqlx::query_as(
        r#"SELECT from_entity_id, to_entity_id, percentage
           FROM "ob-poc".entity_relationships
           WHERE relationship_type IN ('ownership', 'OWNERSHIP')
             AND (effective_to IS NULL OR effective_to > CURRENT_DATE)
           ORDER BY from_entity_id"#,
    )
    .fetch_all(pool)
    .await?;

    // 3. Build upward adjacency list: to_entity_id → Vec<(from_entity_id, pct)>
    let mut upward_adj: HashMap<Uuid, Vec<(Uuid, f64)>> = HashMap::new();
    for (from_id, to_id, pct) in &edge_rows {
        let pct_val = pct
            .map(|d| d.to_string().parse::<f64>().unwrap_or(0.0))
            .unwrap_or(0.0);
        upward_adj
            .entry(*to_id)
            .or_default()
            .push((*from_id, pct_val));
    }

    // 4. Load entity metadata for terminus detection
    let mut all_entity_ids: HashSet<Uuid> = HashSet::new();
    for (from_id, to_id, _) in &edge_rows {
        all_entity_ids.insert(*from_id);
        all_entity_ids.insert(*to_id);
    }
    for eid in &subject_entity_ids {
        all_entity_ids.insert(*eid);
    }

    let entity_id_vec: Vec<Uuid> = all_entity_ids.into_iter().collect();
    let entity_meta_rows: Vec<(Uuid, Option<String>, String)> = sqlx::query_as(
        r#"SELECT e.entity_id, e.name, et.entity_category
           FROM "ob-poc".entities e
           JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
           WHERE e.entity_id = ANY($1)"#,
    )
    .bind(&entity_id_vec)
    .fetch_all(pool)
    .await?;

    let entity_meta: HashMap<Uuid, (Option<String>, bool)> = entity_meta_rows
        .into_iter()
        .map(|(eid, name, category)| (eid, (name, category == "PERSON")))
        .collect();

    // 5. DFS chain traversal per subject entity
    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct ChainCandidate {
        entity_id: Uuid,
        entity_name: Option<String>,
        total_ownership_pct: f64,
        chain_count: i32,
        is_terminus: bool,
        chains: Vec<ChainPath>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct ChainPath {
        path: Vec<Uuid>,
        effective_pct: f64,
    }

    let mut all_candidates: Vec<ChainCandidate> = Vec::new();
    let mut total_chains = 0i32;

    for subject_entity_id in &subject_entity_ids {
        let mut owner_chains: HashMap<Uuid, Vec<ChainPath>> = HashMap::new();

        // DFS: (current_entity, path, cumulative_pct)
        let mut dfs_stack: Vec<(Uuid, Vec<Uuid>, f64)> = Vec::new();

        if let Some(owners) = upward_adj.get(subject_entity_id) {
            for (owner_id, pct) in owners {
                dfs_stack.push((*owner_id, vec![*subject_entity_id, *owner_id], *pct));
            }
        }

        while let Some((current, path, cumulative_pct)) = dfs_stack.pop() {
            let meta = entity_meta.get(&current);
            let is_natural_person = meta.map(|(_, is_np)| *is_np).unwrap_or(false);
            let has_further_owners = upward_adj.contains_key(&current);
            let is_terminus = is_natural_person || !has_further_owners;

            if is_terminus {
                owner_chains.entry(current).or_default().push(ChainPath {
                    path,
                    effective_pct: cumulative_pct,
                });
                continue;
            }

            if let Some(owners) = upward_adj.get(&current) {
                for (owner_id, pct) in owners {
                    // Cycle detection
                    if path.contains(owner_id) {
                        let mut cycle_path = path.clone();
                        cycle_path.push(*owner_id);
                        owner_chains.entry(*owner_id).or_default().push(ChainPath {
                            path: cycle_path,
                            effective_pct: 0.0,
                        });
                        continue;
                    }

                    // Depth guard: max 20 hops
                    if path.len() >= 20 {
                        owner_chains.entry(current).or_default().push(ChainPath {
                            path: path.clone(),
                            effective_pct: cumulative_pct,
                        });
                        continue;
                    }

                    let mut new_path = path.clone();
                    new_path.push(*owner_id);
                    let new_pct = cumulative_pct * pct / 100.0;
                    dfs_stack.push((*owner_id, new_path, new_pct));
                }
            }
        }

        // 6. Aggregate chains and apply threshold filter
        for (owner_id, chains) in owner_chains {
            let total_pct: f64 = chains.iter().map(|c| c.effective_pct).sum();
            let chain_count = chains.len() as i32;
            total_chains += chain_count;

            let has_cycle = chains
                .iter()
                .any(|c| c.effective_pct == 0.0 && c.path.len() > 2);
            if total_pct < threshold && !has_cycle {
                continue;
            }

            let meta = entity_meta.get(&owner_id);
            all_candidates.push(ChainCandidate {
                entity_id: owner_id,
                entity_name: meta.and_then(|(name, _)| name.clone()),
                total_ownership_pct: total_pct,
                chain_count,
                is_terminus: meta.map(|(_, is_np)| *is_np).unwrap_or(false),
                chains,
            });
        }
    }

    // Sort by ownership descending
    all_candidates.sort_by(|a, b| {
        b.total_ownership_pct
            .partial_cmp(&a.total_ownership_pct)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let candidates_found = all_candidates.len() as i32;
    let computation_ms = start.elapsed().as_millis() as i32;

    // 7. Build JSONB snapshots and persist
    let output_snapshot = serde_json::to_value(&all_candidates)?;
    let chains_snapshot = serde_json::to_value(
        &all_candidates
            .iter()
            .flat_map(|c| {
                c.chains.iter().map(move |chain| {
                    serde_json::json!({
                        "owner_entity_id": c.entity_id,
                        "path": chain.path,
                        "effective_pct": chain.effective_pct
                    })
                })
            })
            .collect::<Vec<_>>(),
    )?;

    let primary_subject = subject_entity_ids.first().copied().unwrap_or(Uuid::nil());

    let run_id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO kyc.ubo_determination_runs (
               subject_entity_id, case_id, as_of, config_version, threshold_pct,
               candidates_found, output_snapshot, chains_snapshot,
               computed_at, computed_by, computation_ms
           ) VALUES ($1, $2, CURRENT_DATE, 'v1.0', $3, $4, $5, $6, NOW(), 'skeleton.build', $7)
           RETURNING run_id"#,
    )
    .bind(primary_subject)
    .bind(case_id)
    .bind(
        rust_decimal::Decimal::from_f64_retain(threshold)
            .unwrap_or_else(|| rust_decimal::Decimal::new(500, 2)),
    )
    .bind(candidates_found)
    .bind(&output_snapshot)
    .bind(&chains_snapshot)
    .bind(computation_ms)
    .fetch_one(pool)
    .await?;

    tracing::info!(
        "skeleton.build step 3: ubo.compute-chains case={} run={} candidates={} chains={} in {}ms",
        case_id,
        run_id,
        candidates_found,
        total_chains,
        computation_ms
    );

    Ok((run_id, candidates_found as i64))
}

// ============================================================================
// Step 4: Coverage Compute — 4-prong coverage checks
//
// Mirrors coverage_compute_ops.rs:
//   1. Load candidates from determination run output_snapshot
//   2. Check OWNERSHIP prong (ownership edges with percentages)
//   3. Check IDENTITY prong (verified evidence or workstream flag)
//   4. Check CONTROL prong (control edges documented)
//   5. Check SOURCE_OF_WEALTH prong (SOW evidence)
//   6. Build prong summaries, compute overall coverage
//   7. Persist coverage_snapshot to determination run
// ============================================================================

#[cfg(feature = "database")]
async fn run_coverage_compute(
    pool: &PgPool,
    case_id: Uuid,
    determination_run_id: Uuid,
) -> Result<f64> {
    // 1. Load candidates from determination run's output_snapshot
    let run_row: Option<(serde_json::Value,)> = sqlx::query_as(
        r#"SELECT output_snapshot FROM kyc.ubo_determination_runs
           WHERE run_id = $1 AND case_id = $2"#,
    )
    .bind(determination_run_id)
    .bind(case_id)
    .fetch_optional(pool)
    .await?;

    let output_snapshot = match run_row {
        Some((snap,)) => snap,
        None => {
            // No determination run found — write 0% coverage
            sqlx::query(
                r#"UPDATE kyc.ubo_determination_runs
                   SET coverage_snapshot = $2
                   WHERE run_id = $1"#,
            )
            .bind(determination_run_id)
            .bind(serde_json::json!({"overall_coverage_pct": 0.0, "gaps": []}))
            .execute(pool)
            .await?;
            return Ok(0.0);
        }
    };

    // Extract candidate entity IDs from the output_snapshot
    let candidate_entity_ids: Vec<Uuid> = extract_candidate_entity_ids(&output_snapshot);

    if candidate_entity_ids.is_empty() {
        let snapshot = serde_json::json!({"overall_coverage_pct": 100.0, "gaps": []});
        sqlx::query(
            r#"UPDATE kyc.ubo_determination_runs SET coverage_snapshot = $2 WHERE run_id = $1"#,
        )
        .bind(determination_run_id)
        .bind(&snapshot)
        .execute(pool)
        .await?;
        return Ok(100.0);
    }

    // 2-5. Check coverage across 4 prongs for each candidate
    let prongs = ["OWNERSHIP", "IDENTITY", "CONTROL", "SOURCE_OF_WEALTH"];
    let mut prong_covered: HashMap<&str, (i32, i32)> = HashMap::new(); // (covered, total)
    for p in &prongs {
        prong_covered.insert(p, (0, 0));
    }

    let mut gaps: Vec<serde_json::Value> = Vec::new();

    for entity_id in &candidate_entity_ids {
        // OWNERSHIP: check if entity has ownership edges with percentages
        let ownership_count: (i64,) = sqlx::query_as(
            r#"SELECT COUNT(*) FROM "ob-poc".entity_relationships
               WHERE from_entity_id = $1
                 AND relationship_type = 'ownership'
                 AND percentage IS NOT NULL
                 AND (effective_to IS NULL OR effective_to > CURRENT_DATE)"#,
        )
        .bind(entity_id)
        .fetch_one(pool)
        .await
        .unwrap_or((0,));

        let ownership_covered = ownership_count.0 > 0;
        update_prong(&mut prong_covered, "OWNERSHIP", ownership_covered);
        if !ownership_covered {
            gaps.push(serde_json::json!({
                "gap_id": format!("{}:OWNERSHIP", entity_id),
                "prong": "OWNERSHIP",
                "entity_id": entity_id.to_string(),
                "description": format!("Missing ownership edges with percentages for {}", entity_id),
                "blocking_at_gate": "SKELETON_READY"
            }));
        }

        // IDENTITY: check verified evidence or workstream flag
        let identity_count: (i64,) = sqlx::query_as(
            r#"SELECT COUNT(*) FROM kyc.ubo_evidence ue
               JOIN kyc.ubo_registry ur ON ur.ubo_id = ue.ubo_id
               WHERE ur.ubo_person_id = $1 AND ur.case_id = $2
                 AND ue.evidence_type IN ('IDENTITY_DOC', 'PROOF_OF_ADDRESS')
                 AND ue.status = 'VERIFIED'"#,
        )
        .bind(entity_id)
        .bind(case_id)
        .fetch_one(pool)
        .await
        .unwrap_or((0,));

        let ws_verified: Option<(bool,)> = sqlx::query_as(
            r#"SELECT identity_verified FROM kyc.entity_workstreams
               WHERE entity_id = $1 AND case_id = $2 AND identity_verified = true
               LIMIT 1"#,
        )
        .bind(entity_id)
        .bind(case_id)
        .fetch_optional(pool)
        .await?;

        let identity_covered = identity_count.0 > 0 || ws_verified.is_some();
        update_prong(&mut prong_covered, "IDENTITY", identity_covered);
        if !identity_covered {
            gaps.push(serde_json::json!({
                "gap_id": format!("{}:IDENTITY", entity_id),
                "prong": "IDENTITY",
                "entity_id": entity_id.to_string(),
                "description": format!("Missing verified identity document for {}", entity_id),
                "blocking_at_gate": "EVIDENCE_COMPLETE"
            }));
        }

        // CONTROL: check control edges documented
        let control_count: (i64,) = sqlx::query_as(
            r#"SELECT COUNT(*) FROM "ob-poc".entity_relationships
               WHERE from_entity_id = $1
                 AND (relationship_type = 'control' OR control_type IS NOT NULL)
                 AND (effective_to IS NULL OR effective_to > CURRENT_DATE)"#,
        )
        .bind(entity_id)
        .fetch_one(pool)
        .await
        .unwrap_or((0,));

        let control_covered = control_count.0 > 0;
        update_prong(&mut prong_covered, "CONTROL", control_covered);
        if !control_covered {
            gaps.push(serde_json::json!({
                "gap_id": format!("{}:CONTROL", entity_id),
                "prong": "CONTROL",
                "entity_id": entity_id.to_string(),
                "description": format!("No control relationship documented for {}", entity_id),
                "blocking_at_gate": "EVIDENCE_COMPLETE"
            }));
        }

        // SOURCE_OF_WEALTH: check SOW evidence
        let sow_count: (i64,) = sqlx::query_as(
            r#"SELECT COUNT(*) FROM kyc.ubo_evidence ue
               JOIN kyc.ubo_registry ur ON ur.ubo_id = ue.ubo_id
               WHERE ur.ubo_person_id = $1 AND ur.case_id = $2
                 AND ue.evidence_type IN ('SOURCE_OF_WEALTH', 'SOURCE_OF_FUNDS',
                                          'ANNUAL_RETURN', 'CHAIN_PROOF')
                 AND ue.status IN ('VERIFIED', 'RECEIVED')"#,
        )
        .bind(entity_id)
        .bind(case_id)
        .fetch_one(pool)
        .await
        .unwrap_or((0,));

        let sow_covered = sow_count.0 > 0;
        update_prong(&mut prong_covered, "SOURCE_OF_WEALTH", sow_covered);
        if !sow_covered {
            gaps.push(serde_json::json!({
                "gap_id": format!("{}:SOURCE_OF_WEALTH", entity_id),
                "prong": "SOURCE_OF_WEALTH",
                "entity_id": entity_id.to_string(),
                "description": format!("Missing source of wealth evidence for {}", entity_id),
                "blocking_at_gate": "EVIDENCE_COMPLETE"
            }));
        }
    }

    // 6. Compute prong summaries and overall coverage
    let mut prong_summaries: Vec<serde_json::Value> = Vec::new();
    let mut overall_sum = 0.0f64;

    for prong in &prongs {
        let (covered, total) = prong_covered.get(prong).copied().unwrap_or((0, 0));
        let pct = if total > 0 {
            (covered as f64 / total as f64) * 100.0
        } else {
            100.0
        };
        overall_sum += pct;
        prong_summaries.push(serde_json::json!({
            "prong": prong,
            "covered": covered,
            "total": total,
            "coverage_pct": pct
        }));
    }

    let overall_coverage_pct = if prongs.is_empty() {
        100.0
    } else {
        overall_sum / prongs.len() as f64
    };

    // 7. Persist coverage snapshot
    let coverage_snapshot = serde_json::json!({
        "overall_coverage_pct": overall_coverage_pct,
        "prong_coverage": prong_summaries,
        "gaps": gaps,
        "gaps_blocking_skeleton": gaps.iter()
            .filter(|g| g.get("blocking_at_gate").and_then(|v| v.as_str()) == Some("SKELETON_READY"))
            .count()
    });

    sqlx::query(
        r#"UPDATE kyc.ubo_determination_runs SET coverage_snapshot = $2 WHERE run_id = $1"#,
    )
    .bind(determination_run_id)
    .bind(&coverage_snapshot)
    .execute(pool)
    .await?;

    tracing::info!(
        "skeleton.build step 4: coverage.compute case={} overall={:.1}% gaps={}",
        case_id,
        overall_coverage_pct,
        gaps.len()
    );

    Ok(overall_coverage_pct)
}

/// Extract candidate entity IDs from a determination run's output_snapshot JSON.
#[cfg(feature = "database")]
fn extract_candidate_entity_ids(snapshot: &serde_json::Value) -> Vec<Uuid> {
    // The snapshot may be an array of candidates or have a "candidates" key
    let arr = snapshot
        .as_array()
        .or_else(|| snapshot.get("candidates").and_then(|v| v.as_array()));

    let Some(arr) = arr else {
        return vec![];
    };

    arr.iter()
        .filter_map(|item| {
            item.get("entity_id")
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok())
        })
        .collect()
}

/// Update prong (covered, total) counter.
#[cfg(feature = "database")]
fn update_prong(totals: &mut HashMap<&str, (i32, i32)>, prong: &str, is_covered: bool) {
    if let Some(counts) = totals.get_mut(prong) {
        counts.1 += 1;
        if is_covered {
            counts.0 += 1;
        }
    }
}

// ============================================================================
// Step 5: Outreach Plan Generate — gap-to-doc mapping
//
// Mirrors outreach_plan_ops.rs:
//   1. Read gaps from determination run coverage_snapshot
//   2. Map each gap to required document type per spec 2A.2
//   3. Sort by priority (identity=1, ownership=2, control=3, SOW=4)
//   4. Cap at 8 items per plan
//   5. Insert plan + items
// ============================================================================

#[cfg(feature = "database")]
async fn run_outreach_plan(
    pool: &PgPool,
    case_id: Uuid,
    determination_run_id: Uuid,
) -> Result<Option<Uuid>> {
    // 1. Read coverage snapshot to get gaps
    let snapshot_row: Option<(Option<serde_json::Value>,)> = sqlx::query_as(
        r#"SELECT coverage_snapshot FROM kyc.ubo_determination_runs
           WHERE run_id = $1 AND case_id = $2"#,
    )
    .bind(determination_run_id)
    .bind(case_id)
    .fetch_optional(pool)
    .await?;

    let coverage_snapshot = snapshot_row.and_then(|(snap,)| snap);

    // Extract gaps from the coverage snapshot
    let gaps: Vec<serde_json::Value> = match &coverage_snapshot {
        Some(snap) => snap
            .get("gaps")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default(),
        None => vec![],
    };

    if gaps.is_empty() {
        // No gaps — create empty plan
        let plan_id: (Uuid,) = sqlx::query_as(
            r#"INSERT INTO kyc.outreach_plans (case_id, determination_run_id, status, total_items)
               VALUES ($1, $2, 'DRAFT', 0)
               RETURNING plan_id"#,
        )
        .bind(case_id)
        .bind(determination_run_id)
        .fetch_one(pool)
        .await?;

        return Ok(Some(plan_id.0));
    }

    // 2. Map gaps to outreach items with doc type mapping per spec 2A.2
    struct PlannedItem {
        entity_id: Uuid,
        prong: String,
        gap_description: String,
        doc_type: &'static str,
        request_text: String,
        priority: i32,
        gap_ref: String,
    }

    let subject_entity_id: Uuid = sqlx::query_scalar(
        r#"SELECT subject_entity_id FROM kyc.ubo_determination_runs WHERE run_id = $1"#,
    )
    .bind(determination_run_id)
    .fetch_one(pool)
    .await?;

    let mut planned_items: Vec<PlannedItem> = gaps
        .iter()
        .filter_map(|gap| {
            let prong = gap
                .get("prong")
                .and_then(|v| v.as_str())
                .unwrap_or("OWNERSHIP");
            let entity_id = gap
                .get("entity_id")
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok())
                .unwrap_or(subject_entity_id);
            let description = gap
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("Coverage gap identified");

            let doc_type: &'static str = match prong {
                "OWNERSHIP" => "SHARE_REGISTER",
                "IDENTITY" => "PASSPORT",
                "CONTROL" => "BOARD_RESOLUTION",
                "SOURCE_OF_WEALTH" => "SOURCE_OF_WEALTH_DECLARATION",
                _ => "SUPPORTING_EVIDENCE",
            };

            let priority: i32 = match prong {
                "IDENTITY" => 1,
                "OWNERSHIP" => 2,
                "CONTROL" => 3,
                "SOURCE_OF_WEALTH" => 4,
                _ => 5,
            };

            let request_text = format!(
                "Please provide {} for {} verification. Gap: {}",
                doc_type.to_lowercase().replace('_', " "),
                prong.to_lowercase().replace('_', " "),
                description
            );

            let gap_ref = format!("{}:{}", prong, entity_id);

            Some(PlannedItem {
                entity_id,
                prong: prong.to_string(),
                gap_description: description.to_string(),
                doc_type,
                request_text,
                priority,
                gap_ref,
            })
        })
        .collect();

    // 3. Sort by priority, then entity
    planned_items.sort_by(|a, b| {
        a.priority
            .cmp(&b.priority)
            .then(a.entity_id.cmp(&b.entity_id))
    });

    // 4. Cap at 8 items
    planned_items.truncate(8);

    let items_count = planned_items.len() as i32;

    // 5. Insert plan + items
    let plan_id: (Uuid,) = sqlx::query_as(
        r#"INSERT INTO kyc.outreach_plans (case_id, determination_run_id, status, total_items)
           VALUES ($1, $2, 'DRAFT', $3)
           RETURNING plan_id"#,
    )
    .bind(case_id)
    .bind(determination_run_id)
    .bind(items_count)
    .fetch_one(pool)
    .await?;

    for item in &planned_items {
        sqlx::query(
            r#"INSERT INTO kyc.outreach_items (
                   plan_id, prong, target_entity_id, gap_description,
                   request_text, doc_type_requested, priority, closes_gap_ref, status
               ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'PENDING')"#,
        )
        .bind(plan_id.0)
        .bind(&item.prong)
        .bind(item.entity_id)
        .bind(&item.gap_description)
        .bind(&item.request_text)
        .bind(item.doc_type)
        .bind(item.priority)
        .bind(&item.gap_ref)
        .execute(pool)
        .await?;
    }

    tracing::info!(
        "skeleton.build step 5: outreach.plan-generate case={} plan={} items={}",
        case_id,
        plan_id.0,
        items_count
    );

    Ok(Some(plan_id.0))
}

// ============================================================================
// Step 6: Tollgate Evaluate — SKELETON_READY gate evaluation
//
// Mirrors tollgate_evaluate_ops.rs evaluate_skeleton_ready():
//   1. Load gate definition from ob_ref.tollgate_definitions
//   2. Check ownership coverage >= threshold (default 70%)
//   3. Check all entities have at least one ownership edge
//   4. Record evaluation in kyc.tollgate_evaluations
// ============================================================================

#[cfg(feature = "database")]
async fn run_tollgate_evaluate(pool: &PgPool, case_id: Uuid) -> Result<bool> {
    // 1. Load gate definition (with fallback defaults if ref data not seeded)
    let gate_row: Option<(String, serde_json::Value)> = sqlx::query_as(
        r#"SELECT tollgate_id, default_thresholds
           FROM ob_ref.tollgate_definitions
           WHERE tollgate_id = 'SKELETON_READY'"#,
    )
    .bind(case_id)
    .fetch_optional(pool)
    .await?;

    let thresholds = gate_row
        .map(|(_, t)| t)
        .unwrap_or_else(|| serde_json::json!({"ownership_coverage_pct": 70.0}));

    let ownership_threshold = thresholds
        .get("ownership_coverage_pct")
        .and_then(|v| v.as_f64())
        .unwrap_or(70.0);

    // 2. Check ownership coverage
    let coverage_stats: (i64, i64) = sqlx::query_as(
        r#"SELECT
               COUNT(*) AS total_entities,
               COUNT(*) FILTER (WHERE ownership_proved = TRUE) AS ownership_proved_count
           FROM kyc.entity_workstreams
           WHERE case_id = $1"#,
    )
    .bind(case_id)
    .fetch_one(pool)
    .await
    .unwrap_or((0, 0));

    let ownership_pct = if coverage_stats.0 > 0 {
        (coverage_stats.1 as f64 / coverage_stats.0 as f64) * 100.0
    } else {
        0.0
    };

    let ownership_passed = ownership_pct >= ownership_threshold;

    // 3. Check all entities have at least one ownership edge
    let entities_without_edges: (i64,) = sqlx::query_as(
        r#"SELECT COUNT(*)
           FROM kyc.entity_workstreams ew
           WHERE ew.case_id = $1
             AND NOT EXISTS (
                 SELECT 1 FROM "ob-poc".entity_relationships er
                 WHERE (er.from_entity_id = ew.entity_id OR er.to_entity_id = ew.entity_id)
                   AND er.relationship_type IN ('ownership', 'OWNERSHIP')
                   AND (er.effective_to IS NULL OR er.effective_to > CURRENT_DATE)
             )"#,
    )
    .bind(case_id)
    .fetch_one(pool)
    .await
    .unwrap_or((0,));

    let edges_passed = entities_without_edges.0 == 0;
    let passed = ownership_passed && edges_passed;

    // 4. Record evaluation
    let evaluation_detail = serde_json::json!({
        "gate_name": "SKELETON_READY",
        "passed": passed,
        "checks": [
            {
                "criterion": "ownership_coverage_pct",
                "passed": ownership_passed,
                "actual_value": ownership_pct,
                "threshold_value": ownership_threshold,
                "detail": format!(
                    "Ownership coverage {:.1}% (threshold: {:.1}%): {} of {} entities proved",
                    ownership_pct, ownership_threshold, coverage_stats.1, coverage_stats.0
                )
            },
            {
                "criterion": "all_entities_have_ownership_edge",
                "passed": edges_passed,
                "actual_value": entities_without_edges.0,
                "threshold_value": 0,
                "detail": format!(
                    "{} workstream entities without ownership edges",
                    entities_without_edges.0
                )
            }
        ]
    });

    sqlx::query(
        r#"INSERT INTO kyc.tollgate_evaluations (
               case_id, tollgate_id, passed, evaluation_detail, config_version
           ) VALUES ($1, 'SKELETON_READY', $2, $3, 'v1')"#,
    )
    .bind(case_id)
    .bind(passed)
    .bind(&evaluation_detail)
    .execute(pool)
    .await?;

    tracing::info!(
        "skeleton.build step 6: tollgate SKELETON_READY case={} passed={} ownership={:.1}% edges_without={}",
        case_id,
        passed,
        ownership_pct,
        entities_without_edges.0
    );

    Ok(passed)
}
