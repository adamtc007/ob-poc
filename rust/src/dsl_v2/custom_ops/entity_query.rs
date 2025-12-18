//! Entity Query Custom Operation
//!
//! Provides `entity.query` verb that returns a list of entity refs suitable
//! for batch template execution. Unlike `entity.list` which returns records,
//! this returns a binding that can be consumed by `template.batch :source @binding`.

use anyhow::Result;
use async_trait::async_trait;
use uuid::Uuid;

#[cfg(feature = "database")]
use sqlx::PgPool;

use super::CustomOperation;
use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};

/// Entity query result - a list of (entity_id, name) tuples for batch iteration
#[derive(Debug, Clone, Default)]
pub struct EntityQueryResult {
    /// Entity items: (entity_id, display_name)
    pub items: Vec<(Uuid, String)>,
    /// Entity type queried
    pub entity_type: Option<String>,
    /// Total count (may differ from items.len() if limited)
    pub total_count: usize,
}

impl EntityQueryResult {
    /// Get entity IDs only
    pub fn entity_ids(&self) -> Vec<Uuid> {
        self.items.iter().map(|(id, _)| *id).collect()
    }
}

/// `entity.query` - Query entities for batch processing
///
/// Rationale: Returns a list of entity refs suitable for `template.batch :source @binding`.
/// Unlike `entity.list` which returns JSON records, this returns a structured result
/// that can be iterated by batch execution.
///
/// Example DSL:
/// ```clojure
/// (entity.query :type "fund" :name-like "Allianz%" :jurisdiction "LU" :limit 100 :as @funds)
/// (template.batch :id "onboard-fund-cbu" :source @funds ...)
/// ```
pub struct EntityQueryOp;

#[async_trait]
impl CustomOperation for EntityQueryOp {
    fn domain(&self) -> &'static str {
        "entity"
    }

    fn verb(&self) -> &'static str {
        "query"
    }

    fn rationale(&self) -> &'static str {
        "Returns entity list for batch template iteration, not JSON records"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        // Extract arguments
        let entity_type = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "type")
            .and_then(|a| a.value.as_string());

        let name_like = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "name-like")
            .and_then(|a| a.value.as_string());

        let jurisdiction = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "jurisdiction")
            .and_then(|a| a.value.as_string());

        let limit: i64 = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "limit")
            .and_then(|a| a.value.as_integer())
            .unwrap_or(1000);

        // Build query based on entity type
        // Different entity types have different name columns
        let (table, name_col) = match entity_type {
            Some("fund") | Some("limited_company") => ("entity_limited_companies", "company_name"),
            Some("proper_person") | Some("person") => {
                ("entity_proper_persons", "first_name || ' ' || last_name")
            }
            Some("partnership") => ("entity_partnerships", "partnership_name"),
            Some("trust") => ("entity_trusts", "trust_name"),
            _ => {
                // Default: query base entities table with name from extension tables
                // Use a UNION view approach
                return self
                    .execute_unified_query(name_like, jurisdiction, limit, pool)
                    .await;
            }
        };

        // Build WHERE clauses
        let mut conditions = Vec::new();
        let mut bind_idx = 1;

        if name_like.is_some() {
            conditions.push(format!("{} ILIKE ${}", name_col, bind_idx));
            bind_idx += 1;
        }

        if let Some(_jur) = &jurisdiction {
            conditions.push(format!("jurisdiction = ${}", bind_idx));
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        let query = format!(
            r#"SELECT entity_id, {} as name
               FROM "ob-poc".{}
               {}
               ORDER BY {} ASC
               LIMIT {}"#,
            name_col, table, where_clause, name_col, limit
        );

        // Execute query with dynamic binding
        let rows: Vec<(Uuid, String)> = if name_like.is_some() && jurisdiction.is_some() {
            let pattern = format!("%{}%", name_like.unwrap().replace('%', ""));
            sqlx::query_as(&query)
                .bind(pattern)
                .bind(jurisdiction.unwrap())
                .fetch_all(pool)
                .await?
        } else if name_like.is_some() {
            let pattern = name_like.unwrap();
            // Convert user pattern to SQL ILIKE pattern
            let sql_pattern = if pattern.contains('%') {
                pattern.to_string()
            } else {
                format!("%{}%", pattern)
            };
            sqlx::query_as(&query)
                .bind(sql_pattern)
                .fetch_all(pool)
                .await?
        } else if jurisdiction.is_some() {
            sqlx::query_as(&query)
                .bind(jurisdiction.unwrap())
                .fetch_all(pool)
                .await?
        } else {
            sqlx::query_as(&query).fetch_all(pool).await?
        };

        let result = EntityQueryResult {
            total_count: rows.len(),
            items: rows,
            entity_type: entity_type.map(|s| s.to_string()),
        };

        // Store the result in context for the binding
        // The binding name will be set by the executor based on :as argument
        Ok(ExecutionResult::EntityQuery(result))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::EntityQuery(EntityQueryResult::default()))
    }
}

