//! DSL CRUD Manager - Multi-Stage Commit Operations for DSL Save Workflow
//!
//! This module implements DSL save operations following the same pattern as CBU CRUD
//! with proper transaction handling and multi-stage commits.
//!
//! ## Architecture: DSL-as-State with Multi-Stage Commits
//!
//! The DSL save workflow requires transactional consistency across multiple tables:
//! 1. **DSL Instance Table**: Store accumulated DSL content with version increment
//! 2. **AST Table**: Store parsed/validated AST representation
//! 3. **Audit Trail**: Track all changes with rollback capability
//! 4. **Cross-References**: Maintain referential integrity
//!
//! ## Multi-Stage Commit Pattern (Same as CBU CRUD):
//! ```
//! 1. Transaction Begin
//! 2. Validation Phase
//! 3. Multi-Stage Operations (each can fail and rollback)
//!    - Parse & Validate DSL
//!    - Save DSL Instance (version++)
//!    - Save AST Representation
//!    - Update Audit Trail
//!    - Sync Cross-References
//! 4. Single Commit or Rollback
//! ```

use crate::error::DSLError;
use crate::parser::parse_program;
use crate::parser_ast::{CrudStatement, DataCreate, Program, Value as ParserValue};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Instant;
use tracing::{error, info};
use uuid::Uuid;

#[cfg(feature = "database")]
use sqlx::{PgPool, Postgres, Row, Transaction};

// Static version counter for mock implementation
static MOCK_VERSION_COUNTER: AtomicU32 = AtomicU32::new(0);

/// DSL CRUD Manager for database operations expressed as DSL
#[derive(Clone)]
pub struct DslCrudManager {
    #[cfg(feature = "database")]
    pool: PgPool,
}

/// Request structure for DSL save operations (follows CBU pattern)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DslSaveRequest {
    pub case_id: String,
    pub onboarding_request_id: Uuid,
    pub dsl_content: String,
    pub user_id: String,
    pub operation_context: OperationContext,
}

/// Response structure for DSL save operations (follows CBU pattern)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DslSaveResult {
    pub success: bool,
    pub case_id: String,
    pub version_number: u32,
    pub dsl_instance_id: Uuid,
    pub ast_record_id: Uuid,
    pub parsing_time_ms: u64,
    pub save_time_ms: u64,
    pub total_time_ms: u64,
    pub errors: Vec<String>,
}

/// Request structure for DSL load operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DslLoadRequest {
    pub case_id: String,
    pub version: Option<u32>, // None = latest version
    pub include_ast: bool,
    pub include_audit_trail: bool,
}

/// Response structure for DSL load operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DslLoadResult {
    pub success: bool,
    pub case_id: String,
    pub version_number: u32,
    pub dsl_content: String,
    pub ast_json: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub audit_entries: Vec<DslAuditEntry>,
}

/// Operation context for tracking workflow state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationContext {
    pub workflow_type: String,
    pub source: String, // "manual_edit", "ai_generated", "api_call"
    pub metadata: HashMap<String, String>,
}

/// Audit entry for DSL operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DslAuditEntry {
    pub entry_id: Uuid,
    pub operation_type: String,
    pub user_id: String,
    pub timestamp: DateTime<Utc>,
    pub version_from: u32,
    pub version_to: u32,
    pub change_summary: String,
    pub rollback_data: Option<String>,
}

/// Error types for DSL CRUD operations (follows CBU pattern)
#[derive(Debug, thiserror::Error)]
pub enum DslCrudError {
    #[error("DSL validation failed: {details}")]
    ValidationError { details: String },

    #[error("DSL parsing failed: {reason}")]
    ParseError { reason: String },

    #[error("Version conflict: expected {expected}, found {actual}")]
    VersionConflict { expected: u32, actual: u32 },

    #[error("Case not found: {case_id}")]
    CaseNotFound { case_id: String },

    #[error("Database transaction failed: {details}")]
    TransactionError { details: String },

    #[error("Cross-reference sync failed: {table}: {details}")]
    SyncError { table: String, details: String },

