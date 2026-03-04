# Semantic OS — Vision & Scope v3.0

> **Version:** 3.0
> **Date:** 2026-02-27
> **Status:** Living document — consolidation of 9 prior specs
> **Audience:** Engineering, governance, architecture review

---

## 1. Executive Summary

Semantic OS is an **immutable, governance-aware knowledge registry** that serves as the single source of truth for what exists in the system — attributes, entities, verbs, policies, evidence requirements, taxonomies, and their relationships.

It answers three questions at any point in time:

1. **What exists?** — 13 object types stored as immutable snapshots
2. **Who can access it?** — ABAC with security labels, classification, PII tracking
3. **What should the agent do next?** — Context resolution pipeline returning ranked, tier-aware, snapshot-pinned candidates

Semantic OS enforces governance through **three complementary layers**:

| Layer | Enforcement Point | Mechanism |
|-------|-------------------|-----------|
| **Authoring Pipeline** | What enters the registry | ChangeSets, validation stages, stewardship guardrails |
| **Publish Gates** | What becomes active | Proof rule, security labels, version monotonicity |
| **GovernedQuery** | What compiles | Proc macro checks against bincode cache at compile time |

The system is deployed as a **standalone service** (6 Rust crates) with port-trait isolation, REST+JWT API, outbox-driven projections, and optional in-process mode for the ob-poc monolith.

---

## 2. Problem Statement

### Gaps in the Pre-Semantic OS Landscape

| Gap | Consequence | Resolution |
|-----|-------------|------------|
| No formal attribute model | Fields added ad-hoc; no data type, sensitivity, or ownership metadata | `AttributeDef` with data types, constraints, security labels |
| No verb contract registry | Functions called without precondition/postcondition knowledge | `VerbContract` with preconditions, postconditions, required attributes |
| No entity type definitions | Entity kinds implicit in code, not queryable | `EntityTypeDef` with required/optional attribute sets |
| No access control model | All data equally accessible | ABAC with `ActorContext`, `AccessPurpose`, classification-based decisions |
| No governance tiers | Research output and production facts treated identically | `GovernanceTier` (Governed vs Operational) with distinct workflow rigor |
| No trust classification | No way to distinguish proof-grade from convenience data | `TrustClass` (Proof, DecisionSupport, Convenience) with Proof Rule |
| No change tracking | Schema/definition changes committed without audit | Immutable snapshots, content-addressed ChangeSets, governance audit log |
| No point-in-time queries | Cannot answer "what was the state on date X?" | `resolve_at(type, id, as_of)` against immutable snapshot chain |
| No evidence framework | Document requirements and freshness not modeled | `EvidenceRequirement`, observations, freshness contracts |
| Compile-time blind spot | Deprecated/retired verbs discovered only at runtime | `#[governed_query]` proc macro catches lifecycle violations at compile time |

---

## 3. Product Vision

### Foundational Principles

1. **Registry as compiler input** — The registry is not documentation; it is a machine-readable contract that tools, agents, and compilers consume directly.

2. **Immutability** — Every change produces a new snapshot. No in-place updates. Full audit trail.

3. **Governance-aware, not governance-gated** — Both Governed and Operational tiers carry security labels and ABAC. The tier determines *workflow rigor*, not *security posture*.

4. **Compose, don't replace** — Semantic OS composes on top of existing infrastructure (sqlx, Axum, PostgreSQL). It does not require replacing the query layer or execution engine.

### Three Enforcement Layers

```
                    WHAT ENTERS              WHAT ACTIVATES           WHAT COMPILES
                    ───────────              ──────────────           ─────────────
                    Authoring Pipeline       Publish Gates            GovernedQuery
                    │                        │                        │
                    ├─ ChangeSets            ├─ Proof Rule            ├─ Verb lifecycle
                    ├─ Validation (2 stages) ├─ Security Label        ├─ Principal requirement
                    ├─ Stewardship guardrails├─ Version monotonicity  ├─ PII authorization
                    ├─ AgentMode gating      ├─ Governed approval     ├─ Proof rule
                    └─ Content-addressed     └─ Gate framework        └─ Attribute lifecycle
                       idempotency              (5 governance gates)
```

### Non-Negotiable Invariants

