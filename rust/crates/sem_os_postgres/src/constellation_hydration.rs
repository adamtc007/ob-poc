use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::{Executor, Postgres, Transaction};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct HydrationSlotRow {
    pub entity_id: Option<Uuid>,
    pub record_id: Option<Uuid>,
    pub filter_value: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct HydrationEntityDetail {
    pub entity_id: Uuid,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct HydrationGraphEdge {
    pub from_entity_id: Uuid,
    pub to_entity_id: Uuid,
    pub percentage: Option<f64>,
    pub ownership_type: Option<String>,
    pub depth: usize,
}

fn is_missing_relation_error(error: &sqlx::Error, relation_name: &str) -> bool {
    let relation = format!("\"ob-poc\".{}", relation_name);
    matches!(error, sqlx::Error::Database(db_error)
        if db_error.code().as_deref() == Some("42P01")
            && (db_error.message().contains(&relation)
                || db_error.message().contains(relation_name)))
}

/// Query child CBU rows from structure links for a constellation hydration step.
///
/// # Examples
/// ```rust,no_run
/// # async fn demo(pool: &sqlx::PgPool) -> anyhow::Result<()> {
/// use sem_os_postgres::constellation_hydration::query_child_cbu_rows;
/// use uuid::Uuid;
///
/// let _rows = query_child_cbu_rows(pool, Uuid::new_v4(), Some("relationship_selector"), Some("fund")).await?;
/// # Ok(())
/// # }
/// ```
pub async fn query_child_cbu_rows<'e, E>(
    executor: E,
    parent_cbu_id: Uuid,
    filter_column: Option<&str>,
    filter_value: Option<&str>,
) -> Result<Vec<HydrationSlotRow>>
where
    E: Executor<'e, Database = Postgres>,
{
    let rows = match (filter_column, filter_value) {
        (Some("relationship_selector"), Some(selector)) => {
            sqlx::query_as::<_, (Uuid, String, Option<DateTime<Utc>>)>(
                r#"
                SELECT child_cbu_id, relationship_selector, created_at
                FROM "ob-poc".cbu_structure_links
                WHERE parent_cbu_id = $1
                  AND relationship_selector = $2
                  AND status = 'ACTIVE'
                  AND (effective_from IS NULL OR effective_from <= CURRENT_DATE)
                  AND (effective_to IS NULL OR effective_to >= CURRENT_DATE)
                ORDER BY created_at DESC NULLS LAST, updated_at DESC
                "#,
            )
            .bind(parent_cbu_id)
            .bind(selector)
            .fetch_all(executor)
            .await?
        }
        (Some("relationship_type"), Some(relationship_type)) => {
            sqlx::query_as::<_, (Uuid, String, Option<DateTime<Utc>>)>(
                r#"
                SELECT child_cbu_id, relationship_selector, created_at
                FROM "ob-poc".cbu_structure_links
                WHERE parent_cbu_id = $1
                  AND relationship_type = $2
                  AND status = 'ACTIVE'
                  AND (effective_from IS NULL OR effective_from <= CURRENT_DATE)
                  AND (effective_to IS NULL OR effective_to >= CURRENT_DATE)
                ORDER BY created_at DESC NULLS LAST, updated_at DESC
                "#,
            )
            .bind(parent_cbu_id)
            .bind(relationship_type.replace('-', "_").to_ascii_uppercase())
            .fetch_all(executor)
            .await?
        }
        _ => {
            sqlx::query_as::<_, (Uuid, String, Option<DateTime<Utc>>)>(
                r#"
                SELECT child_cbu_id, relationship_selector, created_at
                FROM "ob-poc".cbu_structure_links
                WHERE parent_cbu_id = $1
                  AND status = 'ACTIVE'
                  AND (effective_from IS NULL OR effective_from <= CURRENT_DATE)
                  AND (effective_to IS NULL OR effective_to >= CURRENT_DATE)
                ORDER BY created_at DESC NULLS LAST, updated_at DESC
                "#,
            )
            .bind(parent_cbu_id)
            .fetch_all(executor)
            .await?
        }
    };

    Ok(rows
        .into_iter()
        .map(|(child_cbu_id, selector, created_at)| HydrationSlotRow {
            entity_id: Some(child_cbu_id),
            record_id: Some(child_cbu_id),
            filter_value: Some(selector),
            created_at,
        })
        .collect())
}

/// Query case rows for a CBU or explicit case id during constellation hydration.
///
/// # Examples
/// ```rust,no_run
/// # async fn demo(pool: &sqlx::PgPool) -> anyhow::Result<()> {
/// use sem_os_postgres::constellation_hydration::query_case_rows;
/// use uuid::Uuid;
///
/// let _rows = query_case_rows(pool, Uuid::new_v4(), None).await?;
/// # Ok(())
/// # }
/// ```
pub async fn query_case_rows<'e, E>(
    executor: E,
    cbu_id: Uuid,
    case_id: Option<Uuid>,
) -> Result<Vec<HydrationSlotRow>>
where
    E: Executor<'e, Database = Postgres>,
{
    let rows = if let Some(case_id) = case_id {
        sqlx::query_as::<_, (Uuid, Option<DateTime<Utc>>)>(
            r#"
            SELECT case_id, opened_at
            FROM "ob-poc".cases
            WHERE case_id = $1
            "#,
        )
        .bind(case_id)
        .fetch_all(executor)
        .await
        .or_else(|err| {
            if is_missing_relation_error(&err, "cases") {
                Ok(Vec::new())
            } else {
                Err(err)
            }
        })?
    } else {
        sqlx::query_as::<_, (Uuid, Option<DateTime<Utc>>)>(
            r#"
            SELECT case_id, opened_at
            FROM "ob-poc".cases
            WHERE cbu_id = $1
            ORDER BY opened_at DESC NULLS LAST
            LIMIT 1
            "#,
        )
        .bind(cbu_id)
        .fetch_all(executor)
        .await
        .or_else(|err| {
            if is_missing_relation_error(&err, "cases") {
                Ok(Vec::new())
            } else {
                Err(err)
            }
        })?
    };

    Ok(rows
        .into_iter()
        .map(|(record_id, created_at)| HydrationSlotRow {
            entity_id: None,
            record_id: Some(record_id),
            filter_value: None,
            created_at,
        })
        .collect())
}

