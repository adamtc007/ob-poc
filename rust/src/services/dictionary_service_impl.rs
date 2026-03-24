//! DictionaryService implementation for attribute validation and management

use crate::data_dictionary::{
    AttributeId, DbAttributeDefinition, DictionaryService, SinkConfig, SourceConfig,
};
use crate::services::attribute_identity_service::{
    AttributeIdentityService, ResolvedAttributeIdentity,
};
use async_trait::async_trait;
use sqlx::PgPool;

pub struct DictionaryServiceImpl {
    pool: PgPool,
}

impl DictionaryServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    async fn resolve_attribute_reference(
        &self,
        reference: &str,
    ) -> Result<Option<ResolvedAttributeIdentity>, String> {
        AttributeIdentityService::new(self.pool.clone())
            .resolve_reference(reference)
            .await
            .map_err(|e| e.to_string())
    }

    pub async fn validate_attribute_value_ref(
        &self,
        attribute_ref: &str,
        value: &serde_json::Value,
    ) -> Result<(), String> {
        let resolved = self
            .resolve_attribute_reference(attribute_ref)
            .await?
            .ok_or_else(|| format!("Attribute '{attribute_ref}' not found"))?;

        self.validate_value_against_data_type(
            &resolved.best_display_name(),
            &resolved.best_data_type(),
            value,
        )
    }

    fn validate_value_against_data_type(
        &self,
        attribute_name: &str,
        data_type: &str,
        value: &serde_json::Value,
    ) -> Result<(), String> {
        match data_type {
            "string" | "text" => {
                if !value.is_string() {
                    return Err(format!("Expected string for attribute {}", attribute_name));
                }
            }
            "number" | "numeric" | "integer" | "decimal" => {
                if !value.is_number() {
                    return Err(format!("Expected number for attribute {}", attribute_name));
                }
            }
            "boolean" | "bool" => {
                if !value.is_boolean() {
                    return Err(format!("Expected boolean for attribute {}", attribute_name));
                }
            }
            _ => {}
        }

        Ok(())
    }

    fn definition_from_resolved(
        &self,
        resolved: ResolvedAttributeIdentity,
    ) -> Option<DbAttributeDefinition> {
        let runtime_uuid = resolved.runtime_uuid()?;
        let name = resolved.best_display_name();
        let data_type = resolved.best_data_type();
        let description = resolved.description;
        let group_id = resolved.group_id;
        let domain = resolved.domain;

        let source_config = resolved
            .source_config
            .and_then(|value| serde_json::from_value::<SourceConfig>(value).ok())
            .map(sqlx::types::Json);
        let sink_config = resolved
            .sink_config
            .and_then(|value| serde_json::from_value::<SinkConfig>(value).ok())
            .map(sqlx::types::Json);

        Some(DbAttributeDefinition {
            attribute_id: AttributeId::from_uuid(runtime_uuid),
            name,
            long_description: description,
            data_type,
            source_config,
            sink_config,
            group_id,
            domain,
        })
    }
}

#[async_trait]
impl DictionaryService for DictionaryServiceImpl {
    async fn validate_dsl_attributes(&self, dsl: &str) -> Result<Vec<AttributeId>, String> {
        // Parse DSL to find all @attr{...} references, accepting UUIDs, registry ids, and FQNs.
        let attr_pattern = regex::Regex::new(r"@attr\{([^}]+)\}").unwrap();
        let mut attribute_ids = Vec::new();

        for cap in attr_pattern.captures_iter(dsl) {
            if let Some(raw_ref) = cap.get(1) {
                let raw_ref = raw_ref.as_str().trim();
                let resolved = self
                    .resolve_attribute_reference(raw_ref)
                    .await?
                    .ok_or_else(|| format!("Attribute '{raw_ref}' not found"))?;

                let runtime_uuid = resolved.runtime_uuid().ok_or_else(|| {
                    format!(
                        "Attribute '{}' exists in SemOS governance but has no operational registry mapping yet",
                        raw_ref
                    )
                })?;

                attribute_ids.push(AttributeId::from_uuid(runtime_uuid));
            }
        }

        Ok(attribute_ids)
    }

    async fn get_attribute(
        &self,
        attribute_id: &AttributeId,
    ) -> Result<Option<DbAttributeDefinition>, String> {
        let resolved = self
            .resolve_attribute_reference(&attribute_id.to_string())
            .await?;

        Ok(resolved.and_then(|row| self.definition_from_resolved(row)))
    }

    async fn validate_attribute_value(
        &self,
        attribute_id: &AttributeId,
        value: &serde_json::Value,
    ) -> Result<(), String> {
        let resolved = self
            .resolve_attribute_reference(&attribute_id.to_string())
            .await?
            .ok_or_else(|| format!("Attribute {} not found", attribute_id))?;

        self.validate_value_against_data_type(
            &resolved.best_display_name(),
            &resolved.best_data_type(),
            value,
        )
    }
}
