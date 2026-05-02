# Cross-Workspace State Consistency Architecture

**Document:** `cross-workspace-state-consistency.md`
**Version:** 0.4 — DRAFT FOR PEER REVIEW
**Author:** Adam (Lead Solution Architect) / Claude (Architecture Partner)
**Date:** 2026-04-02
**Status:** Vision & Architecture — Pre-Implementation
**Repo:** `ob-poc` (https://github.com/adamtc007/ob-poc)

---

## Revision History

| Version | Date       | Author | Summary |
|---------|------------|--------|---------|
| 0.1     | 2026-04-02 | Adam/Claude | Initial vision, scope, architecture |
| 0.2     | 2026-04-02 | Adam/Claude | Aligned with codebase: corrected terminology, noted platform DAG is new infrastructure, referenced existing staleness propagation pattern, resolved Q-01/Q-04/Q-05/Q-07, added §3.5 (existing infrastructure reuse) |
| 0.3     | 2026-04-02 | Adam/Claude | Peer review incorporation: established canonical dirty unit (shared attribute version) as architectural invariant, disciplined terminology (superseded/stale/rebuild-candidate), three-stage propagation, consumer granularity layering, rebuild as controlled re-evaluation |
| 0.4     | 2026-04-02 | Adam/Claude | Major simplification: removed soft constraint type (shared atoms are always hard — either it matters or it doesn't), replaced vertex-level rebuild with constellation replay from top (upsert semantics make fine-grained targeting unnecessary), removed rebuild candidate tracking, added shared atom lifecycle FSM and DSL verb surface for registry governance, reduced data model from 8 tables to 6 |

---

## 1. Vision

ob-poc operates a multi-workspace, constellation-governed platform where entity
lifecycle management spans several autonomous domains — KYC, OnBoarding,
CBU, Deal, Instrument Matrix, Product Maintenance, and SemOS Maintenance.
Each workspace governs its own constellation DAG: a directed acyclic graph
of entity-state atoms with dependency edges that define verb execution order,
staleness propagation, and lifecycle gating.

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

1. Declares shared atoms and their ownership at the SemOS registry level,
   governed by DSL verbs with a lifecycle FSM.
2. Extends the existing constellation DAG model upward into a platform-level
   DAG derived on-the-fly from Level 1 structures.
3. Detects supersession of shared attribute versions and propagates staleness
   to consuming workspaces.
4. Restores consistency by replaying consuming constellations from the top,
   relying on upsert semantics for unchanged state and the idempotency
   envelope for external calls.
5. Produces a full compensation audit trail suitable for regulatory review.

The guiding principle: **drift is not prevented — it is made visible, typed,
and mechanically resolvable.** The platform does not attempt to keep all
workspaces synchronised in real-time. It detects divergence, replays the
affected constellation, and lets upsert + idempotency handle the rest.

---

## 2. Scope

### 2.1 In Scope

- Shared atom declaration, ownership, lifecycle, and DSL verb governance.
- Platform-level DAG (Level 0) derived on-the-fly from shared atom registry
  + Level 1 verb footprints.
- Staleness propagation from shared fact versions through consumer references.
- Constellation replay from the top for consuming workspaces with stale
  references (upsert semantics handle unchanged state).
- External call idempotency envelope for third-party interactions.
- Provider capability classification (amendable, cancel-recreate, immutable,
  manual).
- Compensation log and audit trail for regulatory compliance.
- Remediation event lifecycle for constellations that cannot be automatically
  replayed.

### 2.2 Out of Scope

- Real-time cross-workspace event streaming or pub/sub (not required;
  post-commit propagation model is sufficient).
- Changes to intra-workspace constellation DAG structure or verb contracts.
- Vertex-level rebuild targeting (unnecessary given upsert semantics).
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
  confirm this.
- **Verbs use upsert semantics.** Re-running a verb with unchanged inputs
  produces no database mutation (upsert writes the same value). Re-running
  with changed inputs produces the correct update. This eliminates the need
  for fine-grained dependency tracking to minimise the rebuild set.
- Verb input resolution reads from the current fact store at execution time.
  A replayed verb will pick up the latest version of any shared atom.
- The existing constellation DAG traversal algorithm (via `SlotDef.depends_on`
  and `DependencyEntry` in `constellation_map_def.rs`) is generic and can
  inform cross-workspace edge design without structural modification.
- External system interactions are a minority of total verb executions
  (~10%) but represent the highest-risk surface for replay correctness.
  These are the only calls where replay is not inherently idempotent.

---

## 3. Architecture

### 3.1 The `make` Analogy

The rebuild model is directly analogous to the UNIX `make` utility, with
one simplification: because verbs upsert, we do not need to compute minimal
rebuild sets. We replay the full constellation and let upsert semantics
handle the "already up to date" case — equivalent to `make` with every
object file rebuilt but identical objects producing no-op writes.

| `make` Concept          | ob-poc Equivalent                                    |
|-------------------------|------------------------------------------------------|
| Source file (`.h`)      | Shared attribute version (LEI v2, jurisdiction v3)    |
| Object file (`.o`)      | Consumer-held state (completed verb output)           |
| Dependency graph (`Makefile`) | Platform DAG (derived) + constellation DAGs     |
| Timestamp comparison    | Version comparison (`held_version` vs `current_version`) |
| Recompilation           | Constellation replay with `RebuildContext`             |
| `make clean && make`    | Replay from top (upsert = no-op for unchanged state)  |
| Build failure           | Replay verb fails → constellation stale → escalate    |

Key properties:

- **Deterministic.** Replay uses the same runbook as initial execution.
- **Correct by construction.** Upsert + replay from top guarantees all
  downstream state reflects the current shared fact version.
- **Resumable.** If replay fails partway, the cursor position is known.
  Re-run picks up where it stopped.
- **Idempotent (internal).** Upsert with unchanged inputs = no mutation.

### 3.2 Two-Level DAG Hierarchy

```
┌─────────────────────────────────────────────────────┐
│                  LEVEL 0: PLATFORM DAG              │
│                                                     │
│   Derived on-the-fly from shared atom registry      │
│   + Level 1 verb footprints.                        │
│                                                     │
│   Vertices: Shared attribute versions               │
│   Edges:    Cross-workspace consumption deps        │
│   Scope:    Platform-wide (~30–50 nodes)            │
│   Lifecycle: Computed at boot and on registry change │
│                                                     │
│   ┌──────────┐            ┌──────────────────┐      │
│   │entity.lei├───────────►│cbu constellation │      │
│   │ (KYC)    ├───────────►│onboarding const. │      │
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

**The Level 0 DAG is a computed view, not a persisted structure.** It is
derived from the intersection of the shared atom registry and Level 1 verb
footprints:

```
For each shared_atom in registry (status = Active):
    owner = atom.owner_workspace
    backing_table = atom.resolve_backing_table()
    
    For each workspace W ≠ owner:
        For each verb V in W where V.reads_from includes backing_table:
            → edge exists: atom → W.constellation_family
```

This computation runs at boot and after any registry change. It is
O(shared_atoms × verb_count) — trivially fast at ~50 × ~1,464.

### 3.3 Shared Atom Discovery

Verb contracts (`VerbContractBody` in `sem_os_core/src/verb_contract.rs`)
declare their data footprint via `reads_from: Vec<String>` and
`writes_to: Vec<String>`. These are **table-level** declarations (e.g.,
`"entities"`, `"cbus"`), not attribute-level paths.

**Bootstrap limitation:** Table-level granularity means discovery is coarse —
a verb that reads the `entities` table is flagged as a consumer of all shared
atoms backed by that table. This is conservative (over-declaration is safe;
under-declaration is not) and is explicitly scoped as a **bootstrap
mechanism** (D-14). Combined with upsert replay from the top, coarse
discovery has zero cost: over-declared consumers replay their constellation,
upserts no-op for unchanged state, net effect is correct with minimal waste.

Long-term direction is attribute-level or SemOS-declared fact-read contracts
on `VerbContractBody`.

### 3.4 Existing Infrastructure Reuse

The cross-workspace staleness mechanism builds on an existing pattern:
**derived attribute staleness propagation**. The current system already
implements:

- `derived_attribute_values.stale: BOOLEAN` — staleness flag on derived values
- `derived_attribute_dependencies` table — dependency edges between values
- `propagate_derived_chain_staleness(p_attr_id, p_entity_id)` SQL function —
  transitive closure walk that marks all dependents stale
- `mark_stale_by_input()` Rust function (`src/derived_attributes/repository.rs`)

The cross-workspace staleness propagation (§5.1) extends this same pattern
from attribute-scoped dependencies to workspace-scoped fact references.

### 3.5 Architectural Invariants

**INV-1: The canonical unit of drift is the shared attribute version.**
A shared attribute becomes *superseded* when a new version is committed by
its owning workspace. All downstream staleness is a derivative projection
of this fact-level change.

**INV-2: Consumer state is projection; shared fact version is source of truth.**
The `shared_fact_versions` table is the authoritative record. The
`workspace_fact_refs` table is a consumption-state projection — pointers
into the shared fact version history.

**INV-3: Replay scope is the consuming constellation, not individual vertices.**
A superseded shared attribute triggers a full constellation replay for each
consuming workspace. Upsert semantics guarantee that unchanged state is a
no-op and changed state flows through correctly. There is no vertex-level
rebuild targeting.

**INV-4: Replay routes through the existing runbook execution gate.**
No special execution path exists for replays. The standard verb execution
pipeline with `RebuildContext` extending `ReplayEnvelope` is the single
point of entry for all state transitions, whether initial or corrective.

**INV-5: Replay is controlled re-evaluation, not mechanical rerun.**
The corrected attribute may cause state transitions that were previously
valid to become gated out, policy-blocked, or structurally invalid. The
replay pipeline evaluates each step against the current policy and lifecycle
state. Verbs may return new failure modes that did not exist during initial
execution.

**INV-6: If an attribute is shared, it is always enforced.**
There is no "soft" constraint type. An attribute is either in the shared
atom registry (and supersession triggers the full propagation and replay
pipeline) or it is not (and it is a regular attribute with no cross-workspace
implications). The registry admission decision is the only gate.

### 3.6 Shared Atom Lifecycle

Shared atoms are first-class SemOS entities with a governed lifecycle:

```
    ┌───────┐
    │ Draft │
    └───┬───┘
        │ activate_shared_atom
        ▼
    ┌────────┐
    │ Active │◄──── enforce: full propagation pipeline
    └───┬────┘
        │ deprecate_shared_atom
        ▼
    ┌────────────┐
    │ Deprecated │◄── still enforced, no new consumers
    └───┬────────┘
        │ retire_shared_atom
        ▼
    ┌─────────┐
    │ Retired │◄──── historical only, no propagation
    └─────────┘
```

**Draft:** Declared but not yet enforced. Consumer discovery runs (so you
can verify the topology), but supersession does not trigger propagation or
replay. Safe onramp for new shared atoms.

**Active:** Full enforcement. Every supersession triggers the three-stage
propagation pipeline. This is the steady-state for production shared atoms.

**Deprecated:** Still enforced — existing consumers are protected. But
flagged for removal. `register_shared_atom_consumer` is blocked. Used when
retiring a shared attribute from cross-workspace scope.

**Retired:** Deregistered from active enforcement. Historical version
records retained in `shared_fact_versions` for audit. No propagation on
mutation. The attribute reverts to being a regular workspace-local field.

The FSM is declared as `shared_atom_lifecycle.yaml` in
`rust/config/sem_os_seeds/state_machines/`.

### 3.7 Shared Atom DSL Verb Surface

The shared atom registry is governed through SemOS DSL verbs. YAML serves
as seed data at boot (via `register_shared_atom` if not already present).
Runtime changes go through the DSL with full audit trail.

**Registry management** (SemOS Maintenance workspace):

| Verb | Purpose | Lifecycle Gate |
|------|---------|----------------|
| `register_shared_atom(path, owner_workspace, owner_family)` | Declare a new shared atom. Enters `Draft` state. | — |
| `activate_shared_atom(atom_id)` | Promote to `Active`. Propagation enforcement begins. | Draft → Active |
| `deprecate_shared_atom(atom_id)` | Mark for retirement. No new consumers. | Active → Deprecated |
| `retire_shared_atom(atom_id)` | Remove from active enforcement. | Deprecated → Retired |
| `list_shared_atom_consumers(atom_id)` | Show discovered consumers (read-only). | Any state |

**Remediation lifecycle** (consuming workspace):

| Verb | Purpose | Lifecycle Gate |
|------|---------|----------------|
| `acknowledge_shared_update(entity_id, atom_path)` | Advance consumer ref to current version (in-flight entities). | Consumer ref stale |
| `replay_constellation(entity_id, constellation_family)` | Trigger full constellation replay for stale entity (complete entities). | Remediation detected |
| `defer_remediation(entity_id, remediation_id, reason)` | Explicitly accept divergence. | Escalated |
| `revoke_deferral(entity_id, remediation_id)` | Re-open deferred remediation. | Deferred |
| `confirm_external_correction(entity_id, remediation_id, provider_ref)` | Record manual provider correction. | Escalated |

**Operational queries** (any workspace with read access):

| Verb | Purpose |
|------|---------|
| `list_stale_consumers(entity_id?)` | Show all stale consumer references, optionally filtered by entity. |
| `list_open_remediations(entity_id?, workspace?)` | Show unresolved remediation events. |

---

## 4. Capabilities

### 4.1 Shared Atom Declaration

Shared atoms are declared via DSL verb or seeded from YAML at boot.

```yaml
# semos/shared_atoms/entity_lei.yaml (seed data)
shared_atom:
  path: "entity.lei"
  display_name: "Legal Entity Identifier"
  owner:
    workspace: kyc
    constellation_family: kyc_workspace
  validation:
    format: "^[0-9A-Z]{20}$"  # ISO 17442
    gleif_verification: required
```

At boot: if `atom_path` not in registry, `register_shared_atom` is invoked
automatically and the atom enters `Draft`. Promotion to `Active` is an
explicit DSL action — no shared atom is enforced without deliberate
activation.

Consumer discovery is automatic (§3.3) and runs for atoms in any lifecycle
state. This allows verification of the consumer topology while the atom is
still in `Draft`.

### 4.2 Staleness Propagation from Shared Facts

When a shared attribute is superseded (by any verb in the owning workspace
committing a new version), the platform triggers a post-commit staleness
propagation. This is a three-stage process.

**Stage 1 — Attribute version superseded.**
The owning verb commits a new version to `shared_fact_versions`. The prior
version's `is_current` flag is cleared. This is the origin event.

**Stage 2 — Consumer references become stale.**
The propagation function identifies all consuming workspaces (from the
derived platform DAG). For each consumer × entity pair, the
`workspace_fact_refs` row is marked `stale`. This is a data-plane update —
no verb execution, no constellation traversal.

**Stage 3 — Remediation triggered (conditional on entity state).**
For each stale consumer:

| Consumer Entity State | Action |
|-----------------------|--------|
| **In-flight** | Stop at Stage 2. Pre-REPL staleness check (§4.4) will intercept the next verb execution. |
| **Complete / Active** | Create remediation event. The event references the consuming constellation + entity. Replay is triggered per remediation lifecycle (§4.5). |

Propagation is a single post-commit walk. It mirrors the existing
`propagate_derived_chain_staleness()` pattern — trigger-based, not
in-transaction.

**Only atoms with lifecycle status `Active` or `Deprecated` trigger
propagation.** `Draft` and `Retired` atoms are inert.

### 4.3 Constellation Replay

When a consuming constellation has stale references for a complete entity,
the resolution is a full constellation replay from the top.

1. Load the runbook for the consuming constellation + entity.
2. Attach `RebuildContext` (extending `ReplayEnvelope`) with supersession
   metadata (source atom, prior version, new version).
3. Execute `plan_executor.replay_from_gate(runbook, constellation_entry, context)`.
4. Each verb resolves inputs from the current fact store (picking up the
   new shared attribute version).
5. **Verbs with unchanged inputs:** upsert writes the same value. No
   database mutation. Effectively a no-op.
6. **Verbs with changed inputs (due to superseded attribute):** upsert
   writes the correct new value. State updated.
7. **External-calling verbs:** hit the idempotency envelope (§4.5).
   First-run vs. rebuild branching applies.
8. **Verb failure:** replay halts at that step. Cursor position recorded.
   Remediation event escalated. This is INV-5 — the corrected fact-world
   may invalidate previously valid state.

On successful completion: `workspace_fact_refs.held_version` advanced to
current. Status → `current`. Remediation event → `resolved`.

This is not a new execution path. It is the existing runbook executor
with a different starting trigger and `RebuildContext` metadata.

### 4.4 Pre-REPL Staleness Check (In-Flight Entities)

For entities where the consuming workspace has active (non-complete) state,
the verb execution pipeline gains one additional precondition:

```
For each table in verb_contract.reads_from:
    For each shared_atom backed by that table:
        If workspace_fact_refs.held_version < current_version:
            → return StatePreconditionError::StaleSharedFact
```

This check runs in the pre-REPL phase. When it fires, the `NarrationEngine`
surfaces it as a blocker in `NarrationPayload.blockers`, explaining what
changed and what action is needed (`acknowledge_shared_update`).

### 4.5 External Call Idempotency Envelope

Verbs that interact with third-party systems require special handling on
replay. The verb distinguishes between first-run and re-run via the external
call log.

```
On verb execution:
    Check external_call_log for (entity_id, verb_id, provider):
        No prior record → first run:
            Execute external call
            Record in external_call_log
        Prior record exists, input hash differs → replay with changes:
            Execute amendment/correction per provider capability
            Supersede prior record
            Create compensation_record
        Prior record exists, input hash matches → no-op:
            Skip external call (already correct)
```

### 4.6 Provider Capability Classification

Provider capabilities are declared in SemOS metadata:

| Capability           | Behaviour on Replay                                      | Example Providers           |
|----------------------|----------------------------------------------------------|-----------------------------|
| `amendable`          | `PUT`/amend on existing resource. Preferred path.        | Modern custody APIs, SWIFT gpi |
| `cancel_and_recreate`| Cancel prior, create new. Two-step correction.           | Some settlement systems      |
| `immutable`          | Cannot modify. Issue correction referencing original.     | SWIFT MT messages, regulatory filings |
| `manual`             | No API for corrections. Human intervention required.      | Legacy systems, phone/email  |

Classification is per-provider, per-operation:

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

When a constellation replay is triggered for a complete entity, a
Remediation Event tracks the lifecycle. It is anchored to a specific
shared attribute supersession.

```
                 ┌──────────┐
                 │ Detected │
                 └────┬─────┘
                      │ replay_constellation (auto or manual)
                      ▼
                 ┌───────────┐
            ┌────┤ Replaying ├────┐
            │    └───────────┘    │
            │                     │
            ▼                     ▼
     ┌──────────┐          ┌────────────┐
     │ Resolved │          │ Escalated  │
     └──────────┘          └──┬────┬───┘
                              │    │
                              ▼    ▼
                       ┌──────────┐ ┌──────────┐
                       │ Resolved │ │ Deferred │
                       └──────────┘ └──────────┘
```

- **Detected → Replaying**: Constellation replay initiated.
- **Replaying → Resolved**: All verbs replayed successfully. Consumer ref
  advanced. No external corrections failed.
- **Replaying → Escalated**: Replay halted (verb failure or manual provider).
  Human review required.
- **Escalated → Resolved**: Human has resolved the issue (established
  missing relationships, confirmed external corrections, etc.).
- **Escalated → Deferred**: Explicit human decision to accept divergence
  from a specific shared attribute version. Compliance-auditable. Not a
  system failure.

The FSM is declared as `remediation_event_lifecycle.yaml` in
`rust/config/sem_os_seeds/state_machines/`.

### 4.8 The "Do Nothing" Path

The `Deferred` state is architecturally significant. What is being deferred
is a known divergence from a specific shared attribute version — the
remediation event records the exact attribute, old and new versions, and the
affected constellation.

`defer_remediation(entity_id, remediation_id, reason)` records the reason,
the authorising user, and the timestamp. The stale consumer reference remains
flagged but replay is not triggered until the deferral is revoked.

---

## 5. Core Functions

### 5.1 `propagate_shared_fact_staleness`

**Input:** Changed atom reference (path + entity_id + new version).
**Output:** Per-consumer staleness result + remediation events for complete entities.
**Trigger:** Post-commit hook on any verb that mutates an Active shared atom.

```
propagate_shared_fact_staleness(changed_atom) → PropagationResult:

    // ── STAGE 1: Attribute version superseded ──
    new_version = shared_fact_versions.current(changed_atom.path, entity_id)

    // ── STAGE 2: Consumer references become stale ──
    stale_consumers = empty
    for consumer in platform_dag.consumers_of(changed_atom):
        ref = workspace_fact_refs.lookup(
            changed_atom.id, entity_id, consumer.workspace)
        if ref.held_version < new_version:
            ref.status = 'stale'
            ref.stale_since = now()
            stale_consumers.add((consumer, ref))

    // ── STAGE 3: Create remediation events for complete entities ──
    remediations = empty
    for (consumer, ref) in stale_consumers:
        match get_entity_state(entity_id, consumer.workspace):
            InFlight → continue  // pre-REPL check handles this
            Complete | Active →
                remediation = create_remediation_event(
                    entity_id, changed_atom,
                    ref.held_version, new_version,
                    consumer.workspace, consumer.constellation_family)
                remediations.add(remediation)

    return PropagationResult { stale_consumers, remediations }
```

### 5.2 `replay_stale_constellation`

**Input:** Remediation event (constellation + entity reference).
**Output:** Replay result (resolved / escalated).
**Trigger:** Explicit invocation (auto or human-approved, per §4.7).

```
replay_stale_constellation(remediation) → ReplayResult:
    runbook = load_runbook(
        remediation.constellation_family, remediation.entity_id)
    context = RebuildContext::from_replay_envelope(
        trigger: SharedFactSupersession,
        source_atom: remediation.source_atom,
        prior_version: remediation.prior_version,
        new_version: remediation.new_version,
    )

    // This is the EXISTING executor — not new code
    result = plan_executor.replay_from_gate(
        runbook, RunbookGate::ConstellationEntry, context)

    match result:
        Ok(_):
            workspace_fact_refs.advance(
                remediation.atom_id, remediation.entity_id,
                remediation.consumer_workspace, remediation.new_version)
            workspace_fact_refs.status = 'current'
            remediation.status = 'resolved'
            return ReplayResult::Resolved

        Err(step, error):
            // INV-5: corrected fact-world invalidated a state transition
            remediation.status = 'escalated'
            remediation.failed_at_step = step
            remediation.failure_reason = error
            return ReplayResult::Escalated(step, error)
```

### 5.3 `check_staleness` (Pre-REPL)

**Input:** Verb contract read set, shared atom registry, fact version store.
**Output:** Pass / `StatePreconditionError::StaleSharedFact`.
**Trigger:** Every verb execution (pre-REPL phase).

```
check_staleness(verb_contract, entity_id, workspace) → Result<(), PreconditionError>:
    for table in verb_contract.reads_from:
        for atom in shared_atom_registry.active_atoms_backed_by(table):
            ref = workspace_fact_refs.lookup(atom.id, entity_id, workspace)
            current = shared_fact_versions.current_version(atom.id, entity_id)
            if ref.held_version < current:
                return Err(StaleSharedFact {
                    atom: atom.path,
                    held: ref.held_version,
                    current: current,
                    owner: atom.owner_workspace,
                })
    return Ok(())
```

### 5.4 `execute_with_idempotency` (External Call Wrapper)

**Input:** Verb execution context, external call log, provider capabilities.
**Output:** External call result + compensation record (if applicable).
**Trigger:** Verb body, for any operation classified as external.

```
execute_with_idempotency(verb_ctx, provider, operation, params):
    request_hash = hash(params)
    prior = external_call_log.lookup(entity_id, verb_id, provider)

    match (prior, prior.request_hash == request_hash):
        (None, _):
            result = provider.execute(operation, params)
            external_call_log.record(entity_id, verb_id, provider,
                                     result.external_ref, request_hash,
                                     result.response)
            return result

        (Some(prior), true):
            return prior.cached_result  // no-op

        (Some(prior), false):
            capability = provider_registry.capability(provider, operation)
            match capability:
                Amendable:
                    result = provider.amend(prior.external_ref, params)
                CancelAndRecreate:
                    provider.cancel(prior.external_ref)
                    result = provider.create(params)
                Immutable:
                    result = provider.submit_correction(
                        prior.external_ref, params)
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

### 6.1 `shared_atom_registry`

Declares shared atoms, their ownership, and lifecycle state.

| Column              | Type        | Description                                    |
|---------------------|-------------|------------------------------------------------|
| `id`                | `UUID`      | PK.                                            |
| `atom_path`         | `TEXT`      | Dot-notation attribute path. e.g. `entity.lei` |
| `display_name`      | `TEXT`      | Human-readable name.                           |
| `owner_workspace`   | `TEXT`      | Workspace that owns this atom.                 |
| `owner_constellation_family` | `TEXT` | Constellation family within owner workspace. |
| `lifecycle_status`  | `TEXT`      | `draft` / `active` / `deprecated` / `retired`. |
| `validation_rule`   | `JSONB`     | Format, external verification requirements.    |
| `created_at`        | `TIMESTAMPTZ` | Registry entry creation.                     |
| `activated_at`      | `TIMESTAMPTZ` | When promoted to Active (NULL if Draft).     |
| `updated_at`        | `TIMESTAMPTZ` | Last metadata update.                        |

**Index:** `UNIQUE (atom_path)`.

### 6.2 `shared_fact_versions`

**Source of truth** (INV-1, INV-2). Versioned fact store for shared atoms.
One row per entity × atom × version. All consumer-held versions are
references into this history.

| Column              | Type        | Description                                    |
|---------------------|-------------|------------------------------------------------|
| `id`                | `UUID`      | PK.                                            |
| `atom_id`           | `UUID`      | FK → `shared_atom_registry.id`.                |
| `entity_id`         | `UUID`      | The entity this fact belongs to.               |
| `version`           | `INT`       | Monotonically increasing version number.       |
| `value`             | `JSONB`     | The fact value at this version.                |
| `mutated_by_verb`   | `UUID`      | FK → verb execution record.                    |
| `mutated_by_user`   | `UUID`      | User who triggered the mutation.               |
| `mutated_at`        | `TIMESTAMPTZ` | When this version was committed.             |
| `is_current`        | `BOOL`      | True for the latest version. Denormalised.     |

**Indexes:** `UNIQUE (atom_id, entity_id, version)`.
`(atom_id, entity_id) WHERE is_current = true`.

### 6.3 `workspace_fact_refs`

**Consumption-state projection** (INV-2). Consumer-held pointers into the
version history maintained in `shared_fact_versions`. A stale row means the
consumer is operating against a superseded attribute version.

| Column              | Type        | Description                                    |
|---------------------|-------------|------------------------------------------------|
| `id`                | `UUID`      | PK.                                            |
| `atom_id`           | `UUID`      | FK → `shared_atom_registry.id`.                |
| `entity_id`         | `UUID`      | The entity.                                    |
| `consumer_workspace`| `TEXT`      | The consuming workspace.                       |
| `held_version`      | `INT`       | Version last operated against.                 |
| `status`            | `TEXT`      | `current` / `stale` / `deferred`.              |
| `stale_since`       | `TIMESTAMPTZ` | When staleness was detected (NULL if current). |
| `remediation_id`    | `UUID`      | FK → `remediation_events.id` (NULL if current). |

**Index:** `UNIQUE (atom_id, entity_id, consumer_workspace)`.

### 6.4 `external_call_log`

Records every third-party interaction. Enables idempotency on replay.

| Column              | Type        | Description                                    |
|---------------------|-------------|------------------------------------------------|
| `id`                | `UUID`      | PK.                                            |
| `entity_id`         | `UUID`      | The entity.                                    |
| `verb_id`           | `TEXT`      | The DSL verb that made the call.               |
| `provider`          | `TEXT`      | Provider identifier.                           |
| `operation`         | `TEXT`      | Operation name.                                |
| `external_ref`      | `TEXT`      | Provider's reference for the created resource.  |
| `request_hash`      | `BIGINT`    | Hash of request parameters.                    |
| `request_snapshot`  | `JSONB`     | Full request payload (audit).                  |
| `response_snapshot` | `JSONB`     | Full response payload (audit).                 |
| `created_at`        | `TIMESTAMPTZ` | When the call was made.                      |
| `superseded_by`     | `UUID`      | FK → self. Points to correction record.        |
| `is_current`        | `BOOL`      | True for the latest call.                      |

**Index:** `(entity_id, verb_id, provider) WHERE is_current = true`.

### 6.5 `compensation_records`

Regulatory audit trail for every external correction triggered by replay.

| Column              | Type        | Description                                    |
|---------------------|-------------|------------------------------------------------|
| `id`                | `UUID`      | PK.                                            |
| `remediation_id`    | `UUID`      | FK → `remediation_events.id`.                  |
| `entity_id`         | `UUID`      | The entity.                                    |
| `provider`          | `TEXT`      | Provider identifier.                           |
| `original_call_id`  | `UUID`      | FK → `external_call_log.id` (original).        |
| `correction_call_id`| `UUID`      | FK → `external_call_log.id` (correction).      |
| `correction_type`   | `TEXT`      | `amend` / `cancel_recreate` / `correction_filing` / `manual`. |
| `changed_fields`    | `JSONB`     | Diff between original and correction.          |
| `outcome`           | `TEXT`      | `success` / `pending` / `failed`.              |
| `confirmed_at`      | `TIMESTAMPTZ` | When provider confirmed correction.           |
| `confirmed_by`      | `TEXT`      | User or system that confirmed.                 |
| `created_at`        | `TIMESTAMPTZ` | When compensation was initiated.              |

### 6.6 `remediation_events`

Lifecycle entity tracking the resolution of a shared attribute supersession.

| Column              | Type        | Description                                    |
|---------------------|-------------|------------------------------------------------|
| `id`                | `UUID`      | PK.                                            |
| `entity_id`         | `UUID`      | The entity affected.                           |
| `source_atom_id`    | `UUID`      | FK → `shared_atom_registry.id`.                |
| `source_workspace`  | `TEXT`      | Workspace that committed the supersession.     |
| `prior_version`     | `INT`       | Version before supersession.                   |
| `new_version`       | `INT`       | Version after supersession.                    |
| `affected_workspace`| `TEXT`      | Consuming workspace affected.                  |
| `affected_constellation_family` | `TEXT` | Consuming constellation family.         |
| `status`            | `TEXT`      | `detected` / `replaying` / `resolved` / `escalated` / `deferred`. |
| `failed_at_step`    | `TEXT`      | Runbook step where replay failed (NULL if resolved). |
| `failure_reason`    | `TEXT`      | Error from the failed verb (NULL if resolved). |
| `deferral_reason`   | `TEXT`      | NULL unless status = `deferred`.               |
| `resolved_at`       | `TIMESTAMPTZ` | When final state was reached.                |
| `resolved_by`       | `UUID`      | User who resolved/deferred.                    |
| `created_at`        | `TIMESTAMPTZ` | When supersession was detected.              |

**Index:** `(entity_id, status)`.

### 6.7 `provider_capabilities`

Reference data for third-party correction behaviour.

| Column              | Type        | Description                                    |
|---------------------|-------------|------------------------------------------------|
| `id`                | `UUID`      | PK.                                            |
| `provider`          | `TEXT`      | Provider identifier.                           |
| `operation`         | `TEXT`      | Operation name.                                |
| `capability`        | `TEXT`      | `amendable` / `cancel_and_recreate` / `immutable` / `manual`. |
| `amend_details`     | `JSONB`     | Endpoint, method, constraints.                 |
| `notes`             | `TEXT`      | Human-readable notes.                          |

**Index:** `UNIQUE (provider, operation)`.

---

## 7. Entity Relationship Summary

```
shared_atom_registry ◄─── SOURCE OF TRUTH ANCHOR
    │                      (lifecycle: Draft → Active → Deprecated → Retired)
    │
    ├──< shared_fact_versions           [Authoritative version history — INV-1]
    │       │
    │       └── (entity_id, version) ──> value at that version
    │
    └──< workspace_fact_refs            [Consumption-state projection — INV-2]
            │
            └── held_version → pointer into shared_fact_versions
                    │
                    └── stale? → remediation_events
                                      │
                                      │  replay: full constellation from top
                                      │  upsert: unchanged = no-op
                                      │  idempotency envelope: external calls
                                      │
                                      ├──< compensation_records
                                      │        │
                                      │        ├── original_call ──> external_call_log
                                      │        └── correction_call ──> external_call_log
                                      │
                                      └── status FSM (§4.7)

provider_capabilities (reference data, joined at runtime)

NOTE: Level 0 platform DAG is DERIVED at boot from
      shared_atom_registry + verb footprints (reads_from).
      No persisted edge table. See §3.2.
```

---

## 8. Operational Scenarios

### 8.1 LEI Correction — Onboarding In-Flight

1. KYC workspace: `correct_lei(entity_id, new_lei)` verb executes.
2. Post-commit: new row in `shared_fact_versions` (version N+1).
   **Stage 1: attribute superseded.**
3. `propagate_shared_fact_staleness` runs. Finds OnBoarding consumer.
   `workspace_fact_refs` marked `stale`. **Stage 2: consumer stale.**
4. Entity is in-flight. **Stage 3: no remediation event.**
5. Next OnBoarding verb: `check_staleness` fires → `StaleSharedFact`.
   NarrationEngine surfaces blocker.
6. User/Sage runs `acknowledge_shared_update(entity_id, "entity.lei")`.
7. Consumer ref advanced to N+1. Status → `current`.
8. OnBoarding verbs resume with new LEI.

### 8.2 LEI Correction — Onboarding Complete (Compatible)

1. Steps 1–2 as above. **Stage 1: attribute superseded.**
2. Consumer ref marked `stale`. **Stage 2: consumer stale.**
3. Entity state is `complete`. **Stage 3: remediation event created.**
4. `replay_constellation(entity_id, "onboarding_workspace")` triggered.
   Status → `replaying`.
5. Runbook replays from top:
   - Verbs 1–12: inputs unchanged. Upserts are no-ops.
   - Verb 13 (`setup_sub_custodian_lei_ref`): input changed (new LEI).
     Upsert writes corrected value.
   - Verb 14 (`register_settlement_instructions`): external call.
     Idempotency envelope detects changed hash. Provider: `amendable`.
     Issues PUT. Compensation record created.
   - Verbs 15–47: cascade. Some pick up changed output from verb 13,
     upsert corrected values. Others unchanged, no-op.
6. Replay completes. Consumer ref advanced. Remediation → `resolved`.

### 8.3 LEI Correction — Breaking Change

1. Steps 1–3 as §8.2.
2. Replay proceeds. Verb 13 (`setup_sub_custodian_lei_ref`) fails — new LEI
   resolves to a jurisdiction with no sub-custodian relationship.
3. Replay halts at step 13. Remediation → `escalated`. `failed_at_step`
   and `failure_reason` recorded.
4. Human reviews. Options:
   - Establish new sub-custodian relationship, re-trigger replay.
   - `defer_remediation` (document reason for regulatory record).
   - Revert KYC change.

### 8.4 LEI Correction — Manual Provider

1. Steps 1–3 as §8.2.
2. Replay proceeds. Verb 14 (`register_settlement_instructions`) hits
   idempotency envelope. Provider capability: `manual`. Returns
   `ManualInterventionRequired`.
3. Remediation → `escalated`.
4. Human contacts provider, confirms correction externally.
5. `confirm_external_correction(entity_id, remediation_id, provider_ref)`.
6. Compensation record created. Replay resumes from step 14.
7. Remaining verbs complete. Remediation → `resolved`.

---

## 9. Terminology

Strict hierarchy: **superseded → stale → replay.**

| Term | Definition | Level |
|------|------------|-------|
| **Superseded** | A shared attribute version that is no longer current because a newer version has been committed by the owning workspace. The canonical origin of all downstream staleness (INV-1). | Attribute (origin) |
| **Stale** | A consumer reference that points to a superseded attribute version. A derived property (INV-2). | Consumer (projection) |
| **Replay** | Full constellation re-execution from the top for a stale consumer. Upsert semantics handle unchanged state. Idempotency envelope handles external calls (INV-3, INV-4). | Constellation (resolution) |

---

## 10. Design Decisions

| # | Decision | Rationale |
|---|----------|-----------|
| D-01 | Anchor on attributes, not verbs. | O(shared_atoms) ≪ O(verbs). Ownership is a property of data, not behaviour. |
| D-02 | Two-level DAG, Level 0 derived on-the-fly. | Same structural primitive at different elevation. Level 0 is a computed view from registry + verb footprints. No persisted edge table. No cache invalidation. |
| D-03 | `make`-style replay, not reactive event bus. | Post-commit marking + explicit replay is simpler, auditable, and compatible with custody banking's "human in the loop" requirements. |
| D-04 | Version comparison, not version-vector. | Shared atom count is O(50). Version vectors are O(workspace²) overhead with no benefit at this scale. |
| D-05 | External idempotency at verb level, not platform level. | Verbs encapsulate their own I/O. Provider capability is a verb concern. |
| D-06 | `Deferred` is a valid terminal state. | Custody banking requires explicit, auditable "do nothing" decisions. Divergence is sometimes the legally correct state. |
| D-07 | Boot-time discovery + YAML seed. | Matches existing platform patterns. YAML seeds registry; runtime changes via DSL verbs. |
| D-08 | Async post-commit propagation. | Matches existing `propagate_derived_chain_staleness` pattern. Avoids adding latency to owning-workspace verbs. |
| D-09 | `RebuildContext` extends `ReplayEnvelope`. | Reuses existing runbook replay infrastructure. No parallel execution context type. |
| D-10 | Staleness surfaces via NarrationEngine. | Existing blocker/suggested_next surface. No new UX pathway. |
| D-11 | **No soft constraints.** If it's shared, it's enforced. | Either the attribute matters for cross-workspace consistency or it doesn't. Registry admission is the only gate. Eliminates an entire branching dimension from the propagation and replay pipeline. |
| D-12 | **Canonical unit of drift is the shared attribute version.** | All downstream staleness is a derivative projection of a fact-level supersession (INV-1). |
| D-13 | **Constellation replay from top, not vertex-level rebuild.** | Upsert semantics make fine-grained targeting unnecessary. Unchanged verbs = no-op. Changed verbs = correct update. Eliminates dependency tracking, transitive closure computation, and rebuild candidate management. |
| D-14 | **Table-level `reads_from` is a bootstrap discovery mechanism.** | Coarse discovery + replay from top = zero cost for over-declaration. Long-term: attribute-level footprints on `VerbContractBody`. |
| D-15 | **Shared atoms are governed SemOS entities with a lifecycle FSM.** | Draft → Active → Deprecated → Retired. YAML seeds; DSL verbs govern transitions. No attribute is enforced without deliberate activation. |

---

## 11. Open Questions

| # | Question | Context |
|---|----------|---------|
| Q-01 | **Auto-replay for complete entities: require human approval or execute automatically?** Regulatory risk tolerance question. May vary by jurisdiction (LU vs IE vs UK vs US). | §4.3, §4.7, §8.2 |
| Q-02 | **Compensation record retention policy.** UCITS may require 10-year retention. Need per-jurisdiction requirements. | §6.5 |
| Q-03 | **Multiple concurrent supersessions.** If LEI and jurisdiction both change in rapid succession, does each trigger a separate replay, or should they be batched? Batching is more efficient; separate replays are simpler. | §5.1 |

---

## 12. Implementation Phasing

| Phase | Deliverable | Dependencies |
|-------|-------------|-------------|
| P1    | `shared_atom_registry` table + lifecycle FSM YAML + registry verbs (`register`, `activate`, `deprecate`, `retire`). Migration. | None. |
| P2    | `shared_fact_versions` table. Post-commit version insertion for Active atoms. | P1. |
| P3    | `workspace_fact_refs` table. `check_staleness` pre-REPL hook. NarrationEngine blocker integration. | P1, P2. |
| P4    | `propagate_shared_fact_staleness` function (three-stage). Post-commit trigger. | P1, P2, P3. |
| P5    | `replay_stale_constellation` function. `RebuildContext` extending `ReplayEnvelope`. | P4. |
| P6    | `remediation_events` table + FSM YAML. Remediation verbs (`defer`, `revoke_deferral`, `confirm_external_correction`). | P4, P5. |
| P7    | `external_call_log` table. `execute_with_idempotency` wrapper. | P5. |
| P8    | `provider_capabilities` reference data. Amendment/correction branching. | P7. |
| P9    | `compensation_records` table. Audit trail generation. | P7, P8. |
| P10   | Shared atom YAML seed declarations for initial set (LEI, jurisdiction, fund structure type, UBO threshold, regulatory classification). Boot-time seeding via `register_shared_atom`. | P1. |

**E-invariant:** Each phase must reach 100% before proceeding. Progress
gates at every phase boundary. Do not commit — Adam reviews the diff first.

---

## 13. Glossary

| Term | Definition |
|------|------------|
| **Shared Atom** | An attribute whose value is owned by one workspace but consumed by one or more other workspaces. Governed SemOS entity with lifecycle FSM (Draft → Active → Deprecated → Retired). |
| **Superseded Attribute Version** | A shared fact version that is no longer current. The canonical origin of all downstream staleness (INV-1). |
| **Stale Consumer** | A workspace reference pointing to a superseded attribute version. A derived property (INV-2). |
| **Constellation Replay** | Full re-execution of a consuming constellation from the top. Upsert = no-op for unchanged state. Idempotency envelope = safe external calls (INV-3). |
| **Controlled Re-Evaluation** | Replay that respects policy gates, lifecycle constraints, and structural validity. Not mechanical rerun (INV-5). |
| **Platform DAG** | Level 0 DAG. Derived on-the-fly from shared atom registry + verb footprints. Not persisted. |
| **Constellation DAG** | Level 1 DAG. Existing model. YAML-driven via `SlotDef.depends_on`. |
| **Idempotency Envelope** | Wrapper around external-calling verbs. First-run vs. replay branching based on provider capability. |
| **Compensation Record** | Audit trail for external corrections triggered by replay. Regulatory evidence. |
| **Remediation Event** | Lifecycle entity tracking supersession resolution. FSM: Detected → Replaying → Resolved / Escalated → Resolved / Deferred. |
| **Provider Capability** | Per-provider, per-operation correction classification: amendable, cancel-and-recreate, immutable, manual. |

---

## 14. Codebase References

| Concept | File | Notes |
|---------|------|-------|
| Verb contract R/W sets | `rust/crates/sem_os_core/src/verb_contract.rs:54-58` | `reads_from`, `writes_to` |
| Constellation slot deps | `rust/crates/sem_os_core/src/constellation_map_def.rs:37` | `depends_on: Vec<DependencyEntry>` |
| DependencyEntry enum | `rust/crates/sem_os_core/src/constellation_map_def.rs:88-110` | Simple + Explicit |
| WorkspaceKind enum | `rust/src/repl/types_v2.rs:90-98` | 7 workspace variants |
| Constellation families | `rust/src/repl/types_v2.rs:130-237` | Per-workspace family lists |
| Derived staleness propagation | `rust/src/derived_attributes/repository.rs:263-326` | `mark_stale_by_input()` |
| SQL staleness function | `migrations/master-schema.sql` | `propagate_derived_chain_staleness()` |
| Execution records | `migrations/master-schema.sql` | `dsl_idempotency` table |
| Runbook plan executor | `rust/src/runbook/plan_executor.rs` | `advance_plan_step()` |
| ReplayEnvelope | `rust/src/runbook/types.rs` | Exact-replay determinism boundary |
| State machine YAML pattern | `rust/config/sem_os_seeds/state_machines/` | Pattern for new FSMs |
| NarrationEngine | `rust/src/agent/narration_engine.rs` | Blocker/suggested_next surface |

---

*End of document. Submitted for peer review.*