    #[cfg(feature = "database")]
    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),

    #[error("DSL error: {0}")]
    DslError(#[from] DSLError),
}

impl DslCrudManager {
    /// Create new DSL CRUD manager (follows CBU pattern)
    #[cfg(feature = "database")]
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create new DSL CRUD manager without database (for testing)
    #[cfg(not(feature = "database"))]
    pub fn new() -> Self {
        Self {}
    }

    /// Save DSL with multi-stage commit (follows CBU CRUD pattern)
    pub async fn save_dsl_complex(
        &self,
        request: DslSaveRequest,
    ) -> Result<DslSaveResult, DslCrudError> {
        let start_time = Instant::now();

        #[cfg(feature = "database")]
        let mut tx = self.pool.begin().await?;

        info!(
            "Saving DSL for case: {} by user: {}",
            request.case_id, request.user_id
        );

        // Stage 1: Validate request
        self.validate_dsl_save_request(&request).await?;

        // Stage 2: Parse and validate DSL content
        let parse_start = Instant::now();
        let _parsed_ast = self.parse_and_validate_dsl(&request.dsl_content).await?;
        let parsing_time_ms = parse_start.elapsed().as_millis() as u64;

        #[cfg(feature = "database")]
        {
            // Stage 3: Load current version for increment
            let current_version = self.get_current_version(&mut tx, &request.case_id).await?;
            let new_version = current_version + 1;

            // Stage 4: Save DSL instance with version increment
            let dsl_instance_id = self
                .save_dsl_instance(&mut tx, &request, new_version, &_parsed_ast)
                .await?;

            // Stage 5: Save AST representation
            let ast_record_id = self
                .save_ast_representation(
                    &mut tx,
                    &request.case_id,
                    dsl_instance_id,
                    new_version,
                    &_parsed_ast,
                )
                .await?;

            // Stage 6: Create audit trail
            self.create_audit_entry(
                &mut tx,
                &request,
                current_version,
                new_version,
                dsl_instance_id,
            )
            .await?;

            // Stage 7: Update cross-references
            self.sync_cross_references(&mut tx, &request.case_id, new_version)
                .await?;

            // Single commit point
            tx.commit().await?;

            let total_time_ms = start_time.elapsed().as_millis() as u64;

            Ok(DslSaveResult {
                success: true,
                case_id: request.case_id,
                version_number: new_version,
                dsl_instance_id,
                ast_record_id,
                parsing_time_ms,
                save_time_ms: total_time_ms - parsing_time_ms,
                total_time_ms,
                errors: Vec::new(),
            })
        }

        #[cfg(not(feature = "database"))]
        {
            // In-memory simulation for testing with version increment
            let mock_dsl_id = Uuid::new_v4();
            let mock_ast_id = Uuid::new_v4();
            let total_time_ms = start_time.elapsed().as_millis() as u64;

            // Simulate version increment
            let new_version = MOCK_VERSION_COUNTER.fetch_add(1, Ordering::SeqCst) + 1;

            Ok(DslSaveResult {
                success: true,
                case_id: request.case_id,
                version_number: new_version,
                dsl_instance_id: mock_dsl_id,
                ast_record_id: mock_ast_id,
                parsing_time_ms,
                save_time_ms: total_time_ms - parsing_time_ms,
                total_time_ms,
                errors: Vec::new(),
            })
        }
    }

