//! G3 — Semantic Pack Resolution (V&S §6.3).
//!
//! T2.3 wires the adapter over the constraint gate + SemReg fail-closed
//! (ledger C-005, C-009, C-015, C-016). Both existing validators are
//! boolean/violation-reporting, not enum-typed by outcome kind, so this
//! adapter maps their combined result onto §6.3's named variants using the
//! same fail-closed default `MissingPack` uses today (C-016: unavailable
//! allowed-set fails closed) — an ambiguous state never falls through to
//! `Resolved`.

use crate::gate::{Gate, GateId, GateResult};

/// `PackResolution` — V&S §6.3 "Output". Variant names mirror the possible
/// outcomes listed there. No active pack means no execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackResolutionOutcome {
    Resolved(ResolvedPack),
    AmbiguousPack,
    MissingPack,
    PackDeniesIntent,
    PackDeniesEntity,
}

/// Success-form proof: exactly one SemOS pack governs this execution.
/// Constructible only from within this module.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ResolvedPack {
    pack_id: String,
}

impl ResolvedPack {
    // Called by the (future) T2.3 adapter; the only caller today is the
    // cfg(test) bridge below.
    #[allow(dead_code)]
    fn new(pack_id: impl Into<String>) -> Self {
        Self {
            pack_id: pack_id.into(),
        }
    }

    pub fn pack_id(&self) -> &str {
        &self.pack_id
    }
}

#[cfg(test)]
pub(crate) mod tests_support {
    use super::ResolvedPack;

    pub(crate) fn resolved(pack_id: &str) -> ResolvedPack {
        ResolvedPack::new(pack_id)
    }
}

/// Pre-computed input for the pack resolution gate. `candidate_pack_ids` is
/// the set of packs whose `allowed_verbs`/workspace scope admit the verb
/// under evaluation (constraint gate, C-015); `semreg_allowed_set_available`
/// mirrors the compiler's fail-closed check when the SemReg allowed-set
/// resolves to `None` (C-016).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PackResolutionInput {
    pub candidate_pack_ids: Vec<String>,
    pub semreg_allowed_set_available: bool,
    /// `true` when the constraint gate reported a hard, empty-intersection
    /// deadlock for the verb under evaluation (`is_empty_intersection`).
    pub constraint_denies_intent: bool,
}

fn decide(input: &PackResolutionInput) -> PackResolutionOutcome {
    if !input.semreg_allowed_set_available {
        // C-016: unavailable allowed-set fails closed — never resolves.
        return PackResolutionOutcome::MissingPack;
    }
    if input.constraint_denies_intent {
        return PackResolutionOutcome::PackDeniesIntent;
    }
    match input.candidate_pack_ids.as_slice() {
        [] => PackResolutionOutcome::MissingPack,
        [single] => PackResolutionOutcome::Resolved(ResolvedPack::new(single.clone())),
        _ => PackResolutionOutcome::AmbiguousPack,
    }
}

/// T2.3 adapter: `Gate<crate::context::EvaluationContext>` impl for G3.
pub struct PackResolutionGate;

impl Gate<crate::context::EvaluationContext> for PackResolutionGate {
    fn id(&self) -> GateId {
        GateId::PackResolution
    }

    fn evaluate(&self, ctx: &crate::context::EvaluationContext) -> GateResult {
        let Some(input) = &ctx.pack_resolution else {
            return GateResult::Failure("no PackResolutionInput supplied".to_string());
        };
        match decide(input) {
            PackResolutionOutcome::Resolved(_) => GateResult::Success,
            other => GateResult::Failure(format!("{other:?}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolved_pack_is_constructible_within_its_own_module() {
        let pack = ResolvedPack::new("ob-poc.cbu");
        assert_eq!(pack.pack_id(), "ob-poc.cbu");
    }

    fn base_input() -> PackResolutionInput {
        PackResolutionInput {
            candidate_pack_ids: vec!["ob-poc.cbu".to_string()],
            semreg_allowed_set_available: true,
            constraint_denies_intent: false,
        }
    }

    #[test]
    fn single_candidate_resolves() {
        assert_eq!(
            decide(&base_input()),
            PackResolutionOutcome::Resolved(ResolvedPack::new("ob-poc.cbu"))
        );
    }

    #[test]
    fn unavailable_semreg_set_fails_closed_to_missing_pack() {
        let input = PackResolutionInput {
            semreg_allowed_set_available: false,
            ..base_input()
        };
        assert_eq!(decide(&input), PackResolutionOutcome::MissingPack);
    }

    #[test]
    fn empty_intersection_deadlock_denies_intent() {
        let input = PackResolutionInput {
            constraint_denies_intent: true,
            ..base_input()
        };
        assert_eq!(decide(&input), PackResolutionOutcome::PackDeniesIntent);
    }

    #[test]
    fn multiple_candidates_are_ambiguous() {
        let input = PackResolutionInput {
            candidate_pack_ids: vec!["ob-poc.cbu".to_string(), "ob-poc.kyc".to_string()],
            ..base_input()
        };
        assert_eq!(decide(&input), PackResolutionOutcome::AmbiguousPack);
    }

    #[test]
    fn no_candidates_is_missing_pack() {
        let input = PackResolutionInput {
            candidate_pack_ids: vec![],
            ..base_input()
        };
        assert_eq!(decide(&input), PackResolutionOutcome::MissingPack);
    }

    #[test]
    fn gate_evaluate_fails_closed_when_input_missing() {
        let ctx = crate::context::EvaluationContext::default();
        assert!(matches!(PackResolutionGate.evaluate(&ctx), GateResult::Failure(_)));
        assert_eq!(PackResolutionGate.id(), GateId::PackResolution);
    }
}
