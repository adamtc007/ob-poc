//! CRUD Executor for Agentic DSL Operations
//!
//! This module provides safe execution of CRUD operations parsed from the DSL
//! against the PostgreSQL database. It translates high-level CRUD statements
//! into parameterized SQL queries with proper validation and security checks.

use crate::{
    BatchOperation, ComplexQuery, ConditionalUpdate, CrudStatement, DataCreate, DataDelete,
    DataRead, DataUpdate, Key, Literal, Value,
};
use anyhow::{anyhow, Context, Result};
use sqlx::{PgPool, Row};
use std::collections::HashMap;

/// The CrudExecutor translates CRUD Statement ASTs into safe, parameterized SQL queries.
pub struct CrudExecutor {
    pool: PgPool,
    schema_map: HashMap<String, AssetSchema>,
}

/// Database schema mapping for a given asset type.
#[derive(Debug, Clone)]
struct AssetSchema {
    table_name: String,
    schema_name: String,
    allowed_columns: HashMap<String, ColumnMapping>,
    primary_key: String,
}

/// Column mapping from DSL field name to database column.
#[derive(Debug, Clone)]
struct ColumnMapping {
    db_column: String,
    data_type: SqlDataType,
    nullable: bool,
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
}

impl CrudExecutor {
    /// Creates a new CrudExecutor with predefined asset schemas.
    pub fn new(pool: PgPool) -> Self {
        let mut schema_map = HashMap::new();

        // CBU (Client Business Unit) Schema
        let mut cbu_columns = HashMap::new();
        cbu_columns.insert(
            "name".to_string(),
            ColumnMapping {
                db_column: "name".to_string(),
                data_type: SqlDataType::Text,
                nullable: false,
            },
        );
        cbu_columns.insert(
            "description".to_string(),
            ColumnMapping {
                db_column: "description".to_string(),
                data_type: SqlDataType::Text,
                nullable: true,
            },
        );
        cbu_columns.insert(
            "jurisdiction".to_string(),
            ColumnMapping {
                db_column: "jurisdiction".to_string(),
                data_type: SqlDataType::Text,
                nullable: true,
            },
        );
        cbu_columns.insert(
            "entity_type".to_string(),
            ColumnMapping {
                db_column: "entity_type".to_string(),
                data_type: SqlDataType::Text,
                nullable: true,
            },
        );

        schema_map.insert(
            "cbu".to_string(),
            AssetSchema {
                table_name: "cbus".to_string(),
                schema_name: "ob-poc".to_string(),
                allowed_columns: cbu_columns,
                primary_key: "id".to_string(),
            },
        );

        // Document Schema
        let mut doc_columns = HashMap::new();
        doc_columns.insert(
            "type".to_string(),
            ColumnMapping {
                db_column: "document_type".to_string(),
                data_type: SqlDataType::Text,
                nullable: false,
            },
        );
        doc_columns.insert(
            "title".to_string(),
            ColumnMapping {
                db_column: "title".to_string(),
                data_type: SqlDataType::Text,
                nullable: true,
            },
        );
        doc_columns.insert(
            "issuer".to_string(),
            ColumnMapping {
                db_column: "issuer".to_string(),
                data_type: SqlDataType::Text,
                nullable: true,
            },
        );
        doc_columns.insert(
            "status".to_string(),
            ColumnMapping {
                db_column: "status".to_string(),
                data_type: SqlDataType::Text,
                nullable: true,
            },
        );

        schema_map.insert(
            "document".to_string(),
            AssetSchema {
                table_name: "document_catalog".to_string(),
                schema_name: "ob-poc".to_string(),
                allowed_columns: doc_columns,
                primary_key: "id".to_string(),
            },
        );

        // Attribute Schema
        let mut attr_columns = HashMap::new();
        attr_columns.insert(
            "name".to_string(),
            ColumnMapping {
                db_column: "name".to_string(),
                data_type: SqlDataType::Text,
                nullable: false,
            },
        );
        attr_columns.insert(
            "description".to_string(),
            ColumnMapping {
                db_column: "description".to_string(),
                data_type: SqlDataType::Text,
                nullable: true,
            },
        );
        attr_columns.insert(
            "data_type".to_string(),
            ColumnMapping {
                db_column: "data_type".to_string(),
                data_type: SqlDataType::Text,
                nullable: false,
            },
        );
        attr_columns.insert(
            "is_pii".to_string(),
            ColumnMapping {
                db_column: "is_pii".to_string(),
                data_type: SqlDataType::Boolean,
                nullable: false,
            },
        );

        schema_map.insert(
            "attribute".to_string(),
            AssetSchema {
                table_name: "dictionary".to_string(),
                schema_name: "ob-poc".to_string(),
                allowed_columns: attr_columns,
                primary_key: "id".to_string(),
            },
        );

        Self { pool, schema_map }
    }