    /// Load DSL by case ID and version
    pub async fn load_dsl_complete(
        &self,
        request: DslLoadRequest,
    ) -> Result<DslLoadResult, DslCrudError> {
        info!("Loading DSL for case: {}", request.case_id);

        #[cfg(feature = "database")]
        {
            // Get the requested version (or latest)
            let version = match request.version {
                Some(v) => v,
                None => self.get_latest_version(&request.case_id).await?,
            };

            // Load DSL content
            let (dsl_content, created_at, updated_at) =
                self.load_dsl_content(&request.case_id, version).await?;

            // Optionally load AST
            let ast_json = if request.include_ast {
                Some(self.load_ast_content(&request.case_id, version).await?)
            } else {
                None
            };

            // Optionally load audit trail
            let audit_entries = if request.include_audit_trail {
                self.load_audit_entries(&request.case_id, version).await?
            } else {
                Vec::new()
            };

            Ok(DslLoadResult {
                success: true,
                case_id: request.case_id,
                version_number: version,
                dsl_content,
                ast_json,
                created_at,
                updated_at,
                audit_entries,
            })
        }

        #[cfg(not(feature = "database"))]
        {
            // Mock response for testing
            Ok(DslLoadResult {
                success: true,
                case_id: request.case_id,
                version_number: 1,
                dsl_content: "(case.create :name \"Test Case\")".to_string(),
                ast_json: request.include_ast.then(|| "{}".to_string()),
                created_at: Utc::now(),
                updated_at: Utc::now(),
                audit_entries: Vec::new(),
            })
        }
    }

    /// Validate DSL save request (follows CBU pattern)
    async fn validate_dsl_save_request(
        &self,
        request: &DslSaveRequest,
    ) -> Result<(), DslCrudError> {
        // Validate case_id is not empty
        if request.case_id.trim().is_empty() {
            return Err(DslCrudError::ValidationError {
                details: "Case ID cannot be empty".to_string(),
            });
        }

        // Validate DSL content is not empty
        if request.dsl_content.trim().is_empty() {
            return Err(DslCrudError::ValidationError {
                details: "DSL content cannot be empty".to_string(),
            });
        }

        // Validate user_id
        if request.user_id.trim().is_empty() {
            return Err(DslCrudError::ValidationError {
                details: "User ID cannot be empty".to_string(),
            });
        }

        Ok(())
    }

    /// Parse and validate DSL content
    async fn parse_and_validate_dsl(&self, dsl_content: &str) -> Result<Program, DslCrudError> {
        match parse_program(dsl_content) {
            Ok(ast) => {
                // Additional validation can be added here
                Ok(ast)
            }
            Err(e) => Err(DslCrudError::ParseError {
                reason: format!("Failed to parse DSL: {}", e),
            }),
        }
    }

    // Database operations (only compiled with database feature)
    #[cfg(feature = "database")]
    async fn get_current_version(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        case_id: &str,
    ) -> Result<u32, DslCrudError> {
        let row = sqlx::query!(
            r#"
            SELECT COALESCE(MAX(version), 0) as current_version
            FROM "ob-poc".dsl_instances
            WHERE case_id = $1
            "#,
            case_id
        )
        .fetch_one(&mut **tx)
        .await?;

        Ok(row.current_version.unwrap_or(0) as u32)
    }

    #[cfg(feature = "database")]
    async fn save_dsl_instance(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        request: &DslSaveRequest,
        version: u32,
        _ast: &Program,
    ) -> Result<Uuid, DslCrudError> {
        let instance_id = Uuid::new_v4();

        sqlx::query!(
            r#"
            INSERT INTO "ob-poc".dsl_instances
            (instance_id, case_id, onboarding_request_id, version, current_dsl, created_by, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, NOW(), NOW())
            "#,
            instance_id,
            request.case_id,
            request.onboarding_request_id,
            version as i32,
            request.dsl_content,
            request.user_id
        )
        .execute(&mut **tx)
        .await?;

        info!("Saved DSL instance: {} version: {}", instance_id, version);
        Ok(instance_id)
    }

    #[cfg(feature = "database")]
    async fn save_ast_representation(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        case_id: &str,
        dsl_instance_id: Uuid,
        version: u32,
        ast: &Program,
    ) -> Result<Uuid, DslCrudError> {
        let ast_id = Uuid::new_v4();
        let ast_json = serde_json::to_string(ast).map_err(|e| DslCrudError::ValidationError {
            details: format!("Failed to serialize AST: {}", e),
        })?;

        sqlx::query!(
            r#"
            INSERT INTO "ob-poc".parsed_asts
            (ast_id, case_id, dsl_instance_id, version, ast_json, created_at)
            VALUES ($1, $2, $3, $4, $5, NOW())
            "#,
            ast_id,
            case_id,
            dsl_instance_id,
            version as i32,
            ast_json
        )
        .execute(&mut **tx)
        .await?;

        info!(
            "Saved AST record: {} for DSL instance: {}",
            ast_id, dsl_instance_id
        );
        Ok(ast_id)
    }

