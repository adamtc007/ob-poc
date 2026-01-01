# TODO: Intent Feedback Capture for Continuous Learning

## Overview

Capture user interactions with the intent matching system to enable continuous improvement of verb patterns and embeddings. This is **ML feedback capture**, not operational logging - append-only inserts to PostgreSQL with offline batch analysis.

---

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    RUNTIME (per interaction)                │
│                                                             │
│  User Input → Matcher → Result                              │
│                  │                                          │
│                  ├── INSERT intent_feedback (match data)    │
│                  │                                          │
│            [user action]                                    │
│                  │                                          │
│                  └── INSERT intent_feedback_outcome         │
│                                                             │
└─────────────────────────────────────────────────────────────┘
                              │
                              │ (nightly/weekly batch)
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                    OFFLINE ANALYSIS                         │
│                                                             │
│  1. Pattern Discovery: successful inputs not in patterns    │
│  2. Confusion Analysis: correction pairs                    │
│  3. Gap Analysis: abandoned/failed matches                  │
│  4. Embedding Quality: low-score successes                  │
│                                                             │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                    FEEDBACK APPLICATION                     │
│                                                             │
│  1. Add discovered patterns to verb_rag_metadata            │
│  2. Rebuild pattern embeddings in pgvector                  │
│  3. Generate report for human review                        │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

## Task 1: Database Schema

### 1.1 Feedback Capture Table

**File:** `migrations/XXXXXX_add_intent_feedback.sql`

```sql
-- Intent feedback capture for ML continuous learning
-- Append-only: no updates, batch analysis only

CREATE TABLE IF NOT EXISTS "ob-poc".intent_feedback (
    id BIGSERIAL PRIMARY KEY,
    
    -- Session context
    session_id UUID NOT NULL,
    interaction_id UUID NOT NULL DEFAULT gen_random_uuid(),
    
    -- User input (sanitized - no PII/client names)
    user_input TEXT NOT NULL,
    user_input_hash TEXT NOT NULL,  -- For dedup without storing raw text long-term
    input_source TEXT NOT NULL DEFAULT 'chat',  -- 'chat', 'voice', 'command'
    
    -- Match result
    matched_verb TEXT,
    match_score REAL,
    match_confidence TEXT,  -- 'high', 'medium', 'low', 'none'
    semantic_score REAL,
    phonetic_score REAL,
    alternatives JSONB,  -- Top-5 alternatives shown: [{"verb": "...", "score": 0.72}, ...]
    
    -- Outcome (updated by separate insert or single insert if known immediately)
    outcome TEXT,  -- 'executed', 'selected_alt', 'corrected', 'rephrased', 'abandoned', NULL (pending)
    outcome_verb TEXT,  -- The verb that was actually executed (may differ from matched_verb)
    correction_input TEXT,  -- If user rephrased, what did they say?
    time_to_outcome_ms INTEGER,  -- How long between match and outcome
    
    -- Context at time of interaction
    graph_context TEXT,
    workflow_phase TEXT,
    
    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    -- No foreign key to dsl_verbs - we want to capture even if verb is later deleted
    -- This is append-only ML data, not referential integrity
    
    CONSTRAINT valid_source CHECK (input_source IN ('chat', 'voice', 'command')),
    CONSTRAINT valid_confidence CHECK (match_confidence IN ('high', 'medium', 'low', 'none') OR match_confidence IS NULL),
    CONSTRAINT valid_outcome CHECK (outcome IN ('executed', 'selected_alt', 'corrected', 'rephrased', 'abandoned') OR outcome IS NULL)
);

-- Indexes for batch analysis queries
CREATE INDEX idx_feedback_created ON "ob-poc".intent_feedback(created_at);
CREATE INDEX idx_feedback_outcome ON "ob-poc".intent_feedback(outcome) WHERE outcome IS NOT NULL;
CREATE INDEX idx_feedback_verb ON "ob-poc".intent_feedback(matched_verb) WHERE matched_verb IS NOT NULL;
CREATE INDEX idx_feedback_session ON "ob-poc".intent_feedback(session_id);
CREATE INDEX idx_feedback_input_hash ON "ob-poc".intent_feedback(user_input_hash);
CREATE INDEX idx_feedback_confidence ON "ob-poc".intent_feedback(match_confidence);

-- Partial index for pending outcomes (need to be resolved)
CREATE INDEX idx_feedback_pending ON "ob-poc".intent_feedback(interaction_id) 
WHERE outcome IS NULL;

COMMENT ON TABLE "ob-poc".intent_feedback IS 
'ML feedback capture for intent matching continuous learning. Append-only, batch analysis.';
```

