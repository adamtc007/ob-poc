//! Outreach plan-generation verb (1 plugin verb) — YAML-first
//! re-implementation of `research.outreach.plan-generate` from
//! `rust/config/verbs/research/outreach.yaml`.
//!
//! Reads coverage gaps from a UBO determination run, maps each
//! gap to a document type per spec 2A.2, prioritises + bundles
//! per entity (cap 8), inserts `outreach_plans` + `outreach_items`.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::{json, Value};
use uuid::Uuid;

use dsl_runtime::domain_ops::helpers::{self, json_extract_string_opt, json_extract_uuid};
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::SemOsVerbOp;

const MAX_ITEMS_PER_PLAN: usize = 8;

fn gap_prong_to_doc_type(prong: &str) -> (&'static str, &'static str) {
    match prong {
        "OWNERSHIP" => ("SHARE_REGISTER", "OWNERSHIP_CERTIFICATE"),
        "IDENTITY" => ("PASSPORT", "NATIONAL_ID"),
        "CONTROL" => ("BOARD_RESOLUTION", "MANAGEMENT_AGREEMENT"),
        "SOURCE_OF_WEALTH" => (
            "SOURCE_OF_WEALTH_DECLARATION",
            "SOURCE_OF_WEALTH_DECLARATION",
        ),
        _ => ("SUPPORTING_EVIDENCE", "SUPPORTING_EVIDENCE"),
    }
}

fn build_request_text(prong: &str, doc_type: &str, gap_description: &str) -> String {
    let doc_human = doc_type.to_lowercase().replace('_', " ");
    let purpose = match prong {
        "OWNERSHIP" => "to evidence ownership",
        "IDENTITY" => "for identity verification",
        "CONTROL" => "to evidence control arrangements",
        "SOURCE_OF_WEALTH" => "for source of wealth verification",
        _ => "for verification",
    };
    format!(
        "Please provide {} {}. Gap: {}",
        doc_human, purpose, gap_description
    )
}

fn prong_priority(prong: &str) -> i32 {
    match prong {
        "IDENTITY" => 1,
        "OWNERSHIP" => 2,
        "CONTROL" => 3,
        "SOURCE_OF_WEALTH" => 4,
        _ => 5,
    }
}

#[derive(Debug, Clone)]
struct CoverageGap {
    prong: String,
    entity_id: Option<Uuid>,
    description: String,
}

