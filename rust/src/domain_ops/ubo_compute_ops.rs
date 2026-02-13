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
    #[allow(dead_code)]
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
            all_candidates
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

// ============================================================================
// Snapshot Result Types (typed structs per CLAUDE.md Non-Negotiable Rule #1)
// ============================================================================

/// Result of capturing a UBO determination snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UboSnapshotCaptureResult {
    pub run_id: Uuid,
    pub code_hash: String,
    pub config_version: String,
    pub candidates_captured: i64,
    pub chains_captured: i64,
}

/// Result of diffing two UBO determination snapshots.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UboSnapshotDiffResult {
    pub run_id_a: Uuid,
    pub run_id_b: Uuid,
    pub added: Vec<Uuid>,
    pub removed: Vec<Uuid>,
    pub changed: Vec<UboChange>,
}

/// A single field-level change between two UBO snapshots.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UboChange {
    pub entity_id: Uuid,
    pub field: String,
    pub old_value: String,
    pub new_value: String,
}

// ============================================================================
// UboSnapshotCaptureOp
// ============================================================================

/// Serialize a determination run's computed candidates into output_snapshot +
/// chains_snapshot JSONB columns, recording the code_hash and config_version
/// for deterministic replay auditing.
///
/// Rationale: Aggregates data across ubo_registry and entity_relationships via
/// subqueries into JSONB snapshots with cryptographic code hash — not expressible
/// as a single CRUD operation.
#[register_custom_op]
pub struct UboSnapshotCaptureOp;

#[async_trait]
impl CustomOperation for UboSnapshotCaptureOp {
    fn domain(&self) -> &'static str {
        "ubo"
    }

    fn verb(&self) -> &'static str {
        "snapshot.capture"
    }

    fn rationale(&self) -> &'static str {
        "Aggregates ubo_registry candidates and entity_relationships chains into \
         JSONB snapshots with SHA-256 code hash for deterministic replay auditing"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use sha2::{Digest, Sha256};

        // ------------------------------------------------------------------
        // 1. Extract arguments
        // ------------------------------------------------------------------
        let case_id = extract_uuid(verb_call, ctx, "case-id")?;
        let determination_run_id = extract_uuid(verb_call, ctx, "determination-run-id")?;

        // ------------------------------------------------------------------
        // 2. Compute code_hash (SHA-256 of algorithm identifier)
        // ------------------------------------------------------------------
        let mut hasher = Sha256::new();
        hasher.update(b"ubo_compute_v1");
        let code_hash = format!("{:x}", hasher.finalize());
        let config_version = "1.0.0".to_string();

        // ------------------------------------------------------------------
        // 3. Capture snapshots via UPDATE with correlated subqueries
        // ------------------------------------------------------------------
        sqlx::query(
            r#"UPDATE kyc.ubo_determination_runs
               SET output_snapshot = (
                     SELECT jsonb_agg(jsonb_build_object(
                       'ubo_id', ur.ubo_id,
                       'entity_id', ur.entity_id,
                       'status', ur.status,
                       'ownership_pct', ur.ownership_pct
                     ))
                     FROM kyc.ubo_registry ur
                     WHERE ur.determination_run_id = $1
                   ),
                   chains_snapshot = (
                     SELECT jsonb_agg(jsonb_build_object(
                       'from_entity_id', er.from_entity_id,
                       'to_entity_id', er.to_entity_id,
                       'relationship_type', er.relationship_type,
                       'ownership_pct', er.ownership_pct
                     ))
                     FROM "ob-poc".entity_relationships er
                     JOIN kyc.entity_workstreams ew ON er.to_entity_id = ew.entity_id
                     WHERE ew.case_id = $2
                       AND er.relationship_type = 'OWNERSHIP'
                       AND er.effective_to IS NULL
                   ),
                   code_hash = $3,
                   config_version = $4,
                   completed_at = NOW()
               WHERE run_id = $1"#,
        )
        .bind(determination_run_id)
        .bind(case_id)
        .bind(&code_hash)
        .bind(&config_version)
        .execute(pool)
        .await?;

        // ------------------------------------------------------------------
        // 4. Count captured items for the result
        // ------------------------------------------------------------------
        let candidates_captured: (Option<i64>,) = sqlx::query_as(
            r#"SELECT jsonb_array_length(output_snapshot)::bigint
               FROM kyc.ubo_determination_runs
               WHERE run_id = $1"#,
        )
        .bind(determination_run_id)
        .fetch_one(pool)
        .await?;

        let chains_captured: (Option<i64>,) = sqlx::query_as(
            r#"SELECT jsonb_array_length(chains_snapshot)::bigint
               FROM kyc.ubo_determination_runs
               WHERE run_id = $1"#,
        )
        .bind(determination_run_id)
        .fetch_one(pool)
        .await?;

        // Bind run_id for downstream DSL references
        if let Some(ref binding) = verb_call.binding {
            ctx.bind(binding, determination_run_id);
        }

        tracing::info!(
            "ubo.snapshot.capture: run={} candidates={} chains={} code_hash={}",
            determination_run_id,
            candidates_captured.0.unwrap_or(0),
            chains_captured.0.unwrap_or(0),
            &code_hash[..12]
        );

        let result = UboSnapshotCaptureResult {
            run_id: determination_run_id,
            code_hash,
            config_version,
            candidates_captured: candidates_captured.0.unwrap_or(0),
            chains_captured: chains_captured.0.unwrap_or(0),
        };

        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::to_value(
            UboSnapshotCaptureResult {
                run_id: Uuid::nil(),
                code_hash: String::new(),
                config_version: "1.0.0".to_string(),
                candidates_captured: 0,
                chains_captured: 0,
            },
        )?))
    }
}

