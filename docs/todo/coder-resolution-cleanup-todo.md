# Coder Resolution Cleanup TODO

Derived from:
- `/rust/target/coder-clash-matrix/clash_matrix.csv`
- `/rust/target/coder-clash-matrix/clash_matrix.md`
- current verb metadata scan across `rust/config/verbs/**/*.yaml`

Purpose:
- reduce residual Coder ambiguity after `harm_class`, `action_class`, and first-pass context filtering
- turn the clash export into concrete cleanup work that unblocks deterministic resolution

Current state:
- clash pairs: `1030`
- dominant bucket: `ActionDifferentiable = 875`
- metadata gaps: `1050` verbs missing at least one of `harm_class`, `action_class`, `phase_tags`

## Merge Candidates

These are near-synonyms that should collapse to one canonical verb plus aliases.

| Current Name(s) | Proposed Canonical Name | Rationale | Unblocks |
| --- | --- | --- | --- |
| `cbu.read`, `cbu.show` | `cbu.read` + alias `cbu.show` | Two read-only verbs for the same CBU detail operation; `show` adds lexical noise, not semantics | Reduces `cbu-id` read clashes and makes safe-read resolution deterministic |
| `deal.get`, `deal.summary` | `deal.read` with optional `view=summary` or alias `deal.get` | User intent is usually "show the deal"; separate naming overstates a view distinction that the resolver cannot exploit | Removes read/detail ambiguity in `deal` domain |
| `contract.list`, `contract.list-subscriptions` | `contract.list-subscriptions` + alias `contract.list` | Bare `list` is underspecified; the specific meaning in practice is subscriptions | Cuts generic-list collisions in `contract` |
| `session.load-system`, `session.set-structure` | `session.set-structure` + alias `session.load-system` | Both change navigation focus; one should be canonical and the other an alias for backward compatibility | Reduces navigation/write clash surface in `session` |
| `deal.list`, `deal.active-rate-cards` when used as broad inventory reads | keep both but alias generic "show deal rate cards" to `deal.list-rate-cards` | Current naming allows generic inventory phrasing to hit multiple list-like verbs | Makes rate-card read queries land on one canonical list surface |

## Split Candidates

These are overloaded verbs or families that are acting like two operations and should be separated structurally.

| Current Name | Proposed Split | Rationale | Unblocks |
| --- | --- | --- | --- |
| `view.status` | `view.selection-status` and `view.navigation-status` | Current name collapses selection state and navigation state into one vague read surface | Lets Coder choose the right read verb from phase/context instead of lexical guesswork |
| `agent.confirm` | `agent.confirm-decision` and `agent.confirm-mutation` | Same noun/decision surface currently spans different pending-state mechanics | Prevents confirmation routing from colliding across decision and mutation flows |
| `service-resource.activate` | keep `service-resource.activate` for instance state, keep `service-resource.activate-lifecycle` for lifecycle state, but explicitly separate invocation phrases and metadata | Current pair is only weakly distinguished by suffix; clash export shows state-level confusion | Lets state/precondition filters remove invalid lifecycle candidates before scoring |
| `cbu.delete` | `cbu.delete` and `cbu.delete-cascade` with explicit destructive metadata and confirmation policy split | Both already exist, but the distinction is not strong enough in metadata | Isolates destructive paths from normal delete resolution |
| `research.outreach.mark-sent` / `research.outreach.send-reminder` / `research.outreach.close-request` | split into explicit action classes and state preconditions | Same `request-id` signature, different lifecycle state | Lets state-aware filtering handle outreach request verbs deterministically |

## Rename Candidates

These are vague names that should become action-encoded so the resolver does not have to infer intent from nouns alone.

