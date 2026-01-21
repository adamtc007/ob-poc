# TODO: Implement Robust Feedback Loop for Verb Learning

**Created:** 2026-01-21
**Status:** READY FOR IMPLEMENTATION
**Priority:** HIGH
**Complexity:** Medium-High (spans DB, Rust, background jobs)

---

## Executive Summary

The semantic verb matching pipeline has the architecture for continuous learning, but lacks the **promotion discipline** to safely auto-apply learned patterns. Without proper guardrails, the learning loop risks:

1. **Garbage-in learning** - promoting noisy patterns from unclear outcomes
2. **Popularity bias** - generic phrases ("create", "add") dominating and dragging matches toward wrong verbs
3. **Semantic drift** - patterns that worked once but conflict with others

This TODO implements a **staged promotion pipeline** with quality gates, success tracking, and collision detection.

---

## Current State

**What exists:**
- `intent_feedback` table captures phrase → verb matches + outcomes
- `dsl_generation_log` tracks DSL execution success/failure
- `v_learning_feedback` view joins them with `learning_signal`
- `learning_candidates` table for staging
- `add_learned_pattern()` function to promote to `dsl_verbs.intent_patterns`
- `FeedbackAnalyzer` discovers patterns from batch analysis
- `PatternLearner` auto-applies discoveries

**What's missing:**
- Success rate tracking per candidate (success_count / total_count)
- Quality gates (length, stopwords, collision check)
- Cool-down period before promotion
- Automatic outcome expiry (pending → abandoned)
- Domain/category tagging to prevent popularity bias
- Weekly metrics dashboard
- Background job scheduling

---

## Architecture: Promotion Pipeline

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        FEEDBACK → LEARNING PIPELINE                          │
│                                                                              │
│  User Interaction                                                            │
│      │                                                                       │
│      ▼                                                                       │
│  intent_feedback (capture phrase + match)                                   │
│      │                                                                       │
│      ▼                                                                       │
│  dsl_generation_log (capture execution outcome)                             │
│      │                                                                       │
│      ▼                                                                       │
│  v_learning_feedback (join with learning_signal)                            │
│      │                                                                       │
│      ▼                                                                       │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │  FILTER 1: Strong Signal Only                                        │    │
│  │                                                                       │    │
│  │  ACCEPT:                                                              │    │
│  │    • learning_signal = 'success' (executed + DSL succeeded)          │    │
│  │    • outcome = 'selected_alt' (user picked from candidates)          │    │
│  │    • outcome = 'corrected' + outcome_verb IS NOT NULL                │    │
│  │                                                                       │    │
│  │  REJECT:                                                              │    │
│  │    • outcome = 'abandoned' (weak/no signal)                          │    │
│  │    • outcome IS NULL after 30 min (expire to abandoned)              │    │
│  │    • learning_signal = 'false_positive' (DSL execution failed)       │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│      │                                                                       │
│      ▼                                                                       │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │  FILTER 2: Quality Gates                                             │    │
│  │                                                                       │    │
│  │  • Word count: 3 ≤ words ≤ 15                                        │    │
│  │  • Not stopwords-only ("the", "a", "please", "can you")             │    │
│  │  • Not already in verb_pattern_embeddings for this verb             │    │
│  │  • Collision check: doesn't match another verb at >0.92 similarity  │    │
│  │  • Not in phrase_blocklist                                          │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│      │                                                                       │
│      ▼                                                                       │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │  ACCUMULATE: learning_candidates                                     │    │
│  │                                                                       │    │
│  │  • Upsert by fingerprint = hash(normalize(phrase) + verb)           │    │
│  │  • Track: occurrence_count, success_count, total_count              │    │
│  │  • Track: first_seen, last_seen, last_success_at                    │    │
│  │  • Store: domain_hint (from verb category)                          │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│      │                                                                       │
│      ▼                                                                       │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │  PROMOTE: Auto-apply when ALL conditions met                         │    │
│  │                                                                       │    │
│  │  • occurrence_count ≥ 5                                              │    │
│  │  • success_rate ≥ 0.80                                               │    │
│  │  • age ≥ 24 hours (cool-down)                                        │    │
│  │  • collision_safe = true                                             │    │
│  │                                                                       │    │
│  │  → add_learned_pattern(verb, phrase)                                 │    │
│  │  → Mark candidate status = 'applied'                                 │    │
│  │  → Log to learning_audit                                             │    │
│  │                                                                       │    │
│  │  ELSE IF occurrence_count ≥ 3 AND age ≥ 7 days:                      │    │
│  │  → Queue for manual review (status = 'needs_review')                 │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│      │                                                                       │
│      ▼                                                                       │
│  populate_embeddings (picks up new patterns on next run)                    │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Implementation Plan

