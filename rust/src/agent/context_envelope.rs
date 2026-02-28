//! ContextEnvelope — structured SemReg resolution result.
//!
//! Replaces the flat `SemRegVerbPolicy` enum with a rich envelope that preserves:
//! - Allowed verb set with contract summaries
//! - Pruned verbs with structured reasons
//! - Deterministic fingerprint (SHA-256 of sorted FQNs)
//! - Evidence gaps, governance signals, snapshot provenance

use chrono::{DateTime, Utc};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use uuid::Uuid;

use sem_os_core::abac::AccessDecision;
use sem_os_core::context_resolution::ContextResolutionResponse;

/// Structured result of SemReg context resolution.
///
/// Carries the full resolution output — not just a bare `HashSet<String>`.
/// Every orchestrator call produces one of these (or `unavailable()` if SemReg
/// is not configured).
#[derive(Debug, Clone, Serialize)]
pub struct ContextEnvelope {
    /// Verbs explicitly allowed by ABAC + tier + preconditions.
    pub allowed_verbs: HashSet<String>,
    /// Summary of each allowed verb contract (for downstream consumers).
    pub allowed_verb_contracts: Vec<VerbCandidateSummary>,
    /// Verbs that were considered but pruned, with structured reasons.
    pub pruned_verbs: Vec<PrunedVerb>,
    /// SHA-256 fingerprint of the sorted allowed verb FQN set.
    /// Deterministic: same verbs → same fingerprint.
    pub fingerprint: AllowedVerbSetFingerprint,
    /// Evidence gaps identified during resolution.
    pub evidence_gaps: Vec<String>,
    /// Governance signals (staleness, unowned objects, etc.).
    pub governance_signals: Vec<GovernanceSignalSummary>,
    /// Snapshot set ID that was resolved against (provenance).
    pub snapshot_set_id: Option<String>,
    /// When this envelope was computed.
    pub computed_at: DateTime<Utc>,
    /// Whether this is a "deny all" result (resolution succeeded, zero verbs).
    deny_all: bool,
    /// Whether SemReg was unavailable (resolution failed or not configured).
    unavailable: bool,
}

/// Summary of an allowed verb contract (lightweight projection).
#[derive(Debug, Clone, Serialize)]
pub struct VerbCandidateSummary {
    pub fqn: String,
    pub description: String,
    pub governance_tier: String,
    pub rank_score: f64,
    pub preconditions_met: bool,
    pub verb_snapshot_id: Uuid,
}

/// A verb that was pruned from the allowed set with a structured reason.
#[derive(Debug, Clone, Serialize)]
pub struct PrunedVerb {
    pub fqn: String,
    pub reason: PruneReason,
}

/// Why a verb was pruned from the allowed set.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PruneReason {
    /// ABAC denied access.
    AbacDenied {
        actor_role: String,
        required: String,
    },
    /// Entity kind mismatch (verb applies to different entity types).
    EntityKindMismatch {
        verb_kinds: Vec<String>,
        subject_kind: String,
    },
    /// Tier excluded by view/evidence mode.
    TierExcluded { tier: String, reason: String },
    /// No taxonomy overlap between verb and subject.
    TaxonomyNoOverlap { verb_taxonomies: Vec<String> },
    /// Preconditions not met.
    PreconditionFailed { precondition: String },
    /// AgentMode blocked the verb.
    AgentModeBlocked { mode: String },
    /// Policy rule denied the verb.
    PolicyDenied { policy_fqn: String, reason: String },
}

/// SHA-256 fingerprint of sorted allowed verb FQNs.
///
/// Deterministic: identical verb sets always produce the same fingerprint.
/// Format: `"v1:<hex>"` (versioned for future algorithm changes).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AllowedVerbSetFingerprint(pub String);

impl std::fmt::Display for AllowedVerbSetFingerprint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl AllowedVerbSetFingerprint {
    /// Compute from a set of allowed verb FQNs.
    pub fn compute(allowed: &HashSet<String>) -> Self {
        let mut sorted: Vec<&str> = allowed.iter().map(|s| s.as_str()).collect();
        sorted.sort();
        let joined = sorted.join("\n");

        let mut hasher = Sha256::new();
        hasher.update(joined.as_bytes());
        let hash = hasher.finalize();
        let hex = hex::encode(hash);
        AllowedVerbSetFingerprint(format!("v1:{hex}"))
    }

