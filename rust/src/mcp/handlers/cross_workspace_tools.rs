//! MCP tool handlers for cross-workspace state consistency.
//!
//! Read-only diagnostic/query tools. Write operations go through DSL verbs.

use anyhow::{anyhow, Result};
use serde_json::{json, Value};

use super::core::ToolHandlers;

impl ToolHandlers {
    /// List shared atoms with optional lifecycle status filter.
    pub(super) async fn shared_atom_list(&self, args: Value) -> Result<Value> {
        let pool = self.require_pool()?;
        let status_filter = args["status"].as_str();

        let lifecycle = status_filter
            .map(dsl_runtime::cross_workspace::types::SharedAtomLifecycle::try_from_str)
            .transpose()?;

        let atoms = dsl_runtime::cross_workspace::repository::list_shared_atoms(pool, lifecycle).await?;

        Ok(json!({
            "atoms": atoms,
            "count": atoms.len(),
        }))
    }

    /// Show platform DAG consumers for a shared atom.
    pub(super) async fn shared_atom_consumers(&self, args: Value) -> Result<Value> {
        let pool = self.require_pool()?;
        let atom_path = args["atom_path"]
            .as_str()
            .ok_or_else(|| anyhow!("atom_path required"))?;

        let atom = dsl_runtime::cross_workspace::repository::get_by_path(pool, atom_path)
            .await?
            .ok_or_else(|| anyhow!("Shared atom '{}' not found", atom_path))?;

        // Build platform DAG to find consumers
        // For now, return atom details + lifecycle. Full DAG derivation
        // requires verb footprint loading which is boot-time only.
        Ok(json!({
            "atom_path": atom.atom_path,
            "display_name": atom.display_name,
            "owner_workspace": atom.owner_workspace,
            "owner_constellation_family": atom.owner_constellation_family,
            "lifecycle_status": atom.lifecycle_status,
            "activated_at": atom.activated_at,
        }))
    }

    /// Check stale shared fact refs for an entity in a workspace.
    pub(super) async fn staleness_check(&self, args: Value) -> Result<Value> {
        let pool = self.require_pool()?;
        let consumer_workspace = args["workspace"]
            .as_str()
            .ok_or_else(|| anyhow!("workspace required"))?;

        let entity_id = args["entity_id"]
            .as_str()
            .and_then(|s| s.parse::<uuid::Uuid>().ok());

        let stale_refs = if let Some(eid) = entity_id {
            dsl_runtime::cross_workspace::fact_refs::check_staleness_for_entity(
                pool,
                consumer_workspace,
                eid,
            )
            .await?
        } else {
            dsl_runtime::cross_workspace::fact_refs::list_stale_refs(pool, consumer_workspace).await?
        };

        Ok(json!({
            "workspace": consumer_workspace,
            "stale_count": stale_refs.len(),
            "stale_refs": stale_refs,
        }))
    }

    /// List unresolved remediation events.
    pub(super) async fn remediation_list_open(&self, args: Value) -> Result<Value> {
        let pool = self.require_pool()?;
        let entity_id = args["entity_id"]
            .as_str()
            .and_then(|s| s.parse::<uuid::Uuid>().ok());
        let workspace = args["workspace"].as_str();

        let events =
            dsl_runtime::cross_workspace::remediation::list_open(pool, entity_id, workspace).await?;

        Ok(json!({
            "open_count": events.len(),
            "events": events,
        }))
    }

    /// Get details of a single remediation event.
    pub(super) async fn remediation_status(&self, args: Value) -> Result<Value> {
        let pool = self.require_pool()?;
        let id_str = args["remediation_id"]
            .as_str()
            .ok_or_else(|| anyhow!("remediation_id required"))?;
        let id: uuid::Uuid = id_str.parse().map_err(|_| anyhow!("Invalid UUID"))?;

        let event = dsl_runtime::cross_workspace::remediation::get_by_id(pool, id)
            .await?
            .ok_or_else(|| anyhow!("Remediation event {} not found", id))?;

        Ok(json!({
            "id": event.id,
            "entity_id": event.entity_id,
            "source_workspace": event.source_workspace,
            "affected_workspace": event.affected_workspace,
            "affected_constellation_family": event.affected_constellation_family,
            "status": event.status,
            "prior_version": event.prior_version,
            "new_version": event.new_version,
            "failed_at_step": event.failed_at_step,
            "failure_reason": event.failure_reason,
            "deferral_reason": event.deferral_reason,
            "created_at": event.created_at.to_rfc3339(),
            "resolved_at": event.resolved_at.map(|t| t.to_rfc3339()),
        }))
    }

    /// List provider capability classifications.
    pub(super) async fn provider_capabilities(&self, args: Value) -> Result<Value> {
        let pool = self.require_pool()?;
        let provider_filter = args["provider"].as_str();

        let caps = if let Some(provider) = provider_filter {
            dsl_runtime::cross_workspace::providers::list_for_provider(pool, provider).await?
        } else {
            dsl_runtime::cross_workspace::providers::list_all(pool).await?
        };

        Ok(json!({
            "capabilities": caps,
            "count": caps.len(),
        }))
    }

    /// Query compensation records for a remediation event.
    pub(super) async fn compensation_audit(&self, args: Value) -> Result<Value> {
        let pool = self.require_pool()?;
        let id_str = args["remediation_id"]
            .as_str()
            .ok_or_else(|| anyhow!("remediation_id required"))?;
        let id: uuid::Uuid = id_str.parse().map_err(|_| anyhow!("Invalid UUID"))?;

        let records = dsl_runtime::cross_workspace::compensation::list_for_remediation(pool, id).await?;

        Ok(json!({
            "remediation_id": id,
            "records": records,
            "count": records.len(),
        }))
    }

    /// Get version history for a shared fact.
    pub(super) async fn shared_fact_history(&self, args: Value) -> Result<Value> {
        let pool = self.require_pool()?;
        let atom_path = args["atom_path"]
            .as_str()
            .ok_or_else(|| anyhow!("atom_path required"))?;
        let entity_id_str = args["entity_id"]
            .as_str()
            .ok_or_else(|| anyhow!("entity_id required"))?;
        let entity_id: uuid::Uuid = entity_id_str
            .parse()
            .map_err(|_| anyhow!("Invalid entity_id UUID"))?;

        // Resolve atom
        let atom = dsl_runtime::cross_workspace::repository::get_by_path(pool, atom_path)
            .await?
            .ok_or_else(|| anyhow!("Shared atom '{}' not found", atom_path))?;

        let versions =
            dsl_runtime::cross_workspace::fact_versions::get_version_history(pool, atom.id, entity_id)
                .await?;

        let version_summaries: Vec<Value> = versions
            .iter()
            .map(|v| {
                json!({
                    "version": v.version,
                    "value": v.value,
                    "is_current": v.is_current,
                    "mutated_by_verb": v.mutated_by_verb,
                    "mutated_at": v.mutated_at.to_rfc3339(),
                })
            })
            .collect();

        Ok(json!({
            "atom_path": atom_path,
            "entity_id": entity_id,
            "versions": version_summaries,
            "version_count": version_summaries.len(),
        }))
    }
}
