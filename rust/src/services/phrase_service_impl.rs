//! ob-poc impl of [`dsl_runtime::service_traits::PhraseService`].
//!
//! Single-method dispatch for the 9 governed-phrase-authoring verbs.
//! Bridge stays in ob-poc because every snapshot write goes through
//! `crate::sem_reg::store::SnapshotStore` and the schema-typed
//! sem_reg ids/types modules — multi-consumer surfaces with no
//! dsl-runtime analogue.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use sem_os_core::principal::Principal;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use dsl_runtime::service_traits::PhraseService;

use crate::sem_reg::store::SnapshotStore;
use crate::sem_reg::types::{ChangeType, ObjectType, SnapshotMeta, SnapshotRow, SnapshotStatus};

// ── Result types (mirror the relocated phrase_ops result shapes) ──────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ObserveMissesResult {
    miss_count: i64,
    wrong_match_count: i64,
    top_miss_patterns: Vec<MissPattern>,
    top_wrong_match_patterns: Vec<WrongMatchPattern>,
    watermark_advanced_to: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MissPattern {
    utterance: String,
    occurrences: i64,
    first_seen: String,
    last_seen: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WrongMatchPattern {
    utterance: String,
    matched_verb: String,
    occurrences: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WorkspaceCoverage {
    domain: String,
    verb_count: i64,
    phrase_count: i64,
    avg_phrases_per_verb: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CollisionReport {
    candidate_phrase: String,
    target_verb: String,
    workspace: Option<String>,
    exact_conflicts: Vec<ExactConflict>,
    semantic_near_misses: Vec<SemanticNearMiss>,
    cross_workspace_conflicts: Vec<CrossWorkspaceConflict>,
    safe_to_propose: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExactConflict {
    existing_verb: String,
    source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SemanticNearMiss {
    phrase: String,
    verb_fqn: String,
    similarity: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CrossWorkspaceConflict {
    phrase: String,
    verb_fqn: String,
    workspace: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PhraseProposalResult {
    snapshot_id: Uuid,
    phrase: String,
    verb_fqn: String,
    confidence: f64,
    risk_tier: String,
    collision_safe: bool,
    state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BatchProposeResult {
    proposals_generated: i64,
    skipped_duplicates: i64,
    message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PhraseApproveResult {
    published_snapshot_id: Uuid,
    phrase: String,
    verb_fqn: String,
    status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PhraseLifecycleResult {
    snapshot_id: Uuid,
    state: String,
    reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PhraseProposalSummary {
    snapshot_id: Uuid,
    phrase: String,
    verb_fqn: String,
    workspace: Option<String>,
    state: String,
    created_by: String,
    version: String,
    collision_report: Option<Value>,
    evidence: Option<Value>,
}

// ── Bridge ────────────────────────────────────────────────────────────────────

pub struct ObPocPhraseService;

impl ObPocPhraseService {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ObPocPhraseService {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PhraseService for ObPocPhraseService {
    async fn dispatch_phrase_verb(
        &self,
        pool: &PgPool,
        verb_name: &str,
        args: &Value,
        principal: &Principal,
    ) -> Result<Value> {
        match verb_name {
            "observe-misses" => phrase_observe_misses(pool, args).await,
            "coverage-report" => phrase_coverage_report(pool).await,
            "check-collisions" => phrase_check_collisions(pool, args).await,
            "propose" => phrase_propose(pool, args, principal).await,
            "batch-propose" => phrase_batch_propose(pool, args, principal).await,
            "review-proposals" => phrase_review_proposals(pool).await,
            "approve" => phrase_approve(pool, args, principal).await,
            "reject" => phrase_reject(pool, args, principal).await,
            "defer" => phrase_defer(pool, args, principal).await,
            other => Err(anyhow!("unknown phrase verb: {other}")),
        }
    }
}

// ── arg helpers ───────────────────────────────────────────────────────────────

fn arg_string(args: &Value, name: &str) -> Result<String> {
    args.get(name)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow!("Missing {name} argument"))
}

fn arg_string_opt(args: &Value, name: &str) -> Option<String> {
    args.get(name)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

fn arg_int_opt(args: &Value, name: &str) -> Option<i64> {
    args.get(name).and_then(|v| match v {
        Value::Number(n) => n.as_i64(),
        Value::String(s) => s.parse().ok(),
        _ => None,
    })
}

// ── shared helpers ────────────────────────────────────────────────────────────

async fn run_collision_check(
    pool: &PgPool,
    phrase: &str,
    target_verb: &str,
    workspace: Option<&str>,
) -> Result<(CollisionReport, f64)> {
    let mut exact_conflicts = Vec::new();
    let mut cross_workspace_conflicts = Vec::new();

    let bank_rows: Vec<(String, Option<String>, String)> = sqlx::query_as(
        r#"
        SELECT verb_fqn, workspace, source
        FROM "ob-poc".phrase_bank
        WHERE phrase = $1 AND active = TRUE
        "#,
    )
    .bind(phrase)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    for (verb_fqn, ws, source) in &bank_rows {
        if verb_fqn != target_verb {
            if ws.as_deref() == workspace {
                exact_conflicts.push(ExactConflict {
                    existing_verb: verb_fqn.clone(),
                    source: format!("phrase_bank ({})", source),
                });
            } else {
                cross_workspace_conflicts.push(CrossWorkspaceConflict {
                    phrase: phrase.to_string(),
                    verb_fqn: verb_fqn.clone(),
                    workspace: ws.clone(),
                });
            }
        }
    }

    let embed_rows: Vec<(String,)> = sqlx::query_as(
        r#"
        SELECT DISTINCT verb_fqn
        FROM "ob-poc".verb_pattern_embeddings
        WHERE pattern = $1 AND verb_fqn != $2
        "#,
    )
    .bind(phrase)
    .bind(target_verb)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    for (verb_fqn,) in embed_rows {
        exact_conflicts.push(ExactConflict {
            existing_verb: verb_fqn,
            source: "verb_pattern_embeddings".to_string(),
        });
    }

    let semantic_rows: Vec<(String, String, f64)> = sqlx::query_as(
        r#"
        SELECT pattern, verb_fqn, 1.0 - (embedding <=> (
            SELECT embedding FROM "ob-poc".verb_pattern_embeddings
            WHERE pattern = $1
            LIMIT 1
        )) AS similarity
        FROM "ob-poc".verb_pattern_embeddings
        WHERE verb_fqn != $2
          AND pattern != $1
        ORDER BY embedding <=> (
            SELECT embedding FROM "ob-poc".verb_pattern_embeddings
            WHERE pattern = $1
            LIMIT 1
        )
        LIMIT 10
        "#,
    )
    .bind(phrase)
    .bind(target_verb)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let max_similarity = semantic_rows
        .iter()
        .map(|(_, _, sim)| *sim)
        .fold(0.0_f64, f64::max);

    let semantic_near_misses: Vec<SemanticNearMiss> = semantic_rows
        .into_iter()
        .filter(|(_, _, sim)| *sim > 0.85)
        .map(|(p, v, sim)| SemanticNearMiss {
            phrase: p,
            verb_fqn: v,
            similarity: (sim * 1000.0).round() / 1000.0,
        })
        .collect();

    let safe_to_propose = exact_conflicts.is_empty() && cross_workspace_conflicts.is_empty();

    let report = CollisionReport {
        candidate_phrase: phrase.to_string(),
        target_verb: target_verb.to_string(),
        workspace: workspace.map(|s| s.to_string()),
        exact_conflicts,
        semantic_near_misses,
        cross_workspace_conflicts,
        safe_to_propose,
    };

    Ok((report, max_similarity))
}

fn risk_tier_for_verb(verb_fqn: &str) -> &'static str {
    let lower = verb_fqn.to_lowercase();
    let critical_keywords = [
        "approve",
        "reject",
        "terminate",
        "delete",
        "close",
        "certify",
    ];
    let standard_keywords = ["read", "list", "get", "show", "describe", "search"];
    if critical_keywords.iter().any(|kw| lower.contains(kw)) {
        "critical"
    } else if standard_keywords.iter().any(|kw| lower.contains(kw)) {
        "standard"
    } else {
        "elevated"
    }
}

fn compute_proposal_confidence(max_semantic_similarity: f64) -> f64 {
    let frequency = 0.5;
    let breadth = 0.5;
    let collision_safety = 1.0 - max_semantic_similarity.clamp(0.0, 1.0);
    let rephrase_confirmation = 0.0;
    let wrong_match_severity = 0.0;
    let raw = 0.25 * frequency
        + 0.20 * breadth
        + 0.20 * collision_safety
        + 0.15 * rephrase_confirmation
        + 0.20 * wrong_match_severity;
    (raw * 1000.0).round() / 1000.0
}

fn next_phrase_meta(
    predecessor: Option<&SnapshotRow>,
    object_id: Uuid,
    created_by: &str,
    change_type: ChangeType,
    change_rationale: Option<String>,
    status: SnapshotStatus,
) -> SnapshotMeta {
    let mut meta =
        SnapshotMeta::new_operational(ObjectType::PhraseMapping, object_id, created_by.to_string());
    meta.change_type = change_type;
    meta.change_rationale = change_rationale;
    meta.status = status;
    if let Some(pred) = predecessor {
        meta.version_major = pred.version_major;
        meta.version_minor = pred.version_minor + 1;
        meta.predecessor_id = Some(pred.snapshot_id);
    }
    meta
}

async fn publish_phrase_snapshot_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    meta: &SnapshotMeta,
    definition: &Value,
) -> Result<Uuid> {
    if let Some(predecessor_id) = meta.predecessor_id {
        let affected = sqlx::query(
            r#"
            UPDATE sem_reg.snapshots
            SET effective_until = NOW()
            WHERE snapshot_id = $1 AND effective_until IS NULL
            "#,
        )
        .bind(predecessor_id)
        .execute(&mut **tx)
        .await?
        .rows_affected();
        if affected == 0 {
            return Err(anyhow!(
                "Predecessor snapshot {} not found or already superseded",
                predecessor_id
            ));
        }
    }

    let security_label = serde_json::to_value(&meta.security_label)?;
    let snapshot_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO sem_reg.snapshots (
            snapshot_set_id, object_type, object_id,
            version_major, version_minor, status,
            governance_tier, trust_class, security_label,
            predecessor_id, change_type, change_rationale,
            created_by, approved_by, definition
        ) VALUES (
            NULL, $1::sem_reg.object_type, $2,
            $3, $4, $5::sem_reg.snapshot_status,
            $6::sem_reg.governance_tier, $7::sem_reg.trust_class, $8,
            $9, $10::sem_reg.change_type, $11,
            $12, $13, $14
        )
        RETURNING snapshot_id
        "#,
    )
    .bind(meta.object_type.as_ref())
    .bind(meta.object_id)
    .bind(meta.version_major)
    .bind(meta.version_minor)
    .bind(meta.status.as_ref())
    .bind(meta.governance_tier.as_ref())
    .bind(meta.trust_class.as_ref())
    .bind(security_label)
    .bind(meta.predecessor_id)
    .bind(meta.change_type.as_ref())
    .bind(&meta.change_rationale)
    .bind(&meta.created_by)
    .bind(&meta.approved_by)
    .bind(definition)
    .fetch_one(&mut **tx)
    .await?;

    Ok(snapshot_id)
}

// ── phrase.observe-misses ─────────────────────────────────────────────────────

async fn phrase_observe_misses(pool: &PgPool, args: &Value) -> Result<Value> {
    let limit = arg_int_opt(args, "limit").unwrap_or(100);
    let watermark: (i64,) = sqlx::query_as(
        r#"SELECT last_observed_sequence FROM "ob-poc".phrase_observation_state WHERE id = 1"#,
    )
    .fetch_one(pool)
    .await?;
    let last_seq = watermark.0;
    let miss_rows: Vec<(String, i64)> = sqlx::query_as(
        r#"
        SELECT
            op->>'utterance' AS utterance,
            COUNT(*)::bigint AS occurrences
        FROM "ob-poc".session_traces
        WHERE sequence > $1
          AND op->>'kind' = 'utterance'
          AND (
              op->'result'->>'match_status' = 'no_match'
              OR op->'result'->>'match_status' IS NULL
          )
          AND op->>'utterance' IS NOT NULL
        GROUP BY op->>'utterance'
        ORDER BY occurrences DESC
        LIMIT $2
        "#,
    )
    .bind(last_seq)
    .bind(limit)
    .fetch_all(pool)
    .await
    .unwrap_or_default();
    let wrong_match_rows: Vec<(String, String, i64)> = sqlx::query_as(
        r#"
        SELECT
            op->>'utterance' AS utterance,
            op->'result'->>'matched_verb' AS matched_verb,
            COUNT(*)::bigint AS occurrences
        FROM "ob-poc".session_traces
        WHERE sequence > $1
          AND op->>'kind' = 'utterance'
          AND op->'result'->>'match_status' = 'wrong_match'
          AND op->>'utterance' IS NOT NULL
        GROUP BY op->>'utterance', op->'result'->>'matched_verb'
        ORDER BY occurrences DESC
        LIMIT $2
        "#,
    )
    .bind(last_seq)
    .bind(limit)
    .fetch_all(pool)
    .await
    .unwrap_or_default();
    let new_watermark: (Option<i64>,) =
        sqlx::query_as(r#"SELECT MAX(sequence) FROM "ob-poc".session_traces WHERE sequence > $1"#)
            .bind(last_seq)
            .fetch_one(pool)
            .await?;
    let advanced_to = new_watermark.0.unwrap_or(last_seq);
    let miss_count = miss_rows.iter().map(|(_, c)| c).sum::<i64>();
    let wrong_match_count = wrong_match_rows.iter().map(|(_, _, c)| c).sum::<i64>();
    sqlx::query(
        r#"
        UPDATE "ob-poc".phrase_observation_state
        SET last_observed_sequence = $1,
            last_run_at = NOW(),
            patterns_found = $2,
            wrong_match_patterns_found = $3
        WHERE id = 1
        "#,
    )
    .bind(advanced_to)
    .bind(miss_count as i32)
    .bind(wrong_match_count as i32)
    .execute(pool)
    .await?;
    let top_miss_patterns: Vec<MissPattern> = miss_rows
        .into_iter()
        .map(|(utterance, occurrences)| MissPattern {
            utterance,
            occurrences,
            first_seen: String::new(),
            last_seen: String::new(),
        })
        .collect();
    let top_wrong_match_patterns: Vec<WrongMatchPattern> = wrong_match_rows
        .into_iter()
        .map(|(utterance, matched_verb, occurrences)| WrongMatchPattern {
            utterance,
            matched_verb,
            occurrences,
        })
        .collect();
    let result = ObserveMissesResult {
        miss_count,
        wrong_match_count,
        top_miss_patterns,
        top_wrong_match_patterns,
        watermark_advanced_to: advanced_to,
    };
    Ok(serde_json::to_value(result)?)
}

// ── phrase.coverage-report ────────────────────────────────────────────────────

async fn phrase_coverage_report(pool: &PgPool) -> Result<Value> {
    let rows: Vec<(String, i64, i64)> = sqlx::query_as(
        r#"
        SELECT
            v.domain,
            COUNT(DISTINCT v.verb_fqn)::bigint AS verb_count,
            COALESCE(e.phrase_count, 0)::bigint AS phrase_count
        FROM "ob-poc".dsl_verbs v
        LEFT JOIN (
            SELECT
                split_part(verb_fqn, '.', 1) AS domain,
                COUNT(*)::bigint AS phrase_count
            FROM "ob-poc".verb_pattern_embeddings
            GROUP BY split_part(verb_fqn, '.', 1)
        ) e ON e.domain = v.domain
        GROUP BY v.domain, e.phrase_count
        ORDER BY verb_count DESC
        "#,
    )
    .fetch_all(pool)
    .await?;
    let entries: Vec<Value> = rows
        .into_iter()
        .map(|(domain, verb_count, phrase_count)| {
            let avg = if verb_count > 0 {
                phrase_count as f64 / verb_count as f64
            } else {
                0.0
            };
            let coverage = WorkspaceCoverage {
                domain,
                verb_count,
                phrase_count,
                avg_phrases_per_verb: (avg * 100.0).round() / 100.0,
            };
            serde_json::to_value(coverage).unwrap_or_default()
        })
        .collect();
    Ok(json!(entries))
}

// ── phrase.check-collisions ───────────────────────────────────────────────────

async fn phrase_check_collisions(pool: &PgPool, args: &Value) -> Result<Value> {
    let phrase = arg_string(args, "phrase")?;
    let target_verb = arg_string(args, "target-verb")?;
    let workspace = arg_string_opt(args, "workspace");
    let (report, _max_similarity) =
        run_collision_check(pool, &phrase, &target_verb, workspace.as_deref()).await?;
    Ok(serde_json::to_value(report)?)
}

// ── phrase.propose ────────────────────────────────────────────────────────────

async fn phrase_propose(pool: &PgPool, args: &Value, principal: &Principal) -> Result<Value> {
    let phrase = arg_string(args, "phrase")?;
    let target_verb = arg_string(args, "target-verb")?;
    let workspace = arg_string_opt(args, "workspace");
    let rationale = arg_string_opt(args, "rationale");
    let (collision_report, max_similarity) =
        run_collision_check(pool, &phrase, &target_verb, workspace.as_deref()).await?;
    let confidence = compute_proposal_confidence(max_similarity);
    let risk_tier = risk_tier_for_verb(&target_verb);
    let collision_report_json = serde_json::to_value(&collision_report)?;
    let definition = json!({
        "phrase": phrase,
        "verb_fqn": target_verb,
        "workspace": workspace,
        "source": "governed",
        "risk_tier": risk_tier,
        "state": "proposed",
        "confidence": confidence,
        "rationale": rationale,
        "collision_report": collision_report_json,
    });
    let semantic_id = format!("phrase:{}:{}", target_verb, phrase);
    let object_id = crate::sem_reg::ids::object_id_for(ObjectType::PhraseMapping, &semantic_id);
    let mut tx = pool.begin().await?;
    let meta = next_phrase_meta(
        None,
        object_id,
        &principal.actor_id,
        ChangeType::NonBreaking,
        rationale.clone(),
        SnapshotStatus::Active,
    );
    let snapshot_id = publish_phrase_snapshot_in_tx(&mut tx, &meta, &definition).await?;
    tx.commit().await?;
    let result = PhraseProposalResult {
        snapshot_id,
        phrase,
        verb_fqn: target_verb,
        confidence,
        risk_tier: risk_tier.to_string(),
        collision_safe: collision_report.safe_to_propose,
        state: "proposed".to_string(),
    };
    Ok(serde_json::to_value(result)?)
}

// ── phrase.batch-propose ──────────────────────────────────────────────────────

async fn phrase_batch_propose(pool: &PgPool, args: &Value, principal: &Principal) -> Result<Value> {
    let limit = arg_int_opt(args, "limit").unwrap_or(50) as usize;
    let watermark_result: Result<(i64,), _> = sqlx::query_as(
        r#"SELECT last_observed_sequence FROM "ob-poc".phrase_observation_state WHERE id = 1"#,
    )
    .fetch_one(pool)
    .await;
    let last_seq = match watermark_result {
        Ok((seq,)) => seq,
        Err(_) => {
            let result = BatchProposeResult {
                proposals_generated: 0,
                skipped_duplicates: 0,
                message: "No phrase observation state found. Run phrase.observe-misses first."
                    .to_string(),
            };
            return Ok(serde_json::to_value(result)?);
        }
    };
    let miss_rows: Vec<(String,)> = sqlx::query_as(
        r#"
        SELECT DISTINCT op->>'utterance' AS utterance
        FROM "ob-poc".session_traces
        WHERE sequence > $1
          AND op->>'kind' = 'utterance'
          AND (
              op->'result'->>'match_status' = 'no_match'
              OR op->'result'->>'match_status' IS NULL
          )
          AND op->>'utterance' IS NOT NULL
        ORDER BY utterance
        "#,
    )
    .bind(last_seq)
    .fetch_all(pool)
    .await
    .unwrap_or_default();
    if miss_rows.is_empty() {
        let result = BatchProposeResult {
            proposals_generated: 0,
            skipped_duplicates: 0,
            message: "No session trace data available for observation".to_string(),
        };
        return Ok(serde_json::to_value(result)?);
    }
    let mut seen_phrases = std::collections::HashSet::new();
    let mut proposals: Vec<Value> = Vec::new();
    let mut skipped = 0_i64;
    for (utterance,) in &miss_rows {
        if proposals.len() >= limit {
            break;
        }
        let phrase = utterance.trim().to_string();
        if phrase.is_empty() || !seen_phrases.insert(phrase.clone()) {
            skipped += 1;
            continue;
        }
        let best_match: Option<(String, f64)> = sqlx::query_as(
            r#"
            SELECT verb_fqn, 1.0 - (embedding <=> (
                SELECT embedding FROM "ob-poc".verb_pattern_embeddings
                WHERE pattern = $1
                LIMIT 1
            )) AS similarity
            FROM "ob-poc".verb_pattern_embeddings
            WHERE pattern != $1
            ORDER BY embedding <=> (
                SELECT embedding FROM "ob-poc".verb_pattern_embeddings
                WHERE pattern = $1
                LIMIT 1
            )
            LIMIT 1
            "#,
        )
        .bind(&phrase)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten();
        let (target_verb, _best_sim) = match best_match {
            Some(m) => m,
            None => {
                skipped += 1;
                continue;
            }
        };
        let (collision_report, max_similarity) =
            run_collision_check(pool, &phrase, &target_verb, None).await?;
        let confidence = compute_proposal_confidence(max_similarity);
        let risk_tier = risk_tier_for_verb(&target_verb);
        let collision_report_json = serde_json::to_value(&collision_report)?;
        let definition = json!({
            "phrase": phrase,
            "verb_fqn": target_verb,
            "workspace": null,
            "source": "governed",
            "risk_tier": risk_tier,
            "state": "proposed",
            "confidence": confidence,
            "rationale": "Auto-generated from session trace miss patterns",
            "collision_report": collision_report_json,
        });
        let semantic_id = format!("phrase:{}:{}", target_verb, phrase);
        let object_id = crate::sem_reg::ids::object_id_for(ObjectType::PhraseMapping, &semantic_id);
        let mut tx = pool.begin().await?;
        let meta = next_phrase_meta(
            None,
            object_id,
            &principal.actor_id,
            ChangeType::NonBreaking,
            Some("Auto-generated from session trace miss patterns".to_string()),
            SnapshotStatus::Active,
        );
        let snapshot_id = publish_phrase_snapshot_in_tx(&mut tx, &meta, &definition).await?;
        tx.commit().await?;
        let proposal = PhraseProposalResult {
            snapshot_id,
            phrase,
            verb_fqn: target_verb,
            confidence,
            risk_tier: risk_tier.to_string(),
            collision_safe: collision_report.safe_to_propose,
            state: "proposed".to_string(),
        };
        proposals.push(serde_json::to_value(proposal)?);
    }
    proposals.sort_by(|a, b| {
        let tier_order = |t: &str| -> u8 {
            match t {
                "critical" => 0,
                "elevated" => 1,
                "standard" => 2,
                _ => 3,
            }
        };
        let a_tier = a
            .get("risk_tier")
            .and_then(|v| v.as_str())
            .unwrap_or("elevated");
        let b_tier = b
            .get("risk_tier")
            .and_then(|v| v.as_str())
            .unwrap_or("elevated");
        let tier_cmp = tier_order(a_tier).cmp(&tier_order(b_tier));
        if tier_cmp != std::cmp::Ordering::Equal {
            return tier_cmp;
        }
        let a_conf = a.get("confidence").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let b_conf = b.get("confidence").and_then(|v| v.as_f64()).unwrap_or(0.0);
        b_conf
            .partial_cmp(&a_conf)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let generated = proposals.len() as i64;
    let mut result_set = Vec::with_capacity(proposals.len() + 1);
    let summary = BatchProposeResult {
        proposals_generated: generated,
        skipped_duplicates: skipped,
        message: format!(
            "Generated {} proposals from session trace miss patterns",
            generated
        ),
    };
    result_set.push(serde_json::to_value(summary)?);
    result_set.extend(proposals);
    Ok(json!(result_set))
}

// ── phrase.review-proposals ───────────────────────────────────────────────────

async fn phrase_review_proposals(pool: &PgPool) -> Result<Value> {
    let rows: Vec<(Uuid, Value, String, i32, i32)> = sqlx::query_as(
        r#"
        SELECT
            snapshot_id,
            definition,
            created_by,
            version_major,
            version_minor
        FROM sem_reg.snapshots
        WHERE object_type = 'phrase_mapping'
          AND status = 'active'
          AND effective_until IS NULL
          AND COALESCE(definition->>'state', 'proposed') != 'published'
        ORDER BY effective_from DESC
        "#,
    )
    .fetch_all(pool)
    .await?;
    let proposals: Vec<Value> = rows
        .into_iter()
        .map(
            |(snapshot_id, definition, created_by, ver_major, ver_minor)| {
                let phrase = definition
                    .get("phrase")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let verb_fqn = definition
                    .get("verb_fqn")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let workspace = definition
                    .get("workspace")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let state = definition
                    .get("state")
                    .and_then(|v| v.as_str())
                    .unwrap_or("proposed")
                    .to_string();
                let collision_report = definition.get("collision_report").cloned();
                let evidence = definition.get("evidence").cloned();
                serde_json::to_value(PhraseProposalSummary {
                    snapshot_id,
                    phrase,
                    verb_fqn,
                    workspace,
                    state,
                    created_by,
                    version: format!("{}.{}", ver_major, ver_minor),
                    collision_report,
                    evidence,
                })
                .unwrap_or_default()
            },
        )
        .collect();
    Ok(json!(proposals))
}

// ── phrase.approve ────────────────────────────────────────────────────────────

async fn phrase_approve(pool: &PgPool, args: &Value, principal: &Principal) -> Result<Value> {
    let proposal_id_str = arg_string(args, "proposal-id")?;
    let proposal_id: Uuid = proposal_id_str
        .parse()
        .map_err(|_| anyhow!("Invalid UUID for proposal-id: {}", proposal_id_str))?;
    let rationale = arg_string_opt(args, "rationale");
    let proposal = SnapshotStore::get_by_id(pool, proposal_id)
        .await?
        .ok_or_else(|| anyhow!("Phrase proposal snapshot {} not found", proposal_id))?;
    if proposal.object_type != ObjectType::PhraseMapping {
        return Err(anyhow!(
            "Snapshot {} is not a phrase_mapping (found {:?})",
            proposal_id,
            proposal.object_type
        ));
    }
    let mut definition: Value = proposal.definition.clone();
    definition["state"] = Value::String("published".to_string());
    if let Some(ref r) = rationale {
        definition["approval_rationale"] = Value::String(r.clone());
    }
    let mut tx = pool.begin().await?;
    let meta = next_phrase_meta(
        Some(&proposal),
        proposal.object_id,
        &principal.actor_id,
        ChangeType::NonBreaking,
        rationale,
        SnapshotStatus::Active,
    );
    let published_snapshot_id = publish_phrase_snapshot_in_tx(&mut tx, &meta, &definition).await?;
    tx.commit().await?;
    let phrase = definition
        .get("phrase")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let verb_fqn = definition
        .get("verb_fqn")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let result = PhraseApproveResult {
        published_snapshot_id,
        phrase,
        verb_fqn,
        status: "published".to_string(),
    };
    Ok(serde_json::to_value(result)?)
}

// ── phrase.reject ─────────────────────────────────────────────────────────────

async fn phrase_reject(pool: &PgPool, args: &Value, principal: &Principal) -> Result<Value> {
    let proposal_id_str = arg_string(args, "proposal-id")?;
    let proposal_id: Uuid = proposal_id_str
        .parse()
        .map_err(|_| anyhow!("Invalid UUID for proposal-id: {}", proposal_id_str))?;
    let reason = arg_string(args, "reason")?;
    let proposal = SnapshotStore::get_by_id(pool, proposal_id)
        .await?
        .ok_or_else(|| anyhow!("Phrase proposal snapshot {} not found", proposal_id))?;
    if proposal.object_type != ObjectType::PhraseMapping {
        return Err(anyhow!(
            "Snapshot {} is not a phrase_mapping (found {:?})",
            proposal_id,
            proposal.object_type
        ));
    }
    let mut definition: Value = proposal.definition.clone();
    definition["state"] = Value::String("rejected".to_string());
    definition["rejection_reason"] = Value::String(reason.clone());
    let mut tx = pool.begin().await?;
    let meta = next_phrase_meta(
        Some(&proposal),
        proposal.object_id,
        &principal.actor_id,
        ChangeType::NonBreaking,
        Some(reason.clone()),
        SnapshotStatus::Deprecated,
    );
    let rejected_snapshot_id = publish_phrase_snapshot_in_tx(&mut tx, &meta, &definition).await?;
    tx.commit().await?;
    let result = PhraseLifecycleResult {
        snapshot_id: rejected_snapshot_id,
        state: "rejected".to_string(),
        reason: Some(reason),
    };
    Ok(serde_json::to_value(result)?)
}

// ── phrase.defer ──────────────────────────────────────────────────────────────

async fn phrase_defer(pool: &PgPool, args: &Value, principal: &Principal) -> Result<Value> {
    let proposal_id_str = arg_string(args, "proposal-id")?;
    let proposal_id: Uuid = proposal_id_str
        .parse()
        .map_err(|_| anyhow!("Invalid UUID for proposal-id: {}", proposal_id_str))?;
    let reason = arg_string_opt(args, "reason");
    let proposal = SnapshotStore::get_by_id(pool, proposal_id)
        .await?
        .ok_or_else(|| anyhow!("Phrase proposal snapshot {} not found", proposal_id))?;
    if proposal.object_type != ObjectType::PhraseMapping {
        return Err(anyhow!(
            "Snapshot {} is not a phrase_mapping (found {:?})",
            proposal_id,
            proposal.object_type
        ));
    }
    let mut definition: Value = proposal.definition.clone();
    definition["state"] = Value::String("deferred".to_string());
    if let Some(ref r) = reason {
        definition["deferral_reason"] = Value::String(r.clone());
    }
    let mut tx = pool.begin().await?;
    let meta = next_phrase_meta(
        Some(&proposal),
        proposal.object_id,
        &principal.actor_id,
        ChangeType::NonBreaking,
        reason.clone(),
        SnapshotStatus::Active,
    );
    let deferred_snapshot_id = publish_phrase_snapshot_in_tx(&mut tx, &meta, &definition).await?;
    tx.commit().await?;
    let result = PhraseLifecycleResult {
        snapshot_id: deferred_snapshot_id,
        state: "deferred".to_string(),
        reason,
    };
    Ok(serde_json::to_value(result)?)
}
