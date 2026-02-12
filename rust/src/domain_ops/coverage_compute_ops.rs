//! Coverage Computation Operations (Phase 2.4)
//!
//! Computes per-prong edge/evidence coverage for a UBO determination run,
//! generates stable gap identifiers, checks blocking-at-gate conditions,
//! and persists the result as JSONB in `kyc.ubo_determination_runs.coverage_snapshot`.
//!
//! ## Coverage Prongs
//!
//! Each UBO candidate/edge is checked across four prongs:
//! - **OWNERSHIP**: has ownership edges with percentages documented
//! - **IDENTITY**: has identity documents verified for the UBO person
//! - **CONTROL**: has control edges documented in the graph
//! - **SOURCE_OF_WEALTH**: has source of wealth evidence collected
//!
//! ## Gap Identifiers
//!
//! Stable gap IDs follow the pattern:
//! - Edge gaps: `"{relationship_id}:{prong}"`
//! - Entity gaps: `"{entity_id}:{prong}"`
//!
//! ## Blocking-at-Gate
//!
//! The SKELETON_READY tollgate requires the OWNERSHIP prong to be covered.
//! EVIDENCE_COMPLETE requires all four prongs. Gaps annotated with
//! `blocking_at_gate` indicate which tollgate they block.
//!
//! ## Spec References
//!
//! - KYC/UBO Architecture v0.5, section 6.3 (coverage computation)
//! - KYC/UBO Architecture v0.5, section 2A.3 (prong model)

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use ob_poc_macros::register_custom_op;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[cfg(feature = "database")]
use sqlx::PgPool;

use super::helpers::extract_uuid;
use super::{CustomOperation, ExecutionContext, ExecutionResult, VerbCall};

// =============================================================================
// Result Types
// =============================================================================

/// Top-level result of coverage computation for a determination run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageComputeResult {
    pub run_id: Uuid,
    pub case_id: Uuid,
    pub overall_coverage_pct: f64,
    pub prong_coverage: Vec<ProngCoverage>,
    pub gaps: Vec<CoverageGap>,
    pub gaps_blocking_skeleton: i32,
}

/// Per-prong coverage summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProngCoverage {
    pub prong: String,
    pub covered: i32,
    pub total: i32,
    pub coverage_pct: f64,
}

/// A single coverage gap with stable identifier and gate-blocking annotation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageGap {
    pub gap_id: String,
    pub prong: String,
    pub entity_id: Uuid,
    pub description: String,
    pub blocking_at_gate: Option<String>,
}

// =============================================================================
// Internal helper types (database rows)
// =============================================================================

/// A UBO candidate from the determination run's output_snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct UboCandidate {
    pub entity_id: Uuid,
    pub entity_name: Option<String>,
    pub ubo_type: Option<String>,
    pub effective_ownership: Option<f64>,
}

/// Prong names as constants.
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

// =============================================================================
// CoverageComputeOp
// =============================================================================

/// Compute coverage across KYC prongs for a determination run.
///
/// Loads the determination run's computed candidates/chains, checks each
/// candidate against four prongs (OWNERSHIP, IDENTITY, CONTROL,
/// SOURCE_OF_WEALTH), generates stable gap IDs, annotates blocking gates,
/// and persists the result to `coverage_snapshot`.
#[register_custom_op]
pub struct CoverageComputeOp;

