//! UBO Chain Computation Operations (Phase 2.3)
//!
//! Computes Ultimate Beneficial Ownership chains per KYC/UBO architecture spec section 6.1:
//! 1. Load ownership edges from entity_relationships for entities linked to a case
//! 2. Build directed graph (adjacency list)
//! 3. Traverse upward multiplying percentages along chains
//! 4. Detect cycles (mark as anomaly, no infinite loop)
//! 5. Apply threshold filter (default 5%)
//! 6. Persist results to kyc.ubo_determination_runs with JSONB snapshots
//!
//! ## Key Tables
//! - kyc.entity_workstreams (case → entity linkage)
//! - "ob-poc".entity_relationships (ownership edges)
//! - "ob-poc".entities + entity_types (terminus detection: natural person)
//! - kyc.ubo_determination_runs (output)

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use ob_poc_macros::register_custom_op;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[cfg(feature = "database")]
use sqlx::PgPool;

use super::helpers::{extract_string_opt, extract_uuid};
use super::{CustomOperation, ExecutionContext, ExecutionResult, VerbCall};

// ============================================================================
// Result Types (typed structs per CLAUDE.md Non-Negotiable Rule #1)
// ============================================================================

/// Top-level result of a UBO chain computation run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UboComputeResult {
    pub run_id: Uuid,
    pub case_id: Uuid,
    pub candidates_found: i32,
    pub chains_computed: i32,
    pub threshold_pct: f64,
    pub candidates: Vec<UboCandidate>,
}

/// A single UBO candidate discovered by chain traversal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UboCandidate {
    pub entity_id: Uuid,
    pub entity_name: Option<String>,
    pub total_ownership_pct: f64,
    pub chain_count: i32,
    pub is_terminus: bool,
    pub chains: Vec<OwnershipChain>,
}

/// A single ownership chain from a subject entity up to a beneficial owner.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OwnershipChain {
    pub path: Vec<Uuid>,
    pub effective_pct: f64,
}

// ============================================================================
// Internal Types (graph construction)
// ============================================================================

/// An ownership edge loaded from entity_relationships.
#[cfg(feature = "database")]
#[derive(Debug, Clone)]
struct OwnershipEdge {
    from_entity_id: Uuid,
    to_entity_id: Uuid,
    percentage: f64,
}

/// Metadata for an entity relevant to chain computation.
#[cfg(feature = "database")]
#[derive(Debug, Clone)]
struct EntityMeta {
    entity_id: Uuid,
    name: Option<String>,
    is_natural_person: bool,
}

// ============================================================================
// UboComputeChainsOp
// ============================================================================

/// Compute UBO ownership chains for all entities linked to a KYC case.
///
/// Rationale: Requires in-memory graph construction, recursive chain traversal
/// with percentage multiplication, cycle detection, and JSONB snapshot persistence.
/// Cannot be expressed as a CRUD verb.
#[register_custom_op]
pub struct UboComputeChainsOp;

