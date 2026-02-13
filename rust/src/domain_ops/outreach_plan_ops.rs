//! Outreach Plan Generation Operations (Phase 2.5)
//!
//! Generates an outreach plan from coverage gaps produced by a UBO determination run.
//! Groups gaps by prong, maps each gap to the required document type per spec 2A.2,
//! bundles items by entity (max 8 per plan), and inserts into
//! `kyc.outreach_plans` + `kyc.outreach_items`.
//!
//! ## Rationale
//! Plan generation requires custom code because:
//! - Gap-to-document-type mapping is business logic (spec 2A.2)
//! - Bundling by entity with a cap requires aggregation logic
//! - Must read from determination run coverage_snapshot and cross-reference workstreams

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use ob_poc_macros::register_custom_op;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[cfg(feature = "database")]
use sqlx::PgPool;

use super::helpers::{extract_string_opt, extract_uuid};
use super::{CustomOperation, ExecutionContext, ExecutionResult, VerbCall};

// =============================================================================
// Result Types
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutreachPlanResult {
    pub plan_id: Uuid,
    pub case_id: Uuid,
    pub items_count: i32,
    pub items: Vec<OutreachPlanItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutreachPlanItem {
    pub item_id: Uuid,
    pub entity_id: Uuid,
    pub gap_ref: String,
    pub doc_type: String,
    pub status: String,
}

// =============================================================================
// Gap-to-Document-Type Mapping (Spec 2A.2)
// =============================================================================

/// Map a coverage gap prong to the required document type.
///
/// Returns a tuple of (primary_doc_type, fallback_doc_type) per spec 2A.2.
fn gap_prong_to_doc_type(prong: &str) -> (&'static str, &'static str) {
    match prong {
        "OWNERSHIP" => ("SHARE_REGISTER", "OWNERSHIP_CERTIFICATE"),
        "IDENTITY" => ("PASSPORT", "NATIONAL_ID"),
        "CONTROL" => ("BOARD_RESOLUTION", "MANAGEMENT_AGREEMENT"),
        "SOURCE_OF_WEALTH" => (
            "SOURCE_OF_WEALTH_DECLARATION",
            "SOURCE_OF_WEALTH_DECLARATION",
        ),
        // Default to a generic evidence request for unknown prong types
        _ => ("SUPPORTING_EVIDENCE", "SUPPORTING_EVIDENCE"),
    }
}

/// Build a human-readable request text for an outreach item.
fn build_request_text(prong: &str, doc_type: &str, gap_description: &str) -> String {
    match prong {
        "OWNERSHIP" => format!(
            "Please provide {} to evidence ownership. Gap: {}",
            doc_type.to_lowercase().replace('_', " "),
            gap_description
        ),
        "IDENTITY" => format!(
            "Please provide {} for identity verification. Gap: {}",
            doc_type.to_lowercase().replace('_', " "),
            gap_description
        ),
        "CONTROL" => format!(
            "Please provide {} to evidence control arrangements. Gap: {}",
            doc_type.to_lowercase().replace('_', " "),
            gap_description
        ),
        "SOURCE_OF_WEALTH" => format!(
            "Please provide {} for source of wealth verification. Gap: {}",
            doc_type.to_lowercase().replace('_', " "),
            gap_description
        ),
        _ => format!(
            "Please provide {} for verification. Gap: {}",
            doc_type.to_lowercase().replace('_', " "),
            gap_description
        ),
    }
}

/// Priority for a given prong (lower = higher priority).
fn prong_priority(prong: &str) -> i32 {
    match prong {
        "IDENTITY" => 1,
        "OWNERSHIP" => 2,
        "CONTROL" => 3,
        "SOURCE_OF_WEALTH" => 4,
        _ => 5,
    }
}

/// Maximum items per outreach plan.
const MAX_ITEMS_PER_PLAN: usize = 8;

// =============================================================================
// OutreachPlanGenerateOp
// =============================================================================

/// Generate an outreach plan from coverage gaps.
///
/// Takes a case and determination run, reads the coverage gaps,
/// maps each gap to a required document type, bundles by entity,
/// and inserts the plan + items into the database.
#[register_custom_op]
pub struct OutreachPlanGenerateOp;

#[async_trait]
impl CustomOperation for OutreachPlanGenerateOp {
    fn domain(&self) -> &'static str {
        "research.outreach"
    }

    fn verb(&self) -> &'static str {
        "plan-generate"
    }

    fn rationale(&self) -> &'static str {
        "Gap-to-document mapping, entity bundling with cap, and multi-table insert require custom logic"
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
        let doc_preference = extract_string_opt(verb_call, "doc-preference");

        // ---------------------------------------------------------------
        // 1. Validate case exists
        // ---------------------------------------------------------------
        let case_exists: Option<(Uuid,)> =
            sqlx::query_as(r#"SELECT case_id FROM kyc.cases WHERE case_id = $1"#)
                .bind(case_id)
                .fetch_optional(pool)
                .await?;

        if case_exists.is_none() {
            return Err(anyhow!("Case not found: {}", case_id));
        }

        // ---------------------------------------------------------------
        // 2. Load determination run and its coverage snapshot
        // ---------------------------------------------------------------
        #[derive(sqlx::FromRow)]
        struct DeterminationRow {
            coverage_snapshot: Option<serde_json::Value>,
            subject_entity_id: Uuid,
        }

        let det_run: DeterminationRow = sqlx::query_as(
            r#"
            SELECT coverage_snapshot, subject_entity_id
            FROM kyc.ubo_determination_runs
            WHERE run_id = $1 AND case_id = $2
            "#,
        )
        .bind(determination_run_id)
        .bind(case_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| {
            anyhow!(
                "Determination run {} not found for case {}",
                determination_run_id,
                case_id
            )
        })?;

        // ---------------------------------------------------------------
        // 3. Extract gaps from coverage snapshot
        // ---------------------------------------------------------------
        // coverage_snapshot is JSONB with structure like:
        // { "gaps": [ { "prong": "OWNERSHIP", "entity_id": "...", "description": "..." }, ... ] }
        // OR it may be stored as a top-level array.
        let gaps =
            extract_gaps_from_snapshot(&det_run.coverage_snapshot, det_run.subject_entity_id);

        if gaps.is_empty() {
            // No gaps found — create an empty plan
            let plan_id: (Uuid,) = sqlx::query_as(
                r#"
                INSERT INTO kyc.outreach_plans (case_id, determination_run_id, status, total_items)
                VALUES ($1, $2, 'DRAFT', 0)
                RETURNING plan_id
                "#,
            )
            .bind(case_id)
            .bind(determination_run_id)
            .fetch_one(pool)
            .await?;

            let result = OutreachPlanResult {
                plan_id: plan_id.0,
                case_id,
                items_count: 0,
                items: vec![],
            };

            if let Some(binding) = verb_call.binding.as_deref() {
                ctx.bind(binding, plan_id.0);
            }

            return Ok(ExecutionResult::Record(serde_json::to_value(result)?));
        }

        // ---------------------------------------------------------------
        // 4. Map gaps to outreach items (cap at MAX_ITEMS_PER_PLAN)
        // ---------------------------------------------------------------
        let use_primary = doc_preference
            .as_deref()
            .map(|p| p != "fallback")
            .unwrap_or(true);

        let mut planned_items: Vec<PlannedItem> = gaps
            .iter()
            .map(|gap| {
                let (primary, fallback) = gap_prong_to_doc_type(&gap.prong);
                let doc_type = if use_primary { primary } else { fallback };
                let request_text = build_request_text(&gap.prong, doc_type, &gap.description);
                let priority = prong_priority(&gap.prong);
                let gap_ref = format!(
                    "{}:{}",
                    gap.prong,
                    gap.entity_id
                        .map(|id| id.to_string())
                        .unwrap_or_else(|| "subject".to_string())
                );

                PlannedItem {
                    entity_id: gap.entity_id.unwrap_or(det_run.subject_entity_id),
                    prong: gap.prong.clone(),
                    gap_description: gap.description.clone(),
                    doc_type: doc_type.to_string(),
                    request_text,
                    priority,
                    gap_ref,
                }
            })
            .collect();

        // Sort by priority (identity first), then by entity for bundling
        planned_items.sort_by(|a, b| {
            a.priority
                .cmp(&b.priority)
                .then(a.entity_id.cmp(&b.entity_id))
        });

        // Cap at MAX_ITEMS_PER_PLAN
        planned_items.truncate(MAX_ITEMS_PER_PLAN);

        let items_count = planned_items.len() as i32;

        // ---------------------------------------------------------------
        // 5. Insert plan
        // ---------------------------------------------------------------
        let plan_id: (Uuid,) = sqlx::query_as(
            r#"
            INSERT INTO kyc.outreach_plans (case_id, determination_run_id, status, total_items)
            VALUES ($1, $2, 'DRAFT', $3)
            RETURNING plan_id
            "#,
        )
        .bind(case_id)
        .bind(determination_run_id)
        .bind(items_count)
        .fetch_one(pool)
        .await?;

        // ---------------------------------------------------------------
        // 6. Insert items
        // ---------------------------------------------------------------
        let mut result_items: Vec<OutreachPlanItem> = Vec::with_capacity(planned_items.len());

        for item in &planned_items {
            let item_row: (Uuid, String) = sqlx::query_as(
                r#"
                INSERT INTO kyc.outreach_items (
                    plan_id, prong, target_entity_id, gap_description,
                    request_text, doc_type_requested, priority, closes_gap_ref, status
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'PENDING')
                RETURNING item_id, status
                "#,
            )
            .bind(plan_id.0)
            .bind(&item.prong)
            .bind(item.entity_id)
            .bind(&item.gap_description)
            .bind(&item.request_text)
            .bind(&item.doc_type)
            .bind(item.priority)
            .bind(&item.gap_ref)
            .fetch_one(pool)
            .await?;

            result_items.push(OutreachPlanItem {
                item_id: item_row.0,
                entity_id: item.entity_id,
                gap_ref: item.gap_ref.clone(),
                doc_type: item.doc_type.clone(),
                status: item_row.1,
            });
        }

        // ---------------------------------------------------------------
        // 7. Build result
        // ---------------------------------------------------------------
        let result = OutreachPlanResult {
            plan_id: plan_id.0,
            case_id,
            items_count,
            items: result_items,
        };

        if let Some(binding) = verb_call.binding.as_deref() {
            ctx.bind(binding, plan_id.0);
        }

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

// =============================================================================
// Internal Types
// =============================================================================

/// A gap extracted from the determination run coverage snapshot.
#[derive(Debug, Clone)]
struct CoverageGap {
    prong: String,
    entity_id: Option<Uuid>,
    description: String,
}

/// A planned outreach item ready for insertion.
#[derive(Debug, Clone)]
struct PlannedItem {
    entity_id: Uuid,
    prong: String,
    gap_description: String,
    doc_type: String,
    request_text: String,
    priority: i32,
    gap_ref: String,
}

/// Extract coverage gaps from the determination run's coverage_snapshot JSONB.
///
/// The snapshot may contain gaps in several formats:
/// - `{ "gaps": [ { "prong": "...", "entity_id": "...", "description": "..." } ] }`
/// - `{ "coverage_gaps": [ ... ] }`
/// - A top-level array of gap objects
///
/// Handles all formats gracefully, returning an empty vec if none match.
fn extract_gaps_from_snapshot(
    snapshot: &Option<serde_json::Value>,
    subject_entity_id: Uuid,
) -> Vec<CoverageGap> {
    let Some(snapshot) = snapshot else {
        return vec![];
    };

    // Try "gaps" key first, then "coverage_gaps", then top-level array
    let gap_array = snapshot
        .get("gaps")
        .or_else(|| snapshot.get("coverage_gaps"))
        .or(if snapshot.is_array() {
            Some(snapshot)
        } else {
            None
        });

    let Some(arr) = gap_array.and_then(|v| v.as_array()) else {
        // If the snapshot has ownership_coverage_pct but no explicit gaps array,
        // synthesize gaps from workstream-level data
        return synthesize_gaps_from_coverage(snapshot, subject_entity_id);
    };

    arr.iter()
        .map(|item| {
            let prong = item
                .get("prong")
                .or_else(|| item.get("gap_type"))
                .and_then(|v| v.as_str())
                .unwrap_or("OWNERSHIP")
                .to_string();

            let entity_id = item
                .get("entity_id")
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok());

            let description = item
                .get("description")
                .or_else(|| item.get("gap_description"))
                .and_then(|v| v.as_str())
                .unwrap_or("Coverage gap identified")
                .to_string();

            CoverageGap {
                prong,
                entity_id,
                description,
            }
        })
        .collect()
}

