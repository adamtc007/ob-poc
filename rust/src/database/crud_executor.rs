//! CRUD Executor - Executes CrudStatements against the database
//!
//! This module provides the execution layer that takes CrudStatements
//! from the Forth engine and performs actual database operations.

use crate::parser::ast::{
    CrudStatement, DataCreate, DataDelete, DataRead, DataUpdate, Literal, Value,
};
use anyhow::{anyhow, Result};
use serde_json::Value as JsonValue;
use sqlx::PgPool;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Result of executing a CRUD statement
#[derive(Debug, Clone)]
pub struct CrudExecutionResult {
    /// Type of operation executed
    pub operation: String,
    /// Asset/table affected
    pub asset: String,
    /// Number of rows affected
    pub rows_affected: u64,
    /// Generated ID (for creates)
    pub generated_id: Option<Uuid>,
    /// Retrieved data (for reads)
    pub data: Option<JsonValue>,
}

/// Executor for CRUD statements
pub struct CrudExecutor {
    pool: PgPool,
}

impl CrudExecutor {
    /// Create a new CRUD executor
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Execute a single CRUD statement
    pub async fn execute(&self, stmt: &CrudStatement) -> Result<CrudExecutionResult> {
        match stmt {
            CrudStatement::DataCreate(create) => self.execute_create(create).await,
            CrudStatement::DataRead(read) => self.execute_read(read).await,
            CrudStatement::DataUpdate(update) => self.execute_update(update).await,
            CrudStatement::DataDelete(delete) => self.execute_delete(delete).await,
            _ => Err(anyhow!("Unsupported CRUD statement type")),
        }
    }

    /// Execute multiple CRUD statements in a transaction
    pub async fn execute_all(&self, stmts: &[CrudStatement]) -> Result<Vec<CrudExecutionResult>> {
        let mut results = Vec::new();

        // Start transaction
        let mut tx = self.pool.begin().await?;

        for stmt in stmts {
            let result = match stmt {
                CrudStatement::DataCreate(create) => {
                    self.execute_create_tx(create, &mut tx).await?
                }
                CrudStatement::DataUpdate(update) => {
                    self.execute_update_tx(update, &mut tx).await?
                }
                CrudStatement::DataDelete(delete) => {
                    self.execute_delete_tx(delete, &mut tx).await?
                }
                CrudStatement::DataRead(read) => {
                    // Reads don't need transaction
                    self.execute_read(read).await?
                }
                _ => return Err(anyhow!("Unsupported CRUD statement type")),
            };
            results.push(result);
        }

        // Commit transaction
        tx.commit().await?;

        Ok(results)
    }

    /// Execute a CREATE statement
    async fn execute_create(&self, create: &DataCreate) -> Result<CrudExecutionResult> {
        let mut tx = self.pool.begin().await?;
        let result = self.execute_create_tx(create, &mut tx).await?;
        tx.commit().await?;
        Ok(result)
    }

