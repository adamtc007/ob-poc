//! Authoring pipeline types — ChangeSet lifecycle, artifacts, reports.
//! Pure value types — no sqlx, no DB dependencies.
//! See: docs/semantic_os_research_governed_boundary_v0.4.md §3, §6.1

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── ChangeSet status (7-state, superset of stewardship statuses) ──

/// ChangeSet lifecycle status for the authoring pipeline.
///
/// Transitions:
///   Draft → Validated → DryRunPassed → Published
///   Draft → Rejected (validation failure)
///   Validated → Rejected (dry-run not attempted)
///   Validated → DryRunFailed
///   Published → Superseded (when successor declares supersedes_change_set_id)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeSetStatus {
    Draft,
    UnderReview,
    Approved,
    Validated,
    Rejected,
    DryRunPassed,
    DryRunFailed,
    Published,
    Superseded,
}

impl ChangeSetStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::UnderReview => "under_review",
            Self::Approved => "approved",
            Self::Validated => "validated",
            Self::Rejected => "rejected",
            Self::DryRunPassed => "dry_run_passed",
            Self::DryRunFailed => "dry_run_failed",
            Self::Published => "published",
            Self::Superseded => "superseded",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "draft" => Some(Self::Draft),
            "under_review" | "in_review" => Some(Self::UnderReview),
            "approved" => Some(Self::Approved),
            "validated" => Some(Self::Validated),
            "rejected" => Some(Self::Rejected),
            "dry_run_passed" => Some(Self::DryRunPassed),
            "dry_run_failed" => Some(Self::DryRunFailed),
            "published" => Some(Self::Published),
            "superseded" => Some(Self::Superseded),
            _ => None,
        }
    }

    /// Whether this status is terminal (no further transitions).
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Published | Self::Rejected | Self::DryRunFailed | Self::Superseded
        )
    }

    /// Whether this status is a non-terminal intermediate state.
    pub fn is_intermediate(&self) -> bool {
        matches!(
            self,
            Self::Draft | Self::UnderReview | Self::Approved | Self::Validated | Self::DryRunPassed
        )
    }
}

impl std::fmt::Display for ChangeSetStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// ── Artifact type ──────────────────────────────────────────────

/// Type discriminator for artifacts in a ChangeSet bundle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactType {
    MigrationSql,
    MigrationDownSql,
    VerbYaml,
    AttributeJson,
    TaxonomyJson,
    DocJson,
}

impl ArtifactType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::MigrationSql => "migration_sql",
            Self::MigrationDownSql => "migration_down_sql",
            Self::VerbYaml => "verb_yaml",
            Self::AttributeJson => "attribute_json",
            Self::TaxonomyJson => "taxonomy_json",
            Self::DocJson => "doc_json",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "migration_sql" => Some(Self::MigrationSql),
            "migration_down_sql" => Some(Self::MigrationDownSql),
            "verb_yaml" => Some(Self::VerbYaml),
            "attribute_json" => Some(Self::AttributeJson),
            "taxonomy_json" => Some(Self::TaxonomyJson),
            "doc_json" => Some(Self::DocJson),
            _ => None,
        }
    }
}

impl std::fmt::Display for ArtifactType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// ── ChangeSet (full row) ───────────────────────────────────────

/// A ChangeSet — an immutable bundle of artifacts with content-addressed idempotency.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeSetFull {
    pub change_set_id: Uuid,
    pub status: ChangeSetStatus,
    pub content_hash: String,
    pub hash_version: String,
    pub title: String,
    pub rationale: Option<String>,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
    pub supersedes_change_set_id: Option<Uuid>,
    pub superseded_by: Option<Uuid>,
    pub superseded_at: Option<DateTime<Utc>>,
    pub depends_on: Vec<Uuid>,
    pub evaluated_against_snapshot_set_id: Option<Uuid>,
}

// ── ChangeSet artifact ─────────────────────────────────────────

/// A single artifact within a ChangeSet bundle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeSetArtifact {
    pub artifact_id: Uuid,
    pub change_set_id: Uuid,
    pub artifact_type: ArtifactType,
    pub ordinal: i32,
    pub path: Option<String>,
    pub content: String,
    pub content_hash: String,
    pub metadata: Option<serde_json::Value>,
}

