//! G7 — Bounded Write-Set Derivation (V&S §6.7, pre-execution).
//!
//! T2.6 wires the adapter over `derive_write_set` with contract-union
//! promoted to default (plan A4, ledger C-019); the heuristic mode is
//! deleted, not merely disabled, when that tranche lands.
//!
//! Post-execution write-set *attestation* (V&S §6.7.1, gate G14) is a
//! distinct control owned by `write_set_attestation` (T5) — this module
//! only derives the bound, it does not verify the runtime observed a
//! subset of it.

use uuid::Uuid;

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
}
