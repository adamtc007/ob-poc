use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value as JsonValue;
use sha2::{Digest, Sha256};
use sqlx::{Executor, FromRow, PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::database::locks::{advisory_xact_lock, lock_key};

/// Persisted canonical row for a derived attribute value.
#[derive(Debug, Clone, FromRow)]
pub struct DerivedValueRow {
    pub id: Uuid,
    pub attr_id: Uuid,
    pub entity_id: Uuid,
    pub entity_type: String,
    pub value: JsonValue,
    pub derivation_spec_fqn: String,
    pub spec_snapshot_id: Uuid,
    pub content_hash: String,
    pub input_values: JsonValue,
    pub inherited_security_label: Option<JsonValue>,
    pub dependency_depth: i32,
    pub evaluated_at: DateTime<Utc>,
    pub stale: bool,
    pub superseded_by: Option<Uuid>,
    pub superseded_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// Insert payload for a canonical derived attribute value.
#[derive(Debug, Clone)]
pub struct DerivedValueRowInput {
    pub id: Uuid,
    pub attr_id: Uuid,
    pub entity_id: Uuid,
    pub entity_type: String,
    pub value: JsonValue,
    pub derivation_spec_fqn: String,
    pub spec_snapshot_id: Uuid,
    pub content_hash: String,
    pub input_values: JsonValue,
    pub inherited_security_label: Option<JsonValue>,
    pub dependency_depth: i32,
    pub evaluated_at: DateTime<Utc>,
    pub stale: bool,
}

/// Persisted lineage row for a derived attribute computation.
#[derive(Debug, Clone, FromRow)]
pub struct DependencyRow {
    pub id: Uuid,
    pub derived_value_id: Uuid,
    pub input_kind: String,
    pub input_attr_id: Uuid,
    pub input_entity_id: Uuid,
    pub input_source_row_id: Option<Uuid>,
    pub dependency_role: Option<String>,
    pub resolved_value: Option<JsonValue>,
    pub created_at: DateTime<Utc>,
}

/// Insert payload for a lineage dependency row.
#[derive(Debug, Clone)]
pub struct DependencyRowInput {
    pub input_kind: String,
    pub input_attr_id: Uuid,
    pub input_entity_id: Uuid,
    pub input_source_row_id: Option<Uuid>,
    pub dependency_role: Option<String>,
    pub resolved_value: Option<JsonValue>,
}

/// Stable input payload used for content-hash computation.
#[derive(Debug, Clone, Serialize)]
pub struct ContentHashInput {
    pub input_kind: String,
    pub input_attr_id: Uuid,
    pub input_entity_id: Uuid,
    pub input_source_row_id: Option<Uuid>,
    pub dependency_role: Option<String>,
    pub resolved_value: JsonValue,
}

/// Batch recompute counters.
#[derive(Debug, Clone, Default)]
pub struct BatchRecomputeResult {
    pub picked: usize,
    pub recomputed: usize,
    pub skipped_already_current: usize,
    pub still_stale: usize,
    pub failed: usize,
}

async fn insert_derived_value_inner<'e, E>(
    executor: E,
    row: &DerivedValueRowInput,
) -> Result<DerivedValueRow>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query_as::<_, DerivedValueRow>(
        r#"
        INSERT INTO "ob-poc".derived_attribute_values (
            id, attr_id, entity_id, entity_type, value, derivation_spec_fqn,
            spec_snapshot_id, content_hash, input_values, inherited_security_label,
            dependency_depth, evaluated_at, stale
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
        RETURNING
            id, attr_id, entity_id, entity_type, value, derivation_spec_fqn,
            spec_snapshot_id, content_hash, input_values, inherited_security_label,
            dependency_depth, evaluated_at, stale, superseded_by, superseded_at, created_at
        "#,
    )
    .bind(row.id)
    .bind(row.attr_id)
    .bind(row.entity_id)
    .bind(&row.entity_type)
    .bind(&row.value)
    .bind(&row.derivation_spec_fqn)
    .bind(row.spec_snapshot_id)
    .bind(&row.content_hash)
    .bind(&row.input_values)
    .bind(&row.inherited_security_label)
    .bind(row.dependency_depth)
    .bind(row.evaluated_at)
    .bind(row.stale)
    .fetch_one(executor)
    .await
    .map_err(Into::into)
}

async fn supersede_current_inner<'e, E>(
    executor: E,
    entity_type: &str,
    entity_id: Uuid,
    attr_id: Uuid,
    new_id: Uuid,
) -> Result<Option<Uuid>>
where
    E: Executor<'e, Database = Postgres>,
{
    let row = sqlx::query_scalar::<_, Uuid>(
        r#"
        UPDATE "ob-poc".derived_attribute_values
        SET superseded_by = $4,
            superseded_at = NOW()
        WHERE entity_type = $1
          AND entity_id = $2
          AND attr_id = $3
          AND superseded_by IS NULL
        RETURNING id
        "#,
    )
    .bind(entity_type)
    .bind(entity_id)
    .bind(attr_id)
    .bind(new_id)
    .fetch_optional(executor)
    .await?;

    Ok(row)
}