    /// Empty fingerprint (no verbs).
    pub fn empty() -> Self {
        Self::compute(&HashSet::new())
    }
}

/// Governance signal summary (lightweight projection of GovernanceSignal).
#[derive(Debug, Clone, Serialize)]
pub struct GovernanceSignalSummary {
    pub kind: String,
    pub message: String,
    pub severity: String,
    pub related_fqn: Option<String>,
}

/// Result of a TOCTOU (Time-of-Check / Time-of-Use) recheck.
///
/// After initial SemReg resolution and verb selection, a recheck confirms
/// the selected verb is still in the allowed set. This guards against
/// policy changes between resolution and execution.
///
/// Only performed when `OBPOC_STRICT_SEMREG=true`.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum TocTouResult {
    /// Fingerprint unchanged — the allowed set is identical.
    StillAllowed,
    /// Fingerprint changed but the selected verb is still allowed.
    /// Logged as a governance warning but execution proceeds.
    AllowedButDrifted {
        new_fingerprint: AllowedVerbSetFingerprint,
    },
    /// The selected verb is no longer in the allowed set.
    /// Execution is blocked with a structured error.
    Denied {
        verb_fqn: String,
        new_fingerprint: AllowedVerbSetFingerprint,
    },
}

impl ContextEnvelope {
    /// Build from a successful `ContextResolutionResponse`.
    ///
    /// Partitions candidate_verbs into allowed (AccessDecision::Allow) and
    /// pruned (everything else), preserving structured prune reasons.
    pub fn from_resolution(response: &ContextResolutionResponse) -> Self {
        let mut allowed = HashSet::new();
        let mut allowed_contracts = Vec::new();
        let mut pruned = Vec::new();

        for vc in &response.candidate_verbs {
            if matches!(vc.access_decision, AccessDecision::Allow) {
                allowed.insert(vc.fqn.clone());
                allowed_contracts.push(VerbCandidateSummary {
                    fqn: vc.fqn.clone(),
                    description: vc.description.clone(),
                    governance_tier: format!("{:?}", vc.governance_tier),
                    rank_score: vc.rank_score,
                    preconditions_met: vc.preconditions_met,
                    verb_snapshot_id: vc.verb_snapshot_id,
                });
            } else {
                pruned.push(PrunedVerb {
                    fqn: vc.fqn.clone(),
                    reason: prune_reason_from_candidate(vc),
                });
            }
        }

        let fingerprint = AllowedVerbSetFingerprint::compute(&allowed);

        let evidence_gaps: Vec<String> = response
            .governance_signals
            .iter()
            .filter(|s| {
                matches!(
                    s.kind,
                    sem_os_core::context_resolution::GovernanceSignalKind::StaleEvidence
                        | sem_os_core::context_resolution::GovernanceSignalKind::CoverageGap
                )
            })
            .map(|s| s.message.clone())
            .collect();

        let gov_signals: Vec<GovernanceSignalSummary> = response
            .governance_signals
            .iter()
            .map(|s| GovernanceSignalSummary {
                kind: format!("{:?}", s.kind),
                message: s.message.clone(),
                severity: format!("{:?}", s.severity),
                related_fqn: s.related_fqn.clone(),
            })
            .collect();

        let deny_all = allowed.is_empty();

        ContextEnvelope {
            allowed_verbs: allowed,
            allowed_verb_contracts: allowed_contracts,
            pruned_verbs: pruned,
            fingerprint,
            evidence_gaps,
            governance_signals: gov_signals,
            snapshot_set_id: None,
            computed_at: Utc::now(),
            deny_all,
            unavailable: false,
        }
    }