/// Query tollgate rows for a case during constellation hydration.
///
/// # Examples
/// ```rust,no_run
/// # async fn demo(pool: &sqlx::PgPool) -> anyhow::Result<()> {
/// use sem_os_postgres::constellation_hydration::query_tollgate_rows;
/// use uuid::Uuid;
///
/// let _rows = query_tollgate_rows(pool, Some(Uuid::new_v4())).await?;
/// # Ok(())
/// # }
/// ```
pub async fn query_tollgate_rows<'e, E>(
    executor: E,
    case_id: Option<Uuid>,
) -> Result<Vec<HydrationSlotRow>>
where
    E: Executor<'e, Database = Postgres>,
{
    let Some(case_id) = case_id else {
        return Ok(Vec::new());
    };

    let rows = sqlx::query_as::<_, (Uuid, bool, Option<DateTime<Utc>>)>(
        r#"
        SELECT DISTINCT ON (tollgate_id)
            evaluation_id, passed, evaluated_at
        FROM "ob-poc".tollgate_evaluations
        WHERE case_id = $1
        ORDER BY tollgate_id, evaluated_at DESC NULLS LAST
        "#,
    )
    .bind(case_id)
    .fetch_all(executor)
    .await
    .or_else(|err| {
        if is_missing_relation_error(&err, "tollgate_evaluations") {
            Ok(Vec::new())
        } else {
            Err(err)
        }
    })?;

    Ok(rows
        .into_iter()
        .map(|(record_id, passed, created_at)| HydrationSlotRow {
            entity_id: None,
            record_id: Some(record_id),
            filter_value: Some(if passed {
                String::from("passed")
            } else {
                String::from("failed")
            }),
            created_at,
        })
        .collect())
}

