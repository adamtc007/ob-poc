//! Verb Hash Lookup Service
//!
//! Provides efficient lookup of verb compiled_hash values for execution tracking.
//! Used by the executor to record which verb configurations were used for each execution.

use std::collections::HashMap;

#[cfg(feature = "database")]
use anyhow::Result;
#[cfg(feature = "database")]
use sqlx::PgPool;

/// Service for looking up verb hashes from the database
#[cfg(feature = "database")]
pub(crate) struct VerbHashLookupService {
    pool: PgPool,
    /// Cache of verb hashes (verb_name -> hash)
    cache: tokio::sync::RwLock<HashMap<String, Option<Vec<u8>>>>,
}

#[cfg(feature = "database")]
impl VerbHashLookupService {
    /// Create a new lookup service
    pub(crate) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            cache: tokio::sync::RwLock::new(HashMap::new()),
        }
    }

    /// Get the compiled_hash for a verb, with caching
    pub(crate) async fn get_verb_hash(&self, verb_name: &str) -> Result<Option<Vec<u8>>> {
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
}
