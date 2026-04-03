//! Constellation replay for cross-workspace state consistency.
//!
//! When a shared fact is superseded and the consuming entity is complete,
//! the affected constellation is replayed from the top. Upsert semantics
//! guarantee unchanged state is a no-op; changed state flows through correctly.
//!
//! See: docs/architecture/cross-workspace-state-consistency-v0.4.md §4.3, §5.2

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::repl::types_v2::WorkspaceKind;

// ── RebuildContext ───────────────────────────────────────────────────

/// Why this replay was triggered.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplayTrigger {
    /// A shared fact was superseded — the canonical trigger for cross-workspace replay.
    SharedFactSupersession,
    /// Manual replay requested by operator.
    ManualReplay,
}

/// Metadata envelope for a constellation replay operation.
///
/// Extends the existing `ReplayEnvelope` pattern with shared-fact-correction
/// metadata (INV-4: replay routes through the existing runbook execution gate).
///
/// This is NOT a separate execution context type — it attaches to the standard
/// `RunbookPlan` / `CompiledRunbook` execution pipeline as metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebuildContext {
    /// Why this replay was triggered.
    pub trigger: ReplayTrigger,
    /// The shared atom path that was superseded (e.g., "entity.lei").
    pub source_atom_path: String,
    /// The shared atom registry ID.
    pub source_atom_id: Uuid,
    /// The version the consumer was operating against.
    pub prior_version: i32,
    /// The current (superseding) version.
    pub new_version: i32,
    /// The workspace that committed the superseding version.
    pub source_workspace: String,
    /// The consuming workspace being replayed.
    pub target_workspace: WorkspaceKind,
    /// The constellation family being replayed.
    pub target_constellation_family: String,
    /// The entity this replay is for.
    pub entity_id: Uuid,
    /// When the replay was initiated.
    pub initiated_at: DateTime<Utc>,
    /// Optional remediation event ID (if triggered by staleness propagation).
    pub remediation_id: Option<Uuid>,
}

// ── ReplayResult ─────────────────────────────────────────────────────

/// Outcome of a constellation replay attempt.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum ReplayOutcome {
    /// All steps replayed successfully. Consumer ref advanced.
    Resolved {
        steps_executed: usize,
        steps_unchanged: usize,
    },
    /// Replay halted at a specific step. Remediation escalated.
    Escalated {
        failed_step: usize,
        failed_verb: String,
        failure_reason: String,
        steps_completed_before_failure: usize,
    },
}

/// Full result of a constellation replay, including context and timing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayResult {
    pub context: RebuildContext,
    pub outcome: ReplayOutcome,
    pub started_at: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
}

impl ReplayResult {
    pub fn is_resolved(&self) -> bool {
        matches!(self.outcome, ReplayOutcome::Resolved { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rebuild_context_serde_roundtrip() {
        let ctx = RebuildContext {
            trigger: ReplayTrigger::SharedFactSupersession,
            source_atom_path: "entity.lei".to_string(),
            source_atom_id: Uuid::nil(),
            prior_version: 1,
            new_version: 2,
            source_workspace: "kyc".to_string(),
            target_workspace: WorkspaceKind::OnBoarding,
            target_constellation_family: "onboarding_workspace".to_string(),
            entity_id: Uuid::nil(),
            initiated_at: Utc::now(),
            remediation_id: None,
        };
        let json = serde_json::to_string(&ctx).unwrap();
        let back: RebuildContext = serde_json::from_str(&json).unwrap();
        assert_eq!(back.source_atom_path, "entity.lei");
        assert_eq!(back.prior_version, 1);
        assert_eq!(back.new_version, 2);
    }

    #[test]
    fn replay_outcome_variants() {
        let resolved = ReplayOutcome::Resolved {
            steps_executed: 47,
            steps_unchanged: 40,
        };
        let json = serde_json::to_string(&resolved).unwrap();
        assert!(json.contains("\"outcome\":\"resolved\""));

        let escalated = ReplayOutcome::Escalated {
            failed_step: 13,
            failed_verb: "custody.setup-sub-custodian".to_string(),
            failure_reason: "No sub-custodian for jurisdiction DE".to_string(),
            steps_completed_before_failure: 12,
        };
        let json = serde_json::to_string(&escalated).unwrap();
        assert!(json.contains("\"outcome\":\"escalated\""));
    }
}