#[cfg(feature = "database")]
impl EntityQueryOp {
    /// Execute a unified query across all entity types
    async fn execute_unified_query(
        &self,
        name_like: Option<&str>,
        jurisdiction: Option<&str>,
        limit: i64,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        // Use a UNION query to search across all entity extension tables
        let base_query = r#"
            SELECT entity_id, company_name as name, jurisdiction FROM "ob-poc".entity_limited_companies
            UNION ALL
            SELECT entity_id, partnership_name as name, jurisdiction FROM "ob-poc".entity_partnerships
            UNION ALL
            SELECT entity_id, trust_name as name, jurisdiction FROM "ob-poc".entity_trusts
            UNION ALL
            SELECT entity_id, first_name || ' ' || last_name as name, nationality as jurisdiction
            FROM "ob-poc".entity_proper_persons
        "#;

        let mut conditions = Vec::new();

        if name_like.is_some() {
            conditions.push("name ILIKE $1".to_string());
        }

        if jurisdiction.is_some() {
            let idx = if name_like.is_some() { 2 } else { 1 };
            conditions.push(format!("jurisdiction = ${}", idx));
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        let query = format!(
            "SELECT entity_id, name FROM ({}) AS unified {} ORDER BY name ASC LIMIT {}",
            base_query, where_clause, limit
        );

        let rows: Vec<(Uuid, String)> = if name_like.is_some() && jurisdiction.is_some() {
            let pattern = if name_like.unwrap().contains('%') {
                name_like.unwrap().to_string()
            } else {
                format!("%{}%", name_like.unwrap())
            };
            sqlx::query_as(&query)
                .bind(pattern)
                .bind(jurisdiction.unwrap())
                .fetch_all(pool)
                .await?
        } else if name_like.is_some() {
            let pattern = if name_like.unwrap().contains('%') {
                name_like.unwrap().to_string()
            } else {
                format!("%{}%", name_like.unwrap())
            };
            sqlx::query_as(&query).bind(pattern).fetch_all(pool).await?
        } else if jurisdiction.is_some() {
            sqlx::query_as(&query)
                .bind(jurisdiction.unwrap())
                .fetch_all(pool)
                .await?
        } else {
            sqlx::query_as(&query).fetch_all(pool).await?
        };

        let result = EntityQueryResult {
            total_count: rows.len(),
            items: rows,
            entity_type: None,
        };

        Ok(ExecutionResult::EntityQuery(result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entity_query_result_entity_ids() {
        let result = EntityQueryResult {
            items: vec![
                (Uuid::new_v4(), "Test 1".to_string()),
                (Uuid::new_v4(), "Test 2".to_string()),
            ],
            entity_type: Some("fund".to_string()),
            total_count: 2,
        };

        let ids = result.entity_ids();
        assert_eq!(ids.len(), 2);
    }
}
