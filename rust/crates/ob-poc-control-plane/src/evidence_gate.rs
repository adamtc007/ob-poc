//! G6 — Evidence and Obligation Check (V&S §6.6).
//!
//! T2.5 wires the adapter over SemOS governance evidence gaps + KYC
//! preconditions via an adapter trait so `ob-poc-kyc-substrate` stays the
//! owner (ledger C-040, C-041, C-042).

/// `EvidenceReadiness` — V&S §6.6 "Output". Variant names mirror the
/// possible outcomes listed there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvidenceOutcome {
    Sufficient(EvidenceSufficient),
    MissingRequiredEvidence,
    ExpiredEvidence,
    ConflictingEvidence,
    PendingApproval,
    ObligationOpen,
}

/// Success-form proof: required evidence and obligations are satisfied.
/// Constructible only from within this module.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct EvidenceSufficient {
    satisfied_obligation_ids: Vec<String>,
}

impl EvidenceSufficient {
    // Called by the (future) T2.5 adapter; the only caller today is the
    // cfg(test) bridge below.
    #[allow(dead_code)]
    fn new(satisfied_obligation_ids: Vec<String>) -> Self {
        Self {
            satisfied_obligation_ids,
        }
    }

    pub fn satisfied_obligation_ids(&self) -> &[String] {
        &self.satisfied_obligation_ids
    }
}

#[cfg(test)]
pub(crate) mod tests_support {
    use super::EvidenceSufficient;

    pub(crate) fn sufficient(satisfied_obligation_ids: Vec<String>) -> EvidenceSufficient {
        EvidenceSufficient::new(satisfied_obligation_ids)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evidence_sufficient_is_constructible_within_its_own_module() {
        let evidence = EvidenceSufficient::new(vec!["obligation-1".to_string()]);
        assert_eq!(evidence.satisfied_obligation_ids(), ["obligation-1"]);
    }
}
