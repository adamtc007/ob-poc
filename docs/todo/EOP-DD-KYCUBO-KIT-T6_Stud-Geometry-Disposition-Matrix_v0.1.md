# EOP-DD-KYCUBO-KIT-T6 — Stud-Geometry Disposition Matrix

| | |
|---|---|
| **Document** | EOP-DD-KYCUBO-KIT-T6 |
| **Version** | 0.1 — for ratification (accept-all-or-amend, per row) |
| **Binds to** | EOP-PLAN-KYCUBO-KIT-001 v0.6 §T6.1–T6.4; EOP-DD-KYCUBO-KIT-T0.3 v0.2 (K-G5 register); state-of-tree facts of 2026-08-12 (Precondition enum `lexicon.rs:50`; checker `control.rs:400`; ControlState `control.rs:147`; ObligationState `obligation.rs:130`) |
| **Status** | **RATIFIED 2026-08-12 — ACCEPT ALL 18 rows as drafted.** Q2 judgment calls confirmed: reconcile-conflict legal with zero edges (row 5 as drafted, no amendment); reclassification stays legal last-wins (row 10); reject legal at any stage (row 18). Encoding: 6a/8a in T6.1; rows 1–5 → T6.2; 6–10 remainder → T6.3; 11–18 → T6.4. |

**Reading the table.** 17 verbs carry zero preconditions today (universe 20 minus `verify`, `compute-fold`, `freeze`). Each row: the proposed stud(s) in domain language, the mechanism (EXISTING variant / NEW variant, and which fold-state it reads), and what the stud prevents. Evaluability is per the verified state structs — **no row requires new fold output**; rows marked ⊗ require the T6.1 unified checker (ObligationState reach). Ratify per row: ACCEPT / AMEND (state the change) / DEFER (row stays geometry-free, consciously).

---

## §1 The matrix

**Edge family (T6.2)**

| # | Verb | Proposed stud(s) | Mechanism | Prevents |
|---|---|---|---|---|
| 1 | `ubo.edge.assert-control` | Subject must be registered; no active edge with same (from, to, kind) may exist — contradicting claims go through `supersede` | NEW `SubjectRegistered` (reads `state.registered`); NEW `NoDuplicateActiveEdge` (scans edges map vs event payload) | orphan edges on unregistered subjects; silent double-assertion (K-13 discipline: supersede, never contradict) |
| 2 | `ubo.edge.assert-economic-interest` | Same two as row 1 | Same variants, reused | same; percentage range stays arg-validation (`valid_values`), not a stud |
| 3 | `ubo.edge.attach-evidence` | Target edge must exist and not be superseded | NEW `EdgeExists` + NEW `EdgeActive` (target lookup, same pattern as existing `EvidenceCited`) | evidence attached to nothing / to dead edges |
| 4 | `ubo.edge.supersede` | Target edge must exist and not already be superseded | `EdgeExists` + `EdgeActive` (reused) | double-supersede no-ops polluting the stream |
| 5 | `ubo.edge.reconcile-conflict` | Subject must be registered | `SubjectRegistered` (reused) | vacuous reconciliation events. *(Optional amendment: also require ≥1 active economic edge — flag if wanted; drafted WITHOUT it to keep reconcile callable early.)* |

**Determination family (T6.3; exemplar rows 6a/8a ship in T6.1)**

| # | Verb | Proposed stud(s) | Mechanism | Prevents |
|---|---|---|---|---|
| 6 | `ubo.determination.select-strategy` | Subject registered; structure classified first; **(6a, T6.1 exemplar) class must have an implemented strategy** | `SubjectRegistered`; NEW `StructureClassified` (`structure_class.is_some()`); NEW `StructureClassSupported` (class ∈ pinned implemented set) | strategy before classification; **silently-wrong determinations for the 6 strategy-less classes → fail-closed** |
| 7 | `ubo.determination.apply-smo-fallback` | Reconciled + strategy selected (same stage-gates as compute-fold) | EXISTING `ReconciledProjection` + `StrategySelected` — **zero new machinery** | SMO fallback firing before the determination stage is even set up |
| 8 | `ubo.determination.freeze` | *(8a, T6.1 exemplar)* ADD `StructureClassSupported` to its existing two | reused from 6a | defense in depth at the terminal verb — the guard holds even if select-strategy is bypassed |
| 9 | `kyc.subject.register` | Subject must NOT already be registered | NEW `NotAlreadyRegistered` (`!state.registered`) | double-registration; re-registration becomes a conscious future verb if ever needed |
| 10 | `kyc.subject.classify-structure` | Subject must be registered | `SubjectRegistered` (reused). Reclassification stays legal (last-wins, per current fold) | classifying phantoms; deliberately does NOT freeze the class — reclassify is a real workflow |

**Obligation / person family (T6.4; every row ⊗ = needs the T6.1 unified checker)**

