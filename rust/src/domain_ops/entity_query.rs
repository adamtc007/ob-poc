//! Entity Query Custom Operation
//!
//! Provides `entity.query` verb that returns a list of entity refs suitable
//! for batch template execution. Unlike `entity.list` which returns records,
//! this returns a binding that can be consumed by `template.batch :source @binding`.

use anyhow::Result;
use async_trait::async_trait;
use dsl_runtime_macros::register_custom_op;
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
#[register_custom_op]
pub struct EntityQueryOp;

#[cfg(feature = "database")]
async fn entity_query_impl(
    entity_type: Option<String>,
    name_like: Option<String>,
    jurisdiction: Option<String>,
    limit: i64,
    pool: &PgPool,
) -> Result<EntityQueryResult> {
    let entity_type_ref = entity_type.as_deref();
    let name_like_ref = name_like.as_deref();
    let jurisdiction_ref = jurisdiction.as_deref();

    let (table, name_col) = match entity_type_ref {
        Some("fund") | Some("limited_company") => ("entity_limited_companies", "company_name"),
        Some("proper_person") | Some("person") => {
            ("entity_proper_persons", "first_name || ' ' || last_name")
        }
        Some("partnership") => ("entity_partnerships", "partnership_name"),
        Some("trust") => ("entity_trusts", "trust_name"),
        _ => {
            return execute_unified_entity_query(name_like_ref, jurisdiction_ref, limit, pool)
                .await;
        }
    };

    let mut conditions = Vec::new();
    let mut bind_idx = 1;

    if name_like_ref.is_some() {
        conditions.push(format!("{} ILIKE ${}", name_col, bind_idx));
        bind_idx += 1;
    }

    if jurisdiction_ref.is_some() {
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

    let rows: Vec<(Uuid, String)> = match (name_like_ref, jurisdiction_ref) {
        (Some(name), Some(jur)) => {
            let pattern = format!("%{}%", name.replace('%', ""));
            sqlx::query_as(&query)
                .bind(pattern)
                .bind(jur)
                .fetch_all(pool)
                .await?
        }
        (Some(name), None) => {
            let sql_pattern = if name.contains('%') {
                name.to_string()
            } else {
                format!("%{}%", name)
            };
            sqlx::query_as(&query)
                .bind(sql_pattern)
                .fetch_all(pool)
                .await?
        }
        (None, Some(jur)) => sqlx::query_as(&query).bind(jur).fetch_all(pool).await?,
        (None, None) => sqlx::query_as(&query).fetch_all(pool).await?,
    };

    Ok(EntityQueryResult {
        total_count: rows.len(),
        items: rows,
        entity_type,
    })
}

#[cfg(feature = "database")]
async fn execute_unified_entity_query(
    name_like: Option<&str>,
    jurisdiction: Option<&str>,
    limit: i64,
    pool: &PgPool,
) -> Result<EntityQueryResult> {
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

    let rows: Vec<(Uuid, String)> = match (name_like, jurisdiction) {
        (Some(name), Some(jur)) => {
            let pattern = if name.contains('%') {
                name.to_string()
            } else {
                format!("%{}%", name)
            };
            sqlx::query_as(&query)
                .bind(pattern)
                .bind(jur)
                .fetch_all(pool)
                .await?
        }
        (Some(name), None) => {
            let pattern = if name.contains('%') {
                name.to_string()
            } else {
                format!("%{}%", name)
            };
            sqlx::query_as(&query).bind(pattern).fetch_all(pool).await?
        }
        (None, Some(jur)) => sqlx::query_as(&query).bind(jur).fetch_all(pool).await?,
        (None, None) => sqlx::query_as(&query).fetch_all(pool).await?,
    };

    Ok(EntityQueryResult {
        total_count: rows.len(),
        items: rows,
        entity_type: None,
    })
}

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
        let entity_type = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "type")
            .and_then(|a| a.value.as_string().map(|s| s.to_string()));

        let name_like = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "name-like")
            .and_then(|a| a.value.as_string().map(|s| s.to_string()));

        let jurisdiction = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "jurisdiction")
            .and_then(|a| a.value.as_string().map(|s| s.to_string()));

        let limit: i64 = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "limit")
            .and_then(|a| a.value.as_integer())
            .unwrap_or(1000);

        let result = entity_query_impl(entity_type, name_like, jurisdiction, limit, pool).await?;
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

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        _ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use super::helpers::{json_extract_int_opt, json_extract_string_opt};

        let entity_type = json_extract_string_opt(args, "type");
        let name_like = json_extract_string_opt(args, "name-like");
        let jurisdiction = json_extract_string_opt(args, "jurisdiction");
        let limit = json_extract_int_opt(args, "limit").unwrap_or(1000);
        let result = entity_query_impl(entity_type, name_like, jurisdiction, limit, pool).await?;

        Ok(dsl_runtime::VerbExecutionOutcome::Record(
            serde_json::json!({
                "_type": "entity_query",
                "_debug": format!("{result:?}")
            }),
        ))
    }

    fn is_migrated(&self) -> bool {
        true
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
