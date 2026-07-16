# EOP-DESIGN-CONTROLPLANE-G5-GATE-APPLICABILITY-MATRIX-001 — Gate Applicability Matrix

### Implements: G5 item 2 (`EOP-PLAN-CONTROLPLANE-GRADUATION-001_v0.5` §3), resolving the three
### reasoned-not-confirmed UNKNOWNs `EOP-RESEARCH-CONTROLPLANE-GRADUATION-001.md` §B6 left open.
### Status: **DRAFT** — architect ratification pending (per this branch's established pattern:
### the implementing session proposes, a human ratifies).
### Date: 2026-07-13
### Code artefact: `rust/crates/ob-poc-control-plane/src/applicability.rs` (the matrix is
### declared as an exhaustively-matched `const`-shaped function, `applicability(GateId,
### ExecutionPath) -> Applicability`, mirroring `gate.rs`'s `GATE_DEPENDENCIES` doctrine — this
### doc is the human-readable rationale for that code, not a separate source of truth).

---

## 1. What this matrix is

R:§B6 found that the G1–G14 shadow-evaluation pipeline (`evaluate_shadow`,
`ob-poc-control-plane/src/lib.rs`) ran only from Path A's
`phase5_runtime_recheck` (`sequencer.rs`) — Paths B, C, D never reached the
14-gate stack at all. R:§B6 also produced a *draft* 14×4 applicability
table, but flagged three cells as **reasoned-not-confirmed UNKNOWNs**: G3
(PackResolution) on Path B, G3 on Path C, and the "G3 on C vs D
distinctions" question (is C's reasoning the same shape as D's, or
different?). It also left G9 (RunbookProof) on B/C as an unconfirmed
"may or may not" case.

This document is the **code-confirmed** resolution of those UNKNOWNs (§3
below), plus the full ratified 14×4 matrix (§2), plus a record of what G5
actually wired at the B/C/D call sites this session (§4) versus what
remains a genuine, disclosed generalization gap (§5).