### Phase 1: Schema Changes (Migration 043)

**File:** `migrations/043_feedback_loop_promotion.sql`

```sql
-- Migration 043: Feedback loop promotion infrastructure
-- Adds success tracking, quality gates, and collision detection

-- ============================================================================
-- 1. Add success tracking columns to learning_candidates
-- ============================================================================

ALTER TABLE agent.learning_candidates
ADD COLUMN IF NOT EXISTS success_count INT DEFAULT 0,
ADD COLUMN IF NOT EXISTS total_count INT DEFAULT 0,
ADD COLUMN IF NOT EXISTS last_success_at TIMESTAMPTZ,
ADD COLUMN IF NOT EXISTS domain_hint TEXT,
ADD COLUMN IF NOT EXISTS collision_safe BOOLEAN,
ADD COLUMN IF NOT EXISTS collision_check_at TIMESTAMPTZ,
ADD COLUMN IF NOT EXISTS collision_verb TEXT;  -- If collision detected, which verb?

-- Computed success rate (for queries)
COMMENT ON COLUMN agent.learning_candidates.success_count IS 
    'Count of successful outcomes (executed + DSL succeeded, or user selected this verb)';
COMMENT ON COLUMN agent.learning_candidates.total_count IS 
    'Total signals received (success + failure)';

-- ============================================================================
-- 2. Stopwords table for quality filtering
-- ============================================================================

CREATE TABLE IF NOT EXISTS agent.stopwords (
    word TEXT PRIMARY KEY,
    category TEXT DEFAULT 'generic'  -- 'generic', 'polite', 'filler'
);

-- Seed common stopwords
INSERT INTO agent.stopwords (word, category) VALUES
    ('the', 'generic'), ('a', 'generic'), ('an', 'generic'),
    ('please', 'polite'), ('can', 'polite'), ('could', 'polite'),
    ('you', 'polite'), ('would', 'polite'), ('help', 'polite'),
    ('me', 'generic'), ('i', 'generic'), ('my', 'generic'),
    ('want', 'filler'), ('need', 'filler'), ('like', 'filler'),
    ('to', 'generic'), ('for', 'generic'), ('with', 'generic'),
    ('this', 'generic'), ('that', 'generic'), ('it', 'generic'),
    ('do', 'filler'), ('make', 'filler'), ('get', 'filler'),
    ('just', 'filler'), ('now', 'filler'), ('here', 'filler')
ON CONFLICT (word) DO NOTHING;

-- ============================================================================
-- 3. Function: Record learning signal with success tracking
-- ============================================================================

CREATE OR REPLACE FUNCTION agent.record_learning_signal(
    p_phrase TEXT,
    p_verb TEXT,
    p_is_success BOOLEAN,
    p_signal_type TEXT,  -- 'executed', 'selected_alt', 'corrected'
    p_domain_hint TEXT DEFAULT NULL
) RETURNS BIGINT AS $$
DECLARE
    v_normalized TEXT;
    v_fingerprint TEXT;
    v_word_count INT;
    v_stopword_ratio REAL;
    v_id BIGINT;
BEGIN
    -- Normalize phrase
    v_normalized := lower(trim(regexp_replace(p_phrase, '\s+', ' ', 'g')));
    v_fingerprint := md5(v_normalized || '|' || p_verb);
    
    -- Quality gate: word count
    v_word_count := array_length(string_to_array(v_normalized, ' '), 1);
    IF v_word_count < 3 OR v_word_count > 15 THEN
        RETURN NULL;  -- Reject
    END IF;
    
    -- Quality gate: stopword ratio (reject if >70% stopwords)
    SELECT 
        COUNT(*) FILTER (WHERE s.word IS NOT NULL)::real / NULLIF(v_word_count, 0)
    INTO v_stopword_ratio
    FROM unnest(string_to_array(v_normalized, ' ')) AS w(word)
    LEFT JOIN agent.stopwords s ON s.word = w.word;
    
    IF v_stopword_ratio > 0.70 THEN
        RETURN NULL;  -- Reject (too generic)
    END IF;
    
    -- Upsert candidate
    INSERT INTO agent.learning_candidates (
        fingerprint, 
        learning_type, 
        input_pattern, 
        suggested_output,
        occurrence_count, 
        success_count, 
        total_count, 
        first_seen, 
        last_seen,
        last_success_at,
        domain_hint,
        status
    ) VALUES (
        v_fingerprint, 
        'invocation_phrase', 
        v_normalized, 
        p_verb,
        1,
        CASE WHEN p_is_success THEN 1 ELSE 0 END,
        1,
        NOW(), 
        NOW(),
        CASE WHEN p_is_success THEN NOW() ELSE NULL END,
        COALESCE(p_domain_hint, (
            SELECT category FROM "ob-poc".dsl_verbs WHERE full_name = p_verb
        )),
        'pending'
    )
    ON CONFLICT (fingerprint) DO UPDATE SET
        occurrence_count = agent.learning_candidates.occurrence_count + 1,
        success_count = agent.learning_candidates.success_count + 
            CASE WHEN p_is_success THEN 1 ELSE 0 END,
        total_count = agent.learning_candidates.total_count + 1,
        last_seen = NOW(),
        last_success_at = CASE 
            WHEN p_is_success THEN NOW() 
            ELSE agent.learning_candidates.last_success_at 
        END,
        -- Reset collision check if we get new signals
        collision_safe = NULL,
        collision_check_at = NULL
    RETURNING id INTO v_id;
    
    RETURN v_id;
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- 4. Function: Check collision with existing patterns
-- ============================================================================

CREATE OR REPLACE FUNCTION agent.check_pattern_collision(
    p_candidate_id BIGINT,
    p_similarity_threshold REAL DEFAULT 0.92
) RETURNS BOOLEAN AS $$
DECLARE
    v_phrase TEXT;
    v_verb TEXT;
    v_embedding vector(384);
    v_collision_verb TEXT;
    v_is_safe BOOLEAN := TRUE;
BEGIN
    -- Get candidate details
    SELECT input_pattern, suggested_output
    INTO v_phrase, v_verb
    FROM agent.learning_candidates
    WHERE id = p_candidate_id;
    
    IF v_phrase IS NULL THEN
        RETURN FALSE;
    END IF;
    
    -- Check if phrase already exists for this verb (exact match)
    IF EXISTS (
        SELECT 1 FROM "ob-poc".verb_pattern_embeddings
        WHERE verb_name = v_verb
          AND pattern_normalized = v_phrase
    ) THEN
        -- Already a pattern, no need to add
        UPDATE agent.learning_candidates
        SET status = 'duplicate',
            collision_safe = FALSE,
            collision_check_at = NOW()
        WHERE id = p_candidate_id;
        RETURN FALSE;
    END IF;
    
    -- For semantic collision check, we need the embedding
    -- This requires the phrase to be embedded first
    -- For now, mark as needing embedding check
    UPDATE agent.learning_candidates
    SET collision_safe = TRUE,  -- Optimistic; Rust code does semantic check
        collision_check_at = NOW()
    WHERE id = p_candidate_id;
    
    RETURN TRUE;
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- 5. Function: Get promotable candidates
-- ============================================================================

CREATE OR REPLACE FUNCTION agent.get_promotable_candidates(
    p_min_occurrences INT DEFAULT 5,
    p_min_success_rate REAL DEFAULT 0.80,
    p_min_age_hours INT DEFAULT 24,
    p_limit INT DEFAULT 50
)
RETURNS TABLE (
    id BIGINT,
    phrase TEXT,
    verb TEXT,
    occurrence_count INT,
    success_count INT,
    total_count INT,
    success_rate REAL,
    domain_hint TEXT,
    first_seen TIMESTAMPTZ,
    age_hours REAL
) AS $$
BEGIN
    RETURN QUERY
    SELECT 
        lc.id,
        lc.input_pattern as phrase,
        lc.suggested_output as verb,
        lc.occurrence_count,
        lc.success_count,
        lc.total_count,
        (lc.success_count::real / NULLIF(lc.total_count, 0)) as success_rate,
        lc.domain_hint,
        lc.first_seen,
        EXTRACT(EPOCH FROM (NOW() - lc.first_seen)) / 3600 as age_hours
    FROM agent.learning_candidates lc
    WHERE lc.status = 'pending'
      AND lc.learning_type = 'invocation_phrase'
      -- Occurrence threshold
      AND lc.occurrence_count >= p_min_occurrences
      -- Success rate threshold
      AND (lc.success_count::real / NULLIF(lc.total_count, 0)) >= p_min_success_rate
      -- Age threshold (cool-down)
      AND lc.first_seen < NOW() - make_interval(hours => p_min_age_hours)
      -- Collision check passed (or not yet checked)
      AND (lc.collision_safe IS NULL OR lc.collision_safe = TRUE)
      -- Not blocklisted
      AND NOT EXISTS (
          SELECT 1 FROM agent.phrase_blocklist bl
          WHERE bl.blocked_verb = lc.suggested_output
            AND lower(bl.phrase) = lc.input_pattern
            AND (bl.expires_at IS NULL OR bl.expires_at > NOW())
      )
    ORDER BY 
        lc.occurrence_count DESC,
        (lc.success_count::real / NULLIF(lc.total_count, 0)) DESC
    LIMIT p_limit;
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- 6. Function: Get candidates needing manual review
-- ============================================================================

CREATE OR REPLACE FUNCTION agent.get_review_candidates(
    p_min_occurrences INT DEFAULT 3,
    p_min_age_days INT DEFAULT 7,
    p_limit INT DEFAULT 100
)
RETURNS TABLE (
    id BIGINT,
    phrase TEXT,
    verb TEXT,
    occurrence_count INT,
    success_rate REAL,
    domain_hint TEXT,
    first_seen TIMESTAMPTZ,
    last_seen TIMESTAMPTZ,
    collision_verb TEXT
) AS $$
BEGIN
    RETURN QUERY
    SELECT 
        lc.id,
        lc.input_pattern as phrase,
        lc.suggested_output as verb,
        lc.occurrence_count,
        (lc.success_count::real / NULLIF(lc.total_count, 0)) as success_rate,
        lc.domain_hint,
        lc.first_seen,
        lc.last_seen,
        lc.collision_verb
    FROM agent.learning_candidates lc
    WHERE lc.status = 'pending'
      AND lc.learning_type = 'invocation_phrase'
      AND lc.occurrence_count >= p_min_occurrences
      AND lc.first_seen < NOW() - make_interval(days => p_min_age_days)
      -- Either failed auto-promotion or collision detected
      AND (
          (lc.success_count::real / NULLIF(lc.total_count, 0)) < 0.80
          OR lc.collision_safe = FALSE
          OR lc.occurrence_count < 5
      )
    ORDER BY lc.occurrence_count DESC
    LIMIT p_limit;
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- 7. Function: Apply promotion (with audit)
-- ============================================================================

CREATE OR REPLACE FUNCTION agent.apply_promotion(
    p_candidate_id BIGINT,
    p_actor TEXT DEFAULT 'system_auto'
) RETURNS BOOLEAN AS $$
DECLARE
    v_phrase TEXT;
    v_verb TEXT;
    v_added BOOLEAN;
BEGIN
    -- Get candidate
    SELECT input_pattern, suggested_output
    INTO v_phrase, v_verb
    FROM agent.learning_candidates
    WHERE id = p_candidate_id
      AND status = 'pending';
    
    IF v_phrase IS NULL THEN
        RETURN FALSE;
    END IF;
    
    -- Add to dsl_verbs.intent_patterns
    SELECT "ob-poc".add_learned_pattern(v_verb, v_phrase) INTO v_added;
    
    IF v_added THEN
        -- Update candidate status
        UPDATE agent.learning_candidates
        SET status = 'applied',
            applied_at = NOW()
        WHERE id = p_candidate_id;
        
        -- Audit log
        INSERT INTO agent.learning_audit (
            action, learning_type, candidate_id, actor, details
        ) VALUES (
            'applied',
            'invocation_phrase',
            p_candidate_id,
            p_actor,
            jsonb_build_object(
                'phrase', v_phrase,
                'verb', v_verb
            )
        );
        
        RETURN TRUE;
    END IF;
    
    RETURN FALSE;
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- 8. Function: Expire pending outcomes
-- ============================================================================

CREATE OR REPLACE FUNCTION agent.expire_pending_outcomes(
    p_older_than_minutes INT DEFAULT 30
) RETURNS INT AS $$
DECLARE
    v_count INT;
BEGIN
    UPDATE "ob-poc".intent_feedback
    SET outcome = 'abandoned'
    WHERE outcome IS NULL
      AND created_at < NOW() - make_interval(mins => p_older_than_minutes);
    
    GET DIAGNOSTICS v_count = ROW_COUNT;
    RETURN v_count;
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- 9. Metrics views
-- ============================================================================

-- Weekly learning health dashboard
CREATE OR REPLACE VIEW agent.v_learning_health_weekly AS
SELECT
    DATE_TRUNC('week', f.created_at) as week,
    COUNT(*) as total_interactions,
    COUNT(*) FILTER (WHERE v.learning_signal = 'success') as successes,
    COUNT(*) FILTER (WHERE v.learning_signal IN ('wrong_match', 'correction_needed')) as corrections,
    COUNT(*) FILTER (WHERE v.learning_signal = 'no_match') as no_matches,
    COUNT(*) FILTER (WHERE v.learning_signal = 'false_positive') as false_positives,
    
    -- Hit rates
    ROUND(100.0 * COUNT(*) FILTER (WHERE v.learning_signal = 'success') / 
          NULLIF(COUNT(*), 0), 1) as top1_hit_rate_pct,
    
    -- Scores
    ROUND(AVG(f.match_score) FILTER (WHERE v.learning_signal = 'success')::numeric, 3) as avg_success_score,
    ROUND(AVG(f.match_score) FILTER (WHERE v.learning_signal IN ('wrong_match', 'correction_needed'))::numeric, 3) as avg_correction_score,
    
    -- Confidence distribution
    COUNT(*) FILTER (WHERE f.match_confidence = 'high') as high_confidence,
    COUNT(*) FILTER (WHERE f.match_confidence = 'medium') as medium_confidence,
    COUNT(*) FILTER (WHERE f.match_confidence = 'low') as low_confidence,
    COUNT(*) FILTER (WHERE f.match_confidence = 'none') as no_match_confidence

FROM "ob-poc".intent_feedback f
LEFT JOIN "ob-poc".v_learning_feedback v ON v.feedback_id = f.id
WHERE f.created_at > NOW() - INTERVAL '12 weeks'
GROUP BY 1
ORDER BY 1 DESC;

-- Candidate pipeline status
CREATE OR REPLACE VIEW agent.v_candidate_pipeline AS
SELECT
    status,
    COUNT(*) as count,
    AVG(occurrence_count) as avg_occurrences,
    AVG(success_count::real / NULLIF(total_count, 0)) as avg_success_rate,
    MIN(first_seen) as oldest,
    MAX(last_seen) as newest
FROM agent.learning_candidates
WHERE learning_type = 'invocation_phrase'
GROUP BY status;

-- Top pending candidates (for review)
CREATE OR REPLACE VIEW agent.v_top_pending_candidates AS
SELECT
    id,
    input_pattern as phrase,
    suggested_output as verb,
    occurrence_count,
    success_count,
    total_count,
    ROUND((success_count::real / NULLIF(total_count, 0))::numeric, 2) as success_rate,
    domain_hint,
    collision_safe,
    first_seen,
    last_seen,
    EXTRACT(DAY FROM (NOW() - first_seen)) as age_days
FROM agent.learning_candidates
WHERE status = 'pending'
  AND learning_type = 'invocation_phrase'
ORDER BY occurrence_count DESC, success_rate DESC
LIMIT 100;

-- ============================================================================
-- 10. Indexes for performance
-- ============================================================================

CREATE INDEX IF NOT EXISTS idx_learning_candidates_promotable
ON agent.learning_candidates(occurrence_count DESC, status)
WHERE status = 'pending' AND learning_type = 'invocation_phrase';

CREATE INDEX IF NOT EXISTS idx_learning_candidates_fingerprint
ON agent.learning_candidates(fingerprint);

CREATE INDEX IF NOT EXISTS idx_intent_feedback_learning_signal
ON "ob-poc".intent_feedback(created_at, outcome, matched_verb)
WHERE outcome IS NOT NULL;
```