/// Query mandate rows for a CBU during constellation hydration.
///
/// # Examples
/// ```rust,no_run
/// # async fn demo(pool: &sqlx::PgPool) -> anyhow::Result<()> {
/// use sem_os_postgres::constellation_hydration::query_mandate_rows;
/// use uuid::Uuid;
///
/// let _rows = query_mandate_rows(pool, Uuid::new_v4()).await?;
/// # Ok(())
/// # }
/// ```
pub async fn query_mandate_rows<'e, E>(executor: E, cbu_id: Uuid) -> Result<Vec<HydrationSlotRow>>
where
    E: Executor<'e, Database = Postgres>,
{
    let rows = sqlx::query_as::<_, (Uuid, String, i32, Option<DateTime<Utc>>)>(
        r#"
        SELECT profile_id, status, version, created_at
        FROM "ob-poc".cbu_trading_profiles
        WHERE cbu_id = $1
        ORDER BY
            CASE WHEN status = 'ACTIVE' THEN 0 ELSE 1 END,
            version DESC,
            created_at DESC NULLS LAST
        "#,
    )
    .bind(cbu_id)
    .fetch_all(executor)
    .await
    .or_else(|err| {
        if is_missing_relation_error(&err, "cbu_trading_profiles") {
            Ok(Vec::new())
        } else {
            Err(err)
        }
    })?;

    Ok(rows
        .into_iter()
        .map(
            |(record_id, status, version, created_at)| HydrationSlotRow {
                entity_id: None,
                record_id: Some(record_id),
                filter_value: Some(format!("{status}:v{version}")),
                created_at,
            },
        )
        .collect())
}

/// Query role-bound entity rows for a CBU during constellation hydration.
///
/// # Examples
/// ```rust,no_run
/// # async fn demo(pool: &sqlx::PgPool) -> anyhow::Result<()> {
/// use sem_os_postgres::constellation_hydration::query_role_entity_rows;
/// use uuid::Uuid;
///
/// let _rows = query_role_entity_rows(pool, Uuid::new_v4(), "management-company").await?;
/// # Ok(())
/// # }
/// ```
pub async fn query_role_entity_rows<'e, E>(
    executor: E,
    cbu_id: Uuid,
    role_name: &str,
) -> Result<Vec<HydrationSlotRow>>
where
    E: Executor<'e, Database = Postgres>,
{
    let rows = sqlx::query_as::<_, (Uuid, Option<DateTime<Utc>>)>(
        r#"
        SELECT cer.entity_id, cer.created_at
        FROM "ob-poc".cbu_entity_roles cer
        JOIN "ob-poc".roles r ON r.role_id = cer.role_id
        WHERE cer.cbu_id = $1
          AND REPLACE(LOWER(r.name), '_', '-') = LOWER($2)
        ORDER BY cer.created_at DESC NULLS LAST
        "#,
    )
    .bind(cbu_id)
    .bind(role_name)
    .fetch_all(executor)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(entity_id, created_at)| HydrationSlotRow {
            entity_id: Some(entity_id),
            record_id: Some(entity_id),
            filter_value: Some(role_name.to_string()),
            created_at,
        })
        .collect())
}

