//! Verb Service - Database operations for verb discovery and semantic search
//!
//! Centralizes all verb-related database access for the semantic pipeline:
//! - User learned phrases (exact + semantic)
//! - Global learned phrases (semantic)
//! - Verb pattern embeddings (cold start semantic)
//! - Blocklist checking
//! - Verb descriptions from dsl_verbs

use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

// ============================================================================
// Types
// ============================================================================

/// A user-learned phrase match (exact)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserLearnedExactMatch {
    pub phrase: String,
    pub verb: String,
    pub confidence: f32,
}

/// A semantic match with similarity score
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticMatch {
    pub phrase: String,
    pub verb: String,
    pub similarity: f64,
    pub confidence: Option<f32>,
    pub category: Option<String>,
}

/// Verb description from dsl_verbs table
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbDescription {
    pub full_name: String,
    pub description: Option<String>,
}

// ============================================================================
// Service
// ============================================================================

/// Verb service for centralized DB access
pub struct VerbService {
    pool: PgPool,
}

impl VerbService {
    /// Create a new verb service
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get the pool reference
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    // ========================================================================
    // User Learned Phrases
    // ========================================================================

    /// Find user-learned phrase by exact match
    pub async fn find_user_learned_exact(
        &self,
        user_id: Uuid,
        phrase: &str,
    ) -> Result<Option<UserLearnedExactMatch>, sqlx::Error> {
        let row = sqlx::query_as::<_, (String, String, f32)>(
            r#"
            SELECT phrase, verb, confidence
            FROM agent.user_learned_phrases
            WHERE user_id = $1 AND LOWER(phrase) = $2
            "#,
        )
        .bind(user_id)
        .bind(phrase.to_lowercase())
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|(phrase, verb, confidence)| UserLearnedExactMatch {
            phrase,
            verb,
            confidence,
        }))
    }

    /// Find user-learned phrase by semantic similarity
    pub async fn find_user_learned_semantic(
        &self,
        user_id: Uuid,
        query_embedding: &[f32],
        threshold: f32,
    ) -> Result<Option<SemanticMatch>, sqlx::Error> {
        let row = sqlx::query_as::<_, (String, String, f32, f64)>(
            r#"
            SELECT phrase, verb, confidence, 1 - (embedding <=> $1::vector) as similarity
            FROM agent.user_learned_phrases
            WHERE user_id = $2
              AND embedding IS NOT NULL
            ORDER BY embedding <=> $1::vector
            LIMIT 1
            "#,
        )
        .bind(query_embedding)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some((phrase, verb, confidence, similarity)) if similarity as f32 > threshold => {
                Ok(Some(SemanticMatch {
                    phrase,
                    verb,
                    similarity,
                    confidence: Some(confidence),
                    category: None,
                }))
            }
            _ => Ok(None),
        }
    }

    /// Find user-learned phrases by semantic similarity (top-k) - Issue D/J
    pub async fn find_user_learned_semantic_topk(
        &self,
        user_id: Uuid,
        query_embedding: &[f32],
        threshold: f32,
        limit: usize,
    ) -> Result<Vec<SemanticMatch>, sqlx::Error> {
        let rows = sqlx::query_as::<_, (String, String, f32, f64)>(
            r#"
            SELECT phrase, verb, confidence, 1 - (embedding <=> $1::vector) as similarity
            FROM agent.user_learned_phrases
            WHERE user_id = $2
              AND embedding IS NOT NULL
              AND 1 - (embedding <=> $1::vector) > $4
            ORDER BY embedding <=> $1::vector
            LIMIT $3
            "#,
        )
        .bind(query_embedding)
        .bind(user_id)
        .bind(limit as i32)
        .bind(threshold)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(phrase, verb, confidence, similarity)| SemanticMatch {
                phrase,
                verb,
                similarity,
                confidence: Some(confidence),
                category: None,
            })
            .collect())
    }

    // ========================================================================
    // Global Learned Phrases
    // ========================================================================

    /// Find global learned phrase by semantic similarity
    pub async fn find_global_learned_semantic(
        &self,
        query_embedding: &[f32],
        threshold: f32,
    ) -> Result<Option<SemanticMatch>, sqlx::Error> {
        let row = sqlx::query_as::<_, (String, String, f64)>(
            r#"
            SELECT phrase, verb, 1 - (embedding <=> $1::vector) as similarity
            FROM agent.invocation_phrases
            WHERE embedding IS NOT NULL
            ORDER BY embedding <=> $1::vector
            LIMIT 1
            "#,
        )
        .bind(query_embedding)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some((phrase, verb, similarity)) if similarity as f32 > threshold => {
                Ok(Some(SemanticMatch {
                    phrase,
                    verb,
                    similarity,
                    confidence: None,
                    category: None,
                }))
            }
            _ => Ok(None),
        }
    }

    /// Find global learned phrases by semantic similarity (top-k) - Issue D/J
    pub async fn find_global_learned_semantic_topk(
        &self,
        query_embedding: &[f32],
        threshold: f32,
        limit: usize,
    ) -> Result<Vec<SemanticMatch>, sqlx::Error> {
        let rows = sqlx::query_as::<_, (String, String, f64)>(
            r#"
            SELECT phrase, verb, 1 - (embedding <=> $1::vector) as similarity
            FROM agent.invocation_phrases
            WHERE embedding IS NOT NULL
              AND 1 - (embedding <=> $1::vector) > $3
            ORDER BY embedding <=> $1::vector
            LIMIT $2
            "#,
        )
        .bind(query_embedding)
        .bind(limit as i32)
        .bind(threshold)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(phrase, verb, similarity)| SemanticMatch {
                phrase,
                verb,
                similarity,
                confidence: None,
                category: None,
            })
            .collect())
    }

    // ========================================================================
    // Verb Pattern Embeddings (Cold Start)
    // ========================================================================

    /// Search verb pattern embeddings by semantic similarity
    /// This is the primary semantic lookup for cold start
    pub async fn search_verb_patterns_semantic(
        &self,
        query_embedding: &[f32],
        limit: usize,
        min_similarity: f32,
    ) -> Result<Vec<SemanticMatch>, sqlx::Error> {
        let rows = sqlx::query_as::<_, (String, String, f64, Option<String>)>(
            r#"
            SELECT pattern_phrase, verb_name, 1 - (embedding <=> $1::vector) as similarity, category
            FROM "ob-poc".verb_pattern_embeddings
            WHERE embedding IS NOT NULL
              AND 1 - (embedding <=> $1::vector) > $3
            ORDER BY embedding <=> $1::vector
            LIMIT $2
            "#,
        )
        .bind(query_embedding)
        .bind(limit as i32)
        .bind(min_similarity)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(phrase, verb, similarity, category)| SemanticMatch {
                phrase,
                verb,
                similarity,
                confidence: None,
                category,
            })
            .collect())
    }

    // ========================================================================
    // Blocklist
    // ========================================================================

    /// Check if a verb is blocked for the given query
    pub async fn check_blocklist(
        &self,
        query_embedding: &[f32],
        user_id: Option<Uuid>,
        verb: &str,
        threshold: f32,
    ) -> Result<bool, sqlx::Error> {
        let blocked = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS (
                SELECT 1 FROM agent.phrase_blocklist
                WHERE blocked_verb = $1
                  AND (user_id IS NULL OR user_id = $2)
                  AND (expires_at IS NULL OR expires_at > now())
                  AND embedding IS NOT NULL
                  AND 1 - (embedding <=> $3::vector) > $4
            )
            "#,
        )
        .bind(verb)
        .bind(user_id)
        .bind(query_embedding)
        .bind(threshold)
        .fetch_one(&self.pool)
        .await?;

        Ok(blocked)
    }

    // ========================================================================
    // Verb Descriptions
    // ========================================================================

    /// Get verb description from dsl_verbs table
    pub async fn get_verb_description(
        &self,
        full_name: &str,
    ) -> Result<Option<String>, sqlx::Error> {
        let row = sqlx::query_as::<_, (Option<String>,)>(
            r#"
            SELECT description
            FROM "ob-poc".dsl_verbs
            WHERE full_name = $1
            "#,
        )
        .bind(full_name)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.and_then(|(desc,)| desc))
    }

    /// Get multiple verb descriptions at once
    pub async fn get_verb_descriptions(
        &self,
        full_names: &[String],
    ) -> Result<Vec<VerbDescription>, sqlx::Error> {
        if full_names.is_empty() {
            return Ok(Vec::new());
        }

        let rows = sqlx::query_as::<_, (String, Option<String>)>(
            r#"
            SELECT full_name, description
            FROM "ob-poc".dsl_verbs
            WHERE full_name = ANY($1)
            "#,
        )
        .bind(full_names)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(full_name, description)| VerbDescription {
                full_name,
                description,
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    // Integration tests would go here
}
