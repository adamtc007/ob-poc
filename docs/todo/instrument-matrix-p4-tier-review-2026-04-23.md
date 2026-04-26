# Instrument Matrix — P.4 Tier Review (2026-04-23)

> **Status:** OPEN — awaiting Adam's decisions.
> **Authority:** Adam-as-pilot-P-G (provisional per v1.1 §13).
> **Scope:** 192 IM-scope verbs with three-axis declarations.
> **Action required:** approve cluster defaults + resolve flagged individual cases.

---

## 1. Clustered defaults summary

Per-cluster tier defaults applied in P.3. Review each row: **APPROVE** (keep),
**RAISE** (tighten), **LOWER** (relax), or **SPLIT** (cluster needs finer grain).

| Cluster | Count | Current baseline | Cluster example | Approve / adjust? |
|---|---|---|---|---|
| **READ** (list / get / lookup / find / show / inspect) | 45 | `benign` | `trading-profile.read`, `trade-gateway.list-gateways` | ? |
| **ANALYSIS** (diff / compare / validate / impact / explain / coverage-matrix) | 10 | `benign` | `trading-profile.validate-go-live-ready`, `booking-principal.coverage-matrix` | ? |
| **CREATE_CONFIG** (create / define / add / set / configure / link / ensure / materialize) | 54 | `reviewable` | `trading-profile.add-component`, `settlement-chain.create-chain` | ? |
| **ACTIVATE** (activate / enable / go-live / start / record / complete) | 10 | `reviewable` (mostly) | `trade-gateway.activate-gateway`, `delivery.record` | **flagged — see §2.1** |
| **APPROVAL** (submit / approve / reject / publish) | 3 + 1 | `reviewable` (submit/approve/reject), `requires_confirmation` (publish) | `trading-profile.submit/.approve/.reject` | **flagged — see §2.2** |
| **SUSPEND_RESUME** (suspend / reactivate / resume / cancel / abort) | 17 | `requires_confirmation` | `cash-sweep.suspend`, `reconciliation.reactivate` | ? |
| **REMOVE** (remove / delete / deactivate / unlink) | 14 | `requires_confirmation` | `matrix-overlay.remove`, `trade-gateway.remove-routing-rule` | ? |
| **TERMINATE** (retire / archive / terminate / decommission / supersede) | 1 + 5 | `requires_confirmation` (booking-principal.retire), `requires_explicit_authorisation` (others) | `collateral-management.terminate`, `trading-profile.supersede` | **flagged — see §2.3** |
| **PRUNE** (prune-asset-family / -market / -class / -counterparty / -counterparty-type) | 5 | `requires_explicit_authorisation` | `trading-profile.prune-asset-family` | ? (recommended: keep — destructive by definition) |
| **OTHER** (capital flows, sync, ca.get-policy, change-vehicle) | 13 | `reviewable` (11), `requires_confirmation` (2) | `movement.subscribe`, `cash-sweep.change-vehicle` | **flagged — see §2.4** |

---

## 2. Flagged individual decisions

Specific tiers I'm less confident on. Please confirm/adjust each.

### 2.1 ACTIVATE cluster — is `reviewable` right for broker/gateway activations?