### 1.2 Analysis Summary Table (Materialized Results)

```sql
-- Materialized analysis results (refreshed by batch job)
CREATE TABLE IF NOT EXISTS "ob-poc".intent_feedback_analysis (
    id SERIAL PRIMARY KEY,
    analysis_type TEXT NOT NULL,  -- 'pattern_discovery', 'confusion_pair', 'gap', 'low_score_success'
    analysis_date DATE NOT NULL DEFAULT CURRENT_DATE,
    
    -- Analysis payload
    data JSONB NOT NULL,
    
    -- Status
    reviewed BOOLEAN DEFAULT FALSE,
    applied BOOLEAN DEFAULT FALSE,
    reviewed_by TEXT,
    reviewed_at TIMESTAMPTZ,
    
    created_at TIMESTAMPTZ DEFAULT NOW(),
    
    UNIQUE(analysis_type, analysis_date, data)
);

CREATE INDEX idx_analysis_type_date ON "ob-poc".intent_feedback_analysis(analysis_type, analysis_date);
CREATE INDEX idx_analysis_pending ON "ob-poc".intent_feedback_analysis(reviewed) WHERE NOT reviewed;
```

---

## Task 2: Rust Data Structures

**File:** `rust/src/session/feedback/types.rs`

```rust
//! Intent feedback types for ML continuous learning

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::Uuid;

/// Source of user input
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum InputSource {
    Chat,
    Voice,
    Command,
}

/// Match confidence level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum MatchConfidence {
    High,
    Medium,
    Low,
    None,
}

/// Outcome of an intent match
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum Outcome {
    /// User executed the matched verb
    Executed,
    /// User selected an alternative from the suggestions
    SelectedAlt,
    /// User explicitly corrected ("no, I meant X")
    Corrected,
    /// User immediately rephrased their input
    Rephrased,
    /// User abandoned (changed topic, session ended)
    Abandoned,
}

/// Alternative verb suggestion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alternative {
    pub verb: String,
    pub score: f32,
}

/// Intent feedback record for capture
#[derive(Debug, Clone)]
pub struct IntentFeedback {
    pub session_id: Uuid,
    pub interaction_id: Uuid,
    pub user_input: String,
    pub user_input_hash: String,
    pub input_source: InputSource,
    pub matched_verb: Option<String>,
    pub match_score: Option<f32>,
    pub match_confidence: Option<MatchConfidence>,
    pub semantic_score: Option<f32>,
    pub phonetic_score: Option<f32>,
    pub alternatives: Vec<Alternative>,
    pub graph_context: Option<String>,
    pub workflow_phase: Option<String>,
}

/// Outcome update for an existing feedback record
#[derive(Debug, Clone)]
pub struct OutcomeUpdate {
    pub interaction_id: Uuid,
    pub outcome: Outcome,
    pub outcome_verb: Option<String>,
    pub correction_input: Option<String>,
    pub time_to_outcome_ms: Option<i32>,
}

/// Analysis result types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AnalysisResult {
    /// New pattern discovered from successful executions
    PatternDiscovery {
        user_input: String,
        verb: String,
        occurrence_count: i64,
        avg_score: f32,
    },
    /// Confusion between two verbs
    ConfusionPair {
        matched_verb: String,
        actual_verb: String,
        confusion_count: i64,
        example_inputs: Vec<String>,
    },
    /// Input with no good match (potential new verb or pattern)
    Gap {
        user_input: String,
        occurrence_count: i64,
        best_match: Option<String>,
        best_score: Option<f32>,
    },
    /// Low score match that was actually correct
    LowScoreSuccess {
        user_input: String,
        verb: String,
        avg_score: f32,
        occurrence_count: i64,
    },
}
```

---

## Task 3: Feedback Repository

**File:** `rust/src/session/feedback/repository.rs`

