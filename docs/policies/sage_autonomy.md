# Sage Autonomy Policy — Tier-Keyed Rules

> **Phase:** v1.2 Tranche 1 DoD item 10.
> **Consumer:** Tranche 3 Sage integration (Phase 3.C).
> **Status:** Documented; ready for voluntary honouring until Tranche 3 makes architectural.

This policy is the architectural contract Sage honours when proposing or executing
verbs and runbooks. It consumes the **effective consequence tier** — the result of
P11 escalation rules + P12 runbook composition applied to a verb's declared baseline
tier — not the baseline alone.

## Per-tier autonomy rules

| Effective tier | Sage autonomy | UX surface | Audit trail |
|----------------|---------------|------------|-------------|
| `benign` | **Execute without confirmation.** | Silent completion; brief one-line acknowledgement. | Standard audit log. |
| `reviewable` | **Execute with announcement.** | Sage says "doing X now"; one-line summary on completion. | Standard audit log. User can review before next step. |
| `requires_confirmation` | **Pause and ask.** Must receive an affirmative reply ("yes" / "proceed" / "ok") before executing. | Sage paraphrases the action; user confirms. | Audit log captures user confirmation token. |
| `requires_explicit_authorisation` | **Pause and require explicit authorisation.** Must receive a paraphrased acknowledgement (user re-states the action, or types a confirmation phrase) — not a one-word "yes". | Sage paraphrases the action with full effect description; user's reply must demonstrate intent (re-state the entity, action, or consequence). | Audit log captures full transcript of authorisation exchange. ABAC gate may require additional credentials. |

## Effective tier always wins

Per v1.2 P11 (monotonic floor):

- The **baseline tier** is the architectural floor. Sage cannot execute below baseline.
- Escalation rules raise effective tier above baseline when context matches. Sage honours
  the elevated tier.
- For runbooks, P12 composition produces an effective runbook tier. Sage honours that
  composed tier as if it were a single verb's effective tier.

Sage **never** infers a lower tier from "common sense" or context. The declared baseline
is binding; only escalation rules can deviate, and only upward.

## Composition transparency

When a runbook escalates above the maximum step's effective tier (Component B aggregation
or Component C cross-scope), Sage's UX surfaces the *reason* — which composition rule
fired, and the resulting tier. The user sees an honest "this runbook is at tier T
because [reason]" message rather than an unexplained gate.

## Escalation transparency

When an individual verb escalates above its baseline (P11 escalation rule fired), Sage's
UX surfaces the rule name from the verb declaration: "Tier raised to X by rule
'large_holding_threshold' because [predicate]." This makes the policy visible, not
opaque.

## Override and degradation behaviour

- **User override of `reviewable`:** allowed silently if user pre-configures "execute
  reviewable without announcement"; Sage still logs.
- **User override of `requires_confirmation`:** disallowed at policy level — confirmation
  is the gate, not a UX preference.
- **User override of `requires_explicit_authorisation`:** disallowed at policy level —
  same reason. ABAC gate may further restrict.
- **Sage failing to execute due to gate:** Sage reports the gate's reason and stops. No
  retry-without-gate path.

## Per-axis autonomy refinements (Tranche 3 surface)

The four-tier policy is the baseline surface. Tranche 3 may layer additional refinements
(e.g. session-preference overrides, temporal cooldowns after a `requires_explicit_authorisation`
event). Out of scope for Tranche 1; documented here as forward-looking.

## Audit trail mandate

Every verb invocation Sage makes — regardless of tier — produces an audit log entry
containing: effective tier, baseline tier, fired escalation rules, fired composition
rules (if part of a runbook), user authorisation tokens (if any), entity bindings,
final outcome.

Audit log is the canonical record for organisational P-G review (v1.2 §13). No tier
decision is silently exercised.

## Test mode

Sage running against a test fixture honours the same policy. Test mode does not relax
the gate; it may stub user replies to keep tests automatable, but the stub is auditable.

---

**End of Sage autonomy policy.**
