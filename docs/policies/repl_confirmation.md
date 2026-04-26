# REPL Confirmation Policy — Tier-Keyed Rules

> **Phase:** v1.2 Tranche 1 DoD item 10.
> **Consumer:** Tranche 3 REPL integration (Phase 3.C).
> **Status:** Documented; ready for voluntary honouring until Tranche 3 makes architectural.

The REPL is the user-authoritative access pattern (P14). The user is the ultimate
authority; the REPL's job is to make tier-keyed gates visible at the right friction
level so the user can exercise informed consent.

This policy consumes **effective consequence tier** (P11 + P12), not baseline.

## Per-tier confirmation behaviour

| Effective tier | REPL behaviour | Prompt shape |
|----------------|----------------|--------------|
| `benign` | **Execute on submit.** No prompt. | (none) |
| `reviewable` | **Execute on submit; show what's about to happen.** Brief inline preview before execution; user can cancel within a short window. | `→ <action summary>` (italic preview) |
| `requires_confirmation` | **Pause and require user keystroke.** User submits the verb; REPL displays a confirmation prompt; user types `y` / `yes` / `enter` to proceed or `n` / `cancel` to abort. | `Confirm: <action with entity name>? [y/N]` |
| `requires_explicit_authorisation` | **Pause and require typed paraphrase.** REPL displays the full action with consequences; user must type an acknowledgement phrase (e.g. the entity name or a confirmation token derived from the action) — not just `y`. | `This will <full effect>. Type "<confirmation token>" to authorise:` |

## Per-runbook confirmation

When the user invokes a macro, an ad-hoc multi-verb runbook, or a Sage-proposed runbook,
the REPL applies the **composed effective tier** (P12) to the whole runbook before any
step executes. The composed tier is computed once; per-step gates are not redundantly
prompted.

| Composed effective tier | REPL runbook prompt |
|--------------------------|---------------------|
| `benign` | Execute all steps; show summary on completion. |
| `reviewable` | Show the planned step list; execute on `y`/Enter. |
| `requires_confirmation` | Show the planned step list with each step's effective tier; user confirms the runbook (one prompt for the whole runbook). |
| `requires_explicit_authorisation` | Show the full plan with effects and the *reason* for escalation (which composition rule fired, or which step contributed). Require typed paraphrase. |

If a step's effective tier exceeds the composed runbook tier (rare but possible if
escalation rules fire mid-runbook on entities not yet known at composition time), the
REPL pauses at that step and re-confirms.

## Escalation transparency

The user sees *why* a verb is at its current effective tier:

- Baseline tier displayed.
- Each fired escalation rule named with its `reason:` text from the verb declaration.
- Composed runbook tier displayed with the contributing component (A max-step / B
  aggregation / C cross-scope) and the reason.

Example UX:

```
$ deal.contracted deal-1234

This deal has 3 contributing booking principals (BP_aggregation rule fired:
"large multi-principal deal").
Composed tier: requires_explicit_authorisation (escalated from requires_confirmation
by aggregation rule "large multi-principal deal").

This will mark deal-1234 as CONTRACTED, gating BP clearance #1, #2, #3 and
service consumption activation.

Type "deal-1234 CONTRACTED" to authorise:
```

## Per-tier UX defaults

- `benign`: execute silently. No prompt.
- `reviewable`: 0.5-second pre-execution dwell with a brief preview line; user can ^C
  to cancel.
- `requires_confirmation`: blocking prompt; default to N (cancel on Enter without `y`).
- `requires_explicit_authorisation`: blocking typed-paraphrase. No default; idle prompt
  times out at 10 minutes with abort.

## Override behaviour

- **User pre-configures "skip-confirmation for reviewable":** allowed; REPL still shows
  a one-line execution log. The audit trail records the override preference.
- **User attempts to override `requires_confirmation`:** REPL ignores the override
  preference; this tier is architecturally gated.
- **User attempts to override `requires_explicit_authorisation`:** same — ignored.

Effective tier is the architecture; user preferences cannot weaken it.

## Audit trail

Every verb / runbook invocation produces an audit log entry capturing:

- Effective tier, baseline tier, fired escalation rules, composed-runbook reason (if
  applicable).
- User authorisation token (if `requires_confirmation` or `requires_explicit_authorisation`).
- Time-to-confirmation latency (UX metric).
- Per-step outcomes for runbooks.

## Failure modes

- **User cancels at confirmation:** verb / runbook does not execute; no partial state.
- **User authorisation phrase mismatches:** REPL re-prompts with hint; after 3 mismatches,
  abort.
- **Underlying gate blocks execution despite user authorisation:** REPL surfaces the
  gate's reason ("KYC case not approved"); user sees it's a system gate, not a
  confirmation gate.

## Tranche 3 implementation note

This policy is *documented* in Tranche 1; Tranche 3 Phase 3.C wires it into the REPL
runtime. Pre-Tranche-3 REPL behaviour is unchanged — voluntary honouring only.

---

**End of REPL confirmation policy.**