```rust
//! Repository for intent feedback capture and analysis

use super::types::*;
use anyhow::Result;
use sqlx::PgPool;
use std::sync::Arc;

pub struct FeedbackRepository {
    pool: Arc<PgPool>,
}

impl FeedbackRepository {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
    
    /// Capture an intent match (append-only insert)
    pub async fn capture(&self, feedback: &IntentFeedback) -> Result<()> {
        let alternatives_json = serde_json::to_value(&feedback.alternatives)?;
        
        sqlx::query(r#"
            INSERT INTO "ob-poc".intent_feedback (
                session_id, interaction_id, user_input, user_input_hash,
                input_source, matched_verb, match_score, match_confidence,
                semantic_score, phonetic_score, alternatives,
                graph_context, workflow_phase
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13
            )
        "#)
        .bind(&feedback.session_id)
        .bind(&feedback.interaction_id)
        .bind(&feedback.user_input)
        .bind(&feedback.user_input_hash)
        .bind(&feedback.input_source)
        .bind(&feedback.matched_verb)
        .bind(&feedback.match_score)
        .bind(&feedback.match_confidence)
        .bind(&feedback.semantic_score)
        .bind(&feedback.phonetic_score)
        .bind(&alternatives_json)
        .bind(&feedback.graph_context)
        .bind(&feedback.workflow_phase)
        .execute(self.pool.as_ref())
        .await?;
        
        Ok(())
    }
    
    /// Record outcome for an interaction
    /// Note: This updates the existing row - acceptable for outcome tracking
    pub async fn record_outcome(&self, update: &OutcomeUpdate) -> Result<()> {
        sqlx::query(r#"
            UPDATE "ob-poc".intent_feedback
            SET outcome = $2,
                outcome_verb = $3,
                correction_input = $4,
                time_to_outcome_ms = $5
            WHERE interaction_id = $1
              AND outcome IS NULL  -- Only update if not already set
        "#)
        .bind(&update.interaction_id)
        .bind(&update.outcome)
        .bind(&update.outcome_verb)
        .bind(&update.correction_input)
        .bind(&update.time_to_outcome_ms)
        .execute(self.pool.as_ref())
        .await?;
        
        Ok(())
    }
    
    /// Mark stale pending interactions as abandoned
    /// Run periodically (e.g., every hour)
    pub async fn expire_pending(&self, older_than_minutes: i32) -> Result<u64> {
        let result = sqlx::query(r#"
            UPDATE "ob-poc".intent_feedback
            SET outcome = 'abandoned'
            WHERE outcome IS NULL
              AND created_at < NOW() - make_interval(mins => $1)
        "#)
        .bind(older_than_minutes)
        .execute(self.pool.as_ref())
        .await?;
        
        Ok(result.rows_affected())
    }
}
```

---

## Task 4: Analysis Queries

**File:** `rust/src/session/feedback/analysis.rs`

