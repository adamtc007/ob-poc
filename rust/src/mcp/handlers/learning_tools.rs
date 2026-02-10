//! Learning, promotion, and teaching tool handlers.
//!
//! Extracted from `core.rs` — contains all handlers related to:
//! - Intent blocking and learning import/list/approve/reject/stats
//! - Intent analyze/list/approve/reject/reload (AgentLearningInspector)
//! - Learning analyze/apply, embeddings status
//! - Promotion pipeline (run cycle, candidates, review, approve, reject, health, status)
//! - Teaching tools (teach phrase, unteach phrase, teaching status)

use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use uuid::Uuid;

use super::core::ToolHandlers;

// ============================================================================
// Row Structs used by learning handlers
// ============================================================================

/// Row struct for learning candidate queries
#[derive(Debug, sqlx::FromRow)]
#[allow(dead_code)] // Fields required by FromRow derive
struct LearningCandidateRow {
    id: i64,
    learning_type: String,
    input_pattern: String,
    suggested_output: String,
}

/// Row struct for top corrections queries
#[derive(Debug, sqlx::FromRow)]
struct TopCorrectionRow {
    input_pattern: String,
    suggested_output: String,
    occurrence_count: i32,
}

impl ToolHandlers {
    // =========================================================================
    // Learning Management Handlers
    // =========================================================================

    pub(super) async fn intent_block(&self, args: Value) -> Result<Value> {
        let phrase = args["phrase"]
            .as_str()
            .ok_or_else(|| anyhow!("phrase required"))?;
        let blocked_verb = args["blocked_verb"]
            .as_str()
            .ok_or_else(|| anyhow!("blocked_verb required"))?;
        let reason = args["reason"].as_str();
        let scope = args["scope"].as_str().unwrap_or("global");
        let user_id: Option<Uuid> = if scope == "user_specific" {
            args["user_id"].as_str().and_then(|s| s.parse().ok())
        } else {
            None
        };
        let expires = args["expires"]
            .as_str()
            .and_then(|s| parse_duration(s).ok());

        let pool = self.require_pool()?;

        // Generate embedding (target mode - storing in DB)
        let embedding: Option<Vec<f32>> = self.embedder.embed_target(phrase).await.ok();

        let id = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO agent.phrase_blocklist
                (phrase, blocked_verb, embedding, reason, user_id, expires_at)
            VALUES ($1, $2, $3::vector, $4, $5, $6)
            ON CONFLICT (phrase, blocked_verb, COALESCE(user_id, '00000000-0000-0000-0000-000000000000'::uuid))
            DO UPDATE SET
                reason = COALESCE($4, agent.phrase_blocklist.reason),
                expires_at = $6,
                embedding = COALESCE($3::vector, agent.phrase_blocklist.embedding)
            RETURNING id
            "#,
        )
        .bind(phrase)
        .bind(blocked_verb)
        .bind(embedding.as_ref())
        .bind(reason)
        .bind(user_id)
        .bind(expires.map(|d| chrono::Utc::now() + d))
        .fetch_one(pool)
        .await?;