// ============================================================================
// UboSnapshotDiffOp
// ============================================================================

/// Compare two UBO determination snapshots and return added, removed, and changed
/// candidates between them.
///
/// Rationale: Loads two JSONB snapshots, parses them in Rust, and performs set-diff
/// logic with field-level change detection — not expressible as a CRUD operation.
#[register_custom_op]
pub struct UboSnapshotDiffOp;

/// Internal representation of a snapshot candidate row for diffing.
#[cfg(feature = "database")]
#[derive(Debug, Clone, Deserialize)]
struct SnapshotCandidate {
    entity_id: Uuid,
    status: Option<String>,
    ownership_pct: Option<f64>,
    #[allow(dead_code)]
    ubo_id: Option<Uuid>,
}

#[async_trait]
impl CustomOperation for UboSnapshotDiffOp {
    fn domain(&self) -> &'static str {
        "ubo"
    }

    fn verb(&self) -> &'static str {
        "snapshot.diff"
    }

    fn rationale(&self) -> &'static str {
        "Loads two JSONB snapshots by run_id, deserializes, and computes set-diff \
         with field-level change detection for UBO candidates"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        // ------------------------------------------------------------------
        // 1. Extract arguments
        // ------------------------------------------------------------------
        let run_id_a = extract_uuid(verb_call, ctx, "run-id-a")?;
        let run_id_b = extract_uuid(verb_call, ctx, "run-id-b")?;

        // ------------------------------------------------------------------
        // 2. Load both snapshots in a single query
        // ------------------------------------------------------------------
        let run_ids = vec![run_id_a, run_id_b];
        let rows: Vec<(Uuid, Option<serde_json::Value>, Option<serde_json::Value>)> =
            sqlx::query_as(
                r#"SELECT run_id, output_snapshot, chains_snapshot
                   FROM kyc.ubo_determination_runs
                   WHERE run_id = ANY($1)"#,
            )
            .bind(&run_ids)
            .fetch_all(pool)
            .await?;

        // Find snapshots for each run
        let snap_a = rows
            .iter()
            .find(|(rid, _, _)| *rid == run_id_a)
            .ok_or_else(|| anyhow!("Determination run {} not found", run_id_a))?;
        let snap_b = rows
            .iter()
            .find(|(rid, _, _)| *rid == run_id_b)
            .ok_or_else(|| anyhow!("Determination run {} not found", run_id_b))?;

        // ------------------------------------------------------------------
        // 3. Parse output_snapshot arrays into candidate maps
        // ------------------------------------------------------------------
        let parse_candidates =
            |snapshot: &Option<serde_json::Value>| -> Result<std::collections::HashMap<Uuid, SnapshotCandidate>> {
                let mut map = std::collections::HashMap::new();
                if let Some(val) = snapshot {
                    let candidates: Vec<SnapshotCandidate> = serde_json::from_value(val.clone())
                        .map_err(|e| anyhow!("Failed to parse output_snapshot: {}", e))?;
                    for c in candidates {
                        map.insert(c.entity_id, c);
                    }
                }
                Ok(map)
            };

        let map_a = parse_candidates(&snap_a.1)?;
        let map_b = parse_candidates(&snap_b.1)?;

        // ------------------------------------------------------------------
        // 4. Compute diff: added, removed, changed
        // ------------------------------------------------------------------
        let mut added: Vec<Uuid> = Vec::new();
        let mut removed: Vec<Uuid> = Vec::new();
        let mut changed: Vec<UboChange> = Vec::new();

        // Entities in B but not in A → added
        for entity_id in map_b.keys() {
            if !map_a.contains_key(entity_id) {
                added.push(*entity_id);
            }
        }

        // Entities in A but not in B → removed
        for entity_id in map_a.keys() {
            if !map_b.contains_key(entity_id) {
                removed.push(*entity_id);
            }
        }

        // Entities in both → check for field-level changes
        for (entity_id, cand_a) in &map_a {
            if let Some(cand_b) = map_b.get(entity_id) {
                // Compare status
                if cand_a.status != cand_b.status {
                    changed.push(UboChange {
                        entity_id: *entity_id,
                        field: "status".to_string(),
                        old_value: cand_a.status.clone().unwrap_or_else(|| "null".to_string()),
                        new_value: cand_b.status.clone().unwrap_or_else(|| "null".to_string()),
                    });
                }

                // Compare ownership_pct (with tolerance for float comparison)
                let pct_a = cand_a.ownership_pct.unwrap_or(0.0);
                let pct_b = cand_b.ownership_pct.unwrap_or(0.0);
                if (pct_a - pct_b).abs() > 0.001 {
                    changed.push(UboChange {
                        entity_id: *entity_id,
                        field: "ownership_pct".to_string(),
                        old_value: format!("{:.4}", pct_a),
                        new_value: format!("{:.4}", pct_b),
                    });
                }
            }
        }

        // Sort for deterministic output
        added.sort();
        removed.sort();
        changed.sort_by(|a, b| a.entity_id.cmp(&b.entity_id).then(a.field.cmp(&b.field)));

        tracing::info!(
            "ubo.snapshot.diff: run_a={} run_b={} added={} removed={} changed={}",
            run_id_a,
            run_id_b,
            added.len(),
            removed.len(),
            changed.len()
        );

        let result = UboSnapshotDiffResult {
            run_id_a,
            run_id_b,
            added,
            removed,
            changed,
        };

        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::to_value(
            UboSnapshotDiffResult {
                run_id_a: Uuid::nil(),
                run_id_b: Uuid::nil(),
                added: vec![],
                removed: vec![],
                changed: vec![],
            },
        )?))
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snapshot_capture_metadata() {
        let op = UboSnapshotCaptureOp;
        assert_eq!(op.domain(), "ubo");
        assert_eq!(op.verb(), "snapshot.capture");
        assert!(!op.rationale().is_empty());
    }

    #[test]
    fn test_snapshot_diff_metadata() {
        let op = UboSnapshotDiffOp;
        assert_eq!(op.domain(), "ubo");
        assert_eq!(op.verb(), "snapshot.diff");
        assert!(!op.rationale().is_empty());
    }
}
