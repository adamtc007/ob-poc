//! DSL State Store Implementation
//!
//! This module provides persistence for DSL state using PostgreSQL as the backing store.
//! It implements the StateStore trait for managing DSL state with full event sourcing capabilities.

use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::{PgPool, Row};
use std::collections::HashMap;
use uuid::Uuid;

use super::{DslState, StateMetadata, StateStore};
use crate::data_dictionary::AttributeId;
use crate::dsl::operations::ExecutableDslOperation as DslOperation;

/// PostgreSQL-backed implementation of the StateStore trait
pub struct PostgresStateStore {
    pool: PgPool,
}

impl PostgresStateStore {
    /// Create a new PostgresStateStore with the given database pool
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Initialize the database schema for state storage
    pub async fn initialize_schema(&self) -> Result<()> {
        // Create the state storage tables if they don't exist
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS dsl_states (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                business_unit_id VARCHAR NOT NULL,
                version BIGINT NOT NULL,
                operations JSONB NOT NULL,
                current_state JSONB NOT NULL,
                metadata JSONB NOT NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

                UNIQUE(business_unit_id, version)
            );

            CREATE INDEX IF NOT EXISTS idx_dsl_states_business_unit
            ON dsl_states(business_unit_id);

            CREATE INDEX IF NOT EXISTS idx_dsl_states_version
            ON dsl_states(business_unit_id, version DESC);

            CREATE INDEX IF NOT EXISTS idx_dsl_states_updated
            ON dsl_states(updated_at DESC);

            CREATE TABLE IF NOT EXISTS dsl_state_snapshots (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                business_unit_id VARCHAR NOT NULL,
                state_id UUID NOT NULL REFERENCES dsl_states(id),
                snapshot_name VARCHAR,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                created_by VARCHAR NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_snapshots_business_unit
            ON dsl_state_snapshots(business_unit_id);
            "#,
        )
        .execute(&self.pool)
        .await
        .context("Failed to initialize DSL state storage schema")?;

        Ok(())
    }

    /// Get the latest state version for a business unit
    async fn get_latest_version(&self, business_unit_id: &str) -> Result<Option<u64>> {
        let row = sqlx::query(
            "SELECT MAX(version) as max_version FROM dsl_states WHERE business_unit_id = $1",
        )
        .bind(business_unit_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.and_then(|r| r.get::<Option<i64>, _>("max_version").map(|v| v as u64)))
    }

    /// Rebuild state from all operations for a business unit
    async fn rebuild_state_from_operations(
        &self,
        business_unit_id: &str,
    ) -> Result<Option<DslState>> {
        // Get all operations for this business unit, ordered by version
        let rows = sqlx::query(
            r#"
            SELECT operations, metadata, version, created_at
            FROM dsl_states
            WHERE business_unit_id = $1
            ORDER BY version ASC
            "#,
        )
        .bind(business_unit_id)
        .fetch_all(&self.pool)
        .await?;

        if rows.is_empty() {
            return Ok(None);
        }

        let mut all_operations = Vec::new();
        let mut latest_metadata: Option<StateMetadata> = None;

        // Collect all operations from all versions
        for row in rows {
            let operations_json: Value = row.get("operations");
            let metadata_json: Value = row.get("metadata");

            // Parse operations
            let operations: Vec<DslOperation> = serde_json::from_value(operations_json)
                .context("Failed to parse operations from database")?;

            // Parse metadata (keep the latest)
            let metadata: StateMetadata = serde_json::from_value(metadata_json)
                .context("Failed to parse metadata from database")?;

            all_operations.extend(operations);
            latest_metadata = Some(metadata);
        }

        if let Some(metadata) = latest_metadata {
            // Rebuild state from scratch using all operations
            let state = DslState::rebuild_from_operations(
                business_unit_id.to_string(),
                all_operations,
                metadata.domain.clone(),
            );
            Ok(Some(state))
        } else {
            Ok(None)
        }
    }
}

