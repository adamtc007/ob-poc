//! Repository for the `shared_fact_versions` table.
//!
//! Versioned fact store for shared atoms — one row per entity × atom × version.
//! The `is_current` flag is maintained by a database trigger (auto-supersede).

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

// ── Row types ────────────────────────────────────────────────────────

/// A persisted shared fact version row.
#[derive(Debug, Clone, FromRow)]
pub struct SharedFactVersionRow {
    pub id: Uuid,
    pub atom_id: Uuid,
    pub entity_id: Uuid,
    pub version: i32,
    pub value: serde_json::Value,
    pub mutated_by_verb: Option<String>,
    pub mutated_by_user: Option<Uuid>,
    pub mutated_at: DateTime<Utc>,
    pub is_current: bool,
}

/// Input for inserting a new fact version.
#[derive(Debug, Clone)]
pub struct InsertFactVersionInput {
    pub atom_id: Uuid,
    pub entity_id: Uuid,
    pub value: serde_json::Value,
    pub mutated_by_verb: Option<String>,
    pub mutated_by_user: Option<Uuid>,
}

/// Summary of a fact version for API responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedFactVersionSummary {
    pub id: Uuid,
    pub atom_id: Uuid,
    pub entity_id: Uuid,
    pub version: i32,
    pub value: serde_json::Value,
    pub mutated_at: DateTime<Utc>,
    pub is_current: bool,
}

impl From<SharedFactVersionRow> for SharedFactVersionSummary {
    fn from(row: SharedFactVersionRow) -> Self {
        Self {
            id: row.id,
            atom_id: row.atom_id,
            entity_id: row.entity_id,
            version: row.version,
            value: row.value,
            mutated_at: row.mutated_at,
            is_current: row.is_current,
        }
    }
}

// ── Insert ───────────────────────────────────────────────────────────

/// Insert a new fact version. Auto-increments the version number.
///
/// The database trigger `trg_shared_fact_version_supersede` automatically
/// clears `is_current` on prior versions for the same (atom_id, entity_id).
pub async fn insert_version(
    pool: &PgPool,
    input: &InsertFactVersionInput,
) -> Result<SharedFactVersionRow> {
    let row = sqlx::query_as::<_, SharedFactVersionRow>(
        r#"
        INSERT INTO "ob-poc".shared_fact_versions (
            atom_id, entity_id, version, value,
            mutated_by_verb, mutated_by_user
        )
        VALUES (
            $1, $2,
            COALESCE(
                (SELECT MAX(version) + 1
                 FROM "ob-poc".shared_fact_versions
                 WHERE atom_id = $1 AND entity_id = $2),
                1
            ),
            $3, $4, $5
        )
        RETURNING id, atom_id, entity_id, version, value,
                  mutated_by_verb, mutated_by_user, mutated_at, is_current
        "#,
    )
    .bind(input.atom_id)
    .bind(input.entity_id)
    .bind(&input.value)
    .bind(&input.mutated_by_verb)
    .bind(input.mutated_by_user)
    .fetch_one(pool)
    .await?;

    Ok(row)
}

// ── Read ─────────────────────────────────────────────────────────────

/// Get the current (latest) version of a shared fact for an entity.
pub async fn get_current_version(
    pool: &PgPool,
    atom_id: Uuid,
    entity_id: Uuid,
) -> Result<Option<SharedFactVersionRow>> {
    let row = sqlx::query_as::<_, SharedFactVersionRow>(
        r#"
        SELECT id, atom_id, entity_id, version, value,
               mutated_by_verb, mutated_by_user, mutated_at, is_current
        FROM "ob-poc".shared_fact_versions
        WHERE atom_id = $1 AND entity_id = $2 AND is_current = true
        "#,
    )
    .bind(atom_id)
    .bind(entity_id)
    .fetch_optional(pool)
    .await?;

    Ok(row)
}

/// Get the current version number (integer) for a shared fact.
/// Returns 0 if no versions exist.
pub async fn current_version_number(pool: &PgPool, atom_id: Uuid, entity_id: Uuid) -> Result<i32> {
    let version = sqlx::query_scalar::<_, Option<i32>>(
        r#"
        SELECT version
        FROM "ob-poc".shared_fact_versions
        WHERE atom_id = $1 AND entity_id = $2 AND is_current = true
        "#,
    )
    .bind(atom_id)
    .bind(entity_id)
    .fetch_one(pool)
    .await?;

    Ok(version.unwrap_or(0))
}

