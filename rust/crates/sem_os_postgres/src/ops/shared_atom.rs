//! Shared-atom domain verbs (8 plugin verbs) — SemOS-side YAML-first
//! re-implementation of the plugin subset of
//! `rust/config/verbs/shared-atom.yaml`.
//!
//! Manages the shared atom registry — cross-workspace attribute
//! declarations with lifecycle governance
//! (Draft → Active → Deprecated → Retired). Delegates to the
//! `dsl_runtime::cross_workspace::{repository, fact_refs,
//! fact_versions, replay, types}` helpers (same transitional
//! `scope.pool()` pattern as slice #13).

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use chrono::Utc;
use serde_json::Value;

use dsl_runtime::cross_workspace::{
    fact_refs, fact_versions,
    replay::{RebuildContext, ReplayOutcome, ReplayResult, ReplayTrigger},
    repository,
    types::{RegisterSharedAtomInput, SharedAtomLifecycle},
};
use dsl_runtime::domain_ops::helpers::{
    json_extract_string, json_extract_string_opt, json_extract_uuid,
};
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::SemOsVerbOp;

fn parse_lifecycle_filter(value: Option<String>) -> Result<Option<SharedAtomLifecycle>> {
    value
        .map(|s| match s.as_str() {
            "draft" => Ok(SharedAtomLifecycle::Draft),
            "active" => Ok(SharedAtomLifecycle::Active),
            "deprecated" => Ok(SharedAtomLifecycle::Deprecated),
            "retired" => Ok(SharedAtomLifecycle::Retired),
            other => Err(anyhow!("Unknown status filter: {other}")),
        })
        .transpose()
}

async fn transition_atom(
    scope: &mut dyn TransactionScope,
    args: &Value,
    target: SharedAtomLifecycle,
) -> Result<VerbExecutionOutcome> {
    let atom_path = json_extract_string(args, "atom-path")?;
    let atom = repository::get_by_path(scope.pool(), &atom_path)
        .await?
        .ok_or_else(|| anyhow!("Shared atom '{}' not found", atom_path))?;
    let result = repository::transition_lifecycle(scope.pool(), atom.id, target).await?;
    Ok(VerbExecutionOutcome::Record(serde_json::to_value(result)?))
}

// ── shared-atom.register ──────────────────────────────────────────────────────

pub struct Register;

#[async_trait]
impl SemOsVerbOp for Register {
    fn fqn(&self) -> &str {
        "shared-atom.register"
    }
    async fn execute(
        &self,
        args: &Value,
        _ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let input = RegisterSharedAtomInput {
            atom_path: json_extract_string(args, "atom-path")?,
            display_name: json_extract_string(args, "display-name")?,
            owner_workspace: json_extract_string(args, "owner-workspace")?,
            owner_constellation_family: json_extract_string(args, "owner-constellation-family")?,
            validation_rule: None,
        };
        let def = repository::insert_shared_atom(scope.pool(), &input).await?;
        Ok(VerbExecutionOutcome::Record(serde_json::to_value(def)?))
    }
}

// ── shared-atom.activate / deprecate / retire ─────────────────────────────────

pub struct Activate;

#[async_trait]
impl SemOsVerbOp for Activate {
    fn fqn(&self) -> &str {
        "shared-atom.activate"
    }
    async fn execute(
        &self,
        args: &Value,
        _ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        transition_atom(scope, args, SharedAtomLifecycle::Active).await
    }
}

pub struct Deprecate;

#[async_trait]
impl SemOsVerbOp for Deprecate {
    fn fqn(&self) -> &str {
        "shared-atom.deprecate"
    }
    async fn execute(
        &self,
        args: &Value,
        _ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        transition_atom(scope, args, SharedAtomLifecycle::Deprecated).await
    }
}

pub struct Retire;

#[async_trait]
impl SemOsVerbOp for Retire {
    fn fqn(&self) -> &str {
        "shared-atom.retire"
    }
    async fn execute(
        &self,
        args: &Value,
        _ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        transition_atom(scope, args, SharedAtomLifecycle::Retired).await
    }
}

// ── shared-atom.list ──────────────────────────────────────────────────────────

pub struct List;

#[async_trait]
impl SemOsVerbOp for List {
    fn fqn(&self) -> &str {
        "shared-atom.list"
    }
    async fn execute(
        &self,
        args: &Value,
        _ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let status_filter = parse_lifecycle_filter(json_extract_string_opt(args, "status"))?;
        let atoms = repository::list_shared_atoms(scope.pool(), status_filter).await?;
        let records: Vec<Value> = atoms
            .into_iter()
            .map(serde_json::to_value)
            .collect::<Result<_, _>>()?;
        Ok(VerbExecutionOutcome::RecordSet(records))
    }
}

// ── shared-atom.list-consumers ────────────────────────────────────────────────

pub struct ListConsumers;

#[async_trait]
impl SemOsVerbOp for ListConsumers {
    fn fqn(&self) -> &str {
        "shared-atom.list-consumers"
    }
    async fn execute(
        &self,
        args: &Value,
        _ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let _atom_path = json_extract_string(args, "atom-path")?;
        // Phase 10 will implement the Level 0 DAG derivation.
        Ok(VerbExecutionOutcome::RecordSet(Vec::new()))
    }
}

// ── shared-atom.replay-constellation ──────────────────────────────────────────

pub struct ReplayConstellation;

#[async_trait]
impl SemOsVerbOp for ReplayConstellation {
    fn fqn(&self) -> &str {
        "shared-atom.replay-constellation"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let entity_id = json_extract_uuid(args, ctx, "entity-id")?;
        let constellation_family = json_extract_string(args, "constellation-family")?;
        let atom_path = json_extract_string(args, "atom-path")?;

        let atom = repository::get_by_path(scope.pool(), &atom_path)
            .await?
            .ok_or_else(|| anyhow!("Shared atom '{}' not found", atom_path))?;

        let current_version =
            fact_versions::current_version_number(scope.pool(), atom.id, entity_id).await?;
        let stale_refs =
            fact_refs::check_staleness_for_entity(scope.pool(), &constellation_family, entity_id)
                .await?;

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
            scope.pool(),
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

        Ok(VerbExecutionOutcome::Record(serde_json::to_value(result)?))
    }
}

// ── shared-atom.acknowledge-shared-update ─────────────────────────────────────

pub struct AcknowledgeSharedUpdate;

#[async_trait]
impl SemOsVerbOp for AcknowledgeSharedUpdate {
    fn fqn(&self) -> &str {
        "shared-atom.acknowledge-shared-update"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let entity_id = json_extract_uuid(args, ctx, "entity-id")?;
        let atom_path = json_extract_string(args, "atom-path")?;

        let atom = repository::get_by_path(scope.pool(), &atom_path)
            .await?
            .ok_or_else(|| anyhow!("Shared atom '{}' not found", atom_path))?;

        let current_version =
            fact_versions::current_version_number(scope.pool(), atom.id, entity_id).await?;

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
        .fetch_all(scope.executor())
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

        Ok(VerbExecutionOutcome::Record(serde_json::to_value(result)?))
    }
}
