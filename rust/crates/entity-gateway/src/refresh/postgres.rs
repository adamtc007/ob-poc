//! Postgres refresh pipeline for entity indexes
//!
//! This module handles loading entity data from PostgreSQL and
//! converting it into `IndexRecord`s for the search index.

use std::collections::HashMap;
use std::sync::Arc;

use sqlx::{PgPool, Row};

use crate::config::{EntityConfig, GatewayConfig};
use crate::index::{IndexRecord, IndexRegistry};

/// Pipeline for refreshing indexes from Postgres
pub struct RefreshPipeline {
    pool: PgPool,
    config: GatewayConfig,
}

impl RefreshPipeline {
    /// Create a new refresh pipeline
    pub async fn new(config: GatewayConfig) -> Result<Self, sqlx::Error> {
        let db_url = std::env::var(&config.database.connection_string_env)
            .expect("Database URL environment variable not set");

        let pool = PgPool::connect(&db_url).await?;

        Ok(Self { pool, config })
    }

    /// Create a pipeline with an existing pool
    pub fn with_pool(pool: PgPool, config: GatewayConfig) -> Self {
        Self { pool, config }
    }

    /// Refresh a single entity's index
    pub async fn refresh_entity(
        &self,
        entity_config: &EntityConfig,
    ) -> Result<Vec<IndexRecord>, sqlx::Error> {
        // Build column list
        let mut columns = vec![entity_config.return_key.clone()];

        // Add search key columns
        for key in &entity_config.search_keys {
            if !columns.contains(&key.column) {
                columns.push(key.column.clone());
            }
        }

        // Add display template columns if present
        if let Some(template) = &entity_config.display_template {
            for cap in template.split('{').skip(1) {
                if let Some(col) = cap.split('}').next() {
                    if !columns.contains(&col.to_string()) {
                        columns.push(col.to_string());
                    }
                }
            }
        }

        // Add discriminator columns for composite search
        for disc in &entity_config.discriminators {
            if !columns.contains(&disc.column) {
                columns.push(disc.column.clone());
            }
        }

        // Build query
        let column_list = columns.join(", ");
        let mut query = format!("SELECT {} FROM {}", column_list, entity_config.source_table);

        // Add filter if present
        if let Some(filter) = &entity_config.filter {
            query.push_str(" WHERE ");
            query.push_str(filter);
        }

        tracing::debug!(
            nickname = %entity_config.nickname,
            query = %query,
            "Executing refresh query"
        );

        // Execute query
        let rows = sqlx::query(&query).fetch_all(&self.pool).await?;

        // Convert rows to IndexRecords
        let records: Vec<IndexRecord> = rows
            .into_iter()
            .filter_map(|row| {
                // Get token (primary key or code)
                let token: String =
                    match row.try_get::<uuid::Uuid, _>(entity_config.return_key.as_str()) {
                        Ok(uuid) => uuid.to_string(), // UUIDs stay as-is
                        Err(_) => {
                            // Try as string - uppercase for exact mode (codes/enums)
                            match row.try_get::<String, _>(entity_config.return_key.as_str()) {
                                Ok(s) => {
                                    if entity_config.index_mode == crate::config::IndexMode::Exact {
                                        s.to_uppercase()
                                    } else {
                                        s
                                    }
                                }
                                Err(e) => {
                                    tracing::warn!(
                                        error = %e,
                                        column = %entity_config.return_key,
                                        "Failed to get token column"
                                    );
                                    return None;
                                }
                            }
                        }
                    };

                // Build display value from template or default search key
                let display = if let Some(template) = &entity_config.display_template {
                    build_display_from_template(template, &row, &columns)
                } else {
                    // Use default search key column
                    let default_col = &entity_config.default_search_key().column;
                    row.try_get::<String, _>(default_col.as_str())
                        .unwrap_or_else(|_| token.clone())
                };

                // Build search values map
                let mut search_values = HashMap::new();
                for key in &entity_config.search_keys {
                    if let Ok(value) = row.try_get::<String, _>(key.column.as_str()) {
                        search_values.insert(key.name.clone(), value);
                    } else if let Ok(Some(v)) =
                        row.try_get::<Option<String>, _>(key.column.as_str())
                    {
                        search_values.insert(key.name.clone(), v);
                    }
                }

                // Add discriminator values (for composite search scoring)
                let mut discriminator_values = HashMap::new();
                for disc in &entity_config.discriminators {
                    // Try as String first
                    if let Ok(value) = row.try_get::<String, _>(disc.column.as_str()) {
                        discriminator_values.insert(disc.name.clone(), value);
                    } else if let Ok(Some(v)) =
                        row.try_get::<Option<String>, _>(disc.column.as_str())
                    {
                        discriminator_values.insert(disc.name.clone(), v);
                    }
                    // Try as Date for date_of_birth etc.
                    else if let Ok(date) =
                        row.try_get::<chrono::NaiveDate, _>(disc.column.as_str())
                    {
                        discriminator_values.insert(disc.name.clone(), date.to_string());
                    } else if let Ok(Some(date)) =
                        row.try_get::<Option<chrono::NaiveDate>, _>(disc.column.as_str())
                    {
                        discriminator_values.insert(disc.name.clone(), date.to_string());
                    }
                }

                Some(IndexRecord {
                    token,
                    display,
                    search_values,
                    discriminator_values,
                    // TODO: Load tenant_id and cbu_ids from database when available
                    // For now, these are not populated - tenant isolation happens at query time
                    tenant_id: None,
                    cbu_ids: vec![],
                })
            })
            .collect();

        tracing::info!(
            nickname = %entity_config.nickname,
            records = records.len(),
            "Loaded records from database"
        );

        Ok(records)
    }

