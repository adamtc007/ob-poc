//! Semantic Matcher - combines embeddings with phonetic fallback
//!
//! The main entry point for voice command matching.

use crate::{
    embedder::Embedder,
    phonetic::PhoneticMatcher,
    types::{MatchMethod, MatchResult, MatcherConfig, MatcherError, VerbPattern},
};
use pgvector::Vector;
use sqlx::PgPool;
use tracing::{debug, info, instrument, warn};

/// Semantic matcher for voice commands
///
/// Combines:
/// 1. Semantic similarity via sentence embeddings
/// 2. Phonetic matching via Double Metaphone
/// 3. Exact match fallback
/// 4. Optional caching
pub struct SemanticMatcher {
    embedder: Embedder,
    phonetic: PhoneticMatcher,
    pool: PgPool,
    config: MatcherConfig,
}

impl SemanticMatcher {
    /// Create a new semantic matcher
    ///
    /// This will download the embedding model on first use (~22MB).
    pub async fn new(pool: PgPool) -> Result<Self, MatcherError> {
        Self::with_config(pool, MatcherConfig::default()).await
    }

    /// Create with custom configuration
    pub async fn with_config(pool: PgPool, config: MatcherConfig) -> Result<Self, MatcherError> {
        info!(
            "Initializing SemanticMatcher with model: {}",
            config.model_name
        );

        let embedder = Embedder::with_model(&config.model_name)
            .map_err(|e| MatcherError::ModelLoad(e.to_string()))?;

        Ok(Self {
            embedder,
            phonetic: PhoneticMatcher::new(),
            pool,
            config,
        })
    }

    /// Find the best matching verb for a voice transcript with alternatives
    ///
    /// Returns the primary match and up to `max_alternatives` alternative matches.
    /// This is useful for feedback capture where we want to know what alternatives
    /// the user might have selected instead.
    #[instrument(skip(self), fields(transcript = %transcript))]
    pub async fn find_match_with_alternatives(
        &self,
        transcript: &str,
        max_alternatives: usize,
    ) -> Result<(MatchResult, Vec<MatchResult>), MatcherError> {
        let normalized = transcript.trim().to_lowercase();

        if normalized.is_empty() {
            return Err(MatcherError::NoMatch);
        }

        // Get embedding for semantic search (query mode - user input searching patterns)
        let embedding = self
            .embedder
            .embed_query(&normalized)
            .map_err(|e| MatcherError::Embedding(e.to_string()))?;

        // Get more candidates than needed to filter for alternatives
        let semantic_matches = self
            .find_similar_patterns(&embedding, self.config.top_k.max(max_alternatives + 3))
            .await?;

        if semantic_matches.is_empty() {
            return Err(MatcherError::NoMatch);
        }

        // Primary match is the best one
        let primary = semantic_matches[0].clone();

        // Alternatives are the next best matches, but only include those above min threshold
        // and with different verb names
        let alternatives: Vec<MatchResult> = semantic_matches
            .into_iter()
            .skip(1)
            .filter(|m| m.similarity >= self.config.min_similarity)
            .filter(|m| m.verb_name != primary.verb_name)
            .take(max_alternatives)
            .collect();

        Ok((primary, alternatives))
    }