```rust
//! Batch analysis of intent feedback for pattern learning

use super::types::*;
use anyhow::Result;
use sqlx::PgPool;
use std::sync::Arc;

pub struct FeedbackAnalyzer {
    pool: Arc<PgPool>,
}

impl FeedbackAnalyzer {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
    
    /// Discover new patterns from successful executions
    /// Finds inputs that led to execution but aren't in current patterns
    pub async fn discover_patterns(
        &self,
        min_occurrences: i64,
        days_back: i32,
    ) -> Result<Vec<AnalysisResult>> {
        let rows: Vec<(String, String, i64, f32)> = sqlx::query_as(r#"
            SELECT 
                f.user_input,
                f.outcome_verb,
                COUNT(*) as occurrence_count,
                AVG(f.match_score)::real as avg_score
            FROM "ob-poc".intent_feedback f
            WHERE f.outcome IN ('executed', 'selected_alt')
              AND f.outcome_verb IS NOT NULL
              AND f.created_at > NOW() - make_interval(days => $2)
              -- Exclude inputs already in patterns
              AND NOT EXISTS (
                  SELECT 1 FROM "ob-poc".verb_pattern_embeddings p
                  WHERE p.verb_full_name = f.outcome_verb
                    AND LOWER(p.pattern) = LOWER(f.user_input)
              )
            GROUP BY f.user_input, f.outcome_verb
            HAVING COUNT(*) >= $1
            ORDER BY COUNT(*) DESC
            LIMIT 100
        "#)
        .bind(min_occurrences)
        .bind(days_back)
        .fetch_all(self.pool.as_ref())
        .await?;
        
        Ok(rows.into_iter()
            .map(|(user_input, verb, occurrence_count, avg_score)| {
                AnalysisResult::PatternDiscovery {
                    user_input,
                    verb,
                    occurrence_count,
                    avg_score,
                }
            })
            .collect())
    }
    
    /// Find confusion pairs (verb A matched but user wanted verb B)
    pub async fn find_confusion_pairs(
        &self,
        min_confusions: i64,
        days_back: i32,
    ) -> Result<Vec<AnalysisResult>> {
        let rows: Vec<(String, String, i64, Vec<String>)> = sqlx::query_as(r#"
            SELECT 
                matched_verb,
                outcome_verb,
                COUNT(*) as confusion_count,
                ARRAY_AGG(DISTINCT user_input ORDER BY user_input) as example_inputs
            FROM "ob-poc".intent_feedback
            WHERE outcome IN ('corrected', 'selected_alt')
              AND matched_verb IS NOT NULL
              AND outcome_verb IS NOT NULL
              AND matched_verb != outcome_verb
              AND created_at > NOW() - make_interval(days => $2)
            GROUP BY matched_verb, outcome_verb
            HAVING COUNT(*) >= $1
            ORDER BY COUNT(*) DESC
            LIMIT 50
        "#)
        .bind(min_confusions)
        .bind(days_back)
        .fetch_all(self.pool.as_ref())
        .await?;
        
        Ok(rows.into_iter()
            .map(|(matched_verb, actual_verb, confusion_count, example_inputs)| {
                AnalysisResult::ConfusionPair {
                    matched_verb,
                    actual_verb,
                    confusion_count,
                    example_inputs: example_inputs.into_iter().take(5).collect(),
                }
            })
            .collect())
    }
    
    /// Find gaps - inputs where matching failed or user abandoned
    pub async fn find_gaps(
        &self,
        min_occurrences: i64,
        days_back: i32,
    ) -> Result<Vec<AnalysisResult>> {
        let rows: Vec<(String, i64, Option<String>, Option<f32>)> = sqlx::query_as(r#"
            SELECT 
                user_input,
                COUNT(*) as occurrence_count,
                matched_verb as best_match,
                MAX(match_score) as best_score
            FROM "ob-poc".intent_feedback
            WHERE (outcome = 'abandoned' OR match_confidence IN ('low', 'none'))
              AND created_at > NOW() - make_interval(days => $2)
            GROUP BY user_input, matched_verb
            HAVING COUNT(*) >= $1
            ORDER BY COUNT(*) DESC
            LIMIT 100
        "#)
        .bind(min_occurrences)
        .bind(days_back)
        .fetch_all(self.pool.as_ref())
        .await?;
        
        Ok(rows.into_iter()
            .map(|(user_input, occurrence_count, best_match, best_score)| {
                AnalysisResult::Gap {
                    user_input,
                    occurrence_count,
                    best_match,
                    best_score,
                }
            })
            .collect())
    }
    
    /// Find low-score matches that were actually correct
    /// These are candidates for new patterns to boost scores
    pub async fn find_low_score_successes(
        &self,
        max_score: f32,
        min_occurrences: i64,
        days_back: i32,
    ) -> Result<Vec<AnalysisResult>> {
        let rows: Vec<(String, String, f32, i64)> = sqlx::query_as(r#"
            SELECT 
                user_input,
                outcome_verb,
                AVG(match_score)::real as avg_score,
                COUNT(*) as occurrence_count
            FROM "ob-poc".intent_feedback
            WHERE outcome = 'executed'
              AND match_score < $1
              AND match_score IS NOT NULL
              AND outcome_verb IS NOT NULL
              AND created_at > NOW() - make_interval(days => $3)
            GROUP BY user_input, outcome_verb
            HAVING COUNT(*) >= $2
            ORDER BY COUNT(*) DESC
            LIMIT 100
        "#)
        .bind(max_score)
        .bind(min_occurrences)
        .bind(days_back)
        .fetch_all(self.pool.as_ref())
        .await?;
        
        Ok(rows.into_iter()
            .map(|(user_input, verb, avg_score, occurrence_count)| {
                AnalysisResult::LowScoreSuccess {
                    user_input,
                    verb,
                    avg_score,
                    occurrence_count,
                }
            })
            .collect())
    }
    
    /// Run full analysis and store results
    pub async fn run_full_analysis(&self, days_back: i32) -> Result<AnalysisReport> {
        let mut report = AnalysisReport::default();
        
        // Pattern discoveries (seen 3+ times)
        report.pattern_discoveries = self.discover_patterns(3, days_back).await?;
        
        // Confusion pairs (seen 2+ times)
        report.confusion_pairs = self.find_confusion_pairs(2, days_back).await?;
        
        // Gaps (seen 3+ times)
        report.gaps = self.find_gaps(3, days_back).await?;
        
        // Low score successes (score < 0.7, seen 3+ times)
        report.low_score_successes = self.find_low_score_successes(0.7, 3, days_back).await?;
        
        // Store results for review
        self.store_analysis_results(&report).await?;
        
        Ok(report)
    }
    
    /// Store analysis results for review
    async fn store_analysis_results(&self, report: &AnalysisReport) -> Result<()> {
        let today = chrono::Utc::now().date_naive();
        
        for result in &report.pattern_discoveries {
            let data = serde_json::to_value(result)?;
            sqlx::query(r#"
                INSERT INTO "ob-poc".intent_feedback_analysis 
                    (analysis_type, analysis_date, data)
                VALUES ('pattern_discovery', $1, $2)
                ON CONFLICT (analysis_type, analysis_date, data) DO NOTHING
            "#)
            .bind(today)
            .bind(data)
            .execute(self.pool.as_ref())
            .await?;
        }
        
        for result in &report.confusion_pairs {
            let data = serde_json::to_value(result)?;
            sqlx::query(r#"
                INSERT INTO "ob-poc".intent_feedback_analysis 
                    (analysis_type, analysis_date, data)
                VALUES ('confusion_pair', $1, $2)
                ON CONFLICT (analysis_type, analysis_date, data) DO NOTHING
            "#)
            .bind(today)
            .bind(data)
            .execute(self.pool.as_ref())
            .await?;
        }
        
        for result in &report.gaps {
            let data = serde_json::to_value(result)?;
            sqlx::query(r#"
                INSERT INTO "ob-poc".intent_feedback_analysis 
                    (analysis_type, analysis_date, data)
                VALUES ('gap', $1, $2)
                ON CONFLICT (analysis_type, analysis_date, data) DO NOTHING
            "#)
            .bind(today)
            .bind(data)
            .execute(self.pool.as_ref())
            .await?;
        }
        
        for result in &report.low_score_successes {
            let data = serde_json::to_value(result)?;
            sqlx::query(r#"
                INSERT INTO "ob-poc".intent_feedback_analysis 
                    (analysis_type, analysis_date, data)
                VALUES ('low_score_success', $1, $2)
                ON CONFLICT (analysis_type, analysis_date, data) DO NOTHING
            "#)
            .bind(today)
            .bind(data)
            .execute(self.pool.as_ref())
            .await?;
        }
        
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct AnalysisReport {
    pub pattern_discoveries: Vec<AnalysisResult>,
    pub confusion_pairs: Vec<AnalysisResult>,
    pub gaps: Vec<AnalysisResult>,
    pub low_score_successes: Vec<AnalysisResult>,
}

impl AnalysisReport {
    pub fn summary(&self) -> String {
        format!(
            "Analysis: {} patterns, {} confusions, {} gaps, {} low-score successes",
            self.pattern_discoveries.len(),
            self.confusion_pairs.len(),
            self.gaps.len(),
            self.low_score_successes.len()
        )
    }
}
```

