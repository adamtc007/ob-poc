//! View State Audit Trail Repository
//!
//! Provides persistence for view state changes throughout the DSL execution pipeline.
//! This closes the "side door" where view state could be lost between execution
//! and session persistence.
//!
//! ## Integration Points
//!
//! This repository is called from:
//! 1. `DslExecutor::execute_step()` - records input view state before execution
//! 2. `DslExecutor::execute_step()` - records output view state after view.* operations
//! 3. `SessionRepository::update_view_state()` - syncs session current view
//!
//! ## Audit Trail
//!
//! Every view state change is recorded with:
//! - Link to the idempotency key (execution audit trail)
//! - Link to the session (if available)
//! - Full view state snapshot (for reconstruction)
//! - Selection array (for batch operation auditing)

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::session::ViewState;

/// A recorded view state change
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewStateChange {
    pub change_id: Uuid,
    pub idempotency_key: String,
    pub session_id: Option<Uuid>,
    pub verb_name: String,
    pub taxonomy_context: serde_json::Value,
    pub selection: Vec<Uuid>,
    pub selection_count: i32,
    pub refinements: serde_json::Value,
    pub stack_depth: i32,
    pub view_state_snapshot: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub audit_user_id: Option<Uuid>,
}

/// Input for recording a view state change
#[derive(Debug, Clone)]
pub struct RecordViewStateChange {
    pub idempotency_key: String,
    pub session_id: Option<Uuid>,
    pub verb_name: String,
    pub view_state: ViewState,
    pub audit_user_id: Option<Uuid>,
}

/// Session view history entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionViewHistoryEntry {
    pub session_id: Uuid,
    pub change_id: Uuid,
    pub verb_name: String,
    pub selection_count: i32,
    pub stack_depth: i32,
    pub node_type: Option<String>,
    pub label: Option<String>,
    pub refinements: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub session_status: Option<String>,
    pub primary_domain: Option<String>,
}

/// View state audit repository
pub struct ViewStateAuditRepository {
    pool: PgPool,
}

