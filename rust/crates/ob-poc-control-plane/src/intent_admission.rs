//! G1 — Intent Admission (V&S §6.1).
//!
//! T1 defines the outcome/proof shape only; T2.1 wires the adapter over
//! `SessionVerbSurface` + `SemOsContextEnvelope` (ledger C-007, C-009).

use uuid::Uuid;

/// `IntentAdmissionDecision` — V&S §6.1 "Output". Variant names mirror the
/// possible outcomes listed there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntentAdmissionDecision {
    Admitted(AdmittedIntent),
    RejectedUnknownIntent,
    RejectedOutsidePack,
    RejectedDeprecated,
    RejectedUnauthorisedSurface,
    /// A candidate intent lacking a valid interpretation attestation
    /// (§6.13.1) for an AI-originated request.
    RejectedAttestationInsufficient,
}

/// Success-form proof: the intent is recognised, in-pack, current, and
/// (for AI-originated intents) carries a valid attestation.
///
/// Constructible only from within this module — the only place code can
/// obtain an `AdmittedIntent` is by matching
/// `IntentAdmissionDecision::Admitted(_)`, which in turn is only produced
/// by this gate's own (future, T2.1) evaluation logic.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct AdmittedIntent {
    intent_id: Uuid,
    verb_fqn: String,
    /// `true` for AI-originated intents whose interpretation attestation
    /// (§6.13.1) was present and valid; always `true` for operator-typed
    /// intents (no attestation requirement).
    attested: bool,
}

impl AdmittedIntent {
    // Called by the (future) T2.1 adapter; the only caller today is the
    // cfg(test) bridge below.
    #[allow(dead_code)]
    fn new(intent_id: Uuid, verb_fqn: impl Into<String>, attested: bool) -> Self {
        Self {
            intent_id,
            verb_fqn: verb_fqn.into(),
            attested,
        }
    }

    pub fn intent_id(&self) -> Uuid {
        self.intent_id
    }

    pub fn verb_fqn(&self) -> &str {
        &self.verb_fqn
    }

    pub fn attested(&self) -> bool {
        self.attested
    }
}

/// Test-only fixture bridge. `cfg(test)`-gated (compiled out entirely in
/// non-test builds), so it does not loosen the production visibility of
/// `AdmittedIntent::new` — it exists so crate-internal integration-style
/// tests elsewhere (e.g. `envelope::tests`) can obtain a fixture without
/// duplicating this module's construction logic.
#[cfg(test)]
pub(crate) mod tests_support {
    use super::AdmittedIntent;
    use uuid::Uuid;

    pub(crate) fn admitted(id: Uuid, verb_fqn: &str) -> AdmittedIntent {
        AdmittedIntent::new(id, verb_fqn, true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admitted_intent_is_constructible_within_its_own_module() {
        let id = Uuid::nil();
        let admitted = AdmittedIntent::new(id, "cbu.confirm", true);
        assert_eq!(admitted.intent_id(), id);
        assert_eq!(admitted.verb_fqn(), "cbu.confirm");
        assert!(admitted.attested());
    }
}
