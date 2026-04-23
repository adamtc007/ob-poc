# Tranche 2 — KYC Workspace Kickoff (2026-04-23)

> **Status:** kickoff. Mirrors the Instrument Matrix pilot pattern
> (P.1 → P.9). Phase P.1 infrastructure is already landed — all
> Tranche-2 workspaces reuse it. KYC is the **second** workspace
> through this pattern.
>
> **Parent docs:**
> - `instrument-matrix-pilot-findings-2026-04-23.md` (closed pilot)
> - `catalogue-platform-refinement-v1_2.md` (architectural baseline)
> - 7 sanity review passes for the IM pilot
>
> **Authority:** Adam-as-Tranche-2-authority (same pilot convention
> per v1.1 §13). P-G for estate-wide sign-off remains a separate
> organisational requirement.

---

## 1. Scope vs. pilot

| Dimension | IM pilot | KYC Tranche 2 | Ratio |
|---|---|---|---|
| Pack FQNs | 186 (post-prune) | **102** | 0.55× |
| Verb files | 19 | **16** | 0.84× |
| Verbs to declare | ~176 | **~96** | 0.55× |
| Pre-existing state machines | 1 (`trading_profile_lifecycle`) | **2** (`kyc_case_lifecycle` + `entity_kyc_lifecycle`) | 2× |
| Schema-backed CHECK constraints | ~3 enum columns | **6+** across 11 tables | 2× |
| Constellation maps | 3 | 3 | 1× |
| Rust SemOsVerbOp impls | 39 | **5** | 0.13× |
| Test utterances | 24 | **28** | 1.17× |

**Net:** KYC is **smaller in verb count** but **richer in pre-existing
state-model density**. The pilot infrastructure (schema, validator,
composition engine, startup gate, lint, reconcile CLI) carries forward
intact; what changes is the per-workspace authoring of DAG taxonomy +
three-axis declarations.

---

## 2. Phase map (mirror of IM pilot)

Using the same 9-phase structure, with phase content adapted to KYC:

| Phase | Scope | Key deliverable |
|---|---|---|
| T2-K-1 (equiv P.1) | **Skip** — infra already landed | Reuse pilot's schema, validator, composition engine, startup gate, lint, reconcile CLI |
| T2-K-2-prep | Pre-phase audits (A-1-kyc + A-2-kyc) | Slot inventory with classification + pack-delta ledger |
| T2-K-2 (equiv P.2) | KYC DAG taxonomy YAML | `rust/config/sem_os_seeds/dag_taxonomies/kyc_dag.yaml` — lift existing state machines, add overall lifecycle, cross-slot constraints |
| T2-K-3 (equiv P.3) | Three-axis declarations on ~96 verbs | Retrofit + new-verb authoring |
| T2-K-4 (equiv P.4) | Tier review under Adam-authority | Decision ledger + applied changes + escalation rules |
| T2-K-5 (equiv P.5) | Runtime triage | 28 KYC utterances → Bucket 1/2/3 |
| T2-K-6 (equiv P.6) | DB-free validation | Already covered — the smoke test reads the full catalogue |
| T2-K-7 (equiv P.7) | Reconcile CLI | Already covers KYC (shares validator) |
| T2-K-8 (equiv P.8) | Catalogue workspace | Already has skeleton from pilot; extends naturally |
| T2-K-9 (equiv P.9) | Findings report | Per-workspace empirical data + v1.2 refinements |

**Expected deliverables LOC:** similar in shape to IM pilot but ~55%
smaller in raw verb count, offset by the 2 pre-existing state machines
that need lifting into formal DAG YAML (more reconcile-existing work).

---

## 3. Pre-phase audits — T2-K-2-prep

### 3.1 A-1-kyc slot inventory

Structure matches IM's A-1 v3. For each slot in the 3 KYC constellation
maps (`kyc_workspace.yaml`, `kyc_extended.yaml`, `kyc_onboarding.yaml`):

Mandatory classification per slot:
- `declare-stateless` — compositional projection, no lifecycle
- `new-state-machine` — needs formal taxonomy YAML, state count proposed
- `reconcile-existing` — lift from existing state machine into formal
  DAG taxonomy section