    /// Create an "unavailable" envelope (SemReg not configured or resolution failed).
    pub fn unavailable() -> Self {
        ContextEnvelope {
            allowed_verbs: HashSet::new(),
            allowed_verb_contracts: vec![],
            pruned_verbs: vec![],
            fingerprint: AllowedVerbSetFingerprint::empty(),
            evidence_gaps: vec![],
            governance_signals: vec![],
            snapshot_set_id: None,
            computed_at: Utc::now(),
            deny_all: false,
            unavailable: true,
        }
    }

    /// Create a "deny all" envelope (resolution succeeded, zero verbs allowed).
    pub fn deny_all() -> Self {
        ContextEnvelope {
            allowed_verbs: HashSet::new(),
            allowed_verb_contracts: vec![],
            pruned_verbs: vec![],
            fingerprint: AllowedVerbSetFingerprint::empty(),
            evidence_gaps: vec![],
            governance_signals: vec![],
            snapshot_set_id: None,
            computed_at: Utc::now(),
            deny_all: true,
            unavailable: false,
        }
    }

    /// Check if a specific verb FQN is in the allowed set.
    pub fn is_allowed(&self, fqn: &str) -> bool {
        self.allowed_verbs.contains(fqn)
    }

    /// True if resolution succeeded but zero verbs are allowed.
    pub fn is_deny_all(&self) -> bool {
        self.deny_all
    }

    /// True if SemReg was unavailable (not configured or resolution failed).
    pub fn is_unavailable(&self) -> bool {
        self.unavailable
    }

    /// Backward-compatible label matching old `SemRegVerbPolicy::label()`.
    pub fn label(&self) -> &'static str {
        if self.unavailable {
            "unavailable"
        } else if self.deny_all {
            "deny_all"
        } else {
            "allowed_set"
        }
    }

    /// Number of pruned verbs.
    pub fn pruned_count(&self) -> usize {
        self.pruned_verbs.len()
    }

    /// Fingerprint string for trace/telemetry.
    pub fn fingerprint_str(&self) -> &str {
        &self.fingerprint.0
    }

    /// Create a test envelope with specific allowed verbs (deny_all = false, unavailable = false).
    #[cfg(test)]
    pub fn test_with_verbs(verbs: &[&str]) -> Self {
        let allowed: HashSet<String> = verbs.iter().map(|v| v.to_string()).collect();
        let fingerprint = AllowedVerbSetFingerprint::compute(&allowed);
        ContextEnvelope {
            allowed_verbs: allowed,
            allowed_verb_contracts: vec![],
            pruned_verbs: vec![],
            fingerprint,
            evidence_gaps: vec![],
            governance_signals: vec![],
            snapshot_set_id: None,
            computed_at: Utc::now(),
            deny_all: false,
            unavailable: false,
        }
    }

    /// Perform a TOCTOU recheck against a fresh envelope.
    ///
    /// Compares the original fingerprint with the new envelope's fingerprint
    /// and checks whether the selected verb is still allowed.
    ///
    /// Returns `None` if this envelope is unavailable (no TOCTOU possible).
    pub fn toctou_recheck(
        &self,
        new_envelope: &ContextEnvelope,
        selected_verb: &str,
    ) -> Option<TocTouResult> {
        // No recheck possible if either envelope is unavailable
        if self.is_unavailable() || new_envelope.is_unavailable() {
            return None;
        }

        // Fast path: fingerprints match → nothing changed
        if self.fingerprint == new_envelope.fingerprint {
            return Some(TocTouResult::StillAllowed);
        }

        // Fingerprints differ — check if the selected verb survived
        if new_envelope.is_allowed(selected_verb) {
            Some(TocTouResult::AllowedButDrifted {
                new_fingerprint: new_envelope.fingerprint.clone(),
            })
        } else {
            Some(TocTouResult::Denied {
                verb_fqn: selected_verb.to_string(),
                new_fingerprint: new_envelope.fingerprint.clone(),
            })
        }
    }
}

