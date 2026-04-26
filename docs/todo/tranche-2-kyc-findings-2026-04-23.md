# Tranche 2 — KYC Findings Report (2026-04-23)

> **Status:** KYC CLOSED. Second workspace through the pilot pattern.
> **Parent docs:**
> - `instrument-matrix-pilot-findings-2026-04-23.md` (pilot baseline)
> - `tranche-2-kyc-kickoff-2026-04-23.md` (kickoff plan)
> - `catalogue-platform-refinement-v1_2.md` (spec)

---

## 1. Delivery summary

9 phases in the pilot-mirror pattern, executed in ~90 minutes of
focused work vs. the kickoff estimate of 2-3 hours. Pilot
infrastructure fully reused:

| Phase | Deliverable | Effort actual |
|---|---|---|
| T2-K-1 (equiv P.1) | SKIP — infra reused | 0 |
| T2-K-2-prep | A-2-kyc pack delta (1 FQN pruned) | 5 min |
| T2-K-2 | `kyc_dag.yaml` (1.1k LOC: 8-phase overall lifecycle + 13 slot state machines + 7 cross-slot constraints + prune rules) | ~25 min |
| T2-K-3 | 142 three-axis declarations via retrofit script (97 KYC + 45 adjacent) | ~5 min |
| T2-K-4 | 4 tier raises (ubo-registry.promote, tollgate.override, screening.sanctions, screening.pep) | ~5 min |
| T2-K-5 | Runtime triage: **72/72 Bucket 1 (100%)** | ~10 min |
| T2-K-6/7/8 | SKIP — infra/tools reused; DB-free smoke + reconcile CLI + catalogue workspace prototype all covered by pilot | 0 |
| T2-K-9 | This document | ~5 min |

**Validator terminal state:** 390 / 1184 declared, 0 structural errors,
0 well-formedness errors, 0 warnings.

**Net declarations added this session**: +145 (from post-pilot's 245).

---

## 2. What the pilot infrastructure delivered (confirmed at 2× scale)

Validated at 2 workspaces now:

- **Schema + validator + composition engine** (dsl-core/config/): drop-in
  reuse. 0 new code required for KYC.
- **Pattern-based retrofit script** (/tmp/t2k3_retrofit.py): declared 142
  verbs in ~5 min. The classifier patterns held — only 4 tier raises
  needed post-retrofit.
- **Pack-hygiene check** (V1.2-5): caught 1 stale FQN at Tranche 2
  entry (kyc.delete-case). Cost: 1 sed + commit. Prevented drift from
  accumulating.
- **Triage script**: pack-scope filter → direct bucket categorisation.
  No code changes.

**This is the v1.2-10 effort-estimate-revision validating.** The
one-time infrastructure from the pilot amortized cleanly across a
second workspace. The main cost was DAG taxonomy authoring, which is
the inherently workspace-specific part.

---

## 3. KYC-specific findings (v1.3 candidates)

Four findings surfaced during KYC Tranche 2 that aren't in v1.2 and
warrant v1.3 consideration:

### V1.3-CAND-1 — UBO epistemic-vs-fact state pattern

UBO determination has TWO interleaved state machines:
- **kyc_ubo_registry** (9 states) — the *epistemic* state of our knowledge
  about a candidate UBO (CANDIDATE → IDENTIFIED → PROVABLE → PROVED →
  REVIEWED → APPROVED/WAIVED/REJECTED/EXPIRED).
- **entity_ubos** (categorical, no lifecycle) — the factual UBO record
  itself, once promoted.

This epistemic-vs-fact distinction is subtly different from v1.2's
state-machine model (which assumes one entity → one lifecycle). Worth
formalising as a v1.3 architectural pattern: some domain entities have
separate **knowledge state** and **fact state**, linked but independent.

**Sources:** KYC T2-K-2 §2.2 kyc_ubo_registry slot.

### V1.3-CAND-2 — Cross-workspace state dependencies

KYC produces a decision (kyc_decisions.status = CLEARED) that's a
**precondition** for Instrument Matrix mandate activation. This is a
cross-workspace read-only dependency that v1.2's `cross_slot_constraints`
pattern wasn't designed for (v1.2 constraints are intra-DAG; this is
across DAGs).

Proposed v1.3 amendment: add `cross_workspace_constraints:` as a new
section in DAG taxonomy YAML, referencing (this-workspace-slot,
other-workspace-slot, rule) tuples. Enables validator to catch bugs
like "Instrument Matrix trading_profile.activate requested without
KYC decision in place."