async fn mark_stale_inner<'e, E>(executor: E, derived_value_id: Uuid) -> Result<()>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query(
        r#"
        UPDATE "ob-poc".derived_attribute_values
        SET stale = TRUE
        WHERE id = $1
          AND stale = FALSE
        "#,
    )
    .bind(derived_value_id)
    .execute(executor)
    .await?;
    Ok(())
}

/// Insert a canonical derived value row.
///
/// # Examples
/// ```ignore
/// let row = insert_derived_value(&pool, &input).await?;
/// ```
pub async fn insert_derived_value(
    pool: &PgPool,
    row: &DerivedValueRowInput,
) -> Result<DerivedValueRow> {
    insert_derived_value_inner(pool, row).await
}

/// Insert canonical lineage dependencies for a derived value row.
///
/// # Examples
/// ```ignore
/// let rows = insert_dependencies(&pool, derived_value_id, &deps).await?;
/// ```
pub async fn insert_dependencies(
    pool: &PgPool,
    derived_value_id: Uuid,
    deps: &[DependencyRowInput],
) -> Result<Vec<DependencyRow>> {
    let mut rows = Vec::with_capacity(deps.len());
    for dep in deps {
        let row = sqlx::query_as::<_, DependencyRow>(
            r#"
            INSERT INTO "ob-poc".derived_attribute_dependencies (
                derived_value_id, input_kind, input_attr_id, input_entity_id,
                input_source_row_id, dependency_role, resolved_value
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT DO NOTHING
            RETURNING
                id, derived_value_id, input_kind, input_attr_id, input_entity_id,
                input_source_row_id, dependency_role, resolved_value, created_at
            "#,
        )
        .bind(derived_value_id)
        .bind(&dep.input_kind)
        .bind(dep.input_attr_id)
        .bind(dep.input_entity_id)
        .bind(dep.input_source_row_id)
        .bind(&dep.dependency_role)
        .bind(&dep.resolved_value)
        .fetch_optional(pool)
        .await?;

        if let Some(row) = row {
            rows.push(row);
        }
    }
    Ok(rows)
}

/// Supersede the current non-superseded row for a target entity attribute.
///
/// # Examples
/// ```ignore
/// let previous = supersede_current(&pool, "cbu", entity_id, attr_id, new_id).await?;
/// ```
pub async fn supersede_current(
    pool: &PgPool,
    entity_type: &str,
    entity_id: Uuid,
    attr_id: Uuid,
    new_id: Uuid,
) -> Result<Option<Uuid>> {
    supersede_current_inner(pool, entity_type, entity_id, attr_id, new_id).await
}

/// Mark a specific derived row stale.
///
/// # Examples
/// ```ignore
/// mark_stale(&pool, derived_value_id).await?;
/// ```
pub async fn mark_stale(pool: &PgPool, derived_value_id: Uuid) -> Result<()> {
    mark_stale_inner(pool, derived_value_id).await
}

/// Mark all impacted rows stale based on an input attribute/entity pair.
///
/// # Examples
/// ```ignore
/// let count = mark_stale_by_input(&pool, attr_id, entity_id).await?;
/// ```
pub async fn mark_stale_by_input(
    pool: &PgPool,
    input_attr_id: Uuid,
    input_entity_id: Uuid,
) -> Result<u64> {
    let result = sqlx::query(
        r#"
        UPDATE "ob-poc".derived_attribute_values dav
        SET stale = TRUE
        WHERE dav.superseded_by IS NULL
          AND dav.stale = FALSE
          AND EXISTS (
              SELECT 1
              FROM "ob-poc".derived_attribute_dependencies dad
              WHERE dad.derived_value_id = dav.id
                AND dad.input_attr_id = $1
                AND dad.input_entity_id = $2
          )
        "#,
    )
    .bind(input_attr_id)
    .bind(input_entity_id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}

/// Mark all current rows for an old derivation snapshot stale.
///
/// # Examples
/// ```ignore
/// let count = mark_stale_by_spec(&pool, "risk.score", old_snapshot_id).await?;
/// ```
pub async fn mark_stale_by_spec(
    pool: &PgPool,
    spec_fqn: &str,
    old_snapshot_id: Uuid,
) -> Result<u64> {
    let result = sqlx::query(
        r#"
        UPDATE "ob-poc".derived_attribute_values
        SET stale = TRUE
        WHERE derivation_spec_fqn = $1
          AND spec_snapshot_id = $2
          AND superseded_by IS NULL
          AND stale = FALSE
        "#,
    )
    .bind(spec_fqn)
    .bind(old_snapshot_id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}

/// Read the current execution-safe derived row.
///
/// # Examples
/// ```ignore
/// let row = get_current(&pool, "cbu", entity_id, attr_id).await?;
/// ```
pub async fn get_current(
    pool: &PgPool,
    entity_type: &str,
    entity_id: Uuid,
    attr_id: Uuid,
) -> Result<Option<DerivedValueRow>> {
    get_current_inner(pool, entity_type, entity_id, attr_id).await
}

async fn get_current_inner<'e, E>(
    executor: E,
    entity_type: &str,
    entity_id: Uuid,
    attr_id: Uuid,
) -> Result<Option<DerivedValueRow>>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query_as::<_, DerivedValueRow>(
        r#"
        SELECT
            id, attr_id, entity_id, entity_type, value, derivation_spec_fqn,
            spec_snapshot_id, content_hash, input_values, inherited_security_label,
            dependency_depth, evaluated_at, stale, superseded_by, superseded_at, created_at
        FROM "ob-poc".v_derived_current
        WHERE entity_type = $1
          AND entity_id = $2
          AND attr_id = $3
        "#,
    )
    .bind(entity_type)
    .bind(entity_id)
    .bind(attr_id)
    .fetch_optional(executor)
    .await
    .map_err(Into::into)
}

/// Read the latest non-superseded derived row, including stale rows.
///
/// # Examples
/// ```ignore
/// let row = get_latest(&pool, "cbu", entity_id, attr_id).await?;
/// ```
pub async fn get_latest(
    pool: &PgPool,
    entity_type: &str,
    entity_id: Uuid,
    attr_id: Uuid,
) -> Result<Option<DerivedValueRow>> {
    sqlx::query_as::<_, DerivedValueRow>(
        r#"
        SELECT
            id, attr_id, entity_id, entity_type, value, derivation_spec_fqn,
            spec_snapshot_id, content_hash, input_values, inherited_security_label,
            dependency_depth, evaluated_at, stale, superseded_by, superseded_at, created_at
        FROM "ob-poc".v_derived_latest
        WHERE entity_type = $1
          AND entity_id = $2
          AND attr_id = $3
        "#,
    )
    .bind(entity_type)
    .bind(entity_id)
    .bind(attr_id)
    .fetch_optional(pool)
    .await
    .map_err(Into::into)
}

/// Read the deterministic recompute queue.
///
/// # Examples
/// ```ignore
/// let queue = get_recompute_queue(&pool, 100).await?;
/// ```
pub async fn get_recompute_queue(pool: &PgPool, limit: i64) -> Result<Vec<DerivedValueRow>> {
    sqlx::query_as::<_, DerivedValueRow>(
        r#"
        SELECT
            id, attr_id, entity_id, entity_type, value, derivation_spec_fqn,
            spec_snapshot_id, content_hash, input_values, inherited_security_label,
            dependency_depth, evaluated_at, stale, superseded_by, superseded_at, created_at
        FROM "ob-poc".v_derived_recompute_queue
        LIMIT $1
        "#,
    )
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(Into::into)
}

/// Read direct dependency rows for a derived value.
///
/// # Examples
/// ```ignore
/// let deps = get_direct_dependencies(&pool, derived_value_id).await?;
/// ```
pub async fn get_direct_dependencies(
    pool: &PgPool,
    derived_value_id: Uuid,
) -> Result<Vec<DependencyRow>> {
    sqlx::query_as::<_, DependencyRow>(
        r#"
        SELECT
            id, derived_value_id, input_kind, input_attr_id, input_entity_id,
            input_source_row_id, dependency_role, resolved_value, created_at
        FROM "ob-poc".derived_attribute_dependencies
        WHERE derived_value_id = $1
        ORDER BY created_at, id
        "#,
    )
    .bind(derived_value_id)
    .fetch_all(pool)
    .await
    .map_err(Into::into)
}

/// Read the reverse impact set for a source row.
///
/// # Examples
/// ```ignore
/// let rows = get_reverse_impact(&pool, source_row_id).await?;
/// ```
pub async fn get_reverse_impact(
    pool: &PgPool,
    input_source_row_id: Uuid,
) -> Result<Vec<DerivedValueRow>> {
    sqlx::query_as::<_, DerivedValueRow>(
        r#"
        SELECT DISTINCT
            dav.id, dav.attr_id, dav.entity_id, dav.entity_type, dav.value, dav.derivation_spec_fqn,
            dav.spec_snapshot_id, dav.content_hash, dav.input_values, dav.inherited_security_label,
            dav.dependency_depth, dav.evaluated_at, dav.stale, dav.superseded_by, dav.superseded_at, dav.created_at
        FROM "ob-poc".v_derived_latest dav
        JOIN "ob-poc".derived_attribute_dependencies dad
          ON dad.derived_value_id = dav.id
        WHERE dad.input_source_row_id = $1
        ORDER BY dav.evaluated_at DESC
        "#,
    )
    .bind(input_source_row_id)
    .fetch_all(pool)
    .await
    .map_err(Into::into)
}

/// Read the reverse impact set for an input attribute/entity pair.
///
/// # Examples
/// ```ignore
/// let rows = get_entity_scoped_impact(&pool, attr_id, entity_id).await?;
/// ```
pub async fn get_entity_scoped_impact(
    pool: &PgPool,
    input_attr_id: Uuid,
    input_entity_id: Uuid,
) -> Result<Vec<DerivedValueRow>> {
    sqlx::query_as::<_, DerivedValueRow>(
        r#"
        SELECT DISTINCT
            dav.id, dav.attr_id, dav.entity_id, dav.entity_type, dav.value, dav.derivation_spec_fqn,
            dav.spec_snapshot_id, dav.content_hash, dav.input_values, dav.inherited_security_label,
            dav.dependency_depth, dav.evaluated_at, dav.stale, dav.superseded_by, dav.superseded_at, dav.created_at
        FROM "ob-poc".v_derived_latest dav
        JOIN "ob-poc".derived_attribute_dependencies dad
          ON dad.derived_value_id = dav.id
        WHERE dad.input_attr_id = $1
          AND dad.input_entity_id = $2
        ORDER BY dav.evaluated_at DESC
        "#,
    )
    .bind(input_attr_id)
    .bind(input_entity_id)
    .fetch_all(pool)
    .await
    .map_err(Into::into)
}

/// Read the transitive downstream closure for a derived value.
///
/// # Examples
/// ```ignore
/// let rows = get_transitive_closure(&pool, derived_value_id, 10).await?;
/// ```
pub async fn get_transitive_closure(
    pool: &PgPool,
    derived_value_id: Uuid,
    max_depth: i32,
) -> Result<Vec<DerivedValueRow>> {
    if max_depth < 1 {
        return Err(anyhow!("max_depth must be at least 1"));
    }

    sqlx::query_as::<_, DerivedValueRow>(
        r#"
        WITH RECURSIVE closure AS (
            SELECT
                dav.id, dav.attr_id, dav.entity_id, dav.entity_type, dav.value, dav.derivation_spec_fqn,
                dav.spec_snapshot_id, dav.content_hash, dav.input_values, dav.inherited_security_label,
                dav.dependency_depth, dav.evaluated_at, dav.stale, dav.superseded_by, dav.superseded_at,
                dav.created_at, 0::int AS hop_depth
            FROM "ob-poc".derived_attribute_values dav
            WHERE dav.id = $1

            UNION ALL

            SELECT
                child.id, child.attr_id, child.entity_id, child.entity_type, child.value, child.derivation_spec_fqn,
                child.spec_snapshot_id, child.content_hash, child.input_values, child.inherited_security_label,
                child.dependency_depth, child.evaluated_at, child.stale, child.superseded_by, child.superseded_at,
                child.created_at, closure.hop_depth + 1
            FROM closure
            JOIN "ob-poc".derived_attribute_dependencies dad
              ON dad.input_kind = 'derived_value'
             AND dad.input_source_row_id = closure.id
            JOIN "ob-poc".derived_attribute_values child
              ON child.id = dad.derived_value_id
            WHERE closure.hop_depth < $2
        )
        SELECT DISTINCT ON (id)
            id, attr_id, entity_id, entity_type, value, derivation_spec_fqn,
            spec_snapshot_id, content_hash, input_values, inherited_security_label,
            dependency_depth, evaluated_at, stale, superseded_by, superseded_at, created_at
        FROM closure
        WHERE hop_depth > 0
        ORDER BY id, hop_depth
        "#,
    )
    .bind(derived_value_id)
    .bind(max_depth)
    .fetch_all(pool)
    .await
    .map_err(Into::into)
}

/// Compute the deterministic content hash for a derivation attempt.
///
/// # Examples
/// ```ignore
/// let hash = compute_content_hash(spec_snapshot_id, &inputs);
/// ```
pub fn compute_content_hash(spec_snapshot_id: Uuid, sorted_inputs: &[ContentHashInput]) -> String {
    let mut inputs = sorted_inputs.to_vec();
    inputs.sort_by(|left, right| {
        (
            left.input_attr_id,
            left.input_entity_id,
            left.input_kind.as_str(),
            left.dependency_role.as_deref().unwrap_or(""),
            left.input_source_row_id,
        )
            .cmp(&(
                right.input_attr_id,
                right.input_entity_id,
                right.input_kind.as_str(),
                right.dependency_role.as_deref().unwrap_or(""),
                right.input_source_row_id,
            ))
    });

    let payload = serde_json::to_vec(&(spec_snapshot_id, inputs))
        .expect("content-hash payload should always serialize");
    let mut hasher = Sha256::new();
    hasher.update(payload);
    hex::encode(hasher.finalize())
}

/// Acquire the per-target advisory lock for a derivation transaction.
///
/// # Examples
/// ```ignore
/// let mut tx = pool.begin().await?;
/// acquire_derivation_lock(&mut tx, "cbu", entity_id, attr_id).await?;
/// ```
pub async fn acquire_derivation_lock(
    tx: &mut Transaction<'_, Postgres>,
    entity_type: &str,
    entity_id: Uuid,
    attr_id: Uuid,
) -> Result<()> {
    let lock_id = lock_key(
        &format!("derived:{}", entity_type),
        &format!("{entity_id}:{attr_id}"),
    );
    advisory_xact_lock(tx, lock_id).await?;
    Ok(())
}

pub(crate) async fn insert_derived_value_tx(
    tx: &mut Transaction<'_, Postgres>,
    row: &DerivedValueRowInput,
) -> Result<DerivedValueRow> {
    insert_derived_value_inner(&mut **tx, row).await
}

pub(crate) async fn insert_dependencies_tx(
    tx: &mut Transaction<'_, Postgres>,
    derived_value_id: Uuid,
    deps: &[DependencyRowInput],
) -> Result<Vec<DependencyRow>> {
    let mut rows = Vec::with_capacity(deps.len());
    for dep in deps {
        let row = sqlx::query_as::<_, DependencyRow>(
            r#"
            INSERT INTO "ob-poc".derived_attribute_dependencies (
                derived_value_id, input_kind, input_attr_id, input_entity_id,
                input_source_row_id, dependency_role, resolved_value
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT DO NOTHING
            RETURNING
                id, derived_value_id, input_kind, input_attr_id, input_entity_id,
                input_source_row_id, dependency_role, resolved_value, created_at
            "#,
        )
        .bind(derived_value_id)
        .bind(&dep.input_kind)
        .bind(dep.input_attr_id)
        .bind(dep.input_entity_id)
        .bind(dep.input_source_row_id)
        .bind(&dep.dependency_role)
        .bind(&dep.resolved_value)
        .fetch_optional(&mut **tx)
        .await?;

        if let Some(row) = row {
            rows.push(row);
        }
    }
    Ok(rows)
}

pub(crate) async fn supersede_current_tx(
    tx: &mut Transaction<'_, Postgres>,
    entity_type: &str,
    entity_id: Uuid,
    attr_id: Uuid,
    new_id: Uuid,
) -> Result<Option<Uuid>> {
    supersede_current_inner(&mut **tx, entity_type, entity_id, attr_id, new_id).await
}

pub(crate) async fn get_current_tx(
    tx: &mut Transaction<'_, Postgres>,
    entity_type: &str,
    entity_id: Uuid,
    attr_id: Uuid,
) -> Result<Option<DerivedValueRow>> {
    get_current_inner(&mut **tx, entity_type, entity_id, attr_id).await
}

#[cfg(test)]
mod tests {
    use super::{compute_content_hash, ContentHashInput};
    use serde_json::json;
    use uuid::Uuid;

    #[test]
    fn content_hash_is_order_stable() {
        let snapshot_id = Uuid::new_v4();
        let attr_a = Uuid::new_v4();
        let attr_b = Uuid::new_v4();
        let entity_id = Uuid::new_v4();

        let first = vec![
            ContentHashInput {
                input_kind: "observation".to_string(),
                input_attr_id: attr_b,
                input_entity_id: entity_id,
                input_source_row_id: None,
                dependency_role: Some("secondary".to_string()),
                resolved_value: json!(2),
            },
            ContentHashInput {
                input_kind: "observation".to_string(),
                input_attr_id: attr_a,
                input_entity_id: entity_id,
                input_source_row_id: None,
                dependency_role: Some("primary".to_string()),
                resolved_value: json!(1),
            },
        ];
        let second = vec![first[1].clone(), first[0].clone()];

        assert_eq!(
            compute_content_hash(snapshot_id, &first),
            compute_content_hash(snapshot_id, &second)
        );
    }
}
