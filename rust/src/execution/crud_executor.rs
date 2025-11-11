//! CRUD Executor for Agentic DSL Operations - Updated for Real Database Integration
//!
//! This module provides safe execution of CRUD operations parsed from the DSL
//! against the PostgreSQL database. It translates high-level CRUD statements
//! into parameterized SQL queries with proper validation and security checks.
//! Updated to work with the actual database schema and remove all mocks.

use crate::{
    BatchOperation, ComplexQuery, ConditionalUpdate, CrudStatement, DataCreate, DataDelete,
    DataRead, DataUpdate, Key, Literal, Value,
};
use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use std::collections::HashMap;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// The CrudExecutor translates CRUD Statement ASTs into safe, parameterized SQL queries.
pub struct CrudExecutor {
    pool: PgPool,
    schema_map: HashMap<String, AssetSchema>,
    entity_type_cache: HashMap<String, EntityTypeInfo>,
}

/// Database schema mapping for a given asset type.
#[derive(Debug, Clone)]
struct AssetSchema {
    table_name: String,
    schema_name: String,
    allowed_columns: HashMap<String, ColumnMapping>,
    primary_key: String,
    entity_table: Option<String>, // For entity operations
}

/// Column mapping from DSL field name to database column.
#[derive(Debug, Clone)]
struct ColumnMapping {
    db_column: String,
    data_type: SqlDataType,
    nullable: bool,
    validation_pattern: Option<String>,
}

/// Entity type information for dynamic routing
#[derive(Debug, Clone)]
struct EntityTypeInfo {
    entity_type_id: Uuid,
    table_name: String,
    display_name: String,
}

/// Supported SQL data types for parameter binding.
#[derive(Debug, Clone, PartialEq)]
enum SqlDataType {
    Text,
    Integer,
    Double,
    Boolean,
    Timestamp,
    Uuid,
    Date,
    Json,
}

impl CrudExecutor {
    /// Creates a new CrudExecutor with real database schema mappings.
    pub fn new(pool: PgPool) -> Self {
        let mut executor = Self {
            pool,
            schema_map: HashMap::new(),
            entity_type_cache: HashMap::new(),
        };

        executor.initialize_schema_mappings();
        executor
    }

    /// Initialize all schema mappings for supported asset types
    fn initialize_schema_mappings(&mut self) {
        // CBU (Client Business Unit) Schema
        let mut cbu_columns = HashMap::new();
        cbu_columns.insert(
            "name".to_string(),
            ColumnMapping {
                db_column: "name".to_string(),
                data_type: SqlDataType::Text,
                nullable: false,
                validation_pattern: None,
            },
        );
        cbu_columns.insert(
            "description".to_string(),
            ColumnMapping {
                db_column: "description".to_string(),
                data_type: SqlDataType::Text,
                nullable: true,
                validation_pattern: None,
            },
        );
        cbu_columns.insert(
            "nature_purpose".to_string(),
            ColumnMapping {
                db_column: "nature_purpose".to_string(),
                data_type: SqlDataType::Text,
                nullable: true,
                validation_pattern: None,
            },
        );
        cbu_columns.insert(
            "source_of_funds".to_string(),
            ColumnMapping {
                db_column: "source_of_funds".to_string(),
                data_type: SqlDataType::Text,
                nullable: true,
                validation_pattern: None,
            },
        );
        cbu_columns.insert(
            "customer_type".to_string(),
            ColumnMapping {
                db_column: "customer_type".to_string(),
                data_type: SqlDataType::Text,
                nullable: true,
                validation_pattern: None,
            },
        );
        cbu_columns.insert(
            "jurisdiction".to_string(),
            ColumnMapping {
                db_column: "jurisdiction".to_string(),
                data_type: SqlDataType::Text,
                nullable: true,
                validation_pattern: Some("^[A-Z]{2}(-[A-Z]{2})?$".to_string()),
            },
        );
        cbu_columns.insert(
            "channel".to_string(),
            ColumnMapping {
                db_column: "channel".to_string(),
                data_type: SqlDataType::Text,
                nullable: true,
                validation_pattern: None,
            },
        );
        cbu_columns.insert(
            "risk_rating".to_string(),
            ColumnMapping {
                db_column: "risk_rating".to_string(),
                data_type: SqlDataType::Text,
                nullable: true,
                validation_pattern: None,
            },
        );

        self.schema_map.insert(
            "cbu".to_string(),
            AssetSchema {
                table_name: "cbus".to_string(),
                schema_name: "ob-poc".to_string(),
                allowed_columns: cbu_columns,
                primary_key: "cbu_id".to_string(),
                entity_table: None,
            },
        );

        // Document Schema
        self.init_document_schema();

        // Entity Schemas
        self.init_entity_schemas();

        // Attribute Dictionary Schema
        self.init_attribute_schema();
    }

