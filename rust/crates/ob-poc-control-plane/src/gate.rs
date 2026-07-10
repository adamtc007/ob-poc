//! Gate identity, the declared dependency graph (V&S §6.16.1), and the
//! collect-where-independent evaluator (T1.3).
//!
//! The dependency graph is a **governance artefact**, not an implementation
//! detail: it is declared here as a `const` table, exactly as V&S §6.16.1
//! requires, so it is versioned, testable, and identical across every
//! implementation of the evaluation loop — "independent" must not become an
//! ad-hoc per-implementation judgement.

use std::collections::BTreeMap;

/// The fourteen control points enumerated in V&S §6 (one per `G1`–`G14`
/// gate mapping used throughout the Phase 0 inventory and this plan).
///
/// Mapping to V&S sections (for traceability, not enforced by the type):
/// G1 §6.1 Intent Admission · G2 §6.2 Entity Binding · G3 §6.3 Semantic Pack
/// Resolution · G4 §6.4 DAG/State-Slot Enforcement · G5 §6.5 Authority and
/// Policy Gate · G6 §6.6 Evidence and Obligation Check · G7 §6.7 Bounded
/// Write-Set Derivation (pre-execution) · G8 §6.8 STP Eligibility
/// Classification · G9 §6.9 Runbook Proof Generation · G10 §6.10 Runtime
/// Execution Envelope · G11 §6.11 Audit and Replay Record · G12 §6.12
/// Version Pinning · G13 §6.15 Decision Snapshot (pins) · G14 §6.7.1
/// Write-Set Attestation (post-execution).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GateId {
    IntentAdmission,
    EntityBinding,
    PackResolution,
    DagProof,
    Authority,
    Evidence,
    WriteSet,
    StpClassifier,
    RunbookProof,
    ExecutionEnvelope,
    AuditReplay,
    VersionPinning,
    DecisionSnapshot,
    WriteSetAttestation,
}

impl GateId {
    /// All fourteen gates, in a stable, deterministic order.
    pub const ALL: [GateId; 14] = [
        GateId::IntentAdmission,
        GateId::EntityBinding,
        GateId::PackResolution,
        GateId::DagProof,
        GateId::Authority,
        GateId::Evidence,
        GateId::WriteSet,
        GateId::StpClassifier,
        GateId::RunbookProof,
        GateId::ExecutionEnvelope,
        GateId::AuditReplay,
        GateId::VersionPinning,
        GateId::DecisionSnapshot,
        GateId::WriteSetAttestation,
    ];
}

/// The outcome of evaluating a single gate.
///
/// `NotImplemented` is the T1 stub outcome for every gate — no adapter has
/// been wired yet (that starts at T2). In shadow mode `NotImplemented` maps
/// to `not_evaluated` for any dependent gate, exactly like a real failure:
/// a stub must never be silently treated as a pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateResult {
    Success,
    Failure(String),
    /// Recorded when one or more declared predecessors did not succeed.
    /// The evaluator produces this automatically; gate implementations
    /// never return it directly.
    NotEvaluated { blocked_by: Vec<GateId> },
    NotImplemented,
}

impl GateResult {
    pub fn is_success(&self) -> bool {
        matches!(self, GateResult::Success)
    }
}

/// A single control point. Implementations for T2+ wrap an existing
/// validator (see the ownership ledger); T1's fourteen implementations are
/// all `UnimplementedGate`, returning `NotImplemented` unconditionally.
pub trait Gate<Ctx> {
    fn id(&self) -> GateId;
    fn evaluate(&self, ctx: &Ctx) -> GateResult;
}

/// A gate stub. Every gate in T1 is one of these; T2+ tranches replace the
/// registration of a given `GateId` with a real adapter type, one gate at a
/// time, per the plan's tranche-by-tranche discipline.
pub struct UnimplementedGate(pub GateId);