// ── ChangeSet manifest (input for propose) ─────────────────────

/// Input manifest for `propose_change_set`.
/// Parsed from the `changeset.yaml` file in the bundle root.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeSetManifest {
    pub title: String,
    pub rationale: Option<String>,
    pub depends_on: Vec<Uuid>,
    pub supersedes: Option<Uuid>,
    pub artifacts: Vec<ArtifactManifestEntry>,
}

/// An entry in the manifest describing an artifact.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactManifestEntry {
    pub artifact_type: ArtifactType,
    pub path: String,
    pub content_hash: Option<String>,
}

// ── Validation report ──────────────────────────────────────────

/// Stage discriminator for validation reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationStage {
    Validate,
    DryRun,
}

impl ValidationStage {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Validate => "validate",
            Self::DryRun => "dry_run",
        }
    }
}

/// A validation error with structured code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    pub code: String,
    pub severity: ErrorSeverity,
    pub message: String,
    pub artifact_path: Option<String>,
    pub line: Option<u32>,
    /// Optional structured context for diagnostics (e.g., dependency IDs,
    /// conflicting values). Skipped if None in serialization.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<serde_json::Value>,
}

/// Severity for validation errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorSeverity {
    Error,
    Warning,
    Info,
}

/// Stage 1 (pure) validation report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationReport {
    pub ok: bool,
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationError>,
}

impl ValidationReport {
    pub fn empty_ok() -> Self {
        Self {
            ok: true,
            errors: vec![],
            warnings: vec![],
        }
    }
}

/// Stage 2 (DB-backed) dry-run report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DryRunReport {
    pub ok: bool,
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationError>,
    pub scratch_schema_apply_ms: Option<u64>,
    pub diff_summary: Option<DiffSummary>,
}

// ── Diff summary ───────────────────────────────────────────────

/// Structural diff summary between two artifact sets or against active snapshot set.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffSummary {
    pub added: Vec<DiffEntry>,
    pub modified: Vec<DiffEntry>,
    pub removed: Vec<DiffEntry>,
    pub breaking_changes: Vec<DiffEntry>,
}

/// A single entry in a diff summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffEntry {
    pub fqn: String,
    pub object_type: String,
    pub detail: Option<String>,
}

// ── Publish plan (blast-radius analysis) ──────────────────────

/// Publish plan with blast-radius analysis.
/// Returned by `plan_publish` — read-only, does not modify state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishPlan {
    /// The ChangeSet being planned.
    pub change_set_id: Uuid,
    /// Current status of the ChangeSet.
    pub status: ChangeSetStatus,
    /// Structural diff summary.
    pub diff: DiffSummary,
    /// Number of forward DDL migrations in the bundle.
    pub migration_count: usize,
    /// Number of down (rollback) migrations in the bundle.
    pub down_migration_count: usize,
    /// Whether any breaking changes were detected.
    pub has_breaking_changes: bool,
    /// Count of breaking changes.
    pub breaking_change_count: usize,
    /// Distinct artifact types affected.
    pub affected_artifact_types: Vec<String>,
    /// Whether this ChangeSet supersedes another.
    pub supersedes: Option<Uuid>,
    /// IDs of ChangeSets this one depends on.
    pub depends_on: Vec<Uuid>,
    /// Whether a stale dry-run was detected (evaluated_against != current active).
    pub stale_dry_run: bool,
    /// The snapshot set the dry-run was evaluated against.
    pub evaluated_against_snapshot_set_id: Option<Uuid>,
    /// The current active snapshot set (for drift comparison).
    pub current_active_snapshot_set_id: Option<Uuid>,
}

// ── Governance audit ───────────────────────────────────────────

/// Result of a governance verb invocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "status")]
pub enum AuditResult {
    Success { detail: Option<String> },
    Failure { code: String, message: String },
}

// AgentMode re-exported from agent_mode.rs (single canonical definition)
pub use super::agent_mode::AgentMode;

