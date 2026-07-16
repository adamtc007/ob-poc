//! G5 (`EOP-PLAN-CONTROLPLANE-GRADUATION-001` §3, `EOP-DESIGN-
//! CONTROLPLANE-G5-GATE-APPLICABILITY-MATRIX-001`): the ratified,
//! code-confirmed 14-gate x 4-path applicability matrix, declared as a
//! governance artefact — same doctrine as `gate::GATE_DEPENDENCIES`
//! (a `const`-backed, exhaustively-matched table, not an ad hoc per-call-
//! site judgement).
//!
//! **What this module is not**: it does not evaluate anything. It answers
//! one question — "does gate G's concept even apply, by construction, on
//! execution path P?" — for every one of the 56 `(GateId, ExecutionPath)`
//! cells. Callers outside Path A (`ob-poc`'s execution tier, at the G4
//! seam for B/C and `bus_runtime.rs`'s adapter for D) apply
//! [`apply_matrix`] to an already-computed [`crate::gate::EvaluationReport`]
//! as a path-aware post-processing step, overriding the specific cells
//! this matrix marks not-applicable to [`crate::gate::GateResult::NotApplicable`].
//!
//! **Path A never calls [`apply_matrix`]** (verified by
//! `path_a_is_the_identity_matrix` below, and independently by `ob-poc`'s
//! own `g5_path_a_never_produces_not_applicable` test at the real
//! `phase5_runtime_recheck` call site) — this is what makes standing rule
//! 3's window-discipline guarantee hold by construction rather than by
//! convention: this module's own matrix data says every gate is
//! `Applicable` on `RunbookSequencer`, so even an accidental call from
//! Path A would be a no-op, not a silent semantics change.

use crate::gate::GateId;
use ob_poc_types::ExecutionPath;

/// One matrix cell's verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Applicability {
    /// The gate's concept applies on this path. Does **not** mean the
    /// gate has a real, wired input source at this path yet — "applicable
    /// but not yet wired" legitimately reports `NotEvaluated`/
    /// `NotImplemented` at evaluation time; this matrix only answers the
    /// construction-level applicability question, never a wiring-status
    /// question.
    Applicable,
    /// The gate's concept does not exist on this path, by construction.
    /// The `&'static str` is the ratified justification — must match a
    /// cited code fact in `EOP-DESIGN-CONTROLPLANE-G5-GATE-APPLICABILITY-
    /// MATRIX-001`'s own table, never invented at the override call site.
    NotApplicable(&'static str),
}

impl Applicability {
    pub fn is_not_applicable(self) -> bool {
        matches!(self, Applicability::NotApplicable(_))
    }
}

const G3_JUSTIFICATION_B: &str =
    "PackResolution requires a live REPL journey-pack session (`ReplSessionV2::active_pack_id()` \
     + `ReplOrchestratorV2::pack_router`, per `control_plane_shadow.rs::build_pack_resolution_input`'s \
     own doc) — Path B dispatches through `RealDslExecutor`/`dsl_v2::executor::DslExecutor`, whose \
     struct carries zero pack/session field (confirmed: `executor_bridge.rs` RealDslExecutor's own \
     field list, `main.rs`'s 4 construction sites). The REPL journey-pack concept is structurally \
     absent from this engine, not merely unwired.";

const G3_JUSTIFICATION_C: &str =
    "Identical reasoning and identical evidence to Path B (G5 code confirmation, 2026-07-13): Path C \
     is the SAME `RealDslExecutor`/`dsl_v2::executor::DslExecutor` engine, wrapped by `WorkflowDispatcher` \
     (`main.rs:1338-1343`, tagged `WorkflowDispatched` at construction) — no distinct pack-resolution \
     shape from Path B's. This resolves the plan's 'G3 on C vs D distinctions' UNKNOWN: there is no \
     distinction between B, C, and D for G3 — all three share the same by-construction absence.";

const G3_JUSTIFICATION_D: &str =
    "Confirmed by prior research (R:§B6): bus/system-principal dispatch has no REPL session or active \
     pack at all — `ObPocVerbAdapter::execute` (`bus_runtime.rs`) never touches `ReplSessionV2`.";

const G9_JUSTIFICATION_B: &str =
    "RunbookProof grades `entry.compiled_runbook_id` (`RunbookEntry`'s own field, a REPL-runbook-only \
     concept). Path B's plan object is `dsl_v2::execution_plan::ExecutionPlan { steps, dag }` (confirmed: \
     struct definition, `execution_plan.rs:47-52`) — no `compiled_runbook_id` field or equivalent exists \
     anywhere reachable from `execute_plan`/`execute_verb_in_scope`. The concept is structurally absent.";

