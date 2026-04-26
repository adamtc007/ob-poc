//! External call idempotency envelope.
//!
//! Wraps any verb that calls a third-party system, providing first-run vs.
//! rebuild branching based on the external_call_log.
//!
//! See: docs/architecture/cross-workspace-state-consistency-v0.4.md §4.5, §5.4

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

// ── Types ────────────────────────────────────────────────────────────

/// Provider capability classification for replay behaviour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderCapability {
    Amendable,
    CancelAndRecreate,
    Immutable,
    Manual,
}

impl ProviderCapability {
    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "amendable" => Ok(Self::Amendable),
            "cancel_and_recreate" => Ok(Self::CancelAndRecreate),
            "immutable" => Ok(Self::Immutable),
            "manual" => Ok(Self::Manual),
            other => Err(anyhow::anyhow!("Unknown provider capability: {other}")),
        }
    }
}

/// Result of the idempotency check.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum IdempotencyAction {
    /// No prior call — this is the first execution.
    FirstRun,
    /// Prior call exists with same request hash — skip (no-op).
    Skip { prior_call_id: Uuid },
    /// Prior call exists with different request hash — needs correction.
    NeedsCorrection {
        prior_call_id: Uuid,
        prior_external_ref: Option<String>,
        capability: ProviderCapability,
    },
    /// Provider requires manual intervention.
    ManualIntervention {
        prior_call_id: Uuid,
        prior_external_ref: Option<String>,
    },
}

/// External call log row.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct ExternalCallRow {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub verb_fqn: String,
    pub provider: String,
    pub operation: String,
    pub external_ref: Option<String>,
    pub request_hash: i64,
    pub request_snapshot: Option<serde_json::Value>,
    pub response_snapshot: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub superseded_by: Option<Uuid>,
    pub is_current: bool,
}

// ── Idempotency check ────────────────────────────────────────────────

/// Check whether an external call should be executed, skipped, or amended.
///
/// This is the core idempotency decision function per doc §5.4.
pub async fn check_idempotency(
    pool: &PgPool,
    entity_id: Uuid,
    verb_fqn: &str,
    provider: &str,
    request_hash: i64,
) -> Result<IdempotencyAction> {
    // Look up the current call for this (entity, verb, provider)
    let prior = sqlx::query_as::<_, ExternalCallRow>(
        r#"
        SELECT id, entity_id, verb_fqn, provider, operation, external_ref,
               request_hash, request_snapshot, response_snapshot,
               created_at, superseded_by, is_current
        FROM "ob-poc".external_call_log
        WHERE entity_id = $1 AND verb_fqn = $2 AND provider = $3 AND is_current = true
        "#,
    )
    .bind(entity_id)
    .bind(verb_fqn)
    .bind(provider)
    .fetch_optional(pool)
    .await?;

    let prior = match prior {
        None => return Ok(IdempotencyAction::FirstRun),
        Some(p) => p,
    };

    // Same request hash → skip (already correct)
    if prior.request_hash == request_hash {
        return Ok(IdempotencyAction::Skip {
            prior_call_id: prior.id,
        });
    }

    // Different hash → needs correction. Look up provider capability.
    let capability = lookup_capability(pool, provider, &prior.operation).await?;

    match capability {
        ProviderCapability::Manual => Ok(IdempotencyAction::ManualIntervention {
            prior_call_id: prior.id,
            prior_external_ref: prior.external_ref,
        }),
        cap => Ok(IdempotencyAction::NeedsCorrection {
            prior_call_id: prior.id,
            prior_external_ref: prior.external_ref,
            capability: cap,
        }),
    }
}

// ── Record calls ─────────────────────────────────────────────────────

/// Input for recording an external call.
#[derive(Debug, Clone)]
pub struct RecordCallInput {
    pub entity_id: Uuid,
    pub verb_fqn: String,
    pub provider: String,
    pub operation: String,
    pub external_ref: Option<String>,
    pub request_hash: i64,
    pub request_snapshot: Option<serde_json::Value>,
    pub response_snapshot: Option<serde_json::Value>,
}

/// Record a new external call in the log.
pub async fn record_call(pool: &PgPool, input: &RecordCallInput) -> Result<ExternalCallRow> {
    let row = sqlx::query_as::<_, ExternalCallRow>(
        r#"
        INSERT INTO "ob-poc".external_call_log (
            entity_id, verb_fqn, provider, operation, external_ref,
            request_hash, request_snapshot, response_snapshot
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        RETURNING id, entity_id, verb_fqn, provider, operation, external_ref,
                  request_hash, request_snapshot, response_snapshot,
                  created_at, superseded_by, is_current
        "#,
    )
    .bind(input.entity_id)
    .bind(&input.verb_fqn)
    .bind(&input.provider)
    .bind(&input.operation)
    .bind(&input.external_ref)
    .bind(input.request_hash)
    .bind(&input.request_snapshot)
    .bind(&input.response_snapshot)
    .fetch_one(pool)
    .await?;

    Ok(row)
}

/// Supersede a prior call (mark it non-current, link to the new call).
pub async fn supersede_call(pool: &PgPool, prior_id: Uuid, new_id: Uuid) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE "ob-poc".external_call_log
        SET is_current = false, superseded_by = $2
        WHERE id = $1
        "#,
    )
    .bind(prior_id)
    .bind(new_id)
    .execute(pool)
    .await?;
    Ok(())
}

// ── Provider capability lookup ───────────────────────────────────────

async fn lookup_capability(
    pool: &PgPool,
    provider: &str,
    operation: &str,
) -> Result<ProviderCapability> {
    let cap = sqlx::query_scalar::<_, String>(
        r#"
        SELECT capability
        FROM "ob-poc".provider_capabilities
        WHERE provider = $1 AND operation = $2
        "#,
    )
    .bind(provider)
    .bind(operation)
    .fetch_optional(pool)
    .await?;

    match cap {
        Some(c) => ProviderCapability::parse(&c),
        None => {
            // Default to manual if provider/operation not classified
            Ok(ProviderCapability::Manual)
        }
    }
}
