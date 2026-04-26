# Semantic OS & SemReg — Detailed Annex

> This annex covers Semantic OS, SemReg, context resolution (CCIR), ABAC,
> stewardship, governed authoring, SessionVerbSurface, scanner/seed bundles,
> constellation maps, and state machines.
> For the high-level overview see the root `CLAUDE.md`.

---

## Architecture Overview

Semantic OS is the **single authoritative source of truth** for verb availability. It is not a separate system — it is integrated into the core orchestrator at Stage 2.5 (CCIR) and Stage 2.5 (SessionVerbSurface). All DSL verb discovery flows through it. No exceptions, no bypasses.

The current runtime model is:

`workspace/domain -> SEM-OS constellation map -> typed slot/node -> bound UUID entity -> recovered state -> grounded legal verbs`

Two practical consequences follow.

- A verb floating in YAML is not part of the deterministic agent surface unless
  it is connected to a SEM-OS slot/state context.
- Some verbs are intentionally multi-context. They may appear in more than one
  constellation or slot path, but they are still surfaced only through the
  currently grounded SEM-OS context.

**Crates:**

| Crate | Purpose |
|-------|---------|
| `sem_os_core` | Pure domain types, ABAC, context resolution, ports (no sqlx) |
| `sem_os_postgres` | PostgreSQL implementations of ports **+ every plugin-verb op body** (post Phase 5c-migrate slice #80) |
| `sem_os_server` | Standalone REST API + JWT auth |
| `sem_os_client` | Trait: in-process or HTTP access |
| `sem_os_harness` | Integration test framework (102 scenarios) |
| `sem_os_obpoc_adapter` | Verb YAML → seed bundle conversion |

---

## Plugin Verb Dispatch (post Phase 5c-migrate slice #80)

A single trait — `sem_os_postgres::ops::SemOsVerbOp` — is the sole execution contract for every plugin verb. No `CustomOperation`, no `inventory::collect!`, no `#[register_custom_op]` proc-macro: these were deleted in slice #80 along with the `dsl-runtime-macros` crate.

**Signature:**

```rust
#[async_trait]
pub trait SemOsVerbOp: Send + Sync {
    fn fqn(&self) -> &str;  // e.g. "entity.ghost"
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome>;
}
```

**Registration — explicit, two-source:**

1. `sem_os_postgres::ops::build_registry()` — the canonical `SemOsVerbOpRegistry` holding 567 ops that live in `sem_os_postgres::ops::<domain>::*` (no ob-poc internals reached from these op bodies).
2. `ob_poc::domain_ops::extend_registry(&mut SemOsVerbOpRegistry)` — appends 119 Pattern B ops that live in `rust/src/domain_ops/*` because they bridge to ob-poc internals (`DslExecutor`, `TemplateExpander`, `BookingPrincipalRepository`, `gleif::client`, `bpmn_integration`, etc.) that can't be inverted behind a service trait without a disproportionate refactor.

`ob-poc-web::main` calls both and threads the resulting registry into `ObPocVerbExecutor::with_sem_os_ops(...)`.

**Dispatch path:** `ObPocVerbExecutor::execute_verb` (Stage 2.5 of the agent pipeline) opens a `PgTransactionScope` from the pool, looks up the op by FQN, calls `op.execute(args, ctx, &mut scope).await`. Commits on `Ok`, rolls back on `Err`. Platform services (attribute identity, lifecycle catalog, phrase bank, etc.) are accessed via `ctx.service::<dyn X>()?` — the service registry is threaded onto every context.

**Test entry points:**
- `cargo test -p ob-poc --lib -- test_plugin_verb_coverage` — asserts every YAML `behavior: plugin` verb has a matching FQN in the combined registry.
- `cargo test -p ob-poc --lib -- test_extend_registry_adds_pattern_b_ops` — smoke test for Pattern B registration.

---

## Boundary Status Update (2026-04-16)

The Sem OS crate split is now materially aligned with a standalone capability boundary, but two different surfaces need to stay distinct:

- **Official capability surface:** `sem_os_client`, selected `sem_os_core` contracts, and the narrow `sem_os_server` embedding API (`build_router`, `JwtConfig`, `OutboxDispatcher`)
- **Family integration surface:** `sem_os_core::service`, `sem_os_core::ports`, `sem_os_postgres`, and the `sem_os_obpoc_adapter` bridge used by `ob-poc`

Recent cleanup narrowed the production boundary without changing behavior:

- `sem_os_server` no longer exposes its handler/error module tree as the supported API surface
- `sem_os_harness` support modules are test-only, so platform harnesses should not treat harness internals as runtime API
- dormant `/tools/*` server handlers were removed because the routes are intentionally absent from the live standalone surface

The main deferred boundary issue is still `sem_os_obpoc_adapter`: scanner/seed helpers remain public because `ob-poc` still consumes them directly. That is an adapter-facade problem, not a reason to widen the Sem OS service contract itself.

---

## Semantic Registry (SemReg)

### Immutable Snapshot Architecture

All registry objects share a single table (`sem_reg.snapshots`). Every change creates a **new snapshot with predecessor link** — no in-place updates.

```sql
sem_reg.snapshots (
  snapshot_id UUID PRIMARY KEY,
  snapshot_set_id UUID,         -- Groups related snapshots
  object_type TEXT,             -- VerbContract | EntityTypeDef | AttributeDef | ...
  object_id UUID,
  version_major INT, version_minor INT,
  status TEXT,                  -- Draft | Active | Deprecated | Retired
  governance_tier TEXT,         -- Governed | Operational
  trust_class TEXT,             -- Proof | DecisionSupport | Convenience
  security_label JSONB,
  effective_from TIMESTAMPTZ,
  effective_until TIMESTAMPTZ,
  predecessor_id UUID,
  change_type TEXT,
  change_rationale TEXT,
  created_by TEXT,
  approved_by TEXT,
  definition JSONB,             -- The actual body
  created_at TIMESTAMPTZ
)
```

**Key invariants:**
- No in-place updates — all changes are new snapshots
- Proof Rule: Only `governance_tier = Governed` may have `trust_class = Proof` (DB CHECK constraint)
- Operational snapshots auto-approve (no manual gate)
- Full point-in-time resolution via `snapshot_set_id`

### Object Types (24 total)

| Type | Purpose |
|------|---------|
| `VerbContract` | Verb args, returns, preconditions, lifecycle requirements |
| `EntityTypeDef` | Entity shape, attributes, relationships, evidence requirements |
| `AttributeDef` | Attribute name, type, source (derived vs proven), evidence grade |
| `RelationshipTypeDef` | Entity-entity relationships (ownership, control, beneficial interest) |
| `TaxonomyDef` / `TaxonomyNode` / `MembershipRule` | Categorization schemes + membership conditions |
| `ViewDef` | Constellation-level view: verbs + attributes for a domain/subject |
| `PolicyRule` | Conditional verb allow/deny based on actor/object properties |
| `EvidenceRequirement` | What evidence must exist before a verb is executable |
| `DocumentTypeDef` | Document schema + handling controls |
| `ConstellationFamilyDef` | Abstract grouping of related constellations |
| `ConstellationMap` | Concrete constellation: slots, state machines, verbs per slot |
| `StateMachine` | State transition rules for a slot/node |
| `StateGraph` | DAG of state constraints (cross-domain workflows) |
| `UniverseDef` | Discovery universe: domains, entry questions, grounding thresholds |
| `MacroDef` | Compound intent (e.g., "lux sicav setup" → sequence of verbs) |

**Key files:**
- `rust/src/sem_reg/mod.rs` — module structure
- `rust/src/sem_reg/registry.rs` — `RegistryService` CRUD operations
- `rust/src/sem_reg/gates.rs` — publish gate functions
- `rust/crates/sem_os_core/src/types.rs` — core enums (`GovernanceTier`, `TrustClass`, `ObjectType`, `SecurityLabel`)

---

## Context Resolution (CCIR — 12-Step Pure Pipeline)

**File:** `rust/crates/sem_os_core/src/context_resolution.rs`

Pure scoring/ranking logic. DB loads happen in preparation; all 12 steps are pure functions.

### Pipeline Stages

```
1.  Determine snapshot epoch (as-of time)
2.  Resolve subject → entity type + jurisdiction + state
2b. Load taxonomy memberships + evaluate conditional memberships
2c. Load subject relationships
3.  Select applicable ViewDefs by taxonomy overlap
4.  Extract verb_surface + attribute_prominence from top view
5.  Filter verbs by taxonomy + ABAC + tier
6.  Filter attributes similarly
7.  Rank by ViewDef prominence weights
8.  Evaluate preconditions
9.  Evaluate policies
10. Compute composite access decision
11. Generate governance signals
12. Compute confidence score
```

### Request

```rust
pub struct ContextResolutionRequest {
    pub subject: SubjectRef,                    // EntityId | CaseId | DocumentId | TaskId
    pub intent_summary: Option<String>,         // From Sage
    pub raw_utterance: Option<String>,
    pub actor: ActorContext,
    pub goals: Vec<String>,
    pub constraints: ResolutionConstraints,
    pub evidence_mode: EvidenceMode,            // Strict | Normal | Exploratory | Governance
    pub point_in_time: Option<DateTime<Utc>>,
    pub entity_kind: Option<String>,
    pub entity_confidence: Option<f64>,
    pub discovery: DiscoveryContext,
}
```

**Evidence modes:**

| Mode | Behavior |
|------|----------|
| `Strict` | Governed + Proof/DecisionSupport only; Operational excluded |
| `Normal` (default) | Governed primary; Operational if view includes it |
| `Exploratory` | All tiers + trust classes, annotated |
| `Governance` | Coverage metrics: stewardship gaps, classification gaps, stale evidence |

### Response

```rust
pub struct ContextResolutionResponse {
    pub applicable_views: Vec<RankedView>,
    pub candidate_verbs: Vec<VerbCandidate>,
    pub candidate_attributes: Vec<AttributeCandidate>,
    pub required_preconditions: Vec<PreconditionStatus>,
    pub disambiguation_questions: Vec<DisambiguationPrompt>,
    pub evidence: EvidenceSummary,
    pub policy_verdicts: Vec<PolicyVerdict>,
    pub security_handling: AccessDecision,
    pub governance_signals: Vec<GovernanceSignal>,
    pub entity_kind_pruned_verbs: Vec<EntityKindPrunedVerb>,
    pub confidence: f64,
    pub grounded_action_surface: Option<GroundedActionSurface>,
    pub resolution_stage: ResolutionStage,      // Discovery | Grounded
    pub discovery_surface: Option<DiscoverySurface>,
}
```

### Grounded Action Surface

Structured provenance of what Sem OS grounded for this turn:

```rust
pub struct GroundedActionSurface {
    pub resolved_constellation: Option<String>,
    pub resolved_slot_path: Option<String>,     // e.g. "cbu/kyc_case"
    pub resolved_state_machine: Option<String>,
    pub current_state: Option<String>,           // e.g. "intake"
    pub traversed_edges: Vec<GroundedTraversalEdge>,
    pub valid_actions: Vec<GroundedActionOption>,
    pub blocked_actions: Vec<BlockedActionOption>,
    pub dsl_candidates: Vec<DslCandidate>,
}
```

## Runtime Hydration Boundary

The active workspace/session action surface is produced by server-side
constellation hydration plus reducer/state-machine evaluation.

- `api::constellation_routes::resolve_context()` picks the workspace default
  constellation map.
- `sem_os_runtime::hydrate_constellation()` binds concrete entities and reducer
  state into that map.
- `compute_action_surface()` produces slot-local legal verbs.
- The REPL/UI scoped verb surface is the flattened union of those hydrated slot
  verbs.

This is the authoritative runtime path used by the UI and observatory.

Note that the authored SEM-OS map corpus is broader than the currently
implemented generic hydrator. The active agent/session discovery path is clean,
but some non-primary authored joins still require future generic hydration work.

### Discovery Surface

When Sem OS is still in discovery (no subject selected yet):

```rust
pub struct DiscoverySurface {
    pub matched_universes: Vec<RankedUniverse>,
    pub matched_domains: Vec<RankedUniverseDomain>,
    pub matched_families: Vec<RankedConstellationFamily>,
    pub matched_constellations: Vec<RankedConstellation>,
    pub missing_inputs: Vec<GroundingInput>,
    pub entry_questions: Vec<EntryQuestion>,
    pub grounding_readiness: GroundingReadiness, // NotReady | FamilyReady | ConstellationReady | Grounded
}
```

---

## ABAC (Attribute-Based Access Control)

**File:** `rust/crates/sem_os_core/src/abac.rs`

```rust
pub struct ActorContext {
    pub actor_id: String,
    pub roles: Vec<String>,                  // e.g. ["analyst", "compliance_officer"]
    pub department: Option<String>,
    pub clearance: Option<Classification>,   // Public | Internal | Confidential | Restricted
    pub jurisdictions: Vec<String>,
}

pub struct SecurityLabel {
    pub classification: Classification,
    pub pii: bool,
    pub jurisdictions: Vec<String>,
    pub purpose_limitation: Vec<String>,     // e.g. ["KYC_CDD", "AML_SCREENING"]
    pub handling_controls: Vec<HandlingControl>, // MaskByDefault | NoExport | NoLlmExternal | DualControl | SecureViewerOnly
}

pub enum AccessDecision {
    Allow,
    Deny { reason: String },
    AllowWithMasking { masked_fields: Vec<String> },
}

pub fn evaluate_abac(actor: &ActorContext, label: &SecurityLabel, purpose: AccessPurpose) -> AccessDecision
```

**Evaluation rules:**
1. Actor clearance must be ≥ object classification
2. Actor jurisdictions must cover object jurisdictions
3. Actor access purpose must match object's purpose_limitation (if populated)
4. PII objects require explicit PII clearance

---

## Publish Gates

Four gates evaluated before any snapshot is persisted:

| Gate | Rule | Severity |
|------|------|----------|
| Proof Rule | `Operational + Proof` → reject | Blocking |
| Security Label | `PII + Public/Internal` → reject | Blocking |
| Governed Approval | `Governed + no approver` → reject | Blocking |
| Version Monotonicity | New version < predecessor → reject | Blocking |

**File:** `rust/src/sem_reg/gates.rs`

Additional guardrails (G01–G15) run at changeset validation time:

| ID | Check |
|----|-------|
| G01 | Proof rule violation |
| G02 | Security label on both tiers |
| G03 | Circular dependency detection |
| G06 | PII governance gap |
| G07 | Orphaned relationship target |
| G09 | Entity kind mismatch |
| G11 | Taxonomy loop detection |
| G12 | Missing entity_kind on entity type |
| G15 | Governance tier regression |

---

## SemOsContextEnvelope

**File:** `rust/src/agent/sem_os_context_envelope.rs`

Replaces flat `SemRegVerbPolicy`. Preserves full resolution output.

```rust
pub struct SemOsContextEnvelope {
    pub allowed_verbs: HashSet<String>,
    pub allowed_verb_contracts: Vec<VerbCandidateSummary>,
    pub pruned_verbs: Vec<PrunedVerb>,
    pub fingerprint: AllowedVerbSetFingerprint,        // "v1:<sha256>"
    pub evidence_gaps: Vec<String>,
    pub governance_signals: Vec<GovernanceSignalSummary>,
    pub snapshot_set_id: Option<String>,
    pub computed_at: DateTime<Utc>,
    pub resolution_stage: ResolutionStage,
    pub discovery_surface: Option<DiscoverySurface>,
    pub grounded_action_surface: Option<GroundedActionSurface>,
    // deny_all, unavailable: private — use #[cfg(test)] test_with_verbs() for tests
}

pub enum PruneReason {
    AbacDenied { actor_role: String, required: String },
    EntityKindMismatch { verb_kinds: Vec<String>, subject_kind: String },
    TierExcluded { tier: String, reason: String },
    TaxonomyNoOverlap { verb_taxonomies: Vec<String> },
    PreconditionFailed { precondition: String },
    AgentModeBlocked { mode: String },
    PolicyDenied { policy_fqn: String, reason: String },
}
```

Exclusion reasons are **additive** (SI-3): all reasons captured per verb, not first-hit.

### TOCTOU Recheck

```rust
pub enum TocTouResult {
    StillAllowed,
    AllowedButDrifted { new_fingerprint: AllowedVerbSetFingerprint },
    Denied { verb_fqn: String, new_fingerprint: AllowedVerbSetFingerprint },
}
```

Only performed when `OBPOC_STRICT_SEMREG=true`. Envelope is computed once before the loop; recheck happens before execution.

---

## SessionVerbSurface — 6-Step Governance Pipeline

**File:** `rust/src/agent/verb_surface.rs`

The single convergence point for all governance layers. Computed once per turn at orchestrator Stage 2.5.

### Pipeline

| Step | Filter |
|------|--------|
| 1 | Base set from RuntimeVerbRegistry (~1,455 verbs) |
| 2 | AgentMode filter (Research vs Governed) |
| 3 | Scope + Workflow (merged: group scope + workflow phase) |
| 4 | SemReg CCIR (SemOsContextEnvelope allowed set) |
| 5 | Lifecycle state filter (`requires_states` from verb config) |
| 6 | Rank + composite state bias + fingerprint |
| (7) | FailPolicy — if SemReg unavailable |

### Output

```rust
pub struct SessionVerbSurface {
    pub verbs: Vec<SurfaceVerb>,
    pub excluded: Vec<ExcludedVerb>,
    pub surface_fingerprint: SurfaceFingerprint,   // "vs1:<hex>"
    pub semreg_fingerprint: Option<AllowedVerbSetFingerprint>, // "v1:<hex>"
    pub fail_policy_applied: VerbSurfaceFailPolicy,
    pub computed_at: DateTime<Utc>,
    pub filter_summary: FilterSummary,             // Step-by-step counts
}

pub enum VerbSurfaceFailPolicy {
    FailClosed,  // ~30 safe-harbor verbs (default)
    FailOpen,    // full registry (dev-only)
}

pub struct FilterSummary {
    pub total_registry: usize,
    pub after_agent_mode: usize,
    pub after_workflow: usize,
    pub after_group_scope: usize,
    pub after_semreg: usize,
    pub after_lifecycle: usize,
    pub after_actor: usize,
    pub final_count: usize,
}
```

### Dual Fingerprints (Invariant SI-2)

| Fingerprint | Scope | Detects |
|-------------|-------|---------|
| `vs1:<hex>` | Final visible set + filter context | Scope/workflow/lifecycle removals |
| `v1:<hex>` | CCIR allowed set only | SemReg policy changes |

Different hashes reveal whether scope/lifecycle filters removed verbs that SemReg allowed.

### Safe-Harbor Domains (FailClosed)

When SemReg is unavailable: `agent`, `audit`, `focus`, `registry`, `schema`, `session`, `view` (all read-only — Invariant SI-1).

### No-Group Domains

When no client group is loaded: `agent`, `audit`, `client-group`, `focus`, `gleif`, `onboarding`, `registry`, `schema`, `session`, `view` (forces group selection).

### Workflow-Scoped Domain Allowlists

| Workflow | Allowed Domains |
|----------|----------------|
| `semos-onboarding` | cbu, entity, session, view, agent, contract, deal, billing, trading-profile, custody, onboarding, gleif, research |
| `semos-kyc` | kyc, screening, document, requirement, ubo, session, view, agent, entity |
| `semos-data-management` | registry, changeset, governance, schema, authoring, deal, cbu, document, product, session, view, agent, audit |
| `semos-stewardship` | focus, changeset, governance, audit, maintenance, registry, schema, session, view, agent |

---

## AgentMode Gating

**File:** `rust/crates/sem_os_core/src/authoring/agent_mode.rs`

```rust
pub enum AgentMode {
    Research,     // Exploration, introspection, changeset authoring
    #[default]
    Governed,     // Validated business verbs, publishing
    Maintenance,  // Full surface: maintenance.*, governance.*, registry.*, changeset.*, authoring.*
}
```

| Verb Category | Research | Governed | Maintenance |
|---|---|---|---|
| Business verbs (cbu.*, entity.*, kyc-case.*) | Blocked | Allowed (via SemReg) | Blocked |
| Authoring (authoring.propose, authoring.validate) | Allowed | Blocked | Allowed |
| Changeset (changeset.*, review, publish) | Allowed | Blocked (propose only) | Allowed |
| Introspection (db_introspect.*) | Full surface | verify + describe only | Full surface |
| Registry/schema/focus/audit/agent | Allowed | Allowed | Allowed |
| Maintenance (maintenance.*) | Blocked | Allowed | Allowed |
| Navigation (nav.*) | Allowed | Allowed | Allowed |

---

## Stewardship — Changeset Authoring & Show Loop

**File:** `rust/src/sem_reg/stewardship/mod.rs`

### Changeset Lifecycle

```
Draft → ReadyForReview → UnderReview → Approved → Rejected → Published
```

```rust
pub struct Changeset {
    pub changeset_id: Uuid,
    pub status: ChangesetStatus,
    pub title: String,
    pub scope: String,               // Domain affected
    pub owner: String,
    pub entries: Vec<ChangesetEntry>, // Draft snapshots
}
```

### Impact Analysis

```rust
pub struct ChangesetImpactReport {
    pub total_affected: usize,
    pub affected_verbs: Vec<AffectedSnapshot>,
    pub affected_views: Vec<AffectedSnapshot>,
    pub affected_policies: Vec<AffectedSnapshot>,
    pub risk_summary: RiskSummary,
    pub dependents: Vec<DependentSnapshot>,
}
```

Risk levels: Low | Medium | High | Critical (based on # verbs affected, tier, trust class).

### Show Loop (Phase 1)

4 viewport types for stewardship focus management:

```rust
pub enum ViewportKind {
    DependencyTree,
    ImpactSummary,
    TaxonomyHierarchy,
    BreakingChanges,
}
```

---

## Observatory — Visual Projection Layer

**Plan:** `docs/observatory-implementation-plan.md`
**Status:** Phases 1-3 complete, Phases 4-8 pending

The Observatory renders as a standalone egui/eframe WASM application in a separate browser tab (`/observatory/:sessionId`). Chat UI remains React. Both tabs share the session ID and communicate via the same REST API.

### Rust Backend (Phase 1)

| File | Purpose |
|---|---|
| `sem_os_core/src/observatory/orientation.rs` | OrientationContract (6 questions: mode, level, focus, scope, lens, actions), ViewLevel (6 levels), EntryReason, OrientationDelta |
| `sem_os_core/src/observatory/projection.rs` | `project_orientation()` — pure transform from ContextResolutionResponse + FocusState → OrientationContract |
| `sem_os_core/src/observatory/graph_scene_projection.rs` | `project_graph_scene()` — HydratedConstellation → GraphSceneModel (layout strategy per ViewLevel) |
| `ob-poc-types/src/graph_scene.rs` | WASM-safe types: GraphSceneModel, SceneNode, SceneEdge, LayoutStrategy, DrillTarget |
| `api/observatory_routes.rs` | 6 REST endpoints under `/api/observatory/` |
| `sem_os_postgres::ops::nav` | 7 nav.* verb handlers (SemOsVerbOp impls; relocated from `rust/src/domain_ops/navigation_ops.rs` in Phase 5c-migrate slice #2) |
| `config/verbs/navigation.yaml` | 7 nav.* verb YAML definitions |

### DAG Identity (2026-04-13)

All Observatory endpoints project from the session's `tos.hydrated_state` — the same `HydratedSlot` tree the runbook compiler and narration engine read. No independent hydration.

**Two kinds of state on the session:**
- **Resource state (the DAG):** `tos.hydrated_state.hydrated_constellation` — rehydrated after verb execution writes
- **Viewport state:** `WorkspaceFrame.view_level`, `.focus_slot_path`, `.nav_snapshots` — mutated by nav verbs, no rehydration

**Key functions:**
- `project_orientation_from_repl_session()` — projects OrientationContract from TOS (available_actions from `HydratedSlot.available_verbs`)
- `slots_from_hydrated()` — single conversion point for GraphSceneModel projection
- `orientation_from_repl_or_legacy()` — tries REPL session first, falls back to legacy
- `apply_nav_result_if_present()` — orchestrator interprets nav verb results, writes viewport state

**Frontend:** All navigation routes through `chatApi.sendMessage()` → `POST /session/:id/input`. Zero calls to `observatoryApi.navigate()` remain. Cross-cache invalidation between Chat and Observatory query keys in both directions.

**Side doors closed:** SE-1 through SE-5 (independent hydration), SX-3 (`/navigate` bypassed by frontend). SE-10/SE-11 (OnboardingStateView) has DAG-sourced preferred path with DB fallback.

### Architecture: React Shell + egui Canvas

**React shell** (`ob-poc-ui-react/src/features/observatory/`): LocationHeader, Breadcrumbs, ViewportRenderer (Focus/Object/Diff/Gates), ActionPalette, MissionControl, ConstellationCanvas wrapper. All typed via `types/observatory.ts`.

**egui canvas** (`observatory-wasm/`, repo root): constellation renderer only, embedded in React via `<canvas>` element + wasm-bindgen.
- **Build:** `cd observatory-wasm && wasm-pack build --target web --release`
- **Depends on:** `ob-poc-types` only (no sem_os_core — avoids tokio/prost WASM blockers)
- **API:** `start_canvas(id)`, `set_scene(json)`, `set_view_level(level)`, `on_action(callback)`
- React pushes GraphSceneModel → egui renders → egui fires ObservatoryAction → React handles

5 level renderers: Universe (force-directed clusters), Cluster (bounded CBU nodes), System (deterministic orbital with edge rendering), Planet (entity relationship graph), Core (tree/DAG ownership chains).

### ShowPacket.orientation

`ShowPacket` carries an optional `orientation: Option<OrientationContract>` field. The show-packet endpoint populates it by calling `project_orientation()` after computing the base ShowPacket.

---

## Scanner & Seed Bundles

**File:** `rust/crates/sem_os_obpoc_adapter/src/scanner.rs`

### Scanner Pipeline (verb-first bootstrap)

```
1. Load verb YAML configs
2. Scan verb contracts (args, returns, preconditions, lifecycle)
3. Infer entity types from verb consumption/production patterns
4. Infer attributes from verb arg types
5. Suggest security labels (domain + tags)
6. Load domain metadata (reads/writes/workspace affinity)
7. Enrich verb contracts + entity types
8. Collect taxonomy/policy/view/derivation seeds
9. Serialize into SeedBundle (versioned DTO)
10. Compute deterministic SHA-256 bundle hash
11. Publish via SnapshotStore (idempotent)
```

### Pure Conversion Functions

```rust
pub fn verb_config_to_contract(domain: &str, action: &str, config: &VerbConfig) -> VerbContractBody
pub fn infer_entity_types_from_verbs(config: &VerbsConfig) -> Vec<EntityTypeDefBody>
pub fn infer_attributes_from_verbs(config: &VerbsConfig, entities: &[EntityTypeDefBody]) -> Vec<AttributeDefBody>
pub fn suggest_security_label(fqn: &str, domain: &str, tags: &[String]) -> SecurityLabel
pub fn enrich_verb_contracts(bodies: &mut [VerbContractBody], meta: &DomainMetadata)
pub fn enrich_entity_types(bodies: &mut [EntityTypeDefBody], meta: &DomainMetadata)
```

Entry point: `rust/crates/sem_os_obpoc_adapter/src/lib.rs` — `build_seed_bundle_with_metadata()`

### Verb Output Declarations

**File:** `rust/crates/sem_os_core/src/verb_contract.rs`

Entity-creating verbs declare their outputs via `VerbOutput`. These declarations enable forward-reference binding in multi-workspace runbook plans (R5).

```rust
pub struct VerbOutput {
    pub field_name: String,           // e.g. "created_cbu_id"
    pub output_type: String,          // e.g. "uuid", "record"
    pub entity_kind: Option<String>,  // e.g. "cbu", "entity"
    pub description: Option<String>,
}
```

Outputs are declared in verb YAML under `outputs:` and compiled into `VerbContractBody.outputs`. The plan compiler (`rust/src/runbook/plan_compiler.rs`) uses them to detect forward references between steps.

### Session Trace Enrichment

**Migration 128** adds `verb_resolved TEXT` and `execution_result JSONB` columns to `session_traces`, enabling trace entries to survive DB round-trips with full execution context. The trace repository (`rust/src/repl/trace_repository.rs`) persists and restores these fields.

Trace entries are now generated from the orchestrator execution path (not just structural stack ops):
- `TraceOp::Input` — generated at `process()` entry for every user utterance
- `TraceOp::VerbExecuted` — generated after both Durable and Sync completion in `execute_runbook_from()`

### Domain Metadata

**File:** `rust/config/sem_os_seeds/domain_metadata.yaml`

```yaml
domains:
  kyc:
    tables:
      cases:
        governance_tier: governed
        classification: confidential
        pii: true
    verb_data_footprint:
      kyc-case.create:
        reads: [client_groups, entities]
        writes: [cases, case_assignments]
        workspace_affinity: [kyc]
        constellation_families: [kyc_operations]
        subject_kinds: [kyc_case]
```

---

## Constellation Maps & State Machines

**Config:** `rust/config/sem_os_seeds/`

### Constellation Map Structure

```yaml
constellation: kyc.onboarding
slots:
  cbu:
    type: cbu
    cardinality: root
    verbs:
      show: { verb: cbu.show, when: filled }

  kyc_case:
    type: case
    cardinality: mandatory
    depends_on: [cbu]
    state_machine: kyc_case_lifecycle
    verbs:
      create: { verb: kyc-case.create, when: empty }
      read:   { verb: kyc-case.read,   when: filled }
      close:  { verb: kyc-case.close,  when: filled }
    children:
      screening:
        type: entity_graph
        cardinality: optional
        depends_on: [{ slot: kyc_case, min_state: discovery }]
        verbs:
          run:  { verb: screening.run,  when: empty }
          read: { verb: screening.read, when: filled }
```

**Types:** `rust/crates/sem_os_core/src/constellation_map_def.rs` (`SlotDef`, `VerbPaletteEntry`)

### State Machine

```yaml
state_machine: kyc_case_lifecycle
states: [intake, discovery, assessment, review, blocked, approved, rejected, withdrawn, expired]
initial: intake
transitions:
  - from: intake       to: discovery   verbs: [kyc-case.update-status]
  - from: discovery    to: assessment  verbs: [kyc-case.update-status]
  - from: assessment   to: review      verbs: [kyc-case.update-status, kyc-case.set-risk-rating]
  - from: review       to: approved    verbs: [kyc-case.close]
  - from: review       to: rejected    verbs: [kyc-case.close]
```

**Types:** `rust/crates/sem_os_core/src/state_machine_def.rs` (`StateMachineDefBody`, `TransitionDef`, `ReducerDef`)

State machines feed `current_state` into SessionVerbSurface lifecycle filter (Step 5).

### Universe (Discovery Navigation)

```yaml
fqn: universe.group_onboarding
default_entry_domain: client-group
domains:
  - domain_id: client-group
    label: "Client Group"
    candidate_family_ids: [client_group_ownership]
  - domain_id: kyc
    label: "KYC Management"
    candidate_family_ids: [kyc_operations]
```

---

## Onboarding State View

`OnboardingStateView` is returned on every `ChatResponse`. Two sourcing paths exist:

1. **DAG-sourced (preferred):** `try_onboarding_from_repl_response()` in `agent_enrichment.rs` reads from the REPL response's `session_feedback.tos.hydrated_constellation` — the same `HydratedSlot` tree the compiler and narration engine use. Calls `GroupCompositeState::from_hydrated_constellation()` which walks the slot tree to derive CBU state.

2. **DB-sourced (fallback):** `compute_onboarding_state_from_db()` runs raw SQL against `cbus`, `cases`, `screenings` tables. Used only when the REPL response doesn't carry a hydrated constellation (pre-workspace states). Marked transitional (SE-10/SE-11 in Observatory audit).

### GroupCompositeState

```rust
pub struct GroupCompositeState {
    pub cbu_count: usize,
    pub domain_counts: HashMap<String, usize>,    // "kyc_case" → 5
    pub has_ubo_determination: bool,
    pub has_control_chain: bool,
    pub cbu_states: Vec<CbuStateSummary>,
    pub next_likely_verbs: Vec<ScoredVerbHint>,   // +0.20 boost in surface
    pub blocked_verbs: Vec<BlockedVerbHint>,       // -0.20 penalty in surface
}
```

### Six-Layer Onboarding DAG

```
Layer 1: Group identity (prospect research)
Layer 2: Group UBO/control determination
Layer 3: CBU identification + validation
Layer 4: CBU KYC case creation
Layer 5: Screening + evidence collection
Layer 6: Tollgate approval + activation
```

**Forward verbs** advance the workflow; **revert verbs** step back. Both driven by constellation map + state machine definitions.

---

## Orchestrator Integration

```rust
// Stage 2 — resolve
let envelope = resolve_sem_reg_verbs(ctx, utterance, sage_intent.as_ref(), ..).await;

// Stage 2.5 — surface
let surface_ctx = VerbSurfaceContext {
    agent_mode: ctx.agent_mode,
    stage_focus: ctx.stage_focus.as_deref(),
    envelope: &envelope,
    fail_policy: if policy.semreg_fail_closed() { FailClosed } else { FailOpen },
    has_group_scope: ctx.scope.as_ref().and_then(|s| s.client_group_id).is_some(),
    composite_state: None,
    entity_state: None,
};
let surface = compute_session_verb_surface(&surface_ctx);

// Stage A — constrained verb search
let allowed_verbs = surface.allowed_fqns();
let candidates = searcher.search_with_constraint(utterance, &allowed_verbs);
```

---

## Key Invariants

| ID | Invariant | Mechanism |
|----|-----------|-----------|
| SI-1 | Safe-harbor verbs are read-only | Validated in `validate_fail_closed_safe_harbor_harm_class()` |
| SI-2 | Dual fingerprints detect CCIR vs surface divergence | `surface_fingerprint ≠ semreg_fingerprint` comparison |
| SI-3 | Exclusion reasons are additive | `Vec<SurfacePrune>` per `ExcludedVerb` |
| P-1 | No ungoverned expansion in group scope | `NO_GROUP_ALLOWED_DOMAINS` allowlist |
| P-2 | TOCTOU protection | Single envelope before loop; recheck before execution |
| P-3 | Workflow phase narrows domains | `workflow_allowed_domains()` per `stage_focus` |
| P-4 | Proof rule | DB CHECK constraint + gate validation |
| P-5 | SemReg is single gate — no exceptions | All paths via `resolve_sem_reg_verbs()` |

---

## Environment Variables

```bash
SEM_OS_MODE=inprocess                                # inprocess | remote
SEM_OS_DATABASE_URL="postgresql:///data_designer"    # For standalone server
SEM_OS_JWT_SECRET=dev-secret                         # For standalone server
OBPOC_STRICT_SEMREG=true                             # Fail-closed (default: true)
OBPOC_STRICT_SINGLE_PIPELINE=true                    # All verbs via SemReg (default: true)
```

**Standalone server:**
```bash
SEM_OS_DATABASE_URL="postgresql:///data_designer" SEM_OS_JWT_SECRET=dev-secret \
  cargo run -p sem_os_server
```

---

## Key Files Reference

| Path | Purpose |
|------|---------|
| `rust/src/agent/verb_surface.rs` | `SessionVerbSurface` pipeline |
| `rust/src/agent/sem_os_context_envelope.rs` | `SemOsContextEnvelope` + TOCTOU |
| `rust/src/agent/orchestrator.rs` | Stage 2 + 2.5 integration |
| `rust/src/sem_reg/mod.rs` | SemReg module structure |
| `rust/src/sem_reg/gates.rs` | Publish gate functions |
| `rust/src/sem_reg/stewardship/mod.rs` | Changeset + show loop |
| `rust/crates/sem_os_core/src/types.rs` | Core enums |
| `rust/crates/sem_os_core/src/abac.rs` | ABAC types + `evaluate_abac()` |
| `rust/crates/sem_os_core/src/context_resolution.rs` | 12-step pipeline |
| `rust/crates/sem_os_core/src/constellation_map_def.rs` | Slot + verb palette types |
| `rust/crates/sem_os_core/src/state_machine_def.rs` | State machine types |
| `rust/crates/sem_os_core/src/authoring/agent_mode.rs` | `AgentMode` + gating rules |
| `rust/crates/sem_os_core/src/observatory/orientation.rs` | OrientationContract, ViewLevel, EntryReason types |
| `rust/crates/sem_os_core/src/observatory/projection.rs` | `project_orientation()`, `compute_delta()` |
| `rust/crates/sem_os_core/src/observatory/graph_scene_projection.rs` | `project_graph_scene()` — constellation → scene |
| `rust/crates/ob-poc-types/src/graph_scene.rs` | GraphSceneModel, SceneNode (WASM-safe) |
| `rust/src/api/observatory_routes.rs` | Observatory REST endpoints |
| `rust/crates/sem_os_postgres/src/ops/nav.rs` | nav.* SemOsVerbOp handlers (post Phase 5c-migrate slice #2) |
| `rust/crates/sem_os_obpoc_adapter/src/scanner.rs` | Pure conversion functions |
| `rust/crates/sem_os_obpoc_adapter/src/metadata.rs` | `DomainMetadata` loader |
| `rust/config/sem_os_seeds/` | Universes, constellations, state machines, metadata |

---

### Unified Session Pipeline Integration

All user input now routes through `ReplOrchestratorV2.process()` with mandatory tollgates. SemOS governance is enforced at every gate:

1. **ScopeGate** — Bootstrap resolver checks `client_group_alias` table first, then substring/fuzzy match. `session.load-cluster` DSL executes via INV-3 gate. Empty groups (no CBUs) pass through.
2. **WorkspaceSelection** — 6 workspaces (CBU, Deal, KYC, OnBoarding, ProductMaintenance, InstrumentMatrix). Each has at least one journey pack.
3. **JourneySelection** — Pack routing via `PackRouter`. Empty `workspaces` list = allowed everywhere.
4. **InPack** — Verb matching via `IntentService` (HybridVerbSearcher), gated by `SemOsContextEnvelope`. SentencePlayback for user confirmation.

**Response adapter** (`rust/src/api/response_adapter.rs`) converts `ReplResponseV2` → `ChatResponse` for frontend compatibility. Each gate response maps to a `DecisionPacket` with `ClarifyGroup`/`ClarifyWorkspace`/`ClarifyJourney` kind.

**Session persistence:** `persist_session_checkpoint()` runs after every `process()` call. Trace entries (`TraceOp::Input`, `VerbExecuted`, `StateTransition`) flushed to `session_traces` table. Full audit trail.

**Dead code removed:** `cbu_session_routes.rs` (-917), `agent_dsl_routes.rs` (-2,437), `agent_learning_routes.rs` (-952), `vnext-repl` gates (-101), legacy fallback (-73). Total: -4,480 lines.

---

## Two-Tier Attribute Model (2026-04-02)

Attributes are classified by `AttributeVisibility` (External | Internal):

| | Above the Line (External) | Below the Line (Internal) |
|---|---|---|
| **Governance** | Governed — full changeset ceremony | Operational — auto-approved |
| **Evidence grade** | Any (including regulatory_evidence) | Forced to `prohibited` |
| **Trust class** | Can be `Proof` | Forced to `Convenience` |
| **Visibility** | `External` (default) | `Internal` |
| **Create verb** | `attribute.define` | `attribute.define-internal` |
| **Update verb** | Changeset path (compose → refine → publish) | `attribute.update-internal` (lightweight, no changeset) |
| **Derived variant** | `attribute.define-derived` | `attribute.define-derived` (with `is_derived=true`) |

**Internal attributes** are system flags, routing indicators, BNY implementation-specific classification codes — no relevance to external entities/clients. Engineering creates these frequently with minimal governance overhead.

**Guard:** `attribute.update-internal` refuses to update External/governed attributes — returns error directing to the changeset path.

**Schema:** `attribute_registry.visibility` column (text, CHECK: 'external'/'internal', default 'external'). Migration 130.

**Source type:** `cbu_attr_values.source` CHECK constraint extended with `'system'` for internal attribute values.

---

## Governance & Attribute Macros

The SemOS Maintenance workspace uses operator macros for multi-step governance workflows. Full macro system documentation is in `docs/annex-macros.md`.

**Governance macros** (in `rust/config/verb_schemas/macros/governance.yaml`):

| Macro | Steps | Purpose |
|-------|-------|---------|
| `governance.bootstrap-attribute-registry` | 3 | Bridge ungoverned → SemOS + sync SRDEFs + check gaps |
| `governance.define-service-dictionary` | 4 | Check gaps + sync + rollup + gaps |
| `governance.full-publish-pipeline` | 5 | Precheck + validate + dry-run + plan + publish |
| `governance.reconcile-registry` | 3 | Bridge + sync + recompute stale derived values |

**Attribute macros** (in `rust/config/verb_schemas/macros/attribute.yaml`):

| Macro | Steps | Purpose |
|-------|-------|---------|
| `attribute.seed-domain` | 1 | Generate attribute.define calls for a verb domain |
| `attribute.seed-derived` | 1 | Generate attribute.define-derived calls for a derivation domain |

All 6 macros are wired into the `semos-maintenance` pack (`allowed_verbs`) and the `semos_workspace` constellation map (6 slots under `# ── Governance macros`). Available only in the `sem_os_maintenance` workspace. Mode tags: `stewardship` / `governance`.

---

## Cross-Workspace State Consistency

**Architecture doc:** `docs/architecture/cross-workspace-state-consistency-v0.4.md`

When a shared fact (LEI, jurisdiction, fund structure type) is mutated in its owning workspace, consuming workspaces silently drift. The cross-workspace consistency mechanism detects drift, propagates staleness, and enables constellation replay.

**Key concepts:**
- **Shared Atom** — attribute owned by one workspace, consumed by others. Governed SemOS entity with lifecycle FSM (Draft → Active → Deprecated → Retired).
- **Superseded Attribute Version** — a shared fact version replaced by a newer one. Canonical origin of all downstream staleness (INV-1).
- **Constellation Replay** — full re-execution of a consuming constellation from the top. Upsert semantics = no-op for unchanged state.
- **Remediation Event** — lifecycle entity tracking the resolution of a supersession (Detected → Replaying → Resolved / Escalated → Resolved / Deferred).

**Implementation status (P1-P6 of 10):**

| Phase | Status | Deliverable |
|-------|--------|-------------|
| P1 | ✅ | `shared_atom_registry` table, 6 registry verbs, lifecycle FSM |
| P2 | ✅ | `shared_fact_versions` table, `produces_shared_facts` on VerbContractBody |
| P3 | ✅ | `workspace_fact_refs` table, pre-REPL staleness check, NarrationEngine blockers |
| P4 | ✅ | SQL staleness propagation trigger (three-stage) |
| P5 | ✅ | `RebuildContext` type, `replay-constellation` + `acknowledge-shared-update` verbs |
| P6 | ✅ | `remediation_events` table, FSM, 4 remediation verbs |
| P7 | ✅ | `external_call_log` table, idempotency check + record_call |
| P8 | ✅ | `provider_capabilities` table with 8 seed entries |
| P9 | ✅ | `compensation_records` table, 3 TraceOp variants |
| P10 | ✅ | 5 shared atom YAML seeds, platform DAG derivation (3 unit tests) |

**New tables (4 migrations):**
- `shared_atom_registry` — atom declarations with lifecycle status
- `shared_fact_versions` — versioned fact store (source of truth)
- `workspace_fact_refs` — consumption-state projection (consumer-held pointers)
- `remediation_events` — lifecycle entities for drift resolution

**New module:** `rust/src/cross_workspace/` (types, repository, fact_versions, fact_refs, replay, remediation)

**New verbs (12):**
- `shared-atom.*` (8): register, activate, deprecate, retire, list, list-consumers, replay-constellation, acknowledge-shared-update
- `remediation.*` (4): list-open, defer, revoke-deferral, confirm-external-correction

**New macros (10):**
- `shared-atom.*` (8): register-and-activate, full-consistency-check, deprecate-and-retire, detect-and-remediate, batch-replay, acknowledge-batch
- `remediation.*` (2): defer-with-audit-trail, resolve-with-confirmation
- Batch macros use `foreach:` for iteration over entity ID lists
- 6 scenario routes in `scenario_index.yaml`, 24 constellation map slots in `semos_workspace.yaml`

**Key invariants:**
- INV-1: Canonical unit of drift = shared attribute version
- INV-2: Consumer state is projection; shared fact version is source of truth
- INV-3: Replay scope = consuming constellation (not individual vertices)
- INV-4: Replay routes through existing runbook execution gate
- INV-5: Replay is controlled re-evaluation, not mechanical rerun
- INV-6: If shared, always enforced (no soft constraints)

---

## Catalogue Platform v1.3 — Cross-Workspace Runtime Stack (2026-04-25)

> **Spec:** `docs/todo/catalogue-platform-refinement-v1_3.md`
> **Status:** CODE COMPLETE — opt-in via `ReplOrchestratorV2::with_gate_pipeline`

The v1.3 stack adds runtime enforcement of cross-workspace semantics
declared in DAG taxonomies (`rust/config/sem_os_seeds/dag_taxonomies/*.yaml`).
Three modes:

- **Mode A (V1.3-1) blocking gate** — `cross_workspace_constraints:` —
  pre-transition check; verb dispatch rejected if source workspace's
  state doesn't match required.
- **Mode B (V1.3-2) aggregate / tollgate** — `derived_cross_workspace_state:` —
  derived states computed by AND-ing other workspaces' states (e.g.
  `cbu.operationally_active` = KYC.APPROVED AND Deal.CONTRACTED AND
  IM.trading_profile.ACTIVE AND evidence.verified).
- **Mode C (V1.3-3) hierarchy cascade** — `parent_slot:` + `state_dependency:` —
  parent transition propagates to child slots (e.g. parent CBU SUSPENDED →
  all child CBUs SUSPENDED).

### DAG inventory (9 workspaces)

| Workspace | DAG file | Slots | Notes |
|---|---|---|---|
| Instrument Matrix | `instrument_matrix_dag.yaml` | 22 | R-4 phase-axis re-anchored on CBU-trading-enablement |
| KYC | `kyc_dag.yaml` | 32 | Validation lifecycle |
| Deal | `deal_dag.yaml` | 22 | R-5: BAC gate + pricing approval + terminal granularity + SLA + dual_lifecycle |
| CBU | `cbu_dag.yaml` | 24 | R-3: re-centred on money-making apparatus; tollgate hosted here |
| SemOS Maintenance | `semos_maintenance_dag.yaml` | 14 | Governance — 5 stateful + 9 stateless slots |
| Book-Setup | `book_setup_dag.yaml` | 11 | Journey workspace — book-setup lifecycle (8 phases) |
| Session-Bootstrap | `session_bootstrap_dag.yaml` | 3 | Smallest DAG; transitional scope-resolution pack |
| Onboarding-Request | `onboarding_request_dag.yaml` | 5 | Deal→Ops handoff journey |
| Product-Service-Taxonomy | `product_service_taxonomy_dag.yaml` | 5 | Read-only catalog browsing |

### Runtime modules (dsl-core + dsl-runtime)

#### `dsl-core::config::dag_registry::DagRegistry`

Build-time index over loaded DAGs. Five lookup methods:

```rust
pub fn dag(&self, workspace: &str) -> Option<&Dag>;
pub fn constraints_for_transition(&self, ws, slot, from, to) -> Vec<&CrossWorkspaceConstraint>;
pub fn derived_states_for_slot(&self, ws, slot) -> Vec<&DerivedCrossWorkspaceState>;
pub fn parent_slot_for(&self, ws, slot) -> Option<&ParentSlot>;
pub fn children_of(&self, parent_ws, parent_slot) -> &[SlotKey];
pub fn transitions_for_verb(&self, verb_fqn: &str) -> &[TransitionRef];
```

Construct once at startup via `ConfigLoader::from_env().load_dag_registry()`.
Share as `Arc<DagRegistry>` across the runtime.

#### `dsl-runtime::cross_workspace::SlotStateProvider`

Trait abstracting cross-workspace slot-state lookups:

```rust
async fn read_slot_state(&self, ws, slot, entity_id, pool) -> Option<String>;
```

Implementation: `PostgresSlotStateProvider` with a 24-row dispatch
table mapping (workspace, slot) → (table, state_column, pk_column).
Add new mappings in `slot_state.rs::resolve_slot_table`.

#### `dsl-runtime::cross_workspace::SqlPredicateResolver`

Production `PredicateResolver` for the canonical FK-equality predicate
shape: `{src_table}.{src_col} = this_X.{tgt_col}`. Two-stage SQL
(read target column off target row, then SELECT source pk by matched
column). Identifier hygiene enforced. Unparseable predicates return
`Ok(None)` (gate checker treats as constraint violation).

#### `dsl-runtime::cross_workspace::GateChecker` (Mode A)

```rust
pub async fn check_transition(
    &self,
    target_ws, target_slot, target_entity_id,
    from_state, to_state, pool,
) -> Vec<GateViolation>;
```

`GateViolation` carries `severity` (error/warning/informational);
caller decides reject vs warn vs log.

#### `dsl-runtime::cross_workspace::DerivedStateEvaluator` + `DerivedStateProjector` (Mode B)

`DerivedStateEvaluator` evaluates one `DerivedCrossWorkspaceState`
against the live system. `DerivedStateProjector` composes registry
+ evaluator into a multi-host projection method:

```rust
pub async fn project_for(&self, host_ws, host_slot, host_entity_id, pool)
    -> Vec<DerivedStateProjection>;
pub async fn project_batch(&self, targets, pool) -> Vec<DerivedStateProjection>;
```

Per OQ-2: callers wrap in session-scope cache; the evaluator itself
is stateless.

#### `dsl-runtime::cross_workspace::CascadePlanner` + `PostgresChildEntityResolver` (Mode C)

`CascadePlanner.plan_cascade(parent_ws, parent_slot, parent_id, parent_new_state, pool)`
returns `Vec<CascadeAction>` — one action per child entity that needs
to react to the parent transition. Action carries
`(child_workspace, child_slot, child_entity_id, target_state)`.

`PostgresChildEntityResolver` consults the DAG's `parent_slot.join`
declaration (via, parent_fk, child_fk) and runs `SELECT child_fk FROM
{via} WHERE {parent_fk} = $1`.

### TransitionArgs metadata (per-verb opt-in)

Verbs that drive state-machine transitions declare `transition_args:`
in their YAML so the dispatch hook can extract entity_id + target
state from runtime args:

```yaml
update-status:
  description: ...
  behavior: plugin
  transition_args:
    entity_id_arg: deal-id
    target_state_arg: new-status        # optional — omit for fixed-target verbs
    target_workspace: deal               # optional — defaults to verb namespace
    target_slot: deal                    # optional — defaults to workspace
  # ... rest of verb declaration
```

87 verbs declare `transition_args` as of 2026-04-25 (CBU operational +
Deal BAC/SLA + holding encumbrance + share-class lifecycle + manco +
trading-profile + book + cbu-ca + service-consumption + investor +
governance/attribute/phrase publication).

### GatePipeline + Orchestrator hook

```rust
pub struct GatePipeline {
    pub registry: Arc<DagRegistry>,
    pub gate_checker: Arc<GateChecker>,
    pub verb_metadata: Arc<dyn VerbTransitionLookup>,  // HashMapVerbTransitionLookup default
    pub pool: Arc<PgPool>,
    pub cascade_planner: Option<Arc<CascadePlanner>>,
}

ReplOrchestratorV2::new(router, executor)
    .with_verb_execution_port(port)
    .with_gate_pipeline(pipeline);  // ← opt-in
```

When attached, `VerbExecutionPortStepExecutor::execute_step`:

1. **Pre-dispatch**: `pre_dispatch_gate_check()` runs Mode A. Skipped
   when verb has no `transition_args`. On error severity → step fails
   with v1.3 violation message.
2. **Dispatch**: existing `port.execute_verb(...)` call.
3. **Post-dispatch (success only)**: `post_dispatch_cascade()` runs
   Mode C. Plans cascades + applies single-level state writes via
   `SlotStateProvider`'s table mapping. Logs each fired action via
   `tracing::info!`.

Mode B (DerivedStateProjector) is consumer-side: callers (chat
response builders, observatory, narration) invoke it on the read
path to surface aggregate states like `cbu.operationally_active`.

### Production startup (one block)

```rust
let cfg      = ConfigLoader::from_env().load_verbs()?;
let registry = Arc::new(loader.load_dag_registry()?);
let provider = Arc::new(PostgresSlotStateProvider);
let resolver = Arc::new(SqlPredicateResolver);
let gate     = Arc::new(GateChecker::new(registry.clone(), provider.clone(), resolver.clone()));
let evaluator= Arc::new(DerivedStateEvaluator::new(provider, resolver));
let child    = Arc::new(PostgresChildEntityResolver::new(registry.clone()));
let cascade  = Arc::new(CascadePlanner::new(registry.clone(), child));
let lookup   = Arc::new(HashMapVerbTransitionLookup::from_verbs_config(&cfg));
let pipeline = GatePipeline {
    registry: registry.clone(),
    gate_checker: gate,
    verb_metadata: lookup,
    pool: Arc::new(pool),
    cascade_planner: Some(cascade),
};
let orch = ReplOrchestratorV2::new(router, executor)
    .with_verb_execution_port(port)
    .with_gate_pipeline(pipeline);

// Read-path projection
let projector = Arc::new(DerivedStateProjector::new(registry, evaluator));
```

### Schema migrations

- `rust/migrations/20260424_tranche_2_3_dag_alignment.sql` — CBU
  operational + disposition columns; cbu_service_consumption +
  cbu_trading_activity + cbu_corporate_action_events tables;
  share_classes lifecycle; deals BAC + SLA + accountability +
  parent_deal_id; client_books + cbus.book_id.
- `rust/migrations/20260425_manco_regulatory_status.sql` — manco
  state-machine carrier table.

### Limits / follow-ups

- **Recursive cascades**: current cascade execution is single-level
  direct state writes. Multi-level cascade-of-cascades (each level
  re-entering verb dispatch with its own gate checks) is a follow-up.
- **Session-scope derived-state cache**: per OQ-2 should cache
  aggregates within a session, invalidating on touched-slot writes.
  Cache infrastructure not yet wired (each `project_for` is a fresh
  round-trip).
- **ob-poc-web wiring**: `with_gate_pipeline(...)` currently isn't
  called in `main.rs` — orchestrator defaults to no enforcement
  until ops turns it on.

### Key files

- `rust/crates/dsl-core/src/config/dag.rs` — DAG taxonomy YAML types
- `rust/crates/dsl-core/src/config/dag_registry.rs` — runtime index
- `rust/crates/dsl-core/src/config/dag_validator.rs` — build-time validation
- `rust/crates/dsl-runtime/src/cross_workspace/slot_state.rs` — provider
- `rust/crates/dsl-runtime/src/cross_workspace/sql_predicate_resolver.rs`
- `rust/crates/dsl-runtime/src/cross_workspace/gate_checker.rs`
- `rust/crates/dsl-runtime/src/cross_workspace/derived_state.rs`
- `rust/crates/dsl-runtime/src/cross_workspace/derived_state_projector.rs`
- `rust/crates/dsl-runtime/src/cross_workspace/hierarchy_cascade.rs`
- `rust/crates/dsl-runtime/src/cross_workspace/postgres_child_resolver.rs`
- `rust/src/runbook/step_executor_bridge.rs` — `GatePipeline` + dispatch hook
- `rust/src/sequencer.rs` — `ReplOrchestratorV2::with_gate_pipeline`
- `rust/config/sem_os_seeds/dag_taxonomies/*.yaml` — 9 DAG taxonomies
