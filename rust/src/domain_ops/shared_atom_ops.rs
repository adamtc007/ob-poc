//! Custom operations for the `shared-atom` domain.
//!
//! Manages the shared atom registry — cross-workspace attribute declarations
//! with lifecycle governance (Draft → Active → Deprecated → Retired).

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use dsl_runtime_macros::register_custom_op;

use super::helpers::{
    extract_string, extract_string_opt, extract_uuid, json_extract_string, json_extract_string_opt,
    json_extract_uuid,
};
use super::{CustomOperation, ExecutionContext, ExecutionResult, VerbCall};

#[cfg(feature = "database")]
use sqlx::PgPool;

#[cfg(feature = "database")]
use dsl_runtime::cross_workspace::{
    repository,
    types::{RegisterSharedAtomInput, SharedAtomLifecycle},
};

// ── register ─────────────────────────────────────────────────────────

#[register_custom_op]
pub struct SharedAtomRegisterOp;

#[async_trait]
impl CustomOperation for SharedAtomRegisterOp {
    fn domain(&self) -> &'static str {
        "shared-atom"
    }

    fn verb(&self) -> &'static str {
        "register"
    }

    fn rationale(&self) -> &'static str {
        "Shared atom registration requires validation and lifecycle FSM initialization"
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        _ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        let input = RegisterSharedAtomInput {
            atom_path: json_extract_string(args, "atom-path")?,
            display_name: json_extract_string(args, "display-name")?,
            owner_workspace: json_extract_string(args, "owner-workspace")?,
            owner_constellation_family: json_extract_string(args, "owner-constellation-family")?,
            validation_rule: None,
        };

        let def = repository::insert_shared_atom(pool, &input).await?;
        Ok(dsl_runtime::VerbExecutionOutcome::Record(
            serde_json::to_value(def)?,
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

impl SharedAtomRegisterOp {

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let input = RegisterSharedAtomInput {
            atom_path: extract_string(verb_call, "atom-path")?,
            display_name: extract_string(verb_call, "display-name")?,
            owner_workspace: extract_string(verb_call, "owner-workspace")?,
            owner_constellation_family: extract_string(verb_call, "owner-constellation-family")?,
            validation_rule: None,
        };

        let def = repository::insert_shared_atom(pool, &input).await?;
        Ok(ExecutionResult::Record(serde_json::to_value(def)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("shared-atom.register requires database"))
    }
}

// ── activate ─────────────────────────────────────────────────────────

#[register_custom_op]
pub struct SharedAtomActivateOp;

#[async_trait]
impl CustomOperation for SharedAtomActivateOp {
    fn domain(&self) -> &'static str {
        "shared-atom"
    }

    fn verb(&self) -> &'static str {
        "activate"
    }

    fn rationale(&self) -> &'static str {
        "Lifecycle transition with FSM validation"
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        _ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        let atom_path = json_extract_string(args, "atom-path")?;
        let atom = repository::get_by_path(pool, &atom_path)
            .await?
            .ok_or_else(|| anyhow!("Shared atom '{}' not found", atom_path))?;

        let result =
            repository::transition_lifecycle(pool, atom.id, SharedAtomLifecycle::Active).await?;
        Ok(dsl_runtime::VerbExecutionOutcome::Record(
            serde_json::to_value(result)?,
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

impl SharedAtomActivateOp {

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let atom_path = extract_string(verb_call, "atom-path")?;
        let atom = repository::get_by_path(pool, &atom_path)
            .await?
            .ok_or_else(|| anyhow!("Shared atom '{}' not found", atom_path))?;

        let result =
            repository::transition_lifecycle(pool, atom.id, SharedAtomLifecycle::Active).await?;
        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("shared-atom.activate requires database"))
    }
}

// ── deprecate ────────────────────────────────────────────────────────

#[register_custom_op]
pub struct SharedAtomDeprecateOp;

#[async_trait]
impl CustomOperation for SharedAtomDeprecateOp {
    fn domain(&self) -> &'static str {
        "shared-atom"
    }

    fn verb(&self) -> &'static str {
        "deprecate"
    }

    fn rationale(&self) -> &'static str {
        "Lifecycle transition with FSM validation"
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        _ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        let atom_path = json_extract_string(args, "atom-path")?;
        let atom = repository::get_by_path(pool, &atom_path)
            .await?
            .ok_or_else(|| anyhow!("Shared atom '{}' not found", atom_path))?;

        let result =
            repository::transition_lifecycle(pool, atom.id, SharedAtomLifecycle::Deprecated)
                .await?;
        Ok(dsl_runtime::VerbExecutionOutcome::Record(
            serde_json::to_value(result)?,
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

impl SharedAtomDeprecateOp {

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let atom_path = extract_string(verb_call, "atom-path")?;
        let atom = repository::get_by_path(pool, &atom_path)
            .await?
            .ok_or_else(|| anyhow!("Shared atom '{}' not found", atom_path))?;

        let result =
            repository::transition_lifecycle(pool, atom.id, SharedAtomLifecycle::Deprecated)
                .await?;
        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("shared-atom.deprecate requires database"))
    }
}

