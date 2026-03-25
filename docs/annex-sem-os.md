# Semantic OS & SemReg — Detailed Annex

> This annex covers Semantic OS, SemReg, context resolution (CCIR), ABAC,
> stewardship, governed authoring, SessionVerbSurface, scanner/seed bundles,
> constellation maps, and state machines.
> For the high-level overview see the root `CLAUDE.md`.

---

## Architecture Overview

Semantic OS is the **single authoritative source of truth** for verb availability. It is not a separate system — it is integrated into the core orchestrator at Stage 2.5 (CCIR) and Stage 2.5 (SessionVerbSurface). All DSL verb discovery flows through it. No exceptions, no bypasses.

**Crates:**

| Crate | Purpose |
|-------|---------|
| `sem_os_core` | Pure domain types, ABAC, context resolution, ports (no sqlx) |
| `sem_os_postgres` | PostgreSQL implementations of ports |
| `sem_os_server` | Standalone REST API + JWT auth |
| `sem_os_client` | Trait: in-process or HTTP access |
| `sem_os_harness` | Integration test framework (102 scenarios) |
| `sem_os_obpoc_adapter` | Verb YAML → seed bundle conversion |

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
| 1 | Base set from RuntimeVerbRegistry (~1,200 verbs) |
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
    Research,    // Exploration, introspection, changeset authoring
    #[default]
    Governed,    // Validated business verbs, publishing
}
```

| Verb Category | Research | Governed |
|---|---|---|
| Business verbs (cbu.*, entity.*, kyc-case.*) | Blocked | Allowed (via SemReg) |
| Authoring (authoring.propose, authoring.validate) | Allowed | Blocked |
| Changeset (changeset.*, review, publish) | Allowed | Blocked (propose only) |
| Introspection (db_introspect.*) | Full surface | verify + describe only |
| Registry/schema/focus/audit/agent | Allowed | Allowed |

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

`OnboardingStateView` is computed from live DB composite state and returned on every `ChatResponse`.

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
| `rust/crates/sem_os_obpoc_adapter/src/scanner.rs` | Pure conversion functions |
| `rust/crates/sem_os_obpoc_adapter/src/metadata.rs` | `DomainMetadata` loader |
| `rust/config/sem_os_seeds/` | Universes, constellations, state machines, metadata |
