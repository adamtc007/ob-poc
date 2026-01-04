//! Verb Hash Lookup Service
//!
//! Provides efficient lookup of verb compiled_hash values for execution tracking.
//! Used by the executor to record which verb configurations were used for each execution.

use std::collections::HashMap;

#[cfg(feature = "database")]
use anyhow::Result;
#[cfg(feature = "database")]
use sqlx::PgPool;

/// Cached verb hash for a specific verb
#[derive(Debug, Clone)]
pub struct VerbHashInfo {
    pub verb_name: String,
    pub compiled_hash: Option<Vec<u8>>,
}

/// Service for looking up verb hashes from the database
#[cfg(feature = "database")]
pub struct VerbHashLookupService {
    pool: PgPool,
    /// Cache of verb hashes (verb_name -> hash)
    cache: tokio::sync::RwLock<HashMap<String, Option<Vec<u8>>>>,
}

#[cfg(feature = "database")]
impl VerbHashLookupService {
    /// Create a new lookup service
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            cache: tokio::sync::RwLock::new(HashMap::new()),
        }
    }

    /// Get the compiled_hash for a verb, with caching
    pub async fn get_verb_hash(&self, verb_name: &str) -> Result<Option<Vec<u8>>> {
        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some(hash) = cache.get(verb_name) {
                return Ok(hash.clone());
            }
        }

        // Query database
        let row: Option<(Option<Vec<u8>>,)> =
            sqlx::query_as(r#"SELECT compiled_hash FROM "ob-poc".dsl_verbs WHERE verb_name = $1"#)
                .bind(verb_name)
                .fetch_optional(&self.pool)
                .await?;

        let hash = row.and_then(|(h,)| h);

        // Update cache
        {
            let mut cache = self.cache.write().await;
            cache.insert(verb_name.to_string(), hash.clone());
        }

        Ok(hash)
    }

    /// Get compiled_hashes for multiple verbs in a single query
    pub async fn get_verb_hashes(
        &self,
        verb_names: &[String],
    ) -> Result<HashMap<String, Option<Vec<u8>>>> {
        if verb_names.is_empty() {
            return Ok(HashMap::new());
        }

        // Check which verbs are already cached
        let mut result = HashMap::new();
        let mut uncached = Vec::new();

        {
            let cache = self.cache.read().await;
            for name in verb_names {
                if let Some(hash) = cache.get(name) {
                    result.insert(name.clone(), hash.clone());
                } else {
                    uncached.push(name.clone());
                }
            }
        }

        if uncached.is_empty() {
            return Ok(result);
        }

        // Query database for uncached verbs
        let rows: Vec<(String, Option<Vec<u8>>)> = sqlx::query_as(
            r#"SELECT verb_name, compiled_hash FROM "ob-poc".dsl_verbs WHERE verb_name = ANY($1)"#,
        )
        .bind(&uncached)
        .fetch_all(&self.pool)
        .await?;

        // Update cache and result
        {
            let mut cache = self.cache.write().await;
            for (name, hash) in rows {
                cache.insert(name.clone(), hash.clone());
                result.insert(name, hash);
            }
            // Also cache NULL for verbs not found
            for name in &uncached {
                if !result.contains_key(name) {
                    cache.insert(name.clone(), None);
                    result.insert(name.clone(), None);
                }
            }
        }

        Ok(result)
    }

    /// Invalidate the cache (call after verb sync)
    pub async fn invalidate_cache(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
    }

    /// Pre-warm the cache with all verbs
    pub async fn warm_cache(&self) -> Result<usize> {
        let rows: Vec<(String, Option<Vec<u8>>)> =
            sqlx::query_as(r#"SELECT verb_name, compiled_hash FROM "ob-poc".dsl_verbs"#)
                .fetch_all(&self.pool)
                .await?;

        let count = rows.len();
        let mut cache = self.cache.write().await;
        for (name, hash) in rows {
            cache.insert(name, hash);
        }

        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verb_hash_info() {
        let info = VerbHashInfo {
            verb_name: "cbu.ensure".to_string(),
            compiled_hash: Some(vec![1, 2, 3, 4]),
        };
        assert_eq!(info.verb_name, "cbu.ensure");
        assert!(info.compiled_hash.is_some());
    }
}
