//! G7 — Bounded Write-Set Derivation (V&S §6.7, pre-execution).
//!
//! T2.6 wires the adapter over `derive_write_set` with contract-union
//! promoted to default (plan A4, ledger C-019). This adapter's
//! `WriteSetInput.contract_derived` flag records whether the call site
//! actually ran the contract-union path (`derive_write_set_from_contract`,
//! `#[cfg(feature = "write-set-contract")]`) rather than the always-on
//! heuristic UUID scan (`derive_write_set_heuristic`) — the crate-level
//! deletion of the heuristic-only default and the `write-set-contract`
//! feature flag itself is a call-site change in `ob-poc`, tracked as the
//! remaining half of C-019 (not yet flipped as of this adapter landing; see
//! the ownership ledger).
//!
//! Post-execution write-set *attestation* (V&S §6.7.1, gate G14) is a
//! distinct control owned by `write_set_attestation` (T5) — this module
//! only derives the bound, it does not verify the runtime observed a
//! subset of it.

use uuid::Uuid;

use crate::gate::{Gate, GateId, GateResult};

/// `WriteSetProof` — V&S §6.7 "Output". Unlike the other gates, §6.7 does
/// not enumerate named failure variants for derivation itself (a command
/// either has a bounded write-set or derivation cannot proceed), so this
/// gate's outcome is `Option`-shaped rather than a multi-arm enum; see
/// `write_set::Outcome` for the wrapping used by the (future) gate adapter.
///
/// Constructible only from within this module. Fields per §6.7's proof
/// content list.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct WriteSetProof {
    entity_ids: Vec<Uuid>,
    state_slots: Vec<String>,
    tables: Vec<String>,
    allowed_columns: Vec<String>,
    idempotency_key: String,
}

impl WriteSetProof {
    // Called by the (future) T2.6 adapter; the only caller today is the
    // cfg(test) bridge below.
    #[allow(dead_code)]
    fn new(
        entity_ids: Vec<Uuid>,
        state_slots: Vec<String>,
        tables: Vec<String>,
        allowed_columns: Vec<String>,
        idempotency_key: impl Into<String>,
    ) -> Self {
        Self {
            entity_ids,
            state_slots,
            tables,
            allowed_columns,
            idempotency_key: idempotency_key.into(),
        }
    }

    pub fn entity_ids(&self) -> &[Uuid] {
        &self.entity_ids
    }

    pub fn state_slots(&self) -> &[String] {
        &self.state_slots
    }

    pub fn tables(&self) -> &[String] {
        &self.tables
    }

    pub fn allowed_columns(&self) -> &[String] {
        &self.allowed_columns
    }

    pub fn idempotency_key(&self) -> &str {
        &self.idempotency_key
    }
}

/// Wraps derivation outcome for use by the (future) `Gate` adapter: either
/// a bounded proof, or a reason derivation could not proceed (e.g. the
/// compiled runbook this gate depends on is itself absent/invalid).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WriteSetOutcome {
    Bounded(WriteSetProof),
    CannotDerive { reason: String },
}

#[cfg(test)]
pub(crate) mod tests_support {
    use super::WriteSetProof;
    use uuid::Uuid;

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn proof(
        entity_ids: Vec<Uuid>,
        state_slots: Vec<String>,
        tables: Vec<String>,
        allowed_columns: Vec<String>,
        idempotency_key: &str,
    ) -> WriteSetProof {
        WriteSetProof::new(entity_ids, state_slots, tables, allowed_columns, idempotency_key)
    }
}

/// Pre-computed input for the write-set gate. `entity_ids` is
/// `derive_write_set`'s output (heuristic UUID scan and/or contract-union,
/// per `contract_derived`); `tables`/`allowed_columns`/`state_slots` are
/// only populated when `contract_derived` is `true` — the heuristic-only
/// path (`derive_write_set_heuristic`) has no table/column knowledge, so a
/// derivation resting solely on it cannot bound a bona fide `WriteSetProof`
/// and must report `CannotDerive` (this adapter does not fabricate table
/// names the heuristic scan never produced).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct WriteSetInput {
    pub entity_ids: Vec<Uuid>,
    pub state_slots: Vec<String>,
    pub tables: Vec<String>,
    pub allowed_columns: Vec<String>,
    pub idempotency_key: String,
    pub contract_derived: bool,
}

fn decide(input: &WriteSetInput) -> WriteSetOutcome {
    if !input.contract_derived || input.tables.is_empty() {
        return WriteSetOutcome::CannotDerive {
            reason: "heuristic-only derivation carries no table/column bound".to_string(),
        };
    }
    WriteSetOutcome::Bounded(WriteSetProof::new(
        input.entity_ids.clone(),
        input.state_slots.clone(),
        input.tables.clone(),
        input.allowed_columns.clone(),
        input.idempotency_key.clone(),
    ))
}

/// T2.6 adapter: `Gate<crate::context::EvaluationContext>` impl for G7.
pub struct WriteSetGate;

impl Gate<crate::context::EvaluationContext> for WriteSetGate {
    fn id(&self) -> GateId {
        GateId::WriteSet
    }

    fn evaluate(&self, ctx: &crate::context::EvaluationContext) -> GateResult {
        let Some(input) = &ctx.write_set else {
            return GateResult::Failure("no WriteSetInput supplied".to_string());
        };
        match decide(input) {
            WriteSetOutcome::Bounded(_) => GateResult::Success,
            WriteSetOutcome::CannotDerive { reason } => GateResult::Failure(reason),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_set_proof_is_constructible_within_its_own_module() {
        let proof = WriteSetProof::new(
            vec![Uuid::nil()],
            vec!["validation_state".to_string()],
            vec!["ob-poc.cbus".to_string()],
            vec!["status".to_string()],
            "idem-1",
        );
        assert_eq!(proof.idempotency_key(), "idem-1");
        assert_eq!(proof.tables(), ["ob-poc.cbus"]);
    }

    fn base_input() -> WriteSetInput {
        WriteSetInput {
            entity_ids: vec![Uuid::nil()],
            state_slots: vec!["validation_state".to_string()],
            tables: vec!["ob-poc.cbus".to_string()],
            allowed_columns: vec!["status".to_string()],
            idempotency_key: "idem-1".to_string(),
            contract_derived: true,
        }
    }

    #[test]
    fn contract_derived_with_tables_is_bounded() {
        assert_eq!(
            decide(&base_input()),
            WriteSetOutcome::Bounded(WriteSetProof::new(
                vec![Uuid::nil()],
                vec!["validation_state".to_string()],
                vec!["ob-poc.cbus".to_string()],
                vec!["status".to_string()],
                "idem-1",
            ))
        );
    }

    #[test]
    fn heuristic_only_cannot_derive() {
        let input = WriteSetInput {
            contract_derived: false,
            ..base_input()
        };
        assert!(matches!(decide(&input), WriteSetOutcome::CannotDerive { .. }));
    }

    #[test]
    fn contract_derived_with_no_tables_cannot_derive() {
        let input = WriteSetInput {
            tables: vec![],
            ..base_input()
        };
        assert!(matches!(decide(&input), WriteSetOutcome::CannotDerive { .. }));
    }

    #[test]
    fn gate_evaluate_fails_closed_when_input_missing() {
        let ctx = crate::context::EvaluationContext::default();
        assert!(matches!(WriteSetGate.evaluate(&ctx), GateResult::Failure(_)));
        assert_eq!(WriteSetGate.id(), GateId::WriteSet);
    }
}