impl<Ctx> Gate<Ctx> for UnimplementedGate {
    fn id(&self) -> GateId {
        self.0
    }
    fn evaluate(&self, _ctx: &Ctx) -> GateResult {
        GateResult::NotImplemented
    }
}

/// The declared gate dependency graph, exactly per V&S §6.16.1.
///
/// Only the eight admission-time gates that participate in the
/// collect-where-independent evaluation loop have a declared entry in
/// §6.16.1's table; the remaining six (`RunbookProof`, `ExecutionEnvelope`,
/// `AuditReplay`, `VersionPinning`, `DecisionSnapshot`,
/// `WriteSetAttestation`) are downstream/infrastructural artefacts with no
/// declared predecessor edge in that section — they are enumerated here
/// with an empty dependency list so every `GateId` has exactly one row, and
/// gain real dependency semantics (if any) when their owning tranche wires
/// them (T3–T5).
///
/// Authority's evidence dependency is written in §6.16.1 as conditional
/// ("+ evidence decision where policy requires") rather than an
/// unconditional predecessor, so it is deliberately **not** encoded as a
/// hard graph edge here — T2.4's adapter consumes the evidence gate's
/// outcome as a policy-time input, not a blocking dependency for every
/// invocation. Encoding it as an unconditional edge here would be inventing
/// behaviour the source document does not specify.
pub const GATE_DEPENDENCIES: &[(GateId, &[GateId])] = &[
    (GateId::IntentAdmission, &[]),
    (GateId::EntityBinding, &[GateId::IntentAdmission]),
    (
        GateId::PackResolution,
        &[GateId::IntentAdmission, GateId::EntityBinding],
    ),
    (
        GateId::DagProof,
        &[GateId::EntityBinding, GateId::PackResolution],
    ),
    (
        GateId::Authority,
        &[GateId::IntentAdmission, GateId::PackResolution],
    ),
    (
        GateId::Evidence,
        &[GateId::EntityBinding, GateId::PackResolution],
    ),
    (GateId::WriteSet, &[GateId::DagProof]),
    (
        GateId::StpClassifier,
        &[
            GateId::IntentAdmission,
            GateId::EntityBinding,
            GateId::PackResolution,
            GateId::DagProof,
            GateId::Authority,
            GateId::Evidence,
            GateId::WriteSet,
        ],
    ),
    (GateId::RunbookProof, &[]),
    (GateId::ExecutionEnvelope, &[]),
    (GateId::AuditReplay, &[]),
    (GateId::VersionPinning, &[]),
    (GateId::DecisionSnapshot, &[]),
    (GateId::WriteSetAttestation, &[]),
];

/// Look up the declared predecessors for a gate. Panics if `id` is missing
/// from `GATE_DEPENDENCIES` — every `GateId::ALL` entry must have a row; a
/// missing row is a bug in this table, not a runtime condition to recover
/// from.
pub fn declared_dependencies(id: GateId) -> &'static [GateId] {
    GATE_DEPENDENCIES
        .iter()
        .find(|(gate, _)| *gate == id)
        .map(|(_, deps)| *deps)
        .unwrap_or_else(|| panic!("GATE_DEPENDENCIES has no row for {id:?}"))
}

/// Topological order over `GATE_DEPENDENCIES` (Kahn's algorithm). Ties are
/// broken by `GateId::ALL` declaration order for determinism.
fn topological_order() -> Vec<GateId> {
    let mut in_degree: BTreeMap<GateId, usize> = GateId::ALL
        .iter()
        .map(|id| (*id, declared_dependencies(*id).len()))
        .collect();
    let mut ready: Vec<GateId> = GateId::ALL
        .iter()
        .copied()
        .filter(|id| in_degree[id] == 0)
        .collect();
    let mut order = Vec::with_capacity(GateId::ALL.len());

    while !ready.is_empty() {
        ready.sort();
        let next = ready.remove(0);
        order.push(next);
        for id in GateId::ALL {
            if declared_dependencies(id).contains(&next) {
                let degree = in_degree.get_mut(&id).expect("in_degree has every gate");
                *degree -= 1;
                if *degree == 0 {
                    ready.push(id);
                }
            }
        }
    }

    assert_eq!(
        order.len(),
        GateId::ALL.len(),
        "GATE_DEPENDENCIES contains a cycle — the declared graph must be a DAG"
    );
    order
}

