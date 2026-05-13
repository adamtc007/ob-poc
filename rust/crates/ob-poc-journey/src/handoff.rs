//! Pack Handoff Types
//!
//! When one pack completes, it can hand off context to another pack.
//! This module defines the types for that context-forwarding mechanism.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Context forwarded from one pack's completed runbook to a target pack.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackHandoff {
    /// The runbook that completed and triggered this handoff.
    pub source_runbook_id: Uuid,

    /// The target pack to activate next.
    pub target_pack_id: String,

    /// Key-value context to carry forward (e.g. client_group_id, created entities).
    pub forwarded_context: HashMap<String, String>,

    /// Entry IDs from the source runbook whose outcomes are relevant to the target.
    pub forwarded_outcomes: Vec<Uuid>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handoff_roundtrip() {
        let handoff = PackHandoff {
            source_runbook_id: Uuid::new_v4(),
            target_pack_id: "kyc-case".to_string(),
            forwarded_context: HashMap::from([
                ("client_group_id".to_string(), Uuid::new_v4().to_string()),
                ("cbu_id".to_string(), Uuid::new_v4().to_string()),
            ]),
            forwarded_outcomes: vec![Uuid::new_v4(), Uuid::new_v4()],
        };

        let json = serde_json::to_string(&handoff).expect("serialize");
        let deserialized: PackHandoff = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(deserialized.target_pack_id, "kyc-case");
        assert_eq!(deserialized.forwarded_context.len(), 2);
        assert_eq!(deserialized.forwarded_outcomes.len(), 2);
    }
}