    /// Find the best matching verb for a voice transcript
    #[instrument(skip(self), fields(transcript = %transcript))]
    pub async fn find_match(&self, transcript: &str) -> Result<MatchResult, MatcherError> {
        let normalized = transcript.trim().to_lowercase();

        if normalized.is_empty() {
            return Err(MatcherError::NoMatch);
        }

        // 1. Check cache first
        if self.config.use_cache {
            if let Some(cached) = self.check_cache(&normalized).await? {
                debug!("Cache hit for: {}", normalized);
                return Ok(cached);
            }
        }

        // 2. Try exact match
        if let Some(exact) = self.try_exact_match(&normalized).await? {
            debug!("Exact match found: {}", exact.verb_name);
            if self.config.use_cache {
                self.update_cache(&normalized, &exact).await?;
            }
            return Ok(exact);
        }

        // 3. Semantic similarity search (query mode - user input searching patterns)
        let embedding = self
            .embedder
            .embed_query(&normalized)
            .map_err(|e| MatcherError::Embedding(e.to_string()))?;

        let semantic_matches = self
            .find_similar_patterns(&embedding, self.config.top_k)
            .await?;

        // If high confidence match, return immediately
        if let Some(best) = semantic_matches.first() {
            if best.similarity >= self.config.high_confidence_threshold {
                debug!(
                    "High confidence semantic match: {} ({})",
                    best.verb_name, best.similarity
                );
                if self.config.use_cache {
                    self.update_cache(&normalized, best).await?;
                }
                return Ok(best.clone());
            }
        }

        // 4. Try phonetic fallback
        let phonetic_codes = self.phonetic.encode_phrase(&normalized);
        if !phonetic_codes.is_empty() {
            if let Some(phonetic_match) = self.find_phonetic_match(&phonetic_codes).await? {
                // Prefer phonetic if it's better than low-confidence semantic
                let semantic_best = semantic_matches
                    .first()
                    .map(|m| m.similarity)
                    .unwrap_or(0.0);
                if phonetic_match.similarity > semantic_best {
                    debug!(
                        "Phonetic match preferred: {} ({})",
                        phonetic_match.verb_name, phonetic_match.similarity
                    );
                    if self.config.use_cache {
                        self.update_cache(&normalized, &phonetic_match).await?;
                    }
                    return Ok(phonetic_match);
                }
            }
        }

        // 5. Return best semantic match if above minimum threshold
        if let Some(best) = semantic_matches.into_iter().next() {
            if best.similarity >= self.config.min_similarity {
                debug!(
                    "Returning semantic match: {} ({})",
                    best.verb_name, best.similarity
                );
                if self.config.use_cache {
                    self.update_cache(&normalized, &best).await?;
                }
                return Ok(best);
            }
        }

        warn!("No match found for: {}", normalized);
        Err(MatcherError::NoMatch)
    }

