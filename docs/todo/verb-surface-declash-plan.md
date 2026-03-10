# Verb Surface Declash Plan

## Goal

Reduce deterministic Coder ambiguity by refactoring the verb surface, not by changing the Sage -> Coder -> DSL architecture.

This plan is limited to verb-surface cleanup:
- merge true synonyms
- split overloaded verbs
- rename vague verbs so action semantics are explicit
- add missing metadata needed by the resolver

Execution target:
- start with the highest-impact domains from the clash export
- `agent`
- `deal`
- `view`
- `client-group`

## Scope

In scope:
- YAML verb config changes
- alias/canonical naming cleanup
- metadata tagging (`harm_class`, `action_class`, `phase_tags`, `subject_kinds`)
- compatibility aliases where needed
- deterministic harness reruns after each domain batch

Out of scope:
- Sage/Coder architecture changes
- LLM-first routing changes
- runbook engine redesign
- UI redesign
- broad whole-registry renaming in one sweep

## Success Metric

A domain batch is only complete when both conditions hold:

1. For every surviving pair of verbs in the domain, their difference can be stated in one sentence using semantic fields, not hand-waving.
2. Every surviving verb has a unique semantic address across:
   - `domain`
   - `action_class`
   - `subject_kinds` / required entity signature
   - `harm_class`
   - `phase_tags` or state/precondition where relevant

Operational gate:
- rerun clash export after each batch and show pair-count reduction in that domain
- rerun utterance coverage after each major batch

## Inputs

Primary inputs:
- `/Users/adamtc007/Developer/ob-poc/rust/target/coder-clash-matrix/clash_matrix.csv`
- `/Users/adamtc007/Developer/ob-poc/rust/target/coder-clash-matrix/clash_matrix.md`
- [coder-resolution-cleanup-todo.md](/Users/adamtc007/Developer/ob-poc/docs/todo/coder-resolution-cleanup-todo.md)

Supporting inputs:
- verb YAML under `rust/config/verbs/`
- current Coder filters and diagnostics under `rust/src/sage/`

## Non-Negotiable Rules

1. Refactor in domain batches, not registry-wide.
2. Preserve backward compatibility with aliases where user-facing phrasing already exists.
3. Do not silently change runtime semantics while renaming.
4. Every file edit must be followed by `cargo check -p ob-poc`.
5. Each batch ends with a clash rerun.
6. No batch is complete if it only renames strings without adding the required metadata.

## Execution Order

### Batch D1 — `agent`

Why first:
- currently the highest clash domain in the export
- mixes confirmation, policy, mode, history, and learning verbs
- chat/session mechanics are sensitive to ambiguity here

Objectives:
- split confirmation verbs by intent
- rename vague `get-*` / `set-*` patterns to action-encoded names where necessary
- tag all `agent.*` verbs with explicit `harm_class`, `action_class`, and relevant `phase_tags`

Expected cleanup:
- `agent.confirm` -> split into `agent.confirm-decision` and `agent.confirm-mutation`
- `agent.get-mode` -> `agent.read-mode`
- `agent.get-policy` -> `agent.read-policy`
- audit `agent.learn`, `agent.reject`, `agent.pause`, `agent.resume`, `agent.history` for distinct action semantics

Files likely touched:
- `rust/config/verbs/agent*.yaml`
- `rust/config/agent/*.yaml` where phrases/indices refer to old FQNs
- `rust/src/api/agent_service.rs`
- `rust/src/api/agent_routes.rs`
- `rust/src/agent/orchestrator.rs`
- tests covering confirmation/state routing

Verification:
- `cargo check -p ob-poc`
- targeted chat/session tests
- clash export and compare `agent` pair count before/after

### Batch D2 — `deal`

Why second:
- very high clash count
- dense business surface with many read/list/detail verbs sharing `deal-id`
- directly affects business utterance coverage

Objectives:
- normalize read/detail/list naming
- eliminate vague read families where names do not encode the actual target view
- add full metadata coverage across the domain

Expected cleanup:
- normalize `deal.get` / `deal.read`
- decide whether `deal.summary` is a distinct view or alias of detail read
- distinguish `deal.list-*` families from detail reads with explicit action and target semantics
- ensure `active-rate-cards`, `list-rate-cards`, `list-documents`, `list-products`, `list-slas` have explicit action metadata and are not competing with generic read verbs

Files likely touched:
- `rust/config/verbs/deal*.yaml`
- any noun/phrase indices referencing deal verb names
- coverage fixtures where canonical verb names change