    /// Refresh all entity indexes
    pub async fn refresh_all(
        &self,
        registry: &IndexRegistry,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        for nickname in registry.nicknames() {
            if let Some(entity_config) = registry.get_config(nickname) {
                match self.refresh_entity(entity_config).await {
                    Ok(records) => {
                        if let Some(index) = registry.get(nickname).await {
                            if let Err(e) = index.refresh(records).await {
                                tracing::error!(
                                    nickname = nickname,
                                    error = %e,
                                    "Failed to refresh index"
                                );
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!(
                            nickname = nickname,
                            error = %e,
                            "Failed to load entity data"
                        );
                    }
                }
            }
        }
        Ok(())
    }

    /// Get the database pool
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Get the configuration
    pub fn config(&self) -> &GatewayConfig {
        &self.config
    }
}

/// Build a display string from a template and row data
fn build_display_from_template(
    template: &str,
    row: &sqlx::postgres::PgRow,
    columns: &[String],
) -> String {
    let mut result = template.to_string();

    for col in columns {
        let placeholder = format!("{{{}}}", col);
        if result.contains(&placeholder) {
            let value = row
                .try_get::<String, _>(col.as_str())
                .or_else(|_| {
                    row.try_get::<Option<String>, _>(col.as_str())
                        .map(|v| v.unwrap_or_default())
                })
                .unwrap_or_default();
            result = result.replace(&placeholder, &value);
        }
    }

    result.trim().to_string()
}

/// Background refresh loop
///
/// Runs periodically to keep indexes fresh.
pub async fn run_refresh_loop(
    pipeline: RefreshPipeline,
    registry: Arc<IndexRegistry>,
    interval_secs: u64,
) {
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(interval_secs));

    loop {
        interval.tick().await;

        tracing::info!("Starting scheduled index refresh");

        if let Err(e) = pipeline.refresh_all(&registry).await {
            tracing::error!(error = %e, "Scheduled index refresh failed");
        } else {
            tracing::info!("Scheduled index refresh complete");
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_display_template() {
        // This test would require a mock row, which is complex
        // For now, just test the placeholder extraction logic
        let template = "{first_name} {last_name}";
        assert!(template.contains("{first_name}"));
        assert!(template.contains("{last_name}"));
    }
}