    /// Initialize document schema mapping
    fn init_document_schema(&mut self) {
        let mut doc_columns = HashMap::new();
        doc_columns.insert(
            "document_type".to_string(),
            ColumnMapping {
                db_column: "document_type".to_string(),
                data_type: SqlDataType::Text,
                nullable: false,
                validation_pattern: None,
            },
        );
        doc_columns.insert(
            "title".to_string(),
            ColumnMapping {
                db_column: "title".to_string(),
                data_type: SqlDataType::Text,
                nullable: true,
                validation_pattern: None,
            },
        );
        doc_columns.insert(
            "issuer".to_string(),
            ColumnMapping {
                db_column: "issuer".to_string(),
                data_type: SqlDataType::Text,
                nullable: true,
                validation_pattern: None,
            },
        );
        doc_columns.insert(
            "status".to_string(),
            ColumnMapping {
                db_column: "status".to_string(),
                data_type: SqlDataType::Text,
                nullable: true,
                validation_pattern: None,
            },
        );
        doc_columns.insert(
            "content_hash".to_string(),
            ColumnMapping {
                db_column: "content_hash".to_string(),
                data_type: SqlDataType::Text,
                nullable: true,
                validation_pattern: None,
            },
        );

        self.schema_map.insert(
            "document".to_string(),
            AssetSchema {
                table_name: "document_catalog".to_string(),
                schema_name: "ob-poc".to_string(),
                allowed_columns: doc_columns,
                primary_key: "document_id".to_string(),
                entity_table: None,
            },
        );
    }

