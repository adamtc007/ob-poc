//! Staged Runbook Repository
//!
//! Database operations for staged runbooks and commands.
//! All mutations produce audit events.

use anyhow::Result;
use sqlx::PgPool;
use uuid::Uuid;

use super::events::{PickerCandidate, RunbookSummary};
use super::staged_runbook::{
    PickerCandidate as StagedPickerCandidate, ResolutionSource, ResolutionStatus, ResolvedEntity,
    RunbookStatus, StagedCommand, StagedRunbook,
};

/// Repository for staged runbook operations
pub struct StagedRunbookRepository<'a> {
    pool: &'a PgPool,
}

impl<'a> StagedRunbookRepository<'a> {
    /// Create a new repository
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    // =========================================================================
    // Runbook Operations
    // =========================================================================

    /// Get or create a runbook for a session
    pub async fn get_or_create_runbook(
        &self,
        session_id: &str,
        client_group_id: Option<Uuid>,
        persona: Option<&str>,
    ) -> Result<Uuid> {
        let runbook_id = sqlx::query_scalar!(
            r#"SELECT "ob-poc".get_or_create_runbook($1, $2, $3) as "id!""#,
            session_id,
            client_group_id,
            persona
        )
        .fetch_one(self.pool)
        .await?;

        Ok(runbook_id)
    }

    /// Get runbook by ID
    pub async fn get_runbook(&self, runbook_id: Uuid) -> Result<Option<StagedRunbook>> {
        let row = sqlx::query!(
            r#"
            SELECT
                id,
                session_id,
                client_group_id,
                persona,
                status,
                created_at,
                updated_at
            FROM "ob-poc".staged_runbook
            WHERE id = $1
            "#,
            runbook_id
        )
        .fetch_optional(self.pool)
        .await?;

        match row {
            Some(r) => {
                let commands = self.get_commands(runbook_id).await?;
                Ok(Some(StagedRunbook {
                    id: r.id,
                    session_id: r.session_id,
                    client_group_id: r.client_group_id,
                    persona: r.persona,
                    status: RunbookStatus::from_db(&r.status),
                    commands,
                    created_at: r.created_at.unwrap_or_else(chrono::Utc::now),
                    updated_at: r.updated_at.unwrap_or_else(chrono::Utc::now),
                }))
            }
            None => Ok(None),
        }
    }

    /// Get active runbook for session
    pub async fn get_active_runbook(&self, session_id: &str) -> Result<Option<StagedRunbook>> {
        let row = sqlx::query!(
            r#"
            SELECT id
            FROM "ob-poc".staged_runbook
            WHERE session_id = $1 AND status = 'building'
            ORDER BY created_at DESC
            LIMIT 1
            "#,
            session_id
        )
        .fetch_optional(self.pool)
        .await?;

        match row {
            Some(r) => self.get_runbook(r.id).await,
            None => Ok(None),
        }
    }

    /// Get runbook summary
    pub async fn get_runbook_summary(&self, runbook_id: Uuid) -> Result<Option<RunbookSummary>> {
        let row = sqlx::query!(
            r#"
            SELECT
                runbook_id as "id!",
                status as "status!",
                command_count as "command_count!",
                resolved_count as "resolved_count!",
                pending_count as "pending_count!",
                ambiguous_count as "ambiguous_count!",
                failed_count as "failed_count!"
            FROM "ob-poc".v_runbook_summary
            WHERE runbook_id = $1
            "#,
            runbook_id
        )
        .fetch_optional(self.pool)
        .await?;

        Ok(row.map(|r| RunbookSummary {
            id: r.id,
            status: RunbookStatus::from_db(&r.status),
            command_count: r.command_count as usize,
            resolved_count: r.resolved_count as usize,
            pending_count: r.pending_count as usize,
            ambiguous_count: r.ambiguous_count as usize,
            failed_count: r.failed_count as usize,
        }))
    }

