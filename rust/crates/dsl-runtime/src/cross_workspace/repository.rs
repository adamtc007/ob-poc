//! Repository for the `shared_atom_registry` table.
//!
//! Follows the same pattern as `derived_attributes/repository.rs` — thin
//! async wrappers around typed `sqlx` queries.

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use super::types::{
    LifecycleTransitionResult, RegisterSharedAtomInput, SharedAtomDef, SharedAtomLifecycle,
    SharedAtomSummary, SharedAtomValidation,
};

// ── Row types (DB-mapped) ────────────────────────────────────────────

#[derive(Debug, Clone, FromRow)]
struct SharedAtomRow {
    id: Uuid,
    atom_path: String,
    display_name: String,
    owner_workspace: String,
    owner_constellation_family: String,
    lifecycle_status: String,
    validation_rule: Option<serde_json::Value>,
    created_at: DateTime<Utc>,
    activated_at: Option<DateTime<Utc>>,
    updated_at: DateTime<Utc>,
}

impl SharedAtomRow {
    fn into_def(self) -> Result<SharedAtomDef> {
        let lifecycle_status = parse_lifecycle(&self.lifecycle_status)?;
        let validation_rule = self
            .validation_rule
            .map(serde_json::from_value::<SharedAtomValidation>)
            .transpose()
            .map_err(|e| anyhow!("Invalid validation_rule JSON: {e}"))?;

        Ok(SharedAtomDef {
            id: self.id,
            atom_path: self.atom_path,
            display_name: self.display_name,
            owner_workspace: self.owner_workspace,
            owner_constellation_family: self.owner_constellation_family,
            lifecycle_status,
            validation_rule,
            created_at: self.created_at,
            activated_at: self.activated_at,
            updated_at: self.updated_at,
        })
    }
}

fn parse_lifecycle(s: &str) -> Result<SharedAtomLifecycle> {
    match s {
        "draft" => Ok(SharedAtomLifecycle::Draft),
        "active" => Ok(SharedAtomLifecycle::Active),
        "deprecated" => Ok(SharedAtomLifecycle::Deprecated),
        "retired" => Ok(SharedAtomLifecycle::Retired),
        other => Err(anyhow!("Unknown lifecycle_status: {other}")),
    }
}

fn lifecycle_to_str(l: SharedAtomLifecycle) -> &'static str {
    match l {
        SharedAtomLifecycle::Draft => "draft",
        SharedAtomLifecycle::Active => "active",
        SharedAtomLifecycle::Deprecated => "deprecated",
        SharedAtomLifecycle::Retired => "retired",
    }
}

// ── Insert ───────────────────────────────────────────────────────────

/// Register a new shared atom. Enters `Draft` state.
///
/// Returns error if `atom_path` already exists (UNIQUE constraint).
pub async fn insert_shared_atom(
    pool: &PgPool,
    input: &RegisterSharedAtomInput,
) -> Result<SharedAtomDef> {
    let validation_json = input
        .validation_rule
        .as_ref()
        .map(serde_json::to_value)
        .transpose()?;

    let row = sqlx::query_as::<_, SharedAtomRow>(
        r#"
        INSERT INTO "ob-poc".shared_atom_registry (
            atom_path, display_name, owner_workspace,
            owner_constellation_family, validation_rule
        )
        VALUES ($1, $2, $3, $4, $5)
        RETURNING id, atom_path, display_name, owner_workspace,
                  owner_constellation_family, lifecycle_status,
                  validation_rule, created_at, activated_at, updated_at
        "#,
    )
    .bind(&input.atom_path)
    .bind(&input.display_name)
    .bind(&input.owner_workspace)
    .bind(&input.owner_constellation_family)
    .bind(&validation_json)
    .fetch_one(pool)
    .await?;

    row.into_def()
}

/// Upsert a shared atom from YAML seed data. If already present, skip.
pub async fn upsert_from_seed(
    pool: &PgPool,
    input: &RegisterSharedAtomInput,
) -> Result<SharedAtomDef> {
    let validation_json = input
        .validation_rule
        .as_ref()
        .map(serde_json::to_value)
        .transpose()?;

    let row = sqlx::query_as::<_, SharedAtomRow>(
        r#"
        INSERT INTO "ob-poc".shared_atom_registry (
            atom_path, display_name, owner_workspace,
            owner_constellation_family, validation_rule
        )
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (atom_path) DO UPDATE
            SET updated_at = now()
        RETURNING id, atom_path, display_name, owner_workspace,
                  owner_constellation_family, lifecycle_status,
                  validation_rule, created_at, activated_at, updated_at
        "#,
    )
    .bind(&input.atom_path)
    .bind(&input.display_name)
    .bind(&input.owner_workspace)
    .bind(&input.owner_constellation_family)
    .bind(&validation_json)
    .fetch_one(pool)
    .await?;

    row.into_def()
}

// ── Read ─────────────────────────────────────────────────────────────