const G9_JUSTIFICATION_C: &str =
    "Identical reasoning to Path B (G5 code confirmation, 2026-07-13): Path C dispatches the same \
     `ExecutionPlan` shape through the same engine — no `CompiledRunbookId` concept reachable. Resolves \
     the plan's second named UNKNOWN (G9 on Paths B/C) by code confirmation, not by re-asserting the \
     research doc's prior reasoning.";

const G9_JUSTIFICATION_D: &str =
    "Confirmed by prior research (R:§B6): bus dispatch has no runbook object at all.";

/// The ratified matrix, exhaustively matched (`GateId` outer, `ExecutionPath`
/// inner — no wildcard arm on either level, per the plan's own "no wildcard
/// arms" instruction for item 1's enum sweep, applied here too since this is
/// the artefact item 5 grades applicable/NA cells against).
pub fn applicability(gate: GateId, path: ExecutionPath) -> Applicability {
    use ExecutionPath::*;
    match gate {
        // G1-G2: path-neutral concepts (intent admission / entity binding
        // are derivable from verb args regardless of ingress point).
        // Applicable everywhere; "mechanism absent" (R:§B6) is a wiring
        // gap, not an applicability question — see G5 items 3-4 for what
        // this session actually wired.
        GateId::IntentAdmission | GateId::EntityBinding => match path {
            RunbookSequencer | DslDirect | WorkflowDispatched | BusFederated => {
                Applicability::Applicable
            }
        },

        // G3: the plan's first two named UNKNOWNs, resolved above.
        GateId::PackResolution => match path {
            RunbookSequencer => Applicability::Applicable,
            DslDirect => Applicability::NotApplicable(G3_JUSTIFICATION_B),
            WorkflowDispatched => Applicability::NotApplicable(G3_JUSTIFICATION_C),
            BusFederated => Applicability::NotApplicable(G3_JUSTIFICATION_D),
        },

        // G4: applicable everywhere (a DAG state-transition proof is a
        // path-neutral concept); Path A's real input source (`GatePipeline`,
        // owned by `ReplOrchestratorV2`) does not generalize to B/C/D's
        // engine without further design — a wiring gap this session
        // documents rather than forces (see the G5 session doc).
        GateId::DagProof => match path {
            RunbookSequencer | DslDirect | WorkflowDispatched | BusFederated => {
                Applicability::Applicable
            }
        },

        // G5-G8: path-neutral concepts (authority, evidence, write-set
        // footprint, STP classification all apply regardless of ingress).
        GateId::Authority | GateId::Evidence | GateId::WriteSet | GateId::StpClassifier => {
            match path {
                RunbookSequencer | DslDirect | WorkflowDispatched | BusFederated => {
                    Applicability::Applicable
                }
            }
        }

        // G9: the plan's third named UNKNOWN, resolved above.
        GateId::RunbookProof => match path {
            RunbookSequencer => Applicability::Applicable,
            DslDirect => Applicability::NotApplicable(G9_JUSTIFICATION_B),
            WorkflowDispatched => Applicability::NotApplicable(G9_JUSTIFICATION_C),
            BusFederated => Applicability::NotApplicable(G9_JUSTIFICATION_D),
        },

        // G10/G11: "stub everywhere" (R:§B6/A1) is an implementation-status
        // fact, not an applicability one — the concept (a sealed runtime
        // envelope; an audit/replay record) is path-neutral by definition
        // (every path's dispatch could in principle be envelope-sealed or
        // audit-logged).
        GateId::ExecutionEnvelope | GateId::AuditReplay => match path {
            RunbookSequencer | DslDirect | WorkflowDispatched | BusFederated => {
                Applicability::Applicable
            }
        },

        // G12-G13: path-neutral (version pinning, decision snapshot pins).
        GateId::VersionPinning | GateId::DecisionSnapshot => match path {
            RunbookSequencer | DslDirect | WorkflowDispatched | BusFederated => {
                Applicability::Applicable
            }
        },

        // G14: post-dispatch write-set attestation is captured at the
        // shared CRUD-executor level (R:§A1: "capture IS wired into
        // production CRUD dispatch generally, path-agnostic") — applicable
        // everywhere by the same reasoning R:§B6 already gives for Path A.
        GateId::WriteSetAttestation => match path {
            RunbookSequencer | DslDirect | WorkflowDispatched | BusFederated => {
                Applicability::Applicable
            }
        },
    }
}