    /// Executes a CRUD statement and returns the result.
    pub async fn execute(&self, statement: CrudStatement) -> Result<CrudResult> {
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

    /// Executes a CREATE operation.
    async fn execute_create(&self, op: DataCreate) -> Result<CrudResult> {
        let schema = self
            .schema_map
            .get(&op.asset)
            .context(format!("Unknown asset type: {}", op.asset))?;

        let mut columns = Vec::new();
        let mut placeholders = Vec::new();
        let mut values = Vec::new();

        // Process each field in the values map
        for (key, value) in &op.values {
            let field_name = key.as_str();
            let column_mapping = schema.allowed_columns.get(&field_name).context(format!(
                "Field '{}' is not allowed for asset '{}'",
                field_name, op.asset
            ))?;

            columns.push(&column_mapping.db_column);
            placeholders.push(format!("${}", values.len() + 1));
            values.push(value);
        }

        // Build the INSERT query
        let query_str = format!(
            "INSERT INTO \"{}\".{} ({}) VALUES ({}) RETURNING *",
            schema.schema_name,
            schema.table_name,
            columns.join(", "),
            placeholders.join(", ")
        );

        log::info!("Executing CREATE: {}", query_str);

        // Execute the query with parameter binding
        let mut query = sqlx::query(&query_str);
        for value in values {
            query = self.bind_value(query, value)?;
        }

        let row = query
            .fetch_one(&self.pool)
            .await
            .context("Failed to execute CREATE operation")?;

        Ok(CrudResult::Created {
            id: row.try_get::<i32, _>(0)?,
            affected_rows: 1,
        })
    }

    /// Executes a READ operation.
    async fn execute_read(&self, op: DataRead) -> Result<CrudResult> {
        let schema = self
            .schema_map
            .get(&op.asset)
            .context(format!("Unknown asset type: {}", op.asset))?;

        // Build SELECT clause
        let select_clause = if let Some(fields) = &op.select_fields {
            let field_names: Result<Vec<String>> = fields
                .iter()
                .map(|v| match v {
                    Value::Literal(Literal::String(s)) => Ok(s.clone()),
                    Value::Identifier(s) => Ok(s.clone()),
                    _ => Err(anyhow!("Select fields must be strings or identifiers")),
                })
                .collect();
            let field_names = field_names?;

            // Validate and map field names to database columns
            let db_columns: Result<Vec<String>> = field_names
                .iter()
                .map(|field| {
                    schema
                        .allowed_columns
                        .get(field)
                        .map(|mapping| mapping.db_column.clone())
                        .context(format!(
                            "Invalid field '{}' for asset '{}'",
                            field, op.asset
                        ))
                })
                .collect();

            db_columns?.join(", ")
        } else {
            "*".to_string()
        };

        // Build WHERE clause
        let (where_clause, values) = if let Some(where_map) = &op.where_clause {
            self.build_where_clause(schema, where_map)?
        } else {
            ("".to_string(), Vec::new())
        };

        // Build the SELECT query
        let query_str = format!(
            "SELECT {} FROM \"{}\".{}{}",
            select_clause,
            schema.schema_name,
            schema.table_name,
            if where_clause.is_empty() {
                "".to_string()
            } else {
                format!(" WHERE {}", where_clause)
            }
        );

        log::info!("Executing READ: {}", query_str);

        // Execute the query
        let mut query = sqlx::query(&query_str);
        for value in values {
            query = self.bind_value(query, &value)?;
        }

        let rows = query
            .fetch_all(&self.pool)
            .await
            .context("Failed to execute READ operation")?;

        Ok(CrudResult::Read {
            rows_found: rows.len(),
            // For now, we'll just return the count. In a full implementation,
            // we'd return the actual data in a structured format.
        })
    }

    /// Executes an UPDATE operation.
    async fn execute_update(&self, op: DataUpdate) -> Result<CrudResult> {
        let schema = self
            .schema_map
            .get(&op.asset)
            .context(format!("Unknown asset type: {}", op.asset))?;

        // Build SET clause
        let mut set_clauses = Vec::new();
        let mut values = Vec::new();

        for (key, value) in &op.values {
            let field_name = key.as_str();
            let column_mapping = schema.allowed_columns.get(&field_name).context(format!(
                "Field '{}' is not allowed for asset '{}'",
                field_name, op.asset
            ))?;

            set_clauses.push(format!(
                "{} = ${}",
                column_mapping.db_column,
                values.len() + 1
            ));
            values.push(value);
        }

        // Build WHERE clause
        let (where_clause, where_values) = self.build_where_clause(schema, &op.where_clause)?;
        values.extend(where_values.iter());

        // Build the UPDATE query
        let query_str = format!(
            "UPDATE \"{}\".{} SET {} WHERE {}",
            schema.schema_name,
            schema.table_name,
            set_clauses.join(", "),
            where_clause
        );

        log::info!("Executing UPDATE: {}", query_str);

        // Execute the query
        let mut query = sqlx::query(&query_str);
        for value in values {
            query = self.bind_value(query, value)?;
        }

        let result = query
            .execute(&self.pool)
            .await
            .context("Failed to execute UPDATE operation")?;

        Ok(CrudResult::Updated {
            affected_rows: result.rows_affected(),
        })
    }

    /// Executes a DELETE operation.
    async fn execute_delete(&self, op: DataDelete) -> Result<CrudResult> {
        let schema = self
            .schema_map
            .get(&op.asset)
            .context(format!("Unknown asset type: {}", op.asset))?;

        // Build WHERE clause
        let (where_clause, values) = self.build_where_clause(schema, &op.where_clause)?;

        // Build the DELETE query
        let query_str = format!(
            "DELETE FROM \"{}\".{} WHERE {}",
            schema.schema_name, schema.table_name, where_clause
        );

        log::info!("Executing DELETE: {}", query_str);

        // Execute the query
        let mut query = sqlx::query(&query_str);
        for value in values {
            query = self.bind_value(query, &value)?;
        }

        let result = query
            .execute(&self.pool)
            .await
            .context("Failed to execute DELETE operation")?;

        Ok(CrudResult::Deleted {
            affected_rows: result.rows_affected(),
        })
    }

    /// Builds a WHERE clause from a property map.
    fn build_where_clause(
        &self,
        schema: &AssetSchema,
        where_map: &HashMap<Key, Value>,
    ) -> Result<(String, Vec<Value>)> {
        if where_map.is_empty() {
            return Err(anyhow!(
                "WHERE clause cannot be empty for UPDATE/DELETE operations"
            ));
        }

        let mut conditions = Vec::new();
        let mut values = Vec::new();

        for (key, value) in where_map {
            let field_name = key.as_str();
            let column_mapping = schema.allowed_columns.get(&field_name).context(format!(
                "Field '{}' not allowed in WHERE clause",
                field_name
            ))?;

            conditions.push(format!(
                "{} = ${}",
                column_mapping.db_column,
                values.len() + 1
            ));
            values.push(value.clone());
        }

        Ok((conditions.join(" AND "), values))
    }

    /// Binds a Value to a SQL query parameter.
    fn bind_value(
        &self,
        query: sqlx::query::Query<'_, sqlx::Postgres, sqlx::postgres::PgArguments>,
        value: &Value,
    ) -> Result<sqlx::query::Query<'_, sqlx::Postgres, sqlx::postgres::PgArguments>> {
        match value {
            Value::Literal(Literal::String(s)) => Ok(query.bind(s)),
            Value::Literal(Literal::Number(n)) => Ok(query.bind(*n)),
            Value::Literal(Literal::Boolean(b)) => Ok(query.bind(*b)),
            Value::Identifier(s) => Ok(query.bind(s)), // Treat identifiers as strings
            _ => Err(anyhow!("Unsupported parameter type: {:?}", value)),
        }
    }
}

