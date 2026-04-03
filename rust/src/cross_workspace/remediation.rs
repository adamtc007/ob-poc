//! Repository for the `remediation_events` table.
//!
//! Lifecycle entity tracking the resolution of cross-workspace state drift
//! caused by a superseded shared attribute version.
//!
//! FSM: Detected → Replaying → Resolved | Escalated → Resolved | Deferred

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

// ── Types ────────────────────────────────────────────────────────────

/// Remediation event status (FSM states).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemediationStatus {
    Detected,
    Replaying,
    Resolved,
    Escalated,
    Deferred,
}

impl RemediationStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Detected => "detected",
            Self::Replaying => "replaying",
            Self::Resolved => "resolved",
            Self::Escalated => "escalated",
            Self::Deferred => "deferred",
        }
    }

    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "detected" => Ok(Self::Detected),
            "replaying" => Ok(Self::Replaying),
            "resolved" => Ok(Self::Resolved),
            "escalated" => Ok(Self::Escalated),
            "deferred" => Ok(Self::Deferred),
            other => Err(anyhow!("Unknown remediation status: {other}")),
        }
    }
}

/// A persisted remediation event row.
#[derive(Debug, Clone, FromRow)]
pub struct RemediationEventRow {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub source_atom_id: Uuid,
    pub source_workspace: String,
    pub prior_version: i32,
    pub new_version: i32,
    pub affected_workspace: String,
    pub affected_constellation_family: String,
    pub status: String,
    pub failed_at_step: Option<String>,
    pub failure_reason: Option<String>,
    pub deferral_reason: Option<String>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub resolved_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

/// Summary for API responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemediationEventSummary {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub source_atom_path: String,
    pub source_workspace: String,
    pub prior_version: i32,
    pub new_version: i32,
    pub affected_workspace: String,
    pub affected_constellation_family: String,
    pub status: RemediationStatus,
    pub failed_at_step: Option<String>,
    pub failure_reason: Option<String>,
    pub deferral_reason: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Input for creating a new remediation event.
#[derive(Debug, Clone)]
pub struct CreateRemediationInput {
    pub entity_id: Uuid,
    pub source_atom_id: Uuid,
    pub source_workspace: String,
    pub prior_version: i32,
    pub new_version: i32,
    pub affected_workspace: String,
    pub affected_constellation_family: String,
}

// ── Insert ───────────────────────────────────────────────────────────

/// Create a new remediation event (status: detected).
pub async fn create_remediation_event(
    pool: &PgPool,
    input: &CreateRemediationInput,
) -> Result<RemediationEventRow> {
    let row = sqlx::query_as::<_, RemediationEventRow>(
        r#"
        INSERT INTO "ob-poc".remediation_events (
            entity_id, source_atom_id, source_workspace,
            prior_version, new_version,
            affected_workspace, affected_constellation_family
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING id, entity_id, source_atom_id, source_workspace,
                  prior_version, new_version,
                  affected_workspace, affected_constellation_family,
                  status, failed_at_step, failure_reason, deferral_reason,
                  resolved_at, resolved_by, created_at
        "#,
    )
    .bind(input.entity_id)
    .bind(input.source_atom_id)
    .bind(&input.source_workspace)
    .bind(input.prior_version)
    .bind(input.new_version)
    .bind(&input.affected_workspace)
    .bind(&input.affected_constellation_family)
    .fetch_one(pool)
    .await?;

    Ok(row)
}

// ── Read ─────────────────────────────────────────────────────────────

/// List open (non-resolved, non-deferred) remediation events.
pub async fn list_open(
    pool: &PgPool,
    entity_id: Option<Uuid>,
    workspace: Option<&str>,
) -> Result<Vec<RemediationEventSummary>> {
    let rows = sqlx::query_as::<_, RemediationJoinRow>(
        r#"
        SELECT r.id, r.entity_id, a.atom_path AS source_atom_path,
               r.source_workspace, r.prior_version, r.new_version,
               r.affected_workspace, r.affected_constellation_family,
               r.status, r.failed_at_step, r.failure_reason, r.deferral_reason,
               r.created_at
        FROM "ob-poc".remediation_events r
        JOIN "ob-poc".shared_atom_registry a ON a.id = r.source_atom_id
        WHERE r.status NOT IN ('resolved', 'deferred')
          AND ($1::uuid IS NULL OR r.entity_id = $1)
          AND ($2::text IS NULL OR r.affected_workspace = $2)
        ORDER BY r.created_at DESC
        "#,
    )
    .bind(entity_id)
    .bind(workspace)
    .fetch_all(pool)
    .await?;

    rows.into_iter().map(|r| r.into_summary()).collect()
}

/// Get a single remediation event by ID.
pub async fn get_by_id(pool: &PgPool, id: Uuid) -> Result<Option<RemediationEventRow>> {
    let row = sqlx::query_as::<_, RemediationEventRow>(
        r#"
        SELECT id, entity_id, source_atom_id, source_workspace,
               prior_version, new_version,
               affected_workspace, affected_constellation_family,
               status, failed_at_step, failure_reason, deferral_reason,
               resolved_at, resolved_by, created_at
        FROM "ob-poc".remediation_events
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(row)
}

// ── Status transitions ───────────────────────────────────────────────

/// Transition to Replaying.
pub async fn begin_replay(pool: &PgPool, id: Uuid) -> Result<()> {
    transition(pool, id, "detected", "replaying").await
}

/// Transition to Resolved (successful replay).
pub async fn mark_resolved(pool: &PgPool, id: Uuid, resolved_by: Option<Uuid>) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE "ob-poc".remediation_events
        SET status = 'resolved',
            resolved_at = now(),
            resolved_by = $2
        WHERE id = $1 AND status IN ('replaying', 'escalated')
        "#,
    )
    .bind(id)
    .bind(resolved_by)
    .execute(pool)
    .await?;
    Ok(())
}

/// Transition to Escalated (replay failed).
pub async fn escalate(
    pool: &PgPool,
    id: Uuid,
    failed_step: &str,
    failure_reason: &str,
) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE "ob-poc".remediation_events
        SET status = 'escalated',
            failed_at_step = $2,
            failure_reason = $3
        WHERE id = $1 AND status = 'replaying'
        "#,
    )
    .bind(id)
    .bind(failed_step)
    .bind(failure_reason)
    .execute(pool)
    .await?;
    Ok(())
}