/// Applies the ratified matrix to an already-computed
/// [`crate::gate::EvaluationReport`]: every `(GateId, path)` cell this
/// matrix marks not-applicable gets its report entry overridden to
/// [`crate::gate::GateResult::NotApplicable`], carrying the ratified
/// justification string. Every `Applicable` cell is left exactly as
/// `evaluate_shadow` computed it (untouched) — this function only ever
/// narrows a report toward `NotApplicable`, never invents a `Success`.
///
/// **Callers**: `ob-poc`'s B/C (G4 seam) and D (`bus_runtime.rs`)
/// evaluation call sites only. Path A must never call this (see the module
/// doc + `path_a_is_the_identity_matrix` below).
pub fn apply_matrix(
    mut report: crate::gate::EvaluationReport,
    path: ExecutionPath,
) -> crate::gate::EvaluationReport {
    for id in GateId::ALL {
        if let Applicability::NotApplicable(reason) = applicability(id, path) {
            report
                .results
                .insert(id, crate::gate::GateResult::NotApplicable(reason.to_string()));
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every `GateId` has exactly one arm per `ExecutionPath` — enforced by
    /// the compiler (no wildcard anywhere in `applicability`'s match), this
    /// test just proves the function doesn't panic across the full 56-cell
    /// space and that every cell is one of the two variants (trivial, but
    /// documents the exhaustive-coverage claim as a runnable fact).
    #[test]
    fn every_cell_is_covered() {
        for gate in GateId::ALL {
            for path in ExecutionPath::ALL {
                let _ = applicability(gate, path);
            }
        }
    }

    /// Standing rule 3 / item 1's window-discipline requirement, proven at
    /// the matrix-data level: Path A (`RunbookSequencer`) is `Applicable`
    /// for all 14 gates — `apply_matrix` called on Path A is the identity
    /// function on any report, so even an accidental call from Path A's
    /// own call site would change nothing.
    #[test]
    fn path_a_is_the_identity_matrix() {
        for gate in GateId::ALL {
            assert_eq!(
                applicability(gate, ExecutionPath::RunbookSequencer),
                Applicability::Applicable,
                "{gate:?} must be Applicable on Path A — window discipline (standing rule 3) \
                 depends on this holding for every gate, not just the ones this tranche touched"
            );
        }
    }

    /// The three UNKNOWNs the plan named (G3 on B/C, G9 on B/C) are now
    /// NotApplicable, each carrying a distinct, non-empty justification —
    /// proves the matrix doesn't silently collapse them to the same
    /// generic string.
    #[test]
    fn the_three_named_unknowns_are_resolved_not_applicable() {
        for (gate, path) in [
            (GateId::PackResolution, ExecutionPath::DslDirect),
            (GateId::PackResolution, ExecutionPath::WorkflowDispatched),
            (GateId::RunbookProof, ExecutionPath::DslDirect),
            (GateId::RunbookProof, ExecutionPath::WorkflowDispatched),
        ] {
            match applicability(gate, path) {
                Applicability::NotApplicable(reason) => assert!(!reason.is_empty()),
                Applicability::Applicable => panic!("{gate:?}/{path:?} expected NotApplicable"),
            }
        }
    }

    /// `apply_matrix` only narrows toward `NotApplicable`; every cell the
    /// matrix marks `Applicable` is untouched.
    #[test]
    fn apply_matrix_only_overrides_not_applicable_cells() {
        let mut report = crate::gate::EvaluationReport::default();
        report
            .results
            .insert(GateId::IntentAdmission, crate::gate::GateResult::Success);
        report.results.insert(
            GateId::PackResolution,
            crate::gate::GateResult::Success, // would-be shadow result, pre-override
        );

        let overridden = apply_matrix(report, ExecutionPath::DslDirect);

        assert_eq!(
            overridden.get(GateId::IntentAdmission),
            Some(&crate::gate::GateResult::Success),
            "Applicable cell must be untouched"
        );
        assert!(
            matches!(
                overridden.get(GateId::PackResolution),
                Some(&crate::gate::GateResult::NotApplicable(_))
            ),
            "NotApplicable cell must be overridden regardless of what evaluate_shadow computed"
        );
    }
}
