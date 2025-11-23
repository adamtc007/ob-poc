//! Vocabulary Provider - Query vocabulary registry for valid verbs
#![allow(dead_code)]

use sqlx::PgPool;
use anyhow::Result;

/// Provides vocabulary data for DSL generation
pub struct VocabularyProvider {
    pool: PgPool,
}

impl VocabularyProvider {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
    
    /// Get all active verbs for a domain
    pub async fn get_verbs_for_domain(&self, domain: &str) -> Result<Vec<String>> {
        let verbs = sqlx::query_scalar::<_, String>(
            r#"
            SELECT verb_name 
            FROM "ob-poc".vocabulary_registry 
            WHERE domain = $1 AND is_active = true
            ORDER BY usage_count DESC
            "#
        )
        .bind(domain)
        .fetch_all(&self.pool)
        .await?;
        
        Ok(verbs)
    }
    
    /// Check if a verb exists
    pub async fn verb_exists(&self, verb_name: &str) -> Result<bool> {
        let exists = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM "ob-poc".vocabulary_registry 
                WHERE verb_name = $1 AND is_active = true
            )
            "#
        )
        .bind(verb_name)
        .fetch_one(&self.pool)
        .await?;
        
        Ok(exists)
    }
}