Current state:
- `trade-gateway.activate-gateway` → **reviewable**
- `trade-gateway.enable-gateway` → **reviewable**
- `matrix-overlay.activate` → **reviewable**
- `trading-profile.activate` → **reviewable** (pre-P.3 pack verb; we've also added `trading-profile.go-live` at **requires_explicit_authorisation** for the new parallel_run → active transition)
- `delivery.record / .start / .complete / .fail` → **reviewable**
- `movement.record-commitment / .fail` → **reviewable**
- vs. the newer `trading-profile.go-live` → **requires_explicit_authorisation**
- vs. `settlement-chain.go-live` / `collateral-management.activate` / `reconciliation.activate` → **requires_confirmation**

**Question:** Should all ACTIVATE-cluster verbs baseline at **requires_confirmation**
(activation usually means "this thing is live and emitting real signals"), or is the
current mix intentional?

**My recommendation:** **RAISE trade-gateway.activate-gateway / .enable-gateway to `requires_confirmation`.**
Gateway activation connects to real broker/exchange; confirmation gate is prudent.
Leave delivery / movement at reviewable (they're per-event recording).
Consider **deprecating `trading-profile.activate`** in favour of `go-live` — we may
have a legacy-vs-pilot-named-verb duplication.

**Adam: ?**

### 2.2 APPROVAL cluster — mandate approval tier

Current state:
- `trading-profile.submit` → **reviewable**
- `trading-profile.approve` → **reviewable**
- `trading-profile.reject` → **reviewable**
- `trading-profile.publish-template-version` → **requires_confirmation**

**Question:** Mandate approval is a compliance-gated decision. Reviewable means
"user sees message but no gate." Should approval verbs require confirmation or
explicit authorisation at baseline?

**My recommendation:** **RAISE `.submit / .approve / .reject` to `requires_confirmation`.**
These are the compliance-checkpoints of the onboarding lifecycle. The operator
should be asked "are you sure?" before they're clicked. Escalation to
`requires_explicit_authorisation` could apply when the mandate has specific
attributes (e.g. high-risk jurisdiction CBU, derivatives scope, etc.).

**Adam: ?**

### 2.3 TERMINATE cluster — one inconsistency

Current state:
- `booking-principal.retire` → **requires_confirmation** (but script pattern `retire` matched SUSPEND_RESUME/REMOVE→confirmation path, not TERMINATE's usual req_ex_auth)
- All other TERMINATE verbs → **requires_explicit_authorisation**

**Question:** Should `booking-principal.retire` match the others at
`requires_explicit_authorisation`? Arguably retiring a booking rule IS terminal
but it's also just "remove-rule" per Adam's Q8.

**My recommendation:** **LOWER to `reviewable` OR keep at `requires_confirmation`.**
booking-principal is a rule-entity (Q8); retiring a rule isn't as heavy as
terminating collateral management or retiring a gateway.

**Adam: ?**

### 2.4 OTHER cluster — capital-flow verbs

Current state:
- `movement.subscribe / .redeem / .transfer-in / .transfer-out / .settle / .confirm / .distribute / .distribute-recallable / .capital-call` → **reviewable**
- `movement.cancel / .fail` → `requires_confirmation` / `reviewable`
- `cash-sweep.change-vehicle` → **reviewable**

**Question:** Capital-flow events (subscribe, redeem, distribute) are real money
movements. Even if the DAG captures only the CONFIG declaration of these (not
the runtime event), the verb baseline should likely be higher.

**My recommendation:** **RAISE `movement.subscribe / .redeem / .distribute /
.distribute-recallable / .capital-call / .transfer-in / .transfer-out / .settle
/ .confirm` to `requires_confirmation`.** These verbs record CBU-level capital
events; operator confirmation is prudent. Leave `.cancel` at current
`requires_confirmation`.

**Adam: ?**

### 2.5 ANALYSIS cluster — matrix-overlay oddballs

Current state:
- Most ANALYSIS verbs → **benign** (correct — read-only analytics)
- `matrix-overlay.effective-matrix` → **reviewable** (slightly odd)
- `matrix-overlay.unified-gaps` → **reviewable**

**Question:** These compute overlay-aware matrix views. They read only (no
write). Why `reviewable` not `benign`? (The script pattern matched them because
they don't match `list/get/read` prefix.)

**My recommendation:** **LOWER to `benign`.** They're read-only analytics like
the other ANALYSIS verbs.

**Adam: ?**

### 2.6 APPROVAL — publish-template-version

Current state:
- `trading-profile.publish-template-version` → **requires_confirmation**

**Question:** Publishing a template version is governance-level (changes the
default template future CBUs clone from). Is `requires_confirmation` enough or
should it be `requires_explicit_authorisation`?

**My recommendation:** **RAISE to `requires_explicit_authorisation`.** Forward-
only template change that affects every future CBU cloned from this template.
Compliance should sign off.

**Adam: ?**

---

## 3. Escalation rules — currently only one verb

Only `corporate-action-event.elect` carries an escalation rule:

```yaml
escalation:
  - name: high_impact_election
    when:
      op: or
      preds:
        - op: arg_eq
          arg: option
          value: "tender_yes"
        - op: entity_attr_in
          entity_kind: corporate_action_event
          attr: event-type
          values: ["merger", "spinoff", "liquidation"]
    tier: requires_explicit_authorisation
    reason: "Election has material economic impact"
```

**Question:** Are there other verbs where context-dependent escalation would
sharpen the tier? Candidates worth considering:

- `trading-profile.submit` → escalate to `requires_explicit_authorisation` if
  the profile references high-risk jurisdiction CBUs (IR/KP/RU etc.) or
  derivatives scope.
- `movement.subscribe` → escalate if capital > threshold (e.g. > 10M USD
  equivalent).
- `cash-sweep.change-vehicle` → escalate if new vehicle is non-standard
  (e.g. not in approved STIF/MMF list).

**Adam: do you want these added, or flag as estate-scale follow-up?**

---

## 4. Decision ledger — please fill in

For each flagged item in §2 + §3, mark your decision:

| # | Item | Current | Proposed | Adam decision |
|---|---|---|---|---|
| 2.1.a | trade-gateway.activate-gateway | reviewable | requires_confirmation | **?** |
| 2.1.b | trade-gateway.enable-gateway | reviewable | requires_confirmation | **?** |
| 2.1.c | trading-profile.activate (legacy) | reviewable | deprecate or align to go-live | **?** |
| 2.2.a | trading-profile.submit | reviewable | requires_confirmation | **?** |
| 2.2.b | trading-profile.approve | reviewable | requires_confirmation | **?** |
| 2.2.c | trading-profile.reject | reviewable | requires_confirmation | **?** |
| 2.3 | booking-principal.retire | requires_confirmation | keep or lower to reviewable | **?** |
| 2.4 | movement.subscribe/.redeem/.distribute/etc | reviewable | requires_confirmation | **?** |
| 2.5.a | matrix-overlay.effective-matrix | reviewable | benign | **?** |
| 2.5.b | matrix-overlay.unified-gaps | reviewable | benign | **?** |
| 2.6 | trading-profile.publish-template-version | requires_confirmation | requires_explicit_authorisation | **?** |
| 3 | Add escalation rules to submit / subscribe / change-vehicle | none | consider | **?** |

---

## 5. Everything else — cluster-level approvals

If approving the cluster defaults as-is, mark **✓ APPROVE** below:

- [ ] READ → benign  (45 verbs)
- [ ] ANALYSIS → benign/reviewable (10+2 verbs) — sub-decision in §2.5
- [ ] CREATE_CONFIG → reviewable (54 verbs)
- [ ] ACTIVATE → reviewable/requires_confirmation/requires_explicit_authorisation (mix) — sub-decisions in §2.1
- [ ] APPROVAL → reviewable (3 verbs) — sub-decisions in §2.2 / §2.6
- [ ] SUSPEND_RESUME → requires_confirmation (17 verbs)
- [ ] REMOVE → requires_confirmation (14 verbs)
- [ ] TERMINATE → requires_explicit_authorisation (5 verbs) + 1 outlier (§2.3)
- [ ] PRUNE → requires_explicit_authorisation (5 verbs) — recommended keep
- [ ] OTHER → reviewable / requires_confirmation — sub-decisions in §2.4

---

## 6. Next steps after P.4 decisions

Once Adam's decisions are recorded:

1. **Apply tier changes** — mechanical edits to the YAML files.
2. **Author escalation rules** — for the verbs where §3 answered yes.
3. **Validator re-runs clean** — structural + well-formedness + warnings.
4. **Commit as "Pilot P.4: tier review applied under Adam-as-authority (pilot
   provisional)"** with decision ledger captured for estate-scale re-review.
5. **Proceed to P.5** — runtime triage (buckets 1/2/3) using the 39 oracle
   utterances against the reviewed catalogue.

All P.4 decisions marked `provisional — Adam-as-authority pilot convention` for
estate-scale re-review under real P-G governance (pilot plan §P.4, v1.1 §13).
