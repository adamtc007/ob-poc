//! Confidence Decay System
//!
//! Manages confidence scores for learned phrase→verb mappings:
//! - Decays confidence when a learned phrase leads to wrong verb selection
//! - Boosts confidence when the correct verb is confirmed
//! - Handles ambiguous phrases that may legitimately map to multiple verbs

use anyhow::Result;
use sqlx::PgPool;
use uuid::Uuid;

/// Confidence decay manager
pub struct ConfidenceDecay {
    pool: PgPool,
    /// Multiplier for decay on wrong selection (e.g., 0.7 = 30% reduction)
    decay_factor: f32,
    /// Amount to add on correct confirmation (e.g., 0.2)
    boost_amount: f32,
    /// Minimum confidence floor (e.g., 0.1)
    min_confidence: f32,
    /// Maximum confidence ceiling (e.g., 1.0)
    max_confidence: f32,
}

impl ConfidenceDecay {
    /// Create with default parameters
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            decay_factor: 0.7,
            boost_amount: 0.2,
            min_confidence: 0.1,
            max_confidence: 1.0,
        }
    }

    /// Create with custom parameters
    pub fn with_params(
        pool: PgPool,
        decay_factor: f32,
        boost_amount: f32,
        min_confidence: f32,
        max_confidence: f32,
    ) -> Self {
        Self {
            pool,
            decay_factor,
            boost_amount,
            min_confidence,
            max_confidence,
        }
    }

    /// Apply decay when a learned phrase led to wrong verb selection
    ///
    /// For user-specific phrases: reduces confidence score
    /// For global phrases: reduces occurrence_count
    pub async fn decay_wrong(
        &self,
        phrase: &str,
        wrong_verb: &str,
        user_id: Option<Uuid>,
    ) -> Result<DecayResult> {
        let normalized = phrase.trim().to_lowercase();

        // Try user-specific table first
        if let Some(uid) = user_id {
            let result = sqlx::query_as::<_, (f32,)>(
                r#"
                UPDATE agent.user_learned_phrases
                SET confidence = GREATEST($1, confidence * $2),
                    updated_at = now()
                WHERE user_id = $3
                  AND LOWER(phrase) = $4
                  AND verb = $5
                RETURNING confidence
                "#,
            )
            .bind(self.min_confidence)
            .bind(self.decay_factor)
            .bind(uid)
            .bind(&normalized)
            .bind(wrong_verb)
            .fetch_optional(&self.pool)
            .await?;

            if let Some((new_conf,)) = result {
                return Ok(DecayResult {
                    phrase: phrase.to_string(),
                    verb: wrong_verb.to_string(),
                    new_confidence: new_conf,
                    action: DecayAction::Decayed,
                    scope: DecayScope::UserSpecific(uid),
                });
            }
        }

        // Fall back to global table - reduce occurrence count
        let result = sqlx::query_as::<_, (i32,)>(
            r#"
            UPDATE agent.invocation_phrases
            SET occurrence_count = GREATEST(1, occurrence_count - 1),
                updated_at = now()
            WHERE LOWER(phrase) = $1 AND verb = $2
            RETURNING occurrence_count
            "#,
        )
        .bind(&normalized)
        .bind(wrong_verb)
        .fetch_optional(&self.pool)
        .await?;

        match result {
            Some((count,)) => Ok(DecayResult {
                phrase: phrase.to_string(),
                verb: wrong_verb.to_string(),
                new_confidence: (count as f32 / 10.0).min(1.0), // Approximate confidence from count
                action: DecayAction::Decayed,
                scope: DecayScope::Global,
            }),
            None => Ok(DecayResult {
                phrase: phrase.to_string(),
                verb: wrong_verb.to_string(),
                new_confidence: 0.0,
                action: DecayAction::NotFound,
                scope: DecayScope::Global,
            }),
        }
    }

    /// Boost confidence when correct verb is confirmed
    ///
    /// Creates or updates the phrase→verb mapping with increased confidence
    pub async fn boost_correct(
        &self,
        phrase: &str,
        correct_verb: &str,
        user_id: Option<Uuid>,
    ) -> Result<DecayResult> {
        let normalized = phrase.trim().to_lowercase();

        if let Some(uid) = user_id {
            let result = sqlx::query_as::<_, (f32,)>(
                r#"
                INSERT INTO agent.user_learned_phrases
                    (user_id, phrase, verb, confidence, source)
                VALUES ($1, $2, $3, $4, 'confidence_boost')
                ON CONFLICT (user_id, phrase) DO UPDATE
                SET verb = EXCLUDED.verb,
                    confidence = LEAST($5, agent.user_learned_phrases.confidence + $4),
                    occurrence_count = agent.user_learned_phrases.occurrence_count + 1,
                    updated_at = now()
                RETURNING confidence
                "#,
            )
            .bind(uid)
            .bind(&normalized)
            .bind(correct_verb)
            .bind(self.boost_amount)
            .bind(self.max_confidence)
            .fetch_one(&self.pool)
            .await?;

            return Ok(DecayResult {
                phrase: phrase.to_string(),
                verb: correct_verb.to_string(),
                new_confidence: result.0,
                action: DecayAction::Boosted,
                scope: DecayScope::UserSpecific(uid),
            });
        }

        // Global table - increase occurrence count
        let result = sqlx::query_as::<_, (i32,)>(
            r#"
            INSERT INTO agent.invocation_phrases
                (phrase, verb, occurrence_count, source)
            VALUES ($1, $2, 1, 'confidence_boost')
            ON CONFLICT (phrase) DO UPDATE
            SET verb = EXCLUDED.verb,
                occurrence_count = agent.invocation_phrases.occurrence_count + 1,
                updated_at = now()
            RETURNING occurrence_count
            "#,
        )
        .bind(&normalized)
        .bind(correct_verb)
        .fetch_one(&self.pool)
        .await?;

        Ok(DecayResult {
            phrase: phrase.to_string(),
            verb: correct_verb.to_string(),
            new_confidence: (result.0 as f32 / 10.0).min(1.0),
            action: DecayAction::Boosted,
            scope: DecayScope::Global,
        })
    }

    /// Handle a verb correction event
    ///
    /// Decays the wrong verb's confidence and boosts the correct verb's confidence
    pub async fn handle_correction(
        &self,
        phrase: &str,
        wrong_verb: &str,
        correct_verb: &str,
        user_id: Option<Uuid>,
    ) -> Result<CorrectionResult> {
        let decay_result = self.decay_wrong(phrase, wrong_verb, user_id).await?;
        let boost_result = self.boost_correct(phrase, correct_verb, user_id).await?;

        tracing::info!(
            phrase = %phrase,
            wrong_verb = %wrong_verb,
            correct_verb = %correct_verb,
            decay_conf = decay_result.new_confidence,
            boost_conf = boost_result.new_confidence,
            "Applied confidence correction"
        );

        Ok(CorrectionResult {
            phrase: phrase.to_string(),
            wrong_verb: wrong_verb.to_string(),
            wrong_new_confidence: decay_result.new_confidence,
            correct_verb: correct_verb.to_string(),
            correct_new_confidence: boost_result.new_confidence,
            scope: boost_result.scope,
        })
    }
}