fn extract_gaps_from_snapshot(
    snapshot: &Option<Value>,
    subject_entity_id: Uuid,
) -> Vec<CoverageGap> {
    let Some(snapshot) = snapshot else {
        return vec![];
    };

    let gap_array = snapshot
        .get("gaps")
        .or_else(|| snapshot.get("coverage_gaps"))
        .or(if snapshot.is_array() {
            Some(snapshot)
        } else {
            None
        });

    let Some(arr) = gap_array.and_then(|v| v.as_array()) else {
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

fn synthesize_gaps_from_coverage(snapshot: &Value, subject_entity_id: Uuid) -> Vec<CoverageGap> {
    let mut gaps = Vec::new();
    for (key, prong, purpose) in [
        ("ownership_coverage_pct", "OWNERSHIP", "evidence required to close gap"),
        ("identity_verified_pct", "IDENTITY", "documents required"),
        ("control_verified_pct", "CONTROL", "evidence required"),
    ] {
        if let Some(pct) = snapshot.get(key).and_then(|v| v.as_f64()) {
            if pct < 100.0 {
                gaps.push(CoverageGap {
                    prong: prong.to_string(),
                    entity_id: Some(subject_entity_id),
                    description: format!("{} coverage at {:.1}%, {}", prong, pct, purpose),
                });
            }
        }
    }
    gaps
}

pub struct PlanGenerate;

#[async_trait]
impl SemOsVerbOp for PlanGenerate {
    fn fqn(&self) -> &str {
        "research.outreach.plan-generate"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let case_id = json_extract_uuid(args, ctx, "case-id")?;
        let determination_run_id = json_extract_uuid(args, ctx, "determination-run-id")?;
        let doc_preference = json_extract_string_opt(args, "doc-preference");

        let case_exists: Option<(Uuid,)> =
            sqlx::query_as(r#"SELECT case_id FROM "ob-poc".cases WHERE case_id = $1"#)
                .bind(case_id)
                .fetch_optional(scope.executor())
                .await?;
        if case_exists.is_none() {
            return Err(anyhow!("Case not found: {}", case_id));
        }

        #[derive(sqlx::FromRow)]
        struct DeterminationRow {
            coverage_snapshot: Option<Value>,
            subject_entity_id: Uuid,
        }

        let det_run: DeterminationRow = sqlx::query_as(
            r#"
            SELECT coverage_snapshot, subject_entity_id
            FROM "ob-poc".ubo_determination_runs
            WHERE run_id = $1 AND case_id = $2
            "#,
        )
        .bind(determination_run_id)
        .bind(case_id)
        .fetch_optional(scope.executor())
        .await?
        .ok_or_else(|| {
            anyhow!(
                "Determination run {} not found for case {}",
                determination_run_id,
                case_id
            )
        })?;

        let gaps = extract_gaps_from_snapshot(&det_run.coverage_snapshot, det_run.subject_entity_id);

        if gaps.is_empty() {
            let (plan_id,): (Uuid,) = sqlx::query_as(
                r#"
                INSERT INTO "ob-poc".outreach_plans (case_id, determination_run_id, status, total_items)
                VALUES ($1, $2, 'DRAFT', 0)
                RETURNING plan_id
                "#,
            )
            .bind(case_id)
            .bind(determination_run_id)
            .fetch_one(scope.executor())
            .await?;
            return Ok(VerbExecutionOutcome::Record(json!({
                "plan_id": plan_id,
                "case_id": case_id,
                "items_count": 0,
                "items": [],
            })));
        }

        let use_primary = doc_preference
            .as_deref()
            .map(|p| p != "fallback")
            .unwrap_or(true);

        let mut planned_items: Vec<(Uuid, String, String, String, String, i32, String)> = gaps
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
                (
                    gap.entity_id.unwrap_or(det_run.subject_entity_id),
                    gap.prong.clone(),
                    gap.description.clone(),
                    doc_type.to_string(),
                    request_text,
                    priority,
                    gap_ref,
                )
            })
            .collect();

        planned_items.sort_by(|a, b| a.5.cmp(&b.5).then(a.0.cmp(&b.0)));
        planned_items.truncate(MAX_ITEMS_PER_PLAN);
        let items_count = planned_items.len() as i32;

        let (plan_id,): (Uuid,) = sqlx::query_as(
            r#"
            INSERT INTO "ob-poc".outreach_plans (case_id, determination_run_id, status, total_items)
            VALUES ($1, $2, 'DRAFT', $3)
            RETURNING plan_id
            "#,
        )
        .bind(case_id)
        .bind(determination_run_id)
        .bind(items_count)
        .fetch_one(scope.executor())
        .await?;

        let mut result_items: Vec<Value> = Vec::with_capacity(planned_items.len());
        for (entity_id, prong, gap_description, doc_type, request_text, priority, gap_ref) in
            &planned_items
        {
            let item_row: (Uuid, String) = sqlx::query_as(
                r#"
                INSERT INTO "ob-poc".outreach_items (
                    plan_id, prong, target_entity_id, gap_description,
                    request_text, doc_type_requested, priority, closes_gap_ref, status
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'PENDING')
                RETURNING item_id, status
                "#,
            )
            .bind(plan_id)
            .bind(prong)
            .bind(entity_id)
            .bind(gap_description)
            .bind(request_text)
            .bind(doc_type)
            .bind(priority)
            .bind(gap_ref)
            .fetch_one(scope.executor())
            .await?;

            result_items.push(json!({
                "item_id": item_row.0,
                "entity_id": entity_id,
                "gap_ref": gap_ref,
                "doc_type": doc_type,
                "status": item_row.1,
            }));
        }

        helpers::emit_pending_state_advance(
            ctx,
            case_id,
            "research:outreach_plan_generated",
            "research/outreach-plan",
            "research.outreach.plan-generate",
        );

        Ok(VerbExecutionOutcome::Record(json!({
            "plan_id": plan_id,
            "case_id": case_id,
            "items_count": items_count,
            "items": result_items,
        })))
    }
}
