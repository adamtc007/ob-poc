# Instrument Matrix DAG — IM-Mandate Sanity Review, Pass 2 (2026-04-23)

> **Context.** Pass 1 (in `instrument-matrix-dag-workspace-diagram-2026-04-23.md` §6)
> caught obvious gaps: counterparty, investment guidelines, mandate-amendment
> branching, ISDA, `superseded` state, terminology. Adam pushed back: "did you
> apply IM-mandate knowledge *independently*, not just restate the inventory?"
>
> Fair challenge. Pass 1 was partial. This pass is a second independent walk,
> asking "what would a real IM operations team say is missing, that pass 1
> didn't flag hard enough?"
>
> **Purpose:** record second-pass findings cleanly so Adam can triage.
> Structure: gaps I should have flagged (§2), state-machine refinements within
> existing slots (§3), triage recommendations (§4).

---

## 1. What pass 1 got right

For completeness — this pass doesn't re-litigate these:

- Counterparty approval (G-1) — resolved: cross-workspace reference.
- Investment guidelines (G-2) — resolved: CBU-level attributes.
- ISDA as read-only projection from legal-ops — flagged.
- `superseded` state on trading_profile_lifecycle — flagged.
- Mandate amendment branching (minor/material/urgent) — flagged.
- Hybrid persistence pattern for document-shaped slots — captured.
- Terminology: trading_profile → UI label "Mandate" — captured.

Good findings. What follows is what I should have ALSO flagged.

---

## 2. Structural gaps — new slots that IM operations would expect

Four candidates, ordered by how firmly I'd argue they belong in
Instrument Matrix vs. sitting in another workspace.

### S-1 (HIGH): Reconciliation slot

**IM reality.** Reconciliation is foundational to IM operations. Every
mandate has at least three recon streams running continuously:
- **Position recon** — IM's book of record vs. custodian's SOR.
- **Cash recon** — IM's cash ledger vs. nostro account statements.
- **NAV recon** — IM calc vs. administrator calc (for funds).

Each recon has a state machine:
`scheduled → running → matched | breaks_found → breaks_investigating → resolved | escalated`.

Breaks have incident-ticket-like lifecycle (new → assigned → investigating
→ resolved / written-off).

**Current IM model handles this how?** Not at all. `delivery` models
discrete service events, not recon. `matrix-overlay` is unrelated.

**Options:**
- (a) Add a `reconciliation` slot with state machine to Instrument
  Matrix. Real operational scope.
- (b) Keep out of IM — recon belongs in a separate "operations" or
  "middle-office" workspace. Reference only.
- (c) Hybrid — the CONFIGURATION of what-gets-reconciled lives on the
  mandate; the EVENTS live elsewhere. Similar to corporate_action_policy
  (config on mandate) vs CA events (separate).

**My recommendation:** (c). The recon configuration (which SoR to
compare against, which tolerance thresholds, which escalation path)
is mandate-config. The recon events + breaks are a separate workspace
surface. But IM should know `reconciliation_config` exists as a slot
— currently it doesn't.

**Ask for Adam:** is recon config a part of the mandate you care about
modeling, or is it purely middle-office and outside IM pilot scope?

---

### S-2 (HIGH): Corporate Action EVENT lifecycle (distinct from POLICY)

**Pass 1 flaw.** I conflated "corporate action policy" (the mandate's
config for how to handle CAs) with the CA event itself. They're
different animals.

**IM reality.** A CA event — say, an AAPL 4:1 split — hits a mandate
and flows through:
```
announced → impact_assessed → response_due → election_pending
  → elected → processed → settled → reconciled
```

With branches:
- `response_due` → `default_applied` if client doesn't respond by cut-off.
- `election_pending` → `amended` if election changes before cut-off.
- Any state → `failed` or `disputed` for ops issues.

**Current IM model handles this how?** `corporate_action_policy` slot
(declare-stateless in A-1 v3) handles the POLICY config. The pack has
`corporate-action.define-event-type`, `.set-preferences`, etc. — all
policy verbs. The EVENT lifecycle is not modeled.

**Where does the CA event live?** It's a live operational entity that
hits many mandates simultaneously (one event → N mandates) and needs
ops tracking per-mandate. That's a genuine missing slot.

**Options:**
- (a) Add a `corporate_action_event` slot to Instrument Matrix (scoped
  per-mandate-instance of an event).
- (b) Own the event in a separate CA-processing workspace; Instrument
  Matrix references.
- (c) Partial — the per-mandate election state lives here; the global
  event data lives elsewhere.

