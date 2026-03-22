//! Composite session state for state-to-intent bias.
//!
//! Aggregates group-level entity states (CBUs, cases, screenings, documents)
//! into a snapshot that drives verb scoring and next-step prediction.
//!
//! The core insight: when we know the "as-is" state of a group's composites,
//! we can predict the "to-be" state and bias verb resolution accordingly.
//! E.g., CBU exists but no KYC case → "KYC" likely means `kyc-case.create`.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Snapshot of a group's composite state for intent prediction.
///
/// Built from the session's loaded CBUs and their downstream entities.
/// Used by the orchestrator to inject state-aware scoring into verb search.
///
/// UBO/ownership/control is a GROUP-level concern — the corporate hierarchy
/// is determined once for the group and inherited by all CBUs.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GroupCompositeState {
    /// Number of CBUs in scope for this group.
    pub cbu_count: usize,

    /// Per-domain existence flags derived from group entities.
    /// Key = domain concept (e.g., "kyc_case", "screening").
    /// Value = how many entities of that type exist in the group.
    pub domain_counts: HashMap<String, usize>,

    // ── Group-level state (shared across all CBUs) ────────────
    /// Whether UBO/ownership determination has been run for this group.
    /// This is a group-level concern — CBUs inherit from the group hierarchy.
    pub has_ubo_determination: bool,

    /// Whether the control chain has been mapped for this group.
    pub has_control_chain: bool,

    /// CBU-level onboarding state summaries.
    pub cbu_states: Vec<CbuStateSummary>,

    /// Verbs that would advance the group's workflow.
    /// Derived from "as-is → to-be" gap analysis.
    pub next_likely_verbs: Vec<ScoredVerbHint>,

    /// Verbs that are blocked given current entity states.
    pub blocked_verbs: Vec<BlockedVerbHint>,
}

/// Summary of a single CBU's onboarding state within the group composite.
///
/// Tracks the onboarding lifecycle only (group → CBU → case → screening → docs → tollgate).
/// Street-side state (trading profiles, custody, SSI) is a separate downstream concern
/// that only matters after KYC approval.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CbuStateSummary {
    pub cbu_id: String,
    pub cbu_name: Option<String>,
    /// CBU lifecycle: DISCOVERED → VALIDATED → ACTIVE
    pub lifecycle_state: Option<String>,
    // ── Onboarding lifecycle ──────────────────────────
    pub has_kyc_case: bool,
    pub kyc_case_status: Option<String>,
    pub has_screening: bool,
    pub screening_complete: bool,
    pub document_coverage_pct: Option<f64>,
}

/// A verb likely to be the user's next intent, with a reason.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoredVerbHint {
    pub verb_fqn: String,
    /// Score boost to apply when this verb appears in candidates (0.0 to 0.20).
    pub boost: f32,
    /// Human-readable reason for the boost.
    pub reason: String,
}

/// A verb that is blocked by current entity state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockedVerbHint {
    pub verb_fqn: String,
    /// Score penalty to apply (0.0 to -0.20).
    pub penalty: f32,
    pub reason: String,
}

impl GroupCompositeState {
    /// Compute the score adjustment for a given verb based on composite state.
    ///
    /// Returns a value in [-0.15, +0.15] that should be added to the verb's
    /// raw search score before ambiguity resolution.
    pub fn compute_state_boost(&self, verb_fqn: &str) -> f32 {
        let mut boost = 0.0f32;

        // Check next-likely verbs (positive boost)
        for hint in &self.next_likely_verbs {
            if hint.verb_fqn == verb_fqn {
                boost += hint.boost;
            }
        }

        // Check blocked verbs (negative penalty)
        for blocked in &self.blocked_verbs {
            if blocked.verb_fqn == verb_fqn {
                boost += blocked.penalty; // penalty is already negative
            }
        }

        boost.clamp(-0.15, 0.15)
    }