        Ok(json!({
            "blocked": true,
            "block_id": id.to_string(),
            "phrase": phrase,
            "blocked_verb": blocked_verb,
            "scope": scope,
            "has_embedding": embedding.is_some(),
            "message": format!(
                "Blocked '{}' for phrase pattern '{}'. {}",
                blocked_verb, phrase,
                if embedding.is_some() { "Semantic matching enabled." } else { "Exact match only (no embedder)." }
            )
        }))
    }

    /// Bulk import phrase->verb mappings
    pub(super) async fn learning_import(&self, args: Value) -> Result<Value> {
        let source = args["source"]
            .as_str()
            .ok_or_else(|| anyhow!("source required"))?;
        let format = args["format"].as_str().unwrap_or("yaml");
        let scope = args["scope"].as_str().unwrap_or("global");
        let user_id: Option<Uuid> = if scope == "user_specific" {
            args["user_id"].as_str().and_then(|s| s.parse().ok())
        } else {
            None
        };
        let dry_run = args["dry_run"].as_bool().unwrap_or(false);

        // Get content
        let content = match source {
            "file" => {
                let path = args["path"]
                    .as_str()
                    .ok_or_else(|| anyhow!("path required for file source"))?;
                std::fs::read_to_string(path)?
            }
            "inline" => args["content"]
                .as_str()
                .ok_or_else(|| anyhow!("content required for inline source"))?
                .to_string(),
            _ => return Err(anyhow!("Invalid source: {}", source)),
        };

        // Parse content
        let import_data: ImportData = match format {
            "yaml" => serde_yaml::from_str(&content)?,
            "json" => serde_json::from_str(&content)?,
            "csv" => parse_csv_import(&content)?,
            _ => return Err(anyhow!("Unknown format: {}", format)),
        };

        // Validate
        let mut validation_errors = Vec::new();
        for (i, phrase) in import_data.phrases.iter().enumerate() {
            if phrase.phrase.is_empty() {
                validation_errors.push(format!("Row {}: empty phrase", i + 1));
            }
            if phrase.verb.is_empty() {
                validation_errors.push(format!("Row {}: empty verb", i + 1));
            }
        }

        if !validation_errors.is_empty() {
            return Ok(json!({
                "success": false,
                "validation_errors": validation_errors,
                "message": "Import failed validation"
            }));
        }

        if dry_run {
            return Ok(json!({
                "success": true,
                "dry_run": true,
                "would_import": import_data.phrases.len(),
                "message": "Validation passed, ready to import"
            }));
        }

        let pool = self.require_pool()?;
        let mut imported = 0;
        let mut errors = Vec::new();

        // Batch embed (target mode - storing in DB)
        let embeddings: Vec<Option<Vec<f32>>> = {
            let texts: Vec<&str> = import_data
                .phrases
                .iter()
                .map(|p| p.phrase.as_str())
                .collect();
            match self.embedder.embed_batch_targets(&texts).await {
                Ok(embs) => embs.into_iter().map(Some).collect(),
                Err(_) => vec![None; import_data.phrases.len()],
            }
        };

        for (phrase_data, embedding) in import_data.phrases.iter().zip(embeddings) {
            let result = if user_id.is_some() {
                sqlx::query(
                    r#"
                    INSERT INTO agent.user_learned_phrases
                        (user_id, phrase, verb, embedding, source)
                    VALUES ($1, $2, $3, $4::vector, 'bulk_import')
                    ON CONFLICT (user_id, phrase) DO UPDATE
                    SET verb = $3, embedding = COALESCE($4::vector, agent.user_learned_phrases.embedding), updated_at = now()
                    "#,
                )
                .bind(user_id)
                .bind(&phrase_data.phrase)
                .bind(&phrase_data.verb)
                .bind(embedding.as_ref())
                .execute(pool)
                .await
            } else {
                sqlx::query(
                    r#"
                    INSERT INTO agent.invocation_phrases
                        (phrase, verb, embedding, source)
                    VALUES ($1, $2, $3::vector, 'bulk_import')
                    ON CONFLICT (phrase) DO UPDATE
                    SET verb = $2, embedding = COALESCE($3::vector, agent.invocation_phrases.embedding), updated_at = now()
                    "#,
                )
                .bind(&phrase_data.phrase)
                .bind(&phrase_data.verb)
                .bind(embedding.as_ref())
                .execute(pool)
                .await
            };

            match result {
                Ok(_) => imported += 1,
                Err(e) => errors.push(format!("{}: {}", phrase_data.phrase, e)),
            }
        }

        Ok(json!({
            "success": true,
            "imported": imported,
            "errors": errors,
            "scope": scope,
            "message": format!("Imported {} phrase mappings", imported)
        }))
    }

    /// List pending learning candidates
    pub(super) async fn learning_list(&self, args: Value) -> Result<Value> {
        let pool = self.require_pool()?;
        let status = args["status"].as_str().unwrap_or("pending");
        let learning_type = args["learning_type"].as_str().unwrap_or("all");
        let min_occurrences = args["min_occurrences"].as_i64().unwrap_or(1) as i32;
        let limit = args["limit"].as_i64().unwrap_or(20) as i32;

        let rows = sqlx::query_as::<
            _,
            (
                i64,
                String,
                String,
                String,
                Option<String>,
                i32,
                String,
                String,
                Option<String>,
                chrono::DateTime<chrono::Utc>,
            ),
        >(
            r#"
            SELECT id, learning_type, input_pattern, suggested_output, previous_output,
                   occurrence_count, risk_level, status, user_explanation, created_at
            FROM agent.learning_candidates
            WHERE ($1 = 'all' OR status = $1)
              AND ($2 = 'all' OR learning_type = $2)
              AND occurrence_count >= $3
            ORDER BY
                CASE status WHEN 'pending' THEN 0 ELSE 1 END,
                occurrence_count DESC,
                created_at DESC
            LIMIT $4
            "#,
        )
        .bind(status)
        .bind(learning_type)
        .bind(min_occurrences)
        .bind(limit)
        .fetch_all(pool)
        .await?;

        let items: Vec<Value> = rows
            .iter()
            .map(|r| {
                json!({
                    "id": r.0,
                    "type": r.1,
                    "input": r.2,
                    "suggested": r.3,
                    "previous": r.4,
                    "occurrences": r.5,
                    "risk": r.6,
                    "status": r.7,
                    "explanation": r.8,
                    "created": r.9.to_rfc3339(),
                })
            })
            .collect();

        Ok(json!({
            "candidates": items,
            "count": items.len(),
            "filters": {
                "status": status,
                "learning_type": learning_type,
                "min_occurrences": min_occurrences
            }
        }))
    }

    /// Approve a learning candidate
    pub(super) async fn learning_approve(&self, args: Value) -> Result<Value> {
        let pool = self.require_pool()?;

        let candidate_id: i64 = args["candidate_id"]
            .as_str()
            .or_else(|| args["candidate_id"].as_i64().map(|_| ""))
            .and_then(|s| {
                if s.is_empty() {
                    args["candidate_id"].as_i64()
                } else {
                    s.parse().ok()
                }
            })
            .ok_or_else(|| anyhow!("candidate_id required"))?;
        let apply_immediately = args["apply_immediately"].as_bool().unwrap_or(true);

        // Get candidate
        let candidate = sqlx::query_as::<_, LearningCandidateRow>(
            "SELECT id, learning_type, input_pattern, suggested_output FROM agent.learning_candidates WHERE id = $1",
        )
        .bind(candidate_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| anyhow!("Candidate not found"))?;

        // Update status
        sqlx::query("UPDATE agent.learning_candidates SET status = 'approved', updated_at = now() WHERE id = $1")
            .bind(candidate_id)
            .execute(pool)
            .await?;

        let mut applied = false;
        if apply_immediately {
            // Generate embedding (target mode - storing in DB)
            let embedding: Option<Vec<f32>> = self
                .embedder
                .embed_target(&candidate.input_pattern)
                .await
                .ok();

            let result = match candidate.learning_type.as_str() {
                "invocation_phrase" => {
                    sqlx::query(
                        r#"
                        INSERT INTO agent.invocation_phrases (phrase, verb, embedding, source)
                        VALUES ($1, $2, $3::vector, 'approved_candidate')
                        ON CONFLICT (phrase) DO UPDATE
                        SET verb = $2, embedding = COALESCE($3::vector, agent.invocation_phrases.embedding), updated_at = now()
                        "#,
                    )
                    .bind(&candidate.input_pattern)
                    .bind(&candidate.suggested_output)
                    .bind(embedding.as_ref())
                    .execute(pool)
                    .await
                }
                "entity_alias" => {
                    sqlx::query(
                        r#"
                        INSERT INTO agent.entity_aliases (alias, canonical_name, embedding, source)
                        VALUES ($1, $2, $3::vector, 'approved_candidate')
                        ON CONFLICT (alias) DO UPDATE
                        SET canonical_name = $2, embedding = COALESCE($3::vector, agent.entity_aliases.embedding), updated_at = now()
                        "#,
                    )
                    .bind(&candidate.input_pattern)
                    .bind(&candidate.suggested_output)
                    .bind(embedding.as_ref())
                    .execute(pool)
                    .await
                }
                _ => Ok(Default::default()),
            };

            if result.is_ok() {
                sqlx::query("UPDATE agent.learning_candidates SET status = 'applied', applied_at = now() WHERE id = $1")
                    .bind(candidate_id)
                    .execute(pool)
                    .await?;
                applied = true;
            }
        }

        Ok(json!({
            "approved": true,
            "applied": applied,
            "candidate_id": candidate_id,
            "mapping": format!("'{}' → {}", candidate.input_pattern, candidate.suggested_output)
        }))
    }

    /// Reject a learning candidate
    pub(super) async fn learning_reject(&self, args: Value) -> Result<Value> {
        let pool = self.require_pool()?;

        let candidate_id: i64 = args["candidate_id"]
            .as_str()
            .or_else(|| args["candidate_id"].as_i64().map(|_| ""))
            .and_then(|s| {
                if s.is_empty() {
                    args["candidate_id"].as_i64()
                } else {
                    s.parse().ok()
                }
            })
            .ok_or_else(|| anyhow!("candidate_id required"))?;
        let reason = args["reason"].as_str();
        let add_to_blocklist = args["add_to_blocklist"].as_bool().unwrap_or(false);

        // Get candidate
        let candidate = sqlx::query_as::<_, LearningCandidateRow>(
            "SELECT id, learning_type, input_pattern, suggested_output FROM agent.learning_candidates WHERE id = $1",
        )
        .bind(candidate_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| anyhow!("Candidate not found"))?;

        // Update status
        sqlx::query(
            "UPDATE agent.learning_candidates SET status = 'rejected', user_explanation = COALESCE($2, user_explanation), updated_at = now() WHERE id = $1",
        )
        .bind(candidate_id)
        .bind(reason)
        .execute(pool)
        .await?;

        // Optionally add to blocklist (target mode - storing in DB)
        let blocked = if add_to_blocklist && candidate.learning_type.contains("phrase") {
            let embedding: Option<Vec<f32>> = self
                .embedder
                .embed_target(&candidate.input_pattern)
                .await
                .ok();

            sqlx::query(
                r#"
                INSERT INTO agent.phrase_blocklist (phrase, blocked_verb, embedding, reason, source)
                VALUES ($1, $2, $3::vector, $4, 'rejected_candidate')
                ON CONFLICT (phrase, blocked_verb, COALESCE(user_id, '00000000-0000-0000-0000-000000000000'::uuid)) DO NOTHING
                "#,
            )
            .bind(&candidate.input_pattern)
            .bind(&candidate.suggested_output)
            .bind(embedding.as_ref())
            .bind(reason.unwrap_or("Rejected learning candidate"))
            .execute(pool)
            .await
            .is_ok()
        } else {
            false
        };

        Ok(json!({
            "rejected": true,
            "candidate_id": candidate_id,
            "added_to_blocklist": blocked,
            "reason": reason
        }))
    }

    /// Get learning statistics
    pub(super) async fn learning_stats(&self, args: Value) -> Result<Value> {
        let pool = self.require_pool()?;
        let time_range = args["time_range"].as_str().unwrap_or("week");
        let include_top = args["include_top_corrections"].as_bool().unwrap_or(true);

        let interval = match time_range {
            "day" => "1 day",
            "week" => "7 days",
            "month" => "30 days",
            _ => "1000 years", // "all"
        };

        // Get counts
        let phrase_count =
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM agent.invocation_phrases")
                .fetch_one(pool)
                .await
                .unwrap_or(0);

        let alias_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM agent.entity_aliases")
            .fetch_one(pool)
            .await
            .unwrap_or(0);

        let blocklist_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM agent.phrase_blocklist WHERE expires_at IS NULL OR expires_at > now()")
            .fetch_one(pool)
            .await
            .unwrap_or(0);

        let pending_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM agent.learning_candidates WHERE status = 'pending'",
        )
        .fetch_one(pool)
        .await
        .unwrap_or(0);

        // Get recent activity
        let recent_applied = sqlx::query_scalar::<_, i64>(
            &format!("SELECT COUNT(*) FROM agent.learning_candidates WHERE status = 'applied' AND applied_at > now() - interval '{}'", interval)
        )
        .fetch_one(pool)
        .await
        .unwrap_or(0);

        // Top corrections (if requested)
        let top_corrections: Vec<Value> = if include_top {
            let rows = sqlx::query_as::<_, TopCorrectionRow>(&format!(
                r#"
                    SELECT input_pattern, suggested_output, occurrence_count
                    FROM agent.learning_candidates
                    WHERE created_at > now() - interval '{}'
                    ORDER BY occurrence_count DESC
                    LIMIT 10
                    "#,
                interval
            ))
            .fetch_all(pool)
            .await
            .unwrap_or_default();

            rows.iter()
                .map(|r| json!({"input": r.input_pattern, "output": r.suggested_output, "count": r.occurrence_count}))
                .collect()
        } else {
            Vec::new()
        };

        Ok(json!({
            "totals": {
                "learned_phrases": phrase_count,
                "learned_aliases": alias_count,
                "blocklist_entries": blocklist_count,
                "pending_candidates": pending_count
            },
            "period": {
                "range": time_range,
                "applied": recent_applied
            },
            "top_corrections": top_corrections
        }))
    }

    // =========================================================================
    // Intent Inspection Handlers (AgentLearningInspector)
    // =========================================================================

    pub(super) async fn intent_analyze(&self, args: Value) -> Result<Value> {
        use crate::agent::AgentLearningInspector;

        #[derive(serde::Deserialize)]
        struct Args {
            since_hours: Option<i64>,
        }

        let args: Args = serde_json::from_value(args)?;
        let pool = self.require_pool()?;

        let since = args
            .since_hours
            .map(|h| chrono::Utc::now() - chrono::Duration::hours(h));

        let inspector = AgentLearningInspector::new(pool.clone());
        let stats = inspector.analyze(since).await?;

        Ok(json!({
            "events_processed": stats.events_processed,
            "candidates_created": stats.candidates_created,
            "candidates_updated": stats.candidates_updated,
            "auto_applied": stats.auto_applied,
            "queued_for_review": stats.queued_for_review
        }))
    }

    pub(super) async fn intent_list(&self, args: Value) -> Result<Value> {
        use crate::agent::{AgentLearningInspector, LearningStatus, LearningType};

        #[derive(serde::Deserialize)]
        struct Args {
            status: Option<String>,
            learning_type: Option<String>,
            limit: Option<i64>,
        }

        let args: Args = serde_json::from_value(args)?;
        let pool = self.require_pool()?;

        let status = args.status.and_then(|s| match s.as_str() {
            "pending" => Some(LearningStatus::Pending),
            "approved" => Some(LearningStatus::Approved),
            "rejected" => Some(LearningStatus::Rejected),
            "applied" => Some(LearningStatus::Applied),
            _ => None,
        });

        let learning_type = args.learning_type.and_then(|t| match t.as_str() {
            "entity_alias" => Some(LearningType::EntityAlias),
            "lexicon_token" => Some(LearningType::LexiconToken),
            "invocation_phrase" => Some(LearningType::InvocationPhrase),
            "prompt_change" => Some(LearningType::PromptChange),
            _ => None,
        });

        let inspector = AgentLearningInspector::new(pool.clone());
        let candidates = inspector
            .list_candidates(status, learning_type, args.limit.unwrap_or(20))
            .await?;

        Ok(json!({
            "count": candidates.len(),
            "candidates": candidates.iter().map(|c| json!({
                "fingerprint": c.fingerprint,
                "learning_type": c.learning_type.as_str(),
                "input_pattern": c.input_pattern,
                "suggested_output": c.suggested_output,
                "occurrence_count": c.occurrence_count,
                "risk_level": c.risk_level.as_str(),
                "status": c.status.as_str(),
                "first_seen": c.first_seen.to_rfc3339(),
                "last_seen": c.last_seen.to_rfc3339()
            })).collect::<Vec<_>>()
        }))
    }

    pub(super) async fn intent_approve(&self, args: Value) -> Result<Value> {
        use crate::agent::AgentLearningInspector;

        #[derive(serde::Deserialize)]
        struct Args {
            fingerprint: String,
        }

        let args: Args = serde_json::from_value(args)?;
        let pool = self.require_pool()?;

        let inspector = AgentLearningInspector::new(pool.clone());
        let applied = inspector
            .approve_candidate(&args.fingerprint, "mcp_user")
            .await?;

        Ok(json!({
            "success": true,
            "learning_type": applied.learning_type.as_str(),
            "input_pattern": applied.input_pattern,
            "output": applied.output,
            "applied_at": applied.applied_at.to_rfc3339()
        }))
    }

    pub(super) async fn intent_reject(&self, args: Value) -> Result<Value> {
        use crate::agent::AgentLearningInspector;

        #[derive(serde::Deserialize)]
        struct Args {
            fingerprint: String,
        }

        let args: Args = serde_json::from_value(args)?;
        let pool = self.require_pool()?;

        let inspector = AgentLearningInspector::new(pool.clone());
        inspector
            .reject_candidate(&args.fingerprint, "mcp_user")
            .await?;

        Ok(json!({
            "success": true,
            "fingerprint": args.fingerprint,
            "status": "rejected"
        }))
    }

    pub(super) async fn intent_reload(&self, _args: Value) -> Result<Value> {
        use crate::agent::LearningWarmup;

        let pool = self.require_pool()?;

        let warmup = LearningWarmup::new(pool.clone());
        let (_, stats) = warmup.warmup().await?;

        Ok(json!({
            "success": true,
            "entity_aliases_loaded": stats.entity_aliases_loaded,
            "lexicon_tokens_loaded": stats.lexicon_tokens_loaded,
            "invocation_phrases_loaded": stats.invocation_phrases_loaded,
            "learnings_auto_applied": stats.learnings_auto_applied,
            "duration_ms": stats.duration_ms
        }))
    }

    // =========================================================================
    // Learning Analyze/Apply & Embeddings Status
    // =========================================================================

    pub(super) async fn learning_analyze(&self, args: Value) -> Result<Value> {
        use ob_semantic_matcher::FeedbackService;

        let days_back = args.get("days_back").and_then(|v| v.as_i64()).unwrap_or(7) as i32;

        let feedback_service = FeedbackService::new(self.pool.clone());
        let report = feedback_service.analyze(days_back).await?;

        Ok(json!({
            "success": true,
            "days_analyzed": days_back,
            "summary": report.summary(),
            "pattern_discoveries": report.pattern_discoveries.len(),
            "confusion_pairs": report.confusion_pairs.len(),
            "gaps": report.gaps.len(),
            "low_score_successes": report.low_score_successes.len(),
            "details": {
                "patterns": report.pattern_discoveries.iter().take(10).collect::<Vec<_>>(),
                "confusions": report.confusion_pairs.iter().take(5).collect::<Vec<_>>(),
                "gaps": report.gaps.iter().take(5).collect::<Vec<_>>()
            }
        }))
    }

    /// Apply discovered patterns to improve verb matching
    pub(super) async fn learning_apply(&self, args: Value) -> Result<Value> {
        use crate::agent::learning::trigger_learning_cycle;

        let days_back = args.get("days_back").and_then(|v| v.as_i64()).unwrap_or(7) as i32;

        let min_occurrences = args
            .get("min_occurrences")
            .and_then(|v| v.as_i64())
            .unwrap_or(5);

        let result = trigger_learning_cycle(&self.pool, days_back, min_occurrences).await?;

        let needs_reembed = result.pending_embeddings > 0;

        Ok(json!({
            "success": true,
            "days_analyzed": days_back,
            "min_occurrences": min_occurrences,
            "patterns_discovered": result.patterns_discovered,
            "patterns_applied": result.patterns_applied,
            "applied_patterns": result.applied_patterns,
            "pending_embeddings": result.pending_embeddings,
            "needs_reembed": needs_reembed,
            "message": if needs_reembed {
                format!("{} patterns applied. {} patterns need embedding. Run: cargo run --release --bin populate_embeddings",
                    result.patterns_applied, result.pending_embeddings)
            } else if result.patterns_applied > 0 {
                format!("{} patterns applied and ready for use", result.patterns_applied)
            } else {
                "No new patterns to apply".to_string()
            }
        }))
    }

    /// Check embedding coverage for verb patterns
    pub(super) async fn embeddings_status(&self, _args: Value) -> Result<Value> {
        use ob_semantic_matcher::PatternLearner;

        let learner = PatternLearner::new(self.pool.clone());
        let pending = learner.count_pending_embeddings().await?;

        // Get total counts
        let total_patterns: (i64,) =
            sqlx::query_as(r#"SELECT COUNT(*) FROM "ob-poc".v_verb_intent_patterns"#)
                .fetch_one(&self.pool)
                .await?;

        let total_embeddings: (i64,) = sqlx::query_as(
            r#"SELECT COUNT(*) FROM "ob-poc".verb_pattern_embeddings WHERE embedding IS NOT NULL"#,
        )
        .fetch_one(&self.pool)
        .await?;

        let coverage = if total_patterns.0 > 0 {
            (total_embeddings.0 as f64 / total_patterns.0 as f64 * 100.0).round()
        } else {
            0.0
        };

        Ok(json!({
            "success": true,
            "total_patterns": total_patterns.0,
            "embedded_patterns": total_embeddings.0,
            "pending_patterns": pending,
            "coverage_percent": coverage,
            "needs_reembed": pending > 0,
            "message": if pending > 0 {
                format!("{} patterns need embedding ({}% coverage). Run: cargo run --release --bin populate_embeddings",
                    pending, coverage)
            } else {
                format!("All patterns embedded ({}% coverage)", coverage)
            }
        }))
    }

    // =========================================================================
    // Promotion Pipeline Tools
    // =========================================================================

    /// Run a full promotion cycle
    pub(super) async fn promotion_run_cycle(&self, _args: Value) -> Result<Value> {
        use ob_semantic_matcher::{Embedder, PromotionService};

        let mut service = PromotionService::new(self.pool.clone());

        // Try to add embedder for collision checking
        if let Ok(embedder) = Embedder::new() {
            service = service.with_embedder(embedder);
        }

        let report = service.run_promotion_cycle().await?;

        Ok(json!({
            "success": true,
            "expired_outcomes": report.expired_outcomes,
            "promoted_count": report.promoted.len(),
            "promoted_patterns": report.promoted,
            "skipped": report.skipped,
            "collisions": report.collisions,
            "errors": report.errors,
            "summary": report.summary()
        }))
    }

    /// Get promotable candidates
    pub(super) async fn promotion_candidates(&self, args: Value) -> Result<Value> {
        use ob_semantic_matcher::PromotionService;

        let limit = args.get("limit").and_then(|v| v.as_i64()).unwrap_or(20) as i32;

        let service = PromotionService::new(self.pool.clone());
        let candidates = service.get_promotable_candidates().await?;

        let candidates_json: Vec<_> = candidates
            .iter()
            .take(limit as usize)
            .map(|c| {
                json!({
                    "id": c.id,
                    "phrase": c.phrase,
                    "verb": c.verb,
                    "occurrence_count": c.occurrence_count,
                    "success_count": c.success_count,
                    "total_count": c.total_count,
                    "success_rate": format!("{:.1}%", c.success_rate * 100.0),
                    "domain_hint": c.domain_hint
                })
            })
            .collect();

        Ok(json!({
            "success": true,
            "count": candidates_json.len(),
            "candidates": candidates_json
        }))
    }

    /// Get candidates needing manual review
    pub(super) async fn promotion_review_queue(&self, args: Value) -> Result<Value> {
        use ob_semantic_matcher::PromotionService;

        let min_occurrences = args
            .get("min_occurrences")
            .and_then(|v| v.as_i64())
            .unwrap_or(3) as i32;
        let min_age_days = args
            .get("min_age_days")
            .and_then(|v| v.as_i64())
            .unwrap_or(7) as i32;
        let limit = args.get("limit").and_then(|v| v.as_i64()).unwrap_or(50) as i32;

        let service = PromotionService::new(self.pool.clone());
        let candidates = service
            .get_review_candidates(min_occurrences, min_age_days, limit)
            .await?;

        let candidates_json: Vec<_> = candidates
            .iter()
            .map(|c| {
                json!({
                    "id": c.id,
                    "phrase": c.phrase,
                    "verb": c.verb,
                    "occurrence_count": c.occurrence_count,
                    "success_count": c.success_count,
                    "total_count": c.total_count,
                    "success_rate": format!("{:.1}%", c.success_rate * 100.0),
                    "domain_hint": c.domain_hint,
                    "first_seen": c.first_seen.to_rfc3339(),
                    "last_seen": c.last_seen.to_rfc3339(),
                    "collision_verb": c.collision_verb
                })
            })
            .collect();

        Ok(json!({
            "success": true,
            "count": candidates_json.len(),
            "candidates": candidates_json
        }))
    }

    /// Approve a candidate for promotion
    pub(super) async fn promotion_approve(&self, args: Value) -> Result<Value> {
        use ob_semantic_matcher::PromotionService;

        let candidate_id = args
            .get("candidate_id")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| anyhow!("candidate_id required"))?;
        let actor = args
            .get("actor")
            .and_then(|v| v.as_str())
            .unwrap_or("manual_review");

        let service = PromotionService::new(self.pool.clone());
        let approved = service.approve_candidate(candidate_id, actor).await?;

        if approved {
            Ok(json!({
                "success": true,
                "message": format!("Candidate {} approved and promoted", candidate_id),
                "needs_reembed": true,
                "hint": "Run populate_embeddings to enable semantic matching for the new pattern"
            }))
        } else {
            Ok(json!({
                "success": false,
                "message": format!("Candidate {} not found or already processed", candidate_id)
            }))
        }
    }

    /// Reject a candidate and add to blocklist
    pub(super) async fn promotion_reject(&self, args: Value) -> Result<Value> {
        use ob_semantic_matcher::PromotionService;

        let candidate_id = args
            .get("candidate_id")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| anyhow!("candidate_id required"))?;
        let reason = args
            .get("reason")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("reason required"))?;
        let actor = args
            .get("actor")
            .and_then(|v| v.as_str())
            .unwrap_or("manual_review");

        let service = PromotionService::new(self.pool.clone());
        let rejected = service
            .reject_candidate(candidate_id, reason, actor)
            .await?;

        if rejected {
            Ok(json!({
                "success": true,
                "message": format!("Candidate {} rejected and added to blocklist", candidate_id)
            }))
        } else {
            Ok(json!({
                "success": false,
                "message": format!("Candidate {} not found or already processed", candidate_id)
            }))
        }
    }

    /// Get learning pipeline health metrics
    pub(super) async fn promotion_health(&self, args: Value) -> Result<Value> {
        use ob_semantic_matcher::PromotionService;

        let weeks = args.get("weeks").and_then(|v| v.as_i64()).unwrap_or(8) as i32;

        let service = PromotionService::new(self.pool.clone());
        let metrics = service.get_health_metrics(weeks).await?;

        let metrics_json: Vec<_> = metrics
            .iter()
            .map(|m| {
                json!({
                    "week": m.week.format("%Y-%m-%d").to_string(),
                    "total_interactions": m.total_interactions,
                    "successes": m.successes,
                    "corrections": m.corrections,
                    "no_matches": m.no_matches,
                    "false_positives": m.false_positives,
                    "top1_hit_rate_pct": m.top1_hit_rate_pct,
                    "avg_success_score": m.avg_success_score,
                    "confidence_distribution": {
                        "high": m.high_confidence,
                        "medium": m.medium_confidence,
                        "low": m.low_confidence,
                        "none": m.no_match_confidence
                    }
                })
            })
            .collect();

        Ok(json!({
            "success": true,
            "weeks": metrics_json.len(),
            "metrics": metrics_json
        }))
    }

    /// Get candidate pipeline status summary
    pub(super) async fn promotion_pipeline_status(&self, _args: Value) -> Result<Value> {
        use ob_semantic_matcher::PromotionService;

        let service = PromotionService::new(self.pool.clone());
        let status = service.get_pipeline_status().await?;

        let status_json: Vec<_> = status
            .iter()
            .map(|s| {
                json!({
                    "status": s.status,
                    "count": s.count,
                    "avg_occurrences": s.avg_occurrences,
                    "avg_success_rate": s.avg_success_rate.map(|r| format!("{:.1}%", r * 100.0)),
                    "oldest": s.oldest.map(|t| t.to_rfc3339()),
                    "newest": s.newest.map(|t| t.to_rfc3339())
                })
            })
            .collect();

        Ok(json!({
            "success": true,
            "pipeline": status_json
        }))
    }

    // =========================================================================
    // Teaching Tools (via DSL verbs)
    // =========================================================================

    /// Teach a phrase->verb mapping via DSL execution
    ///
    /// Routes through: (agent.teach :phrase "..." :verb "...")
    pub(super) async fn teach_phrase(&self, args: Value) -> Result<Value> {
        let phrase = args
            .get("phrase")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("phrase required"))?;
        let verb = args
            .get("verb")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("verb required"))?;
        let source = args
            .get("source")
            .and_then(|v| v.as_str())
            .unwrap_or("mcp_teaching");

        // Build DSL and execute via dsl_execute
        let dsl = format!(
            r#"(agent.teach :phrase "{}" :verb "{}" :source "{}")"#,
            phrase.replace('"', r#"\""#),
            verb,
            source
        );

        self.dsl_execute(json!({
            "source": dsl,
            "intent": format!("teach phrase '{}' → {}", phrase, verb)
        }))
        .await
    }

    /// Remove a taught phrase->verb mapping via DSL execution
    ///
    /// Routes through: (agent.unteach :phrase "..." [:verb "..."] [:reason "..."])
    pub(super) async fn unteach_phrase(&self, args: Value) -> Result<Value> {
        let phrase = args
            .get("phrase")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("phrase required"))?;
        let verb = args.get("verb").and_then(|v| v.as_str());
        let reason = args
            .get("reason")
            .and_then(|v| v.as_str())
            .unwrap_or("mcp_unteach");

        // Build DSL with optional verb arg
        let verb_arg = verb
            .map(|v| format!(r#" :verb "{}""#, v))
            .unwrap_or_default();

        let dsl = format!(
            r#"(agent.unteach :phrase "{}"{} :reason "{}")"#,
            phrase.replace('"', r#"\""#),
            verb_arg,
            reason
        );

        self.dsl_execute(json!({
            "source": dsl,
            "intent": format!("unteach phrase '{}'", phrase)
        }))
        .await
    }

    /// Get teaching status via DSL execution
    ///
    /// Routes through: (agent.teaching-status [:limit N] [:include-stats true/false])
    pub(super) async fn teaching_status(&self, args: Value) -> Result<Value> {
        let limit = args.get("limit").and_then(|v| v.as_i64()).unwrap_or(20);
        let include_stats = args
            .get("include_stats")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let dsl = format!(
            r#"(agent.teaching-status :limit {} :include-stats {})"#,
            limit, include_stats
        );

        self.dsl_execute(json!({
            "source": dsl,
            "intent": "get teaching status"
        }))
        .await
    }
}

// =========================================================================
// Learning System Helper Types
// =========================================================================

/// Import data format for bulk learning import
#[derive(Debug, serde::Deserialize)]
struct ImportData {
    phrases: Vec<PhraseMapping>,
}

/// Single phrase->verb mapping for import
#[derive(Debug, serde::Deserialize)]
struct PhraseMapping {
    phrase: String,
    verb: String,
}

/// Parse CSV import format (phrase,verb per line)
fn parse_csv_import(content: &str) -> anyhow::Result<ImportData> {
    let mut phrases = Vec::new();
    for line in content.lines().skip(1) {
        // Skip header
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() >= 2 {
            phrases.push(PhraseMapping {
                phrase: parts[0].trim().to_string(),
                verb: parts[1].trim().to_string(),
            });
        }
    }
    Ok(ImportData { phrases })
}

/// Parse duration string like "30d", "1w", "24h"
fn parse_duration(s: &str) -> anyhow::Result<chrono::Duration> {
    let len = s.len();
    if len < 2 {
        return Err(anyhow::anyhow!("Invalid duration format"));
    }

    let (num_str, unit) = s.split_at(len - 1);
    let num: i64 = num_str.parse()?;

    match unit {
        "d" => Ok(chrono::Duration::days(num)),
        "w" => Ok(chrono::Duration::weeks(num)),
        "h" => Ok(chrono::Duration::hours(num)),
        "m" => Ok(chrono::Duration::minutes(num)),
        _ => Err(anyhow::anyhow!("Unknown duration unit: {}", unit)),
    }
}