---

### Phase 2: Rust Implementation

#### 2.1 Update FeedbackService to call `record_learning_signal`

**File:** `rust/crates/ob-semantic-matcher/src/feedback/service.rs`

```rust
impl FeedbackService {
    /// Record a learning signal from a resolved interaction
    /// Called when we have a strong signal (executed, selected_alt, corrected)
    pub async fn record_learning_signal(
        &self,
        phrase: &str,
        verb: &str,
        is_success: bool,
        signal_type: &str,  // "executed", "selected_alt", "corrected"
        domain_hint: Option<&str>,
    ) -> Result<Option<i64>> {
        let result: Option<(i64,)> = sqlx::query_as(
            r#"SELECT agent.record_learning_signal($1, $2, $3, $4, $5)"#
        )
        .bind(phrase)
        .bind(verb)
        .bind(is_success)
        .bind(signal_type)
        .bind(domain_hint)
        .fetch_optional(&self.pool)
        .await?;
        
        Ok(result.map(|r| r.0))
    }
}
```

#### 2.2 Update outcome recording to trigger learning signal

**File:** `rust/crates/ob-semantic-matcher/src/feedback/repository.rs`

Add after `record_outcome()`:

```rust
impl FeedbackRepository {
    /// Record outcome AND trigger learning signal if appropriate
    pub async fn record_outcome_with_learning(
        &self,
        update: &OutcomeUpdate,
        original_feedback: &IntentFeedback,
    ) -> Result<bool> {
        // Record the outcome
        let updated = self.record_outcome(update).await?;
        
        if !updated {
            return Ok(false);
        }
        
        // Determine if this is a strong signal worth learning from
        let (phrase, verb, is_success, signal_type) = match update.outcome {
            Outcome::Executed => {
                // Success if DSL also executed (not just verb matched)
                // We need to check execution_status from dsl_generation_log
                let verb = update.outcome_verb.as_ref()
                    .or(original_feedback.matched_verb.as_ref());
                if let Some(v) = verb {
                    (original_feedback.user_input.clone(), v.clone(), true, "executed")
                } else {
                    return Ok(true);  // No verb to learn
                }
            }
            Outcome::SelectedAlt => {
                // User selected a different verb - learn the correction
                if let Some(v) = &update.outcome_verb {
                    (original_feedback.user_input.clone(), v.clone(), true, "selected_alt")
                } else {
                    return Ok(true);
                }
            }
            Outcome::Corrected => {
                // User explicitly corrected - strong signal
                if let Some(v) = &update.outcome_verb {
                    (original_feedback.user_input.clone(), v.clone(), true, "corrected")
                } else {
                    return Ok(true);
                }
            }
            Outcome::Rephrased | Outcome::Abandoned => {
                // Weak signals - don't learn
                return Ok(true);
            }
        };
        
        // Record learning signal
        let _ = sqlx::query(
            r#"SELECT agent.record_learning_signal($1, $2, $3, $4, NULL)"#
        )
        .bind(&phrase)
        .bind(&verb)
        .bind(is_success)
        .bind(signal_type)
        .execute(&self.pool)
        .await;
        
        Ok(true)
    }
}
```

