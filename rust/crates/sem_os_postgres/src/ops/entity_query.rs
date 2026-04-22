//! `entity.query` verb — YAML-first re-implementation of the legacy
//! `entity_query_ops` plugin op.
//!
//! Returns a list of entity refs suitable for batch template
//! iteration via `template.batch :source @binding`. Unlike
//! `entity.list` (which returns JSON records), this op surfaces a
//! structured `EntityQueryResult` through the
//! `ExecutionResult::EntityQuery` variant. The `SemOsVerbOp` contract
//! only returns `VerbExecutionOutcome` variants, so the op itself
//! projects a lightweight `{_type, total_count, entity_type}` JSON
//! summary — the full `EntityQueryResult` projection is carried via
//! `ExecutionResult::EntityQuery` in the legacy executor path. Phase
//! 5c-migrate pairs this port with the relocation of
//! `EntityQueryResult` into `ob-poc-types::entity_query`.

use anyhow::Result;
use async_trait::async_trait;
use ob_poc_types::entity_query::EntityQueryResult;
use serde_json::{json, Value};
use uuid::Uuid;

use dsl_runtime::domain_ops::helpers::{json_extract_int_opt, json_extract_string_opt};
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::SemOsVerbOp;

async fn entity_query_impl(
    scope: &mut dyn TransactionScope,
    entity_type: Option<String>,
    name_like: Option<String>,
    jurisdiction: Option<String>,
    limit: i64,
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
            return execute_unified_entity_query(scope, name_like_ref, jurisdiction_ref, limit)
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
                .fetch_all(scope.executor())
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
                .fetch_all(scope.executor())
                .await?
        }
        (None, Some(jur)) => sqlx::query_as(&query).bind(jur).fetch_all(scope.executor()).await?,
        (None, None) => sqlx::query_as(&query).fetch_all(scope.executor()).await?,
    };

    Ok(EntityQueryResult {
        total_count: rows.len(),
        items: rows,
        entity_type,
    })
}

async fn execute_unified_entity_query(
    scope: &mut dyn TransactionScope,
    name_like: Option<&str>,
    jurisdiction: Option<&str>,
    limit: i64,
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
                .fetch_all(scope.executor())
                .await?
        }
        (Some(name), None) => {
            let pattern = if name.contains('%') {
                name.to_string()
            } else {
                format!("%{}%", name)
            };
            sqlx::query_as(&query)
                .bind(pattern)
                .fetch_all(scope.executor())
                .await?
        }
        (None, Some(jur)) => sqlx::query_as(&query).bind(jur).fetch_all(scope.executor()).await?,
        (None, None) => sqlx::query_as(&query).fetch_all(scope.executor()).await?,
    };

    Ok(EntityQueryResult {
        total_count: rows.len(),
        items: rows,
        entity_type: None,
    })
}

pub struct Query;

#[async_trait]
impl SemOsVerbOp for Query {
    fn fqn(&self) -> &str {
        "entity.query"
    }
    async fn execute(
        &self,
        args: &Value,
        _ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let entity_type = json_extract_string_opt(args, "type");
        let name_like = json_extract_string_opt(args, "name-like");
        let jurisdiction = json_extract_string_opt(args, "jurisdiction");
        let limit = json_extract_int_opt(args, "limit").unwrap_or(1000);
        let result = entity_query_impl(scope, entity_type, name_like, jurisdiction, limit).await?;
        Ok(VerbExecutionOutcome::Record(json!({
            "_type": "entity_query",
            "items": result.items.iter().map(|(id, name)| json!({
                "id": id.to_string(),
                "name": name,
            })).collect::<Vec<_>>(),
            "entity_type": result.entity_type,
            "total_count": result.total_count,
        })))
    }
}
