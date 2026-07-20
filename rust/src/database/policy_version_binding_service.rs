//! Policy Version Binding Service
//!
//! Stores and retrieves immutable policy binding rows for runtime document decisions.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

/// Immutable runtime policy binding row.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub(crate) struct PolicyVersionBindingRow {
    pub binding_id: Uuid,
    pub subject_kind: String,
    pub subject_id: Uuid,
    pub semos_snapshot_set_id: Uuid,
    pub requirement_profile_fqn: Option<String>,
    pub requirement_profile_snapshot_id: Option<Uuid>,
    pub verification_rule_fqn: Option<String>,
    pub verification_rule_snapshot_id: Option<Uuid>,
    pub acceptance_policy_fqn: Option<String>,
    pub acceptance_policy_snapshot_id: Option<Uuid>,
    pub document_type_registry_version: Option<String>,
    pub extraction_model_version: Option<String>,
    pub policy_effective_at: Option<DateTime<Utc>>,
    pub computed_at: DateTime<Utc>,
    pub computed_by: Option<String>,
    pub metadata: JsonValue,
}

/// Payload for creating a new runtime policy binding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct NewPolicyVersionBinding {
    pub subject_kind: String,
    pub subject_id: Uuid,
    pub semos_snapshot_set_id: Uuid,
    pub requirement_profile_fqn: Option<String>,
    pub requirement_profile_snapshot_id: Option<Uuid>,
    pub verification_rule_fqn: Option<String>,
    pub verification_rule_snapshot_id: Option<Uuid>,
    pub acceptance_policy_fqn: Option<String>,
    pub acceptance_policy_snapshot_id: Option<Uuid>,
    pub document_type_registry_version: Option<String>,
    pub extraction_model_version: Option<String>,
    pub policy_effective_at: Option<DateTime<Utc>>,
    pub computed_by: Option<String>,
    pub metadata: JsonValue,
}

/// Database service for runtime policy version bindings.
#[derive(Clone, Debug)]
pub(crate) struct PolicyVersionBindingService {
    pool: PgPool,
}

impl PolicyVersionBindingService {
    /// Create a new policy version binding service.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let service = PolicyVersionBindingService::new(pool.clone());
    /// ```
    pub(crate) fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get a reference to the underlying connection pool.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let pool = service.pool();
    /// ```
    pub(crate) fn pool(&self) -> &PgPool {
        &self.pool
    }


    /// Insert a new immutable runtime policy binding row.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let row = service.insert_binding(&new_binding).await?;
    /// ```
    pub(crate) async fn insert_binding(
        &self,
        binding: &NewPolicyVersionBinding,
    ) -> Result<PolicyVersionBindingRow> {
        sqlx::query_as::<_, PolicyVersionBindingRow>(
            r#"
            INSERT INTO "ob-poc".policy_version_bindings (
                subject_kind,
                subject_id,
                semos_snapshot_set_id,
                requirement_profile_fqn,
                requirement_profile_snapshot_id,
                verification_rule_fqn,
                verification_rule_snapshot_id,
                acceptance_policy_fqn,
                acceptance_policy_snapshot_id,
                document_type_registry_version,
                extraction_model_version,
                policy_effective_at,
                computed_by,
                metadata
            )
            VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14
            )
            RETURNING
                binding_id,
                subject_kind,
                subject_id,
                semos_snapshot_set_id,
                requirement_profile_fqn,
                requirement_profile_snapshot_id,
                verification_rule_fqn,
                verification_rule_snapshot_id,
                acceptance_policy_fqn,
                acceptance_policy_snapshot_id,
                document_type_registry_version,
                extraction_model_version,
                policy_effective_at,
                computed_at,
                computed_by,
                metadata
            "#,
        )
        .bind(&binding.subject_kind)
        .bind(binding.subject_id)
        .bind(binding.semos_snapshot_set_id)
        .bind(&binding.requirement_profile_fqn)
        .bind(binding.requirement_profile_snapshot_id)
        .bind(&binding.verification_rule_fqn)
        .bind(binding.verification_rule_snapshot_id)
        .bind(&binding.acceptance_policy_fqn)
        .bind(binding.acceptance_policy_snapshot_id)
        .bind(&binding.document_type_registry_version)
        .bind(&binding.extraction_model_version)
        .bind(binding.policy_effective_at)
        .bind(&binding.computed_by)
        .bind(&binding.metadata)
        .fetch_one(&self.pool)
        .await
        .context("Failed to insert policy version binding")
    }

}