| ID | Invariant | Enforcement |
|----|-----------|-------------|
| I-1 | No in-place updates — every change produces a new immutable snapshot | INSERT-only store, immutability trigger on `sem_reg.snapshots` |
| I-2 | Proof Rule — only `Governed` tier may have `TrustClass::Proof` | Publish gate + runtime check |
| I-3 | Security labels on both tiers — classification, PII, jurisdictions apply regardless of governance tier | Gate enforcement |
| I-4 | Operational auto-approved — no governed approval gates on operational-tier iteration | Tier-based gate bypass |
| I-5 | Point-in-time resolution — `resolve_active(type, id)` and `resolve_at(type, id, as_of)` | Snapshot chain traversal |
| I-6 | Snapshot manifest — every decision record pins the exact snapshot IDs it relied on | `snapshot_manifest: HashMap<Uuid, Uuid>` on `DecisionRecord` |

---

## 4. Architecture Overview

### Layer Cake

```
┌─────────────────────────────────────────────────────────────────────┐
│  CONSUMERS                                                          │
│  Agent (MCP tools)  │  REPL Pipeline  │  CLI (cargo x sem-reg)     │
├─────────────────────┴─────────────────┴────────────────────────────┤
│  API BOUNDARY                                                       │
│  SemOsClient trait (InProcessClient │ HttpClient)                  │
├────────────────────────────────────────────────────────────────────┤
│  SERVICE LAYER                                                      │
│  CoreService (context resolution, publish, bootstrap, authoring)   │
├────────────────────────────────────────────────────────────────────┤
│  DOMAIN                                                             │
│  Types │ Gates │ ABAC │ Security │ Stewardship │ Context Resolution│
├────────────────────────────────────────────────────────────────────┤
│  PORTS (8 traits)                                                   │
│  SnapshotStore │ ChangesetStore │ OutboxStore │ AuditStore │ ...   │
├────────────────────────────────────────────────────────────────────┤
│  ADAPTERS                                                           │
│  PgSnapshotStore │ PgChangesetStore │ PgOutboxStore │ ...          │
├────────────────────────────────────────────────────────────────────┤
│  STORAGE                                                            │
│  PostgreSQL (sem_reg │ sem_reg_pub │ sem_reg_authoring │ stewardship)│
└────────────────────────────────────────────────────────────────────┘
```

### 6 Crates

| Crate | Responsibility | Dependencies |
|-------|---------------|--------------|
| `sem_os_core` | Domain types, service logic, gates, ABAC, stewardship guardrails | No DB dependencies (port traits only) |
| `sem_os_postgres` | 8 PostgreSQL store adapters implementing port traits | `sqlx`, `sem_os_core` |
| `sem_os_server` | Axum REST server, JWT auth, CORS, outbox dispatcher | `sem_os_core`, `sem_os_postgres` |
| `sem_os_client` | `SemOsClient` trait + `InProcessClient` + `HttpClient` | `sem_os_core` |
| `sem_os_harness` | Integration test harness (isolated DB per run) | All crates |
| `sem_os_obpoc_adapter` | Verb YAML → seed bundles, scanner with CRUD/entity-type resolution | `sem_os_core`, `dsl-core` |

### 3 Planes

| Plane | Purpose | Key Operations |
|-------|---------|----------------|
| **Research** | Unconstrained exploration, schema design, attribute discovery | `propose`, `validate`, `dry-run`, `plan`, `diff` |
| **Governed** | Production-grade publishing with gates and audit | `publish`, `rollback`, business verbs |
| **Runtime** | Point-in-time resolution, context-aware verb/attribute selection | `resolve_context`, `dispatch_tool` |

### 8 Port Traits

| Trait | Methods | Purpose |
|-------|---------|---------|
| `SnapshotStore` | save, resolve_active, resolve_at, supersede, list | Core snapshot persistence |
| `ChangesetStore` | create, update_status, list, get, entries | Changeset workflow |
| `OutboxStore` | enqueue, claim, advance_watermark | Event-driven projections |
| `AuditStore` | append, query | Governance audit log |
| `EvidenceInstanceStore` | observations, documents, provenance | Evidence layer |
| `ObjectStore` | generic typed CRUD for all 13 types | Typed convenience layer |
| `ProjectionWriter` | lineage, embeddings, metrics | Projection persistence |
| `BootstrapAuditStore` | check, start, mark_published, mark_failed | Idempotent seed tracking |

---

## 5. Core Domain Model

### 13 Object Types

All 13 types share a single table (`sem_reg.snapshots`) with type-specific bodies stored as JSONB:

| Object Type | Body Struct | Domain |
|-------------|-------------|--------|
| `attribute_def` | `AttributeDefBody` | Data attributes: type, constraints, sensitivity, source triples |
| `entity_type_def` | `EntityTypeDefBody` | Entity kinds with required/optional attribute sets |
| `relationship_type_def` | `RelationshipTypeDefBody` | Typed edges between entity types (edge_class, directionality) |
| `verb_contract` | `VerbContractBody` | Preconditions, postconditions, required attributes, subject_kinds |
| `taxonomy_def` | `TaxonomyDefBody` | Hierarchical classification trees |
| `taxonomy_node` | `TaxonomyNodeBody` | Individual nodes within a taxonomy |
| `membership_rule` | `MembershipRuleBody` | Conditional rules governing taxonomy membership |
| `view_def` | `ViewDefBody` | Verb surface + attribute prominence for a context |
| `policy_rule` | `PolicyRuleBody` | Conditions → verdicts (Allow, Deny, Escalate) |
| `evidence_requirement` | `EvidenceRequirementBody` | Freshness, source, and sufficiency requirements |
| `document_type_def` | `DocumentTypeDefBody` | Document type classification |
| `observation_def` | `ObservationDefBody` | Observation recording templates |
| `derivation_spec` | `DerivationSpecBody` | Derived/composite attribute computation specs |

### Snapshot Structure

```
┌─────────────────────────────────────────────────────────────┐
│  sem_reg.snapshots                                          │
│                                                             │
│  snapshot_id       UUID (PK)                                │
│  object_type       ENUM (13 variants)                       │
│  object_id         UUID (deterministic v5 from type:fqn)    │
│  fqn               TEXT (fully qualified name)              │
│  version           INTEGER (monotonically increasing)       │
│  governance_tier   ENUM (governed, operational)             │
│  trust_class       ENUM (proof, decision_support, convenience)│
│  status            ENUM (draft, active, deprecated, retired)│
│  security_label    JSONB                                    │
│  definition        JSONB (type-specific body)               │
│  predecessor_id    UUID (supersession chain)                │
│  created_at        TIMESTAMPTZ                              │
│  created_by        TEXT                                     │
│                                                             │
│  CONSTRAINT: INSERT-only (immutability trigger)             │
│  CONSTRAINT: status transitions validated                   │
└─────────────────────────────────────────────────────────────┘
```

### Snapshot Lifecycle

```
Draft ──► Active ──► Deprecated ──► Retired
                        │
                        └─► (superseded by new Active snapshot)
```

- **Draft** → Active: Publish gates pass
- **Active** → Deprecated: Successor published (grace period for consumers)
- **Deprecated** → Retired: Grace period expired, no longer resolvable
- **Supersession**: New snapshot links to predecessor via `predecessor_id`

### Security Labels

Every snapshot carries a security label regardless of governance tier:

```rust
struct SecurityLabel {
    classification: Classification,      // Public, Internal, Confidential, Restricted
    pii: bool,                           // Personal Identifiable Information flag
    jurisdictions: Vec<String>,          // Applicable jurisdictions (e.g., "LU", "US")
    handling_controls: Vec<HandlingControl>, // Additional handling requirements
}
```

**Inheritance**: When a derived attribute references inputs, its security label is computed as the maximum classification of all inputs. PII propagates transitively.

### Deterministic Object IDs

Object IDs use UUID v5 (deterministic from `object_type:fqn`):

```
object_id = uuid_v5(NAMESPACE, "attribute_def:cbu.jurisdiction_code")
```

Same YAML on any machine produces the same IDs. This enables idempotent re-bootstrap and drift detection.

---

## 6. Governance & Trust Model

### Governance Tiers

The governance tier determines **workflow rigor**, not security posture:

| Tier | Workflow | Approval | Use Case |
|------|----------|----------|----------|
| **Governed** | Full pipeline (propose → validate → dry-run → publish) | Required (stewardship review) | Production facts, compliance-grade definitions |
| **Operational** | Lightweight (propose → auto-approve → publish) | Auto-approved | Agent scratch work, exploratory definitions, convenience data |

Both tiers carry full security labels and ABAC enforcement.

### Trust Classes

| Class | Meaning | Tier Constraint |
|-------|---------|-----------------|
| **Proof** | Auditable, evidence-backed, suitable for regulatory reporting | Governed only (Proof Rule) |
| **DecisionSupport** | Reliable for business decisions, not regulatory-grade | Either tier |
| **Convenience** | Helpful but not authoritative | Either tier |

**Proof Rule (I-2)**: `TrustClass::Proof` requires `GovernanceTier::Governed`. This is enforced at publish time by the proof rule gate and at compile time by the GovernedQuery proc macro.