/// Get the full version history for a shared fact (newest first).
pub async fn get_version_history(
    pool: &PgPool,
    atom_id: Uuid,
    entity_id: Uuid,
) -> Result<Vec<SharedFactVersionRow>> {
    let rows = sqlx::query_as::<_, SharedFactVersionRow>(
        r#"
        SELECT id, atom_id, entity_id, version, value,
               mutated_by_verb, mutated_by_user, mutated_at, is_current
        FROM "ob-poc".shared_fact_versions
        WHERE atom_id = $1 AND entity_id = $2
        ORDER BY version DESC
        "#,
    )
    .bind(atom_id)
    .bind(entity_id)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

// ── Convenience ──────────────────────────────────────────────────────

/// Record a shared fact version if the atom exists and is active.
///
/// This is the post-commit hook entry point: after a verb mutates a value
/// that backs a shared atom, call this to create a versioned record.
///
/// Returns `None` if the atom_path is not in the registry or not active.
pub async fn record_if_active(
    pool: &PgPool,
    atom_path: &str,
    entity_id: Uuid,
    value: serde_json::Value,
    verb_fqn: Option<&str>,
    user_id: Option<Uuid>,
) -> Result<Option<SharedFactVersionRow>> {
    // Look up the atom — only active/deprecated atoms get versioned
    let atom = sqlx::query_as::<_, AtomLookupRow>(
        r#"
        SELECT id, lifecycle_status
        FROM "ob-poc".shared_atom_registry
        WHERE atom_path = $1
        "#,
    )
    .bind(atom_path)
    .fetch_optional(pool)
    .await?;

    let atom = match atom {
        Some(a) if a.lifecycle_status == "active" || a.lifecycle_status == "deprecated" => a,
        _ => return Ok(None),
    };

    let input = InsertFactVersionInput {
        atom_id: atom.id,
        entity_id,
        value,
        mutated_by_verb: verb_fqn.map(String::from),
        mutated_by_user: user_id,
    };

    let row = insert_version(pool, &input).await?;
    Ok(Some(row))
}

// ── Propagation ──────────────────────────────────────────────────────

/// Result of staleness propagation (Stages 2-3).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropagationResult {
    /// Number of consumer refs marked stale (Stage 2 — done by SQL trigger).
    pub stale_consumer_count: i64,
    /// Consumer workspaces that were marked stale.
    pub stale_consumers: Vec<StaleConsumerRef>,
}

/// A consumer that was marked stale by propagation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaleConsumerRef {
    pub consumer_workspace: String,
    pub entity_id: Uuid,
    pub held_version: i32,
}

/// Query the propagation result after a fact version INSERT.
///
/// The SQL trigger `trg_propagate_shared_fact_staleness` has already marked
/// consumer refs stale. This function reads back the affected consumers
/// for Stage 3 processing (remediation event creation in Rust).
pub async fn get_propagation_result(
    pool: &PgPool,
    atom_id: Uuid,
    entity_id: Uuid,
) -> Result<PropagationResult> {
    let rows = sqlx::query_as::<_, StaleConsumerRow>(
        r#"
        SELECT consumer_workspace, entity_id, held_version
        FROM "ob-poc".workspace_fact_refs
        WHERE atom_id = $1
          AND entity_id = $2
          AND status = 'stale'
        "#,
    )
    .bind(atom_id)
    .bind(entity_id)
    .fetch_all(pool)
    .await?;

    let stale_consumers: Vec<StaleConsumerRef> = rows
        .into_iter()
        .map(|r| StaleConsumerRef {
            consumer_workspace: r.consumer_workspace,
            entity_id: r.entity_id,
            held_version: r.held_version,
        })
        .collect();

    let count = stale_consumers.len() as i64;
    Ok(PropagationResult {
        stale_consumer_count: count,
        stale_consumers,
    })
}

#[derive(Debug, FromRow)]
struct StaleConsumerRow {
    consumer_workspace: String,
    entity_id: Uuid,
    held_version: i32,
}

#[derive(Debug, FromRow)]
struct AtomLookupRow {
    id: Uuid,
    lifecycle_status: String,
}
