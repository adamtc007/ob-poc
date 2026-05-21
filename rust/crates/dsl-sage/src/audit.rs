//! In-memory audit log for Sage state-machine transitions.
//!
//! Each state transition in [`crate::orchestrator::SageOrchestrator::step`]
//! calls [`SageAuditLog::record`] to append an entry.  Tranche 7 will persist
//! this to Postgres; for now the log is held in a `Mutex<Vec<_>>` inside the
//! caller's process.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Entry type
// ---------------------------------------------------------------------------

/// A single audit-log entry recording a Sage state-machine transition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SageAuditEntry {
    /// The session this entry belongs to.
    pub session_id: String,
    /// RFC 3339 timestamp of the transition.
    pub timestamp: String,
    /// Human-readable description, e.g. `"Listening→Matching"`.
    pub transition: String,
    /// Arbitrary structured details (pack name, candidate count, etc.).
    pub details: serde_json::Value,
}

// ---------------------------------------------------------------------------
// Log
// ---------------------------------------------------------------------------

/// In-memory audit log (Tranche 7 will persist this to Postgres).
pub struct SageAuditLog {
    entries: std::sync::Mutex<Vec<SageAuditEntry>>,
}

impl SageAuditLog {
    /// Create an empty log.
    pub fn new() -> Self {
        Self {
            entries: std::sync::Mutex::new(vec![]),
        }
    }

    /// Append one entry.
    pub fn record(&self, session_id: &str, transition: &str, details: serde_json::Value) {
        self.entries.lock().unwrap().push(SageAuditEntry {
            session_id: session_id.to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            transition: transition.to_string(),
            details,
        });
    }

    /// Return all entries for a given session, in insertion order.
    pub fn entries_for_session(&self, session_id: &str) -> Vec<SageAuditEntry> {
        self.entries
            .lock()
            .unwrap()
            .iter()
            .filter(|e| e.session_id == session_id)
            .cloned()
            .collect()
    }
}

impl Default for SageAuditLog {
    fn default() -> Self {
        Self::new()
    }
}
