# Tier-Assignment Authority — Provisional Designation (P-G)

> **Status:** Active for the v1.2 full-estate reconciliation activity.
> **Spec reference:** v1.2 §13 prerequisite P-G — "documented provisional designation".
> **Resolves:** Tranche 1 DoD item 11.
> **Date:** 2026-04-26.

## Purpose

This document satisfies v1.2 §13 P-G as a **documented provisional named authority** for
tier-assignment governance during the current Catalogue Platform Refinement activity.

The v1.2 §13 amendment carved out a path for provisional designation: a named individual
or body with the three preconditions below satisfies "named authority" without requiring
organisational delegation upfront. Organisational P-G remains the eventual destination
and is recorded as an open governance question.

## §13 three preconditions

v1.2 §13 (lines 739-745) admits documented provisional designation provided:

1. **The named individual or body is explicitly identified.** ✓ See "Named authority"
   below.
2. **The provisional nature is documented in the activity's governance record.** ✓
   This document, the activity prompt, and v1.2 §13 itself all record the provisional
   framing.
3. **The provisional designation is treated as an interim measure, with an organisational
   P-G as the eventual destination, recorded as an open governance question for the
   platform.** ✓ See "Open organisational P-G question" below.

## Named authority

**Adam, in the role of architectural authority for the activity.**

Adam is named as the tier-assignment authority for the v1.2 full-estate reconciliation
activity — specifically R.5 (Tier governance pass under provisional Adam-as-authority,
v1.2 Phase 2.C) and R.8 (post-reconciliation coherence pass, v1.2 Phase 2.G).

Authority scope:
- All tier baseline assignments across the ~1,282-verb estate.
- All escalation rule additions / amendments during R.5.
- All tier disambiguation decisions where the cluster review surfaces ambiguity.
- Phase 2.G coherence findings — accept, defer, or amend.

Authority limits:
- Does not extend beyond the current reconciliation activity (Tranches 1 + 2). Tranche 3
  governance is a separate scope.
- Does not commit organisational policy or budget — purely architectural.
- All decisions are explicitly revisable under future organisational P-G.

## Provisional nature documentation

The "provisional" qualifier is not cosmetic. It binds the following:

- **Decisions stand until reviewed by organisational P-G.** When organisational P-G is
  established, its first task is to review the audit trail of provisional decisions and
  either ratify, amend, or reverse each one.
- **The audit trail is exhaustive.** Every non-trivial tier decision during R.5 and R.8
  produces a row in the tier-decision-record (`docs/governance/tier-decisions-2026-Qx.md`,
  produced during R.5). The row captures: verb FQN, baseline assigned, rationale,
  alternative considered, anomaly cluster (if any), provisional-Adam-signature.
- **No decision is silent.** A verb's tier carrying through R.5 without an explicit
  decision row counts as "default-mechanical" — applied per the standard cluster
  pattern, no rationale needed beyond the cluster's documented norm.

## Open organisational P-G question

Recorded as an open governance question for the platform:

> **Question:** What is the organisational P-G structure (named authority or body) that
> will replace the provisional Adam-as-architectural-authority designation when the
> v1.2 full-estate reconciliation activity completes?

**Status:** Open. Not blocking the current activity per v1.2 §13 amendment.

**Candidate models** (per v1.2 §13 lines 749-753, all to be evaluated organisationally):

- **Architecture committee** — a small standing group (platform lead, compliance lead,
  security lead, domain architect).
- **Platform lead** — single individual with delegated authority, consulting subject-
  matter experts as needed.
- **Per-workspace ownership** — workspace owners own tier decisions within their
  workspaces, with cross-workspace escalation to a central authority.
- **Tier-tiered authority** — lower tiers (benign / reviewable) assigned by author with
  validator sanity check; higher tiers require committee approval.

**Resolution timing:** When organisational partnership is available. The activity does
not pause for this resolution; the activity proceeds under provisional Adam-as-
architectural-authority and the audit trail makes review possible.

## Inheritance to Tranche 3

When Tranche 3 (Catalogue workspace, governed authorship mechanism) begins, it inherits:

- The reconciled catalogue with three-axis declarations under provisional authority.
- The tier-decision-record with full audit trail.
- This document's open organisational-P-G question.
- The expectation that organisational P-G review the provisional decisions as its first
  task.

Tranche 3's authorship mechanism does not depend on organisational P-G — it depends on
*some* named authority, which provisional designation satisfies. Tranche 3 may begin
under provisional authority, with the same revisability framing.

## Revocation / replacement

If, during the activity, organisational P-G is established:

1. The provisional Adam-as-architectural-authority designation transfers to the
   organisational authority.
2. In-progress R.5 / R.8 work continues; remaining decisions are signed off by the
   organisational authority.
3. Decisions made under provisional authority are reviewed by the organisational
   authority as its first task.

If, during the activity, the provisional designation is revoked without organisational
replacement:

1. The activity pauses (per v1.2 §13 atomic-with-respect-to-P-G framing).
2. Decisions made before revocation stand pending review.
3. Resumption requires re-establishing a named authority (provisional or organisational).

---

**End of provisional tier-assignment authority designation.**