/// A governance audit log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceAuditEntry {
    pub entry_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub verb: String,
    pub agent_session_id: Option<Uuid>,
    pub agent_mode: Option<AgentMode>,
    pub change_set_id: Option<Uuid>,
    pub snapshot_set_id: Option<Uuid>,
    pub active_snapshot_set_id: Uuid,
    pub result: AuditResult,
    pub duration_ms: u64,
    pub metadata: Option<serde_json::Value>,
}

// ── Publish batch ──────────────────────────────────────────────

/// Record of a batch publish operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishBatch {
    pub batch_id: Uuid,
    pub change_set_ids: Vec<Uuid>,
    pub snapshot_set_id: Uuid,
    pub published_at: DateTime<Utc>,
    pub publisher: String,
}

// ── Health types ──────────────────────────────────────────────

/// Health check: active snapshot set info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveSnapshotHealth {
    pub active_snapshot_set_id: Option<Uuid>,
    pub object_count: i64,
}

/// Health check: pending ChangeSet counts by status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingChangeSetsHealth {
    pub counts: Vec<StatusCount>,
    pub total_pending: i64,
}

/// A (status, count) pair for health reporting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusCount {
    pub status: String,
    pub count: i64,
}

/// Health check: stale dry-run info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaleDryRunsHealth {
    pub stale_count: i64,
    pub stale_change_set_ids: Vec<Uuid>,
}

/// Health check: outbox projection lag.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectionLagHealth {
    pub unprocessed_events: i64,
    pub oldest_pending_age_seconds: Option<i64>,
}

// ── Tests ──────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_change_set_status_round_trip() {
        for status in [
            ChangeSetStatus::Draft,
            ChangeSetStatus::UnderReview,
            ChangeSetStatus::Approved,
            ChangeSetStatus::Validated,
            ChangeSetStatus::Rejected,
            ChangeSetStatus::DryRunPassed,
            ChangeSetStatus::DryRunFailed,
            ChangeSetStatus::Published,
            ChangeSetStatus::Superseded,
        ] {
            let s = status.as_str();
            assert_eq!(
                ChangeSetStatus::parse(s),
                Some(status),
                "round-trip failed for {s}"
            );
        }
    }

    #[test]
    fn test_artifact_type_round_trip() {
        for at in [
            ArtifactType::MigrationSql,
            ArtifactType::MigrationDownSql,
            ArtifactType::VerbYaml,
            ArtifactType::AttributeJson,
            ArtifactType::TaxonomyJson,
            ArtifactType::DocJson,
        ] {
            let s = at.as_str();
            assert_eq!(
                ArtifactType::parse(s),
                Some(at),
                "round-trip failed for {s}"
            );
        }
    }

    #[test]
    fn test_terminal_statuses() {
        assert!(ChangeSetStatus::Published.is_terminal());
        assert!(ChangeSetStatus::Rejected.is_terminal());
        assert!(ChangeSetStatus::Superseded.is_terminal());
        assert!(ChangeSetStatus::DryRunFailed.is_terminal());
        assert!(!ChangeSetStatus::Draft.is_terminal());
        assert!(!ChangeSetStatus::UnderReview.is_terminal());
        assert!(!ChangeSetStatus::Approved.is_terminal());
        assert!(!ChangeSetStatus::Validated.is_terminal());
        assert!(!ChangeSetStatus::DryRunPassed.is_terminal());
    }

    #[test]
    fn test_validation_report_empty_ok() {
        let report = ValidationReport::empty_ok();
        assert!(report.ok);
        assert!(report.errors.is_empty());
    }

    #[test]
    fn test_audit_result_serde() {
        let success = AuditResult::Success {
            detail: Some("all good".into()),
        };
        let json = serde_json::to_value(&success).unwrap();
        assert_eq!(json["status"], "success");

        let failure = AuditResult::Failure {
            code: "PUBLISH:DRIFT_DETECTED".into(),
            message: "snapshot set changed".into(),
        };
        let json = serde_json::to_value(&failure).unwrap();
        assert_eq!(json["status"], "failure");
        assert_eq!(json["code"], "PUBLISH:DRIFT_DETECTED");
    }
}
