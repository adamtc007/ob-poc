//! Attribute identity resolution across legacy dictionary, operational registry,
//! and SemOS-governed attribute definitions.

use anyhow::{Context, Result};
use serde_json::Value as JsonValue;
use sqlx::PgPool;
use uuid::Uuid;

/// Resolved attribute identity across the currently coexisting namespaces.
#[derive(Debug, Clone, sqlx::FromRow)]
pub(crate) struct ResolvedAttributeIdentity {
    pub(crate) registry_uuid: Option<Uuid>,
    pub(crate) legacy_dictionary_uuid: Option<Uuid>,
    pub(crate) registry_id: Option<String>,
    pub(crate) attribute_fqn: Option<String>,
    pub(crate) semos_attribute_fqn: Option<String>,
    pub(crate) display_name: Option<String>,
    pub(crate) description: Option<String>,
    pub(crate) data_type: Option<String>,
    pub(crate) domain: Option<String>,
    pub(crate) source_config: Option<JsonValue>,
    pub(crate) sink_config: Option<JsonValue>,
    pub(crate) group_id: Option<String>,
}

impl ResolvedAttributeIdentity {
    pub(crate) fn runtime_uuid(&self) -> Option<Uuid> {
        self.registry_uuid.or(self.legacy_dictionary_uuid)
    }

    pub(crate) fn best_display_name(&self) -> String {
        self.display_name
            .clone()
            .or_else(|| self.registry_id.clone())
            .or_else(|| self.semos_attribute_fqn.clone())
            .or_else(|| self.attribute_fqn.clone())
            .unwrap_or_else(|| "unknown-attribute".to_string())
    }

    pub(crate) fn best_data_type(&self) -> String {
        self.data_type
            .clone()
            .unwrap_or_else(|| "string".to_string())
    }
}

/// Shared resolver for attribute identity during the reconciliation period.
#[derive(Debug, Clone)]
pub(crate) struct AttributeIdentityService {
    pool: PgPool,
}

impl AttributeIdentityService {
    pub(crate) fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub(crate) async fn resolve_reference(
        &self,
        reference: &str,
    ) -> Result<Option<ResolvedAttributeIdentity>> {
        let trimmed = reference.trim();

        let row = sqlx::query_as::<_, ResolvedAttributeIdentity>(
            r#"
            WITH candidate AS (
                SELECT
                    NULL::uuid AS registry_uuid,
                    NULL::uuid AS legacy_dictionary_uuid,
                    NULL::text AS registry_id,
                    sad.fqn AS attribute_fqn,
                    sad.fqn AS semos_attribute_fqn,
                    sad.attr_name AS display_name,
                    sad.description AS description,
                    sad.data_type AS data_type,
                    sad.domain AS domain,
                    NULL::jsonb AS source_config,
                    NULL::jsonb AS sink_config,
                    NULL::varchar AS group_id,
                    2 AS precedence
                FROM sem_reg.v_active_attribute_defs sad
                WHERE sad.fqn = $1

                UNION ALL

                SELECT
                    ar.uuid AS registry_uuid,
                    d.attribute_id AS legacy_dictionary_uuid,
                    ar.id AS registry_id,
                    NULL::text AS attribute_fqn,
                    ar.metadata #>> '{sem_os,attribute_fqn}' AS semos_attribute_fqn,
                    COALESCE(ar.display_name, d.name) AS display_name,
                    d.long_description AS description,
                    COALESCE(ar.value_type, d.mask) AS data_type,
                    COALESCE(ar.domain, d.domain) AS domain,
                    d.source AS source_config,
                    d.sink AS sink_config,
                    COALESCE(ar.group_id, d.group_id) AS group_id,
                    CASE
                        WHEN ar.uuid::text = $1 OR d.attribute_id::text = $1 THEN 0
                        WHEN ar.id = $1 THEN 1
                        WHEN ar.metadata #>> '{sem_os,attribute_fqn}' = $1 THEN 2
                        WHEN EXISTS (
                            SELECT 1
                            FROM jsonb_array_elements_text(COALESCE(ar.metadata #> '{sem_os,aliases}', '[]'::jsonb)) alias(value)
                            WHERE alias.value = $1
                        ) THEN 2
                        WHEN LOWER(ar.display_name) = LOWER($1) THEN 3
                        WHEN d.name = $1 THEN 4
                        ELSE 10
                    END AS precedence
                FROM "ob-poc".attribute_registry ar
                FULL OUTER JOIN "ob-poc".dictionary d
                    ON d.attribute_id = ar.uuid
                WHERE ar.uuid::text = $1
                   OR ar.id = $1
                   OR ar.metadata #>> '{sem_os,attribute_fqn}' = $1
                   OR EXISTS (
                        SELECT 1
                        FROM jsonb_array_elements_text(COALESCE(ar.metadata #> '{sem_os,aliases}', '[]'::jsonb)) alias(value)
                        WHERE alias.value = $1
                   )
                   OR LOWER(ar.display_name) = LOWER($1)
                   OR d.attribute_id::text = $1
                   OR d.name = $1
            )
            SELECT
                registry_uuid,
                legacy_dictionary_uuid,
                registry_id,
                attribute_fqn,
                semos_attribute_fqn,
                display_name,
                description,
                data_type,
                domain,
                source_config,
                sink_config,
                group_id
            FROM candidate
            ORDER BY precedence
            LIMIT 1
            "#,
        )
        .bind(trimmed)
        .fetch_optional(&self.pool)
        .await
        .with_context(|| format!("Failed to resolve attribute reference '{trimmed}'"))?;

        Ok(row)
    }

    pub(crate) async fn resolve_runtime_uuid(&self, reference: &str) -> Result<Option<Uuid>> {
        Ok(self
            .resolve_reference(reference)
            .await?
            .and_then(|resolved| resolved.runtime_uuid()))
    }
}