---

## Task 5: Input Sanitization

**File:** `rust/src/session/feedback/sanitize.rs`

```rust
//! Sanitize user input before logging
//! Removes potential PII, client names, account numbers

use lazy_static::lazy_static;
use regex::Regex;
use sha2::{Sha256, Digest};

lazy_static! {
    // Common patterns to redact
    static ref ACCOUNT_NUMBER: Regex = Regex::new(r"\b\d{8,12}\b").unwrap();
    static ref EMAIL: Regex = Regex::new(r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Z|a-z]{2,}\b").unwrap();
    static ref PHONE: Regex = Regex::new(r"\b\+?[\d\s\-\(\)]{10,}\b").unwrap();
}

/// Sanitize user input for logging
/// Returns (sanitized_text, hash_of_original)
pub fn sanitize_input(input: &str, known_entities: &[&str]) -> (String, String) {
    let mut sanitized = input.to_string();
    
    // Replace known entity names with [ENTITY]
    for entity in known_entities {
        if entity.len() >= 3 {  // Only replace meaningful names
            let pattern = regex::escape(entity);
            if let Ok(re) = Regex::new(&format!(r"(?i)\b{}\b", pattern)) {
                sanitized = re.replace_all(&sanitized, "[ENTITY]").to_string();
            }
        }
    }
    
    // Replace potential account numbers
    sanitized = ACCOUNT_NUMBER.replace_all(&sanitized, "[ACCOUNT]").to_string();
    
    // Replace emails
    sanitized = EMAIL.replace_all(&sanitized, "[EMAIL]").to_string();
    
    // Replace phone numbers
    sanitized = PHONE.replace_all(&sanitized, "[PHONE]").to_string();
    
    // Hash for dedup (hash original, not sanitized)
    let hash = compute_hash(input);
    
    (sanitized, hash)
}

fn compute_hash(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let result = hasher.finalize();
    hex::encode(&result[..8])  // First 8 bytes = 16 hex chars
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_sanitize_entities() {
        let (sanitized, _) = sanitize_input(
            "show me the Acme Corp ownership",
            &["Acme Corp", "BigBank Ltd"],
        );
        assert_eq!(sanitized, "show me the [ENTITY] ownership");
    }
    
    #[test]
    fn test_sanitize_account_numbers() {
        let (sanitized, _) = sanitize_input(
            "look up account 12345678901",
            &[],
        );
        assert_eq!(sanitized, "look up account [ACCOUNT]");
    }
    
    #[test]
    fn test_hash_consistency() {
        let (_, hash1) = sanitize_input("test input", &[]);
        let (_, hash2) = sanitize_input("test input", &[]);
        assert_eq!(hash1, hash2);
    }
}
```