#[async_trait]
impl StateStore for PostgresStateStore {
    async fn get_state(&self, business_unit_id: &str) -> Result<Option<DslState>> {
        // Get the latest version of the state
        let row = sqlx::query(
            r#"
            SELECT operations, current_state, metadata, version, created_at, updated_at
            FROM dsl_states
            WHERE business_unit_id = $1
            ORDER BY version DESC
            LIMIT 1
            "#,
        )
        .bind(business_unit_id)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => {
                let operations_json: Value = row.get("operations");
                let current_state_json: Value = row.get("current_state");
                let metadata_json: Value = row.get("metadata");
                let version: i64 = row.get("version");

                // Parse operations
                let operations: Vec<DslOperation> = serde_json::from_value(operations_json)
                    .context("Failed to parse operations from database")?;

                // Parse current state
                let current_state: HashMap<AttributeId, Value> =
                    serde_json::from_value(current_state_json)
                        .context("Failed to parse current state from database")?;

                // Parse metadata
                let metadata: StateMetadata = serde_json::from_value(metadata_json)
                    .context("Failed to parse metadata from database")?;

                Ok(Some(DslState {
                    business_unit_id: business_unit_id.to_string(),
                    operations,
                    current_state,
                    metadata,
                    version: version as u64,
                }))
            }
            None => Ok(None),
        }
    }

    async fn save_state(&self, state: &DslState) -> Result<()> {
        // Serialize the state components
        let operations_json =
            serde_json::to_value(&state.operations).context("Failed to serialize operations")?;

        let current_state_json = serde_json::to_value(&state.current_state)
            .context("Failed to serialize current state")?;

        let metadata_json =
            serde_json::to_value(&state.metadata).context("Failed to serialize metadata")?;

        // Insert the new state version
        sqlx::query(
            r#"
            INSERT INTO dsl_states
            (business_unit_id, version, operations, current_state, metadata, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(&state.business_unit_id)
        .bind(state.version as i64)
        .bind(&operations_json)
        .bind(&current_state_json)
        .bind(&metadata_json)
        .bind(Utc::now())
        .execute(&self.pool)
        .await
        .context("Failed to save DSL state to database")?;

        Ok(())
    }

    async fn get_state_history(
        &self,
        business_unit_id: &str,
        limit: Option<u32>,
    ) -> Result<Vec<DslState>> {
        let limit = limit.unwrap_or(100) as i64;

        let rows = sqlx::query(
            r#"
            SELECT operations, current_state, metadata, version, created_at, updated_at
            FROM dsl_states
            WHERE business_unit_id = $1
            ORDER BY version DESC
            LIMIT $2
            "#,
        )
        .bind(business_unit_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        let mut states = Vec::new();

        for row in rows {
            let operations_json: Value = row.get("operations");
            let current_state_json: Value = row.get("current_state");
            let metadata_json: Value = row.get("metadata");
            let version: i64 = row.get("version");

            // Parse operations
            let operations: Vec<DslOperation> = serde_json::from_value(operations_json)
                .context("Failed to parse operations from database")?;

            // Parse current state
            let current_state: HashMap<AttributeId, Value> =
                serde_json::from_value(current_state_json)
                    .context("Failed to parse current state from database")?;

            // Parse metadata
            let metadata: StateMetadata = serde_json::from_value(metadata_json)
                .context("Failed to parse metadata from database")?;

            states.push(DslState {
                business_unit_id: business_unit_id.to_string(),
                operations,
                current_state,
                metadata,
                version: version as u64,
            });
        }

        Ok(states)
    }

    async fn create_snapshot(&self, business_unit_id: &str) -> Result<Uuid> {
        // Get the latest state
        let latest_state = self.get_state(business_unit_id).await?.ok_or_else(|| {
            anyhow::anyhow!("No state found for business unit: {}", business_unit_id)
        })?;

        // Get the database ID of the latest state record
        let state_row = sqlx::query(
            "SELECT id FROM dsl_states WHERE business_unit_id = $1 ORDER BY version DESC LIMIT 1",
        )
        .bind(business_unit_id)
        .fetch_one(&self.pool)
        .await?;

        let state_id: Uuid = state_row.get("id");

        // Create snapshot record
        let snapshot_id = Uuid::new_v4();
        let snapshot_name = format!(
            "snapshot-v{}-{}",
            latest_state.version,
            Utc::now().format("%Y%m%d-%H%M%S")
        );

        sqlx::query(
            r#"
            INSERT INTO dsl_state_snapshots
            (id, business_unit_id, state_id, snapshot_name, created_by)
            VALUES ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(snapshot_id)
        .bind(business_unit_id)
        .bind(state_id)
        .bind(&snapshot_name)
        .bind("system") // In a real system, this would be the current user
        .execute(&self.pool)
        .await
        .context("Failed to create state snapshot")?;

        Ok(snapshot_id)
    }

    async fn restore_from_snapshot(&self, snapshot_id: Uuid) -> Result<DslState> {
        // Get the snapshot record
        let snapshot_row = sqlx::query(
            r#"
            SELECT s.business_unit_id, s.state_id, ds.operations, ds.current_state,
                   ds.metadata, ds.version
            FROM dsl_state_snapshots s
            JOIN dsl_states ds ON s.state_id = ds.id
            WHERE s.id = $1
            "#,
        )
        .bind(snapshot_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Snapshot not found: {}", snapshot_id))?;

        let business_unit_id: String = snapshot_row.get("business_unit_id");
        let operations_json: Value = snapshot_row.get("operations");
        let current_state_json: Value = snapshot_row.get("current_state");
        let metadata_json: Value = snapshot_row.get("metadata");
        let version: i64 = snapshot_row.get("version");

        // Parse the state components
        let operations: Vec<DslOperation> = serde_json::from_value(operations_json)
            .context("Failed to parse operations from snapshot")?;

        let current_state: HashMap<AttributeId, Value> = serde_json::from_value(current_state_json)
            .context("Failed to parse current state from snapshot")?;

        let metadata: StateMetadata = serde_json::from_value(metadata_json)
            .context("Failed to parse metadata from snapshot")?;

        Ok(DslState {
            business_unit_id,
            operations,
            current_state,
            metadata,
            version: version as u64,
        })
    }
}

/// In-memory implementation for testing and development
pub struct InMemoryStateStore {
    states: tokio::sync::RwLock<HashMap<String, DslState>>,
    snapshots: tokio::sync::RwLock<HashMap<Uuid, DslState>>,
}

impl InMemoryStateStore {
    pub fn new() -> Self {
        Self {
            states: tokio::sync::RwLock::new(HashMap::new()),
            snapshots: tokio::sync::RwLock::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl StateStore for InMemoryStateStore {
    async fn get_state(&self, business_unit_id: &str) -> Result<Option<DslState>> {
        let states = self.states.read().await;
        Ok(states.get(business_unit_id).cloned())
    }

    async fn save_state(&self, state: &DslState) -> Result<()> {
        let mut states = self.states.write().await;
        states.insert(state.business_unit_id.clone(), state.clone());
        Ok(())
    }

    async fn get_state_history(
        &self,
        business_unit_id: &str,
        _limit: Option<u32>,
    ) -> Result<Vec<DslState>> {
        let states = self.states.read().await;
        match states.get(business_unit_id) {
            Some(state) => Ok(vec![state.clone()]),
            None => Ok(vec![]),
        }
    }

    async fn create_snapshot(&self, business_unit_id: &str) -> Result<Uuid> {
        let states = self.states.read().await;
        let state = states.get(business_unit_id).ok_or_else(|| {
            anyhow::anyhow!("No state found for business unit: {}", business_unit_id)
        })?;

        let snapshot_id = Uuid::new_v4();
        let mut snapshots = self.snapshots.write().await;
        snapshots.insert(snapshot_id, state.clone());

        Ok(snapshot_id)
    }

    async fn restore_from_snapshot(&self, snapshot_id: Uuid) -> Result<DslState> {
        let snapshots = self.snapshots.read().await;
        snapshots
            .get(&snapshot_id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Snapshot not found: {}", snapshot_id))
    }
}

impl Default for InMemoryStateStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_dictionary::AttributeId;
    use crate::dsl::operations::ExecutableDslOperation as DslOperation;
    use chrono::Utc;
    use std::collections::HashMap;

    fn create_test_state() -> DslState {
        let now = Utc::now();
        DslState {
            business_unit_id: "TEST-001".to_string(),
            operations: vec![],
            current_state: {
                let mut state = HashMap::new();
                state.insert(AttributeId::new(), serde_json::json!("test_value"));
                state
            },
            metadata: StateMetadata {
                created_at: now,
                updated_at: now,
                domain: "test".to_string(),
                status: "active".to_string(),
                tags: vec!["test".to_string()],
                compliance_flags: vec![],
            },
            version: 1,
        }
    }

    #[tokio::test]
    async fn test_in_memory_state_store() {
        let store = InMemoryStateStore::new();
        let test_state = create_test_state();

        // Test save and get
        store.save_state(&test_state).await.unwrap();
        let retrieved = store.get_state("TEST-001").await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().business_unit_id, "TEST-001");

        // Test snapshot creation
        let snapshot_id = store.create_snapshot("TEST-001").await.unwrap();
        let restored = store.restore_from_snapshot(snapshot_id).await.unwrap();
        assert_eq!(restored.business_unit_id, "TEST-001");

        // Test history
        let history = store.get_state_history("TEST-001", None).await.unwrap();
        assert_eq!(history.len(), 1);
    }

    #[tokio::test]
    async fn test_state_operations() {
        let state = create_test_state();
        let operation = DslOperation {
            operation_type: "validate".to_string(),
            parameters: {
                let mut params = HashMap::new();
                params.insert("test".to_string(), serde_json::json!("value"));
                params
            },
            metadata: HashMap::new(),
        };

        let new_state = state.apply_operation(operation);
        assert_eq!(new_state.version, 2);
        assert_eq!(new_state.operations.len(), 1);
    }
}
