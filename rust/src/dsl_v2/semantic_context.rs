//! Semantic Context Store
//!
//! Provides vector-based semantic similarity for enhanced suggestions.
//! Leverages the existing rag_embeddings infrastructure.

#[cfg(feature = "database")]
use sqlx::PgPool;

/// Store for semantic context and vector operations
pub struct SemanticContextStore {
    #[cfg(feature = "database")]
    pool: PgPool,
    initialized: bool,
}

impl SemanticContextStore {
    #[cfg(feature = "database")]
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            initialized: false,
        }
    }

    #[cfg(not(feature = "database"))]
    pub fn new() -> Self {
        Self { initialized: false }
    }

    #[cfg(feature = "database")]
    pub async fn initialize(&mut self) -> Result<(), String> {
        // Verify vector extension is available
        let count: Option<i64> = sqlx::query_scalar!(
            r#"SELECT COUNT(*)::bigint FROM pg_extension WHERE extname = 'vector'"#
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| format!("pgvector check failed: {}", e))?;

        if count.unwrap_or(0) == 0 {
            // Vector extension not installed - not an error, just limited functionality
            tracing::warn!("pgvector extension not installed, similarity features disabled");
        }

        self.initialized = true;
        Ok(())
    }

    #[cfg(not(feature = "database"))]
    pub async fn initialize(&mut self) -> Result<(), String> {
        self.initialized = true;
        Ok(())
    }

    /// Find semantically similar document types using vector embeddings
    #[cfg(feature = "database")]
    pub async fn find_similar_documents(
        &self,
        entity_type: &str,
        limit: usize,
    ) -> Result<Vec<String>, String> {
        if !self.initialized {
            return Ok(vec![]);
        }

        // Query documents similar to the entity type's typical documents
        let results = sqlx::query_scalar!(
            r#"
            SELECT dt.type_code
            FROM "ob-poc".document_types dt
            WHERE dt.embedding IS NOT NULL
              AND EXISTS (
                  SELECT 1 FROM "ob-poc".entity_types et
                  WHERE et.type_code = $1 AND et.embedding IS NOT NULL
              )
            ORDER BY dt.embedding <=> (
                SELECT et.embedding FROM "ob-poc".entity_types et
                WHERE et.type_code = $1
                LIMIT 1
            )
            LIMIT $2
            "#,
            entity_type,
            limit as i64
        )
        .fetch_all(&self.pool)
        .await
        .unwrap_or_default();

        Ok(results)
    }

    #[cfg(not(feature = "database"))]
    pub async fn find_similar_documents(
        &self,
        _entity_type: &str,
        _limit: usize,
    ) -> Result<Vec<String>, String> {
        Ok(vec![])
    }

    /// Get pre-computed similarity scores from cache
    #[cfg(feature = "database")]
    pub async fn get_cached_similarities(
        &self,
        source_type: &str,
        source_code: &str,
        target_type: &str,
        min_similarity: f64,
    ) -> Result<Vec<(String, f64)>, String> {
        let results = sqlx::query!(
            r#"
            SELECT target_code, cosine_similarity
            FROM "ob-poc".csg_semantic_similarity_cache
            WHERE source_type = $1
              AND source_code = $2
              AND target_type = $3
              AND cosine_similarity >= $4
              AND expires_at > NOW()
            ORDER BY cosine_similarity DESC
            "#,
            source_type,
            source_code,
            target_type,
            min_similarity
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| format!("Failed to fetch similarities: {}", e))?;

        Ok(results
            .into_iter()
            .map(|r| (r.target_code, r.cosine_similarity))
            .collect())
    }

    #[cfg(not(feature = "database"))]
    pub async fn get_cached_similarities(
        &self,
        _source_type: &str,
        _source_code: &str,
        _target_type: &str,
        _min_similarity: f64,
    ) -> Result<Vec<(String, f64)>, String> {
        Ok(vec![])
    }

    /// Check if store is initialized
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }
}

#[cfg(not(feature = "database"))]
impl Default for SemanticContextStore {
    fn default() -> Self {
        Self::new()
    }
}
