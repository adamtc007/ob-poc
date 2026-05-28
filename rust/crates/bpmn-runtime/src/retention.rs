//! Audit log retention policy for the journey runtime.
//!
//! [`RetentionPolicy`] is configuration-only. The runtime itself does not
//! enforce retention — callers (e.g., a maintenance job or `PostgresJourneyStore`)
//! use [`crate::store::JourneyStore::find_archivable_instances`] to discover
//! candidates and [`crate::store::JourneyStore::archive_instance_log`] to act.

use crate::types::InstanceId;

/// Retention configuration for journey log entries.
///
/// # Defaults
/// - Archive completed instances after 90 days.
/// - Move archived entries to cold storage after 7 years.
#[derive(Debug, Clone)]
pub struct RetentionPolicy {
    /// Completed instances older than this many days are eligible for archival.
    pub archive_after_days: u64,
    /// Archived entries older than this many years are eligible for cold storage.
    pub cold_storage_after_years: u64,
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self {
            archive_after_days: 90,
            cold_storage_after_years: 7,
        }
    }
}

/// Instances and entry counts identified as archivable under a given policy.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) struct RetentionCandidates {
    /// Instance IDs eligible for archival.
    pub instance_ids: Vec<InstanceId>,
    /// Total number of log entries across all candidate instances.
    pub log_entry_count: usize,
}
