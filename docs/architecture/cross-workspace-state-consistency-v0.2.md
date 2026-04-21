# Cross-Workspace State Consistency Architecture

**Document:** `cross-workspace-state-consistency.md`
**Version:** 0.2 — DRAFT FOR PEER REVIEW
**Author:** Adam (Lead Solution Architect) / Claude (Architecture Partner)
**Date:** 2026-04-02
**Status:** Vision & Architecture — Pre-Implementation
**Repo:** `ob-poc` (https://github.com/adamtc007/ob-poc)

---

## Revision History

| Version | Date       | Author | Summary                              |
|---------|------------|--------|--------------------------------------|
| 0.1     | 2026-04-02 | Adam/Claude | Initial vision, scope, architecture |
| 0.2     | 2026-04-02 | Adam/Claude | Aligned with codebase: corrected terminology (verb contract fields, workspace names, constellation families), noted platform DAG is NEW infrastructure (no existing `constellation_edges` table), updated shared atom YAML examples with actual family names, referenced existing staleness propagation pattern (`propagate_derived_chain_staleness`), updated execution audit to reference `dsl_idempotency`, noted `RebuildContext` extends existing `ReplayEnvelope`, resolved Q-01/Q-04/Q-05/Q-07, added §3.5 (existing infrastructure reuse) |

---

## 1. Vision

ob-poc operates a multi-workspace, constellation-governed platform where entity
lifecycle management spans several autonomous domains — KYC, OnBoarding,
CBU, Deal, Instrument Matrix, Product Maintenance, and SemOS Maintenance. Each workspace governs
its own constellation DAG: a directed acyclic graph of entity-state atoms
with dependency edges that define verb execution order, staleness propagation,
and lifecycle gating.

Today, workspace isolation is structurally sound. Logic collision — two
workspaces interfering with each other's verb execution — is prevented by
constellation-scoped verb sets, workspace pack constraints, and the mandatory
tollgate sequence (ScopeGate → WorkspaceSelection → JourneySelection → InPack).

**State collision is not addressed.** When a shared fact (e.g. LEI, domicile
jurisdiction, fund structure type, UBO beneficial ownership threshold) is
mutated in its owning workspace, consuming workspaces have no visibility
of the change. Downstream state built on the prior version silently drifts.
In custody banking — where operational, legal, and regulatory reality is
constructed on top of these facts — silent drift is not an acceptable
failure mode.

This document defines a platform-level state consistency mechanism that:

1. Declares shared atoms and their ownership at the SemOS registry level.
2. Extends the existing constellation DAG model upward into a platform-level
   DAG with cross-workspace dependency edges.
3. Uses a `make`-style dirty-flag propagation and topological rebuild to
   restore consistency when shared facts change.
4. Handles external (third-party) side effects through an idempotency
   envelope with provider-capability-aware amendment logic.
5. Produces a full compensation audit trail suitable for regulatory review.

The guiding principle: **drift is not prevented — it is made visible, typed,
and mechanically resolvable.** The platform does not attempt to keep all
workspaces synchronised in real-time. It detects divergence, computes the
minimal rebuild set, and re-executes only the affected vertices.

---

## 2. Scope

### 2.1 In Scope

- Platform-level DAG (Level 0) as a structural extension of constellation
  DAGs (Level 1).
- Shared atom declaration, ownership, and consumer registration in SemOS.
- Cross-workspace dependency edge types: `hard` and `soft`.
- Dirty-flag propagation via transitive closure on the platform DAG.
- Topological rebuild of dirty vertices using existing re-runnable DSL
  runbooks.
- External call idempotency envelope for third-party interactions.
- Provider capability classification (amendable, cancel-recreate, immutable,
  manual).
- Compensation log and audit trail for regulatory compliance.
- Remediation event lifecycle for vertices that cannot be automatically
  rebuilt.

### 2.2 Out of Scope

- Real-time cross-workspace event streaming or pub/sub (not required;
  dirty-flag model is post-commit, not reactive).
- Changes to intra-workspace constellation DAG structure or verb contracts.
- Automated conflict resolution heuristics or AI-driven reconciliation
  (v2 concern — "Referee Agent").
- NL/Sage surface for remediation — this document covers the platform
  mechanics only.
- Performance optimisation of DAG traversal at scale (premature; shared atom
  count is expected to be O(50), not O(10,000)).

### 2.3 Assumptions

- DSL runbooks are re-runnable by design. The existing `RunbookPlan` with
  cursor-driven execution (`rust/src/runbook/plan_executor.rs`),
  `ReplayEnvelope` for exact-replay, and content-addressed immutable plan IDs
  confirm this. Replaying from a state gate with corrected inputs produces
  correct output.
- Verb input resolution reads from the current fact store at execution time.
  A forced re-run of a verb will pick up the latest version of any shared
  atom.
- The existing constellation DAG traversal algorithm (via `SlotDef.depends_on`
  and `DependencyEntry` in `constellation_map_def.rs`) is generic and can
  inform cross-workspace edge design without structural modification.
- External system interactions are a minority of total verb executions
  (~10%) but represent the highest-risk surface for rebuild correctness.

---

## 3. Architecture

### 3.1 The `make` Analogy

The rebuild model is directly analogous to the UNIX `make` utility:

| `make` Concept          | ob-poc Equivalent                                    |
|-------------------------|------------------------------------------------------|
| Source file (`.h`)      | Shared atom (LEI, jurisdiction, fund structure type)  |
| Object file (`.o`)      | Vertex state (completed verb output)                  |
| Dependency graph (`Makefile`) | Platform DAG + constellation DAGs              |
| Timestamp comparison    | Version comparison (`held_version` vs `current_version`) |
| Dirty flag              | `stale` marker on vertex                              |
| Recompilation           | Verb re-execution with `RebuildContext`                |
| Incremental build       | Only dirty vertices re-run                            |
| Build failure           | Verb re-execution fails → vertex stays dirty → escalate |

Key properties inherited from `make`:

- **Deterministic.** The rebuild set is computed from the DAG, not assessed
  by heuristic or simulation.
- **Incremental.** Only vertices downstream of the changed atom are
  re-executed.
- **Resumable.** If a rebuild fails partway, the dirty set is known. Re-run
  picks up where it stopped.
- **Idempotent (internal).** Re-running a verb with the same inputs produces
  the same output.

### 3.2 Two-Level DAG Hierarchy

The platform operates a two-level DAG hierarchy sharing identical structural
primitives (vertices, typed edges, topological traversal, staleness
propagation):

```
┌─────────────────────────────────────────────────────┐
│                  LEVEL 0: PLATFORM DAG              │
│                                                     │
│   Vertices: Shared atoms (LEI, jurisdiction, ...)   │
│   Edges:    Cross-workspace dependencies            │
│   Scope:    Platform-wide (~30–50 nodes)            │
│   Purpose:  Dirty-flag propagation across domains   │
│                                                     │
│   ┌──────────┐    hard    ┌──────────────────┐      │
│   │entity.lei├───────────►│cbu::lei_ref      │      │
│   │ (KYC)    ├───soft────►│onboarding::pack  │      │
│   └──────────┘            └──────────────────┘      │
│                                                     │
├─────────────────────────────────────────────────────┤
│              LEVEL 1: CONSTELLATION DAGs            │
│                                                     │
│   Vertices: Entity-state atoms (SlotDef nodes)      │
│   Edges:    Intra-workspace verb dependencies       │
│             (SlotDef.depends_on: Vec<DependencyEntry>)
│   Scope:    Per-constellation (100s of nodes)       │
│   Purpose:  Verb execution order, lifecycle gating  │
│                                                     │
│   Existing model, unchanged.                        │
└─────────────────────────────────────────────────────┘
```

**The connection between levels:** A constellation DAG vertex that reads a
shared atom has an implicit upward edge into the platform DAG. The platform
DAG vertex for `entity.lei` IS the same atom that appears in the KYC
constellation. It additionally carries edges that cross constellation
boundaries.

**Schema implication:** The platform DAG is **new infrastructure**. Level 1
constellation DAGs are schema-less (YAML-driven via `SlotDef.depends_on` in
`constellation_map_def.rs`). There is no existing `constellation_edges` table.
The platform DAG requires a new `shared_atom_consumers` table (§6.2) to
persist cross-workspace edges. The same traversal algorithm applies, but the
storage layer is distinct.

### 3.3 Cross-Workspace Edge Types

| Type   | Semantics                                                  | Rebuild Behaviour                            |
|--------|------------------------------------------------------------|----------------------------------------------|
| `hard` | Consumer **breaks** if shared atom changes.                | Verb execution blocked until rebuild succeeds.|
| `soft` | Consumer **should know** the atom changed. Advisory only.  | Vertex flagged stale. Execution permitted.    |

Edge type is a property of the consumer's dependency declaration, not of the
atom itself. The same atom (LEI) may have a `hard` consumer (CBU workspace)
and a `soft` consumer (OnBoarding workspace).

### 3.4 Shared Atom Discovery

Verb contracts (`VerbContractBody` in `sem_os_core/src/verb_contract.rs`)
declare their data footprint via `reads_from: Vec<String>` and
`writes_to: Vec<String>`. These are **table-level** declarations (e.g.,
`"entities"`, `"cbus"`), not attribute-level paths.

Cross-workspace dependencies are discovered through a two-step join:

```
For each declared shared_atom:
    1. Resolve atom.path (e.g., "entity.lei") to its backing table
       via attribute_registry (domain → table mapping)
    2. Scan all verb footprints where reads_from includes that table
       → any verb in a non-owning workspace = implicit cross-workspace consumer
```

This is an O(shared_atoms × verb_count) scan at platform boot or registry
reload. With ~50 shared atoms and ~1,464 verbs, this is trivially fast.
Manual consumer declaration in the shared atom YAML (§4.1) serves as a
seed and override; automated discovery supplements it.

**Note:** The table-level granularity of `reads_from` means discovery is
coarse — a verb that reads the `entities` table is flagged as a consumer of
all shared atoms backed by that table. This is conservative (over-declaration
is safe; under-declaration is not). Future refinement could add column-level
footprint declarations to `VerbContractBody`.

### 3.5 Existing Infrastructure Reuse

The cross-workspace staleness mechanism builds on an existing pattern: **derived
attribute staleness propagation**. The current system already implements:

- `derived_attribute_values.stale: BOOLEAN` — dirty flag on derived values
- `derived_attribute_dependencies` table — dependency edges between values
- `propagate_derived_chain_staleness(p_attr_id, p_entity_id)` SQL function —
  transitive closure walk that marks all dependents stale
- `mark_stale_by_input()` Rust function (`src/derived_attributes/repository.rs`) —
  marks all derived rows stale based on input attribute/entity pair

The cross-workspace `mark_dirty` (§5.1) extends this same algorithm from
attribute-scoped dependencies to workspace-scoped fact references. The
structural primitives (stale flag, dependency edges, transitive propagation)
are identical.

---

## 4. Capabilities

### 4.1 Shared Atom Declaration

Shared atoms are declared at the SemOS registry level, above any single
constellation. Declaration specifies ownership, constraint defaults, and
known consumers.

```yaml
# semos/shared_atoms/entity_lei.yaml
shared_atom:
  path: "entity.lei"
  display_name: "Legal Entity Identifier"
  owner:
    workspace: kyc
    constellation_family: kyc_workspace
  constraint_default: hard
  consumers:
    - workspace: cbu
      constellation_family: cbu_workspace
      constraint: hard
    - workspace: onboarding
      constellation_family: onboarding_workspace
      constraint: soft
    - workspace: instrument_matrix
      constellation_family: instrument_workspace
      constraint: hard
  validation:
    format: "^[0-9A-Z]{20}$"  # ISO 17442
    gleif_verification: required
```

### 4.2 Dirty-Flag Propagation

When a shared atom is mutated (by any verb in the owning workspace), the
platform triggers a post-commit propagation:

1. Identify the changed atom and its new version.
2. Walk the platform DAG downstream from the atom vertex.
3. For each consumer vertex, check consumer entity state:
   - **In-flight** → flag vertex as `stale`. Next verb execution hits
     pre-REPL precondition check.
   - **Complete / Active** → flag vertex as `dirty`. Enqueue for rebuild.
4. Propagate transitively: if vertex A is dirty and vertex B depends on A's
   output, B is also dirty.

The propagation is a single post-commit graph walk. It does not trigger
any verb execution. It is a marking pass only. This mirrors the existing
`propagate_derived_chain_staleness()` pattern — trigger-based, not
in-transaction.

### 4.3 Topological Rebuild

Dirty vertices are re-executed in topological order (upstream before
downstream) using existing DSL runbooks:

1. Compute the dirty set: transitive closure of all vertices downstream of
   the changed atom.
2. Sort dirty vertices in topological order.
3. For each dirty vertex, resolve inputs from the current fact store (which
   now contains the corrected shared atom value).
4. Execute the originating verb with a `RebuildContext` metadata envelope.
5. On success: vertex output is updated, vertex marked clean. Downstream
   vertices will pick up the new output when they execute.
6. On failure: vertex remains dirty. Remediation event is created and
   escalated.

The rebuild is runbook-gated: it uses the same execution pipeline as initial
onboarding. No special rebuild logic exists outside the `RebuildContext`
flag and the external call idempotency envelope (§4.5).

**Implementation note:** `RebuildContext` extends the existing `ReplayEnvelope`
pattern (`rust/src/runbook/types.rs`) with shared-fact-correction metadata
(source atom, prior version, new version, trigger type). This avoids
introducing a parallel execution context type.

### 4.4 Pre-REPL Staleness Check (In-Flight Entities)

For entities where the consuming workspace has active (non-complete) state,
the existing verb execution pipeline gains one additional precondition:

```
For each table in verb_contract.reads_from:
    For each shared_atom backed by that table:
        Compare workspace's held_version against current_version
        If held_version < current_version:
            If constraint == hard:
                → return StatePreconditionError::StaleSharedFact
            If constraint == soft:
                → log advisory, continue execution
```

This check runs in the pre-REPL phase, before verb body execution. It is a
hash lookup per reads_from entry against the shared fact version store.

**UX integration:** When `StaleSharedFact` fires, the `NarrationEngine`
(`rust/src/agent/narration_engine.rs`) surfaces it as a blocker in
`NarrationPayload.blockers`, explaining what changed and what action is
needed to acknowledge or rebuild.

### 4.5 External Call Idempotency Envelope

Verbs that interact with third-party systems (sub-custodian account opening,
settlement instruction registration, regulatory filing) require special
handling on rebuild. The verb must distinguish between first-run and re-run.

**Decision logic within an external-calling verb:**

```
On verb execution:
    Check external_call_log for (entity_id, verb_id, provider):
        No prior record → first run:
            Execute external call (create account, file report, etc.)
            Record call in external_call_log
        Prior record exists, input hash differs → rebuild with changes:
            Execute amendment/correction per provider capability
            Supersede prior record in external_call_log
            Create compensation_record for audit
        Prior record exists, input hash matches → no-op:
            Skip external call (already correct)
            Mark vertex clean
```

### 4.6 Provider Capability Classification

Not all third-party systems support the same correction patterns. Provider
capabilities are declared in SemOS metadata:

| Capability           | Behaviour on Rebuild                                     | Example Providers           |
|----------------------|----------------------------------------------------------|-----------------------------|
| `amendable`          | `PUT`/amend on existing resource. Preferred path.        | Modern custody APIs, SWIFT gpi |
| `cancel_and_recreate`| Cancel prior, create new. Two-step correction.           | Some settlement systems      |
| `immutable`          | Cannot modify. Issue correction referencing original.     | SWIFT MT messages, regulatory filings |
| `manual`             | No API for corrections. Human intervention required.      | Legacy systems, phone/email  |

The classification is per-provider, per-operation. A provider may support
amendment for account details but require cancel-and-recreate for settlement
instructions.

```yaml
# semos/providers/bny_sub_custody.yaml
provider:
  name: "BNY Sub-Custody Services"
  operations:
    account_opening:
      capability: amendable
      amend_endpoint: "PUT /accounts/{ref}"
      idempotency_key: entity_id + lei + jurisdiction
    settlement_instruction:
      capability: cancel_and_recreate
      cancel_endpoint: "DELETE /instructions/{ref}"
      create_endpoint: "POST /instructions"
    regulatory_filing:
      capability: immutable
      correction_pattern: "Submit amendment filing referencing original"
```

### 4.7 Remediation Event Lifecycle

When a rebuild fails or requires manual intervention, a Remediation Event
is created as a first-class SemOS entity with its own lifecycle FSM:

```
                 ┌──────────┐
                 │ Detected │
                 └────┬─────┘
                      │ (auto: impact assessment)
                      ▼
                 ┌──────────┐
            ┌────┤ Assessed ├────┐
            │    └──────────┘    │
            ▼                    ▼
     ┌────────────┐      ┌────────────┐
     │ Resolving  │      │ Escalated  │
     │ (auto/1-clk│      │ (human     │
     │  rebuild)  │      │  review)   │
     └─────┬──────┘      └──┬────┬───┘
           │                 │    │
           ▼                 ▼    ▼
     ┌──────────┐    ┌──────────┐ ┌──────────┐
     │ Resolved │    │ Resolved │ │ Deferred │
     └──────────┘    └──────────┘ └──────────┘
```

**State transitions:**

- **Detected → Assessed**: Automatic. Platform computes dirty set, classifies
  impact per vertex (compatible / breaking / regulatory).
- **Assessed → Resolving**: All dirty vertices classified as `compatible`.
  Rebuild can proceed automatically or with one-click approval.
- **Assessed → Escalated**: One or more dirty vertices classified as
  `breaking` or `regulatory`. Human review required.
- **Resolving → Resolved**: All dirty vertices successfully rebuilt.
- **Escalated → Resolved**: Human has reviewed, approved corrective action,
  and all vertices rebuilt.
- **Escalated → Deferred**: Explicit human decision to accept divergence.
  Records reason. This is a compliance-auditable outcome — not a system
  failure. In custody banking, the legally binding record may intentionally
  diverge from the corrected source (e.g. onboarding completed under LEI v1;
  correction to v2 is noted but operational state preserved pending
  regulatory cycle).

**SemOS integration:** The FSM would be declared as a new state machine YAML
(`rust/config/sem_os_seeds/state_machines/remediation_event_lifecycle.yaml`)
following the existing pattern (e.g., `attribute_def_lifecycle.yaml`).

### 4.8 The "Do Nothing" Path

The `Deferred` state is architecturally significant. The system must support
an explicit, audited decision to not rebuild. Reasons include:

- Regulatory reporting cycle has already closed on the prior value.
- Sub-custodian has confirmed the prior value is operationally acceptable.
- Correction is queued for a scheduled maintenance window.
- Legal opinion is that the prior value is the binding record.

`defer_remediation(entity_id, remediation_id, reason)` is a first-class
DSL verb that transitions the remediation event to `Deferred`, records the
reason, the authorising user, and the timestamp. The dirty vertices remain
flagged but are explicitly excluded from rebuild until the deferral is
revoked.

---

## 5. Core Functions

### 5.1 `mark_dirty`

Propagates dirty flags from a changed shared atom through the platform DAG
and into consuming constellation DAGs via transitive closure.

**Input:** Changed atom reference (path + entity_id + new version).
**Output:** Set of dirty vertex IDs, topologically sorted.
**Trigger:** Post-commit hook on any verb that mutates a shared atom.

```
mark_dirty(changed_atom) → Vec<VertexId>:
    root = platform_dag.vertex_for(changed_atom)
    dirty_set = empty
    for vertex in platform_dag.walk_downstream(root):
        if vertex.reads(changed_atom.path):
            vertex.mark_stale(new_version)
            dirty_set.add(vertex)
            // Propagate into constellation DAG
            for child in constellation_dag.walk_downstream(vertex):
                if child.transitively_depends_on(changed_atom.path):
                    child.mark_stale(new_version)
                    dirty_set.add(child)
    return dirty_set.topological_sort()
```

**Reuse note:** This is structurally identical to the existing
`propagate_derived_chain_staleness(p_attr_id, p_entity_id)` SQL function,
extended from attribute-scoped to workspace-scoped dependencies.

### 5.2 `rebuild_dirty`

Re-executes dirty vertices in topological order using existing verb
execution pipeline.

**Input:** Topologically sorted dirty vertex set, current fact store.
**Output:** Per-vertex result (clean / still-dirty / escalated).
**Trigger:** Explicit invocation (auto or human-approved, per §4.7).

```
rebuild_dirty(dirty_vertices, fact_store) → Vec<RebuildResult>:
    results = empty
    for vertex in dirty_vertices:  // topological order
        verb = vertex.originating_verb()
        inputs = resolve_inputs(vertex, fact_store)  // picks up new value
        context = RebuildContext {
            // Extends existing ReplayEnvelope pattern
            trigger: SharedFactCorrection,
            source_atom: changed_atom,
            prior_version: vertex.held_version,
            new_version: fact_store.current_version(changed_atom),
        }
        result = verb.execute(inputs, context)
        match result:
            Ok(output):
                vertex.update_output(output)
                vertex.mark_clean()
                results.add(RebuildResult::Clean(vertex.id))
            Err(domain_error):
                vertex.remain_dirty()
                results.add(RebuildResult::Failed(vertex.id, domain_error))
    return results
```

### 5.3 `check_staleness` (Pre-REPL)

Precondition check injected into the verb execution pipeline for in-flight
entities.

**Input:** Verb contract `reads_from` set, shared atom registry, fact version store.
**Output:** Pass / `StatePreconditionError::StaleSharedFact`.
**Trigger:** Every verb execution (pre-REPL phase).

```
check_staleness(verb_contract, entity_id) → Result<(), PreconditionError>:
    for table in verb_contract.reads_from:
        for shared in shared_atom_registry.atoms_backed_by(table):
            held = workspace_fact_refs.version_for(entity_id, shared.atom_path)
            current = shared_fact_store.current_version(entity_id, shared.atom_path)
            if held < current:
                match shared.constraint_type:
                    Hard → return Err(StaleSharedFact { attr: shared.atom_path, held, current })
                    Soft → log_advisory(shared.atom_path, held, current)
    return Ok(())
```

### 5.4 `execute_with_idempotency` (External Call Wrapper)

Wraps any verb that calls a third-party system, providing first-run vs.
rebuild branching.

**Input:** Verb execution context, external call log, provider capabilities.
**Output:** External call result + compensation record (if applicable).
**Trigger:** Verb body, for any operation classified as external.

```
execute_with_idempotency(verb_ctx, provider, operation, params):
    request_hash = hash(params)
    prior = external_call_log.lookup(entity_id, verb_id, provider)

    match (prior, prior.request_hash == request_hash):
        (None, _):
            // First run
            result = provider.execute(operation, params)
            external_call_log.record(entity_id, verb_id, provider,
                                     result.external_ref, request_hash,
                                     result.response)
            return result

        (Some(prior), true):
            // Rebuild, but inputs unchanged — no-op
            return prior.cached_result

        (Some(prior), false):
            // Rebuild with changed inputs — amend
            capability = provider_registry.capability(provider, operation)
            match capability:
                Amendable:
                    result = provider.amend(prior.external_ref, params)
                CancelAndRecreate:
                    provider.cancel(prior.external_ref)
                    result = provider.create(params)
                Immutable:
                    result = provider.submit_correction(prior.external_ref, params)
                Manual:
                    return ManualInterventionRequired {
                        prior_ref: prior.external_ref,
                        changed_params: diff(prior.params, params),
                    }
            compensation_log.record(prior, result, verb_ctx.rebuild_context)
            external_call_log.supersede(prior.id, result)
            return result
```

---

## 6. Data Model

All tables in the `"ob-poc"` schema, following existing conventions:
`id UUID DEFAULT uuidv7()`, `TIMESTAMPTZ DEFAULT now()`, explicit FK
references with schema prefix.

### 6.1 `shared_atom_registry`

Declares shared atoms and their ownership. Populated from SemOS YAML
declarations. Small, slowly-changing reference data.

| Column              | Type        | Description                                    |
|---------------------|-------------|------------------------------------------------|
| `id`                | `UUID`      | PK (`DEFAULT uuidv7()`).                       |
| `atom_path`         | `TEXT`      | Dot-notation attribute path. e.g. `entity.lei` |
| `display_name`      | `TEXT`      | Human-readable name.                           |
| `owner_workspace`   | `TEXT`      | Workspace that owns this atom.                 |
| `owner_constellation_family` | `TEXT` | Constellation family within owner workspace. |
| `constraint_default`| `TEXT`      | CHECK: `hard` / `soft`. Default for new consumers. |
| `validation_rule`   | `JSONB`     | Format, external verification requirements.    |
| `attribute_registry_id` | `TEXT`  | FK → `attribute_registry.id`. Links to underlying attribute definition. |
| `created_at`        | `TIMESTAMPTZ` | `DEFAULT now()`.                             |
| `updated_at`        | `TIMESTAMPTZ` | `DEFAULT now()`.                             |

**Index:** `UNIQUE (atom_path)`.

### 6.2 `shared_atom_consumers`

Tracks which constellation vertices consume shared atoms and the constraint
type applicable to each consumer.

| Column              | Type        | Description                                    |
|---------------------|-------------|------------------------------------------------|
| `id`                | `UUID`      | PK (`DEFAULT uuidv7()`).                       |
| `atom_id`           | `UUID`      | FK → `shared_atom_registry.id`.                |
| `consumer_workspace`| `TEXT`      | Consuming workspace.                           |
| `consumer_constellation_family` | `TEXT` | Consuming constellation family.         |
| `constraint_type`   | `TEXT`      | CHECK: `hard` / `soft`. Overrides registry default. |
| `discovered_at`     | `TIMESTAMPTZ` | When this consumer was first identified.     |
| `source`            | `TEXT`      | CHECK: `declared` (YAML) / `discovered` (footprint scan). |

**Index:** `UNIQUE (atom_id, consumer_workspace, consumer_constellation_family)`.

### 6.3 `shared_fact_versions`

Versioned fact store for shared atoms. One row per entity × atom × version.
This is the "source file timestamp" in the `make` analogy.

| Column              | Type        | Description                                    |
|---------------------|-------------|------------------------------------------------|
| `id`                | `UUID`      | PK (`DEFAULT uuidv7()`).                       |
| `atom_id`           | `UUID`      | FK → `shared_atom_registry.id`.                |
| `entity_id`         | `UUID`      | The entity this fact belongs to.               |
| `version`           | `INT`       | Monotonically increasing version number.       |
| `value`             | `JSONB`     | The fact value at this version.                |
| `mutated_by_execution` | `UUID`   | References `dsl_idempotency.execution_id`.     |
| `mutated_by_user`   | `UUID`      | User who triggered the mutation (from `dsl_idempotency.actor_id`). |
| `mutated_at`        | `TIMESTAMPTZ` | When this version was committed.             |
| `is_current`        | `BOOL`      | True for the latest version. Denormalised for fast lookup. |

**Indexes:** `UNIQUE (atom_id, entity_id, version)`.
`(atom_id, entity_id) WHERE is_current = true` — fast current-version lookup.

### 6.4 `workspace_fact_refs`

Tracks which version of a shared fact each consuming workspace is currently
operating against. This is the "object file timestamp" in the `make` analogy.

| Column              | Type        | Description                                    |
|---------------------|-------------|------------------------------------------------|
| `id`                | `UUID`      | PK (`DEFAULT uuidv7()`).                       |
| `atom_id`           | `UUID`      | FK → `shared_atom_registry.id`.                |
| `entity_id`         | `UUID`      | The entity.                                    |
| `consumer_workspace`| `TEXT`      | The consuming workspace.                       |
| `held_version`      | `INT`       | Version this workspace last operated against.  |
| `status`            | `TEXT`      | CHECK: `clean` / `stale` / `dirty` / `deferred`. |
| `stale_since`       | `TIMESTAMPTZ` | When the staleness was detected (NULL if clean). |
| `remediation_id`    | `UUID`      | FK → `remediation_events.id` (NULL if clean).  |

**Index:** `UNIQUE (atom_id, entity_id, consumer_workspace)`.

### 6.5 `external_call_log`

Records every interaction with a third-party system, keyed by entity,
verb, and provider. Enables idempotency checks on rebuild.

| Column              | Type        | Description                                    |
|---------------------|-------------|------------------------------------------------|
| `id`                | `UUID`      | PK (`DEFAULT uuidv7()`).                       |
| `entity_id`         | `UUID`      | The entity.                                    |
| `verb_fqn`          | `TEXT`      | The DSL verb FQN that made the call.           |
| `provider`          | `TEXT`      | Provider identifier.                           |
| `operation`         | `TEXT`      | Operation name (e.g. `account_opening`).       |
| `external_ref`      | `TEXT`      | Provider's reference/ID for the created resource.|
| `request_hash`      | `BIGINT`    | Hash of the request parameters.                |
| `request_snapshot`  | `JSONB`     | Full request payload (for audit).              |
| `response_snapshot` | `JSONB`     | Full response payload (for audit).             |
| `created_at`        | `TIMESTAMPTZ` | When the call was made.                      |
| `superseded_by`     | `UUID`      | FK → self. Points to the correction record.    |
| `is_current`        | `BOOL`      | True for the latest call. Denormalised.        |

**Index:** `(entity_id, verb_fqn, provider) WHERE is_current = true`.

### 6.6 `compensation_records`

Audit trail for every external correction triggered by a rebuild. This is
the primary regulatory evidence table.

| Column              | Type        | Description                                    |
|---------------------|-------------|------------------------------------------------|
| `id`                | `UUID`      | PK (`DEFAULT uuidv7()`).                       |
| `remediation_id`    | `UUID`      | FK → `remediation_events.id`.                  |
| `entity_id`         | `UUID`      | The entity.                                    |
| `provider`          | `TEXT`      | Provider identifier.                           |
| `original_call_id`  | `UUID`      | FK → `external_call_log.id` (original).        |
| `correction_call_id`| `UUID`      | FK → `external_call_log.id` (correction).      |
| `correction_type`   | `TEXT`      | CHECK: `amend` / `cancel_recreate` / `correction_filing` / `manual`. |
| `changed_fields`    | `JSONB`     | Diff of what changed between original and correction. |
| `outcome`           | `TEXT`      | CHECK: `success` / `pending` / `failed`.       |
| `confirmed_at`      | `TIMESTAMPTZ` | When provider confirmed the correction.       |
| `confirmed_by`      | `TEXT`      | User or system that confirmed.                 |
| `created_at`        | `TIMESTAMPTZ` | When the compensation was initiated.          |

### 6.7 `remediation_events`

First-class lifecycle entity for tracking the resolution of cross-workspace
state drift.

| Column              | Type        | Description                                    |
|---------------------|-------------|------------------------------------------------|
| `id`                | `UUID`      | PK (`DEFAULT uuidv7()`).                       |
| `entity_id`         | `UUID`      | The entity affected.                           |
| `source_atom_id`    | `UUID`      | FK → `shared_atom_registry.id`.                |
| `source_workspace`  | `TEXT`      | Workspace that made the change.                |
| `prior_version`     | `INT`       | Version before the change.                     |
| `new_version`       | `INT`       | Version after the change.                      |
| `affected_workspace`| `TEXT`      | Consuming workspace affected.                  |
| `affected_constellation_family` | `TEXT` | Consuming constellation family affected. |
| `dirty_vertex_count`| `INT`       | Number of vertices in the dirty set.           |
| `dirty_vertices`    | `JSONB`     | Array of vertex IDs and their impact classification. |
| `status`            | `TEXT`      | CHECK: `detected` / `assessed` / `resolving` / `resolved` / `escalated` / `deferred`. |
| `impact_classification` | `TEXT`  | CHECK: `compatible` / `breaking` / `regulatory`. |
| `deferral_reason`   | `TEXT`      | NULL unless status = `deferred`.               |
| `resolved_at`       | `TIMESTAMPTZ` | When final state was reached.                |
| `resolved_by`       | `UUID`      | User who resolved/deferred.                    |
| `created_at`        | `TIMESTAMPTZ` | When the remediation event was detected.      |

**Index:** `(entity_id, status)` — find open remediation events per entity.

### 6.8 `provider_capabilities`

Reference data classifying third-party provider operations for rebuild
behaviour.

| Column              | Type        | Description                                    |
|---------------------|-------------|------------------------------------------------|
| `id`                | `UUID`      | PK (`DEFAULT uuidv7()`).                       |
| `provider`          | `TEXT`      | Provider identifier.                           |
| `operation`         | `TEXT`      | Operation name.                                |
| `capability`        | `TEXT`      | CHECK: `amendable` / `cancel_and_recreate` / `immutable` / `manual`. |
| `amend_details`     | `JSONB`     | Endpoint, method, constraints for amendment.   |
| `notes`             | `TEXT`      | Human-readable notes on provider behaviour.    |

**Index:** `UNIQUE (provider, operation)`.

---

## 7. Entity Relationship Summary

```
shared_atom_registry
    │
    ├──< shared_atom_consumers
    │
    ├──< shared_fact_versions
    │       │
    │       └── (entity_id, version) ──> value at that version
    │              └── mutated_by_execution ──> dsl_idempotency.execution_id
    │
    └──< workspace_fact_refs
            │
            └── held_version vs shared_fact_versions.current
                    │
                    └──> stale? → remediation_events
                                      │
                                      ├──< compensation_records
                                      │        │
                                      │        ├── original_call ──> external_call_log
                                      │        └── correction_call ──> external_call_log
                                      │
                                      └── status FSM (§4.7)

provider_capabilities (reference data, joined at runtime)
```

---

## 8. Operational Scenarios

### 8.1 LEI Correction — Onboarding In-Flight

1. KYC workspace: `correct_lei(entity_id, new_lei)` verb executes.
2. Post-commit hook writes `shared_fact_versions` row (version N+1).
3. `mark_dirty` walks platform DAG. Finds CBU and OnBoarding consumers. Entity is
   in-flight (onboarding not complete).
4. `workspace_fact_refs` rows for CBU and OnBoarding marked `stale`.
5. Next verb execution in CBU workspace: `check_staleness` fires in pre-REPL.
   Verb returns `StatePreconditionError::StaleSharedFact`.
6. NarrationEngine surfaces blocker: "LEI was corrected in KYC — acknowledge
   update before continuing." User/Sage runs
   `acknowledge_shared_update(entity_id, "entity.lei")`.
7. `workspace_fact_refs.held_version` bumped to N+1. Status → `clean`.
8. CBU verbs resume with new LEI flowing through input resolution.

### 8.2 LEI Correction — Onboarding Complete

1. Steps 1–3 as above. Entity state is `complete`.
2. `mark_dirty` creates `remediation_events` row (status: `detected`).
3. Auto-assessment: `mark_dirty` computes dirty vertex set (4 of 47 onboarding
   vertices depend on LEI). Classifies impact.
4. All 4 vertices classified `compatible` (new LEI is same jurisdiction).
   Remediation status → `assessed`, impact → `compatible`.
5. Remediation status → `resolving`. `rebuild_dirty` executes:
   - Vertex 1: `setup_sub_custodian_lei_ref` — internal only. Re-executes,
     picks up new LEI. Clean.
   - Vertex 2: `register_settlement_instructions` — external call.
     `execute_with_idempotency` finds prior call. Input hash differs.
     Provider capability: `amendable`. Issues PUT to amend. Compensation
     record created. Clean.
   - Vertex 3: `validate_regulatory_classification` — internal. Re-executes.
     Clean.
   - Vertex 4: `generate_confirmation_pack` — internal. Re-executes with
     corrected LEI in document. Clean.
6. All vertices clean. Remediation status → `resolved`.

### 8.3 LEI Correction — Breaking Change

1. Steps 1–4 as §8.2, but new LEI resolves to a different jurisdiction.
2. Vertex 1: `setup_sub_custodian_lei_ref` — re-executes. Sub-custodian
   for new jurisdiction is different entity. Verb fails: no sub-custodian
   relationship exists for new jurisdiction.
3. Vertex remains dirty. Remediation status → `escalated`.
4. Human reviews. Options:
   - Establish new sub-custodian relationship, re-run rebuild.
   - Defer remediation (document reason for regulatory record).
   - Revert KYC change (LEI correction was itself incorrect).

### 8.4 LEI Correction — Manual Provider

1. Steps 1–4 as §8.2.
2. Vertex 2: `register_settlement_instructions` — external call. Provider
   capability: `manual`. Verb returns `ManualInterventionRequired`.
3. Remediation status → `escalated`. Dirty vertices include the manual
   intervention marker with provider details and changed field diff.
4. Human contacts provider (phone/email). Confirms correction externally.
5. Human runs `confirm_external_correction(entity_id, vertex_id, provider_ref)`.
6. Compensation record created with `correction_type: manual`. Vertex clean.
7. Remaining dirty vertices rebuild. Remediation status → `resolved`.

---

## 9. Design Decisions & Open Questions

### 9.1 Decided

| # | Decision | Rationale |
|---|----------|-----------|
| D-01 | Anchor on attributes, not verbs. | O(shared_atoms) ≪ O(verbs). Ownership is a property of data, not behaviour. Verb `reads_from` provides consumption for free. |
| D-02 | Two-level DAG, not separate registry. | Same structural primitive (vertex, edge, traversal) at different elevation. Level 1 is YAML-driven (`SlotDef.depends_on`); Level 0 is table-driven (`shared_atom_consumers`). Same algorithm, different storage. |
| D-03 | `make`-style rebuild, not reactive event bus. | Post-commit marking + explicit rebuild is simpler, auditable, and compatible with custody banking's "human in the loop" requirements. No real-time sync needed. |
| D-04 | Dirty-flag, not version-vector. | Shared atom count is O(50). Version vectors are O(workspace²) overhead with no benefit at this scale. |
| D-05 | External idempotency at verb level, not DAG level. | DAG doesn't know about external systems. Verbs encapsulate their own I/O. Provider capability is a verb concern. |
| D-06 | `Deferred` is a valid terminal state. | Custody banking requires explicit, auditable "do nothing" decisions. Divergence is sometimes the legally correct state. |
| D-07 | Boot-time discovery + YAML override. | Matches existing platform patterns (ScenarioIndex, MacroIndex all load at boot). Boot-time scan of verb footprints supplemented by explicit YAML declarations. (Was Q-01) |
| D-08 | Platform DAG computed at boot, not persisted. | Matches existing pattern — constellation maps are YAML-computed, not persisted. At O(50) shared atoms, computed is fast enough. (Was Q-04) |
| D-09 | Staleness surfaces via NarrationEngine. | NarrationEngine already handles "what's next" / "what's blocking". `StaleSharedFact` surfaces as a `NarrationPayload.blockers` entry. (Was Q-05) |
| D-10 | Async post-commit `mark_dirty`. | Matches existing `propagate_derived_chain_staleness` which is trigger-based, not in-transaction. Avoids adding latency to KYC verbs. (Was Q-07) |
| D-11 | `RebuildContext` extends `ReplayEnvelope`. | Existing runbook replay infrastructure provides the execution envelope. `RebuildContext` adds shared-fact-correction metadata rather than introducing a parallel type. |

### 9.2 Open Questions for Peer Review

| # | Question | Context |
|---|----------|---------|
| Q-02 | **Granularity of dirty propagation: atom-level or vertex-level?** Current design marks individual vertices dirty. Alternative: mark entire constellation dirty and let runbook re-execution determine what actually changed. Finer granularity = less re-work but more tracking overhead. | §4.2, §5.1 |
| Q-03 | **Auto-rebuild for `compatible` classification: require human approval or execute automatically?** Regulatory risk tolerance question, not a technical one. Auto-rebuild is faster; approval gate is safer. May vary by jurisdiction (LU vs IE vs UK vs US). | §4.7, §8.2 |
| Q-06 | **Compensation record retention policy.** Regulatory requirements vary by jurisdiction. UCITS may require 10-year retention. Need to confirm per-jurisdiction requirements and align with platform archival strategy. | §6.6 |
| Q-08 | **Cascade depth limit.** Should transitive dirty propagation have a maximum depth? In theory, the DAG is acyclic so propagation terminates. In practice, a deep cascade across multiple constellations could flag hundreds of vertices. Is there a circuit-breaker threshold? | §5.1 |

---

## 10. Implementation Phasing

Recommended implementation order, aligned with ob-poc Codex TODO conventions:

| Phase | Deliverable | Dependencies |
|-------|-------------|-------------|
| P1    | `shared_atom_registry` + `shared_fact_versions` tables. Migration. | None. |
| P2    | `shared_atom_consumers` table. Boot-time discovery scan from verb footprints (`VerbContractBody.reads_from`). | P1. |
| P3    | `workspace_fact_refs` table. `check_staleness` pre-REPL hook. NarrationEngine blocker integration. | P1, P2. |
| P4    | `mark_dirty` function. Post-commit trigger on shared-atom-mutating verbs (extends `propagate_derived_chain_staleness` pattern). | P1, P2, P3. |
| P5    | `rebuild_dirty` function. `RebuildContext` extending `ReplayEnvelope`. Topological re-execution. | P4. |
| P6    | `remediation_events` table + FSM YAML (`remediation_event_lifecycle.yaml`). Status transitions. | P4, P5. |
| P7    | `external_call_log` table. `execute_with_idempotency` wrapper. | P5. |
| P8    | `provider_capabilities` reference data. Amendment/correction branching. | P7. |
| P9    | `compensation_records` table. Audit trail generation. | P7, P8. |
| P10   | Shared atom YAML declarations for initial set (LEI, jurisdiction, fund structure type, UBO threshold, regulatory classification). | P1, P2. |

**E-invariant:** Each phase must reach 100% before proceeding. Progress
gates at every phase boundary. Do not commit — Adam reviews the diff first.

---

## 11. Glossary

| Term | Definition |
|------|------------|
| **Shared Atom** | An attribute whose value is owned by one workspace but consumed by one or more other workspaces. The unit of cross-workspace dependency. |
| **Platform DAG** | Level 0 DAG. Vertices are shared atoms; edges are cross-workspace dependencies with hard/soft constraint types. Stored in `shared_atom_consumers` table. |
| **Constellation DAG** | Level 1 DAG. Existing model. Intra-workspace verb dependency graph. Schema-less (YAML-driven via `SlotDef.depends_on`). |
| **Dirty Flag** | Marker on a vertex indicating its output was built on a shared fact version that has since been superseded. |
| **Rebuild** | Re-execution of dirty vertices in topological order with corrected inputs. Analogous to `make` recompilation. Uses existing runbook execution pipeline with `RebuildContext` extending `ReplayEnvelope`. |
| **Idempotency Envelope** | Wrapper around external-calling verbs that distinguishes first-run from rebuild and selects the appropriate correction pattern. |
| **Compensation Record** | Audit trail entry documenting an external correction triggered by a rebuild. |
| **Remediation Event** | First-class SemOS entity tracking the lifecycle of a cross-workspace state drift, from detection through resolution or deferral. FSM in `remediation_event_lifecycle.yaml`. |
| **Provider Capability** | Classification of a third-party system's ability to accept corrections: amendable, cancel-and-recreate, immutable, or manual. |

---

## 12. Codebase References

Key files referenced in this document for implementors:

| Concept | File | Notes |
|---------|------|-------|
| Verb contract R/W sets | `rust/crates/sem_os_core/src/verb_contract.rs:54-58` | `reads_from`, `writes_to` fields |
| Constellation slot dependencies | `rust/crates/sem_os_core/src/constellation_map_def.rs:37` | `depends_on: Vec<DependencyEntry>` |
| DependencyEntry enum | `rust/crates/sem_os_core/src/constellation_map_def.rs:88-110` | Simple + Explicit variants |
| WorkspaceKind enum | `rust/src/repl/types_v2.rs:90-98` | 7 workspace variants |
| Workspace constellation families | `rust/src/repl/types_v2.rs:130-237` | Per-workspace family lists |
| ObjectType enum | `rust/crates/sem_os_core/src/types.rs:82-106` | 23 current variants |
| Derived staleness propagation | `rust/src/derived_attributes/repository.rs:263-326` | `mark_stale_by_input()` |
| SQL staleness function | `migrations/master-schema.sql` | `propagate_derived_chain_staleness()` |
| Execution records | `migrations/master-schema.sql` | `dsl_idempotency` table |
| Runbook plan compiler | `rust/src/runbook/plan_compiler.rs` | `compile_runbook_plan()` |
| Runbook plan executor | `rust/src/runbook/plan_executor.rs` | `advance_plan_step()` |
| ReplayEnvelope | `rust/src/runbook/types.rs` | Exact-replay determinism boundary |
| State machine YAML pattern | `rust/config/sem_os_seeds/state_machines/attribute_def_lifecycle.yaml` | Pattern for new FSMs |
| NarrationEngine | `rust/src/agent/narration_engine.rs` | Blocker/suggested_next surface |
| AttributeDefBody | `rust/crates/sem_os_core/src/attribute_def.rs:18-61` | Including `visibility` field |

---

*End of document. Submitted for peer review.*