/// Synthesize gaps from a coverage snapshot that has aggregate metrics
/// but no explicit gap array (e.g., from older determination runs).
fn synthesize_gaps_from_coverage(
    snapshot: &serde_json::Value,
    subject_entity_id: Uuid,
) -> Vec<CoverageGap> {
    let mut gaps = Vec::new();

    // Check ownership coverage
    if let Some(ownership_pct) = snapshot
        .get("ownership_coverage_pct")
        .and_then(|v| v.as_f64())
    {
        if ownership_pct < 100.0 {
            gaps.push(CoverageGap {
                prong: "OWNERSHIP".to_string(),
                entity_id: Some(subject_entity_id),
                description: format!(
                    "Ownership coverage at {:.1}%, evidence required to close gap",
                    ownership_pct
                ),
            });
        }
    }

    // Check identity coverage
    if let Some(identity_pct) = snapshot
        .get("identity_verified_pct")
        .and_then(|v| v.as_f64())
    {
        if identity_pct < 100.0 {
            gaps.push(CoverageGap {
                prong: "IDENTITY".to_string(),
                entity_id: Some(subject_entity_id),
                description: format!(
                    "Identity verification at {:.1}%, documents required",
                    identity_pct
                ),
            });
        }
    }

    // Check control coverage
    if let Some(control_pct) = snapshot
        .get("control_verified_pct")
        .and_then(|v| v.as_f64())
    {
        if control_pct < 100.0 {
            gaps.push(CoverageGap {
                prong: "CONTROL".to_string(),
                entity_id: Some(subject_entity_id),
                description: format!(
                    "Control verification at {:.1}%, evidence required",
                    control_pct
                ),
            });
        }
    }

    gaps
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handler_metadata() {
        let op = OutreachPlanGenerateOp;
        assert_eq!(op.domain(), "research.outreach");
        assert_eq!(op.verb(), "plan-generate");
    }

    #[test]
    fn test_gap_prong_to_doc_type() {
        assert_eq!(
            gap_prong_to_doc_type("OWNERSHIP"),
            ("SHARE_REGISTER", "OWNERSHIP_CERTIFICATE")
        );
        assert_eq!(
            gap_prong_to_doc_type("IDENTITY"),
            ("PASSPORT", "NATIONAL_ID")
        );
        assert_eq!(
            gap_prong_to_doc_type("CONTROL"),
            ("BOARD_RESOLUTION", "MANAGEMENT_AGREEMENT")
        );
        assert_eq!(
            gap_prong_to_doc_type("SOURCE_OF_WEALTH"),
            (
                "SOURCE_OF_WEALTH_DECLARATION",
                "SOURCE_OF_WEALTH_DECLARATION"
            )
        );
        assert_eq!(
            gap_prong_to_doc_type("UNKNOWN"),
            ("SUPPORTING_EVIDENCE", "SUPPORTING_EVIDENCE")
        );
    }

    #[test]
    fn test_prong_priority() {
        assert!(prong_priority("IDENTITY") < prong_priority("OWNERSHIP"));
        assert!(prong_priority("OWNERSHIP") < prong_priority("CONTROL"));
        assert!(prong_priority("CONTROL") < prong_priority("SOURCE_OF_WEALTH"));
        assert!(prong_priority("SOURCE_OF_WEALTH") < prong_priority("UNKNOWN"));
    }

    #[test]
    fn test_extract_gaps_from_snapshot_with_gaps_key() {
        let subject_id = Uuid::new_v4();
        let entity_id = Uuid::new_v4();
        let snapshot = serde_json::json!({
            "gaps": [
                {
                    "prong": "OWNERSHIP",
                    "entity_id": entity_id.to_string(),
                    "description": "Missing share register"
                },
                {
                    "prong": "IDENTITY",
                    "entity_id": entity_id.to_string(),
                    "description": "No passport on file"
                }
            ]
        });

        let gaps = extract_gaps_from_snapshot(&Some(snapshot), subject_id);
        assert_eq!(gaps.len(), 2);
        assert_eq!(gaps[0].prong, "OWNERSHIP");
        assert_eq!(gaps[0].entity_id, Some(entity_id));
        assert_eq!(gaps[1].prong, "IDENTITY");
    }

    #[test]
    fn test_extract_gaps_from_snapshot_with_coverage_gaps_key() {
        let subject_id = Uuid::new_v4();
        let snapshot = serde_json::json!({
            "coverage_gaps": [
                {
                    "gap_type": "CONTROL",
                    "gap_description": "Board resolution missing"
                }
            ]
        });

        let gaps = extract_gaps_from_snapshot(&Some(snapshot), subject_id);
        assert_eq!(gaps.len(), 1);
        assert_eq!(gaps[0].prong, "CONTROL");
        assert_eq!(gaps[0].entity_id, None);
    }

    #[test]
    fn test_extract_gaps_from_snapshot_top_level_array() {
        let subject_id = Uuid::new_v4();
        let snapshot = serde_json::json!([
            {
                "prong": "SOURCE_OF_WEALTH",
                "description": "SoW declaration needed"
            }
        ]);

        let gaps = extract_gaps_from_snapshot(&Some(snapshot), subject_id);
        assert_eq!(gaps.len(), 1);
        assert_eq!(gaps[0].prong, "SOURCE_OF_WEALTH");
    }

    #[test]
    fn test_synthesize_gaps_from_aggregate_coverage() {
        let subject_id = Uuid::new_v4();
        let snapshot = serde_json::json!({
            "ownership_coverage_pct": 75.0,
            "identity_verified_pct": 50.0,
            "control_verified_pct": 100.0
        });

        let gaps = extract_gaps_from_snapshot(&Some(snapshot), subject_id);
        assert_eq!(gaps.len(), 2);
        assert_eq!(gaps[0].prong, "OWNERSHIP");
        assert_eq!(gaps[1].prong, "IDENTITY");
    }

    #[test]
    fn test_extract_gaps_from_none_snapshot() {
        let subject_id = Uuid::new_v4();
        let gaps = extract_gaps_from_snapshot(&None, subject_id);
        assert!(gaps.is_empty());
    }

    #[test]
    fn test_max_items_cap() {
        assert_eq!(MAX_ITEMS_PER_PLAN, 8);
    }

    #[test]
    fn test_build_request_text() {
        let text = build_request_text("OWNERSHIP", "SHARE_REGISTER", "Missing for entity X");
        assert!(text.contains("share register"));
        assert!(text.contains("ownership"));
        assert!(text.contains("Missing for entity X"));

        let text = build_request_text("IDENTITY", "PASSPORT", "ID not verified");
        assert!(text.contains("passport"));
        assert!(text.contains("identity verification"));
    }
}