#[async_trait]
impl CustomOperation for CoverageComputeOp {
    fn domain(&self) -> &'static str {
        "coverage"
    }

    fn verb(&self) -> &'static str {
        "compute"
    }

    fn rationale(&self) -> &'static str {
        "Coverage computation requires aggregation across ownership edges, evidence records, \
         control edges, and identity documents for each UBO candidate, with gap ID generation \
         and tollgate blocking logic"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let case_id = extract_uuid(verb_call, ctx, "case-id")?;
        let determination_run_id = extract_uuid(verb_call, ctx, "determination-run-id")?;

        // ------------------------------------------------------------------
        // 1. Load the determination run
        // ------------------------------------------------------------------
        let run_row: Option<(Uuid, serde_json::Value, Option<serde_json::Value>)> = sqlx::query_as(
            r#"
                SELECT subject_entity_id, output_snapshot, chains_snapshot
                FROM kyc.ubo_determination_runs
                WHERE run_id = $1
                  AND case_id = $2
                "#,
        )
        .bind(determination_run_id)
        .bind(case_id)
        .fetch_optional(pool)
        .await?;

        let (subject_entity_id, output_snapshot, chains_snapshot) = run_row.ok_or_else(|| {
            anyhow!(
                "Determination run {} not found for case {}",
                determination_run_id,
                case_id
            )
        })?;

        // ------------------------------------------------------------------
        // 2. Extract UBO candidates from output_snapshot
        // ------------------------------------------------------------------
        let candidates = extract_candidates(&output_snapshot)?;

        if candidates.is_empty() {
            // No candidates means 100% coverage (nothing to cover)
            let result = CoverageComputeResult {
                run_id: determination_run_id,
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

            persist_coverage_snapshot(pool, determination_run_id, &result).await?;

            return Ok(ExecutionResult::Record(serde_json::to_value(result)?));
        }

        // ------------------------------------------------------------------
        // 3. Check coverage across prongs for each candidate
        // ------------------------------------------------------------------
        let mut all_gaps: Vec<CoverageGap> = Vec::new();
        let mut prong_totals: std::collections::HashMap<&str, (i32, i32)> =
            std::collections::HashMap::new();

        for prong in ALL_PRONGS {
            prong_totals.insert(prong, (0, 0)); // (covered, total)
        }

        for candidate in &candidates {
            let entity_id = candidate.entity_id;

            // --- OWNERSHIP prong ---
            // Check if there are ownership edges with percentages for this entity
            let ownership_covered =
                check_ownership_prong(pool, subject_entity_id, entity_id, &chains_snapshot).await?;
            update_prong_count(&mut prong_totals, PRONG_OWNERSHIP, ownership_covered);
            if !ownership_covered {
                all_gaps.push(CoverageGap {
                    gap_id: format!("{}:{}", entity_id, PRONG_OWNERSHIP),
                    prong: PRONG_OWNERSHIP.to_string(),
                    entity_id,
                    description: format!(
                        "Missing ownership edges with percentages for {}",
                        candidate.entity_name.as_deref().unwrap_or("unknown entity")
                    ),
                    blocking_at_gate: Some("SKELETON_READY".to_string()),
                });
            }

            // --- IDENTITY prong ---
            // Check if identity documents are verified for this UBO person
            let identity_covered = check_identity_prong(pool, entity_id, case_id).await?;
            update_prong_count(&mut prong_totals, PRONG_IDENTITY, identity_covered);
            if !identity_covered {
                all_gaps.push(CoverageGap {
                    gap_id: format!("{}:{}", entity_id, PRONG_IDENTITY),
                    prong: PRONG_IDENTITY.to_string(),
                    entity_id,
                    description: format!(
                        "Missing verified identity document for {}",
                        candidate.entity_name.as_deref().unwrap_or("unknown entity")
                    ),
                    blocking_at_gate: Some("EVIDENCE_COMPLETE".to_string()),
                });
            }

            // --- CONTROL prong ---
            // Check if control edges are documented for this entity
            let control_covered = check_control_prong(pool, subject_entity_id, entity_id).await?;
            update_prong_count(&mut prong_totals, PRONG_CONTROL, control_covered);
            if !control_covered {
                // Control gaps also check edge-level gaps
                let edge_gaps =
                    find_edge_gaps_for_control(pool, subject_entity_id, entity_id).await?;
                if edge_gaps.is_empty() {
                    all_gaps.push(CoverageGap {
                        gap_id: format!("{}:{}", entity_id, PRONG_CONTROL),
                        prong: PRONG_CONTROL.to_string(),
                        entity_id,
                        description: format!(
                            "No control relationship documented for {}",
                            candidate.entity_name.as_deref().unwrap_or("unknown entity")
                        ),
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

            // --- SOURCE_OF_WEALTH prong ---
            // Check if source of wealth evidence exists for this UBO
            let sow_covered = check_source_of_wealth_prong(pool, entity_id, case_id).await?;
            update_prong_count(&mut prong_totals, PRONG_SOURCE_OF_WEALTH, sow_covered);
            if !sow_covered {
                all_gaps.push(CoverageGap {
                    gap_id: format!("{}:{}", entity_id, PRONG_SOURCE_OF_WEALTH),
                    prong: PRONG_SOURCE_OF_WEALTH.to_string(),
                    entity_id,
                    description: format!(
                        "Missing source of wealth evidence for {}",
                        candidate.entity_name.as_deref().unwrap_or("unknown entity")
                    ),
                    blocking_at_gate: Some("EVIDENCE_COMPLETE".to_string()),
                });
            }
        }

        // ------------------------------------------------------------------
        // 4. Build prong coverage summaries
        // ------------------------------------------------------------------
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

        // Overall coverage: average across prongs (weighted equally)
        let overall_coverage_pct = if prong_coverage.is_empty() {
            100.0
        } else {
            let sum: f64 = prong_coverage.iter().map(|p| p.coverage_pct).sum();
            sum / prong_coverage.len() as f64
        };

        // Count gaps blocking SKELETON_READY
        let gaps_blocking_skeleton = all_gaps
            .iter()
            .filter(|g| g.blocking_at_gate.as_deref() == Some("SKELETON_READY"))
            .count() as i32;

        // ------------------------------------------------------------------
        // 5. Build result and persist
        // ------------------------------------------------------------------
        let result = CoverageComputeResult {
            run_id: determination_run_id,
            case_id,
            overall_coverage_pct,
            prong_coverage,
            gaps: all_gaps,
            gaps_blocking_skeleton,
        };

        persist_coverage_snapshot(pool, determination_run_id, &result).await?;

        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("Database feature required"))
    }
}

// =============================================================================
// Helpers
// =============================================================================

/// Extract UBO candidates from the determination run's output_snapshot JSON.
///
/// The output_snapshot is expected to contain a `candidates` array with objects
/// having at least `entity_id`. Falls back to `ubos` array for backward compat.
fn extract_candidates(output_snapshot: &serde_json::Value) -> Result<Vec<UboCandidate>> {
    // Try `candidates` array first, then `ubos` for backward compat
    let arr = output_snapshot
        .get("candidates")
        .or_else(|| output_snapshot.get("ubos"))
        .and_then(|v| v.as_array());

    let Some(arr) = arr else {
        // If the snapshot is itself an array, try that
        if let Some(arr) = output_snapshot.as_array() {
            return parse_candidate_array(arr);
        }
        return Ok(vec![]);
    };

    parse_candidate_array(arr)
}

fn parse_candidate_array(arr: &[serde_json::Value]) -> Result<Vec<UboCandidate>> {
    let mut candidates = Vec::with_capacity(arr.len());
    for item in arr {
        // entity_id can be at top level or nested under ubo_person_id
        let entity_id_str = item
            .get("entity_id")
            .or_else(|| item.get("ubo_person_id"))
            .and_then(|v| v.as_str());

        let entity_id = match entity_id_str {
            Some(s) => Uuid::parse_str(s)
                .map_err(|e| anyhow!("Invalid UUID in output_snapshot candidate: {}: {}", s, e))?,
            None => continue, // skip entries without a valid entity_id
        };

        let entity_name = item
            .get("entity_name")
            .or_else(|| item.get("ubo_name"))
            .and_then(|v| v.as_str())
            .map(String::from);

        let ubo_type = item
            .get("ubo_type")
            .and_then(|v| v.as_str())
            .map(String::from);

        let effective_ownership = item.get("effective_ownership").and_then(|v| v.as_f64());

        candidates.push(UboCandidate {
            entity_id,
            entity_name,
            ubo_type,
            effective_ownership,
        });
    }
    Ok(candidates)
}

/// Update the (covered, total) counter for a prong.
fn update_prong_count(
    totals: &mut std::collections::HashMap<&str, (i32, i32)>,
    prong: &str,
    is_covered: bool,
) {
    if let Some(counts) = totals.get_mut(prong) {
        counts.1 += 1; // total
        if is_covered {
            counts.0 += 1; // covered
        }
    }
}

// =============================================================================
// Per-prong checks (database-gated)
// =============================================================================

/// OWNERSHIP prong: check if ownership edges with percentages exist
/// linking the subject entity to the candidate through any chain.
///
/// Checks both direct relationships in entity_relationships and
/// chain data from chains_snapshot if available.
#[cfg(feature = "database")]
async fn check_ownership_prong(
    pool: &PgPool,
    subject_entity_id: Uuid,
    candidate_entity_id: Uuid,
    chains_snapshot: &Option<serde_json::Value>,
) -> Result<bool> {
    // Check 1: Direct ownership edges from candidate to subject with percentage
    let edge_count: (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*)
        FROM "ob-poc".entity_relationships
        WHERE from_entity_id = $1
          AND to_entity_id = $2
          AND relationship_type = 'ownership'
          AND percentage IS NOT NULL
          AND (effective_to IS NULL OR effective_to > CURRENT_DATE)
        "#,
    )
    .bind(candidate_entity_id)
    .bind(subject_entity_id)
    .fetch_one(pool)
    .await?;

    if edge_count.0 > 0 {
        return Ok(true);
    }

    // Check 2: Any ownership relationship chain involving the candidate
    // (candidate as from_entity_id with any percentage)
    let indirect_count: (i64,) = sqlx::query_as(
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
    .fetch_one(pool)
    .await?;

    if indirect_count.0 > 0 {
        return Ok(true);
    }

    // Check 3: Chain data in chains_snapshot referencing this candidate
    if let Some(chains) = chains_snapshot {
        if let Some(chain_arr) = chains.get("chains").and_then(|v| v.as_array()) {
            for chain in chain_arr {
                let person_id = chain
                    .get("ubo_person_id")
                    .and_then(|v| v.as_str())
                    .and_then(|s| Uuid::parse_str(s).ok());

                if person_id == Some(candidate_entity_id) {
                    // Chain exists with this candidate as UBO
                    if chain
                        .get("effective_ownership")
                        .and_then(|v| v.as_f64())
                        .is_some()
                    {
                        return Ok(true);
                    }
                }
            }
        }
    }

    Ok(false)
}

/// IDENTITY prong: check if verified identity documents exist for the UBO person.
///
/// Checks both `kyc.ubo_evidence` (type IDENTITY_DOC, status VERIFIED) and
/// `kyc.entity_workstreams.identity_verified` flag.
#[cfg(feature = "database")]
async fn check_identity_prong(pool: &PgPool, entity_id: Uuid, case_id: Uuid) -> Result<bool> {
    // Check 1: Verified identity evidence in ubo_evidence
    let evidence_count: (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*)
        FROM kyc.ubo_evidence ue
        JOIN kyc.ubo_registry ur ON ur.ubo_id = ue.ubo_id
        WHERE ur.ubo_person_id = $1
          AND ur.case_id = $2
          AND ue.evidence_type IN ('IDENTITY_DOC', 'PROOF_OF_ADDRESS')
          AND ue.status = 'VERIFIED'
        "#,
    )
    .bind(entity_id)
    .bind(case_id)
    .fetch_one(pool)
    .await?;

    if evidence_count.0 > 0 {
        return Ok(true);
    }

    // Check 2: identity_verified flag on workstream
    let ws_verified: Option<(bool,)> = sqlx::query_as(
        r#"
        SELECT identity_verified
        FROM kyc.entity_workstreams
        WHERE entity_id = $1
          AND case_id = $2
          AND identity_verified = true
        LIMIT 1
        "#,
    )
    .bind(entity_id)
    .bind(case_id)
    .fetch_optional(pool)
    .await?;

    Ok(ws_verified.is_some())
}

/// CONTROL prong: check if control edges are documented for this candidate
/// in relation to the subject entity.
///
/// Checks entity_relationships for relationship_type = 'control' or
/// control_type IS NOT NULL.
#[cfg(feature = "database")]
async fn check_control_prong(
    pool: &PgPool,
    _subject_entity_id: Uuid,
    candidate_entity_id: Uuid,
) -> Result<bool> {
    let control_count: (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*)
        FROM "ob-poc".entity_relationships
        WHERE from_entity_id = $1
          AND (
              relationship_type = 'control'
              OR control_type IS NOT NULL
          )
          AND (effective_to IS NULL OR effective_to > CURRENT_DATE)
        "#,
    )
    .bind(candidate_entity_id)
    .fetch_one(pool)
    .await?;

    Ok(control_count.0 > 0)
}

