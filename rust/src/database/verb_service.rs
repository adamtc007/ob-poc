//! Verb Service - Database operations for verb discovery and semantic search
//!
//! Centralizes all verb-related database access for the semantic pipeline:
//! - User learned phrases (exact + semantic)
//! - Global learned phrases (semantic)
//! - Verb pattern embeddings (cold start semantic)
//! - Blocklist checking
//! - Verb descriptions from dsl_verbs

use pgvector::Vector;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

// ============================================================================
// Public Types
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
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct VerbDescription {
    pub full_name: String,
    pub description: Option<String>,
}

// ============================================================================
// Row Structs (replacing anonymous tuples for FromRow)
// ============================================================================

/// Row struct for user learned exact match queries
#[derive(Debug, sqlx::FromRow)]
struct UserLearnedExactRow {
    phrase: String,
    verb: String,
    confidence: f32,
}

/// Row struct for user learned semantic queries (includes confidence)
#[derive(Debug, sqlx::FromRow)]
struct UserLearnedSemanticRow {
    phrase: String,
    verb: String,
    confidence: f32,
    similarity: f64,
}

/// Row struct for global learned semantic queries (no confidence)
#[derive(Debug, sqlx::FromRow)]
struct GlobalLearnedSemanticRow {
    phrase: String,
    verb: String,
    similarity: f64,
}

/// Row struct for exact pattern lookup from dsl_verbs pattern arrays.
#[derive(Debug, sqlx::FromRow)]
struct ExactPatternRow {
    phrase: String,
    verb: String,
}

/// Row struct for verb pattern embedding queries (includes category)
#[derive(Debug, sqlx::FromRow)]
struct VerbPatternSemanticRow {
    pattern_phrase: String,
    verb_name: String,
    similarity: f64,
    category: Option<String>,
}

/// Row struct for centroid query results
#[derive(Debug, sqlx::FromRow)]
struct VerbCentroidRow {
    verb_name: String,
    similarity: f64,
    phrase_count: i32,
}

