//! Type-Safe Attribute Repository
//!
//! This module provides a type-safe repository for storing and retrieving
//! attribute values with compile-time type checking and runtime validation.

use crate::domains::attributes::types::AttributeType;
use bigdecimal::BigDecimal;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Type-safe repository for attribute operations
#[derive(Clone)]
pub struct AttributeRepository {
    pool: Arc<PgPool>,
    cache: Arc<RwLock<HashMap<String, CachedValue>>>,
}

/// Cached attribute value with TTL
#[derive(Clone, Debug)]
struct CachedValue {
    value: serde_json::Value,
    cached_at: std::time::Instant,
}

impl CachedValue {
    fn new(value: serde_json::Value) -> Self {
        Self {
            value,
            cached_at: std::time::Instant::now(),
        }
    }

    fn is_expired(&self, ttl_secs: u64) -> bool {
        self.cached_at.elapsed().as_secs() > ttl_secs
    }
}

/// Repository errors
#[derive(Debug, thiserror::Error)]
pub enum RepositoryError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Attribute not found: {0}")]
    NotFound(String),

    #[error("Type mismatch: expected {expected}, got {actual}")]
    TypeMismatch { expected: String, actual: String },
}

pub type Result<T> = std::result::Result<T, RepositoryError>;

impl AttributeRepository {
    /// Create a new repository with a database pool
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool: Arc::new(pool),
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get the current value of an attribute for an entity
    pub async fn get<T: AttributeType>(&self, entity_id: Uuid) -> Result<Option<T::Value>> {
        // Check cache first
        let cache_key = format!("{}:{}", entity_id, T::ID);

        {
            let cache = self.cache.read().await;
            if let Some(cached) = cache.get(&cache_key) {
                if !cached.is_expired(300) {
                    // 5 minute TTL
                    return Ok(Some(serde_json::from_value(cached.value.clone())?));
                }
            }
        }

        // Query database
        let row = sqlx::query!(
            r#"
            SELECT value_text, value_number, value_integer, value_boolean,
                   value_date, value_datetime, value_json
            FROM "ob-poc".attribute_values_typed
            WHERE entity_id = $1 AND attribute_id = $2
            AND effective_to IS NULL
            ORDER BY effective_from DESC
            LIMIT 1
            "#,
            entity_id,
            T::ID
        )
        .fetch_optional(&*self.pool)
        .await?;

        match row {
            Some(r) => {
                let value = Self::row_to_typed_value::<T>(
                    r.value_text,
                    r.value_number,
                    r.value_integer,
                    r.value_boolean,
                    r.value_date,
                    r.value_datetime,
                    r.value_json,
                )?;

                // Update cache
                let json_value = serde_json::to_value(&value)?;
                let mut cache = self.cache.write().await;
                cache.insert(cache_key, CachedValue::new(json_value));

                Ok(Some(value))
            }
            None => Ok(None),
        }
    }

    /// Set the value of an attribute for an entity
    pub async fn set<T: AttributeType>(
        &self,
        entity_id: Uuid,
        value: T::Value,
        created_by: Option<&str>,
    ) -> Result<i64> {
        // Validate the value
        T::validate(&value).map_err(|e| RepositoryError::Validation(e.to_string()))?;

        // Serialize based on data type
        let (text, number, integer, boolean, date, datetime, json) =
            Self::serialize_value::<T>(&value)?;

        // Use the database function to set the value
        let result = sqlx::query_scalar!(
            r#"
            SELECT "ob-poc".set_attribute_value(
                $1::UUID,
                $2::TEXT,
                $3::TEXT,
                $4::NUMERIC,
                $5::BIGINT,
                $6::BOOLEAN,
                $7::DATE,
                $8::TIMESTAMPTZ,
                $9::JSONB,
                $10::TEXT
            ) as "id!"
            "#,
            entity_id,
            T::ID,
            text,
            number,
            integer,
            boolean,
            date,
            datetime,
            json,
            created_by.unwrap_or("system")
        )
        .fetch_one(&*self.pool)
        .await?;

        // Invalidate cache
        let cache_key = format!("{}:{}", entity_id, T::ID);
        let mut cache = self.cache.write().await;
        cache.remove(&cache_key);

        Ok(result)
    }

