# INDEX — Control-Plane Governing Artifacts

### EOP-INDEX-CONTROLPLANE-001
### Created: 2026-07-13 (EOP-IMPL-CONTROLPLANE-GRADUATION-G0-001 Slice 3 / EOP-PLAN-CONTROLPLANE-GRADUATION-001 v0.3 G0 item 3)
### Purpose: single canonical entry point listing every live governing artifact for the control-plane program. Every future session's Phase 0 grounding reads this file first, before searching `docs/todo/control-plane/`, `docs/research/`, or `docs/architecture/` independently. Two governing docs were invisible to grounding phases in the week before this file existed; this file exists so a third instance is not acceptable.

Status legend: **LIVE** — current authority, read it. **HISTORICAL** — superseded, read only for background/precedent, do not treat as current truth. **RATIFIED** — a design doc whose decision stands, implementation may not have started.

---

## 1. Vision & Scope (top of the document hierarchy)

| Doc | Status | One-line purpose |
|---|---|---|
| `docs/architecture/EOP-VS-CONTROLPLANE-001_Control-Plane_v0.4.2.md` | **LIVE** | The ratified Vision & Scope for governed AI-led execution — the control plane's constitutional document; all plans/designs below trace back to it. |

## 2. Model Conformance Audits (MCA)

| Doc | Status | One-line purpose |
|---|---|---|
| `docs/research/model-conformance-mca-001.md` | **HISTORICAL** | Scoped MCA run (AB1-AB7 conversational boundary), superseded by MCA-002's full run; retained for the arbitration trail that produced v0.4/v0.4.1. |
| `docs/research/model-conformance-mca-002.md` | **LIVE (findings), HISTORICAL (basis version)** | Full §15 topology + mechanism-clause MCA against v0.4/v0.4.1; findings feed T11's plan. Its own basis version (v0.4.1) has since advanced to v0.4.2 — re-check clause numbers against v0.4.2 before citing. |

## 3. Implementation Plan 001 (landed)

| Doc | Status | One-line purpose |
|---|---|---|
| `docs/todo/control-plane/EOP-PLAN-CONTROLPLANE-001_Implementation-Plan_v0.1.md` | **HISTORICAL (landed)** | The original tranche-by-tranche implementation plan (T1-T11 range); its tranches are complete. Still the source of the E1-E5 completion-invariant definitions cited verbatim by the graduation plan. |
| `docs/research/control-plane-phase0-inventory.md` | **LIVE (reference)** | RR-0..RR-6 Phase-0 inventory — the 45-row RR-3 control census that seeded the ownership ledger; the E1/E4 gates' row universe traces here. |
| `docs/research/control-plane-ownership-ledger.md` | **LIVE** | The current-state ledger: every C-0xx row's disposition, updated by every tranche that touches it. The single source of truth for "is this check closed" — more current than any plan doc's prose. |
| `docs/research/control-plane-pir-001.md` | **HISTORICAL** | Adversarial post-implementation review of plan 001's landed tranches (authorship-blind). Findings already folded into the ledger/plan; read for precedent on review method, not for current state. |

## 4. The "002 track" — mesh-retirement / agent-tier extraction (T11.x, parallel scope)

No single dedicated scope-note file exists; the design docs below collectively constitute it. Referenced by the graduation plan (v0.3 §0, §5) as parallel, out-of-scope work that shares `ob-poc-control-plane`/`ob-poc-agent` crate surface with the graduation program — the E5 surface gate is the collision detector between the two tracks.

| Doc | Status | One-line purpose |
|---|---|---|
| `docs/todo/control-plane/EOP-DESIGN-CONTROLPLANE-T11.1a-BOUNDARY-MAP-001.md` | **RATIFIED (2026-07-12)** | Boundary map unblocking T11.1b's mechanical extraction. |
| `docs/todo/control-plane/EOP-DESIGN-CONTROLPLANE-T11.1b-SLICE2-ORCHESTRATOR-SPLIT-001.md` | **RATIFIED (2026-07-12)** | Design for the orchestrator.rs sub-pass of T11.1b slice 2. |
| `docs/todo/control-plane/EOP-TRACE-CONTROLPLANE-T11.1b-SLICE2-ORCHESTRATOR-BOUNDARY-001.md` | **LIVE (trace, no code moved)** | Traced finding that reshaped the T11.1b split from "two piles of functions" to "one minted grant + everything downstream reads it." |
| `docs/todo/control-plane/EOP-DESIGN-CONTROLPLANE-T11.2-CAPABILITY-INVOCATION-001.md` | **DRAFT, awaiting ratification** | CapabilityInvocation design; T11.2 Part B is deferred-until-consumer per its own ruling — nothing in the graduation plan is that consumer. |
| `docs/todo/control-plane/EOP-DESIGN-CONTROLPLANE-T11.F.2-DEFINITIONAL-FLOOR-001.md` | **DRAFT, awaiting architect review** | Definitional-floor design note (T11.F track). |
| `docs/todo/control-plane/EOP-DESIGN-CONTROLPLANE-T9.2-ATOMIC-ADMISSION-001.md` | **APPROVED WITH AMENDMENTS (v0.2, 2026-07-11)** | Atomic-admission design (T9.2); implementation not yet started. |

