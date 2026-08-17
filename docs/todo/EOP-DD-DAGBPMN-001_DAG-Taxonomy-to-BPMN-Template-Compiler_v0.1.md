# EOP-DD-DAGBPMN-001 — DAG-Taxonomy-to-BPMN-Template Compiler v0.1

**Status:** implemented, RATIFIED 2026-08-17
**Crate:** `rust/crates/dag-to-bpmn`
**CLI:** `cargo x dag-to-bpmn <dag-file> <slot-id> [--out <path>]`

## 1. Context — "Option A" of the shared-board vision

Adam's stated vision: one shared board (a DAG entity-state model with DSL
moves attached), soft-loaded per workspace, producing either a BPMN
workflow template or a KYC UBO taxonomy as output. Prior investigation this
session (2026-08-17) established that the taxonomy/vocabulary layer already
exists and is soft-loaded on both sides — `rust/config/sem_os_seeds/dag_taxonomies/*.yaml`
(domain-pack-owned, parsed by `DagRegistry`/`dsl_types::dag::Dag`) already
*is* "a DAG entity-state model with moves (verb FQNs) attached to the
board," and it already produces a KYC UBO taxonomy as output (`kyc_dag.yaml`
is a real KYC entity-state taxonomy today). The missing half was the other
output: nothing compiled that same shape into a BPMN workflow template.
This document ratifies the compiler that closes that gap — "Option A" —
and records its scope boundary against "Option B" (unifying the two
domains' move-*resolution* engines — `ob-poc-kyc-substrate`'s
`DeterminationStrategy` fold vs. BPMN's `designer-graph` legality logic —
which remains separate, deliberately unstarted, follow-on design work).

Building this compiler serves double duty: it is additive production
tooling, and it is a pressure test of whether the DAG-taxonomy shape is
actually rich enough to be "the board" ahead of any decision on Option B.
§5 records the real result of that pressure test against the live
`kyc_dag.yaml`.

## 2. Why text emission, not a `RailwayGraph` builder

`dsl-bpmn-frontend::RailwayGraph`'s only constructor,
`RailwayGraph::empty()`, is `pub(crate)`, and the crate is
`#![deny(unreachable_pub)]` — there is no public builder API. The only way
to produce a `RailwayGraph`/`JourneySpec` from outside that crate is
`assemble()`, which requires a parsed `AtomBag`, which requires bpmn-lite
DSL source text.

This is the correct design, not merely the only one available. Emitting
textual bpmn-lite DSL and validating it through
`dsl_migrate_verify::compile_to_spec` — the same function
`ob-poc-web::process_registry` uses to compile stored `dsl_source` rows at
startup (`ob-poc-web/src/main.rs`, one `bpmn-runtime::RuntimeEngine` per
definition) — means this compiler inherits `dsl-bpmn-frontend`'s existing
structural validation (`DUPLICATE_NAME`, `GATEWAY_FAN_OUT_ERROR`,
`INVALID_BOUNDARY_TARGET`, `UNREACHABLE_NODE`, `UNTERMINATED_PATH`) for
free instead of reimplementing any of it, touches zero lines of a crate
live in the production process registry, and produces a genuinely
human-readable `.dsl` text artifact — a real "template," not an opaque
struct.

## 3. Ratified mapping (mechanical, not interpretive)

Compiles **one DAG slot's state machine at a time** — cross-slot/cascade
relations (`parent_slot`, `state_dependency`) model hierarchy, not sequence
flow, and are out of scope; forcing them into one BPMN process would invent
semantics the source doesn't express.

- The slot's one `entry: true` state -> a bare `start-event` node.
- Each state in `terminal_states` -> a bare `end-event` node.
- Each distinct `(to, via)` transition -> one `service-task` node bound via
  `:verb (invoke <fqn>)`, named `t__<to>__<verb>` (never just `<to>`, to
  avoid collisions when two different verbs land on the same state).
  Non-entry, non-terminal states are *not* modeled as their own node —
  they're the implicit waypoint between two transition-task nodes; edges
  chain task -> task directly.
- A state with more than one distinct outgoing `(to, via)` gets an
  `exclusive` gateway inserted before the branch (BPMN structurally
  requires this before diverging flows — the one genuine synthesis step
  this compiler performs, and it's mechanical, not invented business
  logic). The first branch (sorted by `(to, via)`) gets `:default true`;
  the rest get `:condition "<verb>"`.
- A state with multiple incoming transitions needs no gateway — multiple
  edges converging on one node is valid BPMN (implicit OR-join), whether
  or not any of those producing states also happen to fan out elsewhere.
- `from: "(any non-terminal)"` expands deterministically to every state not
  in `terminal_states` (this is a defined macro over data the registry
  already has, not a guess). It composes correctly with a state's own
  explicit transitions — if the expansion adds a second distinct outgoing
  target to that state, the state correctly gets a gateway too.
