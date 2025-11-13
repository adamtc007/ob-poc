//! Dictionary Database Service - CRUD operations for attribute management
//!
//! This module provides database operations for the central attribute dictionary
//! that forms the foundation of our AttributeID-as-Type architecture.

use crate::models::dictionary_models::{
    AttributeSearchCriteria, AttributeValidationRequest, AttributeValidationResult,
    DictionaryAttribute, DictionaryAttributeWithMetadata, DictionaryHealthCheck,
    DictionaryStatistics, DiscoveredAttribute, NewDictionaryAttribute, UpdateDictionaryAttribute,
};
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use serde_json::Value as JsonValue;
use sqlx::{PgPool, Row};
use std::collections::HashMap;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Database service for dictionary operations
#[derive(Clone, Debug)]
pub struct DictionaryDatabaseService {
    pool: PgPool,
}

impl DictionaryDatabaseService {
    /// Create a new dictionary database service
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get a reference to the connection pool
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Create a mock dictionary database service for testing
    pub fn new_mock() -> Self {
        // This creates a mock service that will fail on actual database operations
        // but is useful for type checking and basic initialization
        use std::str::FromStr;
        let mock_url = "postgresql://mock:mock@localhost/mock";
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect_lazy(&mock_url)
            .expect("Mock pool creation failed");

        Self { pool }
    }

    /// Create a new dictionary attribute
    pub async fn create_attribute(
        &self,
        new_attribute: NewDictionaryAttribute,
    ) -> Result<DictionaryAttribute> {
        let attribute_id = Uuid::new_v4();

        let result = sqlx::query_as::<_, DictionaryAttribute>(
            r#"
            INSERT INTO "ob-poc".dictionary (
                attribute_id, name, long_description, group_id, mask, domain, vector, source, sink
            ) VALUES ($1, $2, $3, COALESCE($4, 'default'), COALESCE($5, 'string'), $6, $7, $8, $9)
            RETURNING attribute_id, name, long_description, group_id, mask, domain, vector, source, sink, created_at, updated_at
            "#,
        )
        .bind(attribute_id)
        .bind(&new_attribute.name)
        .bind(&new_attribute.long_description)
        .bind(&new_attribute.group_id)
        .bind(&new_attribute.mask)
        .bind(&new_attribute.domain)
        .bind(&new_attribute.vector)
        .bind(&new_attribute.source)
        .bind(&new_attribute.sink)
        .fetch_one(&self.pool)
        .await
        .context("Failed to create dictionary attribute")?;

        info!(
            "Created dictionary attribute '{}' with ID: {}",
            result.name, result.attribute_id
        );

        Ok(result)
    }

