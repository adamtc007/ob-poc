//! SemReg verb filter for the compilation pipeline (§6.2 Step 4).
//!
//! After macro expansion, DAG assembly, and pack constraint checking, the
//! expanded verb set is filtered against the Semantic Registry's context
//! resolution. Verbs that SemReg denies are rejected before write_set
//! derivation or storage.
//!
//! ## Design
//!
//! - **Optional**: The `SemRegFilter` is `Option<SemRegFilter>`. When `None`,
//!   SemReg filtering is skipped (graceful degradation).
//! - **Fail-open on unavailability**: If `resolve_context()` returns an error
//!   (DB down, timeout), the filter logs a warning and allows all verbs. This
//!   matches the existing pattern in `agent/orchestrator.rs`.
//! - **Fail-closed on explicit deny**: If SemReg resolves successfully but
//!   denies verbs, those verbs are rejected.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// SemRegFilterResult
// ---------------------------------------------------------------------------

/// Result of filtering expanded verbs against SemReg.
#[derive(Debug, Clone)]
pub struct SemRegFilterResult {
    /// Verbs that SemReg allows (or all verbs if SemReg is unavailable).
    pub allowed: Vec<String>,

    /// Verbs denied with reasons (empty if all allowed or SemReg unavailable).
    pub denied_with_reasons: Vec<DeniedVerb>,

    /// Whether SemReg was actually consulted (false if unavailable/skipped).
    pub sem_reg_consulted: bool,
}

/// A verb that was denied by SemReg.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeniedVerb {
    /// The verb FQN that was denied.
    pub verb: String,

    /// Human-readable reason for denial.
    pub reason: String,
}

impl SemRegFilterResult {
    /// All verbs allowed (SemReg unavailable or not configured).
    pub fn allow_all(verbs: &[String]) -> Self {
        Self {
            allowed: verbs.to_vec(),
            denied_with_reasons: vec![],
            sem_reg_consulted: false,
        }
    }

    /// Whether any verbs were denied.
    pub fn has_denials(&self) -> bool {
        !self.denied_with_reasons.is_empty()
    }

    /// Get the first denied verb (for error reporting).
    pub fn first_denied(&self) -> Option<&DeniedVerb> {
        self.denied_with_reasons.first()
    }
}

// ---------------------------------------------------------------------------
// filter_verbs_against_allowed_set — pure function
// ---------------------------------------------------------------------------

/// Filter a set of expanded verbs against an allowed set from SemReg.
///
/// This is a pure function that doesn't call SemReg directly — the caller
/// is responsible for obtaining the allowed set (via `resolve_context()`
/// or `ContextEnvelope`).
///
/// # Arguments
///
/// * `expanded_verbs` — verbs from macro expansion
/// * `allowed_verbs` — verbs allowed by SemReg (from `resolve_context().candidate_verbs`)
///
/// # Returns
///
/// A `SemRegFilterResult` with allowed and denied verbs.
pub fn filter_verbs_against_allowed_set(
    expanded_verbs: &[String],
    allowed_verbs: &HashSet<String>,
) -> SemRegFilterResult {
    let mut allowed = Vec::new();
    let mut denied = Vec::new();

    for verb in expanded_verbs {
        if allowed_verbs.contains(verb) {
            allowed.push(verb.clone());
        } else {
            denied.push(DeniedVerb {
                verb: verb.clone(),
                reason: format!(
                    "Verb '{}' is not in the SemReg allowed set for the current context",
                    verb
                ),
            });
        }
    }

    SemRegFilterResult {
        allowed,
        denied_with_reasons: denied,
        sem_reg_consulted: true,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_all_allowed() {
        let expanded = vec!["cbu.create".to_string(), "entity.create".to_string()];
        let mut allowed = HashSet::new();
        allowed.insert("cbu.create".to_string());
        allowed.insert("entity.create".to_string());
        allowed.insert("session.info".to_string());

        let result = filter_verbs_against_allowed_set(&expanded, &allowed);
        assert!(!result.has_denials());
        assert_eq!(result.allowed.len(), 2);
        assert!(result.sem_reg_consulted);
    }

    #[test]
    fn test_filter_one_denied() {
        let expanded = vec![
            "cbu.create".to_string(),
            "entity.delete".to_string(),
            "session.info".to_string(),
        ];
        let mut allowed = HashSet::new();
        allowed.insert("cbu.create".to_string());
        allowed.insert("session.info".to_string());
        // entity.delete NOT in allowed set

        let result = filter_verbs_against_allowed_set(&expanded, &allowed);
        assert!(result.has_denials());
        assert_eq!(result.denied_with_reasons.len(), 1);
        assert_eq!(result.denied_with_reasons[0].verb, "entity.delete");
        assert_eq!(result.allowed.len(), 2);
    }

    #[test]
    fn test_filter_all_denied() {
        let expanded = vec!["dangerous.verb".to_string()];
        let allowed = HashSet::new(); // Empty — deny all

        let result = filter_verbs_against_allowed_set(&expanded, &allowed);
        assert!(result.has_denials());
        assert_eq!(result.denied_with_reasons.len(), 1);
        assert!(result.allowed.is_empty());
    }

    #[test]
    fn test_allow_all_bypass() {
        let verbs = vec!["a".to_string(), "b".to_string()];
        let result = SemRegFilterResult::allow_all(&verbs);
        assert!(!result.has_denials());
        assert!(!result.sem_reg_consulted);
        assert_eq!(result.allowed.len(), 2);
    }

    #[test]
    fn test_first_denied() {
        let expanded = vec!["a".to_string(), "b".to_string()];
        let allowed = HashSet::new();

        let result = filter_verbs_against_allowed_set(&expanded, &allowed);
        assert!(result.first_denied().is_some());
        assert_eq!(result.first_denied().map(|d| d.verb.as_str()), Some("a"));
    }

    #[test]
    fn test_empty_expanded_verbs() {
        let expanded: Vec<String> = vec![];
        let allowed = HashSet::new();

        let result = filter_verbs_against_allowed_set(&expanded, &allowed);
        assert!(!result.has_denials());
        assert!(result.allowed.is_empty());
    }
}