---

## Task 6: Feedback Service Integration

**File:** `rust/src/session/feedback/service.rs`

```rust
//! Feedback capture service - integrates with intent matching

use super::{
    analysis::FeedbackAnalyzer,
    repository::FeedbackRepository,
    sanitize::sanitize_input,
    types::*,
};
use crate::session::embeddings::{HybridMatcher, VerbMatch};
use anyhow::Result;
use sqlx::PgPool;
use sqlx::types::Uuid;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Feedback capture service
pub struct FeedbackService {
    repository: FeedbackRepository,
    analyzer: FeedbackAnalyzer,
    /// Cache of known entity names for sanitization
    known_entities: RwLock<Vec<String>>,
}

impl FeedbackService {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self {
            repository: FeedbackRepository::new(pool.clone()),
            analyzer: FeedbackAnalyzer::new(pool),
            known_entities: RwLock::new(Vec::new()),
        }
    }
    
    /// Update known entities cache (call periodically or on entity load)
    pub async fn update_known_entities(&self, entities: Vec<String>) {
        let mut cache = self.known_entities.write().await;
        *cache = entities;
    }
    
    /// Capture an intent match result
    pub async fn capture_match(
        &self,
        session_id: Uuid,
        user_input: &str,
        input_source: InputSource,
        match_result: Option<&VerbMatch>,
        alternatives: &[VerbMatch],
        graph_context: Option<&str>,
        workflow_phase: Option<&str>,
    ) -> Result<Uuid> {
        // Sanitize input
        let known = self.known_entities.read().await;
        let entity_refs: Vec<&str> = known.iter().map(|s| s.as_str()).collect();
        let (sanitized_input, input_hash) = sanitize_input(user_input, &entity_refs);
        drop(known);
        
        let interaction_id = Uuid::new_v4();
        
        let feedback = IntentFeedback {
            session_id,
            interaction_id,
            user_input: sanitized_input,
            user_input_hash: input_hash,
            input_source,
            matched_verb: match_result.map(|m| m.verb.clone()),
            match_score: match_result.map(|m| m.final_score),
            match_confidence: match_result.map(|m| match m.confidence {
                crate::session::embeddings::MatchConfidence::High => MatchConfidence::High,
                crate::session::embeddings::MatchConfidence::Medium => MatchConfidence::Medium,
                crate::session::embeddings::MatchConfidence::Low => MatchConfidence::Low,
            }),
            semantic_score: match_result.map(|m| m.semantic_score),
            phonetic_score: match_result.map(|m| m.phonetic_score),
            alternatives: alternatives.iter().take(5).map(|a| Alternative {
                verb: a.verb.clone(),
                score: a.final_score,
            }).collect(),
            graph_context: graph_context.map(String::from),
            workflow_phase: workflow_phase.map(String::from),
        };
        
        self.repository.capture(&feedback).await?;
        
        Ok(interaction_id)
    }
    
    /// Record the outcome of an interaction
    pub async fn record_outcome(
        &self,
        interaction_id: Uuid,
        outcome: Outcome,
        outcome_verb: Option<String>,
        correction_input: Option<String>,
        time_to_outcome_ms: Option<i32>,
    ) -> Result<()> {
        let update = OutcomeUpdate {
            interaction_id,
            outcome,
            outcome_verb,
            correction_input,
            time_to_outcome_ms,
        };
        
        self.repository.record_outcome(&update).await
    }
    
    /// Run analysis and get report
    pub async fn analyze(&self, days_back: i32) -> Result<AnalysisReport> {
        self.analyzer.run_full_analysis(days_back).await
    }
    
    /// Expire stale pending interactions
    pub async fn expire_pending(&self, older_than_minutes: i32) -> Result<u64> {
        self.repository.expire_pending(older_than_minutes).await
    }
}
```

---

## Task 7: Module Structure

**File:** `rust/src/session/feedback/mod.rs`