/// The result of a full collect-where-independent evaluation pass: one
/// `GateResult` per `GateId`.
#[derive(Debug, Clone, Default)]
pub struct EvaluationReport {
    pub results: BTreeMap<GateId, GateResult>,
}

impl EvaluationReport {
    pub fn get(&self, id: GateId) -> Option<&GateResult> {
        self.results.get(&id)
    }
}

/// Evaluate every registered gate in dependency order. A gate whose
/// declared predecessors all succeeded is evaluated; otherwise it is
/// recorded `NotEvaluated { blocked_by }` naming every predecessor that did
/// not succeed, without ever calling that gate's `evaluate`. A `GateId`
/// with no registered implementation in `gates` is treated as
/// `NotImplemented` — the same posture as the T1 stub set, so a caller who
/// forgets to register a gate degrades safely rather than being silently
/// skipped from the report.
///
/// This is `collect-where-independent`, not fail-fast: every gate whose
/// inputs do not depend on a failed predecessor is evaluated, and all
/// failures are visible in the returned report (V&S §6.16).
pub fn evaluate_collect_where_independent<Ctx>(
    gates: &BTreeMap<GateId, &dyn Gate<Ctx>>,
    ctx: &Ctx,
) -> EvaluationReport {
    let mut report = EvaluationReport::default();

    for id in topological_order() {
        let deps = declared_dependencies(id);
        let blocked_by: Vec<GateId> = deps
            .iter()
            .copied()
            .filter(|dep| !matches!(report.get(*dep), Some(GateResult::Success)))
            .collect();

        let result = if !blocked_by.is_empty() {
            GateResult::NotEvaluated { blocked_by }
        } else {
            match gates.get(&id) {
                Some(gate) => gate.evaluate(ctx),
                None => GateResult::NotImplemented,
            }
        };

        report.results.insert(id, result);
    }

    report
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every `GateId::ALL` entry must have exactly one row in
    /// `GATE_DEPENDENCIES` — a missing row means `declared_dependencies`
    /// panics on it in production.
    #[test]
    fn every_gate_has_a_dependency_row() {
        for id in GateId::ALL {
            let _ = declared_dependencies(id); // panics on a missing row
        }
        assert_eq!(GATE_DEPENDENCIES.len(), GateId::ALL.len());
    }

    #[test]
    fn topological_order_is_a_valid_linearisation() {
        let order = topological_order();
        assert_eq!(order.len(), GateId::ALL.len());
        for (position, id) in order.iter().enumerate() {
            for dep in declared_dependencies(*id) {
                let dep_position = order
                    .iter()
                    .position(|candidate| candidate == dep)
                    .expect("dependency present in order");
                assert!(
                    dep_position < position,
                    "{id:?} scheduled before its dependency {dep:?}"
                );
            }
        }
    }

    /// Exit criterion (b): "evaluator honours dependency declaration."
    /// A synthetic three-gate chain (independent success -> dependent
    /// failure -> doubly-dependent) proves the evaluator (1) calls
    /// `evaluate` on a gate whose predecessors all succeeded, (2) still
    /// calls `evaluate` on a gate whose predecessor failed (collect, not
    /// fail-fast) but records the *result* of that call rather than
    /// treating failure as success, and (3) refuses to call `evaluate` on a
    /// gate blocked by a non-Success predecessor, instead synthesising
    /// `NotEvaluated { blocked_by }` naming the blocking gate.
    struct FixedGate(GateId, GateResult);
    impl Gate<()> for FixedGate {
        fn id(&self) -> GateId {
            self.0
        }
        fn evaluate(&self, _ctx: &()) -> GateResult {
            self.1.clone()
        }
    }

    #[test]
    fn evaluator_honours_dependency_declaration() {
        // IntentAdmission has no deps -> must be evaluated (Success).
        // EntityBinding depends on IntentAdmission -> evaluated (Failure).
        // PackResolution depends on IntentAdmission + EntityBinding ->
        //   EntityBinding did not succeed, so PackResolution must be
        //   NotEvaluated{blocked_by: [EntityBinding]} without its
        //   `evaluate` ever being called.
        struct PanicsIfCalled;
        impl Gate<()> for PanicsIfCalled {
            fn id(&self) -> GateId {
                GateId::PackResolution
            }
            fn evaluate(&self, _ctx: &()) -> GateResult {
                panic!("PackResolution::evaluate must not be called — blocked by EntityBinding")
            }
        }

        let intent = FixedGate(GateId::IntentAdmission, GateResult::Success);
        let entity = FixedGate(GateId::EntityBinding, GateResult::Failure("no entity".into()));
        let pack = PanicsIfCalled;

        let mut gates: BTreeMap<GateId, &dyn Gate<()>> = BTreeMap::new();
        gates.insert(GateId::IntentAdmission, &intent);
        gates.insert(GateId::EntityBinding, &entity);
        gates.insert(GateId::PackResolution, &pack);

        let report = evaluate_collect_where_independent(&gates, &());

        assert_eq!(report.get(GateId::IntentAdmission), Some(&GateResult::Success));
        assert_eq!(
            report.get(GateId::EntityBinding),
            Some(&GateResult::Failure("no entity".into()))
        );
        assert_eq!(
            report.get(GateId::PackResolution),
            Some(&GateResult::NotEvaluated {
                blocked_by: vec![GateId::EntityBinding]
            })
        );
    }

    /// The default T1 posture: every gate is a `UnimplementedGate`. Only the
    /// zero-dependency gates are ever evaluated (and return
    /// `NotImplemented`); every gate with a declared predecessor is
    /// `NotEvaluated` because `NotImplemented` is not `Success`.
    #[test]
    fn all_stub_gates_produce_not_implemented_or_not_evaluated() {
        let stubs: Vec<UnimplementedGate> = GateId::ALL.iter().map(|id| UnimplementedGate(*id)).collect();
        let mut gates: BTreeMap<GateId, &dyn Gate<()>> = BTreeMap::new();
        for stub in &stubs {
            gates.insert(stub.0, stub);
        }

        let report = evaluate_collect_where_independent(&gates, &());

        for id in GateId::ALL {
            let deps = declared_dependencies(id);
            let result = report.get(id).expect("every gate has a result");
            if deps.is_empty() {
                assert_eq!(
                    result,
                    &GateResult::NotImplemented,
                    "{id:?} has no declared deps and must be evaluated (stubbed)"
                );
            } else {
                assert!(
                    matches!(result, GateResult::NotEvaluated { .. }),
                    "{id:?} has declared deps that are all NotImplemented (not Success), so it must be NotEvaluated, got {result:?}"
                );
            }
        }
    }

    /// A gate present in `GateId::ALL` but missing from the registered
    /// `gates` map degrades to `NotImplemented`, the same as an explicit
    /// `UnimplementedGate` — a forgotten registration is not silently dropped from
    /// the report.
    #[test]
    fn unregistered_gate_degrades_to_not_implemented() {
        let gates: BTreeMap<GateId, &dyn Gate<()>> = BTreeMap::new();
        let report = evaluate_collect_where_independent(&gates, &());
        assert_eq!(
            report.get(GateId::IntentAdmission),
            Some(&GateResult::NotImplemented)
        );
    }
}
