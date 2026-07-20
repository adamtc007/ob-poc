//! Execution audit trail query helpers
//!
//! Provides query functions for auditing DSL executions by verb hash,
//! allowing reconstruction of what verb configuration was active when
//! a specific execution ran.

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

/// Convert bytes to hex string
fn bytes_to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Result from finding executions by verb hash
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ExecutionByVerbHash {
    pub idempotency_key: String,
    pub execution_id: Uuid,
    pub statement_index: i32,
    pub verb: String,
    pub result_type: String,
    pub result_id: Option<Uuid>,
    pub executed_at: DateTime<Utc>,
}

/// Result from the execution verb audit view
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ExecutionVerbAudit {
    pub execution_id: Uuid,
    pub verb: String,
    pub verb_hash: Option<Vec<u8>>,
    pub verb_hash_hex: Option<String>,
    pub current_verb_hash: Option<Vec<u8>>,
    pub current_verb_hash_hex: Option<String>,
    pub verb_config_changed: bool,
    pub executed_at: DateTime<Utc>,
}

/// Result from getting verb config at execution time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct VerbConfigAtExecution {
    pub verb_name: String,
    pub execution_verb_hash: Option<Vec<u8>>,
    pub execution_verb_hash_hex: Option<String>,
    pub current_verb_hash: Option<Vec<u8>>,
    pub current_verb_hash_hex: Option<String>,
    pub config_changed: bool,
    pub current_config_json: Option<serde_json::Value>,
}

/// Execution audit repository for verb hash queries
pub(crate) struct ExecutionAuditRepository {
    pool: PgPool,
}

impl ExecutionAuditRepository {
    pub(crate) fn new(pool: PgPool) -> Self {
        Self { pool }
    }





}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn test_config_changed_detection() {
        let hash1 = vec![1, 2, 3, 4];
        let hash2 = vec![1, 2, 3, 5]; // Different

        // Same hash = not changed
        assert!(!matches!(
            (&Some(hash1.clone()), &Some(hash1.clone())),
            (Some(a), Some(b)) if a != b
        ));

        // Different hash = changed
        assert!(matches!(
            (&Some(hash1.clone()), &Some(hash2.clone())),
            (Some(a), Some(b)) if a != b
        ));
    }
}