/// Derive a structured prune reason from a denied VerbCandidate.
fn prune_reason_from_candidate(vc: &sem_os_core::context_resolution::VerbCandidate) -> PruneReason {
    match &vc.access_decision {
        AccessDecision::Deny { reason } => PruneReason::AbacDenied {
            actor_role: String::new(),
            required: reason.clone(),
        },
        AccessDecision::AllowWithMasking { masked_fields } => PruneReason::PolicyDenied {
            policy_fqn: String::new(),
            reason: format!("Masked fields: {:?}", masked_fields),
        },
        AccessDecision::Allow => {
            // Shouldn't reach here, but handle gracefully
            PruneReason::AbacDenied {
                actor_role: String::new(),
                required: "unexpected allow in pruned set".into(),
            }
        }
    }
}

// ── Tests ──────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fingerprint_deterministic() {
        let set1: HashSet<String> = ["a.b", "c.d", "e.f"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let set2: HashSet<String> = ["e.f", "a.b", "c.d"]
            .iter()
            .map(|s| s.to_string())
            .collect();

        let fp1 = AllowedVerbSetFingerprint::compute(&set1);
        let fp2 = AllowedVerbSetFingerprint::compute(&set2);
        assert_eq!(
            fp1.0, fp2.0,
            "Same verbs in different order must produce same fingerprint"
        );
    }

    #[test]
    fn test_fingerprint_differs_for_different_sets() {
        let set1: HashSet<String> = ["a.b"].iter().map(|s| s.to_string()).collect();
        let set2: HashSet<String> = ["c.d"].iter().map(|s| s.to_string()).collect();

        let fp1 = AllowedVerbSetFingerprint::compute(&set1);
        let fp2 = AllowedVerbSetFingerprint::compute(&set2);
        assert_ne!(fp1.0, fp2.0);
    }

    #[test]
    fn test_fingerprint_format() {
        let set: HashSet<String> = ["kyc.open-case"].iter().map(|s| s.to_string()).collect();
        let fp = AllowedVerbSetFingerprint::compute(&set);
        assert!(fp.0.starts_with("v1:"), "Fingerprint must be versioned");
        // v1: + 64 hex chars
        assert_eq!(fp.0.len(), 3 + 64);
    }

    #[test]
    fn test_empty_fingerprint() {
        let fp = AllowedVerbSetFingerprint::empty();
        assert!(fp.0.starts_with("v1:"));
    }

    #[test]
    fn test_unavailable_envelope() {
        let env = ContextEnvelope::unavailable();
        assert!(env.is_unavailable());
        assert!(!env.is_deny_all());
        assert_eq!(env.label(), "unavailable");
        assert!(env.allowed_verbs.is_empty());
    }

    #[test]
    fn test_deny_all_envelope() {
        let env = ContextEnvelope::deny_all();
        assert!(env.is_deny_all());
        assert!(!env.is_unavailable());
        assert_eq!(env.label(), "deny_all");
        assert!(env.allowed_verbs.is_empty());
    }

    #[test]
    fn test_is_allowed() {
        let mut env = ContextEnvelope::unavailable();
        env.unavailable = false;
        env.allowed_verbs.insert("kyc.open-case".into());
        assert!(env.is_allowed("kyc.open-case"));
        assert!(!env.is_allowed("cbu.create"));
    }

    #[test]
    fn test_label_backward_compat() {
        let unav = ContextEnvelope::unavailable();
        assert_eq!(unav.label(), "unavailable");

        let deny = ContextEnvelope::deny_all();
        assert_eq!(deny.label(), "deny_all");

        let mut allowed = ContextEnvelope::unavailable();
        allowed.unavailable = false;
        allowed.allowed_verbs.insert("a.b".into());
        assert_eq!(allowed.label(), "allowed_set");
    }

    #[test]
    fn test_pruned_count() {
        let mut env = ContextEnvelope::deny_all();
        env.pruned_verbs.push(PrunedVerb {
            fqn: "a.b".into(),
            reason: PruneReason::AbacDenied {
                actor_role: "viewer".into(),
                required: "admin".into(),
            },
        });
        assert_eq!(env.pruned_count(), 1);
    }

    #[test]
    fn test_prune_reason_serialization() {
        let reason = PruneReason::EntityKindMismatch {
            verb_kinds: vec!["fund".into()],
            subject_kind: "cbu".into(),
        };
        let json = serde_json::to_string(&reason).unwrap();
        assert!(json.contains("entity_kind_mismatch"));
        assert!(json.contains("fund"));
        assert!(json.contains("cbu"));
    }

    #[test]
    fn test_envelope_serialization() {
        let env = ContextEnvelope::unavailable();
        let json = serde_json::to_string(&env).unwrap();
        assert!(json.contains("allowed_verbs"));
        assert!(json.contains("fingerprint"));
        assert!(json.contains("computed_at"));
    }

    // ── TOCTOU tests ───────────────────────────────────────────

    #[test]
    fn test_fingerprint_equality() {
        let set: HashSet<String> = ["a.b", "c.d"].iter().map(|s| s.to_string()).collect();
        let fp1 = AllowedVerbSetFingerprint::compute(&set);
        let fp2 = AllowedVerbSetFingerprint::compute(&set);
        assert_eq!(fp1, fp2);

        let other: HashSet<String> = ["x.y"].iter().map(|s| s.to_string()).collect();
        let fp3 = AllowedVerbSetFingerprint::compute(&other);
        assert_ne!(fp1, fp3);
    }

    fn make_envelope_with_verbs(verbs: &[&str]) -> ContextEnvelope {
        let mut env = ContextEnvelope::unavailable();
        env.unavailable = false;
        for v in verbs {
            env.allowed_verbs.insert(v.to_string());
        }
        env.fingerprint = AllowedVerbSetFingerprint::compute(&env.allowed_verbs);
        env
    }

    #[test]
    fn test_toctou_still_allowed() {
        let original = make_envelope_with_verbs(&["cbu.create", "kyc.open-case"]);
        let fresh = make_envelope_with_verbs(&["cbu.create", "kyc.open-case"]);
        let result = original.toctou_recheck(&fresh, "cbu.create");
        assert!(matches!(result, Some(TocTouResult::StillAllowed)));
    }

    #[test]
    fn test_toctou_drifted_but_still_allowed() {
        let original = make_envelope_with_verbs(&["cbu.create", "kyc.open-case"]);
        // Fresh has an extra verb (fingerprint different) but selected verb still present
        let fresh = make_envelope_with_verbs(&["cbu.create", "kyc.open-case", "entity.create"]);
        let result = original.toctou_recheck(&fresh, "cbu.create");
        assert!(matches!(
            result,
            Some(TocTouResult::AllowedButDrifted { .. })
        ));
    }

    #[test]
    fn test_toctou_denied() {
        let original = make_envelope_with_verbs(&["cbu.create", "kyc.open-case"]);
        // Fresh no longer has cbu.create
        let fresh = make_envelope_with_verbs(&["kyc.open-case"]);
        let result = original.toctou_recheck(&fresh, "cbu.create");
        match result {
            Some(TocTouResult::Denied {
                verb_fqn,
                new_fingerprint,
            }) => {
                assert_eq!(verb_fqn, "cbu.create");
                assert!(new_fingerprint.0.starts_with("v1:"));
            }
            other => panic!("Expected Denied, got {:?}", other),
        }
    }

    #[test]
    fn test_toctou_unavailable_returns_none() {
        let unavailable = ContextEnvelope::unavailable();
        let fresh = make_envelope_with_verbs(&["cbu.create"]);
        assert!(unavailable.toctou_recheck(&fresh, "cbu.create").is_none());

        let original = make_envelope_with_verbs(&["cbu.create"]);
        assert!(original
            .toctou_recheck(&unavailable, "cbu.create")
            .is_none());
    }

    #[test]
    fn test_toctou_result_serialization() {
        let result = TocTouResult::AllowedButDrifted {
            new_fingerprint: AllowedVerbSetFingerprint("v1:abc123".into()),
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("allowed_but_drifted"));
        assert!(json.contains("v1:abc123"));

        let denied = TocTouResult::Denied {
            verb_fqn: "cbu.create".into(),
            new_fingerprint: AllowedVerbSetFingerprint("v1:def456".into()),
        };
        let json = serde_json::to_string(&denied).unwrap();
        assert!(json.contains("denied"));
        assert!(json.contains("cbu.create"));
    }
}