#### 2.3 Create PromotionService for background job

**File:** `rust/crates/ob-semantic-matcher/src/feedback/promotion.rs` (NEW)

```rust
//! Automatic pattern promotion from learning candidates
//!
//! Runs as a background job to:
//! 1. Expire pending outcomes
//! 2. Run collision checks on candidates
//! 3. Auto-promote qualified candidates
//! 4. Queue borderline candidates for review

use anyhow::Result;
use sqlx::PgPool;
use tracing::{info, warn};

use crate::Embedder;

pub struct PromotionService {
    pool: PgPool,
    embedder: Option<Embedder>,
    
    // Thresholds (configurable)
    min_occurrences: i32,
    min_success_rate: f32,
    min_age_hours: i32,
    collision_threshold: f32,
}

#[derive(Debug, sqlx::FromRow)]
struct PromotableCandidate {
    id: i64,
    phrase: String,
    verb: String,
    occurrence_count: i32,
    success_count: i32,
    total_count: i32,
    success_rate: f32,
    domain_hint: Option<String>,
}

impl PromotionService {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            embedder: None,
            min_occurrences: 5,
            min_success_rate: 0.80,
            min_age_hours: 24,
            collision_threshold: 0.92,
        }
    }
    
    pub fn with_embedder(mut self, embedder: Embedder) -> Self {
        self.embedder = Some(embedder);
        self
    }
    
    /// Run full promotion cycle
    pub async fn run_promotion_cycle(&self) -> Result<PromotionReport> {
        let mut report = PromotionReport::default();
        
        // 1. Expire stale pending outcomes
        report.expired_outcomes = self.expire_pending_outcomes(30).await?;
        info!("Expired {} pending outcomes", report.expired_outcomes);
        
        // 2. Get promotable candidates
        let candidates = self.get_promotable_candidates().await?;
        info!("Found {} promotable candidates", candidates.len());
        
        // 3. Run collision checks and promote
        for candidate in candidates {
            match self.try_promote(&candidate).await {
                Ok(true) => {
                    report.promoted.push(candidate.phrase.clone());
                    info!("Promoted: '{}' → {}", candidate.phrase, candidate.verb);
                }
                Ok(false) => {
                    report.skipped += 1;
                }
                Err(e) => {
                    warn!("Failed to promote '{}': {}", candidate.phrase, e);
                    report.errors += 1;
                }
            }
        }
        
        Ok(report)
    }
    
    async fn expire_pending_outcomes(&self, older_than_minutes: i32) -> Result<i64> {
        let result: (i32,) = sqlx::query_as(
            r#"SELECT agent.expire_pending_outcomes($1)"#
        )
        .bind(older_than_minutes)
        .fetch_one(&self.pool)
        .await?;
        
        Ok(result.0 as i64)
    }
    
    async fn get_promotable_candidates(&self) -> Result<Vec<PromotableCandidate>> {
        let candidates: Vec<PromotableCandidate> = sqlx::query_as(
            r#"SELECT * FROM agent.get_promotable_candidates($1, $2, $3, 50)"#
        )
        .bind(self.min_occurrences)
        .bind(self.min_success_rate)
        .bind(self.min_age_hours)
        .fetch_all(&self.pool)
        .await?;
        
        Ok(candidates)
    }
    
    async fn try_promote(&self, candidate: &PromotableCandidate) -> Result<bool> {
        // Collision check (semantic similarity to other verbs)
        if let Some(embedder) = &self.embedder {
            if !self.check_collision_safe(candidate, embedder).await? {
                // Mark as collision detected
                sqlx::query(
                    r#"UPDATE agent.learning_candidates 
                       SET collision_safe = false, collision_check_at = NOW()
                       WHERE id = $1"#
                )
                .bind(candidate.id)
                .execute(&self.pool)
                .await?;
                
                return Ok(false);
            }
        }
        
        // Apply promotion
        let result: (bool,) = sqlx::query_as(
            r#"SELECT agent.apply_promotion($1, 'system_auto')"#
        )
        .bind(candidate.id)
        .fetch_one(&self.pool)
        .await?;
        
        Ok(result.0)
    }
    
    async fn check_collision_safe(
        &self, 
        candidate: &PromotableCandidate,
        embedder: &Embedder,
    ) -> Result<bool> {
        // Embed the candidate phrase
        let embedding = embedder.embed(&candidate.phrase)?;
        let embedding_vec = pgvector::Vector::from(embedding);
        
        // Check if it matches another verb too closely
        let collision: Option<(String, f32)> = sqlx::query_as(
            r#"
            SELECT verb_name, (1 - (embedding <=> $1))::real as similarity
            FROM "ob-poc".verb_pattern_embeddings
            WHERE verb_name != $2
              AND embedding IS NOT NULL
              AND (1 - (embedding <=> $1)) > $3
            ORDER BY similarity DESC
            LIMIT 1
            "#
        )
        .bind(&embedding_vec)
        .bind(&candidate.verb)
        .bind(self.collision_threshold)
        .fetch_optional(&self.pool)
        .await?;
        
        if let Some((colliding_verb, similarity)) = collision {
            warn!(
                "Collision detected: '{}' matches {} at {:.3} (target: {})",
                candidate.phrase, colliding_verb, similarity, candidate.verb
            );
            
            // Update candidate with collision info
            sqlx::query(
                r#"UPDATE agent.learning_candidates 
                   SET collision_verb = $2 WHERE id = $1"#
            )
            .bind(candidate.id)
            .bind(&colliding_verb)
            .execute(&self.pool)
            .await?;
            
            return Ok(false);
        }
        
        Ok(true)
    }
}

#[derive(Debug, Default)]
pub struct PromotionReport {
    pub expired_outcomes: i64,
    pub promoted: Vec<String>,
    pub skipped: i32,
    pub errors: i32,
}

impl PromotionReport {
    pub fn summary(&self) -> String {
        format!(
            "Promotion cycle: {} expired, {} promoted, {} skipped, {} errors",
            self.expired_outcomes, self.promoted.len(), self.skipped, self.errors
        )
    }
}
```