**Sources:** KYC T2-K-2 §5 out-of-scope entries.

### V1.3-CAND-3 — Periodic review cadence modelling

KYC cases have calendar-driven re-review obligations (annual for
high-risk, longer for low-risk). The pilot's onboarding-centric
lifecycle doesn't capture cadence — the `remediation` phase models
the re-review *once it starts*, but not the scheduling.

Proposed v1.3 amendment: add `periodic_review_cadence:` to slot
declarations for entities where re-review is regulatory. Schema
extension + scheduler integration (the scheduler itself is Layer 3,
but the policy declaration is DAG-level).

**Sources:** KYC T2-K-2 §1 overall_lifecycle phase `remediation`.

### V1.3-CAND-4 — Remediation workflow distinctness

v1.2 amendment V1.2-4 (prune semantics) is about deletion. Remediation
is the opposite: additive corrections without reverting case phases.
Example: adding a missing UBO after `decided` without reopening the
whole case.

Proposed v1.3: add `remediation_operation:` marker on verbs that
apply mid-lifecycle corrections. Distinct from `prune` (destructive)
and `amend` (full-lifecycle-rework per pass-7 Q-CD).

**Sources:** KYC T2-K-2 §1 overall_lifecycle phase `remediation`.

---

## 4. Pilot + KYC combined — cumulative v1.2 amendment progress

Consolidated against the v1.2 amendment list:

| # | Amendment | Status after KYC |
|---|---|---|
| V1.2-1 | P16 three-layer architecture | DOC (spec §1) |
| V1.2-2 | `overall_lifecycle:` first-class section | LANDED (used by both IM + KYC DAG YAMLs) |
| V1.2-3 | `requires_products:` conditional reachability | LANDED (field present in schema + KYC uses it for kyc_service_agreement) |
| V1.2-4 | Prune semantics as general pattern | LANDED (IM + KYC both author prune_cascade_rules) |
| V1.2-5 | PackFqnWithoutDeclaration validator | LANDED + wired into 3 runtime tools |
| V1.2-6 | §4.1 factual update | DOC |
| V1.2-7 | Borderline-operational-slot pattern | DOC (pattern documented; not enforced) |
| V1.2-8 | DSL over-modeling lint | DEFERRED |
| V1.2-9 | Sem-os scanner for `dag_taxonomies/` | DEFERRED |
| V1.2-10 | Estate-scale effort revision | VALIDATED (1 of 4 workspaces through; pattern holds) |

7 of 10 v1.2 amendments now materially landed (code or doc). 2 deferred
(V1.2-8 lint, V1.2-9 scanner) — both non-critical; one (V1.2-6 is just
a factual update).

4 new v1.3 candidates surfaced (§3 above).

---

## 5. Tranche 2 status — estate-scale progress

Tranche 2 progress (4 primary workspaces):

| Workspace | Status | Declared verbs | Pack size | Notes |
|---|---|---|---|---|
| **Instrument Matrix** | ✅ CLOSED (pilot) | 248 | 186 post-prune | First workspace |
| **KYC** | ✅ CLOSED (this session) | 142 (97 + 45 adjacent) | 100 post-prune | Second workspace |
| **Deal** | ⏳ pending | — | ~40 (post-prune) | Third target |
| **CBU** | ⏳ pending | — | — | Fourth target |

Other non-primary workspaces with pack files: book-setup, onboarding-
request, product-service-taxonomy, semos-maintenance, session-bootstrap.

**Estimated remaining Tranche 2 effort** (based on 2-workspace
empirical):
- Deal: smaller scope (~40 verbs), probably 30-45 min
- CBU: medium scope, probably 1-1.5 hours
- Cross-workspace reconciliation pass: 1 hour (when all 4 primary
  workspaces declared)
- **Total remaining Tranche 2: 2-3 hours focused work**

Dramatically under v1.0's original multi-month estimate. V1.2-10's
effort-revision ("4-6 calendar weeks") is turning out generous; pure
engineering authoring is compressing further. Governance review
coordination (real P-G) remains the estate-scale long pole.

---

## 6. Closure

**KYC Tranche 2 CLOSED.** 9 phases delivered. 100% runtime-triage
alignment. 4 new v1.3 candidates captured.

Next: Deal workspace or CBU.

**T2-K-9 end.**
