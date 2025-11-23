//! Attribute Provider - Query dictionary for attribute definitions
#![allow(dead_code)]

use sqlx::PgPool;
use anyhow::Result;
use uuid::Uuid;

/// Provides attribute data for DSL generation
pub struct AttributeProvider {
    pool: PgPool,
}

impl AttributeProvider {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
    
    /// Get attribute by semantic ID
    pub async fn get_by_semantic_id(&self, semantic_id: &str) -> Result<Option<Uuid>> {
        let attr_id = sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT attribute_id 
            FROM "ob-poc".dictionary 
            WHERE semantic_id = $1
            "#
        )
        .bind(semantic_id)
        .fetch_optional(&self.pool)
        .await?;
        
        Ok(attr_id)
    }
    
    /// Get attributes for a domain
    pub async fn get_for_domain(&self, domain: &str) -> Result<Vec<(Uuid, String)>> {
        let attrs = sqlx::query_as::<_, (Uuid, String)>(
            r#"
            SELECT attribute_id, semantic_id 
            FROM "ob-poc".dictionary 
            WHERE business_domain = $1
            ORDER BY name
            "#
        )
        .bind(domain)
        .fetch_all(&self.pool)
        .await?;
        
        Ok(attrs)
    }
}
