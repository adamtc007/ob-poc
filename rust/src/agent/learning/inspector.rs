//! Agent Learning Inspector
//!
//! On-demand analysis of agent events to identify learning opportunities.
//! Manages the learning candidate lifecycle: detection → threshold → apply.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use super::types::{AgentEvent, AgentEventPayload};

/// Learning candidate from agent interactions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningCandidate {
    pub id: i64,
    pub fingerprint: String,
    pub learning_type: LearningType,
    pub input_pattern: String,
    pub suggested_output: String,
    pub occurrence_count: i32,
    pub risk_level: RiskLevel,
    pub status: LearningStatus,
    pub first_seen: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
}

/// Type of learning to apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LearningType {
    /// Entity alias: "Barclays" → "Barclays PLC"
    EntityAlias,
    /// Lexicon token: new vocabulary
    LexiconToken,
    /// Invocation phrase: "set up ISDA" → isda.create
    InvocationPhrase,
    /// Prompt adjustment (requires human review)
    PromptChange,
}

impl LearningType {
    pub fn as_str(&self) -> &'static str {
        match self {
            LearningType::EntityAlias => "entity_alias",
            LearningType::LexiconToken => "lexicon_token",
            LearningType::InvocationPhrase => "invocation_phrase",
            LearningType::PromptChange => "prompt_change",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "entity_alias" => Some(LearningType::EntityAlias),
            "lexicon_token" => Some(LearningType::LexiconToken),
            "invocation_phrase" => Some(LearningType::InvocationPhrase),
            "prompt_change" => Some(LearningType::PromptChange),
            _ => None,
        }
    }
}

/// Risk level for auto-apply decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    /// Safe to auto-apply after threshold
    Low,
    /// Requires review queue
    Medium,
    /// Human approval only
    High,
}

impl RiskLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            RiskLevel::Low => "low",
            RiskLevel::Medium => "medium",
            RiskLevel::High => "high",
        }
    }
}

/// Learning status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LearningStatus {
    Pending,
    Approved,
    Rejected,
    Applied,
}

impl LearningStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            LearningStatus::Pending => "pending",
            LearningStatus::Approved => "approved",
            LearningStatus::Rejected => "rejected",
            LearningStatus::Applied => "applied",
        }
    }
}

/// Applied learning record for audit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppliedLearning {
    pub learning_type: LearningType,
    pub input_pattern: String,
    pub output: String,
    pub applied_at: DateTime<Utc>,
    pub source: String,
}

/// Statistics from analysis run.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct AnalysisStats {
    pub events_processed: u32,
    pub candidates_created: u32,
    pub candidates_updated: u32,
    pub auto_applied: u32,
    pub queued_for_review: u32,
}

/// Agent learning inspector.
///
/// Analyzes agent events, identifies patterns, manages learning lifecycle.
pub struct AgentLearningInspector {
    pool: PgPool,
    /// Threshold for auto-applying low-risk learnings
    auto_apply_threshold: i32,
}