### ABAC Access Control

Every data access is evaluated against the actor's context:

```rust
struct ActorContext {
    actor_type: ActorType,          // Agent, Analyst, Governance, System
    roles: Vec<String>,             // e.g., ["operator", "kyc_analyst"]
    clearance: Classification,      // Actor's maximum classification level
    purpose: AccessPurpose,         // KYC, Trading, Compliance, Sanctions, ...
    jurisdiction: Option<String>,   // Actor's jurisdiction
}

enum AccessDecision {
    Allow,
    Deny { reason: String },
    AllowWithConstraints { constraints: Vec<Constraint> },
}
```

**Evaluation rules**:
1. Actor clearance must meet or exceed snapshot classification
2. PII-labelled snapshots require explicit PII purpose
3. Jurisdiction restrictions are enforced (snapshot jurisdictions ∩ actor jurisdiction)
4. Purpose-specific restrictions (e.g., sanctions-labelled data requires Sanctions purpose)

---

## 7. Authoring Pipeline — Research→Governed Change Boundary

### Two-Plane Model

The authoring pipeline separates **research** (unconstrained exploration) from **governed** (audited publication):

```
┌─────────────────────────────────────────────────────────────────────┐
│  RESEARCH PLANE                       GOVERNED PLANE                │
│                                                                     │
│  propose_change_set()                publish_snapshot_set()         │
│       │                                   │                         │
│       ▼                                   ▼                         │
│  ChangeSet (Draft)                  Advisory lock                   │
│       │                                   │                         │
│       ▼                                   ▼                         │
│  validate (Stage 1+2)              Drift detection                  │
│       │                                   │                         │
│       ▼                                   ▼                         │
│  dry_run                           Apply + publish                  │
│       │                                   │                         │
│       ▼                                   ▼                         │
│  plan_publish ─────────────────► ChangeSet (Published)              │
└─────────────────────────────────────────────────────────────────────┘
```

### ChangeSet Lifecycle (9-State)

```
Draft → UnderReview → Approved → Validated → DryRunPassed → Published
  │                                  │            │
  └→ Rejected                        └→ Rejected  └→ DryRunFailed
                                                        │
                                                        └→ Superseded
```

### Content-Addressed Idempotency

ChangeSets are identified by `(hash_version, content_hash)`:

```
content_hash = SHA-256(canonical_json(sorted_artifacts))
```

Proposing the same bundle twice returns the existing ChangeSet. The UNIQUE index excludes rejected/superseded ChangeSets.

### Validation Pipeline

| Stage | Environment | Checks |
|-------|-------------|--------|
| **Stage 1** (pure) | No DB required | Hash verification, SQL parsing, YAML parsing, reference resolution, dependency cycle detection |
| **Stage 2** (needs DB) | Scratch schema | DDL safety (no `CONCURRENTLY`, no `DROP TABLE`), compatibility diff, breaking change detection |

### 7 Governance Verbs

| Verb | Transition | Purpose |
|------|-----------|---------|
| `propose` | → Draft | Parse bundle, compute content_hash |
| `validate` | Draft → Validated | Stage 1 artifact integrity |
| `dry_run` | Validated → DryRunPassed | Stage 2 scratch schema |
| `plan_publish` | (read-only) | Diff against active, impact analysis |
| `publish` | DryRunPassed → Published | Advisory lock, drift detect, apply, audit |
| `rollback` | (pointer revert) | Revert active_snapshot_set pointer |
| `diff` | (read-only) | Structural diff between ChangeSets |

### AgentMode Gating

| Mode | Allowed | Blocked |
|------|---------|---------|
| **Research** | Authoring verbs, full `db_introspect`, SemReg reads | Business verbs, publish/rollback |
| **Governed** | Business verbs, publish/rollback, limited `db_introspect` | Authoring exploration verbs |

Default: `Governed`. Mode switch via `agent.set-mode`.

---

## 8. Stewardship Layer

### Purpose

The Stewardship Agent provides a **human-in-the-loop governance layer** for registry changes. It adds guardrails, conflict detection, basis records (evidence for decisions), and a Show Loop for iterative refinement.

### Guardrails (G01-G15)

| Severity | Rules | Description |
|----------|-------|-------------|
| **Block** | G01, G03-G08, G15 | Role permission, type constraint, proof chain, classification, security label, silent meaning change, deprecation, draft uniqueness |
| **Warning** | G02, G10-G13 | Naming conventions, conflicts, stale templates, observation impact, resolution metadata |
| **Advisory** | G09, G14 | AI knowledge boundary, composition hints |