| # | Verb | Proposed stud(s) | Mechanism | Prevents |
|---|---|---|---|---|
| 11 | `kyc.obligation.create` ⊗ | Subject must be registered (cross-fold: reads ControlState from an obligation verb — THE motivating case for the unified checker) | `SubjectRegistered` (reused, via unified checker) | obligations against phantom subjects |
| 12 | `kyc.obligation.update-identity` ⊗ | Target obligation exists; subject not already decided | NEW `ObligationExists` (obligations map); NEW `SubjectNotDecided` (rollup ≠ Approved/Rejected) | updating nothing; mutating tracks after the decision (K-23: decision is final) |
| 13 | `kyc.obligation.update-screening` ⊗ | Same as row 12 | reused | same |
| 14 | `kyc.obligation.update-risk` ⊗ | Same as row 12 | reused | same |
| 15 | `kyc.obligation.satisfy` ⊗ | Same as row 12 | reused | satisfying dead/decided work |
| 16 | `kyc.obligation.waive` ⊗ | Same as row 12. *(Waive's extra sensitivity is an AUTHORITY question — `AuthoritySpec.compliance_officer()` already exists in the entry shape; flagged as an authority ruling, not a stud.)* | reused | same |
| 17 | `kyc.person.approve` ⊗ | **All required obligation tracks terminal (THE K-23 GATE — closes the DD-003 finding that approve is ungated)**; subject not already decided | NEW `SubjectAllTerminal` (`derive_subject_state() == AllTerminal` — helper already exists); `SubjectNotDecided` (reused) | **approving a subject with open obligations — the second known silently-wrong hole, after the strategy guard** |
| 18 | `kyc.person.reject` ⊗ | Subject not already decided. Rejection deliberately allowed at ANY stage (early rejection is a real compliance outcome) | `SubjectNotDecided` (reused) | double-decision; keeps early-reject legal on purpose |

---

## §2 Variant inventory (what T6.1(b) builds — the complete new-machinery bill)

| Variant | Reads | Niladic? | Used by rows |
|---|---|---|---|
| `SubjectRegistered` | `ControlState.registered` | yes | 1,2,5,6,10,11 |
| `NotAlreadyRegistered` | same, negated | yes | 9 |
| `StructureClassified` | `structure_class.is_some()` | yes | 6 |
| `StructureClassSupported` | `structure_class` ∈ pinned implemented-strategy set | yes (set is a pinned const, guarded by the seventh tooth) | 6a, 8a |
| `NoDuplicateActiveEdge` | edges map vs event (from,to,kind) | yes (reads event, like `EvidenceCited`) | 1,2 |
| `EdgeExists` / `EdgeActive` | edges map via `target.edge_id` | yes | 3,4 |
| `ObligationExists` | `ObligationState.obligations` via target | yes | 12–16 |
| `SubjectNotDecided` | rollup `overall_state` | yes | 12–18 |
| `SubjectAllTerminal` | `derive_subject_state()` | yes | 17 |

**Notable:** every variant is **niladic** — the parameterised-variant machinery the plan feared is NOT needed for this matrix. All state reads exist today (facts §"Key state-of-tree" 1–2 in plan v0.6); the only structural work is the unified two-fold checker signature.

## §3 What is deliberately NOT proposed
Percentage/enum arg checks (that's `valid_values`); authority tightening (that's `AuthoritySpec`, a separate ruling); any stud on `verify`/`compute-fold`/`freeze` beyond 8a (they keep their existing geometry); temporal studs (as_of ordering — T5's axis work first); reclassification freezing (row 10 note).

## §4 Ratification
Reply per row (ACCEPT / AMEND / DEFER), or "accept all". On ratification: 6a+8a encode in **T6.1** (with the machinery); rows 1–5 → **T6.2**; 6–10 remainder → **T6.3**; 11–18 → **T6.4**. Each tranche = one kit-hash batch; sessions re-open after.

## §5 Post-ratification rulings

**Row-9 amendment (keyed registration check) — RULED 2026-08-14: NO CHANGE.** The proposed
amendment (widen `NotAlreadyRegistered` from the per-subject `!state.registered` to a keyed
`(subject_root, entity_id)` already-registered check) is **rejected**. Rationale: the stream is
append-only and the fold is convergent — a duplicate `kyc.subject.register` for the same pair
folds to the same state, so duplicates are harmless by construction; blocking them would require
a new keyed (parameterised) precondition primitive, breaking the all-niladic property §2 records,
for zero behavioral benefit. Idempotent/duplicate registration is accepted downstream behavior.
Row 9 stands exactly as ratified.

## Change Log
| Version | Date | Note |
|---|---|---|
| 0.1 | 2026-08-12 | Initial matrix: 17 verbs, 18 rows (freeze appears twice: existing studs + 8a guard), 10 new niladic variants, zero new fold outputs, zero parameterised variants needed. The two silently-wrong holes (strategy guard 6a/8a; K-23 gate row 17) called out as the highest-value rows. |
| 0.1.1 | 2026-08-14 | §5 added: row-9 keyed-registration amendment RULED NO CHANGE (idempotent duplicates accepted; niladic property preserved). |
