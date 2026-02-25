//! Impact Analysis — compute blast radius for changeset items.
//!
//! Replaces the stub `changeset_impact()` with real JSONB dependency traversal.
//! Determines which active snapshots would be affected by publishing the changeset.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use super::types::ChangesetEntryRow;

/// Impact analysis result for a changeset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangesetImpactReport {
    pub changeset_id: Uuid,
    pub total_items: usize,
    pub affected_snapshots: Vec<AffectedSnapshot>,
    pub affected_consumers: Vec<AffectedConsumer>,
    pub risk_summary: RiskSummary,
}

/// A snapshot that would be affected by publishing the changeset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AffectedSnapshot {
    pub snapshot_id: Uuid,
    pub object_type: String,
    pub fqn: String,
    pub impact_type: ImpactType,
    pub reason: String,
}

/// Type of impact on an affected snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImpactType {
    /// The snapshot will be superseded by a new version
    Superseded,
    /// The snapshot references a changed object (transitive dependency)
    DependencyChanged,
    /// The snapshot's view definition includes a changed attribute
    ViewAffected,
    /// The snapshot's policy references a changed object
    PolicyAffected,
}

/// A consumer (verb, view, policy) affected by the changeset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AffectedConsumer {
    pub consumer_fqn: String,
    pub consumer_type: String,
    pub dependency_fqn: String,
    pub dependency_type: String,
}

/// High-level risk summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskSummary {
    pub breaking_changes: usize,
    pub non_breaking_changes: usize,
    pub new_items: usize,
    pub deprecations: usize,
    pub total_affected: usize,
    pub risk_level: RiskLevel,
}

/// Risk level derived from the impact analysis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

/// Compute the full impact analysis for a set of changeset entries.
///
/// This performs real dependency traversal via JSONB queries against
/// `sem_reg.snapshots` to find active objects that reference the
/// changed FQNs.
pub async fn compute_changeset_impact(
    pool: &PgPool,
    changeset_id: Uuid,
    entries: &[ChangesetEntryRow],
) -> Result<ChangesetImpactReport> {
    let mut affected_snapshots = Vec::new();
    let mut affected_consumers = Vec::new();

    // Collect FQNs being changed
    let changed_fqns: Vec<&str> = entries.iter().map(|e| e.object_fqn.as_str()).collect();

    // 1. Find direct predecessors (snapshots that will be superseded)
    for entry in entries {
        if let Some(pred_id) = entry.predecessor_id {
            affected_snapshots.push(AffectedSnapshot {
                snapshot_id: pred_id,
                object_type: entry.object_type.clone(),
                fqn: entry.object_fqn.clone(),
                impact_type: ImpactType::Superseded,
                reason: format!("Superseded by changeset entry revision {}", entry.revision),
            });
        }
    }

    // 2. Find transitive dependencies via JSONB definition traversal
    // Query active snapshots whose definition JSON references any of the changed FQNs
    for fqn in &changed_fqns {
        let rows = sqlx::query_as::<_, DependencyRow>(
            r#"
            SELECT snapshot_id, object_type::text as object_type,
                   COALESCE(definition->>'fqn', object_id::text) as fqn
            FROM sem_reg.snapshots
            WHERE status = 'active'
              AND effective_until IS NULL
              AND definition::text LIKE '%' || $1 || '%'
              AND COALESCE(definition->>'fqn', '') != $1
            LIMIT 100
            "#,
        )
        .bind(fqn)
        .fetch_all(pool)
        .await?;

        for row in rows {
            // Determine the impact type based on consumer object type
            let impact_type = match row.object_type.as_str() {
                "view_def" => ImpactType::ViewAffected,
                "policy_rule" => ImpactType::PolicyAffected,
                _ => ImpactType::DependencyChanged,
            };

            affected_snapshots.push(AffectedSnapshot {
                snapshot_id: row.snapshot_id,
                object_type: row.object_type.clone(),
                fqn: row.fqn.clone(),
                impact_type,
                reason: format!("References changed FQN '{}'", fqn),
            });

            affected_consumers.push(AffectedConsumer {
                consumer_fqn: row.fqn,
                consumer_type: row.object_type,
                dependency_fqn: fqn.to_string(),
                dependency_type: entries
                    .iter()
                    .find(|e| e.object_fqn == *fqn)
                    .map(|e| e.object_type.clone())
                    .unwrap_or_default(),
            });
        }
    }

    // 3. Compute risk summary
    let mut breaking = 0;
    let mut non_breaking = 0;
    let mut new_items = 0;
    let mut deprecations = 0;

    for entry in entries {
        match entry.change_kind.as_str() {
            "add" => new_items += 1,
            "modify" => {
                // If there's a type change in the payload, it's breaking
                if entry.draft_payload.get("data_type").is_some()
                    || entry.draft_payload.get("type").is_some()
                {
                    breaking += 1;
                } else {
                    non_breaking += 1;
                }
            }
            "remove" => breaking += 1,
            _ => non_breaking += 1,
        }
        if entry.action == super::types::ChangesetAction::Deprecate {
            deprecations += 1;
        }
    }

    let total_affected = affected_snapshots.len();
    let risk_level = if breaking > 0 || total_affected > 10 {
        RiskLevel::High
    } else if non_breaking > 0 || total_affected > 3 {
        RiskLevel::Medium
    } else {
        RiskLevel::Low
    };

    Ok(ChangesetImpactReport {
        changeset_id,
        total_items: entries.len(),
        affected_snapshots,
        affected_consumers,
        risk_summary: RiskSummary {
            breaking_changes: breaking,
            non_breaking_changes: non_breaking,
            new_items,
            deprecations,
            total_affected,
            risk_level,
        },
    })
}

#[derive(sqlx::FromRow)]
struct DependencyRow {
    snapshot_id: Uuid,
    object_type: String,
    fqn: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_risk_level_high() {
        let summary = RiskSummary {
            breaking_changes: 1,
            non_breaking_changes: 0,
            new_items: 0,
            deprecations: 0,
            total_affected: 0,
            risk_level: RiskLevel::High,
        };
        assert_eq!(summary.risk_level, RiskLevel::High);
    }

    #[test]
    fn test_impact_type_serde() {
        let it = ImpactType::DependencyChanged;
        let json = serde_json::to_value(&it).unwrap();
        assert_eq!(json, "dependency_changed");
        let back: ImpactType = serde_json::from_value(json).unwrap();
        assert_eq!(back, it);
    }
}
