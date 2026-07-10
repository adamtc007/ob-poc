//! G9 — Runbook Proof Generation (V&S §6.9) and the `CompiledRunbookRef`
//! placeholder consumed by `write_set` (T2.6) and `envelope::seal` (T1.2).
//!
//! No production analogue exists today for `ControlPlaneProof` itself
//! (Phase 0 inventory). T3.4 assembles the real aggregate, persisted
//! beside `sem_reg.decision_records` (reuse `snapshot_manifest` pattern,
//! ledger C-044/RR-4).

use uuid::Uuid;

/// A reference to a compiled runbook, opaque to this crate. `ob-poc`'s
/// REPL/compiler owns runbook compilation (§8.5); the control plane only
/// ever holds a reference to the compiled artefact, never re-implements
/// compilation. The concrete linkage (compiled runbook id/hash) is wired
/// when T2.6/T3.4 land — this placeholder exists so `envelope::seal`'s
/// signature can be written now, matching V&S §9.4 exactly.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct CompiledRunbookRef {
    runbook_id: Uuid,
}

impl CompiledRunbookRef {
    pub fn new(runbook_id: Uuid) -> Self {
        Self { runbook_id }
    }

    pub fn runbook_id(&self) -> Uuid {
        self.runbook_id
    }
}

/// `ControlPlaneProof` — V&S §6.9 "Output". The pre-execution artefact
/// that allows the platform, operator, reviewer or auditor to understand
/// exactly what will happen. T1 defines the shape only (a subset of the
/// full §6.9 field list — every field named there is either a proof type
/// already defined in a sibling module, or lands with T3.4's real
/// assembly); the aggregate grows as T2/T3 land the gates it draws from.
#[derive(Debug, Clone)]
pub struct ControlPlaneProof {
    pub runbook: CompiledRunbookRef,
}