#### 2.4 Background job scheduling

**File:** `rust/src/ob-poc-web/src/main.rs` (update existing)

Add to server startup:

```rust
// Spawn promotion background task
let promotion_pool = pool.clone();
tokio::spawn(async move {
    // Initial delay
    tokio::time::sleep(Duration::from_secs(60)).await;
    
    let embedder = Embedder::new().ok();
    let service = PromotionService::new(promotion_pool)
        .with_embedder(embedder.unwrap_or_else(|| {
            warn!("No embedder for promotion - collision checks disabled");
            // Return a dummy or make embedder optional in service
        }));
    
    loop {
        match service.run_promotion_cycle().await {
            Ok(report) => info!("{}", report.summary()),
            Err(e) => warn!("Promotion cycle failed: {}", e),
        }
        
        // Run every 6 hours
        tokio::time::sleep(Duration::from_secs(6 * 60 * 60)).await;
    }
});
```

---

### Phase 3: MCP Tools for Manual Review

**File:** `rust/src/mcp/tools/learning_tools.rs` (NEW or update existing)

```rust
/// List candidates needing manual review
pub async fn learning_review_list(
    pool: &PgPool,
    limit: Option<i32>,
) -> Result<Vec<ReviewCandidate>> {
    let candidates = sqlx::query_as(
        r#"SELECT * FROM agent.get_review_candidates(3, 7, $1)"#
    )
    .bind(limit.unwrap_or(20))
    .fetch_all(pool)
    .await?;
    
    Ok(candidates)
}

/// Approve a candidate for promotion
pub async fn learning_approve(
    pool: &PgPool,
    candidate_id: i64,
    actor: &str,
) -> Result<bool> {
    let result: (bool,) = sqlx::query_as(
        r#"SELECT agent.apply_promotion($1, $2)"#
    )
    .bind(candidate_id)
    .bind(actor)
    .fetch_one(pool)
    .await?;
    
    Ok(result.0)
}

/// Reject a candidate (move to blocklist)
pub async fn learning_reject(
    pool: &PgPool,
    candidate_id: i64,
    reason: &str,
    actor: &str,
) -> Result<bool> {
    // Get candidate details
    let candidate: Option<(String, String)> = sqlx::query_as(
        r#"SELECT input_pattern, suggested_output 
           FROM agent.learning_candidates WHERE id = $1"#
    )
    .bind(candidate_id)
    .fetch_optional(pool)
    .await?;
    
    let Some((phrase, verb)) = candidate else {
        return Ok(false);
    };
    
    // Add to blocklist
    sqlx::query(
        r#"INSERT INTO agent.phrase_blocklist (phrase, blocked_verb, reason)
           VALUES ($1, $2, $3)
           ON CONFLICT DO NOTHING"#
    )
    .bind(&phrase)
    .bind(&verb)
    .bind(reason)
    .execute(pool)
    .await?;
    
    // Update candidate status
    sqlx::query(
        r#"UPDATE agent.learning_candidates 
           SET status = 'rejected', reviewed_by = $2, reviewed_at = NOW()
           WHERE id = $1"#
    )
    .bind(candidate_id)
    .bind(actor)
    .execute(pool)
    .await?;
    
    // Audit
    sqlx::query(
        r#"INSERT INTO agent.learning_audit (action, learning_type, candidate_id, actor, details)
           VALUES ('rejected', 'invocation_phrase', $1, $2, $3)"#
    )
    .bind(candidate_id)
    .bind(actor)
    .bind(serde_json::json!({"phrase": phrase, "verb": verb, "reason": reason}))
    .execute(pool)
    .await?;
    
    Ok(true)
}

/// Get learning health metrics
pub async fn learning_metrics(
    pool: &PgPool,
    weeks: Option<i32>,
) -> Result<Vec<WeeklyMetrics>> {
    let metrics = sqlx::query_as(
        r#"SELECT * FROM agent.v_learning_health_weekly LIMIT $1"#
    )
    .bind(weeks.unwrap_or(8))
    .fetch_all(pool)
    .await?;
    
    Ok(metrics)
}
```