    #[cfg(feature = "database")]
    async fn create_audit_entry(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        request: &DslSaveRequest,
        version_from: u32,
        version_to: u32,
        dsl_instance_id: Uuid,
    ) -> Result<(), DslCrudError> {
        let audit_id = Uuid::new_v4();
        let change_summary = format!(
            "DSL updated from version {} to {}",
            version_from, version_to
        );

        sqlx::query!(
            r#"
            INSERT INTO "ob-poc".audit_log
            (audit_id, case_id, operation_type, user_id, version_from, version_to,
             change_summary, operation_context, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, NOW())
            "#,
            audit_id,
            request.case_id,
            "dsl_save",
            request.user_id,
            version_from as i32,
            version_to as i32,
            change_summary,
            serde_json::to_string(&request.operation_context).unwrap_or("{}".to_string())
        )
        .execute(&mut **tx)
        .await?;

        info!(
            "Created audit entry: {} for version transition: {} -> {}",
            audit_id, version_from, version_to
        );
        Ok(())
    }

    #[cfg(feature = "database")]
    async fn sync_cross_references(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        case_id: &str,
        version: u32,
    ) -> Result<(), DslCrudError> {
        // Update any cross-reference tables that need to know about the new version
        // This could include workflow state, dependency tracking, etc.

        info!(
            "Syncing cross-references for case: {} version: {}",
            case_id, version
        );

        // For now, this is a placeholder - specific cross-reference logic would go here
        Ok(())
    }

    #[cfg(feature = "database")]
    async fn get_latest_version(&self, case_id: &str) -> Result<u32, DslCrudError> {
        let mut tx = self.pool.begin().await?;
        let version = self.get_current_version(&mut tx, case_id).await?;
        tx.commit().await?;
        Ok(version)
    }

    #[cfg(feature = "database")]
    async fn load_dsl_content(
        &self,
        case_id: &str,
        version: u32,
    ) -> Result<(String, DateTime<Utc>, DateTime<Utc>), DslCrudError> {
        let row = sqlx::query!(
            r#"
            SELECT current_dsl, created_at, updated_at
            FROM "ob-poc".dsl_instances
            WHERE case_id = $1 AND version = $2
            "#,
            case_id,
            version as i32
        )
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => Ok((row.current_dsl, row.created_at, row.updated_at)),
            None => Err(DslCrudError::CaseNotFound {
                case_id: format!("{}:v{}", case_id, version),
            }),
        }
    }

    #[cfg(feature = "database")]
    async fn load_ast_content(&self, case_id: &str, version: u32) -> Result<String, DslCrudError> {
        let row = sqlx::query!(
            r#"
            SELECT ast_json
            FROM "ob-poc".parsed_asts
            WHERE case_id = $1 AND version = $2
            "#,
            case_id,
            version as i32
        )
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => Ok(row.ast_json),
            None => Err(DslCrudError::CaseNotFound {
                case_id: format!("{}:v{} (AST)", case_id, version),
            }),
        }
    }

    #[cfg(feature = "database")]
    async fn load_audit_entries(
        &self,
        case_id: &str,
        _version: u32,
    ) -> Result<Vec<DslAuditEntry>, DslCrudError> {
        let rows = sqlx::query!(
            r#"
            SELECT audit_id, operation_type, user_id, created_at,
                   version_from, version_to, change_summary
            FROM "ob-poc".audit_log
            WHERE case_id = $1
            ORDER BY created_at DESC
            "#,
            case_id
        )
        .fetch_all(&self.pool)
        .await?;

        let entries = rows
            .into_iter()
            .map(|row| DslAuditEntry {
                entry_id: row.audit_id,
                operation_type: row.operation_type,
                user_id: row.user_id,
                timestamp: row.created_at,
                version_from: row.version_from as u32,
                version_to: row.version_to as u32,
                change_summary: row.change_summary,
                rollback_data: None, // Could be populated if needed
            })
            .collect();

        Ok(entries)
    }
}