/// Find specific edge-level gaps for the CONTROL prong.
///
/// Returns (relationship_id, description) pairs for edges that exist
/// but lack control documentation.
#[cfg(feature = "database")]
async fn find_edge_gaps_for_control(
    pool: &PgPool,
    _subject_entity_id: Uuid,
    candidate_entity_id: Uuid,
) -> Result<Vec<(Uuid, String)>> {
    let rows: Vec<(Uuid, String, Option<String>)> = sqlx::query_as(
        r#"
        SELECT r.relationship_id,
               r.relationship_type,
               r.control_type
        FROM "ob-poc".entity_relationships r
        WHERE r.from_entity_id = $1
          AND r.relationship_type = 'ownership'
          AND r.control_type IS NULL
          AND (r.effective_to IS NULL OR r.effective_to > CURRENT_DATE)
        "#,
    )
    .bind(candidate_entity_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(rel_id, _rel_type, _)| {
            (
                rel_id,
                format!("Ownership edge {} has no control_type documented", rel_id),
            )
        })
        .collect())
}

/// SOURCE_OF_WEALTH prong: check if source of wealth evidence exists.
///
/// Checks `kyc.ubo_evidence` for evidence_type in (SOURCE_OF_WEALTH,
/// SOURCE_OF_FUNDS) with status VERIFIED or RECEIVED.
#[cfg(feature = "database")]
async fn check_source_of_wealth_prong(
    pool: &PgPool,
    entity_id: Uuid,
    case_id: Uuid,
) -> Result<bool> {
    let sow_count: (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*)
        FROM kyc.ubo_evidence ue
        JOIN kyc.ubo_registry ur ON ur.ubo_id = ue.ubo_id
        WHERE ur.ubo_person_id = $1
          AND ur.case_id = $2
          AND ue.evidence_type IN ('SOURCE_OF_WEALTH', 'SOURCE_OF_FUNDS',
                                   'ANNUAL_RETURN', 'CHAIN_PROOF')
          AND ue.status IN ('VERIFIED', 'RECEIVED')
        "#,
    )
    .bind(entity_id)
    .bind(case_id)
    .fetch_one(pool)
    .await?;

    Ok(sow_count.0 > 0)
}

/// Persist the coverage result as JSONB into `coverage_snapshot`.
#[cfg(feature = "database")]
async fn persist_coverage_snapshot(
    pool: &PgPool,
    run_id: Uuid,
    result: &CoverageComputeResult,
) -> Result<()> {
    let snapshot_json = serde_json::to_value(result)?;

    sqlx::query(
        r#"
        UPDATE kyc.ubo_determination_runs
        SET coverage_snapshot = $2
        WHERE run_id = $1
        "#,
    )
    .bind(run_id)
    .bind(snapshot_json)
    .execute(pool)
    .await?;

    Ok(())
}