Already-declared state machines (direct pilot pattern match — these
are `reconcile-existing`):
- `kyc_case_lifecycle` — 11 states already formalized in
  `state_machines/kyc_case_lifecycle.yaml`
- `entity_kyc_lifecycle` — 8 states formalized
- `ubo_epistemic_lifecycle` — inspected name; likely UBO-specific

**Expected output:** ~30-40 slot rows (guess based on IM's 21, scaled
by constellation richness; KYC's `kyc_workspace.yaml` has 81 slot
definitions — some will collapse to single classification categories).

### 3.2 A-2-kyc pack-delta

Pack has 102 FQNs across 16 prefixes. Of those, 6 are likely cross-
workspace (`ownership`, `entity`, and some `party.*` that may live in
CBU/IM workspaces). Disposition:

- (a) formalise-yaml — missing declarations to author
- (b) remove-from-pack — stale entries
- (c) legitimate-cross-ref — declared in another workspace

**Expected result:** small delta (<10 FQNs) given V1.2-5 pack-hygiene
already enforces consistency. Most will be (c) cross-references.

### 3.3 Effort estimate for prep

Per the revised estate-scale estimate in v1.2 §V1.2-10:
- A-1-kyc: ~4 hours (domain-dense, 2 pre-existing state machines to
  reconcile)
- A-2-kyc: ~30 minutes (V1.2-5 already enforces, this is categorizing)
- Total prep: ~0.5 day

---

## 4. T2-K-2 (P.2-equivalent) — KYC DAG taxonomy

**Deliverable:** `rust/config/sem_os_seeds/dag_taxonomies/kyc_dag.yaml`

