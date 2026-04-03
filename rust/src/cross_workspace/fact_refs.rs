//! Repository for the `workspace_fact_refs` table.
//!
//! Consumption-state projection (INV-2) — tracks which shared fact version
//! each consuming workspace last acknowledged or built against.

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

// ── Row types ────────────────────────────────────────────────────────

/// Consumer ref status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConsumerRefStatus {
    Current,
    Stale,
    Deferred,
}

impl ConsumerRefStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Current => "current",
            Self::Stale => "stale",
            Self::Deferred => "deferred",
        }
    }

    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "current" => Ok(Self::Current),
            "stale" => Ok(Self::Stale),
            "deferred" => Ok(Self::Deferred),
            other => Err(anyhow::anyhow!("Unknown consumer ref status: {other}")),
        }
    }
}

/// A persisted workspace fact ref row.
#[derive(Debug, Clone, FromRow)]
pub struct WorkspaceFactRefRow {
    pub id: Uuid,
    pub atom_id: Uuid,
    pub entity_id: Uuid,
    pub consumer_workspace: String,
    pub held_version: i32,
    pub status: String,
    pub stale_since: Option<DateTime<Utc>>,
    pub remediation_id: Option<Uuid>,
}

/// Stale shared fact reference — used for pre-REPL checks and narration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaleSharedFactRef {
    pub atom_id: Uuid,
    pub atom_path: String,
    pub entity_id: Uuid,
    pub held_version: i32,
    pub current_version: i32,
    pub owner_workspace: String,
    pub stale_since: Option<DateTime<Utc>>,
}

// ── Upsert ───────────────────────────────────────────────────────────

/// Upsert a consumer reference. Creates or updates the ref for a
/// (atom, entity, workspace) triple.
pub async fn upsert_ref(
    pool: &PgPool,
    atom_id: Uuid,
    entity_id: Uuid,
    consumer_workspace: &str,
    held_version: i32,
) -> Result<WorkspaceFactRefRow> {
    let row = sqlx::query_as::<_, WorkspaceFactRefRow>(
        r#"
        INSERT INTO "ob-poc".workspace_fact_refs (
            atom_id, entity_id, consumer_workspace, held_version, status
        )
        VALUES ($1, $2, $3, $4, 'current')
        ON CONFLICT (atom_id, entity_id, consumer_workspace) DO UPDATE
            SET held_version = $4,
                status = 'current',
                stale_since = NULL
        RETURNING id, atom_id, entity_id, consumer_workspace,
                  held_version, status, stale_since, remediation_id
        "#,
    )
    .bind(atom_id)
    .bind(entity_id)
    .bind(consumer_workspace)
    .bind(held_version)
    .fetch_one(pool)
    .await?;

    Ok(row)
}

// ── Staleness queries ────────────────────────────────────────────────

/// Check for stale shared fact refs in a given workspace for a given entity.
///
/// Returns enriched stale refs with atom path, current version, and owner
/// workspace for narration purposes.
pub async fn check_staleness_for_entity(
    pool: &PgPool,
    consumer_workspace: &str,
    entity_id: Uuid,
) -> Result<Vec<StaleSharedFactRef>> {
    let rows = sqlx::query_as::<_, StaleRefJoinRow>(
        r#"
        SELECT r.atom_id, a.atom_path, r.entity_id,
               r.held_version, r.stale_since,
               a.owner_workspace,
               COALESCE(
                   (SELECT v.version FROM "ob-poc".shared_fact_versions v
                    WHERE v.atom_id = r.atom_id AND v.entity_id = r.entity_id
                    AND v.is_current = true),
                   r.held_version
               ) AS current_version
        FROM "ob-poc".workspace_fact_refs r
        JOIN "ob-poc".shared_atom_registry a ON a.id = r.atom_id
        WHERE r.consumer_workspace = $1
          AND r.entity_id = $2
          AND r.status = 'stale'
        "#,
    )
    .bind(consumer_workspace)
    .bind(entity_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| StaleSharedFactRef {
            atom_id: r.atom_id,
            atom_path: r.atom_path,
            entity_id: r.entity_id,
            held_version: r.held_version,
            current_version: r.current_version,
            owner_workspace: r.owner_workspace,
            stale_since: r.stale_since,
        })
        .collect())
}