```rust
//! Intent feedback capture for ML continuous learning
//!
//! This module provides:
//! - Capture of user interactions with intent matching
//! - Outcome tracking (executed, corrected, abandoned)
//! - Batch analysis for pattern discovery
//! - Input sanitization for privacy

mod analysis;
mod repository;
mod sanitize;
mod service;
mod types;

pub use analysis::{AnalysisReport, FeedbackAnalyzer};
pub use repository::FeedbackRepository;
pub use sanitize::sanitize_input;
pub use service::FeedbackService;
pub use types::*;
```

**Update:** `rust/src/session/mod.rs`

```rust
pub mod feedback;  // ADD
pub mod embeddings;
// ... existing modules
```

---

## Task 8: Integration with HybridMatcher

**File:** Update `rust/src/session/embeddings/matcher.rs`

Add feedback capture to match flow:

```rust
use crate::session::feedback::{FeedbackService, InputSource};

impl HybridMatcher {
    /// Match with feedback capture
    pub async fn match_and_capture(
        &self,
        session_id: Uuid,
        query: &str,
        input_source: InputSource,
        limit: usize,
        feedback_service: &FeedbackService,
        graph_context: Option<&str>,
        workflow_phase: Option<&str>,
    ) -> Result<(Vec<VerbMatch>, Uuid)> {
        // Perform match
        let results = self.match_intent(query, limit).await?;
        
        // Capture feedback
        let interaction_id = feedback_service.capture_match(
            session_id,
            query,
            input_source,
            results.first(),
            &results,
            graph_context,
            workflow_phase,
        ).await?;
        
        Ok((results, interaction_id))
    }
}
```

---

## Task 9: Pattern Learner (Auto-Apply Discoveries)

**File:** `rust/src/session/feedback/learner.rs`

```rust
//! Automatic pattern learning from feedback analysis

use super::analysis::{AnalysisReport, AnalysisResult};
use crate::session::embeddings::HybridMatcher;
use anyhow::Result;
use sqlx::PgPool;
use std::sync::Arc;

pub struct PatternLearner {
    pool: Arc<PgPool>,
}

impl PatternLearner {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
    
    /// Apply high-confidence pattern discoveries
    /// Only auto-applies patterns seen 5+ times with clear verb association
    pub async fn auto_apply_discoveries(
        &self,
        discoveries: &[AnalysisResult],
        min_occurrences: i64,
    ) -> Result<Vec<(String, String)>> {
        let mut applied = Vec::new();
        
        for discovery in discoveries {
            if let AnalysisResult::PatternDiscovery {
                user_input,
                verb,
                occurrence_count,
                avg_score,
            } = discovery {
                // Only auto-apply high-confidence discoveries
                if *occurrence_count >= min_occurrences && *avg_score > 0.5 {
                    // Add pattern to database
                    self.add_pattern(verb, user_input).await?;
                    applied.push((verb.clone(), user_input.clone()));
                    
                    tracing::info!(
                        "Auto-applied pattern: '{}' → {} (seen {} times, avg score {:.2})",
                        user_input, verb, occurrence_count, avg_score
                    );
                }
            }
        }
        
        Ok(applied)
    }
    
    /// Add a new pattern (inserts into verb_pattern_embeddings via trigger/job)
    async fn add_pattern(&self, verb: &str, pattern: &str) -> Result<()> {
        // Insert into intent_patterns array in dsl_verbs
        sqlx::query(r#"
            UPDATE "ob-poc".dsl_verbs
            SET intent_patterns = array_append(
                COALESCE(intent_patterns, ARRAY[]::text[]),
                $2
            )
            WHERE full_name = $1
              AND NOT ($2 = ANY(COALESCE(intent_patterns, ARRAY[]::text[])))
        "#)
        .bind(verb)
        .bind(pattern)
        .execute(self.pool.as_ref())
        .await?;
        
        // Mark as applied in analysis table
        sqlx::query(r#"
            UPDATE "ob-poc".intent_feedback_analysis
            SET applied = true
            WHERE analysis_type = 'pattern_discovery'
              AND data->>'user_input' = $1
              AND data->>'verb' = $2
        "#)
        .bind(pattern)
        .bind(verb)
        .execute(self.pool.as_ref())
        .await?;
        
        Ok(())
    }
}
```

---

## Task 10: Batch Job Entry Point

**File:** `rust/src/bin/feedback_analysis.rs`

