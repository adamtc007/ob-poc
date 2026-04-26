# Verb Shape Templates — Authoring Guide

> **Phase:** v1.3 candidate amendment (2026-04-26).
> **Purpose:** When authoring a new verb, start from the closest stem-template
>   below rather than from scratch. The validator does not enforce these
>   shapes (Phase 2.C governance pass owns tier judgment), but the templates
>   capture the patterns Phase 2.G.4 self-consistency review confirmed as
>   the catalogue's modal shape.
> **Companion:** `tier-decisions-2026-04-26.md` documents per-domain
>   judgments where the template is intentionally overridden.

---

## Read / list / get / show / search / find / lookup / query / fetch / inspect

Default shape:

```yaml
three_axis:
  state_effect: preserving
  external_effects: [observational]
  consequence:
    baseline: benign
```

Reads on internal state. The behavior is typically `crud` (operation:
`select` / `list_by_fk`) or `plugin` for compute-heavy queries.
*488 verbs* in the catalogue use this shape.

## Insert / create / new / register

Default shape:

```yaml
three_axis:
  state_effect: preserving
  external_effects: []
  consequence:
    baseline: reviewable
```

Adds a new record without transitioning an existing entity's state.
Use `state_effect: transition` only when the create itself is a state
transition (e.g. `create-pending-clearance` where the new row IS the
state machine's entry state — but in that case you also need
`transition_args:` per v1.2 §6.2).

## Update / set / configure / edit

Default shape:

```yaml
three_axis:
  state_effect: preserving
  external_effects: []
  consequence:
    baseline: reviewable
```

Modifies a record's non-state fields. If the update transitions the
state column, use `state_effect: transition` + `transition_args:`.

## Delete / remove / drop

Default shape:

```yaml
three_axis:
  state_effect: preserving
  external_effects: []
  consequence:
    baseline: requires_confirmation
```

Hard deletes — always at least `requires_confirmation`. Soft deletes
that flip a `disposition_status` to `soft_deleted` are state transitions
and should be `state_effect: transition` + `transition_args:`.

## Submit / approve / reject (lifecycle decisions)

Default shape:

```yaml
three_axis:
  state_effect: transition
  external_effects: [emitting]
  consequence:
    baseline: requires_confirmation
transition_args:
  entity_id_arg: <noun>-id
  target_workspace: <workspace>
  target_slot: <slot>
```

Most submit/approve/reject verbs are state transitions on a workflow
slot with audit-trail emission. Tighten to `requires_explicit_authorisation`
for sanctions-state, regulatory, or large-value approvals.

## Suspend / reinstate / reactivate / cancel / abort / terminate / archive / retire

Default shape:

```yaml
three_axis:
  state_effect: transition
  external_effects: [emitting]
  consequence:
    baseline: requires_confirmation
transition_args:
  entity_id_arg: <noun>-id
  target_workspace: <workspace>
  target_slot: <slot>
```

Lifecycle-altering verbs. Same-name verbs across domains may legitimately
diverge — see `verb-same-name-variance.md` for the cluster analysis.
Tighten to `requires_explicit_authorisation` for irreversible /
high-stakes operations (manco regulatory, trading-profile lifecycle,
share-class termination).

## Notify / send / alert / publish-event / broadcast

Default shape:

```yaml
three_axis:
  state_effect: preserving
  external_effects: [emitting]
  consequence:
    baseline: reviewable
```

Pure emitters. The verb doesn't transition state but signals an external
recipient. Tighten to `requires_explicit_authorisation` for compliance /
regulatory signals (attestations, disclosure filings, sanctions notices).

## View / focus / drill / zoom / navigate / select

Default shape:

```yaml
three_axis:
  state_effect: preserving
  external_effects: [navigating]
  consequence:
    baseline: benign
```

Viewport / navigation-only verbs. Don't transition real state; user
session focus / observation only. *Cluster:* nav, view, focus, session.

## Compute / calculate / derive / build-graph / analyze

Default shape:

```yaml
three_axis:
  state_effect: preserving
  external_effects: [observational]
  consequence:
    baseline: benign
```

Pure computation over existing data; doesn't write back. If the compute
result is persisted, the wrapping verb is the state-transition verb;
the compute itself stays preserving.

## Import / sync / load / refresh

Default shape:

```yaml
three_axis:
  state_effect: preserving
  external_effects: [observational]
  consequence:
    baseline: reviewable
```

Bulk pulls from external sources. `observational` because the read is
external; `reviewable` because the write internalizes external state
(which can be later proven wrong and reverted).

## Attest / sign-off / disclose / publish-attestation

Default shape:

```yaml
three_axis:
  state_effect: preserving
  external_effects: [emitting]
  consequence:
    baseline: requires_explicit_authorisation
```

Compliance-critical attestations. Always at the highest tier; the user
explicitly authorises a regulatory signal.

---

## When to override

Override the template when:

- **Workspace-level stakes are higher.** A `cancel` in trading-profile
  is `requires_explicit_authorisation`, not `requires_confirmation`,
  because cancelling a trading mandate has irreversible market impact.
- **Workspace-level stakes are lower.** An `activate` in agent-control
  context (start agent loop) is `reviewable`, not `requires_confirmation`,
  because the user is already inside a confirmation flow.
- **Context-dependent stakes.** Use a P11 escalation rule: declare the
  baseline at the lower tier, add an `escalation:` block that raises tier
  when context (arg threshold, entity attribute, session flag) matches.

Document the override in the relevant `tier-decisions-*.md` record.

## Validator interaction

The validator does NOT auto-apply these templates — they're authoring
guidance. v1.2 §6.2 strict checks:

- `state_effect: transition` requires `transition_args:`
- `state_effect: preserving` rejects `transition_args:`
- `transition_args:` (with v1.3 amendment + populated `known_slots`)
  must point at a known (workspace, slot) pair

The templates above land all verbs in compliant shapes by default.

---

**End of verb shape templates.**