### Show Loop

The Show Loop is an iterative refinement cycle for governed changes:

```
Focus → Read → Propose → Show → Refine → (loop)
```

**4 Viewports:**

| Viewport | Key | Content |
|----------|-----|---------|
| Focus Summary | A | Current focus object, status, metadata |
| Object Inspector | C | Full definition, attributes, relationships |
| Diff | D | Predecessor vs. draft comparison |
| Gates | G | Publish gate pre-check results |

### MCP Tools (23 total)

| Category | Count | Examples |
|----------|-------|---------|
| Compose | 4 | `stew_compose_changeset`, `stew_add_item`, `stew_remove_item`, `stew_refine_item` |
| Evidence | 2 | `stew_attach_basis`, `stew_resolve_conflict` |
| Workflow | 4 | `stew_submit_for_review`, `stew_approve_changeset`, `stew_publish_changeset` |
| Query | 5 | `stew_list_changesets`, `stew_describe_changeset`, `stew_compute_impact` |
| Show Loop | 6 | `stew_get_focus`, `stew_set_focus`, `stew_show`, `stew_get_viewport` |
| Suggest | 1 | `stew_suggest` |

### Basis Records & Conflict Detection

- **Basis records**: Evidence attached to changeset entries (documents, observations, external references)
- **Basis claims**: Specific claims derived from basis records
- **Conflict detection**: Automatic detection of concurrent modifications to the same object

---

## 9. Context Resolution

### 12-Step Pipeline

The `resolve_context()` function returns ranked, tier-aware, snapshot-pinned candidates:

```
┌─────────────────────────────────────────────────────────────────────┐
│  resolve_context(subject, actor, goals, evidence_mode, as_of)      │
│                                                                     │
│   1. Determine snapshot epoch (point_in_time or now)               │
│   2. Resolve subject → entity type + jurisdiction + state          │
│  2c. Load subject relationships (edge_class, directionality)       │
│   3. Select applicable ViewDefs by taxonomy overlap                │
│   4. Extract verb surface + attribute prominence from top view     │
│   5. Filter verbs by taxonomy membership + ABAC                   │
│   6. Filter attributes similarly                                   │
│   7. Rank by ViewDef prominence + relationship overlap             │
│   8. Evaluate preconditions for top-N candidate verbs              │
│   9. Evaluate PolicyRules → PolicyVerdicts with snapshot refs      │
│  10. Compute composite AccessDecision                              │
│  11. Generate governance signals (unowned, stale, gaps)            │
│  12. Compute confidence score (deterministic heuristic)            │
└─────────────────────────────────────────────────────────────────────┘
```

### Evidence Modes

| Mode | Behavior |
|------|----------|
| **Strict** | Only Governed + Proof/DecisionSupport primary |
| **Normal** | Governed primary; Operational if view allows, tagged `usable_for_proof = false` |
| **Exploratory** | All tiers, annotated with governance tier |
| **Governance** | Coverage metrics focus |

### Response Structure

```rust
struct ContextResolutionResponse {
    as_of_time: DateTime<Utc>,
    applicable_views: Vec<ViewCandidate>,
    candidate_verbs: Vec<VerbCandidate>,
    candidate_attributes: Vec<AttributeCandidate>,
    required_preconditions: Vec<PreconditionStatus>,
    disambiguation_questions: Vec<String>,
    evidence: Vec<EvidenceItem>,
    policy_verdicts: Vec<PolicyVerdict>,
    security_handling: SecurityHandling,
    governance_signals: Vec<GovernanceSignal>,
    confidence: f64,
}
```

### CCIR — Context-Constrained Intent Resolution

The `ContextEnvelope` carries the full SemReg resolution output into the intent pipeline:

```
SemReg resolve_context() → ContextEnvelope {
    allowed_verbs: HashSet<String>,
    pruned_verbs: Vec<PrunedVerb>,       // 7 PruneReason variants
    fingerprint: AllowedVerbSetFingerprint,  // SHA-256 for TOCTOU
    evidence_gaps, governance_signals,
    snapshot_set_id,
}
```

**PruneReason variants**: `AbacDenied`, `EntityKindMismatch`, `TierExcluded`, `TaxonomyNoOverlap`, `PreconditionFailed`, `AgentModeBlocked`, `PolicyDenied`