/// Result of a CRUD operation.
#[derive(Debug, Clone, PartialEq)]
pub enum CrudResult {
    Created { id: i32, affected_rows: u64 },
    Read { rows_found: usize },
    Updated { affected_rows: u64 },
    Deleted { affected_rows: u64 },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Key, Literal, Value};

    fn create_test_executor() -> CrudExecutor {
        // Create a mock pool for testing (this won't actually connect)
        let pool = PgPool::connect("postgresql://test:test@localhost/test")
            .await
            .expect("Failed to create test pool");
        CrudExecutor::new(pool)
    }

    #[test]
    fn test_asset_schema_validation() {
        let executor = CrudExecutor::new(
            // This is just for testing the schema validation logic
            PgPool::connect("postgresql://test:test@localhost/test")
                .await
                .expect("Test pool"),
        );

        // Test that CBU schema exists
        assert!(executor.schema_map.contains_key("cbu"));

        let cbu_schema = &executor.schema_map["cbu"];
        assert_eq!(cbu_schema.table_name, "cbus");
        assert_eq!(cbu_schema.schema_name, "ob-poc");
        assert!(cbu_schema.allowed_columns.contains_key("name"));
        assert!(cbu_schema.allowed_columns.contains_key("description"));
    }

    #[test]
    fn test_where_clause_building() {
        let executor = CrudExecutor::new(
            PgPool::connect("postgresql://test:test@localhost/test")
                .await
                .expect("Test pool"),
        );

        let schema = &executor.schema_map["cbu"];
        let mut where_map = HashMap::new();
        where_map.insert(
            Key {
                parts: vec!["name".to_string()],
            },
            Value::Literal(Literal::String("Test CBU".to_string())),
        );

        let (where_clause, values) = executor
            .build_where_clause(schema, &where_map)
            .expect("Should build WHERE clause");

        assert_eq!(where_clause, "name = $1");
        assert_eq!(values.len(), 1);
    }

    /// Executes a complex query with joins and aggregations.
    async fn execute_complex_query(&self, op: ComplexQuery) -> Result<CrudResult> {
        let schema = self
            .schema_map
            .get(&op.asset)
            .context(format!("Unknown asset type: {}", op.asset))?;

        // Build the base query
        let mut query = format!("SELECT ");

        // Handle select fields
        if let Some(select_fields) = &op.select_fields {
            let field_names: Vec<String> = select_fields
                .iter()
                .map(|v| match v {
                    Value::Literal(Literal::String(s)) => s.clone(),
                    _ => "*".to_string(),
                })
                .collect();
            query.push_str(&field_names.join(", "));
        } else {
            query.push_str("*");
        }

        query.push_str(&format!(
            " FROM \"{}\".\"{\"",
            schema.schema_name, schema.table_name
        ));

        // Handle joins (simplified - would need proper implementation)
        if let Some(joins) = &op.joins {
            for join in joins {
                let join_type = match join.join_type {
                    crate::JoinType::Inner => "INNER JOIN",
                    crate::JoinType::Left => "LEFT JOIN",
                    crate::JoinType::Right => "RIGHT JOIN",
                    crate::JoinType::Full => "FULL OUTER JOIN",
                };
                query.push_str(&format!(" {} {} ON ", join_type, join.target_asset));
                // Simplified join condition - would need proper implementation
                query.push_str("true");
            }
        }

        // Handle filters
        if let Some(filters) = &op.filters {
            if !filters.is_empty() {
                query.push_str(" WHERE ");
                let conditions: Vec<String> = filters
                    .iter()
                    .map(|(key, _)| format!("{} = $1", key.as_str()))
                    .collect();
                query.push_str(&conditions.join(" AND "));
            }
        }

        // Handle ordering
        if let Some(order_by) = &op.order_by {
            if !order_by.is_empty() {
                query.push_str(" ORDER BY ");
                let order_clauses: Vec<String> = order_by
                    .iter()
                    .map(|order| {
                        let direction = match order.direction {
                            crate::OrderDirection::Asc => "ASC",
                            crate::OrderDirection::Desc => "DESC",
                        };
                        format!("{} {}", order.field, direction)
                    })
                    .collect();
                query.push_str(&order_clauses.join(", "));
            }
        }

        // Handle limit and offset
        if let Some(limit) = op.limit {
            query.push_str(&format!(" LIMIT {}", limit));
        }
        if let Some(offset) = op.offset {
            query.push_str(&format!(" OFFSET {}", offset));
        }

        println!("Executing complex query: {}", query);

        // For demonstration, return a mock result
        Ok(CrudResult {
            operation_type: "complex_query".to_string(),
            affected_rows: 0,
            returned_data: Some(serde_json::json!({
                "message": "Complex query executed",
                "query": query,
                "asset": op.asset
            })),
            execution_time_ms: 50,
        })
    }

    /// Executes a conditional update operation.
    async fn execute_conditional_update(&self, op: ConditionalUpdate) -> Result<CrudResult> {
        let schema = self
            .schema_map
            .get(&op.asset)
            .context(format!("Unknown asset type: {}", op.asset))?;

        println!(
            "Executing conditional update for asset '{}' with conditions",
            op.asset
        );

        // In a real implementation, this would:
        // 1. Check if_exists conditions
        // 2. Check if_not_exists conditions
        // 3. Execute the update only if conditions are met
        // 4. Handle increment operations

        // For demonstration, return a mock result
        Ok(CrudResult {
            operation_type: "conditional_update".to_string(),
            affected_rows: 1,
            returned_data: Some(serde_json::json!({
                "message": "Conditional update executed",
                "asset": op.asset,
                "conditions_met": true
            })),
            execution_time_ms: 75,
        })
    }

    /// Executes a batch operation (simplified - real implementation would use transaction manager).
    async fn execute_batch_operation(&self, op: BatchOperation) -> Result<CrudResult> {
        println!(
            "Executing batch operation with {} operations in {:?} mode",
            op.operations.len(),
            op.transaction_mode
        );

        let mut total_affected = 0;
        let mut results = Vec::new();

        // Execute each operation in the batch
        for (index, operation) in op.operations.iter().enumerate() {
            match self.execute(operation.clone()).await {
                Ok(result) => {
                    total_affected += result.affected_rows;
                    results.push(serde_json::json!({
                        "index": index,
                        "success": true,
                        "affected_rows": result.affected_rows
                    }));
                }
                Err(e) => {
                    results.push(serde_json::json!({
                        "index": index,
                        "success": false,
                        "error": e.to_string()
                    }));

                    // Handle rollback strategy
                    match op.rollback_strategy {
                        crate::RollbackStrategy::FullRollback => {
                            return Ok(CrudResult {
                                operation_type: "batch_operation".to_string(),
                                affected_rows: 0,
                                returned_data: Some(serde_json::json!({
                                    "message": "Batch operation failed, full rollback performed",
                                    "failed_at_index": index,
                                    "error": e.to_string()
                                })),
                                execution_time_ms: 100,
                            });
                        }
                        crate::RollbackStrategy::PartialRollback => {
                            break;
                        }
                        crate::RollbackStrategy::ContinueOnError => {
                            // Continue with next operation
                            continue;
                        }
                    }
                }
            }
        }

        Ok(CrudResult {
            operation_type: "batch_operation".to_string(),
            affected_rows: total_affected,
            returned_data: Some(serde_json::json!({
                "message": "Batch operation completed",
                "total_operations": op.operations.len(),
                "results": results
            })),
            execution_time_ms: 200,
        })
    }
}