impl AgentLearningInspector {
    /// Create new inspector.
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            auto_apply_threshold: 3,
        }
    }

    /// Create with custom threshold.
    pub fn with_threshold(pool: PgPool, threshold: i32) -> Self {
        Self {
            pool,
            auto_apply_threshold: threshold,
        }
    }

    // =========================================================================
    // EVENT PERSISTENCE
    // =========================================================================

    /// Store an agent event for later analysis.
    pub async fn store_event(&self, event: &AgentEvent) -> Result<i64> {
        let event_type = event.payload.event_type_str();

        // Extract fields based on payload type
        let (
            user_message,
            parsed_intents,
            selected_verb,
            generated_dsl,
            was_corrected,
            corrected_dsl,
            correction_type,
            entities_resolved,
            resolution_failures,
            execution_success,
            error_message,
            duration_ms,
        ) = match &event.payload {
            AgentEventPayload::MessageReceived { message, .. } => (
                Some(message.clone()),
                None,
                None,
                None,
                false,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            ),
            AgentEventPayload::IntentExtracted {
                user_message,
                intents,
                duration_ms,
                ..
            } => (
                Some(user_message.clone()),
                Some(serde_json::to_value(intents)?),
                None,
                None,
                false,
                None,
                None,
                None,
                None,
                None,
                None,
                Some(*duration_ms as i32),
            ),
            AgentEventPayload::VerbSelected {
                intent_summary,
                selected_verb,
                ..
            } => (
                Some(intent_summary.clone()),
                None,
                Some(selected_verb.clone()),
                None,
                false,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            ),
            AgentEventPayload::EntityResolved {
                query,
                resolved_to,
                candidates,
                ..
            } => (
                Some(query.clone()),
                None,
                None,
                None,
                false,
                None,
                None,
                Some(serde_json::to_value(resolved_to)?),
                if candidates.is_empty() {
                    None
                } else {
                    Some(serde_json::to_value(candidates)?)
                },
                None,
                None,
                None,
            ),
            AgentEventPayload::EntityResolutionFailed {
                query,
                reason,
                candidates,
            } => (
                Some(query.clone()),
                None,
                None,
                None,
                false,
                None,
                None,
                None,
                Some(serde_json::json!({ "reason": reason, "candidates": candidates })),
                None,
                None,
                None,
            ),
            AgentEventPayload::DslGenerated {
                dsl, duration_ms, ..
            } => (
                None,
                None,
                None,
                Some(dsl.clone()),
                false,
                None,
                None,
                None,
                None,
                None,
                None,
                Some(*duration_ms as i32),
            ),
            AgentEventPayload::UserCorrection {
                original_message,
                generated_dsl,
                corrected_dsl,
                correction_type,
            } => (
                Some(original_message.clone()),
                None,
                None,
                Some(generated_dsl.clone()),
                true,
                Some(corrected_dsl.clone()),
                Some(format!("{:?}", correction_type)),
                None,
                None,
                None,
                None,
                None,
            ),
            AgentEventPayload::ExecutionCompleted {
                dsl,
                success,
                error_message,
                duration_ms,
            } => (
                None,
                None,
                None,
                Some(dsl.clone()),
                false,
                None,
                None,
                None,
                None,
                Some(*success),
                error_message.clone(),
                Some(*duration_ms as i32),
            ),
            AgentEventPayload::SessionSummary { .. } => (
                None, None, None, None, false, None, None, None, None, None, None, None,
            ),
            AgentEventPayload::EsperCommandMatched {
                phrase,
                command_key,
                source,
                match_type,
                extracted_params,
            } => (
                Some(phrase.clone()),
                Some(serde_json::json!({
                    "command_key": command_key,
                    "source": source,
                    "match_type": match_type,
                    "extracted_params": extracted_params,
                })),
                Some(command_key.clone()),
                None,
                false,
                None,
                None,
                None,
                None,
                Some(true), // ESPER commands always succeed
                None,
                None,
            ),
            AgentEventPayload::EsperCommandMiss { phrase } => (
                Some(phrase.clone()),
                None,
                None,
                None,
                false,
                None,
                None,
                None,
                None,
                Some(false), // Miss = fell through to DSL
                None,
                None,
            ),
        };

        let id = sqlx::query_scalar!(
            r#"
            INSERT INTO agent.events (
                event_type, user_message, parsed_intents, selected_verb,
                generated_dsl, was_corrected, corrected_dsl, correction_type,
                entities_resolved, resolution_failures, execution_success,
                error_message, duration_ms
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            RETURNING id
            "#,
            event_type,
            user_message,
            parsed_intents,
            selected_verb,
            generated_dsl,
            was_corrected,
            corrected_dsl,
            correction_type,
            entities_resolved,
            resolution_failures,
            execution_success,
            error_message,
            duration_ms
        )
        .fetch_one(&self.pool)
        .await
        .context("Failed to store agent event")?;

        Ok(id)
    }

    // =========================================================================
    // LEARNING DETECTION
    // =========================================================================

    /// Analyze recent events and create/update learning candidates.
    pub async fn analyze(&self, since: Option<DateTime<Utc>>) -> Result<AnalysisStats> {
        let mut stats = AnalysisStats::default();

        // Get events with corrections (primary learning signal)
        let events = self.get_correction_events(since).await?;
        stats.events_processed = events.len() as u32;

        for event in events {
            if let Some(candidate) = self.extract_learning_candidate(&event)? {
                let (created, updated) = self.upsert_candidate(&candidate).await?;
                if created {
                    stats.candidates_created += 1;
                } else if updated {
                    stats.candidates_updated += 1;
                }
            }
        }

        // Check for auto-apply candidates
        let auto_applied = self.apply_threshold_learnings().await?;
        stats.auto_applied = auto_applied as u32;

        // Count queued for review
        stats.queued_for_review = self.count_pending_review().await? as u32;

        Ok(stats)
    }

    /// Get events where user made corrections.
    async fn get_correction_events(
        &self,
        since: Option<DateTime<Utc>>,
    ) -> Result<Vec<CorrectionEvent>> {
        let since = since.unwrap_or_else(|| Utc::now() - chrono::Duration::days(1));

        let rows = sqlx::query!(
            r#"
            SELECT id, session_id, user_message, generated_dsl, corrected_dsl, correction_type
            FROM agent.events
            WHERE was_corrected = TRUE
              AND timestamp >= $1
            ORDER BY timestamp DESC
            "#,
            since
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| CorrectionEvent {
                id: r.id,
                user_message: r.user_message,
                generated_dsl: r.generated_dsl,
                corrected_dsl: r.corrected_dsl,
                correction_type: r.correction_type,
            })
            .collect())
    }

    /// Extract learning candidate from a correction event.
    fn extract_learning_candidate(
        &self,
        event: &CorrectionEvent,
    ) -> Result<Option<LearningCandidateInput>> {
        let Some(ref user_msg) = event.user_message else {
            return Ok(None);
        };
        let Some(ref _generated) = event.generated_dsl else {
            return Ok(None);
        };
        let Some(ref corrected) = event.corrected_dsl else {
            return Ok(None);
        };

        // Determine learning type and risk from correction type
        let (learning_type, risk_level) = match event.correction_type.as_deref() {
            Some(s) if s.starts_with("VerbChange") => {
                (LearningType::InvocationPhrase, RiskLevel::Medium)
            }
            Some(s) if s.starts_with("EntityChange") => (LearningType::EntityAlias, RiskLevel::Low),
            Some(s) if s.starts_with("ArgumentChange") => {
                (LearningType::LexiconToken, RiskLevel::Low)
            }
            Some("FullRewrite") => (LearningType::PromptChange, RiskLevel::High),
            _ => (LearningType::InvocationPhrase, RiskLevel::Medium),
        };

        // Create fingerprint for deduplication
        let fingerprint = format!(
            "{}:{}:{}",
            learning_type.as_str(),
            user_msg.to_lowercase().trim(),
            corrected.trim()
        );

        Ok(Some(LearningCandidateInput {
            fingerprint,
            learning_type,
            input_pattern: user_msg.clone(),
            suggested_output: corrected.clone(),
            risk_level,
            event_id: event.id,
        }))
    }

    /// Upsert a learning candidate (increment count if exists).
    async fn upsert_candidate(&self, input: &LearningCandidateInput) -> Result<(bool, bool)> {
        let auto_applicable = input.risk_level == RiskLevel::Low;

        let result = sqlx::query!(
            r#"
            INSERT INTO agent.learning_candidates (
                fingerprint, learning_type, input_pattern, suggested_output,
                risk_level, auto_applicable, example_events
            )
            VALUES ($1, $2, $3, $4, $5, $6, ARRAY[$7]::BIGINT[])
            ON CONFLICT (fingerprint) DO UPDATE SET
                occurrence_count = agent.learning_candidates.occurrence_count + 1,
                last_seen = NOW(),
                example_events = array_append(
                    agent.learning_candidates.example_events[1:9],
                    $7
                ),
                updated_at = NOW()
            RETURNING
                (xmax = 0) as "is_insert!"
            "#,
            input.fingerprint,
            input.learning_type.as_str(),
            input.input_pattern,
            input.suggested_output,
            input.risk_level.as_str(),
            auto_applicable,
            input.event_id
        )
        .fetch_one(&self.pool)
        .await?;

        Ok((result.is_insert, !result.is_insert))
    }

    // =========================================================================
    // THRESHOLD-BASED AUTO-APPLY
    // =========================================================================

    /// Apply learnings that have reached the occurrence threshold.
    pub async fn apply_threshold_learnings(&self) -> Result<usize> {
        let candidates = sqlx::query!(
            r#"
            SELECT id, learning_type, input_pattern, suggested_output
            FROM agent.learning_candidates
            WHERE status = 'pending'
              AND auto_applicable = TRUE
              AND occurrence_count >= $1
              AND risk_level = 'low'
            ORDER BY occurrence_count DESC
            "#,
            self.auto_apply_threshold
        )
        .fetch_all(&self.pool)
        .await?;

        let mut applied = 0;
        for candidate in candidates {
            let learning_type =
                LearningType::parse(&candidate.learning_type).unwrap_or(LearningType::EntityAlias);

            match learning_type {
                LearningType::EntityAlias => {
                    self.apply_entity_alias(&candidate.input_pattern, &candidate.suggested_output)
                        .await?;
                }
                LearningType::LexiconToken => {
                    self.apply_lexicon_token(&candidate.input_pattern, &candidate.suggested_output)
                        .await?;
                }
                LearningType::InvocationPhrase => {
                    // Medium risk - don't auto-apply, queue for review
                    continue;
                }
                LearningType::PromptChange => {
                    // High risk - never auto-apply
                    continue;
                }
            }

            // Mark as applied
            sqlx::query!(
                r#"
                UPDATE agent.learning_candidates
                SET status = 'applied', applied_at = NOW()
                WHERE id = $1
                "#,
                candidate.id
            )
            .execute(&self.pool)
            .await?;

            // Audit log
            self.log_learning_applied(candidate.id, &candidate.learning_type, "system_threshold")
                .await?;

            applied += 1;
        }

        Ok(applied)
    }

    /// Apply an entity alias learning.
    async fn apply_entity_alias(&self, alias: &str, canonical: &str) -> Result<()> {
        sqlx::query_scalar!(
            r#"SELECT agent.upsert_entity_alias($1, $2, NULL, 'threshold_auto')"#,
            alias,
            canonical
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(())
    }

    /// Apply a lexicon token learning.
    async fn apply_lexicon_token(&self, token: &str, token_type: &str) -> Result<()> {
        sqlx::query_scalar!(
            r#"SELECT agent.upsert_lexicon_token($1, $2, NULL, 'threshold_auto')"#,
            token,
            token_type
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(())
    }

    /// Log a learning application for audit.
    async fn log_learning_applied(
        &self,
        candidate_id: i64,
        learning_type: &str,
        actor: &str,
    ) -> Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO agent.learning_audit (action, learning_type, candidate_id, actor)
            VALUES ('applied', $1, $2, $3)
            "#,
            learning_type,
            candidate_id,
            actor
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Count candidates pending human review.
    async fn count_pending_review(&self) -> Result<i64> {
        let count = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) as "count!"
            FROM agent.learning_candidates
            WHERE status = 'pending'
              AND (auto_applicable = FALSE OR risk_level != 'low')
            "#
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(count)
    }

    // =========================================================================
    // QUERIES FOR MCP TOOLS
    // =========================================================================

    /// List learning candidates with filtering.
    pub async fn list_candidates(
        &self,
        status: Option<LearningStatus>,
        learning_type: Option<LearningType>,
        limit: i64,
    ) -> Result<Vec<LearningCandidate>> {
        let status_str = status.map(|s| s.as_str().to_string());
        let type_str = learning_type.map(|t| t.as_str().to_string());

        let rows = sqlx::query!(
            r#"
            SELECT id, fingerprint, learning_type, input_pattern, suggested_output,
                   occurrence_count, risk_level, status, first_seen, last_seen
            FROM agent.learning_candidates
            WHERE ($1::TEXT IS NULL OR status = $1)
              AND ($2::TEXT IS NULL OR learning_type = $2)
            ORDER BY occurrence_count DESC, last_seen DESC
            LIMIT $3
            "#,
            status_str,
            type_str,
            limit
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| LearningCandidate {
                id: r.id,
                fingerprint: r.fingerprint,
                learning_type: LearningType::parse(&r.learning_type)
                    .unwrap_or(LearningType::EntityAlias),
                input_pattern: r.input_pattern,
                suggested_output: r.suggested_output,
                occurrence_count: r.occurrence_count.unwrap_or(1),
                risk_level: match r.risk_level.as_deref() {
                    Some("medium") => RiskLevel::Medium,
                    Some("high") => RiskLevel::High,
                    _ => RiskLevel::Low,
                },
                status: match r.status.as_deref() {
                    Some("approved") => LearningStatus::Approved,
                    Some("rejected") => LearningStatus::Rejected,
                    Some("applied") => LearningStatus::Applied,
                    _ => LearningStatus::Pending,
                },
                first_seen: r.first_seen.unwrap_or_else(Utc::now),
                last_seen: r.last_seen.unwrap_or_else(Utc::now),
            })
            .collect())
    }

    /// Manually approve and apply a learning candidate.
    pub async fn approve_candidate(
        &self,
        fingerprint: &str,
        actor: &str,
    ) -> Result<AppliedLearning> {
        let candidate = sqlx::query!(
            r#"
            SELECT id, learning_type, input_pattern, suggested_output
            FROM agent.learning_candidates
            WHERE fingerprint = $1 AND status = 'pending'
            "#,
            fingerprint
        )
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Candidate not found or not pending: {}", fingerprint))?;

        let learning_type =
            LearningType::parse(&candidate.learning_type).unwrap_or(LearningType::EntityAlias);

        // Apply the learning
        match learning_type {
            LearningType::EntityAlias => {
                self.apply_entity_alias(&candidate.input_pattern, &candidate.suggested_output)
                    .await?;
            }
            LearningType::LexiconToken => {
                self.apply_lexicon_token(&candidate.input_pattern, &candidate.suggested_output)
                    .await?;
            }
            LearningType::InvocationPhrase => {
                sqlx::query!(
                    r#"
                    INSERT INTO agent.invocation_phrases (phrase, verb, source)
                    VALUES ($1, $2, 'manual_approval')
                    ON CONFLICT (phrase, verb) DO UPDATE SET
                        occurrence_count = agent.invocation_phrases.occurrence_count + 1,
                        updated_at = NOW()
                    "#,
                    candidate.input_pattern,
                    candidate.suggested_output
                )
                .execute(&self.pool)
                .await?;
            }
            LearningType::PromptChange => {
                // Prompt changes are logged but require manual implementation
                tracing::warn!(
                    "Prompt change approved but requires manual implementation: {} -> {}",
                    candidate.input_pattern,
                    candidate.suggested_output
                );
            }
        }

        // Mark as applied
        sqlx::query!(
            r#"
            UPDATE agent.learning_candidates
            SET status = 'applied', applied_at = NOW(), reviewed_by = $2, reviewed_at = NOW()
            WHERE id = $1
            "#,
            candidate.id,
            actor
        )
        .execute(&self.pool)
        .await?;

        // Audit log
        self.log_learning_applied(candidate.id, &candidate.learning_type, actor)
            .await?;

        Ok(AppliedLearning {
            learning_type,
            input_pattern: candidate.input_pattern,
            output: candidate.suggested_output,
            applied_at: Utc::now(),
            source: actor.to_string(),
        })
    }

    /// Reject a learning candidate.
    pub async fn reject_candidate(&self, fingerprint: &str, actor: &str) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE agent.learning_candidates
            SET status = 'rejected', reviewed_by = $2, reviewed_at = NOW()
            WHERE fingerprint = $1 AND status = 'pending'
            "#,
            fingerprint,
            actor
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    // =========================================================================
    // LOADED LEARNINGS (for warmup)
    // =========================================================================

    /// Get all applied entity aliases.
    pub async fn get_entity_aliases(&self) -> Result<Vec<(String, String, Option<Uuid>)>> {
        let rows = sqlx::query!(
            r#"
            SELECT alias, canonical_name, entity_id
            FROM agent.entity_aliases
            ORDER BY occurrence_count DESC
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| (r.alias, r.canonical_name, r.entity_id))
            .collect())
    }

    /// Get all learned lexicon tokens.
    pub async fn get_lexicon_tokens(&self) -> Result<Vec<(String, String, Option<String>)>> {
        let rows = sqlx::query!(
            r#"
            SELECT token, token_type, token_subtype
            FROM agent.lexicon_tokens
            ORDER BY occurrence_count DESC
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| (r.token, r.token_type, r.token_subtype))
            .collect())
    }

    /// Get all learned invocation phrases.
    pub async fn get_invocation_phrases(&self) -> Result<Vec<(String, String)>> {
        let rows = sqlx::query!(
            r#"
            SELECT phrase, verb
            FROM agent.invocation_phrases
            ORDER BY occurrence_count DESC
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| (r.phrase, r.verb)).collect())
    }
}

/// Internal: correction event from DB.
struct CorrectionEvent {
    id: i64,

    user_message: Option<String>,
    generated_dsl: Option<String>,
    corrected_dsl: Option<String>,
    correction_type: Option<String>,
}

/// Internal: input for creating a learning candidate.
struct LearningCandidateInput {
    fingerprint: String,
    learning_type: LearningType,
    input_pattern: String,
    suggested_output: String,
    risk_level: RiskLevel,
    event_id: i64,
}