Allowed verbs are threaded as **pre-constraints** into verb search (not just post-filter). TOCTOU recheck compares fingerprints before execution.

---

## 10. GovernedQuery — Compile-Time Enforcement

### Purpose

GovernedQuery is a Rust proc macro that makes the Semantic OS registry a **compiler input**. Functions annotated with `#[governed_query(verb = "cbu.create")]` are checked at compile time against a governance cache. This catches lifecycle violations, missing authorization, and PII handling errors before code ships.

### Architecture

```
assets/governed_cache.bin  (bincode, generated by xtask)
        │
        ▼
governed_query_proc crate  (proc-macro, reads cache at compile time)
        │
        ▼
#[governed_query(verb = "cbu.create")]
fn create_cbu(pool: &PgPool, principal: &Principal, ...) -> Result<...>
```

### 5 Governance Checks

| # | Check | Error On | Condition |
|---|-------|----------|-----------|
| 1 | Verb lifecycle | `compile_error!` | Verb not found OR status = Deprecated/Retired |
| 2 | Principal requirement | `compile_error!` | Governed tier AND no `&Principal` param AND !skip_principal_check |
| 3 | PII authorization | `compile_error!` | Verb/attr has pii = true AND !allow_pii |
| 4 | Proof rule | `compile_error!` | trust_class = Proof AND governance_tier != Governed |
| 5 | Attribute lifecycle | `compile_error!` | Referenced attr status = Deprecated/Retired |

### Usage

```rust
// Active governed verb with Principal — compiles
#[governed_query(verb = "cbu.create")]
fn create_cbu(pool: &PgPool, principal: &Principal, name: &str) -> Result<Uuid> {
    // ...
}

// PII verb — requires allow_pii
#[governed_query(verb = "entity.get-pii", attrs = ["entity.tax_id"], allow_pii = true)]
fn get_entity_pii(pool: &PgPool, principal: &Principal, id: Uuid) -> Result<PiiData> {
    // ...
}

// System-internal function — skip Principal check
#[governed_query(verb = "agent.set-mode", skip_principal_check = true)]
fn set_agent_mode(pool: &PgPool, mode: AgentMode) -> Result<()> {
    // ...
}
```

### Cache Management

```bash
# Generate/refresh cache from database
cargo x governed-cache refresh

# View cache statistics
cargo x governed-cache stats

# Run soft-warning checker (deprecation approaching, unused PII auth)
cargo x governed-check
```

### Compose-Not-Replace

GovernedQuery **composes on top of sqlx** — it does not replace the query layer:

```rust
// GovernedQuery verifies governance at compile time
// sqlx verifies SQL at compile time
// Both coexist on the same function
#[governed_query(verb = "cbu.create")]
async fn create_cbu(pool: &PgPool, principal: &Principal, name: &str) -> Result<Uuid> {
    sqlx::query_scalar!("INSERT INTO cbus (name) VALUES ($1) RETURNING cbu_id", name)
        .fetch_one(pool)
        .await
}
```

### Bootstrap Mode

When building for the first time (before the cache exists), set `GOVERNED_CACHE_SKIP=1` to bypass governance checks. The macro emits the function unchanged.

---

## 11. Agent Control Plane

### Plans, Decisions, Escalations

The agent control plane provides structured planning and decision-making with full snapshot provenance:

| Type | Purpose | Key Field |
|------|---------|-----------|
| `AgentPlan` | Multi-step plan with goal and risk assessment | `context_resolution_ref` |
| `PlanStep` | Individual step pinning verb_id + verb_snapshot_id | `verb_snapshot_id` |
| `DecisionRecord` | Immutable decision with complete provenance chain | `snapshot_manifest: HashMap<Uuid, Uuid>` |
| `EscalationRecord` | Human escalation with context and required action | `required_action` |
| `DisambiguationPrompt` | Disambiguation with options and selected choice | `selected_option_id` |

### ~32 MCP Tools (6 categories)

| Category | Tools | Purpose |
|----------|-------|---------|
| Registry query | 7 | Read-only registry lookup (describe attribute/verb/entity_type, search, list) |
| Taxonomy | 3 | Taxonomy navigation (tree, members, classify) |
| Impact/lineage | 5 | Dependency and provenance queries (verb_surface, impact_analysis, lineage) |
| Context resolution | 3 | Context resolution pipeline (resolve_context, describe_view, apply_view) |
| Planning/decisions | 7 | Agent planning and recording (create_plan, add_step, validate, execute, record_decision) |
| Evidence | 3 | Evidence management (record_observation, check_freshness, identify_gaps) |