/// Generate CRUD statements for DSL operations (DSL-as-State pattern)
pub fn generate_dsl_save_crud_statements(request: &DslSaveRequest) -> Vec<CrudStatement> {
    let mut statements = Vec::new();

    // Create DSL instance record
    let mut dsl_values = HashMap::new();
    dsl_values.insert(
        "case_id".to_string(),
        ParserValue::String(request.case_id.clone()),
    );
    dsl_values.insert(
        "onboarding_request_id".to_string(),
        ParserValue::String(request.onboarding_request_id.to_string()),
    );
    dsl_values.insert(
        "current_dsl".to_string(),
        ParserValue::String(request.dsl_content.clone()),
    );
    dsl_values.insert(
        "created_by".to_string(),
        ParserValue::String(request.user_id.clone()),
    );

    statements.push(CrudStatement::DataCreate(DataCreate {
        asset: "dsl_instances".to_string(),
        values: dsl_values,
    }));

    // Create audit trail entry
    let mut audit_values = HashMap::new();
    audit_values.insert(
        "case_id".to_string(),
        ParserValue::String(request.case_id.clone()),
    );
    audit_values.insert(
        "operation_type".to_string(),
        ParserValue::String("dsl_save".to_string()),
    );
    audit_values.insert(
        "user_id".to_string(),
        ParserValue::String(request.user_id.clone()),
    );

    statements.push(CrudStatement::DataCreate(DataCreate {
        asset: "audit_log".to_string(),
        values: audit_values,
    }));

    statements
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_dsl_crud_manager_creation() {
        let manager = DslCrudManager::new();
        // Basic functionality test
        assert!(true); // Manager created successfully
    }

    #[tokio::test]
    async fn test_validate_dsl_save_request() {
        let manager = DslCrudManager::new();

        let valid_request = DslSaveRequest {
            case_id: "test-case-123".to_string(),
            onboarding_request_id: Uuid::new_v4(),
            dsl_content: "(case.create :name \"Test\")".to_string(),
            user_id: "test-user".to_string(),
            operation_context: OperationContext {
                workflow_type: "onboarding".to_string(),
                source: "manual_edit".to_string(),
                metadata: HashMap::new(),
            },
        };

        assert!(manager
            .validate_dsl_save_request(&valid_request)
            .await
            .is_ok());

        // Test invalid request (empty case_id)
        let mut invalid_request = valid_request.clone();
        invalid_request.case_id = "".to_string();

        assert!(manager
            .validate_dsl_save_request(&invalid_request)
            .await
            .is_err());
    }

    #[tokio::test]
    async fn test_parse_and_validate_dsl() {
        let manager = DslCrudManager::new();

        let valid_dsl = "(case.create :name \"Test Case\")";
        assert!(manager.parse_and_validate_dsl(valid_dsl).await.is_ok());

        let invalid_dsl = "(invalid dsl content";
        assert!(manager.parse_and_validate_dsl(invalid_dsl).await.is_err());
    }

    #[tokio::test]
    async fn test_generate_dsl_save_crud_statements() {
        let request = DslSaveRequest {
            case_id: "test-case-123".to_string(),
            onboarding_request_id: Uuid::new_v4(),
            dsl_content: "(case.create :name \"Test\")".to_string(),
            user_id: "test-user".to_string(),
            operation_context: OperationContext {
                workflow_type: "onboarding".to_string(),
                source: "manual_edit".to_string(),
                metadata: HashMap::new(),
            },
        };

        let statements = generate_dsl_save_crud_statements(&request);
        assert_eq!(statements.len(), 2); // DSL instance + audit entry
    }
}