**Status discipline**: this doc is DRAFT, not RATIFIED. Per the pattern
established across every prior session on this branch (G1's seal→consume
doc, G3's enforcement-dimension doc), the implementing session proposes a
design; a human architect ratifies it. Until ratified, `check-invariants.sh
e3`'s per-matrix assertions are real, live, running code (see §4) — but the
matrix's own NORMATIVE status for E3's completion definition (per the plan's
§0 completion-invariant amendment: *"E3 is evaluated per the
gate-applicability matrix... until G5 ratifies the matrix, E3's expected
status remains `fail`"*) is pending this doc's ratification.

---

## 2. The ratified (pending) 14×4 matrix

Columns: **A** = `RunbookSequencer` (Path A), **B** = `DslDirect` (Path B),
**C** = `WorkflowDispatched` (Path C), **D** = `BusFederated` (Path D) — the
four `ob_poc_types::ExecutionPath` variants, matching the graduation
runbook's own A/B/C/D letters.

| Gate | A | B | C | D |
|---|---|---|---|---|
| G1 IntentAdmission | Applicable | Applicable | Applicable | Applicable |
| G2 EntityBinding | Applicable | Applicable | Applicable | Applicable |
| G3 PackResolution | Applicable | **NotApplicable** | **NotApplicable** | NotApplicable |
| G4 DagProof | Applicable | Applicable | Applicable | Applicable |
| G5 Authority | Applicable | Applicable | Applicable | Applicable |
| G6 Evidence | Applicable | Applicable | Applicable | Applicable |
| G7 WriteSet | Applicable | Applicable | Applicable | Applicable |
| G8 StpClassifier | Applicable | Applicable | Applicable | Applicable |
| G9 RunbookProof | Applicable | **NotApplicable** | **NotApplicable** | NotApplicable |
| G10 ExecutionEnvelope | Applicable | Applicable | Applicable | Applicable |
| G11 AuditReplay | Applicable | Applicable | Applicable | Applicable |
| G12 VersionPinning | Applicable | Applicable | Applicable | Applicable |
| G13 DecisionSnapshot | Applicable | Applicable | Applicable | Applicable |
| G14 WriteSetAttestation | Applicable | Applicable | Applicable | Applicable |

Cells in **bold** are this session's own resolutions (G3/G9 on B, G3/G9 on
C — four cells, matching the plan's "G3 and G9 on Paths B/C" naming). D's
column for G3/G9 was already confirmed not-applicable by R:§B6 and is
carried here unchanged, not re-derived.

**"Applicable" is a construction-level claim, not a wiring-status claim.**
Most Applicable cells for B/C/D are NOT yet wired to a real input source
(see §4/§5) — they legitimately report `NotEvaluated`/`NotImplemented`
today, exactly as G10/G11 have always reported for Path A prior to their
own tranches wiring them. "Applicable" only says the *concept* the gate
grades exists on that path; it says nothing about today's evidence.

---

## 3. Resolving the three UNKNOWNs — code confirmation, not re-assertion

### 3.1 G3 (PackResolution) on Path B — **NotApplicable**

R:§B6's own reasoning for B was speculative ("raw/best-effort DSL
execution... may have no active REPL journey pack in the same sense Path
A's compiled-runbook flow does; not independently confirmed"). This session
confirmed it by reading the actual engine Path B dispatches through.

- Path A's real G3 input source is `ReplSessionV2::active_pack_id()` +
  `ReplOrchestratorV2::pack_router` (`control_plane_shadow.rs::
  build_pack_resolution_input`'s own doc, and the function body itself).
- Path B dispatches via `RealDslExecutor` → `dsl_v2::executor::DslExecutor`.
  `RealDslExecutor`'s own struct definition (`rust/src/repl/
  executor_bridge.rs:29-46`) has exactly five fields: `pool`,
  `allow_durable_direct`, `service_registry`, `sem_os_ops`,
  `execution_path`. **No pack, session, or `ReplSessionV2` reference
  anywhere.** `RealDslExecutor::new(pool: PgPool) -> Self` takes no
  session/pack argument either (same file, `:48-57`).
- `main.rs`'s four `RealDslExecutor` construction sites (`inner`,
  `worker_executor`, `legacy_executor`, the bare `executor_v2` fallback —
  `EOP-SESSION-CONTROLPLANE-G4-IMPL-001` §4) confirm the same: none is
  constructed with a pack or session.

**Verdict: NotApplicable-by-construction, not merely unwired.** The REPL
journey-pack concept G3 grades does not exist as a reachable field on the
engine that serves Path B, full stop — the same class of finding R:§B6
already made for Path D by inspection of `ObPocVerbAdapter`.

### 3.2 G3 (PackResolution) on Path C — **NotApplicable**

R:§B6 called this "a stronger not-applicable-by-construction candidate
than B" but still marked it not independently confirmed. This session
confirmed: Path C is **the same `RealDslExecutor`/`dsl_v2::executor::
DslExecutor` engine** as Path B — the `inner` executor `main.rs:1338-1343`
constructs (tagged `ExecutionPath::WorkflowDispatched` at construction) is
wrapped by `WorkflowDispatcher`, which routes Direct-vs-Orchestrated
dispatch but does not change the inner executor's own type or fields. No
distinct pack-resolution mechanism exists for C.

**Verdict: NotApplicable-by-construction, identical evidence and identical
reasoning to Path B.**

### 3.3 "G3 on C vs D distinctions" — **there is no distinction**

The plan's own item 2 named this as a third thing to resolve: is Path C's
G3 applicability shape different from Path D's confirmed not-applicable
verdict? Per §3.1/§3.2 above: **no.** All three of B, C, and D share the
identical structural fact — no REPL pack/session concept reachable from
their respective dispatch engines (`dsl_v2::executor::DslExecutor` for
B/C; `ObPocVerbAdapter`/`ObPocVerbExecutor` for D, per R:§B6's own
citation). This UNKNOWN resolves to "the three non-A paths are uniform for
G3," not to a finer-grained distinction.

### 3.4 G9 (RunbookProof) on Paths B and C — **NotApplicable** (both)

The plan named this as the second UNKNOWN needing resolution alongside G3.
R:§B6 called B "explicitly the case where 'no `CompiledRunbookId`' is a
legitimate, not-rare outcome" (a softer claim than not-applicable) and C
"may or may not carry a compiled-runbook reference; not confirmed."

- Path A's real G9 input source is `entry.compiled_runbook_id` — a field
  on `RunbookEntry`, the REPL runbook's own per-step record
  (`control_plane_shadow.rs::build_runbook_proof_input`'s doc).
- Path B/C's plan object is `dsl_v2::execution_plan::ExecutionPlan`. Its
  full struct definition (`rust/src/dsl_v2/execution_plan.rs:47-52`):
  ```rust
  pub struct ExecutionPlan {
      pub steps: Vec<ExecutionStep>,
      pub dag: PopulatedExecutionDag,
  }
  ```
  No `compiled_runbook_id` field, and no equivalent reachable from
  `execute_plan`/`execute_plan_atomic_in_scope`/`execute_verb_in_scope`
  (the full call chain both B and C's dispatch goes through).

**Verdict: NotApplicable-by-construction for both B and C, same evidence
shape.** This resolves the plan's second named UNKNOWN by code
confirmation rather than re-asserting R:§B6's own hedge.

---

## 4. What G5 actually wired at the B/C/D call sites (this session)

Two new evaluation call sites, both best-effort/spawned (never block real
dispatch, matching Path A's own shadow posture):

- **B/C**: `dsl_v2::executor::DslExecutor::execute_verb_in_scope`
  (`rust/src/dsl_v2/executor.rs`), immediately after the G4 admission block
  and before dispatch branches.
- **D**: `ObPocVerbAdapter::execute` (`rust/crates/ob-poc-web/src/
  bus_runtime.rs`), immediately before the admitting `execute_verb_
  admitting_envelope` call.

Both call `ob_poc_control_plane::evaluate_shadow`, then
`ob_poc_control_plane::applicability::apply_matrix(report, path)` (§2's
matrix, applied as a path-aware override — see `applicability.rs`'s module
doc), then persist via `control_plane_shadow::{build_shadow_decision_row,
insert_shadow_decision}` (both widened `pub(crate)` → `pub` this session so
`ob-poc-web`, a different crate, can reach them for Path D — a disclosed
pub-surface change, see the session doc's blind-review summary).

**Gates genuinely wired to a real, independently-substantive input at
these two call sites**: G1 (IntentAdmission — weaker evidence than Path
A's SemOS-derived signal: "did `runtime_registry().get(domain,verb)`
resolve," disclosed as weaker, not equivalent) and G12 (VersionPinning —
`env!("CARGO_PKG_VERSION")`, identical reuse of Path A's own builder logic,
zero session dependency). Both have zero declared predecessors in
`gate::GATE_DEPENDENCIES`, so they evaluate to a real `Success`/`Failure`
standalone.

**G8 (StpClassifier) input is also built at both call sites** (reusing the
same `is_durable_verb` derivation Path A uses), but it is **not**
independently substantive: `GATE_DEPENDENCIES` declares StpClassifier
depends on `[IntentAdmission, EntityBinding, PackResolution, DagProof,
Authority, Evidence, WriteSet]`. Since G2/G4-G7 are not wired at these call
sites this session, StpClassifier correctly reports
`NotEvaluated{blocked_by:[...]}` under `evaluate_collect_where_independent`'s
real collect-where-independent semantics — this was discovered live (the
E3 matrix probe's first run failed on exactly this before the probe's own
gate list was corrected; see the session doc).

**G3/G9**: overridden to `NotApplicable` by `apply_matrix`, per §2/§3
above — a first-class, ratified-justification-carrying outcome, verified
live by the E3 matrix probe.

---

## 5. Disclosed generalization gap — everything NOT wired this session

G2 (EntityBinding), G4 (DagProof), G5 (Authority), G6 (Evidence), G7
(WriteSet), G10 (ExecutionEnvelope), G11 (AuditReplay), G13
(DecisionSnapshot), G14 (WriteSetAttestation) are `Applicable` on B/C/D per
§2's matrix but were **not** wired to a real input source at the B/C/D
call sites this session. Their Path-A builders all assume state that does
not exist on the engines serving B/C/D:

- `build_entity_binding_input`/`entity_binding_requests` (G2) need a
  `VerbConfigIndex` + a batched `EntityFactsSource` fetch — real I/O this
  session did not wire at either new call site.
- `build_dag_proof_input` (G4) needs a `GatePipeline`, which is owned by
  `ReplOrchestratorV2` — not reachable from `dsl_v2::executor::
  DslExecutor` or `ObPocVerbAdapter` at all.
- Authority/Evidence (G5/G6) at Path A are derived from
  `SemOsContextEnvelope.pruned_verbs`/`evidence_gaps` — no equivalent
  envelope object is built at the B/C/D call sites.
- WriteSet (G7) needs `DomainMetadata` lookup — plumbable in principle,
  not wired this session.
- G10/G11/G14 remain "stub everywhere" per R:§A1 regardless of path (a
  separate, G1/G2-scoped gap, not a B/C/D-specific one).
- G13 (DecisionSnapshot) needs the same batched entity-facts rows as G2.

This is a **genuine generalization gap**, not an oversight papered over:
each of these Path-A builders assumes Sequencer-specific or
`ReplOrchestratorV2`-specific state, and forcing them onto B/C/D's engines
would require either (a) threading that state onto `DslExecutor`/
`ObPocVerbAdapter` (a real design change to those types, out of this
session's mechanical-sweep-plus-bounded-wiring scope), or (b) building
parallel, weaker input sources per gate (a larger piece of work, deferred
rather than forced through). Follow-on tranches should treat this as named,
scoped work — not rediscover it.

---

## 6. Ratification checklist (for the architect)

- [ ] §2's 14×4 matrix — confirm no cell should be re-derived.
- [ ] §3's four resolved UNKNOWN cells — confirm the code citations are
      read correctly (line numbers may drift; the structural claim —
      "zero pack/session field on `RealDslExecutor`," "no
      `compiled_runbook_id` field on `ExecutionPlan`" — should hold
      regardless of exact line numbers).
- [ ] §4/§5's disclosed scope (what's wired vs what's a generalization
      gap) — confirm this is an acceptable partial-completion posture for
      G5's exit gate, or direct a wider wiring pass before ratifying.
- [ ] Once ratified, flip this doc's own header status line to RATIFIED
      (same pattern as the G1/G3 design docs) and apply the
      `invariants-expected.toml` `[e3]` recommendation in the G5 session
      doc.