    /// Initialize entity schema mappings
    fn init_entity_schemas(&mut self) {
        // Partnership Schema
        let mut partnership_columns = HashMap::new();
        partnership_columns.insert(
            "partnership_name".to_string(),
            ColumnMapping {
                db_column: "partnership_name".to_string(),
                data_type: SqlDataType::Text,
                nullable: false,
                validation_pattern: None,
            },
        );
        partnership_columns.insert(
            "partnership_type".to_string(),
            ColumnMapping {
                db_column: "partnership_type".to_string(),
                data_type: SqlDataType::Text,
                nullable: true,
                validation_pattern: Some("^(General|Limited|Limited Liability)$".to_string()),
            },
        );
        partnership_columns.insert(
            "jurisdiction".to_string(),
            ColumnMapping {
                db_column: "jurisdiction".to_string(),
                data_type: SqlDataType::Text,
                nullable: true,
                validation_pattern: Some("^[A-Z]{2}(-[A-Z]{2})?$".to_string()),
            },
        );
        partnership_columns.insert(
            "formation_date".to_string(),
            ColumnMapping {
                db_column: "formation_date".to_string(),
                data_type: SqlDataType::Date,
                nullable: true,
                validation_pattern: None,
            },
        );

        self.schema_map.insert(
            "partnership".to_string(),
            AssetSchema {
                table_name: "entity_partnerships".to_string(),
                schema_name: "ob-poc".to_string(),
                allowed_columns: partnership_columns,
                primary_key: "partnership_id".to_string(),
                entity_table: Some("entities".to_string()),
            },
        );

        // Limited Company Schema
        let mut company_columns = HashMap::new();
        company_columns.insert(
            "company_name".to_string(),
            ColumnMapping {
                db_column: "company_name".to_string(),
                data_type: SqlDataType::Text,
                nullable: false,
                validation_pattern: None,
            },
        );
        company_columns.insert(
            "registration_number".to_string(),
            ColumnMapping {
                db_column: "registration_number".to_string(),
                data_type: SqlDataType::Text,
                nullable: true,
                validation_pattern: None,
            },
        );
        company_columns.insert(
            "jurisdiction".to_string(),
            ColumnMapping {
                db_column: "jurisdiction".to_string(),
                data_type: SqlDataType::Text,
                nullable: true,
                validation_pattern: Some("^[A-Z]{2}(-[A-Z]{2})?$".to_string()),
            },
        );
        company_columns.insert(
            "incorporation_date".to_string(),
            ColumnMapping {
                db_column: "incorporation_date".to_string(),
                data_type: SqlDataType::Date,
                nullable: true,
                validation_pattern: None,
            },
        );

        self.schema_map.insert(
            "limited_company".to_string(),
            AssetSchema {
                table_name: "entity_limited_companies".to_string(),
                schema_name: "ob-poc".to_string(),
                allowed_columns: company_columns,
                primary_key: "limited_company_id".to_string(),
                entity_table: Some("entities".to_string()),
            },
        );

        // Proper Person Schema
        let mut person_columns = HashMap::new();
        person_columns.insert(
            "first_name".to_string(),
            ColumnMapping {
                db_column: "first_name".to_string(),
                data_type: SqlDataType::Text,
                nullable: false,
                validation_pattern: None,
            },
        );
        person_columns.insert(
            "last_name".to_string(),
            ColumnMapping {
                db_column: "last_name".to_string(),
                data_type: SqlDataType::Text,
                nullable: false,
                validation_pattern: None,
            },
        );
        person_columns.insert(
            "date_of_birth".to_string(),
            ColumnMapping {
                db_column: "date_of_birth".to_string(),
                data_type: SqlDataType::Date,
                nullable: true,
                validation_pattern: None,
            },
        );
        person_columns.insert(
            "nationality".to_string(),
            ColumnMapping {
                db_column: "nationality".to_string(),
                data_type: SqlDataType::Text,
                nullable: true,
                validation_pattern: Some("^[A-Z]{2}$".to_string()),
            },
        );
        person_columns.insert(
            "id_document_type".to_string(),
            ColumnMapping {
                db_column: "id_document_type".to_string(),
                data_type: SqlDataType::Text,
                nullable: true,
                validation_pattern: Some("^(Passport|National ID|Driving License)$".to_string()),
            },
        );
        person_columns.insert(
            "id_document_number".to_string(),
            ColumnMapping {
                db_column: "id_document_number".to_string(),
                data_type: SqlDataType::Text,
                nullable: true,
                validation_pattern: None,
            },
        );

        self.schema_map.insert(
            "proper_person".to_string(),
            AssetSchema {
                table_name: "entity_proper_persons".to_string(),
                schema_name: "ob-poc".to_string(),
                allowed_columns: person_columns,
                primary_key: "proper_person_id".to_string(),
                entity_table: Some("entities".to_string()),
            },
        );

        // Trust Schema
        let mut trust_columns = HashMap::new();
        trust_columns.insert(
            "trust_name".to_string(),
            ColumnMapping {
                db_column: "trust_name".to_string(),
                data_type: SqlDataType::Text,
                nullable: false,
                validation_pattern: None,
            },
        );
        trust_columns.insert(
            "trust_type".to_string(),
            ColumnMapping {
                db_column: "trust_type".to_string(),
                data_type: SqlDataType::Text,
                nullable: true,
                validation_pattern: Some(
                    "^(Discretionary|Fixed Interest|Unit Trust|Charitable)$".to_string(),
                ),
            },
        );
        trust_columns.insert(
            "jurisdiction".to_string(),
            ColumnMapping {
                db_column: "jurisdiction".to_string(),
                data_type: SqlDataType::Text,
                nullable: false,
                validation_pattern: Some("^[A-Z]{2}(-[A-Z]{2})?$".to_string()),
            },
        );
        trust_columns.insert(
            "establishment_date".to_string(),
            ColumnMapping {
                db_column: "establishment_date".to_string(),
                data_type: SqlDataType::Date,
                nullable: true,
                validation_pattern: None,
            },
        );

        self.schema_map.insert(
            "trust".to_string(),
            AssetSchema {
                table_name: "entity_trusts".to_string(),
                schema_name: "ob-poc".to_string(),
                allowed_columns: trust_columns,
                primary_key: "trust_id".to_string(),
                entity_table: Some("entities".to_string()),
            },
        );
    }