## 5. Graduation program (current focus — "from shadow-only to genuinely enforcing")

| Doc | Status | One-line purpose |
|---|---|---|
| `docs/todo/control-plane/EOP-RESEARCH-CONTROLPLANE-GRADUATION-001.md` | **LIVE** | Grounding research for the graduation plan; every factual claim in the plan traces to a CONFIRMED finding here. |
| `docs/todo/control-plane/EOP-PLAN-CONTROLPLANE-GRADUATION-001_v0.1.md` | **HISTORICAL (superseded)** | Original graduation plan draft; superseded by v0.3. Kept for the PIR's citation trail. |
| `docs/todo/control-plane/EOP-PIR-CONTROLPLANE-GRADPLAN-001.md` | **LIVE** | Adversarial review of the graduation plan (v0.1); verdict RATIFY-WITH-AMENDMENTS, GRADPLAN-D-001..006 all applied in v0.3. Appended with a v0.3 re-validation addendum (D-010) same day. |
| `docs/todo/control-plane/EOP-PLAN-CONTROLPLANE-GRADUATION-001_v0.3.md` | **LIVE — current authority** | The current graduation plan: tranches G0-G7 + GM (merge, once) + GW (evidence campaign), AD-3 resolved (hold-the-merge), AD-1/AD-2 open. |
| `docs/todo/control-plane/EOP-IMPL-CONTROLPLANE-GRADUATION-G0-001.md` | **LIVE (partially superseded)** | G0/G2-prep implementation slicing against plan v0.1. Slices 1-4 stand as written against v0.3; Slice 5 (merge) is superseded by tranche GM's own preconditions. |
| `docs/todo/control-plane/EOP-RUNBOOK-CONTROLPLANE-GRADUATION-001.md` | **LIVE — v0.3** | The graduation runbook: window definition, per-path readiness, graduation order/procedure, rollback, triage classification. Standing authority for procedure; the plan sequences engineering to make this runbook executable. |
| `docs/todo/workspace-hygiene-001.md` | **LIVE (open)** | Pre-existing, control-plane-independent workspace hygiene failures (ob-poc-boundary golden-count drift, dsl-runtime doctest). Explicitly out of control-plane completion scope per PIR-D-006. |

## 6. Invariant enforcement machinery

| Artifact | Status | One-line purpose |
|---|---|---|
| `docs/todo/control-plane/EOP-SESSION-CONTROLPLANE-INVARIANT-PROMOTION-001.md` | **LIVE — COMPLETE (2026-07-13)** | Session that made E1-E5 executable CI gates (not prose claims). All 6 phases landed. |
| `docs/todo/control-plane/EOP-SESSION-CONTROLPLANE-INVARIANT-PROMOTION-001-evidence-2026-07-13.txt` | **LIVE (evidence artifact)** | Verbatim `check-invariants.sh all` output captured at session close. |
| `invariants-expected.toml` (repo root) | **LIVE** | Per-invariant expected status + detail comments; architect-reviewed flips only — a session recommends a flip in its own doc, does not apply it here unless it earned it in the same diff. |
| `scripts/check-invariants.sh` (repo root) | **LIVE** | The gate script — `e1`..`e5` + `all`, machine-checkable per the invariant-promotion session's own discipline. |
| `.github/workflows/invariants.yml` (repo root) | **LIVE** | CI wiring for the above. |

---

## Not listed here (deliberately)

- Design docs under `docs/todo/control-plane/` for future graduation sub-tranches (e.g. `EOP-DESIGN-CONTROLPLANE-G1-SEAL-CONSUME-001`) that do not exist yet — they will be added to §5 when created.
- `ai-thoughts/` — historical background notes, not authoritative per `CLAUDE.md`'s own standing rule; none are control-plane-specific as of this file's creation.