    /// Execute CREATE within a transaction
    async fn execute_create_tx(
        &self,
        create: &DataCreate,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    ) -> Result<CrudExecutionResult> {
        let generated_id = Uuid::new_v4();

        match create.asset.as_str() {
            "CBU" => {
                let client_name = self
                    .get_string_value(&create.values, "cbu-name")
                    .or_else(|| self.get_string_value(&create.values, "client-name"))
                    .unwrap_or_else(|| "Unknown".to_string());
                let client_type = self
                    .get_string_value(&create.values, "client-type")
                    .unwrap_or_else(|| "CORP".to_string());
                let jurisdiction = self
                    .get_string_value(&create.values, "jurisdiction")
                    .unwrap_or_else(|| "US".to_string());

                sqlx::query(
                    r#"
                    INSERT INTO "ob-poc".cbus (cbu_id, client_name, client_type, jurisdiction, status, created_at, updated_at)
                    VALUES ($1, $2, $3, $4, 'ACTIVE', NOW(), NOW())
                    "#
                )
                .bind(generated_id.to_string())
                .bind(&client_name)
                .bind(&client_type)
                .bind(&jurisdiction)
                .execute(&mut **tx)
                .await?;

                info!("Created CBU: {} ({})", client_name, generated_id);

                Ok(CrudExecutionResult {
                    operation: "CREATE".to_string(),
                    asset: "CBU".to_string(),
                    rows_affected: 1,
                    generated_id: Some(generated_id),
                    data: None,
                })
            }
            "CBU_ENTITY_RELATIONSHIP" => {
                let entity_id = self
                    .get_string_value(&create.values, "entity-id")
                    .unwrap_or_else(|| Uuid::new_v4().to_string());
                let role = self
                    .get_string_value(&create.values, "role")
                    .unwrap_or_else(|| "UNKNOWN".to_string());

                // For now, store in entities table with role
                sqlx::query(
                    r#"
                    INSERT INTO "ob-poc".entities (entity_id, entity_type, legal_name, jurisdiction, status, created_at, updated_at)
                    VALUES ($1::uuid, $2, $3, 'US', 'ACTIVE', NOW(), NOW())
                    ON CONFLICT (entity_id) DO UPDATE SET updated_at = NOW()
                    "#
                )
                .bind(&entity_id)
                .bind(&role)
                .bind(format!("Entity-{}", &entity_id[..8.min(entity_id.len())]))
                .execute(&mut **tx)
                .await?;

                info!("Created entity relationship: {} as {}", entity_id, role);

                Ok(CrudExecutionResult {
                    operation: "CREATE".to_string(),
                    asset: "CBU_ENTITY_RELATIONSHIP".to_string(),
                    rows_affected: 1,
                    generated_id: Some(generated_id),
                    data: None,
                })
            }
            "CBU_PROPER_PERSON" => {
                let person_name = self
                    .get_string_value(&create.values, "person-name")
                    .unwrap_or_else(|| "Unknown Person".to_string());
                let role = self
                    .get_string_value(&create.values, "role")
                    .unwrap_or_else(|| "CONTACT".to_string());

                sqlx::query(
                    r#"
                    INSERT INTO "ob-poc".entities (entity_id, entity_type, legal_name, jurisdiction, status, created_at, updated_at)
                    VALUES ($1::uuid, 'PROPER_PERSON', $2, 'US', 'ACTIVE', NOW(), NOW())
                    "#
                )
                .bind(generated_id)
                .bind(&person_name)
                .execute(&mut **tx)
                .await?;

                info!("Created proper person: {} as {}", person_name, role);

                Ok(CrudExecutionResult {
                    operation: "CREATE".to_string(),
                    asset: "CBU_PROPER_PERSON".to_string(),
                    rows_affected: 1,
                    generated_id: Some(generated_id),
                    data: None,
                })
            }
            _ => {
                warn!("Unknown asset type for CREATE: {}", create.asset);
                Ok(CrudExecutionResult {
                    operation: "CREATE".to_string(),
                    asset: create.asset.clone(),
                    rows_affected: 0,
                    generated_id: None,
                    data: None,
                })
            }
        }
    }

    /// Execute a READ statement
    async fn execute_read(&self, read: &DataRead) -> Result<CrudExecutionResult> {
        match read.asset.as_str() {
            "CBU" => {
                let cbu_id = self.get_string_value(&read.where_clause, "cbu-id");

                let rows = if let Some(id) = cbu_id {
                    sqlx::query_as::<_, (String, String, String, String, String)>(
                        r#"
                        SELECT cbu_id, client_name, client_type, jurisdiction, status
                        FROM "ob-poc".cbus
                        WHERE cbu_id = $1
                        "#,
                    )
                    .bind(&id)
                    .fetch_all(&self.pool)
                    .await?
                } else {
                    let limit = read.limit.unwrap_or(100);
                    sqlx::query_as::<_, (String, String, String, String, String)>(
                        r#"
                        SELECT cbu_id, client_name, client_type, jurisdiction, status
                        FROM "ob-poc".cbus
                        ORDER BY created_at DESC
                        LIMIT $1
                        "#,
                    )
                    .bind(limit as i64)
                    .fetch_all(&self.pool)
                    .await?
                };

                let data: Vec<JsonValue> = rows
                    .into_iter()
                    .map(|(id, name, ctype, jurisdiction, status)| {
                        serde_json::json!({
                            "cbu_id": id,
                            "client_name": name,
                            "client_type": ctype,
                            "jurisdiction": jurisdiction,
                            "status": status
                        })
                    })
                    .collect();

                Ok(CrudExecutionResult {
                    operation: "READ".to_string(),
                    asset: "CBU".to_string(),
                    rows_affected: data.len() as u64,
                    generated_id: None,
                    data: Some(JsonValue::Array(data)),
                })
            }
            _ => {
                warn!("Unknown asset type for READ: {}", read.asset);
                Ok(CrudExecutionResult {
                    operation: "READ".to_string(),
                    asset: read.asset.clone(),
                    rows_affected: 0,
                    generated_id: None,
                    data: Some(JsonValue::Array(vec![])),
                })
            }
        }
    }