/// Query entity or CBU detail payloads for constellation hydration enrichment.
///
/// # Examples
/// ```rust,no_run
/// # async fn demo(pool: &sqlx::PgPool) -> anyhow::Result<()> {
/// use sem_os_postgres::constellation_hydration::query_entity_details;
/// use uuid::Uuid;
///
/// let mut tx = pool.begin().await?;
/// let ids = vec![Uuid::new_v4()];
/// let _details = query_entity_details(&mut tx, &ids).await?;
/// # Ok(())
/// # }
/// ```
pub async fn query_entity_details(
    tx: &mut Transaction<'_, Postgres>,
    entity_ids: &[Uuid],
) -> Result<Vec<HydrationEntityDetail>> {
    let mut details = HashMap::new();

    for entity_id in entity_ids {
        let detail = sqlx::query_as::<_, (String, Option<String>)>(
            r#"
            SELECT
                e.name,
                COALESCE(et.type_code, e.bods_entity_type, et.name) AS entity_type
            FROM "ob-poc".entities e
            LEFT JOIN "ob-poc".entity_types et
              ON et.entity_type_id = e.entity_type_id
            WHERE e.entity_id = $1
            "#,
        )
        .bind(entity_id)
        .fetch_optional(&mut **tx)
        .await?;
        if let Some((name, entity_type)) = detail {
            details.insert(
                *entity_id,
                serde_json::json!({
                    "name": name,
                    "entity_type": entity_type,
                }),
            );
            continue;
        }

        let cbu_detail = sqlx::query_as::<_, (String, Option<String>)>(
            r#"
            SELECT name, jurisdiction
            FROM "ob-poc".cbus
            WHERE cbu_id = $1
            "#,
        )
        .bind(entity_id)
        .fetch_optional(&mut **tx)
        .await?;
        if let Some((name, jurisdiction)) = cbu_detail {
            details.insert(
                *entity_id,
                serde_json::json!({
                    "name": name,
                    "entity_type": "cbu",
                    "jurisdiction": jurisdiction,
                }),
            );
        }
    }

    Ok(details
        .into_iter()
        .map(|(entity_id, payload)| HydrationEntityDetail { entity_id, payload })
        .collect())
}

/// Query recursive ownership graph rows for constellation hydration.
///
/// # Examples
/// ```rust,no_run
/// # async fn demo(pool: &sqlx::PgPool) -> anyhow::Result<()> {
/// use sem_os_postgres::constellation_hydration::query_entity_graph_rows;
/// use uuid::Uuid;
///
/// let seeds = vec![Uuid::new_v4()];
/// let _rows = query_entity_graph_rows(pool, &seeds, 3).await?;
/// # Ok(())
/// # }
/// ```
pub async fn query_entity_graph_rows(
    pool: &sqlx::PgPool,
    seeds: &[Uuid],
    max_depth: i32,
) -> Result<(Vec<HydrationSlotRow>, Vec<HydrationGraphEdge>)> {
    if seeds.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }

    let rows = sqlx::query_as::<_, (Uuid, Uuid, Option<f64>, Option<String>, i32)>(
        r#"
        WITH RECURSIVE ownership_chain AS (
            SELECT
                er.from_entity_id,
                er.to_entity_id,
                er.percentage::float8,
                COALESCE(er.ownership_type, er.control_type, er.relationship_type) AS edge_type,
                1 AS depth
            FROM "ob-poc".entity_relationships_current er
            WHERE er.to_entity_id = ANY($1)
              AND er.relationship_type IN ('ownership', 'control', 'management')

            UNION ALL

            SELECT
                er.from_entity_id,
                er.to_entity_id,
                er.percentage::float8,
                COALESCE(er.ownership_type, er.control_type, er.relationship_type) AS edge_type,
                oc.depth + 1
            FROM ownership_chain oc
            JOIN "ob-poc".entity_relationships_current er
              ON er.to_entity_id = oc.from_entity_id
            WHERE oc.depth < $2
              AND er.relationship_type IN ('ownership', 'control', 'management')
        )
        SELECT from_entity_id, to_entity_id, percentage, edge_type, depth
        FROM ownership_chain
        "#,
    )
    .bind(seeds)
    .bind(max_depth)
    .fetch_all(pool)
    .await?;

    let mut nodes = seeds
        .iter()
        .copied()
        .map(|entity_id| HydrationSlotRow {
            entity_id: Some(entity_id),
            record_id: Some(entity_id),
            filter_value: None,
            created_at: None,
        })
        .collect::<Vec<_>>();
    let edges = rows
        .into_iter()
        .map(
            |(from_entity_id, to_entity_id, percentage, ownership_type, depth)| {
                nodes.push(HydrationSlotRow {
                    entity_id: Some(from_entity_id),
                    record_id: Some(from_entity_id),
                    filter_value: ownership_type.clone(),
                    created_at: None,
                });
                nodes.push(HydrationSlotRow {
                    entity_id: Some(to_entity_id),
                    record_id: Some(to_entity_id),
                    filter_value: ownership_type.clone(),
                    created_at: None,
                });
                HydrationGraphEdge {
                    from_entity_id,
                    to_entity_id,
                    percentage,
                    ownership_type,
                    depth: depth as usize,
                }
            },
        )
        .collect::<Vec<_>>();

    Ok((nodes, edges))
}