    /// Derive next-likely verbs from the group's "as-is" state.
    ///
    /// Timeline: Group → UBO/Control → CBUs → per-CBU onboarding → street-side
    ///
    /// The onboarding DAG (high level):
    ///   1. Group exists
    ///   2. UBO / ownership / control mapped (GROUP level)
    ///   3. CBUs identified (the revenue-generating units)
    ///   4. Per-CBU: Case → Screening → Documents → Tollgate → APPROVED
    ///   5. Post-approval: custody, settlement, go-live (street-side)
    pub fn derive_next_likely_verbs(&mut self) {
        self.next_likely_verbs.clear();
        self.blocked_verbs.clear();

        // ── Layer 1: Group-level UBO/control (comes FIRST) ───────
        // You can't KYC a CBU without knowing who controls it.
        // UBO tells you which persons need screening, what the risk is.
        if !self.has_ubo_determination {
            self.next_likely_verbs.push(ScoredVerbHint {
                verb_fqn: "ubo.discover".into(),
                boost: 0.12,
                reason: "Group ownership not yet determined — UBO discovery first".into(),
            });
            self.next_likely_verbs.push(ScoredVerbHint {
                verb_fqn: "ownership.trace-chain".into(),
                boost: 0.10,
                reason: "Group control chain not mapped — trace ownership".into(),
            });
            self.next_likely_verbs.push(ScoredVerbHint {
                verb_fqn: "gleif.import-tree".into(),
                boost: 0.08,
                reason: "May need GLEIF data to determine ownership".into(),
            });
        }
        if !self.has_control_chain && self.has_ubo_determination {
            self.next_likely_verbs.push(ScoredVerbHint {
                verb_fqn: "control.build-graph".into(),
                boost: 0.10,
                reason: "UBO determined but control graph not built".into(),
            });
        }

        // ── Layer 2: CBUs exist? ─────────────────────────────────
        if self.cbu_count == 0 {
            self.next_likely_verbs.push(ScoredVerbHint {
                verb_fqn: "cbu.create".into(),
                boost: 0.12,
                reason: "No CBUs in group — identify revenue units".into(),
            });
            // Block per-CBU verbs
            for d in &["kyc-case", "screening", "document", "custody"] {
                self.blocked_verbs.push(BlockedVerbHint {
                    verb_fqn: format!("{d}.*"),
                    penalty: -0.10,
                    reason: "No CBU exists yet".into(),
                });
            }
            return;
        }

        // ── Layer 3+: Per-CBU onboarding lifecycle ───────────────
        let mut cbus_without_case = 0;
        let mut cbus_without_screening = 0;
        let mut cbus_with_incomplete_docs = 0;
        let mut cbus_kyc_approved = 0;

        for cbu in &self.cbu_states {
            if !cbu.has_kyc_case {
                cbus_without_case += 1;
                continue;
            }
            let status = cbu.kyc_case_status.as_deref().unwrap_or("INTAKE");
            if status == "APPROVED" {
                cbus_kyc_approved += 1;
                continue;
            }
            if !cbu.has_screening {
                cbus_without_screening += 1;
            }
            if cbu.document_coverage_pct.unwrap_or(0.0) < 1.0 {
                cbus_with_incomplete_docs += 1;
            }
        }

        // Layer 3: No case → open case
        if cbus_without_case > 0 {
            self.next_likely_verbs.push(ScoredVerbHint {
                verb_fqn: "kyc-case.create".into(),
                boost: 0.12,
                reason: format!("{cbus_without_case} CBU(s) need KYC case"),
            });
            self.next_likely_verbs.push(ScoredVerbHint {
                verb_fqn: "kyc.open-case".into(),
                boost: 0.12,
                reason: format!("{cbus_without_case} CBU(s) need KYC case"),
            });
        }

        // Layer 4: Case open, no screening → screen
        if cbus_without_screening > 0 {
            self.next_likely_verbs.push(ScoredVerbHint {
                verb_fqn: "screening.run".into(),
                boost: 0.10,
                reason: format!("{cbus_without_screening} CBU(s) need screening"),
            });
            self.next_likely_verbs.push(ScoredVerbHint {
                verb_fqn: "screening.sanctions".into(),
                boost: 0.08,
                reason: "Sanctions check needed".into(),
            });
            self.next_likely_verbs.push(ScoredVerbHint {
                verb_fqn: "screening.pep".into(),
                boost: 0.08,
                reason: "PEP check needed".into(),
            });
        }

        // Layer 5: Screened, incomplete docs → solicit
        if cbus_with_incomplete_docs > 0 && cbus_without_screening == 0 {
            self.next_likely_verbs.push(ScoredVerbHint {
                verb_fqn: "document.solicit".into(),
                boost: 0.10,
                reason: "Documents incomplete — solicitation next".into(),
            });
            self.next_likely_verbs.push(ScoredVerbHint {
                verb_fqn: "document.solicit-set".into(),
                boost: 0.10,
                reason: "Documents incomplete — bulk request next".into(),
            });
        }

        // Layer 6: All done → status queries
        if cbus_kyc_approved > 0 && cbus_without_case == 0 && cbus_without_screening == 0 {
            self.next_likely_verbs.push(ScoredVerbHint {
                verb_fqn: "kyc-case.read".into(),
                boost: 0.06,
                reason: "Onboarding complete — status check likely".into(),
            });
            self.next_likely_verbs.push(ScoredVerbHint {
                verb_fqn: "deal.read-record".into(),
                boost: 0.06,
                reason: "Onboarding complete — deal review likely".into(),
            });
        }
    }