### Evidence Layer

**4-State Freshness Contract:**

| State | Meaning |
|-------|---------|
| `unknown_no_policy` | No evidence requirement defined for this attribute |
| `unknown_no_observation` | Policy exists but no observation recorded yet |
| `stale` | Observation exists but exceeds freshness threshold |
| `fresh` | Observation exists and is within freshness threshold |

**Entity-Centric Observations**: `attribute_observations` table links observations to specific entities (subject_ref + attribute_fqn), alongside the snapshot-centric `observations` table.

---

## 12. Deployment & Operations

### Standalone Server

```bash
# Start Semantic OS server (standalone, port 4100)
SEM_OS_DATABASE_URL="postgresql:///data_designer" \
  SEM_OS_JWT_SECRET=dev-secret \
  cargo run -p sem_os_server
```

### Environment Variables

| Variable | Default | Purpose |
|----------|---------|---------|
| `SEM_OS_MODE` | `inprocess` | `inprocess` = direct CoreService, `remote` = REST client |
| `SEM_OS_URL` | — | Base URL for remote mode |
| `SEM_OS_DATABASE_URL` | — | Postgres connection string |
| `SEM_OS_JWT_SECRET` | — | Shared secret for JWT signing/verification |
| `SEM_OS_BIND_ADDR` | `0.0.0.0:4100` | Listen address |
| `SEM_OS_DISPATCHER_INTERVAL_MS` | `500` | Outbox dispatcher poll interval |

### Outbox-Driven Projections

```
Publish snapshot → INSERT snapshot + enqueue outbox event (single tx)
       │
       ▼
OutboxDispatcher (background task, configurable interval)
       │
       ├─► Claim event (FOR UPDATE SKIP LOCKED)
       ├─► Project to read-optimized tables (sem_reg_pub.active_snapshot_set)
       └─► Advance watermark
```

### Database Schemas

| Schema | Purpose |
|--------|---------|
| `sem_reg` | Core: snapshots, snapshot_sets, outbox_events, changesets, agent plans/decisions |
| `sem_reg_pub` | Read-optimized projections: active_snapshot_set, projection_watermark |
| `sem_reg_authoring` | Authoring: validation_reports, governance_audit_log, publish_batches, artifacts, archives |
| `stewardship` | Stewardship: events, basis_records, basis_claims, conflict_records, focus_states, viewport_manifests |

### Migrations (078-103)

| Range | Purpose |
|-------|---------|
| 078-086 | Core Semantic OS (Phases 0-9): snapshots, agent, projections |
| 087-089 | Agent/runbook infrastructure |
| 090-091 | Evidence instance layer + peer review fixes |
| 092-094 | Standalone service: outbox, bootstrap audit, projections |
| 095-098 | Changesets + stewardship (Phase 0-1) |
| 099-100 | Governed registry authoring + archive tables |
| 101-102 | Standalone remediation (CHECK constraint, schema ownership) |
| 103 | CCIR telemetry columns |

### CLI Commands

```bash
cd rust/

# Registry overview
cargo x sem-reg stats                     # Counts by object type
cargo x sem-reg validate [--enforce]      # Run publish gates on all active snapshots

# Object inspection
cargo x sem-reg attr-describe <fqn>       # Describe attribute
cargo x sem-reg verb-describe <fqn>       # Describe verb contract
cargo x sem-reg verb-list [-n 100]        # List active verbs
cargo x sem-reg history <type> <fqn>      # Snapshot history

# Context resolution
cargo x sem-reg ctx-resolve --subject <uuid> --subject-type <type> \
    --actor <role> --mode <strict|normal|exploratory|governance>

# Authoring
cargo x sem-reg authoring-list [--status draft|validated|published]
cargo x sem-reg authoring-propose /path/to/bundle/
cargo x sem-reg authoring-validate <changeset-id>
cargo x sem-reg authoring-publish <changeset-id> --publisher <name>

# GovernedQuery
cargo x governed-cache refresh
cargo x governed-cache stats
cargo x governed-check

# Coverage
cargo x sem-reg coverage [--tier governed|operational|all] [--json]
```

### Health Endpoints

| Endpoint | Purpose |
|----------|---------|
| `GET /health` | Basic health check |
| `GET /health/semreg/pending-changesets` | Pending ChangeSet counts by status |
| `GET /health/semreg/stale-dryruns` | Stale dry-run detection |

---

## Appendix A: Document Lineage

This document consolidates and supersedes the following specifications:

| Document | Version | Status | Disposition |
|----------|---------|--------|-------------|
| `semantic-os-v2.1.md` | 2.1 | Strategic vision | **Absorbed** — Sections 1-3 |
| `semantic-os-v1.1.md` | 1.1 | Technical specification | **Absorbed** — Sections 5-6, 9, 11 |
| `semantic-os-standalone-service-v2.0_1.md` | 2.0.1 | Standalone architecture | **Absorbed** — Sections 4, 12 |
| `semantic_os_research_governed_boundary_v0.4.md` | 0.4 | Authoring pipeline | **Absorbed** — Section 7 |
| `semantic-os-research-governed-boundary-v1.0_1.md` | 1.0.1 | Operating model | **Absorbed** — Sections 7-8 |
| `stewardship-agent-architecture-v1.0.1.md` | 1.0.1 | Stewardship spec | **Absorbed** — Section 8 |
| `stewardship-implementation-plan-v2.md` | 2.0 | Implementation plan | **Absorbed** — Section 8 |
| `stewardship-implementation-plan-phase0-phase1.md` | 1.0 | Phase 0-1 detail | **Superseded** by `stewardship-implementation-plan-v2.md` |
| `agent-semantic-pipeline.md` | 1.0 | Semantic pipeline | **Outdated** — model/tiers changed |
| `GOVERNED_QUERY_VISION_AND_SCOPE_v02.md` | 0.2 | GovernedQuery design | **Absorbed** — Section 10 |

### Contradiction Resolutions

| Topic | Earlier Docs | Resolution |
|-------|-------------|------------|
| Object type count | v1.1 says 6 types | Resolved: 13 types (implementation reality) |
| Governance tiers | v2.1 says 3 tiers | Resolved: 2 tiers (Governed, Operational) |
| Trust classes | Some docs omit | Resolved: 3 classes (Proof, DecisionSupport, Convenience) |
| ChangeSet states | v0.4 says 6 states | Resolved: 9 states (added UnderReview, Approved from stewardship) |
| Verb count | v1.0_1 says 74 | Resolved: ~32 core MCP tools + 23 stewardship + 7 governance verbs |

---

## Appendix B: Glossary

| Term | Definition |
|------|-----------|
| **ABAC** | Attribute-Based Access Control — access decisions based on actor attributes, not just roles |
| **AgentMode** | Research or Governed — determines which verbs are available to the agent |
| **ChangeSet** | Content-addressed bundle of artifacts proposed for publication |
| **CCIR** | Context-Constrained Intent Resolution — SemReg-filtered verb search with fingerprinting |
| **ContextEnvelope** | Carries allowed/pruned verbs, fingerprint, and governance signals from SemReg to intent pipeline |
| **CoreService** | Central trait defining all Semantic OS operations |
| **FQN** | Fully Qualified Name — unique identifier for registry objects (e.g., `cbu.jurisdiction_code`) |
| **GovernedQuery** | Compile-time proc macro enforcing governance checks against bincode cache |
| **Governance Tier** | Governed (full pipeline) or Operational (lightweight) — determines workflow rigor |
| **Guardrail** | Stewardship validation rule (G01-G15) with Block/Warning/Advisory severity |
| **Outbox** | Event queue for reliable projection updates (INSERT + enqueue in single transaction) |
| **Port Trait** | Storage abstraction interface (8 traits) — core depends on traits only, never on DB libraries |
| **Principal** | Explicit actor identity passed to every method — no implicit context |
| **Proof Rule** | Invariant: `TrustClass::Proof` requires `GovernanceTier::Governed` |
| **PruneReason** | Structured reason for verb exclusion (7 variants: ABAC, EntityKind, Tier, Taxonomy, Precondition, AgentMode, Policy) |
| **Seed Bundle** | Deterministic set of initial registry entries generated from verb YAML |
| **Show Loop** | Iterative refinement cycle: Focus → Read → Propose → Show → Refine |
| **Snapshot** | Immutable point-in-time record of a registry object |
| **Snapshot Manifest** | Map of object_id → snapshot_id pinning exact versions used in a decision |
| **Snapshot Set** | Named grouping of snapshots for atomic publish |
| **TOCTOU** | Time-of-Check to Time-of-Use — recheck fingerprint before execution to detect drift |
| **Trust Class** | Proof, DecisionSupport, or Convenience — indicates reliability level |
| **Viewport** | Stewardship UI panel (Focus Summary, Inspector, Diff, Gates) |