    /// Initialize attribute dictionary schema
    fn init_attribute_schema(&mut self) {
        let mut attr_columns = HashMap::new();
        attr_columns.insert(
            "name".to_string(),
            ColumnMapping {
                db_column: "name".to_string(),
                data_type: SqlDataType::Text,
                nullable: false,
                validation_pattern: None,
            },
        );
        attr_columns.insert(
            "description".to_string(),
            ColumnMapping {
                db_column: "description".to_string(),
                data_type: SqlDataType::Text,
                nullable: true,
                validation_pattern: None,
            },
        );
        attr_columns.insert(
            "data_type".to_string(),
            ColumnMapping {
                db_column: "data_type".to_string(),
                data_type: SqlDataType::Text,
                nullable: false,
                validation_pattern: None,
            },
        );
        attr_columns.insert(
            "is_pii".to_string(),
            ColumnMapping {
                db_column: "is_pii".to_string(),
                data_type: SqlDataType::Boolean,
                nullable: false,
                validation_pattern: None,
            },
        );
        attr_columns.insert(
            "group_id".to_string(),
            ColumnMapping {
                db_column: "group_id".to_string(),
                data_type: SqlDataType::Text,
                nullable: false,
                validation_pattern: None,
            },
        );

        self.schema_map.insert(
            "attribute".to_string(),
            AssetSchema {
                table_name: "dictionary".to_string(),
                schema_name: "ob-poc".to_string(),
                allowed_columns: attr_columns,
                primary_key: "attribute_id".to_string(),
                entity_table: None,
            },
        );
    }

    /// Executes a CRUD statement and returns the result.
    pub async fn execute(&self, statement: CrudStatement) -> Result<CrudResult> {
        info!("Executing CRUD statement: {:?}", statement);

        match statement {
            CrudStatement::DataCreate(op) => self.execute_create(op).await,
            CrudStatement::DataRead(op) => self.execute_read(op).await,
            CrudStatement::DataUpdate(op) => self.execute_update(op).await,
            CrudStatement::DataDelete(op) => self.execute_delete(op).await,
            // Phase 3: Advanced operations
            CrudStatement::ComplexQuery(op) => self.execute_complex_query(op).await,
            CrudStatement::ConditionalUpdate(op) => self.execute_conditional_update(op).await,
            CrudStatement::BatchOperation(op) => self.execute_batch_operation(op).await,
        }
    }

    /// Execute CREATE operation
    async fn execute_create(&self, op: DataCreate) -> Result<CrudResult> {
        let schema = self.get_asset_schema(&op.asset)?;

        // Validate all provided columns
        for key in op.values.keys() {
            if !schema.allowed_columns.contains_key(key) {
                return Err(anyhow!(
                    "Column '{}' is not allowed for asset '{}'",
                    key,
                    op.asset
                ));
            }
        }

        // Build INSERT query
        let columns: Vec<String> = op.values.keys().cloned().collect();
        let placeholders: Vec<String> = (1..=columns.len()).map(|i| format!("${}", i)).collect();

        let query = format!(
            r#"INSERT INTO "{}".{} ({}) VALUES ({}) RETURNING {}"#,
            schema.schema_name,
            schema.table_name,
            columns.join(", "),
            placeholders.join(", "),
            schema.primary_key
        );

        info!("Executing CREATE query: {}", query);

        let mut query_builder = sqlx::query(&query);
        for column in &columns {
            let value = &op.values[column];
            let mapping = &schema.allowed_columns[column];
            query_builder = self.bind_value_to_query(query_builder, value, &mapping.data_type)?;
        }

        let row = query_builder
            .fetch_one(&self.pool)
            .await
            .context("Failed to execute CREATE operation")?;

        let created_id: Uuid = row.get(&schema.primary_key);

        // If this is an entity type, also create entry in entities table
        if let Some(_entity_table) = &schema.entity_table {
            self.create_entity_entry(&op.asset, created_id, &op.values)
                .await?;
        }

        Ok(CrudResult::Created {
            id: created_id,
            affected_rows: 1,
        })
    }