    /// Find top-k similar patterns using pgvector
    async fn find_similar_patterns(
        &self,
        embedding: &[f32],
        top_k: usize,
    ) -> Result<Vec<MatchResult>, MatcherError> {
        let embedding_vec = Vector::from(embedding.to_vec());

        let rows = sqlx::query_as::<_, (String, String, f32, String, bool, i32)>(
            r#"
            SELECT
                verb_name,
                pattern_phrase,
                1 - (embedding <=> $1::vector) AS similarity,
                category,
                is_agent_bound,
                priority
            FROM "ob-poc".verb_pattern_embeddings
            WHERE 1 - (embedding <=> $1::vector) >= $2
            ORDER BY embedding <=> $1::vector, priority
            LIMIT $3
            "#,
        )
        .bind(&embedding_vec)
        .bind(self.config.min_similarity)
        .bind(top_k as i32)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(
                |(verb_name, pattern_phrase, similarity, category, is_agent_bound, _priority)| {
                    MatchResult {
                        verb_name,
                        pattern_phrase,
                        similarity,
                        match_method: MatchMethod::Semantic,
                        category,
                        is_agent_bound,
                    }
                },
            )
            .collect())
    }

    /// Try exact normalized match
    async fn try_exact_match(&self, normalized: &str) -> Result<Option<MatchResult>, MatcherError> {
        let row = sqlx::query_as::<_, (String, String, String, bool)>(
            r#"
            SELECT verb_name, pattern_phrase, category, is_agent_bound
            FROM "ob-poc".verb_pattern_embeddings
            WHERE pattern_normalized = $1
            LIMIT 1
            "#,
        )
        .bind(normalized)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(
            |(verb_name, pattern_phrase, category, is_agent_bound)| MatchResult {
                verb_name,
                pattern_phrase,
                similarity: 1.0,
                match_method: MatchMethod::Exact,
                category,
                is_agent_bound,
            },
        ))
    }

    /// Find match using phonetic codes
    async fn find_phonetic_match(
        &self,
        codes: &[String],
    ) -> Result<Option<MatchResult>, MatcherError> {
        if codes.is_empty() {
            return Ok(None);
        }

        let row = sqlx::query_as::<_, (String, String, String, bool, i64)>(
            r#"
            SELECT
                verb_name,
                pattern_phrase,
                category,
                is_agent_bound,
                array_length(
                    ARRAY(SELECT unnest(phonetic_codes) INTERSECT SELECT unnest($1::text[])),
                    1
                ) AS match_count
            FROM "ob-poc".verb_pattern_embeddings
            WHERE phonetic_codes && $1::text[]
            ORDER BY match_count DESC, priority
            LIMIT 1
            "#,
        )
        .bind(codes)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(
            |(verb_name, pattern_phrase, category, is_agent_bound, match_count)| {
                // Calculate similarity based on how many codes matched
                let similarity = match_count as f32 / codes.len().max(1) as f32;
                MatchResult {
                    verb_name,
                    pattern_phrase,
                    similarity: similarity.min(0.95), // Cap phonetic matches at 0.95
                    match_method: MatchMethod::Phonetic,
                    category,
                    is_agent_bound,
                }
            },
        ))
    }

    /// Check the cache for a previous match
    async fn check_cache(&self, normalized: &str) -> Result<Option<MatchResult>, MatcherError> {
        let row = sqlx::query_as::<_, (String, f32, String)>(
            r#"
            UPDATE "ob-poc".semantic_match_cache
            SET hit_count = hit_count + 1, last_accessed_at = now()
            WHERE transcript_normalized = $1
            RETURNING matched_verb, similarity_score, match_method
            "#,
        )
        .bind(normalized)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|(verb_name, similarity, _method)| MatchResult {
            verb_name,
            pattern_phrase: normalized.to_string(), // Cache doesn't store original pattern
            similarity,
            match_method: MatchMethod::Cached,
            category: "cached".to_string(),
            is_agent_bound: false, // Will be looked up if needed
        }))
    }

    /// Update the cache with a new match
    async fn update_cache(
        &self,
        normalized: &str,
        result: &MatchResult,
    ) -> Result<(), MatcherError> {
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".semantic_match_cache
                (transcript_normalized, matched_verb, similarity_score, match_method)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (transcript_normalized) DO UPDATE
            SET matched_verb = $2, similarity_score = $3, match_method = $4,
                hit_count = semantic_match_cache.hit_count + 1,
                last_accessed_at = now()
            "#,
        )
        .bind(normalized)
        .bind(&result.verb_name)
        .bind(result.similarity)
        .bind(result.match_method.to_string())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Populate the database with patterns from verb_rag_metadata
    ///
    /// This should be called once at startup or when patterns change.
    pub async fn populate_patterns(
        &self,
        patterns: Vec<VerbPattern>,
    ) -> Result<usize, MatcherError> {
        info!("Populating {} verb patterns", patterns.len());

        let mut count = 0;
        for pattern in patterns {
            let embedding_vec = Vector::from(pattern.embedding);

            sqlx::query(
                r#"
                INSERT INTO "ob-poc".verb_pattern_embeddings
                    (verb_name, pattern_phrase, pattern_normalized, phonetic_codes,
                     embedding, category, is_agent_bound, priority)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                ON CONFLICT (verb_name, pattern_normalized) DO UPDATE
                SET pattern_phrase = $2, phonetic_codes = $4, embedding = $5,
                    category = $6, is_agent_bound = $7, priority = $8,
                    updated_at = now()
                "#,
            )
            .bind(&pattern.verb_name)
            .bind(&pattern.pattern_phrase)
            .bind(&pattern.pattern_normalized)
            .bind(&pattern.phonetic_codes)
            .bind(&embedding_vec)
            .bind(&pattern.category)
            .bind(pattern.is_agent_bound)
            .bind(pattern.priority)
            .execute(&self.pool)
            .await?;

            count += 1;
        }

        info!("Populated {} patterns successfully", count);
        Ok(count)
    }

    /// Get the embedder for external use (e.g., populating patterns)
    pub fn embedder(&self) -> &Embedder {
        &self.embedder
    }

    /// Get the phonetic matcher for external use
    pub fn phonetic(&self) -> &PhoneticMatcher {
        &self.phonetic
    }
}