    /// Execute an UPDATE statement
    async fn execute_update(&self, update: &DataUpdate) -> Result<CrudExecutionResult> {
        let mut tx = self.pool.begin().await?;
        let result = self.execute_update_tx(update, &mut tx).await?;
        tx.commit().await?;
        Ok(result)
    }

    /// Execute UPDATE within a transaction
    async fn execute_update_tx(
        &self,
        update: &DataUpdate,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    ) -> Result<CrudExecutionResult> {
        match update.asset.as_str() {
            "CBU" => {
                let cbu_id = self
                    .get_string_value(&update.where_clause, "cbu-id")
                    .ok_or_else(|| anyhow!("cbu-id required for UPDATE"))?;

                // Build dynamic update
                let status = self.get_string_value(&update.values, "status");
                let client_type = self.get_string_value(&update.values, "client-type");

                let result = if let Some(status) = status {
                    sqlx::query(
                        r#"
                        UPDATE "ob-poc".cbus
                        SET status = $1, updated_at = NOW()
                        WHERE cbu_id = $2
                        "#,
                    )
                    .bind(&status)
                    .bind(&cbu_id)
                    .execute(&mut **tx)
                    .await?
                } else if let Some(ctype) = client_type {
                    sqlx::query(
                        r#"
                        UPDATE "ob-poc".cbus
                        SET client_type = $1, updated_at = NOW()
                        WHERE cbu_id = $2
                        "#,
                    )
                    .bind(&ctype)
                    .bind(&cbu_id)
                    .execute(&mut **tx)
                    .await?
                } else {
                    sqlx::query(
                        r#"
                        UPDATE "ob-poc".cbus
                        SET updated_at = NOW()
                        WHERE cbu_id = $1
                        "#,
                    )
                    .bind(&cbu_id)
                    .execute(&mut **tx)
                    .await?
                };

                info!("Updated CBU: {}", cbu_id);

                Ok(CrudExecutionResult {
                    operation: "UPDATE".to_string(),
                    asset: "CBU".to_string(),
                    rows_affected: result.rows_affected(),
                    generated_id: None,
                    data: None,
                })
            }
            _ => {
                warn!("Unknown asset type for UPDATE: {}", update.asset);
                Ok(CrudExecutionResult {
                    operation: "UPDATE".to_string(),
                    asset: update.asset.clone(),
                    rows_affected: 0,
                    generated_id: None,
                    data: None,
                })
            }
        }
    }

    /// Execute a DELETE statement
    async fn execute_delete(&self, delete: &DataDelete) -> Result<CrudExecutionResult> {
        let mut tx = self.pool.begin().await?;
        let result = self.execute_delete_tx(delete, &mut tx).await?;
        tx.commit().await?;
        Ok(result)
    }

    /// Execute DELETE within a transaction
    async fn execute_delete_tx(
        &self,
        delete: &DataDelete,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    ) -> Result<CrudExecutionResult> {
        match delete.asset.as_str() {
            "CBU" => {
                let cbu_id = self
                    .get_string_value(&delete.where_clause, "cbu-id")
                    .ok_or_else(|| anyhow!("cbu-id required for DELETE"))?;

                let result = sqlx::query(
                    r#"
                    DELETE FROM "ob-poc".cbus
                    WHERE cbu_id = $1
                    "#,
                )
                .bind(&cbu_id)
                .execute(&mut **tx)
                .await?;

                info!("Deleted CBU: {}", cbu_id);

                Ok(CrudExecutionResult {
                    operation: "DELETE".to_string(),
                    asset: "CBU".to_string(),
                    rows_affected: result.rows_affected(),
                    generated_id: None,
                    data: None,
                })
            }
            _ => {
                warn!("Unknown asset type for DELETE: {}", delete.asset);
                Ok(CrudExecutionResult {
                    operation: "DELETE".to_string(),
                    asset: delete.asset.clone(),
                    rows_affected: 0,
                    generated_id: None,
                    data: None,
                })
            }
        }
    }

    /// Helper to extract string value from HashMap
    fn get_string_value(
        &self,
        values: &std::collections::HashMap<String, Value>,
        key: &str,
    ) -> Option<String> {
        values.get(key).and_then(|v| match v {
            Value::Literal(Literal::String(s)) => Some(s.clone()),
            Value::Identifier(s) => Some(s.clone()),
            _ => None,
        })
    }
}