---

## Thresholds Reference

| Parameter | Value | Rationale |
|-----------|-------|-----------|
| `min_occurrences` | 5 | Enough signal, not one-off |
| `min_success_rate` | 0.80 | 4/5 successful uses |
| `min_age_hours` | 24 | Cool-down for burst patterns |
| `min_words` | 3 | Reject "create", "add" |
| `max_words` | 15 | Reject rambling inputs |
| `stopword_ratio_max` | 0.70 | Reject generic phrases |
| `collision_threshold` | 0.92 | Prevent verb confusion |
| `outcome_expiry_minutes` | 30 | Pending → abandoned |
| `review_age_days` | 7 | Queue for manual review |

---

## Metrics to Track (Weekly)

| Metric | Target | Alert If |
|--------|--------|----------|
| Top-1 hit rate | > 85% | < 75% |
| Ambiguity rate | < 10% | > 20% |
| Correction rate | < 5% | > 10% |
| NoMatch rate | < 5% | > 15% |
| Candidates promoted/week | 5-20 | > 50 (review quality) |
| Collision blocks/week | < 5 | > 10 (semantic drift) |

---

## Files to Create/Modify

| File | Action | Description |
|------|--------|-------------|
| `migrations/043_feedback_loop_promotion.sql` | CREATE | Schema changes |
| `rust/crates/ob-semantic-matcher/src/feedback/promotion.rs` | CREATE | Promotion service |
| `rust/crates/ob-semantic-matcher/src/feedback/mod.rs` | UPDATE | Export promotion |
| `rust/crates/ob-semantic-matcher/src/feedback/repository.rs` | UPDATE | Add learning signal call |
| `rust/crates/ob-semantic-matcher/src/feedback/service.rs` | UPDATE | Add record_learning_signal |
| `rust/src/mcp/tools/learning_tools.rs` | CREATE/UPDATE | MCP tools |
| `rust/src/ob-poc-web/src/main.rs` | UPDATE | Background job |

---

## Testing Checklist

- [ ] Migration applies cleanly
- [ ] `record_learning_signal` filters short/generic phrases
- [ ] `get_promotable_candidates` returns correct candidates
- [ ] Collision check blocks phrases matching other verbs
- [ ] `apply_promotion` adds to `dsl_verbs.intent_patterns`
- [ ] Audit log captures all promotions/rejections
- [ ] Background job runs every 6 hours
- [ ] MCP tools work for manual review
- [ ] Metrics views return data
- [ ] `populate_embeddings` picks up new patterns

---

## Success Criteria

After 2 weeks of operation:
- [ ] Top-1 hit rate improved (or stable)
- [ ] Correction rate decreased
- [ ] 10-30 patterns auto-promoted
- [ ] No garbage patterns promoted
- [ ] Weekly review queue < 20 candidates