- `from` given as a real YAML list of state ids is treated as an explicit
  multi-source fan-in.

**Fails loud, cites the exact slot/transition, never silently drops data**
(`ShapeError`, `rust/crates/dag-to-bpmn/src/error.rs`):
- `via` is a list of verb FQNs — ambiguous whether OR-trigger or
  AND-trigger, undecidable from the data.
- `via` is free text with no real verb FQN shape (heuristic: contains `.`,
  no whitespace, no parens — real verb FQNs are dotted identifiers;
  observed free-text annotations are parenthesized prose).
- `via` is missing.
- `to` names a state not declared in `state_machine.states`.
- `from` is a string that is neither a real state id nor the
  `"(any non-terminal)"` literal (covers the `"(STATE_A, STATE_B)"`
  parenthesized-string-not-list authoring convention observed in
  `kyc_dag.yaml` — a real, separate authoring inconsistency this compiler
  does not paper over).
- Zero or more than one `entry: true` state.
- A slot with no `state_machine` (stateless) or a `state_machine:
  reference-name` string form — nothing structured to compile.

Preconditions and `args` on `TransitionDef` are **not** modeled — out of
v1 scope, not summarized or guessed at.

## 4. Critical files

- `rust/crates/dag-to-bpmn/src/{lib,compile,emit,error}.rs` — the compiler.
  `compile_slot(dag, slot_id, process_name) -> Result<CompiledTemplate,
  DagToBpmnError>` is the entire public API.
- `rust/crates/dag-to-bpmn/tests/fixtures.rs` — unit fixtures (linear
  chain, fan-out/gateway, fan-in/no-gateway, `"(any non-terminal)"`
  expansion, and each fail-loud shape), including a RED-first proof that
  the list-valued-`via` guard actually rejects what it claims to (verified
  by temporarily neutering the guard, confirming the test failed, then
  restoring it).
- `rust/crates/dag-to-bpmn/tests/kyc_dag_pressure_test.rs` — loads the real
  `kyc_dag.yaml` and reports, per slot, compiled / no-state-machine /
  rejected-with-reason. This is the actual pressure-test deliverable — see
  §5.
- `rust/xtask/src/dag_to_bpmn.rs` — `cargo x dag-to-bpmn <dag-file>
  <slot-id> [--out <path>]`, a thin CLI wrapper for manual inspection.
- No changes anywhere else. `dsl-bpmn-frontend`, `dsl-lowering`,
  `process_registry.rs`, domain-pack YAML, and the PACK001 lint are all
  untouched — this reads already-loaded `Dag` data and emits an artifact,
  it invokes no macros and declares no pack ownership.

## 5. Pressure-test result (2026-08-17, `kyc_dag.yaml`, 28 slots)

Run via `cargo test -p dag-to-bpmn --test kyc_dag_pressure_test --
--nocapture`:

- **4 compiled clean**: `kyc_ubo_evidence`, `kyc_decision`,
  `entity_proper_person`, `manco`.
- **11 have no structured state machine** to compile (stateless slots, or
  a `state_machine: reference` form).
- **13 rejected**, and every rejection is a real, legitimate gap in the
  source data, not a compiler limitation to fix reactively:
  - Most are **backend/system-triggered transitions with no DSL verb
    behind them at all** — `(backend: no hits)` (screening), `(time-decay)`
    (ubo_evidence), `(backend: sanctions hit)` (holding), `(backend:
    kyc_expires_at trigger)` (investor_kyc), etc. That portion of the KYC
    taxonomy genuinely isn't verb-driven — a real finding about the shape
    of the taxonomy, not a bug here.
  - 2 are the `"(SENT, REMINDED)"` parenthesized-string-not-a-real-list
    `from` authoring convention (`outreach_request`, `investor`) — a fixable
    `kyc_dag.yaml` authoring inconsistency (should be a real YAML list),
    flagged but out of scope to silently paper over here.

**Conclusion for the "is the DAG-taxonomy shape rich enough to be the
board" question**: partially. Roughly a third of `kyc_dag.yaml`'s slots
compile clean today with zero invented semantics. The rest split between
"not applicable" (stateless slots) and "genuinely not verb-driven"
(backend-triggered transitions) — which is a real structural finding, not
a shortfall in this compiler: a meaningful fraction of KYC's lifecycle is
driven by system/time events rather than DSL moves, which any unified
"board" concept (Option B) would need to account for rather than assume
away.

## 6. What this document does not decide

Whether to pursue Option B (unifying the KYC and BPMN move-resolution
engines) is not decided here. §5's finding — that a real fraction of KYC's
own taxonomy isn't move-driven — is relevant input to that decision, not a
verdict on it.