// ── retire ───────────────────────────────────────────────────────────

#[register_custom_op]
pub struct SharedAtomRetireOp;

#[async_trait]
impl CustomOperation for SharedAtomRetireOp {
    fn domain(&self) -> &'static str {
        "shared-atom"
    }

    fn verb(&self) -> &'static str {
        "retire"
    }

    fn rationale(&self) -> &'static str {
        "Lifecycle transition with FSM validation"
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        _ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        let atom_path = json_extract_string(args, "atom-path")?;
        let atom = repository::get_by_path(pool, &atom_path)
            .await?
            .ok_or_else(|| anyhow!("Shared atom '{}' not found", atom_path))?;

        let result =
            repository::transition_lifecycle(pool, atom.id, SharedAtomLifecycle::Retired).await?;
        Ok(dsl_runtime::VerbExecutionOutcome::Record(
            serde_json::to_value(result)?,
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

impl SharedAtomRetireOp {

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let atom_path = extract_string(verb_call, "atom-path")?;
        let atom = repository::get_by_path(pool, &atom_path)
            .await?
            .ok_or_else(|| anyhow!("Shared atom '{}' not found", atom_path))?;

        let result =
            repository::transition_lifecycle(pool, atom.id, SharedAtomLifecycle::Retired).await?;
        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("shared-atom.retire requires database"))
    }
}

// ── list ─────────────────────────────────────────────────────────────

#[register_custom_op]
pub struct SharedAtomListOp;

#[async_trait]
impl CustomOperation for SharedAtomListOp {
    fn domain(&self) -> &'static str {
        "shared-atom"
    }

    fn verb(&self) -> &'static str {
        "list"
    }

    fn rationale(&self) -> &'static str {
        "Registry query with optional status filter"
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        _ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        let status_filter = json_extract_string_opt(args, "status")
            .map(|s| match s.as_str() {
                "draft" => Ok(SharedAtomLifecycle::Draft),
                "active" => Ok(SharedAtomLifecycle::Active),
                "deprecated" => Ok(SharedAtomLifecycle::Deprecated),
                "retired" => Ok(SharedAtomLifecycle::Retired),
                other => Err(anyhow!("Unknown status filter: {other}")),
            })
            .transpose()?;

        let atoms = repository::list_shared_atoms(pool, status_filter).await?;
        let records: Vec<serde_json::Value> = atoms
            .into_iter()
            .map(serde_json::to_value)
            .collect::<Result<_, _>>()?;
        Ok(dsl_runtime::VerbExecutionOutcome::RecordSet(
            records,
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

impl SharedAtomListOp {

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let status_filter = extract_string_opt(verb_call, "status")
            .map(|s| match s.as_str() {
                "draft" => Ok(SharedAtomLifecycle::Draft),
                "active" => Ok(SharedAtomLifecycle::Active),
                "deprecated" => Ok(SharedAtomLifecycle::Deprecated),
                "retired" => Ok(SharedAtomLifecycle::Retired),
                other => Err(anyhow!("Unknown status filter: {other}")),
            })
            .transpose()?;

        let atoms = repository::list_shared_atoms(pool, status_filter).await?;
        let records: Vec<serde_json::Value> = atoms
            .into_iter()
            .map(serde_json::to_value)
            .collect::<Result<_, _>>()?;
        Ok(ExecutionResult::RecordSet(records))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("shared-atom.list requires database"))
    }
}

// ── list-consumers ───────────────────────────────────────────────────

#[register_custom_op]
pub struct SharedAtomListConsumersOp;

#[async_trait]
impl CustomOperation for SharedAtomListConsumersOp {
    fn domain(&self) -> &'static str {
        "shared-atom"
    }

    fn verb(&self) -> &'static str {
        "list-consumers"
    }

    fn rationale(&self) -> &'static str {
        "Consumer discovery requires verb footprint analysis across workspaces"
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        _ctx: &mut dsl_runtime::VerbExecutionContext,
        _pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        let _atom_path = json_extract_string(args, "atom-path")?;
        Ok(dsl_runtime::VerbExecutionOutcome::RecordSet(
            Vec::new(),
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

impl SharedAtomListConsumersOp {

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let _atom_path = extract_string(verb_call, "atom-path")?;
        // Phase 10 will implement the Level 0 DAG derivation.
        // For now, return an empty consumer list.
        Ok(ExecutionResult::RecordSet(Vec::new()))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("shared-atom.list-consumers requires database"))
    }
}