/// Get a shared atom by its dot-notation path (e.g., `entity.lei`).
pub async fn get_by_path(pool: &PgPool, atom_path: &str) -> Result<Option<SharedAtomDef>> {
    let row = sqlx::query_as::<_, SharedAtomRow>(
        r#"
        SELECT id, atom_path, display_name, owner_workspace,
               owner_constellation_family, lifecycle_status,
               validation_rule, created_at, activated_at, updated_at
        FROM "ob-poc".shared_atom_registry
        WHERE atom_path = $1
        "#,
    )
    .bind(atom_path)
    .fetch_optional(pool)
    .await?;

    row.map(|r| r.into_def()).transpose()
}

/// Get a shared atom by ID.
pub async fn get_by_id(pool: &PgPool, id: Uuid) -> Result<Option<SharedAtomDef>> {
    let row = sqlx::query_as::<_, SharedAtomRow>(
        r#"
        SELECT id, atom_path, display_name, owner_workspace,
               owner_constellation_family, lifecycle_status,
               validation_rule, created_at, activated_at, updated_at
        FROM "ob-poc".shared_atom_registry
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    row.map(|r| r.into_def()).transpose()
}

/// List all shared atoms, optionally filtered by lifecycle status.
pub async fn list_shared_atoms(
    pool: &PgPool,
    status_filter: Option<SharedAtomLifecycle>,
) -> Result<Vec<SharedAtomSummary>> {
    let rows = match status_filter {
        Some(status) => {
            sqlx::query_as::<_, SharedAtomSummaryRow>(
                r#"
                SELECT id, atom_path, display_name, owner_workspace,
                       lifecycle_status, created_at, activated_at
                FROM "ob-poc".shared_atom_registry
                WHERE lifecycle_status = $1
                ORDER BY atom_path
                "#,
            )
            .bind(lifecycle_to_str(status))
            .fetch_all(pool)
            .await?
        }
        None => {
            sqlx::query_as::<_, SharedAtomSummaryRow>(
                r#"
                SELECT id, atom_path, display_name, owner_workspace,
                       lifecycle_status, created_at, activated_at
                FROM "ob-poc".shared_atom_registry
                ORDER BY atom_path
                "#,
            )
            .fetch_all(pool)
            .await?
        }
    };

    rows.into_iter().map(|r| r.into_summary()).collect()
}

/// List only active atoms (the common query for propagation/discovery).
pub async fn list_active(pool: &PgPool) -> Result<Vec<SharedAtomDef>> {
    let rows = sqlx::query_as::<_, SharedAtomRow>(
        r#"
        SELECT id, atom_path, display_name, owner_workspace,
               owner_constellation_family, lifecycle_status,
               validation_rule, created_at, activated_at, updated_at
        FROM "ob-poc".shared_atom_registry
        WHERE lifecycle_status = 'active'
        ORDER BY atom_path
        "#,
    )
    .fetch_all(pool)
    .await?;

    rows.into_iter().map(|r| r.into_def()).collect()
}

// ── Lifecycle transitions ────────────────────────────────────────────

/// Transition a shared atom's lifecycle state.
///
/// Validates the transition is allowed by the FSM. Returns error if
/// the current state doesn't permit the target transition.
pub async fn transition_lifecycle(
    pool: &PgPool,
    atom_id: Uuid,
    target: SharedAtomLifecycle,
) -> Result<LifecycleTransitionResult> {
    let current = get_by_id(pool, atom_id)
        .await?
        .ok_or_else(|| anyhow!("Shared atom {atom_id} not found"))?;

    if !current.lifecycle_status.can_transition_to(target) {
        return Err(anyhow!(
            "Cannot transition shared atom '{}' from {:?} to {:?}. Allowed: {:?}",
            current.atom_path,
            current.lifecycle_status,
            target,
            current.lifecycle_status.valid_transitions()
        ));
    }

    let target_str = lifecycle_to_str(target);

    // Set activated_at when transitioning to Active for the first time
    let activated_clause = if target == SharedAtomLifecycle::Active {
        ", activated_at = COALESCE(activated_at, now())"
    } else {
        ""
    };

    let query = format!(
        r#"
        UPDATE "ob-poc".shared_atom_registry
        SET lifecycle_status = $1,
            updated_at = now()
            {activated_clause}
        WHERE id = $2
        "#
    );

    sqlx::query(&query)
        .bind(target_str)
        .bind(atom_id)
        .execute(pool)
        .await?;

    Ok(LifecycleTransitionResult {
        atom_id,
        atom_path: current.atom_path,
        from_status: current.lifecycle_status,
        to_status: target,
    })
}

// ── Internal helper row types ────────────────────────────────────────

#[derive(Debug, Clone, FromRow)]
struct SharedAtomSummaryRow {
    id: Uuid,
    atom_path: String,
    display_name: String,
    owner_workspace: String,
    lifecycle_status: String,
    created_at: DateTime<Utc>,
    activated_at: Option<DateTime<Utc>>,
}

impl SharedAtomSummaryRow {
    fn into_summary(self) -> Result<SharedAtomSummary> {
        Ok(SharedAtomSummary {
            id: self.id,
            atom_path: self.atom_path,
            display_name: self.display_name,
            owner_workspace: self.owner_workspace,
            lifecycle_status: parse_lifecycle(&self.lifecycle_status)?,
            created_at: self.created_at,
            activated_at: self.activated_at,
        })
    }
}