    /// Get attribute by ID
    pub async fn get_by_id(&self, attribute_id: Uuid) -> Result<Option<DictionaryAttribute>> {
        let result = sqlx::query_as::<_, DictionaryAttribute>(
            r#"
            SELECT attribute_id, name, long_description, group_id, mask, domain, vector, source, sink, created_at, updated_at
            FROM "ob-poc".dictionary
            WHERE attribute_id = $1
            "#,
        )
        .bind(attribute_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to fetch dictionary attribute by ID")?;

        Ok(result)
    }

    /// Get attribute by name
    pub async fn get_by_name(&self, name: &str) -> Result<Option<DictionaryAttribute>> {
        let result = sqlx::query_as::<_, DictionaryAttribute>(
            r#"
            SELECT attribute_id, name, long_description, group_id, mask, domain, vector, source, sink, created_at, updated_at
            FROM "ob-poc".dictionary
            WHERE name = $1
            "#,
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to fetch dictionary attribute by name")?;

        Ok(result)
    }

    /// Update an existing attribute
    pub async fn update_attribute(
        &self,
        attribute_id: Uuid,
        updates: UpdateDictionaryAttribute,
    ) -> Result<Option<DictionaryAttribute>> {
        // Use a simpler approach with COALESCE for conditional updates
        if updates.name.is_some()
            || updates.long_description.is_some()
            || updates.group_id.is_some()
            || updates.mask.is_some()
            || updates.domain.is_some()
            || updates.vector.is_some()
            || updates.source.is_some()
            || updates.sink.is_some()
        {
            let result = sqlx::query!(
                r#"
                UPDATE "ob-poc".dictionary
                SET name = COALESCE($1, name),
                    long_description = COALESCE($2, long_description),
                    group_id = COALESCE($3, group_id),
                    mask = COALESCE($4, mask),
                    domain = COALESCE($5, domain),
                    vector = COALESCE($6, vector),
                    source = COALESCE($7, source),
                    sink = COALESCE($8, sink),
                    updated_at = NOW()
                WHERE attribute_id = $9
                RETURNING attribute_id, name, long_description, group_id, mask, domain, vector, source, sink, created_at, updated_at
                "#,
                updates.name as Option<String>,
                updates.long_description as Option<String>,
                updates.group_id as Option<String>,
                updates.mask as Option<String>,
                updates.domain as Option<String>,
                updates.vector as Option<String>,
                updates.source as Option<serde_json::Value>,
                updates.sink as Option<serde_json::Value>,
                attribute_id
            )
            .fetch_optional(&self.pool)
            .await
            .context("Failed to update dictionary attribute")?;

            let result = result.map(|row| DictionaryAttribute {
                attribute_id: row.attribute_id,
                name: row.name,
                long_description: row.long_description,
                group_id: row.group_id,
                mask: row.mask.unwrap_or_else(|| "string".to_string()),
                domain: row.domain,
                vector: row.vector,
                source: row.source,
                sink: row.sink,
                created_at: row.created_at,
                updated_at: row.updated_at,
            });

            if let Some(ref attr) = result {
                info!(
                    "Updated dictionary attribute '{}' with ID: {}",
                    attr.name, attr.attribute_id
                );
            }

            Ok(result)
        } else {
            Ok(self.get_by_id(attribute_id).await?)
        }
    }

    /// Delete an attribute
    pub async fn delete_attribute(&self, attribute_id: Uuid) -> Result<bool> {
        let result = sqlx::query(
            r#"
            DELETE FROM "ob-poc".dictionary
            WHERE attribute_id = $1
            "#,
        )
        .bind(attribute_id)
        .execute(&self.pool)
        .await
        .context("Failed to delete dictionary attribute")?;

        let deleted = result.rows_affected() > 0;

        if deleted {
            info!("Deleted dictionary attribute with ID: {}", attribute_id);
        } else {
            warn!(
                "Attempted to delete non-existent attribute: {}",
                attribute_id
            );
        }

        Ok(deleted)
    }

    /// Search attributes by criteria
    pub async fn search_attributes(
        &self,
        criteria: &AttributeSearchCriteria,
    ) -> Result<Vec<DictionaryAttribute>> {
        // Use a more direct approach with conditional queries
        match (
            &criteria.name_pattern,
            &criteria.group_id,
            &criteria.domain,
            &criteria.mask,
        ) {
            (Some(name_pattern), Some(group_id), Some(domain), Some(mask)) => {
                let rows = sqlx::query!(
                    r#"
                    SELECT attribute_id, name, long_description, group_id, mask, domain, vector, source, sink, created_at, updated_at
                    FROM "ob-poc".dictionary
                    WHERE name ILIKE $1 AND group_id = $2 AND domain = $3 AND mask = $4
                    ORDER BY name
                    LIMIT $5 OFFSET $6
                    "#,
                    format!("%{}%", name_pattern),
                    group_id,
                    domain,
                    mask,
                    criteria.limit.unwrap_or(100) as i32,
                    criteria.offset.unwrap_or(0) as i32
                )
                .fetch_all(&self.pool)
                .await
                .context("Failed to search dictionary attributes by name pattern")?;

                let results = rows
                    .into_iter()
                    .map(|row| DictionaryAttribute {
                        attribute_id: row.attribute_id,
                        name: row.name,
                        long_description: row.long_description,
                        group_id: row.group_id,
                        mask: row.mask.unwrap_or_else(|| "string".to_string()),
                        domain: row.domain,
                        vector: row.vector,
                        source: row.source,
                        sink: row.sink,
                        created_at: row.created_at,
                        updated_at: row.updated_at,
                    })
                    .collect();
                Ok(results)
            }
            (Some(name_pattern), Some(group_id), None, None) => {
                let rows = sqlx::query!(
                    r#"
                    SELECT attribute_id, name, long_description, group_id, mask, domain, vector, source, sink, created_at, updated_at
                    FROM "ob-poc".dictionary
                    WHERE name ILIKE $1 AND group_id = $2
                    ORDER BY name
                    LIMIT $3 OFFSET $4
                    "#,
                    format!("%{}%", name_pattern),
                    group_id,
                    criteria.limit.unwrap_or(100) as i32,
                    criteria.offset.unwrap_or(0) as i32
                )
                .fetch_all(&self.pool)
                .await
                .context("Failed to list all dictionary attributes")?;

                let results = rows
                    .into_iter()
                    .map(|row| DictionaryAttribute {
                        attribute_id: row.attribute_id,
                        name: row.name,
                        long_description: row.long_description,
                        group_id: row.group_id,
                        mask: row.mask.unwrap_or_else(|| "string".to_string()),
                        domain: row.domain,
                        vector: row.vector,
                        source: row.source,
                        sink: row.sink,
                        created_at: row.created_at,
                        updated_at: row.updated_at,
                    })
                    .collect();
                Ok(results)
            }
            (Some(name_pattern), None, None, None) => {
                let rows = sqlx::query!(
                    r#"
                    SELECT attribute_id, name, long_description, group_id, mask, domain, vector, source, sink, created_at, updated_at
                    FROM "ob-poc".dictionary
                    WHERE name ILIKE $1
                    ORDER BY name
                    LIMIT $2 OFFSET $3
                    "#,
                    format!("%{}%", name_pattern),
                    criteria.limit.unwrap_or(50) as i32,
                    criteria.offset.unwrap_or(0) as i32
                )
                .fetch_all(&self.pool)
                .await
                .context("Failed to list all dictionary attributes")?;

                let results = rows
                    .into_iter()
                    .map(|row| DictionaryAttribute {
                        attribute_id: row.attribute_id,
                        name: row.name,
                        long_description: row.long_description,
                        group_id: row.group_id,
                        mask: row.mask.unwrap_or_else(|| "string".to_string()),
                        domain: row.domain,
                        vector: row.vector,
                        source: row.source,
                        sink: row.sink,
                        created_at: row.created_at,
                        updated_at: row.updated_at,
                    })
                    .collect();
                Ok(results)
            }
            _ => {
                // Default case - return all with limit
                let rows = sqlx::query!(
                    r#"
                    SELECT attribute_id, name, long_description, group_id, mask, domain, vector, source, sink, created_at, updated_at
                    FROM "ob-poc".dictionary
                    ORDER BY name
                    LIMIT $1 OFFSET $2
                    "#,
                    criteria.limit.unwrap_or(50) as i32,
                    criteria.offset.unwrap_or(0) as i32
                )
                .fetch_all(&self.pool)
                .await
                .context("Failed to search dictionary attributes by name")?;

                let results = rows
                    .into_iter()
                    .map(|row| DictionaryAttribute {
                        attribute_id: row.attribute_id,
                        name: row.name,
                        long_description: row.long_description,
                        group_id: row.group_id,
                        mask: row.mask.unwrap_or_else(|| "string".to_string()),
                        domain: row.domain,
                        vector: row.vector,
                        source: row.source,
                        sink: row.sink,
                        created_at: row.created_at,
                        updated_at: row.updated_at,
                    })
                    .collect();
                Ok(results)
            }
        }
    }

    /// Get attributes by domain
    pub async fn get_by_domain(&self, domain: &str) -> Result<Vec<DictionaryAttribute>> {
        let results = sqlx::query_as::<_, DictionaryAttribute>(
            r#"
            SELECT attribute_id, name, long_description, group_id, mask, domain, vector, source, sink, created_at, updated_at
            FROM "ob-poc".dictionary
            WHERE domain = $1
            ORDER BY name
            "#,
        )
        .bind(domain)
        .fetch_all(&self.pool)
        .await
        .context("Failed to fetch attributes by domain")?;

        Ok(results)
    }

    /// Get attributes by group
    pub async fn get_by_group(&self, group_id: &str) -> Result<Vec<DictionaryAttribute>> {
        let results = sqlx::query_as::<_, DictionaryAttribute>(
            r#"
            SELECT attribute_id, name, long_description, group_id, mask, domain, vector, source, sink, created_at, updated_at
            FROM "ob-poc".dictionary
            WHERE group_id = $1
            ORDER BY name
            "#,
        )
        .bind(group_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to fetch attributes by group")?;

        Ok(results)
    }

    /// Semantic search for attributes (basic implementation)
    pub async fn semantic_search(
        &self,
        query: &str,
        limit: Option<i32>,
    ) -> Result<Vec<DiscoveredAttribute>> {
        // This is a basic text search implementation
        // In production, you'd integrate with a vector database or use PostgreSQL's full-text search
        let search_limit = limit.unwrap_or(10);

        let results = sqlx::query_as::<_, DictionaryAttribute>(
            r#"
            SELECT attribute_id, name, long_description, group_id, mask, domain, vector, source, sink, created_at, updated_at
            FROM "ob-poc".dictionary
            WHERE name ILIKE $1
               OR long_description ILIKE $1
               OR domain ILIKE $1
            ORDER BY
                CASE
                    WHEN name ILIKE $1 THEN 1
                    WHEN long_description ILIKE $1 THEN 2
                    ELSE 3
                END,
                name
            LIMIT $2
            "#,
        )
        .bind(format!("%{}%", query))
        .bind(search_limit)
        .fetch_all(&self.pool)
        .await
        .context("Failed to perform semantic search")?;

        // Convert to discovered attributes with basic relevance scoring
        let discovered: Vec<DiscoveredAttribute> = results
            .into_iter()
            .enumerate()
            .map(|(index, attr)| {
                let relevance_score = 1.0 - (index as f64 * 0.1); // Simple scoring
                let match_reason = if attr.name.to_lowercase().contains(&query.to_lowercase()) {
                    "Name match".to_string()
                } else if attr.long_description.as_ref().map_or(false, |desc| {
                    desc.to_lowercase().contains(&query.to_lowercase())
                }) {
                    "Description match".to_string()
                } else {
                    "Domain match".to_string()
                };

                DiscoveredAttribute {
                    attribute: attr,
                    relevance_score: relevance_score.max(0.1),
                    match_reason,
                }
            })
            .collect();

        Ok(discovered)
    }

    /// Validate an attribute value (basic implementation)
    pub async fn validate_attribute_value(
        &self,
        request: &AttributeValidationRequest,
    ) -> Result<AttributeValidationResult> {
        // Get the attribute definition
        let attr = self.get_by_id(request.attribute_id).await?;

        let attribute = match attr {
            Some(attr) => attr,
            None => {
                return Ok(AttributeValidationResult {
                    is_valid: false,
                    normalized_value: None,
                    validation_errors: vec![format!(
                        "Attribute {} not found",
                        request.attribute_id
                    )],
                    warnings: vec![],
                });
            }
        };

        // Basic validation based on mask
        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        let normalized_value = Some(request.value.clone());

        match attribute.mask.as_str() {
            "string" => {
                if !request.value.is_string() {
                    errors.push("Value must be a string".to_string());
                }
            }
            "decimal" | "number" => {
                if !request.value.is_number() {
                    errors.push("Value must be a number".to_string());
                }
            }
            "boolean" => {
                if !request.value.is_boolean() {
                    errors.push("Value must be a boolean".to_string());
                }
            }
            "date" => {
                if request.value.is_string() {
                    let date_str = request.value.as_str().unwrap_or("");
                    if chrono::DateTime::parse_from_rfc3339(date_str).is_err() {
                        errors.push("Value must be a valid ISO 8601 date".to_string());
                    }
                } else {
                    errors.push("Date value must be a string in ISO 8601 format".to_string());
                }
            }
            "uuid" => {
                if request.value.is_string() {
                    let uuid_str = request.value.as_str().unwrap_or("");
                    if Uuid::parse_str(uuid_str).is_err() {
                        errors.push("Value must be a valid UUID".to_string());
                    }
                } else {
                    errors.push("UUID value must be a string".to_string());
                }
            }
            _ => {
                warnings.push(format!(
                    "Unknown mask '{}' - validation skipped",
                    attribute.mask
                ));
            }
        }

        Ok(AttributeValidationResult {
            is_valid: errors.is_empty(),
            normalized_value,
            validation_errors: errors,
            warnings,
        })
    }

    /// Get dictionary statistics
    pub async fn get_statistics(&self) -> Result<DictionaryStatistics> {
        // Total attributes
        let total_attributes: i64 =
            sqlx::query_scalar(r#"SELECT COUNT(*) FROM "ob-poc".dictionary"#)
                .fetch_one(&self.pool)
                .await
                .context("Failed to get total attribute count")?;

        // Attributes by domain
        let domain_stats = sqlx::query(
            r#"
            SELECT COALESCE(domain, 'null') as domain, COUNT(*) as count
            FROM "ob-poc".dictionary
            GROUP BY domain
            ORDER BY count DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to get domain statistics")?;

        let mut attributes_by_domain = HashMap::new();
        for row in domain_stats {
            let domain: String = row.get("domain");
            let count: i64 = row.get("count");
            attributes_by_domain.insert(domain, count);
        }

        // Attributes by group
        let group_stats = sqlx::query(
            r#"
            SELECT group_id, COUNT(*) as count
            FROM "ob-poc".dictionary
            GROUP BY group_id
            ORDER BY count DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to get group statistics")?;

        let mut attributes_by_group = HashMap::new();
        for row in group_stats {
            let group_id: String = row.get("group_id");
            let count: i64 = row.get("count");
            attributes_by_group.insert(group_id, count);
        }

        // Attributes by mask
        let mask_stats = sqlx::query(
            r#"
            SELECT mask, COUNT(*) as count
            FROM "ob-poc".dictionary
            GROUP BY mask
            ORDER BY count DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to get mask statistics")?;

        let mut attributes_by_mask = HashMap::new();
        for row in mask_stats {
            let mask: String = row.get("mask");
            let count: i64 = row.get("count");
            attributes_by_mask.insert(mask, count);
        }

        // Recently created attributes
        let recently_created = sqlx::query_as::<_, DictionaryAttribute>(
            r#"
            SELECT attribute_id, name, long_description, group_id, mask, domain, vector, source, sink, created_at, updated_at
            FROM "ob-poc".dictionary
            ORDER BY created_at DESC
            LIMIT 10
            "#
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to get recently created attributes")?;

        Ok(DictionaryStatistics {
            total_attributes,
            attributes_by_domain,
            attributes_by_group,
            attributes_by_mask,
            most_used_attributes: vec![], // Would need usage tracking
            recently_created,
            orphaned_attributes: 0, // Would need cross-reference analysis
        })
    }

    /// Perform health check on dictionary
    pub async fn health_check(&self) -> Result<DictionaryHealthCheck> {
        let total_attributes: i64 =
            sqlx::query_scalar(r#"SELECT COUNT(*) FROM "ob-poc".dictionary"#)
                .fetch_one(&self.pool)
                .await
                .context("Failed to get total attribute count")?;

        let attributes_with_descriptions: i64 = sqlx::query_scalar(
            r#"SELECT COUNT(*) FROM "ob-poc".dictionary WHERE long_description IS NOT NULL AND long_description != ''"#
        )
        .fetch_one(&self.pool)
        .await
        .context("Failed to get attributes with descriptions count")?;

        let missing_domains: i64 =
            sqlx::query_scalar(r#"SELECT COUNT(*) FROM "ob-poc".dictionary WHERE domain IS NULL"#)
                .fetch_one(&self.pool)
                .await
                .context("Failed to get missing domains count")?;

        // Check for duplicate names (shouldn't happen due to unique constraint)
        let duplicate_names = sqlx::query(
            r#"
            SELECT name, COUNT(*) as count
            FROM "ob-poc".dictionary
            GROUP BY name
            HAVING COUNT(*) > 1
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to check for duplicate names")?;

        let duplicate_name_list: Vec<String> = duplicate_names
            .into_iter()
            .map(|row| row.get::<String, _>("name"))
            .collect();

        let mut recommendations = Vec::new();

        if attributes_with_descriptions < total_attributes / 2 {
            recommendations.push(
                "Consider adding descriptions to more attributes for better AI discoverability"
                    .to_string(),
            );
        }

        if missing_domains > 0 {
            recommendations.push(format!(
                "{} attributes are missing domain classification",
                missing_domains
            ));
        }

        if !duplicate_name_list.is_empty() {
            recommendations
                .push("Duplicate attribute names found - this should not happen".to_string());
        }

        let status = if recommendations.is_empty() {
            "healthy".to_string()
        } else if recommendations.len() <= 2 {
            "warning".to_string()
        } else {
            "needs_attention".to_string()
        };

        Ok(DictionaryHealthCheck {
            status,
            total_attributes,
            attributes_with_descriptions,
            attributes_with_validation: 0, // Would need to parse source metadata
            duplicate_names: duplicate_name_list,
            missing_domains,
            recommendations,
            last_check_at: chrono::Utc::now(),
        })
    }

    /// List all attributes with pagination
    pub async fn list_all(
        &self,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<Vec<DictionaryAttribute>> {
        let limit = limit.unwrap_or(50);
        let offset = offset.unwrap_or(0);

        let results = sqlx::query_as::<_, DictionaryAttribute>(
            r#"
            SELECT attribute_id, name, long_description, group_id, mask, domain, vector, source, sink, created_at, updated_at
            FROM "ob-poc".dictionary
            ORDER BY name
            LIMIT $1 OFFSET $2
            "#,
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .context("Failed to list dictionary attributes")?;

        Ok(results)
    }

    /// Check if attribute exists by name
    pub async fn exists_by_name(&self, name: &str) -> Result<bool> {
        let count: i64 =
            sqlx::query_scalar(r#"SELECT COUNT(*) FROM "ob-poc".dictionary WHERE name = $1"#)
                .bind(name)
                .fetch_one(&self.pool)
                .await
                .context("Failed to check if attribute exists")?;

        Ok(count > 0)
    }

    /// Get attribute count
    pub async fn count(&self) -> Result<i64> {
        let count: i64 = sqlx::query_scalar(r#"SELECT COUNT(*) FROM "ob-poc".dictionary"#)
            .fetch_one(&self.pool)
            .await
            .context("Failed to get attribute count")?;

        Ok(count)
    }
}
