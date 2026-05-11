# Slice 3 DAG Session Adapter Plan

Status: planned; implementation started with Sage session entity-resolution feedback.

Date: 2026-05-11

## Decision

Slice 3 is not a generic database adapter.

Slice 3 adapts the existing Sage work-session model into ACP runtime feedback:

```text
utterance
-> existing entity-linking / lookup service
-> workspace and DAG template selection
-> existing workspace hydration
-> hydrated DAG session instance
-> redacted ACP runtime projection
```

The existing services remain the source of truth:

- `LookupService` and `EntityLinkingService` for mention spans, candidate UUIDs, selected UUIDs, confidence, expected kinds, and snapshot metadata.
- `hydrate_workspace_state` for workspace-scoped DAG hydration.
- `HydratedConstellation` / `WorkspaceStateView` for populated DAG instance state.
- `ReplSessionV2` for session visibility and trace continuity.

## Non-Negotiables

1. Do not add a second entity resolver inside ACP.
2. Do not hydrate ACP directly from arbitrary SQL rows.
3. Do not let entity names, labels, documents, provider config, secrets, or commercial terms enter ACP runtime context.
4. Do expose entity-resolution evidence in Sage session feedback so the user can see which UUIDs and candidates drove the current session state.
5. Do treat entity resolution as the front door for existing-vs-new entity handling.

## Implementation Phases

### S3.1 Entity Resolution Visibility

Expose the existing lookup result in Sage session feedback:

- entity snapshot hash, version, and entity count
- expected entity kinds inferred from the verb surface
- mention spans and mention text
- selected UUID, confidence, and top candidate UUIDs
- candidate kind, canonical name, score, and evidence chain

This is session feedback for Sage/UI inspection. ACP runtime projection must separately redact it down to ids, kinds, confidence buckets, counts, snapshot refs, blocker codes, and selected UUIDs.

### S3.2 DAG Hydration Source Adapter

Project from the already hydrated top-of-stack workspace first:

- `ReplSessionV2.workspace_stack.last().hydrated_state`
- `WorkspaceStateView.subject_ref`
- `HydratedConstellation.map_revision`
- hydrated slot paths, states, progress, blockers, and available verbs

Only call `hydrate_workspace_state` when the session has no fresh hydrated DAG.

### S3.3 Existing vs New Entity Handling

Use entity resolution outcome to drive DAG instance population:

- selected UUID: hydrate existing entity/CBU/case/workspace subject
- ambiguous candidates: keep session in clarification, do not guess
- no candidate and create intent: create provisional session-local binding only
- no candidate and inspect/update intent: fail closed with missing entity binding

### S3.4 ACP Runtime Projection

Extend the Slice 2 runtime source builder with a DAG-session source:

- selected entity UUIDs
- subject kind and subject UUID
- workspace and constellation refs
- map revision and hydration snapshot refs
- slot state counts
- blocker/missing-binding codes
- available verb FQN counts or allowlisted verb ids
- runtime hash and projection hash

No raw hydrated slot labels or entity display names enter ACP.

## Acceptance

Required fixtures:

- existing UUID hydrates the expected DAG subject and slot state
- ambiguous entity candidates block hydration until clarified
- new entity intent creates only provisional session-local state
- missing entity for inspect/update fails closed
- stale or missing hydration emits missing source diagnostics
- runtime-present ghost routes still refuse
- ACP output contains no raw entity names, free text, labels, documents, or raw DB fields

Required validation:

```text
cargo check
cargo fmt --check
cargo clippy --lib -- -D warnings
cargo test repl::session_v2::tests::test_session_feedback_exposes_entity_resolution -- --nocapture
cargo test acp_runtime_context_sources::tests -- --nocapture
cargo test --test gate_e_single_path_invariant -- --nocapture
```