    /// Update runbook status
    pub async fn update_runbook_status(
        &self,
        runbook_id: Uuid,
        status: RunbookStatus,
    ) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE "ob-poc".staged_runbook
            SET status = $2, updated_at = now()
            WHERE id = $1
            "#,
            runbook_id,
            status.to_db()
        )
        .execute(self.pool)
        .await?;

        Ok(())
    }

    /// Abort runbook
    pub async fn abort_runbook(&self, runbook_id: Uuid) -> Result<bool> {
        let result = sqlx::query_scalar!(
            r#"SELECT "ob-poc".abort_runbook($1) as "aborted!""#,
            runbook_id
        )
        .fetch_one(self.pool)
        .await?;

        Ok(result)
    }

    // =========================================================================
    // Command Operations
    // =========================================================================

    /// Stage a new command
    pub async fn stage_command(
        &self,
        runbook_id: Uuid,
        dsl_raw: &str,
        verb: &str,
        description: Option<&str>,
        source_prompt: Option<&str>,
    ) -> Result<Uuid> {
        let command_id = sqlx::query_scalar!(
            r#"SELECT "ob-poc".stage_command($1, $2, $3, $4, $5) as "id!""#,
            runbook_id,
            dsl_raw,
            verb,
            description,
            source_prompt
        )
        .fetch_one(self.pool)
        .await?;

        Ok(command_id)
    }

    /// Get all commands for a runbook
    pub async fn get_commands(&self, runbook_id: Uuid) -> Result<Vec<StagedCommand>> {
        let rows = sqlx::query!(
            r#"
            SELECT
                command_id as "id!",
                source_order as "source_order!",
                dag_order,
                dsl_raw as "dsl_raw!",
                dsl_resolved,
                verb as "verb!",
                description,
                resolution_status as "resolution_status!",
                resolution_error,
                depends_on as "depends_on!",
                entity_footprint as "entity_footprint!",
                candidates as "candidates!"
            FROM "ob-poc".v_staged_runbook
            WHERE runbook_id = $1
            ORDER BY COALESCE(dag_order, source_order)
            "#,
            runbook_id
        )
        .fetch_all(self.pool)
        .await?;

        let mut commands = Vec::new();
        for r in rows {
            let entity_footprint: Vec<ResolvedEntity> =
                serde_json::from_value(r.entity_footprint).unwrap_or_default();
            let candidates: Vec<StagedPickerCandidate> =
                serde_json::from_value(r.candidates).unwrap_or_default();

            commands.push(StagedCommand {
                id: r.id,
                source_order: r.source_order,
                dag_order: r.dag_order,
                dsl_raw: r.dsl_raw,
                dsl_resolved: r.dsl_resolved,
                verb: r.verb,
                description: r.description,
                source_prompt: None, // Not in view
                resolution_status: ResolutionStatus::from_db(&r.resolution_status),
                resolution_error: r.resolution_error,
                depends_on: r.depends_on,
                entity_footprint,
                candidates,
            });
        }

        Ok(commands)
    }

    /// Get a single command
    pub async fn get_command(&self, command_id: Uuid) -> Result<Option<StagedCommand>> {
        let row = sqlx::query!(
            r#"
            SELECT
                id,
                runbook_id,
                source_order,
                dag_order,
                dsl_raw,
                dsl_resolved,
                verb,
                description,
                source_prompt,
                resolution_status,
                resolution_error,
                depends_on
            FROM "ob-poc".staged_command
            WHERE id = $1
            "#,
            command_id
        )
        .fetch_optional(self.pool)
        .await?;

        match row {
            Some(r) => {
                let entity_footprint = self.get_entity_footprint(command_id).await?;
                let candidates = self.get_candidates(command_id).await?;

                Ok(Some(StagedCommand {
                    id: r.id,
                    source_order: r.source_order,
                    dag_order: r.dag_order,
                    dsl_raw: r.dsl_raw,
                    dsl_resolved: r.dsl_resolved,
                    verb: r.verb,
                    description: r.description,
                    source_prompt: r.source_prompt,
                    resolution_status: ResolutionStatus::from_db(&r.resolution_status),
                    resolution_error: r.resolution_error,
                    depends_on: r.depends_on.unwrap_or_default(),
                    entity_footprint,
                    candidates,
                }))
            }
            None => Ok(None),
        }
    }

    /// Update command resolution status
    pub async fn update_command_resolution(
        &self,
        command_id: Uuid,
        status: ResolutionStatus,
        dsl_resolved: Option<&str>,
        error: Option<&str>,
    ) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE "ob-poc".staged_command
            SET
                resolution_status = $2,
                dsl_resolved = $3,
                resolution_error = $4
            WHERE id = $1
            "#,
            command_id,
            status.to_db(),
            dsl_resolved,
            error
        )
        .execute(self.pool)
        .await?;

        Ok(())
    }

    /// Update command DAG dependencies
    pub async fn update_command_dependencies(
        &self,
        command_id: Uuid,
        depends_on: &[Uuid],
    ) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE "ob-poc".staged_command
            SET depends_on = $2
            WHERE id = $1
            "#,
            command_id,
            depends_on
        )
        .execute(self.pool)
        .await?;

        Ok(())
    }

    /// Update command DAG order
    pub async fn update_command_dag_order(&self, command_id: Uuid, dag_order: i32) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE "ob-poc".staged_command
            SET dag_order = $2
            WHERE id = $1
            "#,
            command_id,
            dag_order
        )
        .execute(self.pool)
        .await?;

        Ok(())
    }

    /// Remove a command (and cascade dependents)
    pub async fn remove_command(&self, command_id: Uuid) -> Result<Vec<Uuid>> {
        let rows = sqlx::query!(
            r#"
            SELECT removed_id as "id!", was_dependent as "was_dependent!"
            FROM "ob-poc".remove_command($1)
            "#,
            command_id
        )
        .fetch_all(self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| r.id).collect())
    }

    // =========================================================================
    // Entity Footprint Operations
    // =========================================================================

    /// Get entity footprint for a command
    pub async fn get_entity_footprint(&self, command_id: Uuid) -> Result<Vec<ResolvedEntity>> {
        let rows = sqlx::query!(
            r#"
            SELECT
                sce.entity_id,
                e.name::TEXT as "entity_name!",
                sce.arg_name,
                sce.resolution_source,
                sce.original_ref,
                sce.confidence
            FROM "ob-poc".staged_command_entity sce
            JOIN "ob-poc".entities e ON e.entity_id = sce.entity_id
            WHERE sce.command_id = $1
            "#,
            command_id
        )
        .fetch_all(self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| ResolvedEntity {
                entity_id: r.entity_id,
                entity_name: r.entity_name,
                arg_name: r.arg_name,
                resolution_source: ResolutionSource::from_db(&r.resolution_source),
                original_ref: r.original_ref.unwrap_or_default(),
                confidence: r.confidence,
            })
            .collect())
    }

    /// Add resolved entity to command footprint
    pub async fn add_resolved_entity(
        &self,
        command_id: Uuid,
        entity: &ResolvedEntity,
    ) -> Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO "ob-poc".staged_command_entity
                (command_id, entity_id, arg_name, resolution_source, original_ref, confidence)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (command_id, entity_id, arg_name) DO UPDATE
            SET resolution_source = EXCLUDED.resolution_source,
                original_ref = EXCLUDED.original_ref,
                confidence = EXCLUDED.confidence
            "#,
            command_id,
            entity.entity_id,
            entity.arg_name,
            entity.resolution_source.to_db(),
            entity.original_ref,
            entity.confidence
        )
        .execute(self.pool)
        .await?;

        Ok(())
    }

    /// Clear entity footprint for a command
    pub async fn clear_entity_footprint(&self, command_id: Uuid) -> Result<()> {
        sqlx::query!(
            r#"DELETE FROM "ob-poc".staged_command_entity WHERE command_id = $1"#,
            command_id
        )
        .execute(self.pool)
        .await?;

        Ok(())
    }

    // =========================================================================
    // Picker Candidate Operations
    // =========================================================================

    /// Get picker candidates for a command
    pub async fn get_candidates(&self, command_id: Uuid) -> Result<Vec<StagedPickerCandidate>> {
        let rows = sqlx::query!(
            r#"
            SELECT
                scc.entity_id,
                e.name::TEXT as "entity_name!",
                scc.arg_name,
                scc.matched_tag,
                scc.confidence,
                scc.match_type
            FROM "ob-poc".staged_command_candidate scc
            JOIN "ob-poc".entities e ON e.entity_id = scc.entity_id
            WHERE scc.command_id = $1
            ORDER BY scc.confidence DESC
            "#,
            command_id
        )
        .fetch_all(self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| StagedPickerCandidate {
                entity_id: r.entity_id,
                entity_name: r.entity_name,
                arg_name: r.arg_name,
                matched_tag: r.matched_tag,
                confidence: r.confidence,
                match_type: r.match_type,
            })
            .collect())
    }

    /// Add picker candidate
    pub async fn add_candidate(
        &self,
        command_id: Uuid,
        candidate: &PickerCandidate,
        arg_name: &str,
    ) -> Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO "ob-poc".staged_command_candidate
                (command_id, entity_id, arg_name, matched_tag, confidence, match_type)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (command_id, entity_id, arg_name) DO NOTHING
            "#,
            command_id,
            candidate.entity_id,
            arg_name,
            candidate.matched_tag.as_deref(),
            candidate.confidence,
            candidate.match_type
        )
        .execute(self.pool)
        .await?;

        Ok(())
    }

    /// Clear picker candidates for a command
    pub async fn clear_candidates(&self, command_id: Uuid) -> Result<()> {
        sqlx::query!(
            r#"DELETE FROM "ob-poc".staged_command_candidate WHERE command_id = $1"#,
            command_id
        )
        .execute(self.pool)
        .await?;

        Ok(())
    }

    /// Validate picker selection against stored candidates
    /// CRITICAL: This prevents the agent from fabricating entity IDs
    pub async fn validate_picker_selection(
        &self,
        command_id: Uuid,
        entity_ids: &[Uuid],
    ) -> Result<PickerValidationResult> {
        let row = sqlx::query!(
            r#"
            SELECT
                is_valid as "is_valid!",
                invalid_entity_id,
                error_message
            FROM "ob-poc".validate_picker_selection($1, $2)
            "#,
            command_id,
            entity_ids
        )
        .fetch_one(self.pool)
        .await?;

        Ok(PickerValidationResult {
            is_valid: row.is_valid,
            invalid_entity_id: row.invalid_entity_id,
            error_message: row.error_message,
        })
    }

    // =========================================================================
    // Ready Gate Operations
    // =========================================================================

    /// Check if runbook is ready for execution
    pub async fn check_runbook_ready(&self, runbook_id: Uuid) -> Result<ReadinessCheck> {
        let rows = sqlx::query!(
            r#"
            SELECT
                is_ready as "is_ready!",
                blocking_command_id,
                blocking_source_order,
                blocking_status,
                blocking_error
            FROM "ob-poc".check_runbook_ready($1)
            "#,
            runbook_id
        )
        .fetch_all(self.pool)
        .await?;

        let is_ready = rows.first().map(|r| r.is_ready).unwrap_or(false);
        let blockers: Vec<_> = rows
            .into_iter()
            .filter_map(|r| {
                r.blocking_command_id.map(|cmd_id| BlockingCommandInfo {
                    command_id: cmd_id,
                    source_order: r.blocking_source_order.unwrap_or(0),
                    status: r.blocking_status.unwrap_or_default(),
                    error: r.blocking_error,
                })
            })
            .collect();

        Ok(ReadinessCheck { is_ready, blockers })
    }
}

/// Result of picker validation
#[derive(Debug, Clone)]
pub struct PickerValidationResult {
    pub is_valid: bool,
    pub invalid_entity_id: Option<Uuid>,
    pub error_message: Option<String>,
}

/// Result of readiness check
#[derive(Debug, Clone)]
pub struct ReadinessCheck {
    pub is_ready: bool,
    pub blockers: Vec<BlockingCommandInfo>,
}

/// Information about a blocking command
#[derive(Debug, Clone)]
pub struct BlockingCommandInfo {
    pub command_id: Uuid,
    pub source_order: i32,
    pub status: String,
    pub error: Option<String>,
}