```rust
//! Batch job for feedback analysis
//! Run daily/weekly: cargo run --bin feedback_analysis

use anyhow::Result;
use ob_poc::session::feedback::{FeedbackAnalyzer, PatternLearner, FeedbackService};
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    // Connect to database
    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");
    
    let pool = Arc::new(
        PgPoolOptions::new()
            .max_connections(5)
            .connect(&database_url)
            .await?
    );
    
    tracing::info!("Starting feedback analysis...");
    
    // Expire stale pending interactions (older than 30 minutes)
    let feedback_service = FeedbackService::new(pool.clone());
    let expired = feedback_service.expire_pending(30).await?;
    tracing::info!("Expired {} stale pending interactions", expired);
    
    // Run analysis (last 7 days)
    let analyzer = FeedbackAnalyzer::new(pool.clone());
    let report = analyzer.run_full_analysis(7).await?;
    
    tracing::info!("{}", report.summary());
    
    // Auto-apply high-confidence discoveries (5+ occurrences)
    let learner = PatternLearner::new(pool.clone());
    let applied = learner.auto_apply_discoveries(&report.pattern_discoveries, 5).await?;
    
    if !applied.is_empty() {
        tracing::info!("Auto-applied {} new patterns", applied.len());
        
        // Trigger embedding rebuild
        // TODO: Call HybridMatcher::rebuild_embeddings()
        tracing::info!("Embeddings need rebuild - run embedding update job");
    }
    
    // Log findings for human review
    if !report.confusion_pairs.is_empty() {
        tracing::warn!(
            "Found {} confusion pairs - review needed",
            report.confusion_pairs.len()
        );
    }
    
    if !report.gaps.is_empty() {
        tracing::warn!(
            "Found {} input gaps - potential new verbs or patterns needed",
            report.gaps.len()
        );
    }
    
    tracing::info!("Feedback analysis complete");
    Ok(())
}
```

---

## Task 11: Data Retention

**File:** `rust/src/bin/feedback_cleanup.rs`

```rust
//! Data retention cleanup job
//! Run monthly: cargo run --bin feedback_cleanup

use anyhow::Result;
use sqlx::postgres::PgPoolOptions;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    
    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");
    
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&database_url)
        .await?;
    
    // Delete raw feedback older than 90 days
    // Keep aggregated analysis results longer
    let deleted = sqlx::query(r#"
        DELETE FROM "ob-poc".intent_feedback
        WHERE created_at < NOW() - INTERVAL '90 days'
    "#)
    .execute(&pool)
    .await?;
    
    tracing::info!("Deleted {} old feedback records", deleted.rows_affected());
    
    // Delete old analysis results (keep 1 year)
    let deleted = sqlx::query(r#"
        DELETE FROM "ob-poc".intent_feedback_analysis
        WHERE analysis_date < NOW() - INTERVAL '1 year'
    "#)
    .execute(&pool)
    .await?;
    
    tracing::info!("Deleted {} old analysis records", deleted.rows_affected());
    
    Ok(())
}
```

---

## Summary

| Component | Purpose |
|-----------|---------|
| `intent_feedback` table | Append-only capture of match attempts + outcomes |
| `intent_feedback_analysis` table | Stored analysis results for review |
| `FeedbackRepository` | Insert feedback, update outcomes |
| `FeedbackAnalyzer` | Batch queries for pattern discovery, confusion analysis |
| `FeedbackService` | High-level capture API with sanitization |
| `PatternLearner` | Auto-apply high-confidence discoveries |
| `feedback_analysis` bin | Daily/weekly batch job |
| `feedback_cleanup` bin | Monthly retention cleanup |

---

## Implementation Order

| Step | Task | Effort |
|------|------|--------|
| 1 | Run migration (tables + indexes) | 5 min |
| 2 | Implement `types.rs` | 15 min |
| 3 | Implement `sanitize.rs` | 20 min |
| 4 | Implement `repository.rs` | 30 min |
| 5 | Implement `analysis.rs` | 45 min |
| 6 | Implement `service.rs` | 30 min |
| 7 | Implement `learner.rs` | 30 min |
| 8 | Integrate with HybridMatcher | 20 min |
| 9 | Create batch job binaries | 20 min |
| 10 | Wire up outcome tracking in UI/API | 30 min |

**Total: ~4-5 hours**

---

## Learning Loop Cadence

| Frequency | Job | Action |
|-----------|-----|--------|
| Hourly | `expire_pending` | Mark stale interactions as abandoned |
| Daily | `feedback_analysis` | Discover patterns, find confusions |
| Daily | Auto-apply | Add 5+ occurrence patterns automatically |
| Weekly | Human review | Review confusion pairs, gaps |
| Monthly | Rebuild embeddings | If patterns changed significantly |
| Monthly | Data cleanup | Purge >90 day raw data |