Verification:
- `cargo check -p ob-poc`
- targeted scorer tests for `deal` inventory vs detail reads
- clash export and compare `deal` pair count before/after
- rerun utterance coverage after this batch

### Batch D3 — `view`

Why third:
- heavy noun-shaped naming
- navigation verbs are colliding because names under-encode action
- this surface affects read/write safe-mode behavior in chat

Objectives:
- rename noun verbs into explicit navigation/read verbs
- clearly separate read-only view inspection from view-state mutation
- add consistent `phase_tags: [navigation]`

Expected cleanup:
- `view.layout` -> `view.read-layout`
- `view.status` -> split or rename to reflect actual read target
- `view.back-to` -> `view.navigate-back`
- `view.select`, `view.refine`, `view.clear`, `view.breadcrumbs` all need explicit action semantics and harm classes

Files likely touched:
- `rust/config/verbs/view.yaml`
- UI/server code if canonical names are surfaced anywhere
- any navigation-specific tests

Verification:
- `cargo check -p ob-poc`
- targeted navigation tests
- clash export and compare `view` pair count before/after

### Batch D4 — `client-group`

Why fourth:
- high clash count
- strong onboarding phase semantics already exist in the metadata surface but are inconsistent
- discovery/list/entity verbs need sharper naming

Objectives:
- normalize list/discovery naming
- separate workflow/process verbs from pure reads
- enforce consistent `phase_tags: [onboarding]`

Expected cleanup:
- `client-group.parties` -> `client-group.list-parties`
- distinguish `discover-entities`, `start-discovery`, `complete-discovery`
- distinguish discrepancy/relationship/role/unverified list surfaces with explicit action names and target nouns

Files likely touched:
- `rust/config/verbs/client-group.yaml`
- workflow/config references using client-group FQNs
- utterance fixtures if canonical names move

Verification:
- `cargo check -p ob-poc`
- targeted onboarding/client-group tests
- clash export and compare `client-group` pair count before/after

## Per-Batch Refactor Procedure

For each domain batch:

1. Export current clash subset for that domain.
2. Classify each pair as:
   - merge
   - split
   - rename
   - metadata-only fix
3. Patch verb YAML first.
4. Add aliases or compatibility shims where required.
5. Update phrase indices and tests.
6. Run `cargo check -p ob-poc` after each file edit.
7. Run focused tests for that domain.
8. Re-export clash matrix.
9. Record before/after pair counts.

## Alias Strategy

Use aliases when:
- the old name is already user-facing
- the old name is referenced in fixtures, runbooks, or learned phrase memory
- the new name is clearer but semantics are unchanged

Do not use aliases when:
- the old verb is genuinely overloaded
- the split changes execution semantics
- the old name hides a dangerous/destructive path

## Required Metadata Standard

Every touched surviving verb must leave the batch with:
- `harm_class`
- `action_class`
- `phase_tags`
- `subject_kinds` when the verb applies only to certain subject types

Default rule:
- if metadata cannot be stated clearly, the verb is not clean enough to survive unchanged

## Validation Gates

### Gate V1 — Compile
- `cargo check -p ob-poc`

### Gate V2 — Clash Regression
- `cargo test -p ob-poc --test coder_clash_regressions -- --nocapture`
- `cargo test -p ob-poc --test coder_clash_matrix -- --ignored --nocapture`

### Gate V3 — Domain Pair Reduction
- compare domain pair count before/after the batch
- expected outcome: meaningful reduction, not metadata churn without pair reduction

### Gate V4 — End-to-End Coverage
- after `deal` batch and after full four-domain pass:
  - start `ob-poc-web`
  - run `cargo test -p ob-poc --test utterance_api_coverage -- --ignored --nocapture`

## Exit Criteria

The four-domain pass is complete when:
- `agent`, `deal`, `view`, and `client-group` each have materially reduced clash counts
- touched verbs in those domains have full metadata coverage
- renamed/split/merged verbs are reflected in tests and indices
- utterance coverage is re-measured on the live path
- remaining residual clashes in those domains are explainable and intentionally distinct

## Expected Outcome

This should not be sold as architecture work.
It is refactoring to make the existing deterministic architecture converge.

Expected effect:
- fewer noun-collision candidates in Coder
- fewer `<none>` outcomes caused by overloaded surfaces
- better action alignment in read vs write routing
- cleaner basis for future Sage/Coder accuracy work