/// Query recursive ownership graph rows inside an open transaction.
///
/// # Examples
/// ```rust,no_run
/// # async fn demo(pool: &sqlx::PgPool) -> anyhow::Result<()> {
/// use sem_os_postgres::constellation_hydration::query_entity_graph_rows_tx;
/// use uuid::Uuid;
///
/// let mut tx = pool.begin().await?;
/// let seeds = vec![Uuid::new_v4()];
/// let _rows = query_entity_graph_rows_tx(&mut tx, &seeds, 5).await?;
/// # Ok(())
/// # }
/// ```
pub async fn query_entity_graph_rows_tx(
    tx: &mut Transaction<'_, Postgres>,
    seeds: &[Uuid],
    max_depth: i32,
) -> Result<(Vec<HydrationSlotRow>, Vec<HydrationGraphEdge>)> {
    if seeds.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }

    let rows = sqlx::query_as::<_, (Uuid, Uuid, Option<f64>, Option<String>, i32)>(
        r#"
        WITH RECURSIVE ownership_chain AS (
            SELECT
                er.from_entity_id,
                er.to_entity_id,
                er.percentage::float8,
                COALESCE(er.ownership_type, er.control_type, er.relationship_type) AS edge_type,
                1 AS depth
            FROM "ob-poc".entity_relationships_current er
            WHERE er.to_entity_id = ANY($1)
              AND er.relationship_type IN ('ownership', 'control', 'management')

            UNION ALL

            SELECT
                er.from_entity_id,
                er.to_entity_id,
                er.percentage::float8,
                COALESCE(er.ownership_type, er.control_type, er.relationship_type) AS edge_type,
                oc.depth + 1
            FROM ownership_chain oc
            JOIN "ob-poc".entity_relationships_current er
              ON er.to_entity_id = oc.from_entity_id
            WHERE oc.depth < $2
              AND er.relationship_type IN ('ownership', 'control', 'management')
        )
        SELECT from_entity_id, to_entity_id, percentage, edge_type, depth
        FROM ownership_chain
        "#,
    )
    .bind(seeds)
    .bind(max_depth)
    .fetch_all(&mut **tx)
    .await?;

    let mut nodes = seeds
        .iter()
        .copied()
        .map(|entity_id| HydrationSlotRow {
            entity_id: Some(entity_id),
            record_id: Some(entity_id),
            filter_value: None,
            created_at: None,
        })
        .collect::<Vec<_>>();
    let edges = rows
        .into_iter()
        .map(
            |(from_entity_id, to_entity_id, percentage, ownership_type, depth)| {
                nodes.push(HydrationSlotRow {
                    entity_id: Some(from_entity_id),
                    record_id: Some(from_entity_id),
                    filter_value: ownership_type.clone(),
                    created_at: None,
                });
                nodes.push(HydrationSlotRow {
                    entity_id: Some(to_entity_id),
                    record_id: Some(to_entity_id),
                    filter_value: ownership_type.clone(),
                    created_at: None,
                });
                HydrationGraphEdge {
                    from_entity_id,
                    to_entity_id,
                    percentage,
                    ownership_type,
                    depth: depth as usize,
                }
            },
        )
        .collect::<Vec<_>>();

    Ok((nodes, edges))
}
