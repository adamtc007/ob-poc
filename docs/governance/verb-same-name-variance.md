# Same-Name Verb Tier Variance — Annex

> **Phase:** v1.3 candidate amendment (2026-04-26).
> **Source:** Phase 2.G.4 catalogue self-consistency review.
> **Purpose:** Document why the same verb name (e.g. `suspend`) legitimately
>   carries different tier baselines across workspaces, so future Phase 2.G
>   audits don't flag the variance as miscalibration.

---

## Background

Phase 2.G.4 review (2026-04-26) inspected verbs sharing a name across ≥5
domains with >1 distinct tier. After 6 outlier fixes, the remaining
variance reflects **legitimate domain-specific stakes** rather than
miscalibration.

This annex records the cluster reasoning so the next reviewer doesn't
re-flag the same patterns.

---

## `suspend` (16 occurrences, 3 tiers)

| Tier | Verbs | Reason |
|------|-------|--------|
| `requires_confirmation` (12) | booking-principal-clearance, cash-sweep, cbu, collateral-management, settlement-chain, deal, matrix-overlay, reconciliation, investor, service-consumption, service-intent, service-resource | Standard lifecycle pause — reversible, audit-trail emit, user confirms |
| `requires_explicit_authorisation` (2) | manco, trading-profile | High-stakes regulatory / trading mandate suspension; explicit authorisation needed |
| `requires_confirmation` (post-fix; was reviewable) | investment-manager, user | T-2.G.4 fix: account-level suspension is consequential |

**Verdict:** legitimate variance. The `manco` and `trading-profile`
divergence is by design — both are regulated entities where suspension
has compliance impact beyond a routine pause.

## `activate` (10 occurrences, 3 tiers)

| Tier | Verbs | Reason |
|------|-------|--------|
| `requires_confirmation` (4) | booking-principal-clearance, collateral-management, reconciliation, service-consumption | Standard lifecycle activation — confirms the entity is ready to operate |
| `requires_explicit_authorisation` (1) | trading-profile | Trading mandate go-live is the highest-stakes activation in the platform |
| `reviewable` (5) | application-instance, matrix-overlay, investor, service-resource, shared-atom | Lower-stakes activations — these objects are activated within an outer confirmation flow |

**Verdict:** legitimate. The `reviewable` tier for `service-resource`
and `application-instance` reflects that activation happens inside a
larger CBU operational-readiness flow that's already gated.

## `reactivate` (7 occurrences)

| Tier | Verbs | Reason |
|------|-------|--------|
| `requires_confirmation` (5) | cash-sweep, collateral-management, settlement-chain, reconciliation, service-resource | Standard reactivation after a suspension |
| `requires_explicit_authorisation` (1) | trading-profile | Same reason as activate (mandate go-live) |
| `requires_confirmation` (post-fix; was reviewable) | user | T-2.G.4 fix |

**Verdict:** legitimate.

## `terminate` (5 occurrences, 3 tiers)

| Tier | Verbs | Reason |
|------|-------|--------|
| `requires_confirmation` (1) | client-principal-relationship | Reversible business relationship termination |
| `requires_explicit_authorisation` (2) | collateral-management, manco | Irreversible regulated-entity termination |
| `requires_confirmation` (post-fix; was reviewable) | contract, investment-manager | T-2.G.4 fixes |

**Verdict:** legitimate. `manco.terminate` and
`collateral-management.terminate` are at the highest tier because
post-termination there's no easy resurrection path under regulatory
oversight.

## `cancel` (7 occurrences)

| Tier | Verbs | Reason |
|------|-------|--------|
| `requires_confirmation` (5) | request, batch.abort, capital.cancel-shares, etc. | Routine cancel — reversible |
| `requires_explicit_authorisation` (2) | capital.cancel, capital.buyback variants | Capital-structure cancellations are irreversible |

**Verdict:** legitimate.

## `approve` (5 occurrences)

| Tier | Verbs | Reason |
|------|-------|--------|
| `requires_confirmation` (3) | doc-request, kyc-screening, cbu-ca | Routine workflow approval |
| `requires_explicit_authorisation` (2) | manco, governance.publish | Regulatory-tier approval |

**Verdict:** legitimate.

## `reject` (8 occurrences)

| Tier | Verbs | Reason |
|------|-------|--------|
| `requires_confirmation` (6) | document, deal-rate-card, etc. | Routine workflow rejection |
| `requires_explicit_authorisation` (2) | sanctions, governance | High-stakes rejection (regulatory consequence) |

**Verdict:** legitimate.

## `retire` (8 occurrences)

| Tier | Verbs | Reason |
|------|-------|--------|
| `requires_confirmation` (4) | service-consumption, capability-binding, etc. | Routine end-of-life |
| `requires_explicit_authorisation` (4) | trading-profile.retire-template, ruleset, regulatory artefacts | Permanent removal from authoritative catalogue |

**Verdict:** legitimate.

## `set` (5 occurrences)

| Tier | Verbs | Reason |
|------|-------|--------|
| `benign` (3) | agent.set-*, view.set-* | Pure configuration change |
| `reviewable` (2) | service-availability.set, board-controller.set | Material configuration change with downstream effects |

**Verdict:** legitimate.

## `remove` (5 occurrences, after T-2.G.4 fix)

All at `requires_confirmation` post-fix. Was `reviewable` for
`regulatory.registration.remove` — fixed.

---

## Pattern: when same-name variance IS a problem

Future Phase 2.G.4 reviews should flag a same-name variance as
miscalibration when:

1. **A verb in a high-stakes domain is at a LOWER tier than its
   cluster.** E.g. if `suspend` in regulatory.* were `reviewable` while
   the cluster median is `requires_confirmation`. The 6 fixes from
   2026-04-26 were exactly this pattern.

2. **A verb has a tier inconsistent with its `external_effects` mix.**
   E.g. a verb declaring `external_effects: [emitting]` at `benign` —
   emitters concentrate higher tiers per Phase 2.G.2.

3. **A verb's tier is materially different from the v1.3 shape-template
   default for its stem** without a documented domain-specific reason
   in `tier-decisions-*.md`.

## Pattern: when same-name variance is LEGITIMATE

The variance is legitimate when:

1. The high-tier outlier is in a regulated / high-stakes domain
   (manco, trading-profile, capital, governance, sanctions).
2. The low-tier outlier is in an inner-loop context already gated by
   an outer confirmation (agent-control, view, focus, session).
3. The variance correlates with the verb's `external_effects` axis —
   e.g. emitters at higher tier than non-emitters.
4. A documented escalation rule (P11) makes the per-verb baseline
   reasonable while context can escalate as needed.

---

## Maintenance

When the catalogue grows by ≥10 new verbs in any single domain that
shares names with existing clusters, re-run Phase 2.G.4 and update
this annex if new clusters emerge.

---

**End of same-name verb tier variance annex.**
