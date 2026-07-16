//! G12 — Version Pinning (V&S §6.12).
//!
//! T4.4 unifies subsystem pins (SemReg snapshot ids, KYC manifest hash, bus
//! catalogue version ledger C-033, DSL/compiler crate versions, model/prompt
//! version from attestation) into the envelope's version block —
//! `snapshot::PinnedVersionSet` (already defined there, since it's also
//! embedded in G13's `SnapshotPins`) is that unified block; this module
//! only owns G12's own shadow gate, not a second copy of the type.

use crate::gate::{Gate, GateId, GateResult};
use crate::snapshot::PinnedVersionSet;

/// T9.7: pre-computed input for the G12 shadow gate. Same "pins whatever
/// was read, doesn't judge it" law as G13's `DecisionSnapshotGate` — an
/// all-`None` `PinnedVersionSet` still succeeds, because most of its
/// fields have no production source yet (see `PinnedVersionSet`'s own
/// field docs); `None` here is reserved for "this call site didn't even
/// attempt to collect version pins," not "some pins were unavailable."
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct VersionPinningInput {
    pub versions: PinnedVersionSet,
}

/// T9.7 adapter: `Gate<crate::context::EvaluationContext>` impl for G12.
pub struct VersionPinningGate;

impl Gate<crate::context::EvaluationContext> for VersionPinningGate {
    fn id(&self) -> GateId {
        GateId::VersionPinning
    }

    fn evaluate(&self, ctx: &crate::context::EvaluationContext) -> GateResult {
        match &ctx.version_pinning {
            Some(_) => GateResult::Success,
            None => GateResult::Failure("no VersionPinningInput supplied".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_pinning_gate_fails_closed_when_input_missing() {
        let ctx = crate::context::EvaluationContext::default();
        assert!(matches!(
            VersionPinningGate.evaluate(&ctx),
            GateResult::Failure(_)
        ));
        assert_eq!(VersionPinningGate.id(), GateId::VersionPinning);
    }

    #[test]
    fn version_pinning_gate_succeeds_even_with_all_fields_unset() {
        // Mirrors DecisionSnapshotGate's own "empty pins still succeed"
        // law — most PinnedVersionSet fields have no production source
        // today, and that must not read as a failure.
        let ctx = crate::context::EvaluationContext {
            version_pinning: Some(VersionPinningInput::default()),
            ..Default::default()
        };
        assert_eq!(VersionPinningGate.evaluate(&ctx), GateResult::Success);
    }

    #[test]
    fn version_pinning_gate_succeeds_with_a_real_compiler_version() {
        let ctx = crate::context::EvaluationContext {
            version_pinning: Some(VersionPinningInput {
                versions: PinnedVersionSet {
                    compiler_version: Some("1.2.3".to_string()),
                    ..Default::default()
                },
            }),
            ..Default::default()
        };
        assert_eq!(VersionPinningGate.evaluate(&ctx), GateResult::Success);
    }
}