impl ViewStateAuditRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Record a view state change
    ///
    /// This is called from the DSL executor when a view.* operation produces
    /// a new ViewState. It creates a complete audit trail linking the view
    /// state to the execution and session.
    pub async fn record_view_state_change(&self, input: RecordViewStateChange) -> Result<Uuid> {
        // Serialize view state components
        let taxonomy_context = serde_json::to_value(&input.view_state.context)?;
        let refinements = serde_json::to_value(&input.view_state.refinements)?;
        let view_state_snapshot = serde_json::to_value(&input.view_state)?;
        let stack_depth = input.view_state.stack.depth() as i32;

        // Call the PostgreSQL function that atomically records and updates session
        let change_id: Uuid = sqlx::query_scalar(
            r#"
            SELECT "ob-poc".record_view_state_change(
                $1, $2, $3, $4, $5, $6, $7, $8, $9
            )
            "#,
        )
        .bind(&input.idempotency_key)
        .bind(input.session_id)
        .bind(&input.verb_name)
        .bind(&taxonomy_context)
        .bind(&input.view_state.selection)
        .bind(&refinements)
        .bind(stack_depth)
        .bind(&view_state_snapshot)
        .bind(input.audit_user_id)
        .fetch_one(&self.pool)
        .await?;

        tracing::debug!(
            change_id = %change_id,
            idempotency_key = %input.idempotency_key,
            verb = %input.verb_name,
            selection_count = input.view_state.selection.len(),
            "Recorded view state change"
        );

        Ok(change_id)
    }

    /// Update execution record with input view state
    ///
    /// Called before executing a statement to record what selection/view
    /// state was active. This is critical for batch operation auditing.
    pub async fn record_input_view_state(
        &self,
        idempotency_key: &str,
        view_state: &ViewState,
    ) -> Result<()> {
        let view_state_json = serde_json::to_value(view_state)?;

        sqlx::query(
            r#"
            UPDATE "ob-poc".dsl_idempotency
            SET input_view_state = $2,
                input_selection = $3
            WHERE idempotency_key = $1
            "#,
        )
        .bind(idempotency_key)
        .bind(&view_state_json)
        .bind(&view_state.selection)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Update execution record with output view state
    ///
    /// Called after executing a view.* operation to record the resulting
    /// view state. This enables reconstruction of the visual state.
    pub async fn record_output_view_state(
        &self,
        idempotency_key: &str,
        view_state: &ViewState,
    ) -> Result<()> {
        let view_state_json = serde_json::to_value(view_state)?;

        sqlx::query(
            r#"
            UPDATE "ob-poc".dsl_idempotency
            SET output_view_state = $2
            WHERE idempotency_key = $1
            "#,
        )
        .bind(idempotency_key)
        .bind(&view_state_json)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get view state change by ID
    pub async fn get_view_state_change(&self, change_id: Uuid) -> Result<Option<ViewStateChange>> {
        let row = sqlx::query_as::<
            _,
            (
                Uuid,
                String,
                Option<Uuid>,
                String,
                serde_json::Value,
                Vec<Uuid>,
                i32,
                serde_json::Value,
                i32,
                serde_json::Value,
                DateTime<Utc>,
                Option<Uuid>,
            ),
        >(
            r#"
            SELECT
                change_id,
                idempotency_key,
                session_id,
                verb_name,
                taxonomy_context,
                selection,
                selection_count,
                refinements,
                stack_depth,
                view_state_snapshot,
                created_at,
                audit_user_id
            FROM "ob-poc".dsl_view_state_changes
            WHERE change_id = $1
            "#,
        )
        .bind(change_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| ViewStateChange {
            change_id: r.0,
            idempotency_key: r.1,
            session_id: r.2,
            verb_name: r.3,
            taxonomy_context: r.4,
            selection: r.5,
            selection_count: r.6,
            refinements: r.7,
            stack_depth: r.8,
            view_state_snapshot: r.9,
            created_at: r.10,
            audit_user_id: r.11,
        }))
    }

    /// Get view state history for a session
    pub async fn get_session_view_history(
        &self,
        session_id: Uuid,
        limit: Option<i64>,
    ) -> Result<Vec<SessionViewHistoryEntry>> {
        let limit = limit.unwrap_or(100);

        let rows = sqlx::query_as::<
            _,
            (
                Uuid,
                Uuid,
                String,
                i32,
                i32,
                Option<String>,
                Option<String>,
                serde_json::Value,
                DateTime<Utc>,
                Option<String>,
                Option<String>,
            ),
        >(
            r#"
            SELECT
                session_id,
                change_id,
                verb_name,
                selection_count,
                stack_depth,
                node_type,
                label,
                refinements,
                created_at,
                session_status,
                primary_domain
            FROM "ob-poc".v_session_view_history
            WHERE session_id = $1
            ORDER BY created_at DESC
            LIMIT $2
            "#,
        )
        .bind(session_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| SessionViewHistoryEntry {
                session_id: r.0,
                change_id: r.1,
                verb_name: r.2,
                selection_count: r.3,
                stack_depth: r.4,
                node_type: r.5,
                label: r.6,
                refinements: r.7,
                created_at: r.8,
                session_status: r.9,
                primary_domain: r.10,
            })
            .collect())
    }

    /// Find view state changes that affected specific entities
    ///
    /// Uses the GIN index on selection array for efficient lookup
    pub async fn find_changes_affecting_entities(
        &self,
        entity_ids: &[Uuid],
        limit: Option<i64>,
    ) -> Result<Vec<ViewStateChange>> {
        let limit = limit.unwrap_or(100);

        let rows = sqlx::query_as::<
            _,
            (
                Uuid,
                String,
                Option<Uuid>,
                String,
                serde_json::Value,
                Vec<Uuid>,
                i32,
                serde_json::Value,
                i32,
                serde_json::Value,
                DateTime<Utc>,
                Option<Uuid>,
            ),
        >(
            r#"
            SELECT
                change_id,
                idempotency_key,
                session_id,
                verb_name,
                taxonomy_context,
                selection,
                selection_count,
                refinements,
                stack_depth,
                view_state_snapshot,
                created_at,
                audit_user_id
            FROM "ob-poc".dsl_view_state_changes
            WHERE selection && $1
            ORDER BY created_at DESC
            LIMIT $2
            "#,
        )
        .bind(entity_ids)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| ViewStateChange {
                change_id: r.0,
                idempotency_key: r.1,
                session_id: r.2,
                verb_name: r.3,
                taxonomy_context: r.4,
                selection: r.5,
                selection_count: r.6,
                refinements: r.7,
                stack_depth: r.8,
                view_state_snapshot: r.9,
                created_at: r.10,
                audit_user_id: r.11,
            })
            .collect())
    }

    /// Reconstruct view state from snapshot
    pub fn reconstruct_view_state(snapshot: &serde_json::Value) -> Result<ViewState> {
        let view_state: ViewState = serde_json::from_value(snapshot.clone())?;
        Ok(view_state)
    }
}