    /// Get multiple attributes for an entity at once
    pub async fn get_many(
        &self,
        entity_id: Uuid,
        attribute_ids: &[&str],
    ) -> Result<HashMap<String, serde_json::Value>> {
        let attribute_ids_vec: Vec<String> = attribute_ids.iter().map(|s| s.to_string()).collect();
        let rows = sqlx::query!(
            r#"
            SELECT attribute_id, value_text, value_number, value_integer,
                   value_boolean, value_date, value_datetime, value_json
            FROM "ob-poc".attribute_values_typed
            WHERE entity_id = $1 AND attribute_id = ANY($2)
            AND effective_to IS NULL
            "#,
            entity_id,
            &attribute_ids_vec[..]
        )
        .fetch_all(&*self.pool)
        .await?;

        let mut result = HashMap::new();
        for row in rows {
            let value = Self::row_to_json_value(
                row.value_text,
                row.value_number,
                row.value_integer,
                row.value_boolean,
                row.value_json,
            )?;
            result.insert(row.attribute_id, value);
        }

        Ok(result)
    }

    /// Set multiple attributes in a transaction
    pub async fn set_many_transactional<'a>(
        &self,
        entity_id: Uuid,
        attributes: Vec<(&'a str, serde_json::Value)>,
        created_by: Option<&str>,
    ) -> Result<Vec<i64>> {
        let mut tx = self.pool.begin().await?;
        let mut ids = Vec::new();

        for (attr_id, value) in &attributes {
            let (text, number, integer, boolean, date, datetime, json) =
                Self::serialize_json_value(value)?;

            let id = sqlx::query_scalar!(
                r#"
                SELECT "ob-poc".set_attribute_value(
                    $1::UUID, $2::TEXT, $3::TEXT, $4::NUMERIC, $5::BIGINT,
                    $6::BOOLEAN, $7::DATE, $8::TIMESTAMPTZ, $9::JSONB, $10::TEXT
                ) as "id!"
                "#,
                entity_id,
                *attr_id,
                text,
                number,
                integer,
                boolean,
                date,
                datetime,
                json,
                created_by.unwrap_or("system")
            )
            .fetch_one(&mut *tx)
            .await?;

            ids.push(id);
        }

        tx.commit().await?;

        // Invalidate cache for all affected attributes
        let mut cache = self.cache.write().await;
        for (attr_id, _) in &attributes {
            let cache_key = format!("{}:{}", entity_id, attr_id);
            cache.remove(&cache_key);
        }

        Ok(ids)
    }

    /// Get attribute history
    pub async fn get_history<T: AttributeType>(
        &self,
        entity_id: Uuid,
        limit: i64,
    ) -> Result<Vec<AttributeHistoryEntry<T::Value>>> {
        let rows = sqlx::query!(
            r#"
            SELECT id, value_text, value_number, value_integer, value_boolean,
                   value_date, value_datetime, value_json,
                   effective_from, effective_to, created_by
            FROM "ob-poc".attribute_values_typed
            WHERE entity_id = $1 AND attribute_id = $2
            ORDER BY effective_from DESC
            LIMIT $3
            "#,
            entity_id,
            T::ID,
            limit
        )
        .fetch_all(&*self.pool)
        .await?;

        let mut history = Vec::new();
        for row in rows {
            let value = Self::row_to_typed_value::<T>(
                row.value_text,
                row.value_number,
                row.value_integer,
                row.value_boolean,
                row.value_date,
                row.value_datetime,
                row.value_json,
            )?;
            history.push(AttributeHistoryEntry {
                id: row.id,
                value,
                effective_from: row.effective_from.unwrap(),
                effective_to: row.effective_to,
                created_by: row.created_by.unwrap_or_else(|| "unknown".to_string()),
            });
        }

        Ok(history)
    }

    /// Clear the cache
    pub async fn clear_cache(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
    }

    /// Get cache statistics
    pub async fn cache_stats(&self) -> CacheStats {
        let cache = self.cache.read().await;
        CacheStats {
            entries: cache.len(),
            expired: cache.values().filter(|v| v.is_expired(300)).count(),
        }
    }

    // Helper methods

