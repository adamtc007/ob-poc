# ADR: Macro/Verb Schema Parity for ACP Visibility (R1)

Status: accepted, in force from R1.3-5 onward.

Date: 2026-05-11

## Context

ACP pack context parity plan v0.5 §7.2 / §7.7 / §10 require macros to be
projected as first-class artefacts on the Sage/ACP visibility surface so
the agent can pick a macro as a peer to a verb at the DAG dispatch decision
point. Macros encapsulate compound DSL patterns and reduce agent invention
risk; treating them as second-class in the projection forces the agent to
re-compose what SemOS already catalogues.

Runtime execution semantics remain distinct: macros expand to verb
sequences at execution time; verbs do not. The peer relationship holds at
**planning, projection, and dispatch-decision** surfaces only.

## Decision

**Macros and verbs share field names and shapes for the parity fields
that the ACP envelope projects.** No translation table, no rename adapter.
The projection layer (R2) reads from both schemas and emits a single
uniform candidate list.

### Field-level reuse-vs-mirror decisions

| Field | Verb-side type | Macro-side decision |
|---|---|---|
| `requires_states: Vec<String>` | flat field on `VerbLifecycle` (under verb's `lifecycle:` block) | Flat field on `MacroSchema`. No wrapper struct. Same field name, same type. |
| `precondition_checks: Vec<String>` | flat field on `VerbLifecycle` | Flat field on `MacroSchema`. |
| `state_effect: StateEffect` | enum nested inside `three_axis:` | **Reuse** `dsl_core::config::StateEffect` directly. One enum, two consumers. |
| `transition_args: Option<TransitionArgs>` | direct field on `VerbConfig` | **Reuse** `dsl_core::config::TransitionArgs` directly. Required a one-line re-export from `dsl_core::config::mod`. |
| `side_effects: Option<MacroSideEffect>` | `Option<String>` on `VerbMetadata` (no enum) | **Typed mirror** `MacroSideEffect { StateWrite, FactsOnly }` with `#[serde(rename_all = "snake_case")]`. Same YAML strings deserialize into both sides. Verb-side enum extraction is a follow-up. |

### YAML key convention

All new macro fields use kebab-case keys (consistent with the rest of
`MacroSchema`):

```yaml
requires-states: []
precondition-checks: ["requires_scope:client"]
state-effect: transition
side-effects: state_write
transition-args:
  entity_id_arg: cbu
  target_workspace: cbu
  target_slot: products
```

### Creating vs modifying macros

For Slice 1, the 21 cbu-maintenance macros split into:

| Class | Count | `requires_states` | `transition_args` |
|---|---:|---|---|
| **Creating** (`struct.*` — produce a new CBU) | 18 | `[]` (entity doesn't exist yet) | `None` (no entity arg to bind) |
| **Modifying** (`structure.*` product-suite — change an existing CBU) | 3 | `[STRUCTURE_SETUP, READY_FOR_KYC]` | `{entity_id_arg: cbu, ...}` |

The regression test (`test_slice1_macros_declare_plan_kind_and_lifecycle_state`)
asserts both classes hold to their respective contracts.

`state_effect: Transition` is correct for both — creating macros produce
a non-trivial end-state, modifying macros transition between live states.

## Projection-adapter pattern (R2 contract)

R2 will introduce `AcpDispatchOption` (working name) at the envelope
projection layer. Because the parity fields share names and shapes, the
adapter is essentially:

```rust
let options = verbs.iter().map(verb_to_option)
    .chain(macros.iter().map(macro_to_option))
    .collect();
```

No `match` arms over a `DispatchKind` enum at every consumer. The only
kind-aware field is `dispatch_kind: verb | macro` carried on each option
for route trace fidelity.

R3's `selected_dispatch`, `candidates`, and `rejected_candidates` use
this same shape.

## Non-goals

- **Shared trait now.** Extracting a `DispatchOption` trait implemented by
  both `VerbConfig` and `MacroSchema` is a future refactor available
  once parity stabilises. Premature now; would touch every `VerbConfig`
  consumer.
- **Runtime peer-execution.** The executor keeps dispatching verbs.
  Macros expand to verb sequences at runtime as today. The peer
  abstraction lives above the executor, not at it.
- **Backfill non-Slice-1 macros.** The other ~120 production macros
  remain with `Option<...> = None` for the new fields. They can ship
  with their pack reopen. R1 enforces declaration only for the 21
  Slice 1 FQNs.

## Architectural invariants (added during R1 review)

These invariants are load-bearing for the macro/verb parity design and
must hold for the lifetime of the slice. CI enforces them; reviewers
treat violations as P1.

1. **Macros are a planning and compilation surface, not an execution
   surface.** ACP may expose macros for discovery and slot binding, but
   the REPL continues to execute only compiled DSL atomics.

2. **A macro has no mutation authority after expansion.** Once
   compiled, the runbook is an ordered atomic DSL sequence with macro
   provenance attached for audit only. No production code path may
   dispatch a `MacroSchema` directly to a mutation surface — mutation
   reaches the executor only via verb dispatch from a
   compiler-emitted ordered DSL sequence.

These pair with the v0.5 §16 single-path invariant. The §16.7
fuzz/property harness verifies them indirectly (every REPL emission
must trace to a verified envelope = a verified compiler expansion);
a direct CI lint (R2a deliverable) guards against drift.

## Redaction principle — projection ≠ implementation

> Sage needs enough to choose and bind the macro, not enough to become
> the execution engine.

The `DslAtom` projection for a macro **must not** expose the full
ordered `expands-to` body. The macro's internal expansion is the
compiler's surface, not the agent's surface.

Concretely, the `DslAtom` macro projection exposes:

- `expansion_summary: ExpansionSummary { step_count, distinct_verb_fqns,
  distinct_entity_kinds_touched, has_external_correlation }`
- `macro_hash: String` (deterministic hash over canonical macro body,
  enabling drift detection without exposing the body)

The full `expands_to: Vec<MacroExpansionStep>` stays SemOS-internal —
visible to the compiler and the registry, opaque to Sage and the ACP
envelope. Adding any new macro field requires an explicit decision on
which side of the line it sits.

## Reversibility

If parity-field semantics turn out wrong (e.g., we discover macros
need fields verbs don't, or vice versa), the rollback is local to
`MacroSchema` and the 21 macro YAMLs. No `VerbConfig` consumer changes,
no executor changes. The projection-adapter pattern means the worst
case is a re-edit of the adapter mapping function in R2 — not a
codebase-wide refactor.

## References

- `todo/acp-pack-context-parity-plan-v0.5.md` §7.2, §7.7, §10
- `rust/src/dsl_v2/macros/schema.rs` (parity field definitions)
- `rust/crates/dsl-core/src/config/types.rs` (reused types)
- `rust/src/dsl_v2/macros/registry.rs::test_slice1_macros_declare_plan_kind_and_lifecycle_state` (acceptance test)