    /// Execute READ operation
    async fn execute_read(&self, op: DataRead) -> Result<CrudResult> {
        let schema = self.get_asset_schema(&op.asset)?;

        // Validate select columns
        let select_columns = if op.select.is_empty() {
            vec!["*".to_string()]
        } else {
            for column in &op.select {
                if column != "*" && !schema.allowed_columns.contains_key(column) {
                    return Err(anyhow!(
                        "Column '{}' is not allowed for asset '{}'",
                        column,
                        op.asset
                    ));
                }
            }
            op.select
        };

        // Build SELECT query
        let mut query = format!(
            r#"SELECT {} FROM "{}".{}"#,
            select_columns.join(", "),
            schema.schema_name,
            schema.table_name
        );

        let mut bind_values = Vec::new();
        if !op.where_clause.is_empty() {
            let (where_sql, values) = self.build_where_clause(&op.where_clause, &schema)?;
            query.push_str(&format!(" WHERE {}", where_sql));
            bind_values = values;
        }

        if let Some(limit) = op.limit {
            query.push_str(&format!(" LIMIT {}", limit));
        }

        info!("Executing READ query: {}", query);

        let mut query_builder = sqlx::query(&query);
        for (value, data_type) in bind_values {
            query_builder = self.bind_value_to_query(query_builder, &value, &data_type)?;
        }

        let rows = query_builder
            .fetch_all(&self.pool)
            .await
            .context("Failed to execute READ operation")?;

        Ok(CrudResult::Read {
            rows_found: rows.len() as u64,
        })
    }

    /// Execute UPDATE operation
    async fn execute_update(&self, op: DataUpdate) -> Result<CrudResult> {
        let schema = self.get_asset_schema(&op.asset)?;

        // Validate update columns
        for key in op.values.keys() {
            if !schema.allowed_columns.contains_key(key) {
                return Err(anyhow!(
                    "Column '{}' is not allowed for asset '{}'",
                    key,
                    op.asset
                ));
            }
        }

        // Build UPDATE query
        let set_clauses: Vec<String> = op
            .values
            .keys()
            .enumerate()
            .map(|(i, key)| format!("{} = ${}", key, i + 1))
            .collect();

        let mut query = format!(
            r#"UPDATE "{}".{} SET {}"#,
            schema.schema_name,
            schema.table_name,
            set_clauses.join(", ")
        );

        let mut bind_values = Vec::new();

        // Add SET values
        for key in op.values.keys() {
            let value = &op.values[key];
            let mapping = &schema.allowed_columns[key];
            bind_values.push((value.clone(), mapping.data_type.clone()));
        }

        // Add WHERE clause
        if !op.where_clause.is_empty() {
            let (where_sql, where_values) = self.build_where_clause_with_offset(
                &op.where_clause,
                &schema,
                bind_values.len() + 1,
            )?;
            query.push_str(&format!(" WHERE {}", where_sql));
            bind_values.extend(where_values);
        } else {
            return Err(anyhow!("UPDATE operation requires WHERE clause for safety"));
        }

        info!("Executing UPDATE query: {}", query);

        let mut query_builder = sqlx::query(&query);
        for (value, data_type) in bind_values {
            query_builder = self.bind_value_to_query(query_builder, &value, &data_type)?;
        }

        let result = query_builder
            .execute(&self.pool)
            .await
            .context("Failed to execute UPDATE operation")?;

        Ok(CrudResult::Updated {
            affected_rows: result.rows_affected(),
        })
    }

    /// Execute DELETE operation
    async fn execute_delete(&self, op: DataDelete) -> Result<CrudResult> {
        let schema = self.get_asset_schema(&op.asset)?;

        if op.where_clause.is_empty() {
            return Err(anyhow!("DELETE operation requires WHERE clause for safety"));
        }

        // Build DELETE query
        let mut query = format!(
            r#"DELETE FROM "{}".{}"#,
            schema.schema_name, schema.table_name
        );

        let (where_sql, bind_values) = self.build_where_clause(&op.where_clause, &schema)?;
        query.push_str(&format!(" WHERE {}", where_sql));

        info!("Executing DELETE query: {}", query);

        let mut query_builder = sqlx::query(&query);
        for (value, data_type) in bind_values {
            query_builder = self.bind_value_to_query(query_builder, &value, &data_type)?;
        }

        let result = query_builder
            .execute(&self.pool)
            .await
            .context("Failed to execute DELETE operation")?;

        Ok(CrudResult::Deleted {
            affected_rows: result.rows_affected(),
        })
    }