    fn row_to_typed_value<T: AttributeType>(
        value_text: Option<String>,
        value_number: Option<BigDecimal>,
        value_integer: Option<i64>,
        value_boolean: Option<bool>,
        value_date: Option<chrono::NaiveDate>,
        value_datetime: Option<chrono::DateTime<chrono::Utc>>,
        value_json: Option<serde_json::Value>,
    ) -> Result<T::Value> {
        // Convert database row to JSON value first
        let value_json: Option<serde_json::Value> = value_text
            .map(serde_json::Value::String)
            .or_else(|| {
                value_number.map(|d| serde_json::json!(d.to_string().parse::<f64>().unwrap_or(0.0)))
            })
            .or_else(|| value_integer.map(|i| serde_json::json!(i)))
            .or_else(|| value_boolean.map(serde_json::Value::Bool))
            .or_else(|| value_date.map(|d| serde_json::Value::String(d.to_string())))
            .or_else(|| value_datetime.map(|dt| serde_json::Value::String(dt.to_rfc3339())))
            .or_else(|| value_json);

        match value_json {
            Some(json) => Ok(serde_json::from_value(json)?),
            None => Err(RepositoryError::NotFound(T::ID.to_string())),
        }
    }

    fn serialize_value<T: AttributeType>(
        value: &T::Value,
    ) -> Result<(
        Option<String>,
        Option<BigDecimal>,
        Option<i64>,
        Option<bool>,
        Option<chrono::NaiveDate>,
        Option<chrono::DateTime<chrono::Utc>>,
        Option<serde_json::Value>,
    )> {
        let json = serde_json::to_value(value)?;
        Self::serialize_json_value(&json)
    }

    fn serialize_json_value(
        json: &serde_json::Value,
    ) -> Result<(
        Option<String>,
        Option<BigDecimal>,
        Option<i64>,
        Option<bool>,
        Option<chrono::NaiveDate>,
        Option<chrono::DateTime<chrono::Utc>>,
        Option<serde_json::Value>,
    )> {
        use std::str::FromStr;

        Ok(match json {
            serde_json::Value::String(s) => (Some(s.clone()), None, None, None, None, None, None),
            serde_json::Value::Number(n) if n.is_i64() => (
                None,
                None,
                Some(n.as_i64().unwrap()),
                None,
                None,
                None,
                None,
            ),
            serde_json::Value::Number(n) => (
                None,
                Some(BigDecimal::from_str(&n.to_string()).unwrap_or_else(|_| BigDecimal::from(0))),
                None,
                None,
                None,
                None,
                None,
            ),
            serde_json::Value::Bool(b) => (None, None, None, Some(*b), None, None, None),
            _ => (None, None, None, None, None, None, Some(json.clone())),
        })
    }

    fn row_to_json_value(
        value_text: Option<String>,
        value_number: Option<BigDecimal>,
        value_integer: Option<i64>,
        value_boolean: Option<bool>,
        value_json: Option<serde_json::Value>,
    ) -> Result<serde_json::Value> {
        if let Some(v) = value_text {
            return Ok(serde_json::Value::String(v));
        }
        if let Some(v) = value_number {
            return Ok(serde_json::json!(v
                .to_string()
                .parse::<f64>()
                .unwrap_or(0.0)));
        }
        if let Some(v) = value_integer {
            return Ok(serde_json::json!(v));
        }
        if let Some(v) = value_boolean {
            return Ok(serde_json::Value::Bool(v));
        }
        if let Some(v) = value_json {
            return Ok(v);
        }

        Err(RepositoryError::NotFound("No value found".to_string()))
    }
}

/// History entry for an attribute
#[derive(Debug, Clone)]
pub struct AttributeHistoryEntry<T> {
    pub id: i32,
    pub value: T,
    pub effective_from: chrono::DateTime<chrono::Utc>,
    pub effective_to: Option<chrono::DateTime<chrono::Utc>>,
    pub created_by: String,
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub entries: usize,
    pub expired: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    // Tests will be added once we have a test database setup
    // For now, these are placeholder tests that demonstrate the API

    #[test]
    fn test_cache_value_expiry() {
        let value = CachedValue::new(serde_json::json!("test"));
        assert!(!value.is_expired(300));
    }

    #[test]
    fn test_repository_error_display() {
        let err = RepositoryError::NotFound("attr.test".to_string());
        assert_eq!(err.to_string(), "Attribute not found: attr.test");
    }
}