/// Centroid match result with score and phrase count
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbCentroidMatch {
    pub verb_name: String,
    pub score: f64,
    pub phrase_count: i32,
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
        let row = sqlx::query_as::<_, UserLearnedExactRow>(
            r#"
            SELECT phrase, verb, confidence
            FROM "ob-poc".user_learned_phrases
            WHERE user_id = $1 AND LOWER(phrase) = $2
            "#,
        )
        .bind(user_id)
        .bind(phrase.to_lowercase())
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| UserLearnedExactMatch {
            phrase: r.phrase,
            verb: r.verb,
            confidence: r.confidence,
        }))
    }

    /// Find user-learned phrase by semantic similarity
    pub async fn find_user_learned_semantic(
        &self,
        user_id: Uuid,
        query_embedding: &[f32],
        threshold: f32,
    ) -> Result<Option<SemanticMatch>, sqlx::Error> {
        let embedding_vec = Vector::from(query_embedding.to_vec());

        let row = sqlx::query_as::<_, UserLearnedSemanticRow>(
            r#"
            SELECT phrase, verb, confidence, 1 - (embedding <=> $1::vector) as similarity
            FROM "ob-poc".user_learned_phrases
            WHERE user_id = $2
              AND embedding IS NOT NULL
            ORDER BY embedding <=> $1::vector
            LIMIT 1
            "#,
        )
        .bind(&embedding_vec)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(r) if r.similarity as f32 > threshold => Ok(Some(SemanticMatch {
                phrase: r.phrase,
                verb: r.verb,
                similarity: r.similarity,
                confidence: Some(r.confidence),
                category: None,
            })),
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
        let embedding_vec = Vector::from(query_embedding.to_vec());

        let rows = sqlx::query_as::<_, UserLearnedSemanticRow>(
            r#"
            SELECT phrase, verb, confidence, 1 - (embedding <=> $1::vector) as similarity
            FROM "ob-poc".user_learned_phrases
            WHERE user_id = $2
              AND embedding IS NOT NULL
              AND 1 - (embedding <=> $1::vector) > $4
            ORDER BY embedding <=> $1::vector
            LIMIT $3
            "#,
        )
        .bind(&embedding_vec)
        .bind(user_id)
        .bind(limit as i32)
        .bind(threshold)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| SemanticMatch {
                phrase: r.phrase,
                verb: r.verb,
                similarity: r.similarity,
                confidence: Some(r.confidence),
                category: None,
            })
            .collect())
    }

    // ========================================================================
    // Global Learned Phrases
    // ========================================================================

    /// Find exact pattern matches from `dsl_verbs` pattern arrays.
    ///
    /// Checks both:
    /// - `yaml_intent_patterns` (seeded from YAML invocation phrases)
    /// - `intent_patterns` (learned patterns)
    ///
    /// This provides deterministic exact matching even before embedding refresh.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let service = VerbService::new(pool);
    /// let matches = service
    ///     .find_exact_verb_patterns("show me products", None, 5)
    ///     .await?;
    /// ```
    pub async fn find_exact_verb_patterns(
        &self,
        phrase: &str,
        allowed_verbs: Option<&[String]>,
        limit: usize,
    ) -> Result<Vec<SemanticMatch>, sqlx::Error> {
        let normalized = phrase.trim().to_lowercase();
        if normalized.is_empty() || limit == 0 {
            return Ok(Vec::new());
        }

        let rows = if let Some(allowed) = allowed_verbs {
            if allowed.is_empty() {
                Vec::new()
            } else {
                sqlx::query_as::<_, ExactPatternRow>(
                    r#"
                    SELECT DISTINCT ON (dv.full_name)
                        p.pattern AS phrase,
                        dv.full_name AS verb
                    FROM "ob-poc".dsl_verbs dv
                    CROSS JOIN LATERAL unnest(
                        COALESCE(dv.yaml_intent_patterns, '{}'::text[])
                        || COALESCE(dv.intent_patterns, '{}'::text[])
                    ) AS p(pattern)
                    WHERE dv.full_name = ANY($2)
                      AND LOWER(TRIM(p.pattern)) = $1
                    ORDER BY dv.full_name
                    LIMIT $3
                    "#,
                )
                .bind(&normalized)
                .bind(allowed)
                .bind(limit as i32)
                .fetch_all(&self.pool)
                .await?
            }
        } else {
            sqlx::query_as::<_, ExactPatternRow>(
                r#"
                SELECT DISTINCT ON (dv.full_name)
                    p.pattern AS phrase,
                    dv.full_name AS verb
                FROM "ob-poc".dsl_verbs dv
                CROSS JOIN LATERAL unnest(
                    COALESCE(dv.yaml_intent_patterns, '{}'::text[])
                    || COALESCE(dv.intent_patterns, '{}'::text[])
                ) AS p(pattern)
                WHERE LOWER(TRIM(p.pattern)) = $1
                ORDER BY dv.full_name
                LIMIT $2
                "#,
            )
            .bind(&normalized)
            .bind(limit as i32)
            .fetch_all(&self.pool)
            .await?
        };

        Ok(rows
            .into_iter()
            .map(|r| SemanticMatch {
                phrase: r.phrase,
                verb: r.verb,
                similarity: 1.0,
                confidence: None,
                category: Some("exact_pattern".to_string()),
            })
            .collect())
    }

    /// Look up a phrase in the governed phrase_bank (Tier 0, highest precedence).
    ///
    /// Workspace-qualified with precedence ordering:
    /// 1. Workspace-specific governed phrases
    /// 2. Workspace-specific legacy/yaml phrases
    /// 3. Global governed phrases
    /// 4. Global legacy/yaml phrases
    pub async fn find_phrase_bank_exact(
        &self,
        phrase: &str,
        workspace: Option<&str>,
        allowed_verbs: Option<&[String]>,
    ) -> Result<Option<SemanticMatch>, sqlx::Error> {
        let normalized = phrase.trim().to_lowercase();
        if normalized.is_empty() {
            return Ok(None);
        }

        let row = if let Some(allowed) = allowed_verbs {
            if allowed.is_empty() {
                return Ok(None);
            }
            sqlx::query_as::<_, ExactPatternRow>(
                r#"
                SELECT phrase, verb_fqn AS verb
                FROM "ob-poc".phrase_bank
                WHERE phrase = $1
                  AND active = TRUE
                  AND verb_fqn = ANY($3)
                  AND (workspace IS NULL OR workspace = $2)
                ORDER BY
                    CASE WHEN workspace IS NOT NULL THEN 0 ELSE 1 END,
                    CASE source
                        WHEN 'governed' THEN 0
                        WHEN 'legacy'   THEN 1
                        WHEN 'yaml'     THEN 2
                    END
                LIMIT 1
                "#,
            )
            .bind(&normalized)
            .bind(workspace)
            .bind(allowed)
            .fetch_optional(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, ExactPatternRow>(
                r#"
                SELECT phrase, verb_fqn AS verb
                FROM "ob-poc".phrase_bank
                WHERE phrase = $1
                  AND active = TRUE
                  AND (workspace IS NULL OR workspace = $2)
                ORDER BY
                    CASE WHEN workspace IS NOT NULL THEN 0 ELSE 1 END,
                    CASE source
                        WHEN 'governed' THEN 0
                        WHEN 'legacy'   THEN 1
                        WHEN 'yaml'     THEN 2
                    END
                LIMIT 1
                "#,
            )
            .bind(&normalized)
            .bind(workspace)
            .fetch_optional(&self.pool)
            .await?
        };

        Ok(row.map(|r| SemanticMatch {
            phrase: r.phrase,
            verb: r.verb,
            similarity: 1.0,
            confidence: None,
            category: Some("phrase_bank".to_string()),
        }))
    }

    /// Find global learned phrase by semantic similarity
    pub async fn find_global_learned_semantic(
        &self,
        query_embedding: &[f32],
        threshold: f32,
    ) -> Result<Option<SemanticMatch>, sqlx::Error> {
        let embedding_vec = Vector::from(query_embedding.to_vec());

        let row = sqlx::query_as::<_, GlobalLearnedSemanticRow>(
            r#"
            SELECT phrase, verb, 1 - (embedding <=> $1::vector) as similarity
            FROM "ob-poc".invocation_phrases
            WHERE embedding IS NOT NULL
            ORDER BY embedding <=> $1::vector
            LIMIT 1
            "#,
        )
        .bind(&embedding_vec)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(r) if r.similarity as f32 > threshold => Ok(Some(SemanticMatch {
                phrase: r.phrase,
                verb: r.verb,
                similarity: r.similarity,
                confidence: None,
                category: None,
            })),
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
        self.find_global_learned_semantic_topk_scoped(query_embedding, threshold, limit, None)
            .await
    }

    /// Domain-scoped variant of learned phrase semantic search.
    pub async fn find_global_learned_semantic_topk_scoped(
        &self,
        query_embedding: &[f32],
        threshold: f32,
        limit: usize,
        domain_prefix: Option<&str>,
    ) -> Result<Vec<SemanticMatch>, sqlx::Error> {
        let embedding_vec = Vector::from(query_embedding.to_vec());

        let rows = if let Some(domain) = domain_prefix {
            let domain_pattern = format!("{}.%", domain);
            sqlx::query_as::<_, GlobalLearnedSemanticRow>(
                r#"
                SELECT phrase, verb, 1 - (embedding <=> $1::vector) as similarity
                FROM "ob-poc".invocation_phrases
                WHERE embedding IS NOT NULL
                  AND 1 - (embedding <=> $1::vector) > $3
                  AND verb LIKE $4
                ORDER BY embedding <=> $1::vector
                LIMIT $2
                "#,
            )
            .bind(&embedding_vec)
            .bind(limit as i32)
            .bind(threshold)
            .bind(&domain_pattern)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, GlobalLearnedSemanticRow>(
                r#"
                SELECT phrase, verb, 1 - (embedding <=> $1::vector) as similarity
                FROM "ob-poc".invocation_phrases
                WHERE embedding IS NOT NULL
                  AND 1 - (embedding <=> $1::vector) > $3
                ORDER BY embedding <=> $1::vector
                LIMIT $2
                "#,
            )
            .bind(&embedding_vec)
            .bind(limit as i32)
            .bind(threshold)
            .fetch_all(&self.pool)
            .await?
        };

        Ok(rows
            .into_iter()
            .map(|r| SemanticMatch {
                phrase: r.phrase,
                verb: r.verb,
                similarity: r.similarity,
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
        self.search_verb_patterns_semantic_scoped(query_embedding, limit, min_similarity, None)
            .await
    }

    /// Search verb pattern embeddings with optional domain scoping.
    ///
    /// When `domain_prefix` is Some (e.g., "document"), the pgvector query
    /// is narrowed to `WHERE verb_name LIKE 'document.%'`. This reduces the
    /// search space from ~23K patterns to the domain's ~50-200 patterns,
    /// dramatically improving precision.
    pub async fn search_verb_patterns_semantic_scoped(
        &self,
        query_embedding: &[f32],
        limit: usize,
        min_similarity: f32,
        domain_prefix: Option<&str>,
    ) -> Result<Vec<SemanticMatch>, sqlx::Error> {
        tracing::debug!(
            embedding_len = query_embedding.len(),
            limit = limit,
            min_similarity = min_similarity,
            domain_prefix = ?domain_prefix,
            "VerbService: search_verb_patterns_semantic_scoped called"
        );

        let embedding_vec = Vector::from(query_embedding.to_vec());

        let rows = if let Some(domain) = domain_prefix {
            let domain_pattern = format!("{}.%", domain);
            sqlx::query_as::<_, VerbPatternSemanticRow>(
                r#"
                SELECT pattern_phrase, verb_name, 1 - (embedding <=> $1::vector) as similarity, category
                FROM "ob-poc".verb_pattern_embeddings
                WHERE embedding IS NOT NULL
                  AND 1 - (embedding <=> $1::vector) > $3
                  AND verb_name LIKE $4
                ORDER BY embedding <=> $1::vector
                LIMIT $2
                "#,
            )
            .bind(&embedding_vec)
            .bind(limit as i32)
            .bind(min_similarity)
            .bind(&domain_pattern)
            .fetch_all(&self.pool)
            .await
        } else {
            sqlx::query_as::<_, VerbPatternSemanticRow>(
                r#"
                SELECT pattern_phrase, verb_name, 1 - (embedding <=> $1::vector) as similarity, category
                FROM "ob-poc".verb_pattern_embeddings
                WHERE embedding IS NOT NULL
                  AND 1 - (embedding <=> $1::vector) > $3
                ORDER BY embedding <=> $1::vector
                LIMIT $2
                "#,
            )
            .bind(&embedding_vec)
            .bind(limit as i32)
            .bind(min_similarity)
            .fetch_all(&self.pool)
            .await
        };

        match &rows {
            Ok(r) => {
                tracing::debug!(row_count = r.len(), domain = ?domain_prefix, "VerbService: scoped query returned")
            }
            Err(e) => tracing::error!(error = %e, "VerbService: scoped query failed"),
        }

        let rows = rows?;

        Ok(rows
            .into_iter()
            .map(|r| SemanticMatch {
                phrase: r.pattern_phrase,
                verb: r.verb_name,
                similarity: r.similarity,
                confidence: None,
                category: r.category,
            })
            .collect())
    }

    /// Search verb pattern embeddings constrained to a specific set of verb FQNs.
    ///
    /// When the session has an `allowed_verbs` set (from SemOS SessionVerbSurface),
    /// this narrows the pgvector query from ~23K patterns to only patterns belonging
    /// to the allowed verbs (~200-600 patterns for 30-80 verbs). This is the key
    /// architectural change for SemOS-scoped resolution.
    pub async fn search_verb_patterns_semantic_constrained(
        &self,
        query_embedding: &[f32],
        limit: usize,
        min_similarity: f32,
        verb_fqns: &[String],
    ) -> Result<Vec<SemanticMatch>, sqlx::Error> {
        if verb_fqns.is_empty() {
            return Ok(Vec::new());
        }

        tracing::debug!(
            verb_count = verb_fqns.len(),
            limit = limit,
            min_similarity = min_similarity,
            "VerbService: constrained embedding search (verb_name = ANY)"
        );

        let embedding_vec = Vector::from(query_embedding.to_vec());

        let rows = sqlx::query_as::<_, VerbPatternSemanticRow>(
            r#"
            SELECT pattern_phrase, verb_name, 1 - (embedding <=> $1::vector) as similarity, category
            FROM "ob-poc".verb_pattern_embeddings
            WHERE embedding IS NOT NULL
              AND 1 - (embedding <=> $1::vector) > $3
              AND verb_name = ANY($4)
            ORDER BY embedding <=> $1::vector
            LIMIT $2
            "#,
        )
        .bind(&embedding_vec)
        .bind(limit as i32)
        .bind(min_similarity)
        .bind(verb_fqns)
        .fetch_all(&self.pool)
        .await;

        match &rows {
            Ok(r) => {
                tracing::debug!(
                    row_count = r.len(),
                    verb_whitelist_size = verb_fqns.len(),
                    "VerbService: constrained query returned"
                )
            }
            Err(e) => tracing::error!(error = %e, "VerbService: constrained query failed"),
        }

        let rows = rows?;

        Ok(rows
            .into_iter()
            .map(|r| SemanticMatch {
                phrase: r.pattern_phrase,
                verb: r.verb_name,
                similarity: r.similarity,
                confidence: None,
                category: r.category,
            })
            .collect())
    }

    /// Search learned phrases constrained to a specific set of verb FQNs.
    pub async fn find_global_learned_semantic_topk_constrained(
        &self,
        query_embedding: &[f32],
        threshold: f32,
        limit: usize,
        verb_fqns: &[String],
    ) -> Result<Vec<SemanticMatch>, sqlx::Error> {
        if verb_fqns.is_empty() {
            return Ok(Vec::new());
        }

        let embedding_vec = Vector::from(query_embedding.to_vec());

        let rows = sqlx::query_as::<_, GlobalLearnedSemanticRow>(
            r#"
            SELECT phrase, verb, 1 - (embedding <=> $1::vector) as similarity
            FROM "ob-poc".invocation_phrases
            WHERE embedding IS NOT NULL
              AND 1 - (embedding <=> $1::vector) > $3
              AND verb = ANY($4)
            ORDER BY embedding <=> $1::vector
            LIMIT $2
            "#,
        )
        .bind(&embedding_vec)
        .bind(limit as i32)
        .bind(threshold)
        .bind(verb_fqns)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| SemanticMatch {
                phrase: r.phrase,
                verb: r.verb,
                similarity: r.similarity,
                confidence: None,
                category: None,
            })
            .collect())
    }

    // ========================================================================
    // Blocklist
    // ========================================================================

    /// Check if a verb is blocked for the given query
    ///
    /// Uses time-based decay: negative signals decay at ~50% after 2 weeks.
    /// Decay formula: weight * 0.95^days_old (from TODO spec)
    /// Default base weight is 1.0, threshold is compared against decayed similarity.
    pub async fn check_blocklist(
        &self,
        query_embedding: &[f32],
        user_id: Option<Uuid>,
        verb: &str,
        threshold: f32,
    ) -> Result<bool, sqlx::Error> {
        let embedding_vec = Vector::from(query_embedding.to_vec());

        // Decay-aware blocklist check:
        // - Compute days_old from created_at
        // - Apply decay: base_weight * 0.95^days_old
        // - Match is blocked if decayed_similarity > threshold
        //
        // For example:
        //   Day 0:  1.0 * 0.95^0  = 1.0   (full weight)
        //   Day 7:  1.0 * 0.95^7  = 0.70  (70% weight)
        //   Day 14: 1.0 * 0.95^14 = 0.49  (~50% weight)
        //   Day 30: 1.0 * 0.95^30 = 0.21  (21% weight)
        let blocked = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS (
                SELECT 1 FROM "ob-poc".phrase_blocklist
                WHERE blocked_verb = $1
                  AND (user_id IS NULL OR user_id = $2)
                  AND (expires_at IS NULL OR expires_at > now())
                  AND embedding IS NOT NULL
                  AND (
                    -- Compute decayed similarity: similarity * decay_factor
                    -- decay_factor = 0.95^days_old
                    (1 - (embedding <=> $3::vector)) *
                    POWER(0.95, GREATEST(0, EXTRACT(EPOCH FROM (now() - created_at)) / 86400))
                  ) > $4
            )
            "#,
        )
        .bind(verb)
        .bind(user_id)
        .bind(&embedding_vec)
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
        let row = sqlx::query_scalar::<_, Option<String>>(
            r#"
            SELECT description
            FROM "ob-poc".dsl_verbs
            WHERE full_name = $1
            "#,
        )
        .bind(full_name)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.flatten())
    }

    /// Get multiple verb descriptions at once
    pub async fn get_verb_descriptions(
        &self,
        full_names: &[String],
    ) -> Result<Vec<VerbDescription>, sqlx::Error> {
        if full_names.is_empty() {
            return Ok(Vec::new());
        }

        sqlx::query_as::<_, VerbDescription>(
            r#"
            SELECT full_name, description
            FROM "ob-poc".dsl_verbs
            WHERE full_name = ANY($1)
            "#,
        )
        .bind(full_names)
        .fetch_all(&self.pool)
        .await
    }

    // ========================================================================
    // Verb Centroids (Two-Stage Semantic Search)
    // ========================================================================

    /// Query verb centroids for semantic shortlist
    ///
    /// Returns top-K verbs by centroid similarity.
    /// Use this to get a candidate set, then refine with pattern-level matches.
    pub async fn query_centroids(
        &self,
        query_embedding: &[f32],
        limit: i32,
    ) -> Result<Vec<VerbCentroidMatch>, sqlx::Error> {
        let embedding_vec = Vector::from(query_embedding.to_vec());

        let rows = sqlx::query_as::<_, VerbCentroidRow>(
            r#"
            SELECT
                verb_name,
                1 - (embedding <=> $1::vector) as similarity,
                phrase_count
            FROM "ob-poc".verb_centroids
            ORDER BY embedding <=> $1::vector
            LIMIT $2
            "#,
        )
        .bind(&embedding_vec)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| VerbCentroidMatch {
                verb_name: r.verb_name,
                score: r.similarity,
                phrase_count: r.phrase_count,
            })
            .collect())
    }

    /// Query verb centroids with minimum score threshold
    pub async fn query_centroids_with_threshold(
        &self,
        query_embedding: &[f32],
        limit: i32,
        min_score: f32,
    ) -> Result<Vec<VerbCentroidMatch>, sqlx::Error> {
        let embedding_vec = Vector::from(query_embedding.to_vec());

        let rows = sqlx::query_as::<_, VerbCentroidRow>(
            r#"
            SELECT
                verb_name,
                1 - (embedding <=> $1::vector) as similarity,
                phrase_count
            FROM "ob-poc".verb_centroids
            WHERE 1 - (embedding <=> $1::vector) >= $3
            ORDER BY embedding <=> $1::vector
            LIMIT $2
            "#,
        )
        .bind(&embedding_vec)
        .bind(limit)
        .bind(min_score)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| VerbCentroidMatch {
                verb_name: r.verb_name,
                score: r.similarity,
                phrase_count: r.phrase_count,
            })
            .collect())
    }

    /// Search patterns only for specific verbs (for centroid refinement)
    ///
    /// After getting a centroid shortlist, use this to get pattern-level
    /// evidence for only the shortlisted verbs.
    pub async fn search_patterns_for_verbs(
        &self,
        query_embedding: &[f32],
        verb_names: &[&str],
        limit: i32,
    ) -> Result<Vec<SemanticMatch>, sqlx::Error> {
        if verb_names.is_empty() {
            return Ok(Vec::new());
        }

        let embedding_vec = Vector::from(query_embedding.to_vec());
        let verb_names_owned: Vec<String> = verb_names.iter().map(|s| s.to_string()).collect();

        let rows = sqlx::query_as::<_, VerbPatternSemanticRow>(
            r#"
            SELECT
                pattern_phrase,
                verb_name,
                1 - (embedding <=> $1::vector) as similarity,
                category
            FROM "ob-poc".verb_pattern_embeddings
            WHERE verb_name = ANY($2)
              AND embedding IS NOT NULL
            ORDER BY embedding <=> $1::vector
            LIMIT $3
            "#,
        )
        .bind(&embedding_vec)
        .bind(&verb_names_owned)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| SemanticMatch {
                phrase: r.pattern_phrase,
                verb: r.verb_name,
                similarity: r.similarity,
                confidence: None,
                category: r.category,
            })
            .collect())
    }

    /// Search verb patterns by phonetic (dmetaphone) matching
    ///
    /// This is used as a fallback when semantic search returns low confidence,
    /// particularly for handling typos like "allainz" → "allianz".
    ///
    /// The search generates phonetic codes for each word in the query and
    /// finds patterns that contain ANY of those codes.
    pub async fn search_by_phonetic(
        &self,
        query: &str,
        limit: i64,
    ) -> Result<Vec<PhoneticMatch>, sqlx::Error> {
        // Generate phonetic codes for the query words
        // We'll do this in SQL to use the same dmetaphone function
        let rows = sqlx::query_as::<_, PhoneticMatchRow>(
            r#"
            WITH query_phonetics AS (
                -- Generate phonetic codes for each word in the query
                SELECT DISTINCT dmetaphone(word) as code
                FROM unnest(string_to_array(lower($1), ' ')) as word
                WHERE length(word) >= 2
                  AND dmetaphone(word) IS NOT NULL
                  AND dmetaphone(word) != ''
            ),
            matching_patterns AS (
                -- Find patterns that have ANY matching phonetic code
                SELECT DISTINCT ON (vpe.verb_name)
                    vpe.verb_name,
                    vpe.pattern_phrase,
                    vpe.phonetic_codes,
                    -- Count how many query phonetics match
                    (SELECT COUNT(*)
                     FROM query_phonetics qp
                     WHERE qp.code = ANY(vpe.phonetic_codes)) as match_count,
                    -- Total query phonetics for scoring
                    (SELECT COUNT(*) FROM query_phonetics) as total_query_codes
                FROM "ob-poc".verb_pattern_embeddings vpe
                WHERE EXISTS (
                    SELECT 1 FROM query_phonetics qp
                    WHERE qp.code = ANY(vpe.phonetic_codes)
                )
            )
            SELECT
                verb_name,
                pattern_phrase,
                phonetic_codes,
                match_count,
                total_query_codes,
                -- Score: proportion of query phonetics that matched
                CASE
                    WHEN total_query_codes > 0
                    THEN match_count::float / total_query_codes::float
                    ELSE 0.0
                END as phonetic_score
            FROM matching_patterns
            WHERE match_count > 0
            ORDER BY match_count DESC, verb_name
            LIMIT $2
            "#,
        )
        .bind(query)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| PhoneticMatch {
                verb: r.verb_name,
                pattern: r.pattern_phrase,
                phonetic_codes: r.phonetic_codes,
                match_count: r.match_count,
                phonetic_score: r.phonetic_score,
            })
            .collect())
    }
}

/// Row struct for phonetic match queries
#[derive(Debug, sqlx::FromRow)]
struct PhoneticMatchRow {
    verb_name: String,
    pattern_phrase: String,
    phonetic_codes: Vec<String>,
    match_count: i64,
    #[allow(dead_code)]
    total_query_codes: i64,
    phonetic_score: f64,
}

/// A phonetic match result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhoneticMatch {
    pub verb: String,
    pub pattern: String,
    pub phonetic_codes: Vec<String>,
    pub match_count: i64,
    pub phonetic_score: f64,
}

#[cfg(test)]
mod tests {
    // Integration tests would go here
}