| Current Name | Proposed Name | Rationale | Unblocks |
| --- | --- | --- | --- |
| `agent.get-mode` | `agent.read-mode` | `get` is weak; `read-mode` encodes read action directly | Improves action-class scoring in `agent` |
| `agent.get-policy` | `agent.read-policy` | Same issue; explicit read verb is cheaper for the scorer | Reduces `agent` read clashes |
| `client-group.parties` | `client-group.list-parties` | Bare noun verb hides that this is a list read | Helps collection-read queries resolve without lexical fallback |
| `cbu.parties` | `cbu.list-parties` | Same issue in `cbu` domain | Aligns list/read inventory handling with action metadata |
| `ownership.compute` | `ownership.compute-summary` | Current name is too broad versus `ownership.analyze-gaps`, `ownership.reconcile`, `ownership.who-controls` | Makes compute vs analyze distinction machine-readable |
| `view.layout` | `view.read-layout` | Bare noun hides a read operation | Reduces view/navigation action clashes |
| `view.back-to` | `view.navigate-back` | Encodes navigation action explicitly | Improves routing in navigation/write family |
| `deal.get` | `deal.read` | Consistent action naming across business domains | Makes action-class weighting more reliable |

## Metadata Gaps

These are not one-off verbs. They should be fixed in batches because they are blocking the resolver from using the metadata path consistently.

| Current Scope | Proposed Change | Rationale | Unblocks |
| --- | --- | --- | --- |
| `deal.*` | Add explicit `harm_class`, `action_class`, `phase_tags` across the full domain | `deal` is one of the largest clash domains (`87` pairs) and almost all verbs are still inferred, not tagged | Stabilizes read/list/update routing in one of the hottest business domains |
| `agent.*` | Tag the full domain, starting with `confirm`, `reject`, `learn`, `get-*`, `set-*`, `history` | `agent` is currently the single largest clash domain (`92` pairs) | Prevents confirmation and policy reads from colliding with write/session mechanics |
| `trading-profile.*` | Add `phase_tags` and explicit `action_class` to lifecycle and component verbs | `57` clashes and many are lifecycle/status verbs sharing `profile-id` | Lets state/phase filters remove invalid lifecycle candidates early |
| `view.*` | Tag navigation verbs with `phase_tags: [navigation]`, explicit `action_class`, and explicit `harm_class` for write-only navigation state | `55` clashes remain because many verbs are noun-shaped and under-tagged | Cleans up navigation hot path and keeps safe read/write separation |
| `client-group.*` | Add explicit `action_class` and `phase_tags: [onboarding]` consistently, not partially | `52` clashes, many list/discovery verbs share `group-id` | Makes onboarding context a real discriminator |
| `ownership.*` | Add explicit `action_class` and precondition/state metadata to reconcile/compute/analyze verbs | `43` clashes in a semantically dense domain | Separates compute/read/reconcile paths |
| `service-resource.*` | Add lifecycle state metadata plus `phase_tags` and explicit `action_class` | `41` clashes, including real state-differentiable pairs | Lets `C5` filters do more than just lexical ranking |
| `session.*` | Add explicit `harm_class` and `action_class` for navigation/state verbs | `38` clashes and many are navigation mutations masquerading as reads | Keeps chat session routing predictable |
| `investor.*` | Tag lifecycle/list/update families explicitly | `38` clashes, mostly action-differentiable | Reduces collisions in a high-entity business domain |
| `capital.*` | Tag issuance/dilution verbs with stronger action and state metadata | `34` clashes and several real state transitions | Supports future state-precondition filtering |
| global missing metadata set (`1050` verbs) | Add YAML-first `harm_class` and `action_class`; stop relying on inference for hot-path domains | Inference was necessary to start, but it is now the limiting factor | Makes the resolver deterministic instead of heuristic-heavy |

## Suggested Execution Order

1. Metadata batch: `agent`, `deal`, `view`, `client-group`
2. Rename batch: explicit list/read/navigation names in `view`, `cbu`, `client-group`, `deal`
3. Split batch: `agent.confirm`, `service-resource.activate*`, outreach lifecycle verbs
4. Merge batch: `cbu.read/show`, `contract.list*`, selected session aliases
5. Re-run clash export and utterance coverage