    /// Returns true if domain work is impossible given current state.
    ///
    /// Used for wildcard blocking: `domain.*` patterns.
    pub fn is_domain_blocked(&self, verb_fqn: &str) -> bool {
        self.blocked_verbs.iter().any(|b| {
            if b.verb_fqn.ends_with(".*") {
                let prefix = &b.verb_fqn[..b.verb_fqn.len() - 2];
                verb_fqn.starts_with(prefix)
            } else {
                b.verb_fqn == verb_fqn
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_group_boosts_cbu_create() {
        let mut state = GroupCompositeState::default();
        state.derive_next_likely_verbs();

        let boost = state.compute_state_boost("cbu.create");
        assert!(
            boost > 0.0,
            "cbu.create should be boosted when no CBUs exist"
        );

        // Domain verbs should be blocked
        assert!(state.is_domain_blocked("kyc-case.create"));
        assert!(state.is_domain_blocked("screening.run"));
    }

    #[test]
    fn test_cbu_without_case_boosts_case_create() {
        let mut state = GroupCompositeState {
            cbu_count: 1,
            cbu_states: vec![CbuStateSummary {
                cbu_id: "test".into(),
                cbu_name: Some("Test CBU".into()),
                lifecycle_state: Some("active".into()),
                has_kyc_case: false,
                kyc_case_status: None,
                has_screening: false,
                screening_complete: false,
                document_coverage_pct: None,
            }],
            ..Default::default()
        };
        state.derive_next_likely_verbs();

        let boost = state.compute_state_boost("kyc-case.create");
        assert!(
            boost > 0.0,
            "kyc-case.create should be boosted when CBU has no case"
        );

        // Screening is NOT boosted when there's no case — you need a case first.
        // Screening boost only kicks in at Layer 2 (case exists, no screening).
        let screening_boost = state.compute_state_boost("screening.run");
        assert!(
            screening_boost == 0.0,
            "screening.run should NOT be boosted when CBU has no case (case first)"
        );
    }

    #[test]
    fn test_complete_state_boosts_status_queries() {
        let mut state = GroupCompositeState {
            cbu_count: 1,
            cbu_states: vec![CbuStateSummary {
                cbu_id: "test".into(),
                cbu_name: Some("Test CBU".into()),
                lifecycle_state: Some("active".into()),
                has_kyc_case: true,
                kyc_case_status: Some("APPROVED".into()),
                has_screening: true,
                screening_complete: true,
                document_coverage_pct: Some(1.0),
            }],
            ..Default::default()
        };
        state.derive_next_likely_verbs();

        let boost = state.compute_state_boost("kyc-case.read");
        assert!(
            boost > 0.0,
            "kyc-case.read should be boosted when all workflows complete"
        );
    }

    #[test]
    fn test_boost_clamped() {
        let state = GroupCompositeState {
            next_likely_verbs: vec![
                ScoredVerbHint {
                    verb_fqn: "cbu.create".into(),
                    boost: 0.20,
                    reason: "test".into(),
                },
                ScoredVerbHint {
                    verb_fqn: "cbu.create".into(),
                    boost: 0.20,
                    reason: "test2".into(),
                },
            ],
            ..Default::default()
        };

        let boost = state.compute_state_boost("cbu.create");
        assert!(
            boost <= 0.15,
            "Boost should be clamped to 0.15, got {boost}"
        );
    }
}