/// Transition to Deferred (explicit acceptance of divergence).
pub async fn defer(pool: &PgPool, id: Uuid, reason: &str, deferred_by: Option<Uuid>) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE "ob-poc".remediation_events
        SET status = 'deferred',
            deferral_reason = $2,
            resolved_at = now(),
            resolved_by = $3
        WHERE id = $1 AND status = 'escalated'
        "#,
    )
    .bind(id)
    .bind(reason)
    .bind(deferred_by)
    .execute(pool)
    .await?;
    Ok(())
}

/// Revoke deferral — re-open a deferred remediation.
pub async fn revoke_deferral(pool: &PgPool, id: Uuid) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE "ob-poc".remediation_events
        SET status = 'detected',
            deferral_reason = NULL,
            resolved_at = NULL,
            resolved_by = NULL
        WHERE id = $1 AND status = 'deferred'
        "#,
    )
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

// ── Internal helpers ─────────────────────────────────────────────────

async fn transition(pool: &PgPool, id: Uuid, from: &str, to: &str) -> Result<()> {
    let result = sqlx::query(
        r#"
        UPDATE "ob-poc".remediation_events
        SET status = $3
        WHERE id = $1 AND status = $2
        "#,
    )
    .bind(id)
    .bind(from)
    .bind(to)
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(anyhow!(
            "Remediation event {} not found or not in expected status '{}'",
            id,
            from
        ));
    }
    Ok(())
}

#[derive(Debug, FromRow)]
struct RemediationJoinRow {
    id: Uuid,
    entity_id: Uuid,
    source_atom_path: String,
    source_workspace: String,
    prior_version: i32,
    new_version: i32,
    affected_workspace: String,
    affected_constellation_family: String,
    status: String,
    failed_at_step: Option<String>,
    failure_reason: Option<String>,
    deferral_reason: Option<String>,
    created_at: DateTime<Utc>,
}

impl RemediationJoinRow {
    fn into_summary(self) -> Result<RemediationEventSummary> {
        Ok(RemediationEventSummary {
            id: self.id,
            entity_id: self.entity_id,
            source_atom_path: self.source_atom_path,
            source_workspace: self.source_workspace,
            prior_version: self.prior_version,
            new_version: self.new_version,
            affected_workspace: self.affected_workspace,
            affected_constellation_family: self.affected_constellation_family,
            status: RemediationStatus::parse(&self.status)?,
            failed_at_step: self.failed_at_step,
            failure_reason: self.failure_reason,
            deferral_reason: self.deferral_reason,
            created_at: self.created_at,
        })
    }
}
