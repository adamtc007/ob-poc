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

// =============================================================================
// PARAMETER STRUCTS
// =============================================================================

/// Parameters for logging a CRUD operation
#[derive(Debug, Clone)]
pub struct CrudOperationLog {
    /// Type of operation (Create, Read, Update, Delete)
    pub operation_type: OperationType,
    /// Type of asset being operated on
    pub asset_type: AssetType,
    /// Database table name
    pub entity_table_name: String,
    /// DSL that was generated/executed
    pub generated_dsl: Option<String>,
    /// Original AI instruction that triggered this
    pub ai_instruction: Option<String>,
    /// Records affected by this operation
    pub affected_records: Option<JsonValue>,
    /// AI context (provider and model used)
    pub ai_context: Option<AiContext>,
}

/// AI provider context for audit trail
#[derive(Debug, Clone, Default)]
pub struct AiContext {
    pub provider: String,
    pub model: String,
}

// =============================================================================
// SERVICE
// =============================================================================

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
    pub async fn log_crud_operation(&self, log: CrudOperationLog) -> Result<Uuid> {
        let operation_id = Uuid::new_v4();

        let (ai_provider, ai_model) = log
            .ai_context
            .as_ref()
            .map(|ctx| (Some(ctx.provider.as_str()), Some(ctx.model.as_str())))
            .unwrap_or((None, None));

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
        .bind(log.operation_type.to_string())
        .bind(log.asset_type.to_string())
        .bind(&log.entity_table_name)
        .bind(&log.generated_dsl)
        .bind(&log.ai_instruction)
        .bind(&log.affected_records)
        .bind(ai_provider)
        .bind(ai_model)
        .execute(&self.pool)
        .await
        .context("Failed to log CRUD operation")?;

        info!(
            "Logged {} {} operation {} on {}",
            log.operation_type, log.asset_type, operation_id, log.entity_table_name
        );

        Ok(operation_id)
    }
}