Sections (same as IM's instrument_matrix_dag.yaml):

### 4.1 Overall lifecycle (per CBU-KYC-case)

Aggregate 6-phase lifecycle drawing on:
- `cases.status` (INTAKE → DISCOVERY → ASSESSMENT → REVIEW → terminal)
- `entity_workstreams.status`
- `screenings.status`
- `kyc_decisions.status`

Proposed phases (draft for Adam's review):
1. `case_opened` — case in INTAKE, no workstreams yet
2. `discovery` — entity workstreams created, evidence collection
3. `screening_in_flight` — screenings running; red-flags tracked
4. `assessment` — risk-rating set; UBO chain finalized
5. `review_in_progress` — committee / compliance sign-off
6. `decided` — APPROVED / REJECTED / WITHDRAWN / REFER_TO_REGULATOR
7. `remediation` — follow-up obligations from periodic reviews
8. `closed / archived` — terminal

### 4.2 Per-slot state machines

- `kyc_case` — reconcile-existing (lift 11 states from
  `kyc_case_lifecycle.yaml`)
- `entity_kyc` — reconcile-existing (lift 8 states)
- `screening` — new-state-machine (from `screenings.status` CHECK:
  PENDING, RUNNING, CLEAR, HIT_PENDING_REVIEW, HIT_CONFIRMED,
  HIT_DISMISSED, ERROR, EXPIRED)
- `ubo_evidence` — new-state-machine (PENDING, VERIFIED, REJECTED,
  EXPIRED)
- `kyc_decision` — new-state-machine (CLEARED, REJECTED, CONDITIONAL,
  PENDING_REVIEW)
- `kyc_service_agreement` — new-state-machine (currently ACTIVE
  default, no CHECK — will propose: DRAFT, ACTIVE, SUSPENDED,
  TERMINATED)
- `entity_workstream` — new-state-machine (PENDING default, CHECK
  TBD)
- `tollgate` — new-state-machine (drawn from verb behaviors —
  evaluate, override, waive, approve)
- `red_flag` — new-state-machine (raise → resolved / escalated)
- `request` (doc-request) — new-state-machine (per request.yaml verbs)
- ...plus compositional (stateless) slots for `kyc_workstream`,
  `partnership`, `coverage`, `skeleton_build`

### 4.3 Cross-slot constraints

- `case` cannot → APPROVED without all `entity_workstream` rows =
  approved/verified AND all `screenings` = CLEAR OR HIT_DISMISSED.
- `kyc_decision` requires case.status = REVIEW + risk-rating set.
- `ubo_evidence` verification requires underlying `party` + `entity`
  data.

### 4.4 Product-module gates (from V1.2-3)

KYC is cross-product — applies to every CBU regardless of their
product bundle. Different from IM where some slots are product-gated.
Most KYC slots are `always_on` per the pass-4 product-module map.
Exceptions (product-conditional):
- `kyc_service_agreement` gated by KYC-as-a-service product
  (`product.kyc_service`) — this is the sponsor-client KYC
  arrangement.

### 4.5 Prune operations

KYC has fewer prune cases than IM:
- `kyc.prune-party` — remove a party from a case's scope (rare;
  typically case is re-opened with corrected scope instead).
- `kyc.prune-workstream` — remove an entity workstream (requires
  re-scoping the case).

Most KYC work is additive (add parties, add evidence, add screenings).
Prune semantics still apply but with smaller surface than IM's asset-
family granularity.

---

## 5. T2-K-3 (P.3-equivalent) — three-axis declarations

Apply the same pattern-based retrofit script used in P.3.b:
- READ / list / lookup → preserving + [observational] + benign
- CREATE / define / add / link → preserving + [] + reviewable
- ACTIVATE / approve → preserving + [emitting] + reviewable
- SUSPEND / resume / cancel → preserving + [emitting] + requires_confirmation
- REMOVE / delete → preserving + [] + requires_confirmation
- TERMINATE / retire / reject → preserving + [emitting] + requires_explicit_authorisation
- PRUNE → preserving + [] + requires_explicit_authorisation

KYC-specific tier considerations (flag for T2-K-4):
- `kyc.submit-case` / `case.approve` / `case.reject` → likely
  requires_confirmation baseline (compliance decision)
- `screening-ops.run` / `screening.hit-confirmed` → emitting + higher
  tier (real regulatory signal)
- `ubo-registry.promote-to-ubo` → requires_explicit_authorisation
  (formal UBO declaration)
- `red-flag.raise` / `red-flag.escalate` → emitting + reviewable or
  requires_confirmation
- `tollgate.override` / `tollgate.waive` → requires_explicit_authorisation
  (overrides are high-consequence by nature)

Estimated: ~96 verbs × 30s/verb pattern-match = ~50 min mechanical work
+ 10-15 flagged cases for Adam tier review.

---

## 6. Decision required — proceed vs. refine

**Option A — Execute the full Tranche-2-KYC phases in one session**
(estimated 2-3 hours of focused authoring, 15-20 min of Adam decisions
throughout).

**Option B — Execute just T2-K-2-prep now** (slot inventory + pack
delta audits), produce the T2-K-2 DAG taxonomy plan, and pause for
Adam review before authoring.

**Option C — Further refine this kickoff plan** before any
implementation.

**Recommendation:** **Option A — full execution.** The pilot pattern
is established; KYC is smaller in scope; pre-existing state machines
reduce authoring load. The main domain-specific decisions are the tier
cases flagged in §5 — those become the T2-K-4 review ledger, not
blockers for the mechanical phases.

---

## 7. What could surface that's NOT in the pilot pattern

Things I'd expect to flag as Tranche-2 refinements to v1.2:

- **UBO lifecycle distinctiveness** — UBO evidence is epistemic
  (different from ownership facts). Current `ubo_epistemic_lifecycle`
  exists but is less formalized than kyc_case_lifecycle. May need
  a new architectural pattern for "evidence-state-vs-fact-state".
- **Cross-workspace handoffs** — KYC produces a decision that
  Instrument Matrix consumes (a CBU can't have a mandate approved
  without KYC clearance). This is a cross-workspace state dependency
  that v1.2's cross_slot_constraints pattern wasn't designed for.
- **Periodic review cadence** — KYC cases have re-review obligations
  that operate on a calendar. The pilot's onboarding-centric lifecycle
  doesn't capture this. Likely a new v1.3 amendment.
- **Remediation workflow** — distinct from case re-opening; a mid-
  lifecycle intervention that amends the case without reverting to
  earlier phases.

These are v1.3 candidates for findings, not pilot-pattern gaps.

---

## 8. Next step

Await Adam's A/B/C choice on §6. If A (recommended), begin T2-K-2-prep
immediately and continue through the 9 phases in one session.