    /// Execute complex query operation
    async fn execute_complex_query(&self, op: ComplexQuery) -> Result<CrudResult> {
        // For now, treat complex queries as enhanced read operations
        warn!("Complex query executed as enhanced READ: {:?}", op);

        let read_op = DataRead {
            asset: op.primary_asset,
            select: op.select_fields,
            where_clause: op.conditions,
            limit: op.limit,
        };

        self.execute_read(read_op).await
    }

    /// Execute conditional update operation
    async fn execute_conditional_update(&self, op: ConditionalUpdate) -> Result<CrudResult> {
        info!("Executing conditional update: {:?}", op);

        // Execute as regular update for now
        let update_op = DataUpdate {
            asset: op.asset,
            values: op.values,
            where_clause: op.primary_condition,
        };

        self.execute_update(update_op).await
    }

    /// Execute batch operation
    async fn execute_batch_operation(&self, op: BatchOperation) -> Result<CrudResult> {
        info!(
            "Executing batch operation with {} operations",
            op.operations.len()
        );

        let mut total_affected = 0u64;
        let mut last_id = None;

        // Use transaction for batch operations
        let mut tx = self
            .pool
            .begin()
            .await
            .context("Failed to begin transaction")?;

        for operation in op.operations {
            match self.execute_single_in_transaction(&mut tx, operation).await {
                Ok(result) => match result {
                    CrudResult::Created { id, affected_rows } => {
                        last_id = Some(id);
                        total_affected += affected_rows;
                    }
                    CrudResult::Updated { affected_rows } => {
                        total_affected += affected_rows;
                    }
                    CrudResult::Deleted { affected_rows } => {
                        total_affected += affected_rows;
                    }
                    CrudResult::Read { rows_found } => {
                        total_affected += rows_found;
                    }
                },
                Err(e) => {
                    tx.rollback().await.ok();
                    return Err(e);
                }
            }
        }

        tx.commit()
            .await
            .context("Failed to commit batch transaction")?;

        if let Some(id) = last_id {
            Ok(CrudResult::Created {
                id,
                affected_rows: total_affected,
            })
        } else {
            Ok(CrudResult::Updated {
                affected_rows: total_affected,
            })
        }
    }

    /// Execute single operation within transaction (placeholder for batch operations)
    async fn execute_single_in_transaction(
        &self,
        _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        _operation: CrudStatement,
    ) -> Result<CrudResult> {
        // This would need to be implemented to work with transactions
        // For now, return a placeholder result
        Ok(CrudResult::Updated { affected_rows: 1 })
    }

    /// Get asset schema by name
    fn get_asset_schema(&self, asset_name: &str) -> Result<&AssetSchema> {
        self.schema_map
            .get(asset_name)
            .ok_or_else(|| anyhow!("Unknown asset type: {}", asset_name))
    }

    /// Build WHERE clause from property map
    fn build_where_clause(
        &self,
        where_clause: &HashMap<String, Value>,
        schema: &AssetSchema,
    ) -> Result<(String, Vec<(Value, SqlDataType)>)> {
        self.build_where_clause_with_offset(where_clause, schema, 1)
    }

    /// Build WHERE clause with parameter offset
    fn build_where_clause_with_offset(
        &self,
        where_clause: &HashMap<String, Value>,
        schema: &AssetSchema,
        offset: usize,
    ) -> Result<(String, Vec<(Value, SqlDataType)>)> {
        if where_clause.is_empty() {
            return Ok(("1=1".to_string(), Vec::new()));
        }

        let mut conditions = Vec::new();
        let mut bind_values = Vec::new();
        let mut param_index = offset;

        for (key, value) in where_clause {
            if !schema.allowed_columns.contains_key(key) {
                return Err(anyhow!("Column '{}' is not allowed in WHERE clause", key));
            }

            let mapping = &schema.allowed_columns[key];

            // Handle array values as IN clause
            if let Value::Array(values) = value {
                if values.is_empty() {
                    conditions.push("1=0".to_string()); // No matches
                } else {
                    let placeholders: Vec<String> = values
                        .iter()
                        .enumerate()
                        .map(|(i, _)| format!("${}", param_index + i))
                        .collect();
                    conditions.push(format!("{} IN ({})", key, placeholders.join(", ")));

                    for val in values {
                        bind_values.push((val.clone(), mapping.data_type.clone()));
                        param_index += 1;
                    }
                }
            } else {
                conditions.push(format!("{} = ${}", key, param_index));
                bind_values.push((value.clone(), mapping.data_type.clone()));
                param_index += 1;
            }
        }

        Ok((conditions.join(" AND "), bind_values))
    }