**My recommendation:** (c). Per-mandate election + election-window
tracking belongs in IM (it's mandate-specific state). Global CA event
data (the split ratio, the record date) is reference data from an
external feed.

**Ask for Adam:** how does ob-poc currently handle CA events that hit
a mandate? Is there any event-tracking surface today?

---

### S-3 (MEDIUM): Margin / Collateral management slot

**IM reality.** For any mandate that can trade derivatives (which is
most institutional mandates), margin and collateral are first-class
operational concerns:
- **Initial margin** (IM) — collateral posted at trade inception.
- **Variation margin** (VM) — daily mark-to-market flows.
- **Threshold / MTA** (minimum transfer amount) — CSA-defined.
- **Triparty collateral** — held with a triparty agent (e.g. BNY).
- **Collateral schedule** — what's eligible as collateral per CSA.

State machine for a collateral arrangement:
```
proposed → csa_negotiated → collateral_pledged
  → active → margin_call_pending → posted
  → (reconciled | disputed) → terminated
```

**Current IM model.** `isda_framework` covers the legal CSA config
(`.add-csa`, `.add-csa-collateral`, `.link-csa-ssi`). But the operational
margin flow isn't modeled. The `csa-config` is a static rule, not a
lifecycle entity.

**This is a real gap** but ONLY for derivative-trading mandates. For
long-only equity mandates, no CSA → no margin → slot doesn't apply.

**Options:**
- (a) Add a `collateral_management` slot (optional per-mandate).
- (b) Keep out of IM pilot — model when the platform needs derivatives.

**My recommendation:** (b) for pilot. Instrument Matrix pilot is
trading-profile-centric; collateral management is a specialisation
that only applies to derivative-enabled mandates. Flag as estate-scale
follow-up for when the platform needs to service a PB'd hedge fund
mandate or similar.

**Ask for Adam:** are derivative mandates in pilot scope, or
equity/fixed-income long-only? If derivatives are in scope, S-3 escalates
to HIGH.

---

### S-4 (MEDIUM): Exception / break handling slot

**IM reality.** Every trading day generates exceptions:
- Settlement fails (trade didn't settle on SD).
- Unmatched trades (counterparty break).
- Pricing exceptions (stale price, failed source).
- Allocation breaks (block order didn't allocate cleanly).
- CA misses (event not picked up in time).

Each has incident-ticket-like lifecycle:
`raised → triaged → assigned → investigating → resolved | escalated | written_off`.

Ops teams spend huge fractions of their day here. Missing from IM.

**Current IM model.** `delivery.fail` hints at it but isn't a full
exception-management slot.

**Options:**
- (a) Dedicated `exception_management` slot in IM.
- (b) Separate workspace (middle-office ops).
- (c) Leverage the existing ticketing surface (if one exists).

**My recommendation:** (b). Exception handling is cross-mandate, cross-
workspace ops work. IM should reference the exceptions that affect its
mandates but not own the slot. Similar conclusion to reconciliation
(S-1) and CA events (S-2) — there's a consistent pattern emerging: IM
owns CONFIG; a separate workspace owns EVENTS / INCIDENTS.

**Ask for Adam:** does ob-poc have an ops-ticket / incident surface
planned anywhere? If so, IM references it; if not, is that planned for
a later tranche?

---

## 3. State-machine refinements within existing slots

Second-pass refinements I should have caught. Smaller than S-1–S-4;
more "did we get the intermediate states right?"

### R-1: `trading_profile_lifecycle` — missing `parallel_run` / `pilot` state

**IM reality.** Between `approved` and `active`, many IMs run a mandate
in **parallel-run** mode for 1–2 settlement cycles — real settlements,
shadow portfolio, no client impact. This catches configuration bugs
before full activation.

**Proposed refinement:**
```
approved → parallel_run → active
```
with `parallel_run → approved` if issues found (rollback).

**Impact:** adds one state, two transitions. Low-cost. Matches real
ops practice.

### R-2: `settlement_pattern` — missing `parallel_run` + supersede semantic

**IM reality.**
- Between `reviewed` and `live`, chains run in `parallel_run` alongside
  the old chain for 1–2 settlement cycles (same pattern as R-1).
- Chains are rarely just "deactivated" — they're REPLACED by a new
  chain that supersedes them. This is a graph relationship, not a
  state: `chain_A.superseded_by = chain_B.id`.

**Proposed refinements:**
- Add `parallel_run` state between `reviewed` and `live`.
- Add `supersede` relationship (not a state — a reference attribute)
  so ops can trace "chain A was replaced by chain B on 2026-04-12."

### R-3: `trade_gateway` — missing `uat` / `fix_cert` intermediate state

**IM reality.** Between `enabled` (config complete on IM side) and
`active` (session live), brokers/exchanges run UAT + FIX certification.
This is typically 1–3 weeks of testing before go-live.

**Proposed refinement:**
```
enabled → uat_testing → fix_certified → active
```

May be overkill for pilot; could collapse to single `uat` state.

### R-4: `cbu.status` — missing `actively_trading` distinction

**IM reality.** `VALIDATED` means "we've validated the CBU data."
`ACTIVELY_TRADING` means "there's at least one approved mandate active
against this CBU, and trades are flowing." These are meaningfully
different operational states:
- `VALIDATED` but not actively trading = onboarded but dormant (fund
  not yet launched, mandate pending approval).
- `ACTIVELY_TRADING` = in live ops.

**Proposed refinement:** Either add `ACTIVELY_TRADING` as a 6th state
(now 8 with Q7's SUSPENDED + ARCHIVED), OR treat "actively-trading"
as a derived attribute (query: "CBU has ≥ 1 trading_profile.active"?).

**My lean:** derived attribute, not a state. Keeps the state set tight.

### R-5: `corporate_action_policy` stateless — confirm it stays so after S-2

If S-2 (CA EVENT lifecycle) gets added as a new slot, the POLICY slot
stays stateless (it's static config). If S-2 is deferred to another
workspace, consider whether the POLICY itself has approval workflow
(changes to CA election rules are usually material amendments — G-5
mandate-amendment territory).

### R-6: `delivery` lifecycle — partial vs complete

Real delivery events have partial-settlement: `100 of 1000 shares
delivered Day 1; 900 delivered Day 2`. The current `PENDING → IN_PROGRESS
→ DELIVERED` doesn't model partial settlement. Proposed:
```
PENDING → IN_PROGRESS → PARTIALLY_DELIVERED → DELIVERED
                                          → FAILED
```

Low priority — partial settlement is common for derivatives less so
for equities. Depends on S-3 answer.

---

## 4. Triage recommendations

| Finding | Pilot scope? | Notes |
|---|---|---|
| **S-1 Reconciliation** (recon config on mandate) | **ADD to IM** as a config slot (hybrid (c)) | Real gap; operational IM reality |
| **S-2 Corporate Action Event** (per-mandate event tracking) | **ASK Adam** | Need to know how CA events are surfaced today |
| **S-3 Margin / Collateral** | **DEFER** unless pilot includes derivative mandates | Depends on derivative scope |
| **S-4 Exception handling** | **DEFER** to ops workspace | Not pilot scope; IM references |
| **R-1 trading_profile parallel_run** | **ADD in P.2** | Low cost, matches real IM |
| **R-2 settlement_pattern parallel_run + supersede** | **ADD in P.2** | `parallel_run` state + supersede attribute |
| **R-3 trade_gateway UAT intermediate** | **OPTIONAL in P.2** | Could collapse or defer |
| **R-4 CBU actively_trading distinction** | **DERIVED attribute, not state** | Keep state set tight |
| **R-5 CA policy stays stateless** | **YES** | Contingent on S-2 |
| **R-6 delivery partial settlement** | **DEFER** unless derivatives in scope | Depends on S-3 |

---

## 5. Honest self-assessment

Pass 1 covered terminology, obvious structural gaps (counterparty,
investment guidelines), and the mandate-lifecycle `superseded` question.
These are real findings.

What pass 1 missed: I stayed surface-level on **operational lifecycle**
— the event-flow states that dominate IM ops-team daily work.
Reconciliation, corporate-action events, margin calls, exceptions —
these ARE the mandate in practice, and I treated them as out-of-scope
without really asking "should IM own the config, the event, both,
neither?"

I also missed the **parallel-run / UAT** pattern that shows up in
almost every operational-config state machine in IM. It's the
universal "we don't trust the new config yet — run it in shadow mode
for N cycles" step, present in settlement chains, gateways, new
mandates.

Pass 2 is more grounded in what an IM ops team would see as the
real model. Pass 1 was a config-centric view; pass 2 adds the
event/incident-centric view and refines the config states.

---

## 6. Questions for Adam

**Q-A: Recon config on mandate** — add `reconciliation_config` slot
to IM (hybrid-persisted), or keep out of IM pilot?

**Q-B: Corporate Action events** — where are per-mandate CA events
tracked today (if anywhere)? Should IM own a `corporate_action_event`
slot for per-mandate election + tracking state?

**Q-C: Derivatives in pilot scope?** — if yes, S-3 (margin/collateral)
and R-6 (partial delivery) escalate. If no (long-only equity/FI
focus), both defer.

**Q-D: Exception/incident surface** — does ob-poc plan an ops-ticket
surface for settlement fails / unmatched trades / etc.? If yes, IM
references. If no, note as estate-scale follow-up.

**Q-E: Parallel-run state** — add to P.2 authoring for
`trading_profile` and `settlement_pattern`? Low cost; matches real
IM practice.

**Q-F: Trade-gateway UAT state** — add or collapse into `enabled`?

---

**End of pass 2.** The first-pass doc stays authoritative for its
findings (all confirmed); this doc adds what a deeper domain walk
surfaces that pass 1 didn't flag hard enough.