/// List all stale refs for a workspace (across all entities).
pub async fn list_stale_refs(
    pool: &PgPool,
    consumer_workspace: &str,
) -> Result<Vec<StaleSharedFactRef>> {
    let rows = sqlx::query_as::<_, StaleRefJoinRow>(
        r#"
        SELECT r.atom_id, a.atom_path, r.entity_id,
               r.held_version, r.stale_since,
               a.owner_workspace,
               COALESCE(
                   (SELECT v.version FROM "ob-poc".shared_fact_versions v
                    WHERE v.atom_id = r.atom_id AND v.entity_id = r.entity_id
                    AND v.is_current = true),
                   r.held_version
               ) AS current_version
        FROM "ob-poc".workspace_fact_refs r
        JOIN "ob-poc".shared_atom_registry a ON a.id = r.atom_id
        WHERE r.consumer_workspace = $1
          AND r.status = 'stale'
        ORDER BY r.stale_since
        "#,
    )
    .bind(consumer_workspace)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| StaleSharedFactRef {
            atom_id: r.atom_id,
            atom_path: r.atom_path,
            entity_id: r.entity_id,
            held_version: r.held_version,
            current_version: r.current_version,
            owner_workspace: r.owner_workspace,
            stale_since: r.stale_since,
        })
        .collect())
}

// ── Status transitions ───────────────────────────────────────────────

/// Mark a consumer ref as stale (called during staleness propagation).
pub async fn mark_stale(
    pool: &PgPool,
    atom_id: Uuid,
    entity_id: Uuid,
    consumer_workspace: &str,
) -> Result<bool> {
    let result = sqlx::query(
        r#"
        UPDATE "ob-poc".workspace_fact_refs
        SET status = 'stale',
            stale_since = COALESCE(stale_since, now())
        WHERE atom_id = $1
          AND entity_id = $2
          AND consumer_workspace = $3
          AND status = 'current'
        "#,
    )
    .bind(atom_id)
    .bind(entity_id)
    .bind(consumer_workspace)
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

/// Advance a consumer ref to a new version and mark current.
pub async fn advance_to_current(
    pool: &PgPool,
    atom_id: Uuid,
    entity_id: Uuid,
    consumer_workspace: &str,
    new_version: i32,
) -> Result<bool> {
    let result = sqlx::query(
        r#"
        UPDATE "ob-poc".workspace_fact_refs
        SET held_version = $4,
            status = 'current',
            stale_since = NULL
        WHERE atom_id = $1
          AND entity_id = $2
          AND consumer_workspace = $3
        "#,
    )
    .bind(atom_id)
    .bind(entity_id)
    .bind(consumer_workspace)
    .bind(new_version)
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

/// Mark a consumer ref as deferred (explicit acceptance of divergence).
pub async fn mark_deferred(
    pool: &PgPool,
    atom_id: Uuid,
    entity_id: Uuid,
    consumer_workspace: &str,
    remediation_id: Uuid,
) -> Result<bool> {
    let result = sqlx::query(
        r#"
        UPDATE "ob-poc".workspace_fact_refs
        SET status = 'deferred',
            remediation_id = $4
        WHERE atom_id = $1
          AND entity_id = $2
          AND consumer_workspace = $3
          AND status = 'stale'
        "#,
    )
    .bind(atom_id)
    .bind(entity_id)
    .bind(consumer_workspace)
    .bind(remediation_id)
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

// ── Internal row types ───────────────────────────────────────────────

#[derive(Debug, FromRow)]
struct StaleRefJoinRow {
    atom_id: Uuid,
    atom_path: String,
    entity_id: Uuid,
    held_version: i32,
    current_version: i32,
    owner_workspace: String,
    stale_since: Option<DateTime<Utc>>,
}
