//! UBO-compute verbs (3 plugin verbs) — YAML-first
//! re-implementation of the compute subset of
//! `rust/config/verbs/ubo.yaml`.
//!
//! - `ubo.compute-chains` — in-memory ownership graph + upward
//!   chain traversal with percentage multiplication, cycle
//!   detection, and JSONB snapshot persistence.
//! - `ubo.snapshot.capture` — serialize a determination run's
//!   candidates + chains into output/chains snapshot columns
//!   with SHA-256 code hash.
//! - `ubo.snapshot.diff` — set-diff two JSONB snapshots with
//!   field-level change detection.

use std::collections::{HashMap, HashSet};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use dsl_runtime::domain_ops::helpers::{
    json_extract_string_opt, json_extract_uuid, json_extract_uuid_opt,
};
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::SemOsVerbOp;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OwnershipChain {
    path: Vec<Uuid>,
    effective_pct: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct UboCandidate {
    entity_id: Uuid,
    entity_name: Option<String>,
    total_ownership_pct: f64,
    chain_count: i32,
    is_terminus: bool,
    chains: Vec<OwnershipChain>,
}

#[derive(Debug, Clone)]
struct EntityMeta {
    name: Option<String>,
    is_natural_person: bool,
}

// ── ubo.compute-chains ────────────────────────────────────────────────────────

pub struct ComputeChains;

#[async_trait]
impl SemOsVerbOp for ComputeChains {
    fn fqn(&self) -> &str {
        "ubo.compute-chains"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let start = std::time::Instant::now();
        let case_id = json_extract_uuid(args, ctx, "case-id")?;
        let threshold_pct: f64 = json_extract_string_opt(args, "threshold")
            .as_deref()
            .and_then(|v| v.parse().ok())
            .unwrap_or(5.0);
        let workstream_id_filter = json_extract_uuid_opt(args, ctx, "workstream-id");
        let config_version =
            json_extract_string_opt(args, "config-version").unwrap_or_else(|| "v1.0".to_string());

        let subject_entities: Vec<(Uuid, Option<Uuid>)> = if let Some(ws_id) = workstream_id_filter {
            sqlx::query_as(
                r#"SELECT entity_id, workstream_id
                   FROM "ob-poc".entity_workstreams
                   WHERE case_id = $1 AND workstream_id = $2"#,
            )
            .bind(case_id)
            .bind(ws_id)
            .fetch_all(scope.executor())
            .await?
        } else {
            sqlx::query_as(
                r#"SELECT entity_id, workstream_id
                   FROM "ob-poc".entity_workstreams
                   WHERE case_id = $1"#,
            )
            .bind(case_id)
            .fetch_all(scope.executor())
            .await?
        };

        if subject_entities.is_empty() {
            return Err(anyhow!("No entity workstreams found for case {}", case_id));
        }
        let subject_entity_ids: Vec<Uuid> = subject_entities.iter().map(|(eid, _)| *eid).collect();

        let edge_rows: Vec<(Uuid, Uuid, Option<rust_decimal::Decimal>)> = sqlx::query_as(
            r#"SELECT from_entity_id, to_entity_id, percentage
               FROM "ob-poc".entity_relationships
               WHERE relationship_type IN ('ownership', 'OWNERSHIP')
                 AND (effective_to IS NULL OR effective_to > CURRENT_DATE)
               ORDER BY from_entity_id"#,
        )
        .fetch_all(scope.executor())
        .await?;

        let mut upward_adj: HashMap<Uuid, Vec<(Uuid, f64)>> = HashMap::new();
        let mut all_entity_ids: HashSet<Uuid> = HashSet::new();
        for (from_id, to_id, pct) in &edge_rows {
            let pct_f = pct
                .as_ref()
                .map(|d| d.to_string().parse::<f64>().unwrap_or(0.0))
                .unwrap_or(0.0);
            upward_adj.entry(*to_id).or_default().push((*from_id, pct_f));
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
               WHERE e.entity_id = ANY($1) AND e.deleted_at IS NULL"#,
        )
        .bind(&entity_id_vec)
        .fetch_all(scope.executor())
        .await?;

        let entity_map: HashMap<Uuid, EntityMeta> = entity_meta_rows
            .into_iter()
            .map(|(eid, name, category)| {
                (
                    eid,
                    EntityMeta {
                        name,
                        is_natural_person: category == "PERSON",
                    },
                )
            })
            .collect();

        let mut all_candidates: Vec<UboCandidate> = Vec::new();
        let mut total_chains = 0i32;

        for subject_entity_id in &subject_entity_ids {
            let mut owner_chains: HashMap<Uuid, Vec<OwnershipChain>> = HashMap::new();
            let mut stack: Vec<(Uuid, Vec<Uuid>, f64)> = Vec::new();

            if let Some(owners) = upward_adj.get(subject_entity_id) {
                for (owner_id, pct) in owners {
                    stack.push((*owner_id, vec![*subject_entity_id, *owner_id], *pct));
                }
            }

            while let Some((current, path, cumulative_pct)) = stack.pop() {
                let meta = entity_map.get(&current);
                let is_natural_person = meta.map(|m| m.is_natural_person).unwrap_or(false);
                let has_further_owners = upward_adj.contains_key(&current);
                let is_terminus = is_natural_person || !has_further_owners;

                if is_terminus {
                    owner_chains.entry(current).or_default().push(OwnershipChain {
                        path: path.clone(),
                        effective_pct: cumulative_pct,
                    });
                    continue;
                }

                if let Some(owners) = upward_adj.get(&current) {
                    for (owner_id, pct) in owners {
                        if path.contains(owner_id) {
                            let mut cycle_path = path.clone();
                            cycle_path.push(*owner_id);
                            owner_chains.entry(*owner_id).or_default().push(OwnershipChain {
                                path: cycle_path,
                                effective_pct: 0.0,
                            });
                            tracing::warn!(
                                "ubo.compute-chains: cycle detected: {:?} -> {}",
                                path,
                                owner_id
                            );
                            continue;
                        }
                        if path.len() >= 20 {
                            tracing::warn!(
                                "ubo.compute-chains: max depth reached at entity {}",
                                current
                            );
                            owner_chains.entry(current).or_default().push(OwnershipChain {
                                path: path.clone(),
                                effective_pct: cumulative_pct,
                            });
                            continue;
                        }
                        let mut new_path = path.clone();
                        new_path.push(*owner_id);
                        let new_pct = cumulative_pct * pct / 100.0;
                        stack.push((*owner_id, new_path, new_pct));
                    }
                }
            }

            for (owner_id, chains) in owner_chains {
                let total_pct: f64 = chains.iter().map(|c| c.effective_pct).sum();
                let chain_count = chains.len() as i32;
                total_chains += chain_count;
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

        all_candidates.sort_by(|a, b| {
            b.total_ownership_pct
                .partial_cmp(&a.total_ownership_pct)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let candidates_found = all_candidates.len() as i32;
        let output_snapshot = serde_json::to_value(&all_candidates)?;
        let chains_snapshot = serde_json::to_value(
            all_candidates
                .iter()
                .flat_map(|c| {
                    c.chains.iter().map(move |chain| {
                        json!({
                            "owner_entity_id": c.entity_id,
                            "path": chain.path,
                            "effective_pct": chain.effective_pct,
                        })
                    })
                })
                .collect::<Vec<_>>(),
        )?;
        let computation_ms = start.elapsed().as_millis() as i32;
        let primary_subject = subject_entity_ids
            .first()
            .copied()
            .ok_or_else(|| anyhow!("No subject entities"))?;

        let run_id: Uuid = sqlx::query_scalar(
            r#"INSERT INTO "ob-poc".ubo_determination_runs (
                   subject_entity_id, case_id, as_of, config_version, threshold_pct,
                   candidates_found, output_snapshot, chains_snapshot, coverage_snapshot,
                   computed_at, computed_by, computation_ms
               ) VALUES ($1, $2, CURRENT_DATE, $3, $4, $5, $6, $7, NULL, NOW(), 'ubo.compute-chains', $8)
               RETURNING run_id"#,
        )
        .bind(primary_subject)
        .bind(case_id)
        .bind(&config_version)
        .bind(
            rust_decimal::Decimal::from_f64_retain(threshold_pct)
                .unwrap_or_else(|| rust_decimal::Decimal::new(500, 2)),
        )
        .bind(candidates_found)
        .bind(&output_snapshot)
        .bind(&chains_snapshot)
        .bind(computation_ms)
        .fetch_one(scope.executor())
        .await?;

        Ok(VerbExecutionOutcome::Record(json!({
            "run_id": run_id,
            "case_id": case_id,
            "candidates_found": candidates_found,
            "chains_computed": total_chains,
            "threshold_pct": threshold_pct,
            "candidates": all_candidates,
        })))
    }
}

// ── ubo.snapshot.capture ──────────────────────────────────────────────────────

pub struct SnapshotCapture;

#[async_trait]
impl SemOsVerbOp for SnapshotCapture {
    fn fqn(&self) -> &str {
        "ubo.snapshot.capture"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        use sha2::{Digest, Sha256};
        let case_id = json_extract_uuid(args, ctx, "case-id")?;
        let run_id = json_extract_uuid(args, ctx, "determination-run-id")?;

        let mut hasher = Sha256::new();
        hasher.update(b"ubo_compute_v1");
        let code_hash = format!("{:x}", hasher.finalize());
        let config_version = "1.0.0".to_string();

        sqlx::query(
            r#"UPDATE "ob-poc".ubo_determination_runs
               SET output_snapshot = (
                     SELECT jsonb_agg(jsonb_build_object(
                       'ubo_id', ur.ubo_id,
                       'entity_id', ur.entity_id,
                       'status', ur.status,
                       'ownership_pct', ur.ownership_pct
                     ))
                     FROM "ob-poc".kyc_ubo_registry ur
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
                     JOIN "ob-poc".entity_workstreams ew ON er.to_entity_id = ew.entity_id
                     WHERE ew.case_id = $2
                       AND er.relationship_type = 'OWNERSHIP'
                       AND er.effective_to IS NULL
                   ),
                   code_hash = $3,
                   config_version = $4,
                   completed_at = NOW()
               WHERE run_id = $1"#,
        )
        .bind(run_id)
        .bind(case_id)
        .bind(&code_hash)
        .bind(&config_version)
        .execute(scope.executor())
        .await?;

        let (candidates_captured,): (Option<i64>,) = sqlx::query_as(
            r#"SELECT jsonb_array_length(output_snapshot)::bigint
               FROM "ob-poc".ubo_determination_runs WHERE run_id = $1"#,
        )
        .bind(run_id)
        .fetch_one(scope.executor())
        .await?;
        let (chains_captured,): (Option<i64>,) = sqlx::query_as(
            r#"SELECT jsonb_array_length(chains_snapshot)::bigint
               FROM "ob-poc".ubo_determination_runs WHERE run_id = $1"#,
        )
        .bind(run_id)
        .fetch_one(scope.executor())
        .await?;

        Ok(VerbExecutionOutcome::Record(json!({
            "run_id": run_id,
            "code_hash": code_hash,
            "config_version": config_version,
            "candidates_captured": candidates_captured.unwrap_or(0),
            "chains_captured": chains_captured.unwrap_or(0),
        })))
    }
}

// ── ubo.snapshot.diff ─────────────────────────────────────────────────────────

pub struct SnapshotDiff;

#[derive(Debug, Clone, Deserialize)]
struct SnapshotCandidate {
    entity_id: Uuid,
    status: Option<String>,
    ownership_pct: Option<f64>,
}

#[async_trait]
impl SemOsVerbOp for SnapshotDiff {
    fn fqn(&self) -> &str {
        "ubo.snapshot.diff"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let run_id_a = json_extract_uuid(args, ctx, "run-id-a")?;
        let run_id_b = json_extract_uuid(args, ctx, "run-id-b")?;

        let run_ids = vec![run_id_a, run_id_b];
        let rows: Vec<(Uuid, Option<Value>, Option<Value>)> = sqlx::query_as(
            r#"SELECT run_id, output_snapshot, chains_snapshot
               FROM "ob-poc".ubo_determination_runs
               WHERE run_id = ANY($1)"#,
        )
        .bind(&run_ids)
        .fetch_all(scope.executor())
        .await?;

        let snap_a = rows
            .iter()
            .find(|(rid, _, _)| *rid == run_id_a)
            .ok_or_else(|| anyhow!("Determination run {} not found", run_id_a))?;
        let snap_b = rows
            .iter()
            .find(|(rid, _, _)| *rid == run_id_b)
            .ok_or_else(|| anyhow!("Determination run {} not found", run_id_b))?;

        let parse_candidates = |snapshot: &Option<Value>| -> Result<HashMap<Uuid, SnapshotCandidate>> {
            let mut map = HashMap::new();
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

        let mut added: Vec<Uuid> = Vec::new();
        let mut removed: Vec<Uuid> = Vec::new();
        let mut changed: Vec<Value> = Vec::new();

        for entity_id in map_b.keys() {
            if !map_a.contains_key(entity_id) {
                added.push(*entity_id);
            }
        }
        for entity_id in map_a.keys() {
            if !map_b.contains_key(entity_id) {
                removed.push(*entity_id);
            }
        }
        for (entity_id, cand_a) in &map_a {
            if let Some(cand_b) = map_b.get(entity_id) {
                if cand_a.status != cand_b.status {
                    changed.push(json!({
                        "entity_id": entity_id,
                        "field": "status",
                        "old_value": cand_a.status.clone().unwrap_or_else(|| "null".into()),
                        "new_value": cand_b.status.clone().unwrap_or_else(|| "null".into()),
                    }));
                }
                let pct_a = cand_a.ownership_pct.unwrap_or(0.0);
                let pct_b = cand_b.ownership_pct.unwrap_or(0.0);
                if (pct_a - pct_b).abs() > 0.001 {
                    changed.push(json!({
                        "entity_id": entity_id,
                        "field": "ownership_pct",
                        "old_value": format!("{:.4}", pct_a),
                        "new_value": format!("{:.4}", pct_b),
                    }));
                }
            }
        }
        added.sort();
        removed.sort();
        changed.sort_by(|a, b| {
            let ea = a.get("entity_id").and_then(|v| v.as_str()).unwrap_or("");
            let eb = b.get("entity_id").and_then(|v| v.as_str()).unwrap_or("");
            let fa = a.get("field").and_then(|v| v.as_str()).unwrap_or("");
            let fb = b.get("field").and_then(|v| v.as_str()).unwrap_or("");
            ea.cmp(eb).then(fa.cmp(fb))
        });

        Ok(VerbExecutionOutcome::Record(json!({
            "run_id_a": run_id_a,
            "run_id_b": run_id_b,
            "added": added,
            "removed": removed,
            "changed": changed,
        })))
    }
}
