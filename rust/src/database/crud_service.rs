//! CRUD Service - Agentic CRUD operation logging
//!
//! This module provides database operations for logging all CRUD operations
//! for full agentic auditability and traceability.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::{FromRow, PgPool};
use tracing::info;
use uuid::Uuid;

/// Types of assets that can be operated on
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AssetType {
    Cbu,
    ProperPerson,
    Company,
    Trust,
    Partnership,
    Entity,
    Attribute,
    Document,
}

impl std::fmt::Display for AssetType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AssetType::Cbu => write!(f, "CBU"),
            AssetType::ProperPerson => write!(f, "PROPER_PERSON"),
            AssetType::Company => write!(f, "COMPANY"),
            AssetType::Trust => write!(f, "TRUST"),
            AssetType::Partnership => write!(f, "PARTNERSHIP"),
            AssetType::Entity => write!(f, "ENTITY"),
            AssetType::Attribute => write!(f, "ATTRIBUTE"),
            AssetType::Document => write!(f, "DOCUMENT"),
        }
    }
}

/// Types of CRUD operations
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum OperationType {
    Create,
    Read,
    Update,
    Delete,
}

impl std::fmt::Display for OperationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OperationType::Create => write!(f, "CREATE"),
            OperationType::Read => write!(f, "READ"),
            OperationType::Update => write!(f, "UPDATE"),
            OperationType::Delete => write!(f, "DELETE"),
        }
    }
}

