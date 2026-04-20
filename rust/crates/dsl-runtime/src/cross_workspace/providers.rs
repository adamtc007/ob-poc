//! Repository for the `provider_capabilities` reference data table.
//!
//! Per-provider, per-operation correction classification for replay behaviour.
//! See: docs/architecture/cross-workspace-state-consistency-v0.4.md §4.6, §6.7

use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use super::idempotency::ProviderCapability;

/// A provider capability row.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct ProviderCapabilityRow {
    pub id: Uuid,
    pub provider: String,
    pub operation: String,
    pub capability: String,
    pub amend_details: Option<serde_json::Value>,
    pub notes: Option<String>,
}

/// Summary for API responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderCapabilitySummary {
    pub provider: String,
    pub operation: String,
    pub capability: ProviderCapability,
    pub notes: Option<String>,
}

/// List all provider capabilities.
pub async fn list_all(pool: &PgPool) -> Result<Vec<ProviderCapabilitySummary>> {
    let rows = sqlx::query_as::<_, ProviderCapabilityRow>(
        r#"
        SELECT id, provider, operation, capability, amend_details, notes
        FROM "ob-poc".provider_capabilities
        ORDER BY provider, operation
        "#,
    )
    .fetch_all(pool)
    .await?;

    rows.into_iter()
        .map(|r| {
            Ok(ProviderCapabilitySummary {
                provider: r.provider,
                operation: r.operation,
                capability: ProviderCapability::parse(&r.capability)?,
                notes: r.notes,
            })
        })
        .collect()
}

/// List capabilities for a specific provider.
pub async fn list_for_provider(
    pool: &PgPool,
    provider: &str,
) -> Result<Vec<ProviderCapabilitySummary>> {
    let rows = sqlx::query_as::<_, ProviderCapabilityRow>(
        r#"
        SELECT id, provider, operation, capability, amend_details, notes
        FROM "ob-poc".provider_capabilities
        WHERE provider = $1
        ORDER BY operation
        "#,
    )
    .bind(provider)
    .fetch_all(pool)
    .await?;

    rows.into_iter()
        .map(|r| {
            Ok(ProviderCapabilitySummary {
                provider: r.provider,
                operation: r.operation,
                capability: ProviderCapability::parse(&r.capability)?,
                notes: r.notes,
            })
        })
        .collect()
}

/// Get capability for a specific provider + operation.
pub async fn get_capability(
    pool: &PgPool,
    provider: &str,
    operation: &str,
) -> Result<Option<ProviderCapability>> {
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

    cap.map(|c| ProviderCapability::parse(&c)).transpose()
}