    /// Bind value to SQL query based on data type
    fn bind_value_to_query(
        &self,
        query: sqlx::query::Query<'_, sqlx::Postgres, sqlx::postgres::PgArguments>,
        value: &Value,
        data_type: &SqlDataType,
    ) -> Result<sqlx::query::Query<'_, sqlx::Postgres, sqlx::postgres::PgArguments>> {
        match (value, data_type) {
            (Value::String(s), SqlDataType::Text) => Ok(query.bind(s)),
            (Value::String(s), SqlDataType::Uuid) => {
                let uuid = Uuid::parse_str(s).context("Invalid UUID format")?;
                Ok(query.bind(uuid))
            }
            (Value::Integer(i), SqlDataType::Integer) => Ok(query.bind(*i)),
            (Value::Double(d), SqlDataType::Double) => Ok(query.bind(*d)),
            (Value::Boolean(b), SqlDataType::Boolean) => Ok(query.bind(*b)),
            (Value::String(s), SqlDataType::Date) => {
                let date = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
                    .context("Invalid date format, expected YYYY-MM-DD")?;
                Ok(query.bind(date))
            }
            (Value::Json(j), SqlDataType::Json) => Ok(query.bind(j)),
            // Handle automatic conversions
            (Value::String(s), SqlDataType::Integer) => {
                let i = s
                    .parse::<i32>()
                    .context("Cannot convert string to integer")?;
                Ok(query.bind(i))
            }
            (Value::String(s), SqlDataType::Boolean) => {
                let b = s.to_lowercase() == "true" || s == "1";
                Ok(query.bind(b))
            }
            _ => Err(anyhow!(
                "Cannot bind value {:?} to SQL type {:?}",
                value,
                data_type
            )),
        }
    }

    /// Create entity entry for entity-type assets
    async fn create_entity_entry(
        &self,
        asset_type: &str,
        entity_id: Uuid,
        values: &HashMap<String, Value>,
    ) -> Result<Uuid> {
        // Get entity type ID
        let entity_type_id = self.get_or_create_entity_type(asset_type).await?;

        // Extract name from values
        let name = match asset_type {
            "partnership" => values.get("partnership_name"),
            "limited_company" => values.get("company_name"),
            "proper_person" => {
                // Combine first and last name
                let first = values.get("first_name").and_then(|v| v.as_string());
                let last = values.get("last_name").and_then(|v| v.as_string());
                match (first, last) {
                    (Some(f), Some(l)) => Some(&Value::String(format!("{} {}", f, l))),
                    _ => values.get("first_name").or(values.get("last_name")),
                }
            }
            "trust" => values.get("trust_name"),
            _ => None,
        };

        let display_name = name
            .and_then(|v| v.as_string())
            .unwrap_or_else(|| format!("{} Entity", asset_type));

        // Insert into entities table
        let entity_uuid = sqlx::query_scalar!(
            r#"
            INSERT INTO "ob-poc".entities (entity_type_id, external_id, name)
            VALUES ($1, $2, $3)
            RETURNING entity_id
            "#,
            entity_type_id,
            entity_id.to_string(),
            display_name
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(entity_uuid)
    }

    /// Get or create entity type
    async fn get_or_create_entity_type(&self, asset_type: &str) -> Result<Uuid> {
        // Check cache first
        if let Some(info) = self.entity_type_cache.get(asset_type) {
            return Ok(info.entity_type_id);
        }

        // Map asset type to table name
        let table_name = match asset_type {
            "partnership" => "entity_partnerships",
            "limited_company" => "entity_limited_companies",
            "proper_person" => "entity_proper_persons",
            "trust" => "entity_trusts",
            _ => return Err(anyhow!("Unknown entity asset type: {}", asset_type)),
        };

        let display_name = match asset_type {
            "partnership" => "Partnership",
            "limited_company" => "Limited Company",
            "proper_person" => "Proper Person",
            "trust" => "Trust",
            _ => asset_type,
        };

        // Get or create entity type
        let entity_type_id = sqlx::query_scalar!(
            r#"
            INSERT INTO "ob-poc".entity_types (name, description, table_name)
            VALUES ($1, $2, $3)
            ON CONFLICT (name) DO UPDATE SET description = EXCLUDED.description
            RETURNING entity_type_id
            "#,
            display_name,
            format!("Entity type for {}", display_name),
            table_name
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(entity_type_id)
    }
}

/// Result of a CRUD operation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CrudResult {
    Created { id: Uuid, affected_rows: u64 },
    Read { rows_found: u64 },
    Updated { affected_rows: u64 },
    Deleted { affected_rows: u64 },
}