/// CRUD operation record
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CrudOperation {
    pub operation_id: Uuid,
    pub operation_type: String,
    pub asset_type: String,
    pub entity_table_name: String,
    pub generated_dsl: Option<String>,
    pub ai_instruction: Option<String>,
    pub affected_records: Option<JsonValue>,
    pub affected_sinks: Option<JsonValue>,
    pub contributing_sources: Option<JsonValue>,
    pub execution_status: String,
    pub ai_confidence: Option<f64>,
    pub ai_provider: Option<String>,
    pub ai_model: Option<String>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Service for CRUD operation logging
#[derive(Clone, Debug)]
pub struct CrudService {
    pool: PgPool,
}

impl CrudService {
    /// Create a new CRUD service
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get reference to the connection pool
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Log a CRUD operation
    pub async fn log_crud_operation(
        &self,
        operation_type: OperationType,
        asset_type: AssetType,
        entity_table_name: &str,
        generated_dsl: Option<&str>,
        ai_instruction: Option<&str>,
        affected_records: Option<JsonValue>,
        ai_provider: Option<&str>,
        ai_model: Option<&str>,
    ) -> Result<Uuid> {
        let operation_id = Uuid::new_v4();

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".crud_operations (
                operation_id, operation_type, asset_type, entity_table_name,
                generated_dsl, ai_instruction, affected_records,
                execution_status, ai_provider, ai_model, created_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, 'COMPLETED', $8, $9, NOW())
            "#,
        )
        .bind(operation_id)
        .bind(operation_type.to_string())
        .bind(asset_type.to_string())
        .bind(entity_table_name)
        .bind(generated_dsl)
        .bind(ai_instruction)
        .bind(affected_records)
        .bind(ai_provider)
        .bind(ai_model)
        .execute(&self.pool)
        .await
        .context("Failed to log CRUD operation")?;

        info!(
            "Logged {} {} operation {} on {}",
            operation_type, asset_type, operation_id, entity_table_name
        );

        Ok(operation_id)
    }

    /// Log a CRUD operation with sink/source tracking
    pub async fn log_crud_operation_with_sinks(
        &self,
        operation_type: OperationType,
        asset_type: AssetType,
        entity_table_name: &str,
        generated_dsl: Option<&str>,
        ai_instruction: Option<&str>,
        affected_records: Option<JsonValue>,
        affected_sinks: Option<Vec<Uuid>>,
        contributing_sources: Option<Vec<Uuid>>,
        ai_provider: Option<&str>,
        ai_model: Option<&str>,
    ) -> Result<Uuid> {
        let operation_id = Uuid::new_v4();

        let sinks_json = affected_sinks.map(|s| serde_json::to_value(s).unwrap_or_default());
        let sources_json =
            contributing_sources.map(|s| serde_json::to_value(s).unwrap_or_default());

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".crud_operations (
                operation_id, operation_type, asset_type, entity_table_name,
                generated_dsl, ai_instruction, affected_records,
                affected_sinks, contributing_sources,
                execution_status, ai_provider, ai_model, created_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'COMPLETED', $10, $11, NOW())
            "#,
        )
        .bind(operation_id)
        .bind(operation_type.to_string())
        .bind(asset_type.to_string())
        .bind(entity_table_name)
        .bind(generated_dsl)
        .bind(ai_instruction)
        .bind(affected_records)
        .bind(sinks_json)
        .bind(sources_json)
        .bind(ai_provider)
        .bind(ai_model)
        .execute(&self.pool)
        .await
        .context("Failed to log CRUD operation with sinks")?;

        info!(
            "Logged {} {} operation {} on {} (with sink/source tracking)",
            operation_type, asset_type, operation_id, entity_table_name
        );

        Ok(operation_id)
    }

    /// Get CRUD operations for an entity
    pub async fn get_operations_for_entity(
        &self,
        entity_table_name: &str,
        limit: Option<i32>,
    ) -> Result<Vec<CrudOperation>> {
        let results = sqlx::query_as::<_, CrudOperation>(
            r#"
            SELECT operation_id, operation_type, asset_type, entity_table_name,
                   generated_dsl, ai_instruction, affected_records,
                   affected_sinks, contributing_sources,
                   execution_status, ai_confidence, ai_provider, ai_model, created_at
            FROM "ob-poc".crud_operations
            WHERE entity_table_name = $1
            ORDER BY created_at DESC
            LIMIT $2
            "#,
        )
        .bind(entity_table_name)
        .bind(limit.unwrap_or(100))
        .fetch_all(&self.pool)
        .await
        .context("Failed to get CRUD operations for entity")?;

        Ok(results)
    }

    /// Get CRUD operations by asset type
    pub async fn get_operations_by_asset_type(
        &self,
        asset_type: AssetType,
        limit: Option<i32>,
    ) -> Result<Vec<CrudOperation>> {
        let results = sqlx::query_as::<_, CrudOperation>(
            r#"
            SELECT operation_id, operation_type, asset_type, entity_table_name,
                   generated_dsl, ai_instruction, affected_records,
                   affected_sinks, contributing_sources,
                   execution_status, ai_confidence, ai_provider, ai_model, created_at
            FROM "ob-poc".crud_operations
            WHERE asset_type = $1
            ORDER BY created_at DESC
            LIMIT $2
            "#,
        )
        .bind(asset_type.to_string())
        .bind(limit.unwrap_or(100))
        .fetch_all(&self.pool)
        .await
        .context("Failed to get CRUD operations by asset type")?;

        Ok(results)
    }

    /// Get recent CRUD operations
    pub async fn get_recent_operations(&self, limit: Option<i32>) -> Result<Vec<CrudOperation>> {
        let results = sqlx::query_as::<_, CrudOperation>(
            r#"
            SELECT operation_id, operation_type, asset_type, entity_table_name,
                   generated_dsl, ai_instruction, affected_records,
                   affected_sinks, contributing_sources,
                   execution_status, ai_confidence, ai_provider, ai_model, created_at
            FROM "ob-poc".crud_operations
            ORDER BY created_at DESC
            LIMIT $1
            "#,
        )
        .bind(limit.unwrap_or(50))
        .fetch_all(&self.pool)
        .await
        .context("Failed to get recent CRUD operations")?;

        Ok(results)
    }

    /// Get CRUD operation by ID
    pub async fn get_operation_by_id(&self, operation_id: Uuid) -> Result<Option<CrudOperation>> {
        let result = sqlx::query_as::<_, CrudOperation>(
            r#"
            SELECT operation_id, operation_type, asset_type, entity_table_name,
                   generated_dsl, ai_instruction, affected_records,
                   affected_sinks, contributing_sources,
                   execution_status, ai_confidence, ai_provider, ai_model, created_at
            FROM "ob-poc".crud_operations
            WHERE operation_id = $1
            "#,
        )
        .bind(operation_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get CRUD operation by ID")?;

        Ok(result)
    }
}