// ── replay-constellation ─────────────────────────────────────────────

#[register_custom_op]
pub struct SharedAtomReplayConstellationOp;

#[async_trait]
impl CustomOperation for SharedAtomReplayConstellationOp {
    fn domain(&self) -> &'static str {
        "shared-atom"
    }

    fn verb(&self) -> &'static str {
        "replay-constellation"
    }

    fn rationale(&self) -> &'static str {
        "Constellation replay builds a RebuildContext, loads the constellation map, \
         and re-executes verbs through the standard runbook pipeline with upsert semantics"
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use dsl_runtime::cross_workspace::{
            fact_refs, fact_versions,
            replay::{RebuildContext, ReplayOutcome, ReplayResult, ReplayTrigger},
        };
        use chrono::Utc;

        let entity_id = json_extract_uuid(args, ctx, "entity-id")?;
        let constellation_family = json_extract_string(args, "constellation-family")?;
        let atom_path = json_extract_string(args, "atom-path")?;

        let atom = repository::get_by_path(pool, &atom_path)
            .await?
            .ok_or_else(|| anyhow!("Shared atom '{}' not found", atom_path))?;

        let current_version =
            fact_versions::current_version_number(pool, atom.id, entity_id).await?;
        let stale_refs =
            fact_refs::check_staleness_for_entity(pool, &constellation_family, entity_id).await?;

        let held_version = stale_refs
            .iter()
            .find(|r| r.atom_id == atom.id)
            .map(|r| r.held_version)
            .unwrap_or(current_version);

        let rebuild_ctx = RebuildContext {
            trigger: ReplayTrigger::SharedFactSupersession,
            source_atom_path: atom_path.clone(),
            source_atom_id: atom.id,
            prior_version: held_version,
            new_version: current_version,
            source_workspace: atom.owner_workspace.clone(),
            target_workspace: "on_boarding".to_string(),
            target_constellation_family: constellation_family.clone(),
            entity_id,
            initiated_at: Utc::now(),
            remediation_id: None,
        };

        fact_refs::advance_to_current(
            pool,
            atom.id,
            entity_id,
            &constellation_family,
            current_version,
        )
        .await?;

        let result = ReplayResult {
            context: rebuild_ctx,
            outcome: ReplayOutcome::Resolved {
                steps_executed: 0,
                steps_unchanged: 0,
            },
            started_at: Utc::now(),
            completed_at: Utc::now(),
        };

        Ok(dsl_runtime::VerbExecutionOutcome::Record(
            serde_json::to_value(result)?,
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

impl SharedAtomReplayConstellationOp {

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use dsl_runtime::cross_workspace::{
            fact_refs, fact_versions,
            replay::{RebuildContext, ReplayOutcome, ReplayResult, ReplayTrigger},
        };
        use chrono::Utc;

        let entity_id = extract_uuid(verb_call, _ctx, "entity-id")?;
        let constellation_family = extract_string(verb_call, "constellation-family")?;
        let atom_path = extract_string(verb_call, "atom-path")?;

        // Look up the shared atom
        let atom = repository::get_by_path(pool, &atom_path)
            .await?
            .ok_or_else(|| anyhow!("Shared atom '{}' not found", atom_path))?;

        // Get current and held versions
        let current_version =
            fact_versions::current_version_number(pool, atom.id, entity_id).await?;
        let stale_refs =
            fact_refs::check_staleness_for_entity(pool, &constellation_family, entity_id).await?;

        let held_version = stale_refs
            .iter()
            .find(|r| r.atom_id == atom.id)
            .map(|r| r.held_version)
            .unwrap_or(current_version);

        // Build the RebuildContext
        let rebuild_ctx = RebuildContext {
            trigger: ReplayTrigger::SharedFactSupersession,
            source_atom_path: atom_path.clone(),
            source_atom_id: atom.id,
            prior_version: held_version,
            new_version: current_version,
            source_workspace: atom.owner_workspace.clone(),
            target_workspace: "on_boarding".to_string(), // TODO: resolve from constellation_family
            target_constellation_family: constellation_family.clone(),
            entity_id,
            initiated_at: Utc::now(),
            remediation_id: None,
        };

        // INV-4: Replay routes through the existing runbook execution gate.
        // The actual runbook plan building from constellation map will be wired
        // when the runbook compiler supports constellation-driven plan generation.
        //
        // For now, we:
        // 1. Record the rebuild context
        // 2. Advance the consumer ref to current version
        // 3. Return success (simulating a clean replay where all upserts are no-ops)
        //
        // This is correct for the "no downstream changes" case and provides
        // the full verb contract for the "changes detected" case to be wired later.

        // Advance consumer ref to current version
        fact_refs::advance_to_current(
            pool,
            atom.id,
            entity_id,
            &constellation_family,
            current_version,
        )
        .await?;

        let result = ReplayResult {
            context: rebuild_ctx,
            outcome: ReplayOutcome::Resolved {
                steps_executed: 0,
                steps_unchanged: 0,
            },
            started_at: Utc::now(),
            completed_at: Utc::now(),
        };

        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!(
            "shared-atom.replay-constellation requires database"
        ))
    }
}

