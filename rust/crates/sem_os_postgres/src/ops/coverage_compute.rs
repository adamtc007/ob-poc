//! Coverage-compute verb (1 plugin verb) — SemOS-side YAML-first
//! re-implementation of the plugin subset of
//! `rust/config/verbs/coverage.yaml` (verb `compute`).
//!
//! Computes per-prong edge/evidence coverage for a UBO
//! determination run, generates stable gap IDs, checks
//! blocking-at-gate conditions, and persists the result as JSONB
//! in `"ob-poc".ubo_determination_runs.coverage_snapshot`.
//!
//! Prongs: OWNERSHIP, IDENTITY, CONTROL, SOURCE_OF_WEALTH.
//! Gate blocking: SKELETON_READY requires OWNERSHIP covered,
//! EVIDENCE_COMPLETE requires all four prongs covered.

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use dsl_runtime::domain_ops::helpers::json_extract_uuid;
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::SemOsVerbOp;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CoverageComputeResult {
    run_id: Uuid,
    case_id: Uuid,
    overall_coverage_pct: f64,
    prong_coverage: Vec<ProngCoverage>,
    gaps: Vec<CoverageGap>,
    gaps_blocking_skeleton: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProngCoverage {
    prong: String,
    covered: i32,
    total: i32,
    coverage_pct: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CoverageGap {
    gap_id: String,
    prong: String,
    entity_id: Uuid,
    description: String,
    blocking_at_gate: Option<String>,
}

#[derive(Debug, Clone)]
struct UboCandidate {
    entity_id: Uuid,
    entity_name: Option<String>,
}

const PRONG_OWNERSHIP: &str = "OWNERSHIP";
const PRONG_IDENTITY: &str = "IDENTITY";
const PRONG_CONTROL: &str = "CONTROL";
const PRONG_SOURCE_OF_WEALTH: &str = "SOURCE_OF_WEALTH";
const ALL_PRONGS: &[&str] = &[
    PRONG_OWNERSHIP,
    PRONG_IDENTITY,
    PRONG_CONTROL,
    PRONG_SOURCE_OF_WEALTH,
];

// ── coverage.compute ──────────────────────────────────────────────────────────

pub struct Compute;

#[async_trait]
impl SemOsVerbOp for Compute {
    fn fqn(&self) -> &str {
        "coverage.compute"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let case_id = json_extract_uuid(args, ctx, "case-id")?;
        let run_id = json_extract_uuid(args, ctx, "determination-run-id")?;

        let run_row: Option<(Uuid, Value, Option<Value>)> = sqlx::query_as(
            r#"
            SELECT subject_entity_id, output_snapshot, chains_snapshot
            FROM "ob-poc".ubo_determination_runs
            WHERE run_id = $1 AND case_id = $2
            "#,
        )
        .bind(run_id)
        .bind(case_id)
        .fetch_optional(scope.executor())
        .await?;

        let (subject_entity_id, output_snapshot, chains_snapshot) = run_row
            .ok_or_else(|| anyhow!("Determination run {} not found for case {}", run_id, case_id))?;

        let candidates = extract_candidates(&output_snapshot)?;

        if candidates.is_empty() {
            let result = CoverageComputeResult {
                run_id,
                case_id,
                overall_coverage_pct: 100.0,
                prong_coverage: ALL_PRONGS
                    .iter()
                    .map(|p| ProngCoverage {
                        prong: p.to_string(),
                        covered: 0,
                        total: 0,
                        coverage_pct: 100.0,
                    })
                    .collect(),
                gaps: vec![],
                gaps_blocking_skeleton: 0,
            };
            persist_snapshot(scope, run_id, &result).await?;
            return Ok(VerbExecutionOutcome::Record(serde_json::to_value(result)?));
        }

        let mut all_gaps: Vec<CoverageGap> = Vec::new();
        let mut prong_totals: HashMap<&str, (i32, i32)> =
            ALL_PRONGS.iter().map(|p| (*p, (0, 0))).collect();

        for candidate in &candidates {
            let entity_id = candidate.entity_id;
            let label = candidate.entity_name.as_deref().unwrap_or("unknown entity");

            let ownership_covered =
                check_ownership_prong(scope, subject_entity_id, entity_id, &chains_snapshot)
                    .await?;
            bump(&mut prong_totals, PRONG_OWNERSHIP, ownership_covered);
            if !ownership_covered {
                all_gaps.push(CoverageGap {
                    gap_id: format!("{}:{}", entity_id, PRONG_OWNERSHIP),
                    prong: PRONG_OWNERSHIP.to_string(),
                    entity_id,
                    description: format!("Missing ownership edges with percentages for {}", label),
                    blocking_at_gate: Some("SKELETON_READY".to_string()),
                });
            }

            let identity_covered = check_identity_prong(scope, entity_id, case_id).await?;
            bump(&mut prong_totals, PRONG_IDENTITY, identity_covered);
            if !identity_covered {
                all_gaps.push(CoverageGap {
                    gap_id: format!("{}:{}", entity_id, PRONG_IDENTITY),
                    prong: PRONG_IDENTITY.to_string(),
                    entity_id,
                    description: format!("Missing verified identity document for {}", label),
                    blocking_at_gate: Some("EVIDENCE_COMPLETE".to_string()),
                });
            }

            let control_covered = check_control_prong(scope, entity_id).await?;
            bump(&mut prong_totals, PRONG_CONTROL, control_covered);
            if !control_covered {
                let edge_gaps = find_edge_gaps_for_control(scope, entity_id).await?;
                if edge_gaps.is_empty() {
                    all_gaps.push(CoverageGap {
                        gap_id: format!("{}:{}", entity_id, PRONG_CONTROL),
                        prong: PRONG_CONTROL.to_string(),
                        entity_id,
                        description: format!("No control relationship documented for {}", label),
                        blocking_at_gate: Some("EVIDENCE_COMPLETE".to_string()),
                    });
                } else {
                    for (relationship_id, desc) in edge_gaps {
                        all_gaps.push(CoverageGap {
                            gap_id: format!("{}:{}", relationship_id, PRONG_CONTROL),
                            prong: PRONG_CONTROL.to_string(),
                            entity_id,
                            description: desc,
                            blocking_at_gate: Some("EVIDENCE_COMPLETE".to_string()),
                        });
                    }
                }
            }

            let sow_covered = check_source_of_wealth_prong(scope, entity_id, case_id).await?;
            bump(&mut prong_totals, PRONG_SOURCE_OF_WEALTH, sow_covered);
            if !sow_covered {
                all_gaps.push(CoverageGap {
                    gap_id: format!("{}:{}", entity_id, PRONG_SOURCE_OF_WEALTH),
                    prong: PRONG_SOURCE_OF_WEALTH.to_string(),
                    entity_id,
                    description: format!("Missing source of wealth evidence for {}", label),
                    blocking_at_gate: Some("EVIDENCE_COMPLETE".to_string()),
                });
            }
        }

        let prong_coverage: Vec<ProngCoverage> = ALL_PRONGS
            .iter()
            .map(|prong| {
                let (covered, total) = prong_totals.get(prong).copied().unwrap_or((0, 0));
                let coverage_pct = if total > 0 {
                    (covered as f64 / total as f64) * 100.0
                } else {
                    100.0
                };
                ProngCoverage {
                    prong: prong.to_string(),
                    covered,
                    total,
                    coverage_pct,
                }
            })
            .collect();

        let overall_coverage_pct = if prong_coverage.is_empty() {
            100.0
        } else {
            let sum: f64 = prong_coverage.iter().map(|p| p.coverage_pct).sum();
            sum / prong_coverage.len() as f64
        };

        let gaps_blocking_skeleton = all_gaps
            .iter()
            .filter(|g| g.blocking_at_gate.as_deref() == Some("SKELETON_READY"))
            .count() as i32;

        let result = CoverageComputeResult {
            run_id,
            case_id,
            overall_coverage_pct,
            prong_coverage,
            gaps: all_gaps,
            gaps_blocking_skeleton,
        };
        persist_snapshot(scope, run_id, &result).await?;
        Ok(VerbExecutionOutcome::Record(serde_json::to_value(result)?))
    }
}

fn extract_candidates(output_snapshot: &Value) -> Result<Vec<UboCandidate>> {
    let arr = output_snapshot
        .get("candidates")
        .or_else(|| output_snapshot.get("ubos"))
        .and_then(|v| v.as_array());

    let Some(arr) = arr else {
        if let Some(arr) = output_snapshot.as_array() {
            return parse_candidate_array(arr);
        }
        return Ok(vec![]);
    };
    parse_candidate_array(arr)
}

fn parse_candidate_array(arr: &[Value]) -> Result<Vec<UboCandidate>> {
    let mut candidates = Vec::with_capacity(arr.len());
    for item in arr {
        let entity_id_str = item
            .get("entity_id")
            .or_else(|| item.get("ubo_person_id"))
            .and_then(|v| v.as_str());
        let entity_id = match entity_id_str {
            Some(s) => Uuid::parse_str(s)
                .map_err(|e| anyhow!("Invalid UUID in output_snapshot candidate: {}: {}", s, e))?,
            None => continue,
        };
        let entity_name = item
            .get("entity_name")
            .or_else(|| item.get("ubo_name"))
            .and_then(|v| v.as_str())
            .map(String::from);
        candidates.push(UboCandidate {
            entity_id,
            entity_name,
        });
    }
    Ok(candidates)
}

fn bump(totals: &mut HashMap<&str, (i32, i32)>, prong: &str, is_covered: bool) {
    if let Some(counts) = totals.get_mut(prong) {
        counts.1 += 1;
        if is_covered {
            counts.0 += 1;
        }
    }
}

async fn check_ownership_prong(
    scope: &mut dyn TransactionScope,
    subject_entity_id: Uuid,
    candidate_entity_id: Uuid,
    chains_snapshot: &Option<Value>,
) -> Result<bool> {
    let (direct,): (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*)
        FROM "ob-poc".entity_relationships
        WHERE from_entity_id = $1 AND to_entity_id = $2
          AND relationship_type = 'ownership'
          AND percentage IS NOT NULL
          AND (effective_to IS NULL OR effective_to > CURRENT_DATE)
        "#,
    )
    .bind(candidate_entity_id)
    .bind(subject_entity_id)
    .fetch_one(scope.executor())
    .await?;
    if direct > 0 {
        return Ok(true);
    }

    let (indirect,): (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*)
        FROM "ob-poc".entity_relationships
        WHERE from_entity_id = $1
          AND relationship_type = 'ownership'
          AND percentage IS NOT NULL
          AND (effective_to IS NULL OR effective_to > CURRENT_DATE)
        "#,
    )
    .bind(candidate_entity_id)
    .fetch_one(scope.executor())
    .await?;
    if indirect > 0 {
        return Ok(true);
    }

    if let Some(chains) = chains_snapshot {
        if let Some(chain_arr) = chains.get("chains").and_then(|v| v.as_array()) {
            for chain in chain_arr {
                let person_id = chain
                    .get("ubo_person_id")
                    .and_then(|v| v.as_str())
                    .and_then(|s| Uuid::parse_str(s).ok());
                if person_id == Some(candidate_entity_id)
                    && chain
                        .get("effective_ownership")
                        .and_then(|v| v.as_f64())
                        .is_some()
                {
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
}

async fn check_identity_prong(
    scope: &mut dyn TransactionScope,
    entity_id: Uuid,
    case_id: Uuid,
) -> Result<bool> {
    let (evidence_count,): (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*)
        FROM "ob-poc".kyc_ubo_evidence ue
        JOIN "ob-poc".kyc_ubo_registry ur ON ur.ubo_id = ue.ubo_id
        WHERE ur.ubo_person_id = $1 AND ur.case_id = $2
          AND ue.evidence_type IN ('IDENTITY_DOC', 'PROOF_OF_ADDRESS')
          AND ue.status = 'VERIFIED'
        "#,
    )
    .bind(entity_id)
    .bind(case_id)
    .fetch_one(scope.executor())
    .await?;
    if evidence_count > 0 {
        return Ok(true);
    }

    let ws_verified: Option<(bool,)> = sqlx::query_as(
        r#"
        SELECT identity_verified
        FROM "ob-poc".entity_workstreams
        WHERE entity_id = $1 AND case_id = $2
          AND identity_verified = true
        LIMIT 1
        "#,
    )
    .bind(entity_id)
    .bind(case_id)
    .fetch_optional(scope.executor())
    .await?;
    Ok(ws_verified.is_some())
}

async fn check_control_prong(
    scope: &mut dyn TransactionScope,
    candidate_entity_id: Uuid,
) -> Result<bool> {
    let (count,): (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*)
        FROM "ob-poc".entity_relationships
        WHERE from_entity_id = $1
          AND (relationship_type = 'control' OR control_type IS NOT NULL)
          AND (effective_to IS NULL OR effective_to > CURRENT_DATE)
        "#,
    )
    .bind(candidate_entity_id)
    .fetch_one(scope.executor())
    .await?;
    Ok(count > 0)
}

async fn find_edge_gaps_for_control(
    scope: &mut dyn TransactionScope,
    candidate_entity_id: Uuid,
) -> Result<Vec<(Uuid, String)>> {
    let rows: Vec<(Uuid, String, Option<String>)> = sqlx::query_as(
        r#"
        SELECT r.relationship_id, r.relationship_type, r.control_type
        FROM "ob-poc".entity_relationships r
        WHERE r.from_entity_id = $1
          AND r.relationship_type = 'ownership'
          AND r.control_type IS NULL
          AND (r.effective_to IS NULL OR r.effective_to > CURRENT_DATE)
        "#,
    )
    .bind(candidate_entity_id)
    .fetch_all(scope.executor())
    .await?;

    Ok(rows
        .into_iter()
        .map(|(rel_id, _, _)| {
            (
                rel_id,
                format!("Ownership edge {} has no control_type documented", rel_id),
            )
        })
        .collect())
}

async fn check_source_of_wealth_prong(
    scope: &mut dyn TransactionScope,
    entity_id: Uuid,
    case_id: Uuid,
) -> Result<bool> {
    let (count,): (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*)
        FROM "ob-poc".kyc_ubo_evidence ue
        JOIN "ob-poc".kyc_ubo_registry ur ON ur.ubo_id = ue.ubo_id
        WHERE ur.ubo_person_id = $1 AND ur.case_id = $2
          AND ue.evidence_type IN ('SOURCE_OF_WEALTH', 'SOURCE_OF_FUNDS',
                                   'ANNUAL_RETURN', 'CHAIN_PROOF')
          AND ue.status IN ('VERIFIED', 'RECEIVED')
        "#,
    )
    .bind(entity_id)
    .bind(case_id)
    .fetch_one(scope.executor())
    .await?;
    Ok(count > 0)
}

async fn persist_snapshot(
    scope: &mut dyn TransactionScope,
    run_id: Uuid,
    result: &CoverageComputeResult,
) -> Result<()> {
    let snapshot_json = serde_json::to_value(result)?;
    sqlx::query(
        r#"
        UPDATE "ob-poc".ubo_determination_runs
        SET coverage_snapshot = $2
        WHERE run_id = $1
        "#,
    )
    .bind(run_id)
    .bind(snapshot_json)
    .execute(scope.executor())
    .await?;
    Ok(())
}