/// Result of a decay/boost operation
#[derive(Debug, Clone)]
pub struct DecayResult {
    pub phrase: String,
    pub verb: String,
    pub new_confidence: f32,
    pub action: DecayAction,
    pub scope: DecayScope,
}

/// What action was taken
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecayAction {
    Decayed,
    Boosted,
    NotFound,
}

/// Scope of the decay/boost
#[derive(Debug, Clone, Copy)]
pub enum DecayScope {
    Global,
    UserSpecific(Uuid),
}

/// Result of handling a full correction
#[derive(Debug)]
pub struct CorrectionResult {
    pub phrase: String,
    pub wrong_verb: String,
    pub wrong_new_confidence: f32,
    pub correct_verb: String,
    pub correct_new_confidence: f32,
    pub scope: DecayScope,
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_decay_calculations() {
        // Test decay math without needing a DB pool
        let decay_factor = 0.7_f32;
        let boost_amount = 0.2_f32;
        let min_confidence = 0.1_f32;
        let max_confidence = 1.0_f32;

        // Test decay: 1.0 * 0.7 = 0.7
        let confidence = 1.0_f32;
        let decayed = (confidence * decay_factor).max(min_confidence);
        assert!((decayed - 0.7).abs() < 0.001);

        // Test decay floor: 0.1 * 0.7 = 0.07, but min is 0.1
        let low_confidence = 0.1_f32;
        let decayed_low = (low_confidence * decay_factor).max(min_confidence);
        assert!((decayed_low - 0.1).abs() < 0.001);

        // Test boost: 0.5 + 0.2 = 0.7
        let mid_confidence = 0.5_f32;
        let boosted = (mid_confidence + boost_amount).min(max_confidence);
        assert!((boosted - 0.7).abs() < 0.001);

        // Test boost ceiling: 0.9 + 0.2 = 1.1, but max is 1.0
        let high_confidence = 0.9_f32;
        let boosted_high = (high_confidence + boost_amount).min(max_confidence);
        assert!((boosted_high - 1.0).abs() < 0.001);
    }
}