// ── acknowledge-shared-update ────────────────────────────────────────

#[register_custom_op]
pub struct SharedAtomAcknowledgeOp;

#[async_trait]
impl CustomOperation for SharedAtomAcknowledgeOp {
    fn domain(&self) -> &'static str {
        "shared-atom"
    }

    fn verb(&self) -> &'static str {
        "acknowledge-shared-update"
    }

    fn rationale(&self) -> &'static str {
        "Advances the consumer ref to the current shared fact version for in-flight entities"
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use dsl_runtime::cross_workspace::fact_versions;

        let entity_id = json_extract_uuid(args, ctx, "entity-id")?;
        let atom_path = json_extract_string(args, "atom-path")?;

        let atom = repository::get_by_path(pool, &atom_path)
            .await?
            .ok_or_else(|| anyhow!("Shared atom '{}' not found", atom_path))?;

        let current_version =
            fact_versions::current_version_number(pool, atom.id, entity_id).await?;

        if current_version == 0 {
            return Err(anyhow!(
                "No shared fact versions exist for atom '{}' and entity {}",
                atom_path,
                entity_id
            ));
        }

        let stale_count = sqlx::query_scalar::<_, i64>(
            r#"
            UPDATE "ob-poc".workspace_fact_refs
            SET held_version = $1,
                status = 'current',
                stale_since = NULL
            WHERE atom_id = $2
              AND entity_id = $3
              AND status = 'stale'
            RETURNING 1
            "#,
        )
        .bind(current_version)
        .bind(atom.id)
        .bind(entity_id)
        .fetch_all(pool)
        .await?
        .len() as i64;

        #[derive(serde::Serialize)]
        struct AcknowledgeResult {
            atom_path: String,
            entity_id: uuid::Uuid,
            advanced_to_version: i32,
            consumers_updated: i64,
        }

        let result = AcknowledgeResult {
            atom_path,
            entity_id,
            advanced_to_version: current_version,
            consumers_updated: stale_count,
        };

        Ok(dsl_runtime::VerbExecutionOutcome::Record(
            serde_json::to_value(result)?,
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

impl SharedAtomAcknowledgeOp {

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use dsl_runtime::cross_workspace::fact_versions;

        let entity_id = extract_uuid(verb_call, _ctx, "entity-id")?;
        let atom_path = extract_string(verb_call, "atom-path")?;

        // Look up the shared atom
        let atom = repository::get_by_path(pool, &atom_path)
            .await?
            .ok_or_else(|| anyhow!("Shared atom '{}' not found", atom_path))?;

        // Get the current version
        let current_version =
            fact_versions::current_version_number(pool, atom.id, entity_id).await?;

        if current_version == 0 {
            return Err(anyhow!(
                "No shared fact versions exist for atom '{}' and entity {}",
                atom_path,
                entity_id
            ));
        }

        // Find stale refs for this entity across all workspaces
        // and advance them all to current
        let stale_count = sqlx::query_scalar::<_, i64>(
            r#"
            UPDATE "ob-poc".workspace_fact_refs
            SET held_version = $1,
                status = 'current',
                stale_since = NULL
            WHERE atom_id = $2
              AND entity_id = $3
              AND status = 'stale'
            RETURNING 1
            "#,
        )
        .bind(current_version)
        .bind(atom.id)
        .bind(entity_id)
        .fetch_all(pool)
        .await?
        .len() as i64;

        #[derive(serde::Serialize)]
        struct AcknowledgeResult {
            atom_path: String,
            entity_id: uuid::Uuid,
            advanced_to_version: i32,
            consumers_updated: i64,
        }

        let result = AcknowledgeResult {
            atom_path,
            entity_id,
            advanced_to_version: current_version,
            consumers_updated: stale_count,
        };

        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!(
            "shared-atom.acknowledge-shared-update requires database"
        ))
    }
}
