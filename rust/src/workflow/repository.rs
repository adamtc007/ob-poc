//! Workflow Repository
//!
//! Database persistence for workflow instances.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use super::state::{Blocker, WorkflowInstance};
use super::WorkflowError;

/// Repository for workflow instance persistence
pub struct WorkflowRepository {
    pool: PgPool,
}

impl WorkflowRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Save a workflow instance (insert or update)
    pub async fn save(&self, instance: &WorkflowInstance) -> Result<(), WorkflowError> {
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".workflow_instances
            (instance_id, workflow_id, version, subject_type, subject_id,
             current_state, state_entered_at, history, blockers, metadata,
             created_at, updated_at, created_by)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            ON CONFLICT (instance_id) DO UPDATE SET
                current_state = EXCLUDED.current_state,
                state_entered_at = EXCLUDED.state_entered_at,
                history = EXCLUDED.history,
                blockers = EXCLUDED.blockers,
                metadata = EXCLUDED.metadata,
                updated_at = EXCLUDED.updated_at
            "#,
        )
        .bind(instance.instance_id)
        .bind(&instance.workflow_id)
        .bind(instance.version as i32)
        .bind(&instance.subject_type)
        .bind(instance.subject_id)
        .bind(&instance.current_state)
        .bind(instance.state_entered_at)
        .bind(serde_json::to_value(&instance.history).unwrap_or_default())
        .bind(serde_json::to_value(&instance.blockers).unwrap_or_default())
        .bind(serde_json::to_value(&instance.metadata).unwrap_or_default())
        .bind(instance.created_at)
        .bind(instance.updated_at)
        .bind(&instance.created_by)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Load a workflow instance by ID
    pub async fn load(&self, instance_id: Uuid) -> Result<WorkflowInstance, WorkflowError> {
        let row = sqlx::query_as::<_, WorkflowRow>(
            r#"
            SELECT instance_id, workflow_id, version, subject_type, subject_id,
                   current_state, state_entered_at, history, blockers, metadata,
                   created_at, updated_at, created_by
            FROM "ob-poc".workflow_instances
            WHERE instance_id = $1
            "#,
        )
        .bind(instance_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(WorkflowError::InstanceNotFound(instance_id))?;

        Ok(row.into())
    }

    /// Find workflow instance by subject
    pub async fn find_by_subject(
        &self,
        workflow_id: &str,
        subject_type: &str,
        subject_id: Uuid,
    ) -> Result<Option<WorkflowInstance>, WorkflowError> {
        let row = sqlx::query_as::<_, WorkflowRow>(
            r#"
            SELECT instance_id, workflow_id, version, subject_type, subject_id,
                   current_state, state_entered_at, history, blockers, metadata,
                   created_at, updated_at, created_by
            FROM "ob-poc".workflow_instances
            WHERE workflow_id = $1 AND subject_type = $2 AND subject_id = $3
            "#,
        )
        .bind(workflow_id)
        .bind(subject_type)
        .bind(subject_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(Into::into))
    }

    /// List all instances for a subject (across all workflow types)
    pub async fn list_by_subject(
        &self,
        subject_type: &str,
        subject_id: Uuid,
    ) -> Result<Vec<WorkflowInstance>, WorkflowError> {
        let rows = sqlx::query_as::<_, WorkflowRow>(
            r#"
            SELECT instance_id, workflow_id, version, subject_type, subject_id,
                   current_state, state_entered_at, history, blockers, metadata,
                   created_at, updated_at, created_by
            FROM "ob-poc".workflow_instances
            WHERE subject_type = $1 AND subject_id = $2
            ORDER BY created_at DESC
            "#,
        )
        .bind(subject_type)
        .bind(subject_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    /// List instances by state
    pub async fn list_by_state(
        &self,
        workflow_id: &str,
        state: &str,
    ) -> Result<Vec<WorkflowInstance>, WorkflowError> {
        let rows = sqlx::query_as::<_, WorkflowRow>(
            r#"
            SELECT instance_id, workflow_id, version, subject_type, subject_id,
                   current_state, state_entered_at, history, blockers, metadata,
                   created_at, updated_at, created_by
            FROM "ob-poc".workflow_instances
            WHERE workflow_id = $1 AND current_state = $2
            ORDER BY state_entered_at ASC
            "#,
        )
        .bind(workflow_id)
        .bind(state)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    /// Log a state transition to the audit log
    #[allow(clippy::too_many_arguments)]
    pub async fn log_transition(
        &self,
        instance_id: Uuid,
        from_state: Option<&str>,
        to_state: &str,
        transition_type: &str,
        by: Option<&str>,
        reason: Option<&str>,
        blockers: &[Blocker],
    ) -> Result<(), WorkflowError> {
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".workflow_audit_log
            (instance_id, from_state, to_state, transition_type, transitioned_by, reason, blockers_at_transition)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
        )
        .bind(instance_id)
        .bind(from_state)
        .bind(to_state)
        .bind(transition_type)
        .bind(by)
        .bind(reason)
        .bind(serde_json::to_value(blockers).unwrap_or_default())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Delete a workflow instance
    pub async fn delete(&self, instance_id: Uuid) -> Result<(), WorkflowError> {
        sqlx::query(
            r#"
            DELETE FROM "ob-poc".workflow_instances
            WHERE instance_id = $1
            "#,
        )
        .bind(instance_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

/// Database row for workflow instance
#[derive(Debug, sqlx::FromRow)]
struct WorkflowRow {
    instance_id: Uuid,
    workflow_id: String,
    version: i32,
    subject_type: String,
    subject_id: Uuid,
    current_state: String,
    state_entered_at: DateTime<Utc>,
    history: serde_json::Value,
    blockers: serde_json::Value,
    metadata: serde_json::Value,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    created_by: Option<String>,
}

impl From<WorkflowRow> for WorkflowInstance {
    fn from(row: WorkflowRow) -> Self {
        Self {
            instance_id: row.instance_id,
            workflow_id: row.workflow_id,
            version: row.version as u32,
            subject_type: row.subject_type,
            subject_id: row.subject_id,
            current_state: row.current_state,
            state_entered_at: row.state_entered_at,
            history: serde_json::from_value(row.history).unwrap_or_default(),
            blockers: serde_json::from_value(row.blockers).unwrap_or_default(),
            metadata: serde_json::from_value(row.metadata).unwrap_or_default(),
            created_at: row.created_at,
            updated_at: row.updated_at,
            created_by: row.created_by,
        }
    }
}