#[async_trait]
impl CustomOperation for UboComputeChainsOp {
    fn domain(&self) -> &'static str {
        "ubo"
    }

    fn verb(&self) -> &'static str {
        "compute-chains"
    }

    fn rationale(&self) -> &'static str {
        "Builds in-memory ownership graph, traverses chains multiplying percentages, \
         detects cycles, and persists JSONB snapshot to ubo_determination_runs"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let start = std::time::Instant::now();

        // ------------------------------------------------------------------
        // 1. Extract arguments
        // ------------------------------------------------------------------
        let case_id = extract_uuid(verb_call, ctx, "case-id")?;

        // Threshold defaults to 5.0%
        let threshold_pct: f64 = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "threshold")
            .and_then(|a| a.value.as_decimal())
            .map(|d| d.to_string().parse::<f64>().unwrap_or(5.0))
            .unwrap_or(5.0);

        // Optional workstream-id to narrow scope to a single entity
        let workstream_id_filter =
            super::helpers::extract_uuid_opt(verb_call, ctx, "workstream-id");

        // Config version tag (for deterministic replay auditing)
        let config_version =
            extract_string_opt(verb_call, "config-version").unwrap_or_else(|| "v1.0".to_string());

        // ------------------------------------------------------------------
        // 2. Load subject entities from entity_workstreams for this case
        // ------------------------------------------------------------------
        let subject_entities: Vec<(Uuid, Option<Uuid>)> = if let Some(ws_id) = workstream_id_filter
        {
            sqlx::query_as(
                r#"SELECT entity_id, workstream_id
                   FROM kyc.entity_workstreams
                   WHERE case_id = $1 AND workstream_id = $2"#,
            )
            .bind(case_id)
            .bind(ws_id)
            .fetch_all(pool)
            .await?
        } else {
            sqlx::query_as(
                r#"SELECT entity_id, workstream_id
                   FROM kyc.entity_workstreams
                   WHERE case_id = $1"#,
            )
            .bind(case_id)
            .fetch_all(pool)
            .await?
        };

        if subject_entities.is_empty() {
            return Err(anyhow!("No entity workstreams found for case {}", case_id));
        }

        // Collect all subject entity IDs
        let subject_entity_ids: Vec<Uuid> = subject_entities.iter().map(|(eid, _)| *eid).collect();

        // ------------------------------------------------------------------
        // 3. Load all active ownership edges reachable from subject entities
        //    We load broadly (all active ownership edges) since the graph
        //    may traverse entities not directly in the case workstreams.
        // ------------------------------------------------------------------
        let edges: Vec<OwnershipEdge> = {
            let rows: Vec<(Uuid, Uuid, Option<rust_decimal::Decimal>)> = sqlx::query_as(
                r#"SELECT from_entity_id, to_entity_id, percentage
                   FROM "ob-poc".entity_relationships
                   WHERE relationship_type IN ('ownership', 'OWNERSHIP')
                     AND (effective_to IS NULL OR effective_to > CURRENT_DATE)
                   ORDER BY from_entity_id"#,
            )
            .fetch_all(pool)
            .await?;

            rows.into_iter()
                .map(|(from_id, to_id, pct)| OwnershipEdge {
                    from_entity_id: from_id,
                    to_entity_id: to_id,
                    percentage: pct
                        .map(|d| d.to_string().parse::<f64>().unwrap_or(0.0))
                        .unwrap_or(0.0),
                })
                .collect()
        };

        // ------------------------------------------------------------------
        // 4. Build adjacency list: to_entity_id → Vec<(from_entity_id, pct)>
        //    "Who owns this entity?" — edges point from owner to owned,
        //    so we index by to_entity_id (the owned entity) to traverse upward.
        // ------------------------------------------------------------------
        use std::collections::HashMap;

        let mut upward_adj: HashMap<Uuid, Vec<(Uuid, f64)>> = HashMap::new();
        for edge in &edges {
            upward_adj
                .entry(edge.to_entity_id)
                .or_default()
                .push((edge.from_entity_id, edge.percentage));
        }

        // ------------------------------------------------------------------
        // 5. Load entity metadata for terminus detection (natural person check)
        //    and name resolution.
        // ------------------------------------------------------------------
        // Collect all entity IDs that appear in edges
        let mut all_entity_ids: std::collections::HashSet<Uuid> = std::collections::HashSet::new();
        for edge in &edges {
            all_entity_ids.insert(edge.from_entity_id);
            all_entity_ids.insert(edge.to_entity_id);
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

        let entity_map: HashMap<Uuid, EntityMeta> = entity_meta_rows
            .into_iter()
            .map(|(eid, name, category)| {
                (
                    eid,
                    EntityMeta {
                        entity_id: eid,
                        name,
                        is_natural_person: category == "PERSON",
                    },
                )
            })
            .collect();

        // ------------------------------------------------------------------
        // 6. For each subject entity, traverse upward to find UBO candidates
        // ------------------------------------------------------------------
        let mut all_candidates: Vec<UboCandidate> = Vec::new();
        let mut total_chains = 0i32;

        for subject_entity_id in &subject_entity_ids {
            // Accumulator: owner_entity_id → Vec<OwnershipChain>
            let mut owner_chains: HashMap<Uuid, Vec<OwnershipChain>> = HashMap::new();

            // DFS with cycle detection
            // Stack entries: (current_entity, path_so_far, cumulative_pct)
            let mut stack: Vec<(Uuid, Vec<Uuid>, f64)> = Vec::new();

            // Seed: start from the subject entity
            if let Some(owners) = upward_adj.get(subject_entity_id) {
                for (owner_id, pct) in owners {
                    stack.push((*owner_id, vec![*subject_entity_id, *owner_id], *pct));
                }
            }

            while let Some((current, path, cumulative_pct)) = stack.pop() {
                // Check if this entity is a terminus (natural person or no further owners)
                let meta = entity_map.get(&current);
                let is_natural_person = meta.map(|m| m.is_natural_person).unwrap_or(false);
                let has_further_owners = upward_adj.contains_key(&current);

                let is_terminus = is_natural_person || !has_further_owners;

                if is_terminus {
                    // Record this chain
                    let chain = OwnershipChain {
                        path: path.clone(),
                        effective_pct: cumulative_pct,
                    };
                    owner_chains.entry(current).or_default().push(chain);
                    continue;
                }

                // Continue traversal upward
                if let Some(owners) = upward_adj.get(&current) {
                    for (owner_id, pct) in owners {
                        // Cycle detection: skip if owner already in path
                        if path.contains(owner_id) {
                            // Record as anomalous chain (cycle detected)
                            let mut cycle_path = path.clone();
                            cycle_path.push(*owner_id);
                            let chain = OwnershipChain {
                                path: cycle_path,
                                effective_pct: 0.0, // Cycles get 0% effective ownership
                            };
                            owner_chains.entry(*owner_id).or_default().push(chain);

                            tracing::warn!(
                                "ubo.compute-chains: cycle detected in ownership graph: {:?} -> {}",
                                path,
                                owner_id
                            );
                            continue;
                        }

                        // Depth guard: max 20 hops
                        if path.len() >= 20 {
                            tracing::warn!(
                                "ubo.compute-chains: max depth reached at entity {}",
                                current
                            );
                            let chain = OwnershipChain {
                                path: path.clone(),
                                effective_pct: cumulative_pct,
                            };
                            owner_chains.entry(current).or_default().push(chain);
                            continue;
                        }

                        let mut new_path = path.clone();
                        new_path.push(*owner_id);
                        let new_pct = cumulative_pct * pct / 100.0;

                        stack.push((*owner_id, new_path, new_pct));
                    }
                }
            }

            // ------------------------------------------------------------------
            // 7. Aggregate chains per owner and apply threshold filter
            // ------------------------------------------------------------------
            for (owner_id, chains) in owner_chains {
                let total_pct: f64 = chains.iter().map(|c| c.effective_pct).sum();
                let chain_count = chains.len() as i32;
                total_chains += chain_count;

                // Apply threshold — skip owners below threshold unless they are
                // explicitly at 0.0 (cycle anomalies). Cycles are always included
                // as anomalies for audit purposes.
                let has_cycle = chains
                    .iter()
                    .any(|c| c.effective_pct == 0.0 && c.path.len() > 2);
                if total_pct < threshold_pct && !has_cycle {
                    continue;
                }

                let meta = entity_map.get(&owner_id);

                all_candidates.push(UboCandidate {
                    entity_id: owner_id,
                    entity_name: meta.and_then(|m| m.name.clone()),
                    total_ownership_pct: total_pct,
                    chain_count,
                    is_terminus: meta.map(|m| m.is_natural_person).unwrap_or(false),
                    chains,
                });
            }
        }

        // Sort candidates by ownership descending
        all_candidates.sort_by(|a, b| {
            b.total_ownership_pct
                .partial_cmp(&a.total_ownership_pct)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let candidates_found = all_candidates.len() as i32;

        // ------------------------------------------------------------------
        // 8. Build JSONB snapshots
        // ------------------------------------------------------------------
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

        let computation_ms = start.elapsed().as_millis() as i32;

        // ------------------------------------------------------------------
        // 9. Persist to kyc.ubo_determination_runs
        //    One row per subject entity (first in scope for now).
        // ------------------------------------------------------------------
        let primary_subject = subject_entity_ids
            .first()
            .copied()
            .ok_or_else(|| anyhow!("No subject entities"))?;

        let run_id: Uuid = sqlx::query_scalar(
            r#"INSERT INTO kyc.ubo_determination_runs (
                   subject_entity_id,
                   case_id,
                   as_of,
                   config_version,
                   threshold_pct,
                   candidates_found,
                   output_snapshot,
                   chains_snapshot,
                   coverage_snapshot,
                   computed_at,
                   computed_by,
                   computation_ms
               ) VALUES ($1, $2, CURRENT_DATE, $3, $4, $5, $6, $7, NULL, NOW(), 'ubo.compute-chains', $8)
               RETURNING run_id"#,
        )
        .bind(primary_subject)
        .bind(case_id)
        .bind(&config_version)
        .bind(rust_decimal::Decimal::from_f64_retain(threshold_pct)
            .unwrap_or_else(|| rust_decimal::Decimal::new(500, 2))) // fallback 5.00
        .bind(candidates_found)
        .bind(&output_snapshot)
        .bind(&chains_snapshot)
        .bind(computation_ms)
        .fetch_one(pool)
        .await?;

        // Bind run_id for downstream DSL references
        if let Some(ref binding) = verb_call.binding {
            ctx.bind(binding, run_id);
        }

        tracing::info!(
            "ubo.compute-chains: case={} run={} candidates={} chains={} threshold={}% in {}ms",
            case_id,
            run_id,
            candidates_found,
            total_chains,
            threshold_pct,
            computation_ms
        );

        // ------------------------------------------------------------------
        // 10. Return typed result
        // ------------------------------------------------------------------
        let result = UboComputeResult {
            run_id,
            case_id,
            candidates_found,
            chains_computed: total_chains,
            threshold_pct,
            candidates: all_candidates,
        };

        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::json!({
            "run_id": Uuid::nil(),
            "case_id": Uuid::nil(),
            "candidates_found": 0,
            "chains_computed": 0,
            "threshold_pct": 5.0,
            "candidates": []
        })))
    }
}