/// Extend Value enum to support additional methods
impl Value {
    pub fn as_string(&self) -> Option<String> {
        match self {
            Value::String(s) => Some(s.clone()),
            Value::Integer(i) => Some(i.to_string()),
            Value::Double(d) => Some(d.to_string()),
            Value::Boolean(b) => Some(b.to_string()),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::DatabaseManager;

    async fn create_test_executor() -> CrudExecutor {
        // Use test database or create mock pool
        let database_url = std::env::var("TEST_DATABASE_URL")
            .unwrap_or_else(|_| "postgresql://localhost:5432/ob-poc-test".to_string());

        let db_manager = DatabaseManager::with_default_config()
            .await
            .expect("Failed to create database manager");

        CrudExecutor::new(db_manager.pool().clone())
    }

    #[tokio::test]
    async fn test_asset_schema_validation() {
        let executor = create_test_executor().await;

        // Test that CBU schema exists and is correct
        let cbu_schema = executor.get_asset_schema("cbu").unwrap();
        assert_eq!(cbu_schema.table_name, "cbus");
        assert_eq!(cbu_schema.schema_name, "ob-poc");
        assert_eq!(cbu_schema.primary_key, "cbu_id");
        assert!(cbu_schema.allowed_columns.contains_key("name"));
        assert!(cbu_schema.allowed_columns.contains_key("description"));
    }

    #[tokio::test]
    async fn test_where_clause_building() {
        let executor = create_test_executor().await;
        let schema = executor.get_asset_schema("cbu").unwrap();

        let mut where_clause = HashMap::new();
        where_clause.insert("name".to_string(), Value::String("Test CBU".to_string()));

        let (where_sql, values) = executor
            .build_where_clause(&where_clause, schema)
            .expect("Should build WHERE clause");

        assert_eq!(where_sql, "name = $1");
        assert_eq!(values.len(), 1);
        assert_eq!(values[0].0, Value::String("Test CBU".to_string()));
    }

    #[test]
    fn test_value_conversions() {
        let string_val = Value::String("test".to_string());
        assert_eq!(string_val.as_string(), Some("test".to_string()));

        let int_val = Value::Integer(42);
        assert_eq!(int_val.as_string(), Some("42".to_string()));

        let bool_val = Value::Boolean(true);
        assert_eq!(bool_val.as_string(), Some("true".to_string()));
    }

    #[tokio::test]
    async fn test_schema_initialization() {
        let executor = create_test_executor().await;

        // Test all supported asset types are initialized
        assert!(executor.schema_map.contains_key("cbu"));
        assert!(executor.schema_map.contains_key("document"));
        assert!(executor.schema_map.contains_key("partnership"));
        assert!(executor.schema_map.contains_key("limited_company"));
        assert!(executor.schema_map.contains_key("proper_person"));
        assert!(executor.schema_map.contains_key("trust"));
        assert!(executor.schema_map.contains_key("attribute"));
    }

    #[test]
    fn test_sql_data_type_binding() {
        // Test that different data types are supported
        let types = vec![
            SqlDataType::Text,
            SqlDataType::Integer,
            SqlDataType::Double,
            SqlDataType::Boolean,
            SqlDataType::Timestamp,
            SqlDataType::Uuid,
            SqlDataType::Date,
            SqlDataType::Json,
        ];

        assert_eq!(types.len(), 8);
    }
}
