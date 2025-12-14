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

    /// Create an empty store without database (for offline validation)
    #[cfg(feature = "database")]
    pub fn new_empty() -> Self {
        Self {
            pool: sqlx::PgPool::connect_lazy("postgresql://invalid").unwrap(),
            initialized: false,
        }
    }

    #[cfg(not(feature = "database"))]
    pub fn new() -> Self {
        Self { initialized: false }
    }

    #[cfg(not(feature = "database"))]
    pub fn new_empty() -> Self {
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
        _source_type: &str,
        _source_code: &str,
        _target_type: &str,
        _min_similarity: f64,
    ) -> Result<Vec<(String, f64)>, String> {
        // csg_semantic_similarity_cache table was removed in schema cleanup
        // Semantic similarity is now computed on-demand using embeddings
        Ok(vec![])
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires tokio runtime even for lazy pool creation"]
    fn test_new_empty_store() {
        let store = SemanticContextStore::new_empty();
        assert!(!store.is_initialized());
    }

    #[tokio::test]
    #[ignore = "requires valid database connection"]
    async fn test_initialize_empty_store() {
        let mut store = SemanticContextStore::new_empty();
        // Initialize should succeed even without DB
        let result = store.initialize().await;
        assert!(result.is_ok());
        assert!(store.is_initialized());
    }

    #[tokio::test]
    async fn test_find_similar_documents_uninitialized() {
        let store = SemanticContextStore::new_empty();
        // Should return empty vec when not initialized
        let result = store.find_similar_documents("proper_person", 5).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_get_cached_similarities() {
        let store = SemanticContextStore::new_empty();
        // Should return empty vec (cache table removed)
        let result = store
            .get_cached_similarities("entity", "proper_person", "document", 0.5)
            .await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }
}
