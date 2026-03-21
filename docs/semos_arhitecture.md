# Semantic OS — Vision & Scope v3.0

> **Version:** 3.0
> **Date:** 2026-03-21
> **Status:** Living document — consolidation of 9 prior specs, updated for the 2026-03-08 runtime schema consolidation, the 2026-03-12 document-governance bootstrap, the 2026-03-13 NLCI/CBU surface reconciliation, the 2026-03-15 reducer/constellation runtime cutover, the 2026-03-16 DB harness/runtime verification pass, the 2026-03-16 CODEX data-integrity/parser/serialization remediation, the 2026-03-17 discovery-universe + single-pipeline cutover, and the 2026-03-21 SemOS reconciliation remediation verification pass
> **Audience:** Engineering, governance, architecture review

---

## 1. Executive Summary

Semantic OS is an **immutable, governance-aware knowledge registry** that serves as the single source of truth for what exists in the system — attributes, entities, verbs, policies, evidence requirements, taxonomies, and their relationships.

It answers three questions at any point in time:

1. **What exists?** — 16 object types stored as immutable snapshots
2. **Who can access it?** — ABAC with security labels, classification, PII tracking
3. **What should the agent do next?** — Context resolution pipeline returning ranked, tier-aware, snapshot-pinned candidates

Semantic OS enforces governance through **three complementary layers**:

| Layer | Enforcement Point | Mechanism |
|-------|-------------------|-----------|
| **Authoring Pipeline** | What enters the registry | ChangeSets, validation stages, stewardship guardrails |
| **Publish Gates** | What becomes active | Proof rule, security labels, version monotonicity |
| **GovernedQuery** | What compiles | Proc macro checks against bincode cache at compile time |

The system is deployed as a **standalone service** (6 Rust crates) with port-trait isolation, REST+JWT API, outbox-driven projections, and optional in-process mode for the ob-poc monolith.

### Current Discovery/Bootstrap State (2026-03-17)

The intended single-pipeline model is now materially reflected in the live agent path:

`utterance -> Sage bootstrap -> Sem OS resolve_context() -> discovery surface or grounded action surface`

Key properties of the current implementation:

- `resolve_context()` remains the single Sem OS entrypoint; discovery is an internal stage, not a separate public resolver
- empty or under-grounded Sage sessions return a discovery surface, not a broad global verb inventory
- Sem OS is fail-closed for Sage-side utterance discovery; if Sem OS is unavailable, the session is closed rather than widened onto a legacy fallback path
- the chat/session API now carries a typed discovery bootstrap payload to the UI
- the chat UI renders the discovery bootstrap surface directly and allows domain/family/constellation selections to flow back into the same session
- those selections are persisted in session state and threaded into Sem OS as structured discovery hints

The discovery surface currently includes:

- `matched_domains`
- `matched_families`
- `matched_constellations`
- `missing_inputs`
- `entry_questions`
- `grounding_readiness`

This is the active bridge between ambiguous onboarding/opening turns and grounded constellation execution.

### Reconciliation Remediation State (2026-03-21)

The SemOS reconciliation findings recorded on 2026-03-19 are now closed at the code level.

The executed remediation includes:

- direct `entity_kind` filtering on the non-SemOS verb-search path
- expanded canonical entity-kind vocabulary and broader `subject_kinds` derivation coverage
- preserved SemOS prune visibility for `EntityKindMismatch`
- actionable discovery bootstrap questions in the chat/session flow
- authored discovery families for trading, billing, deal, and contract
- runtime validation that constellation map slot verbs and supported bulk macros resolve against the active registry
- post-expansion DSL revalidation before runbook planning
- startup audit of fail-closed safe-harbor verbs against `harm_class`
- end-to-end entity-confidence threading as a widening signal
- park-reason exposure through chat payloads and UI

Verification status for the remediation execution:

- `cargo check` passes
- `cargo fmt --check` passes
- `cargo clippy -- -D warnings` passes
- the dedicated `vnext-repl` regression for post-expansion validation passes

Residual full-suite failures are environment-dependent integration issues rather than unresolved remediation code.

Peer-review bundle for the execution set:

- `artifacts/semos-recon-remediation-review-2026-03-21.tar.gz`

### Non-Lossy Utterance Contract (2026-03-17)

The Sem OS request contract no longer overloads one field to mean both “what the user said” and “what Sage inferred.”

`ContextResolutionRequest` now carries:

- `raw_utterance`
- `intent_summary`
- structured `discovery` hints (`selected_domain_id`, `selected_family_id`, `selected_constellation_id`, `known_inputs`)

This matters for branching correctness. A later branch in the single pipeline must still be able to inspect the original utterance, even if an earlier branch already produced a narrower Sage summary. The active discovery scorer now combines:

- raw utterance
- Sage summary
- goals
- structured discovery answers

with explicit structured selections taking precedence over text scoring.

### Discovery Harness State (2026-03-17)

There is now a dedicated Sem OS discovery harness:

- [semos_discovery_hit_rate.rs](/Users/adamtc007/Developer/ob-poc/rust/tests/semos_discovery_hit_rate.rs)
- [sem_os_discovery_utterances.toml](/Users/adamtc007/Developer/ob-poc/rust/tests/fixtures/sem_os_discovery_utterances.toml)

The harness measures:

- utterance -> top-1 domain
- utterance -> top-1 family
- utterance -> top-1 / top-3 constellation
- grounding-readiness accuracy

It also includes a regression case where the raw utterance contains trigger tokens absent from `intent_summary`, specifically to catch lossy-branch behavior.

Current baseline on the small authored discovery corpus:

- domain top-1: 100%
- family top-1: 100%
- constellation top-1: 100%
- constellation top-3: 100%
- readiness exact: 100%

This should be read as “the new path is coherent and measurable,” not “global discovery quality is solved.” The authored universe/family set is still small, so future value comes from growing the corpus and watching this harness instead of the legacy verb-hit benchmark.

### Current Agent Integration State (2026-03-13)

The downstream `ob-poc` utterance-understanding path has been collapsed to a strict three-step pipeline:

1. `EntityScope`
2. `EntityState`
3. `SelectedVerb`

This is implemented in [`rust/src/semtaxonomy_v2/mod.rs`](/Users/adamtc007/Developer/ob-poc/rust/src/semtaxonomy_v2/mod.rs). Semantic OS remains the constraint and metadata layer, while `ob-poc` composes business proposals from:

- live grounded entity candidates and business state
- SemOS/registry-derived valid transitions and verb contracts

The key integration invariant is now:

- Step 1 is a hard gate.
- If entity scope is ambiguous, the pipeline stops and returns clarification.
- Ambiguity is not allowed to cascade into state derivation or verb selection.

Current live harness state for the new path:

- exact top-verb accuracy: `14/176` (`7.95%`)
- grounded responses: `125/176` (`71.02%`)
- stateful responses: `125/176` (`71.02%`)
- business proposals: `84/176`

These numbers show that the simplified pipeline boundary is in place and that the biggest remaining issue is no longer call-stack complexity. The current bottleneck is metadata quality across the StateGraph triangle:

- live entity/state signals
- canonical verb contracts and invocation phrases
- graph/corpus alignment

The safe Phase 1 cleanup in-repo is complete:
- `entity-context` signal enrichment
- canonical invocation phrase enrichment
- repo-derived reconciliation pack

The unsafe reconciliation work is intentionally parked pending the authoritative external correction table for unresolved verb families such as `struct.*`, `screening.full`, and any external graph-edge correction map.

### Current CBU/NLCI Surface State (2026-03-13)

The downstream `ob-poc` CBU surface is now materially cleaner than the March 11 cutover point:

- the narrow compiler-backed CBU path is live for:
  - `cbu.create`
  - `cbu.read`
  - `cbu.list`
  - `cbu.rename`
  - `cbu.set-jurisdiction`
  - `cbu.set-client-type`
  - `cbu.set-commercial-client`
  - `cbu.set-category`
  - `cbu.submit-for-validation`
  - `cbu.request-proof-update`
  - `cbu.reopen-validation`
- the old duplicate phrase/discovery/test surfaces for those intents were removed instead of being left in place as compatibility debt
- the CBU role surface was reconciled onto one public namespace:
  - `cbu.assign-role`
  - `cbu.remove-role`
  - `cbu.parties`
  - specialist role verbs remain under `cbu.*` as:
    - `cbu.assign-ownership`
    - `cbu.assign-control`
    - `cbu.assign-trust-role`
    - `cbu.assign-fund-role`
    - `cbu.assign-service-provider`
    - `cbu.assign-signatory`
    - `cbu.validate-roles`

The practical SemOS implication is:

- the active resolver/discovery layer is now closer to a single canonical contract
- utterance routing quality is less constrained by duplicate DSL names
- the remaining metadata bottleneck is quality and coverage, not multiple competing public verb families for CBU

### Current Runtime State (2026-03-08)

- Business runtime data is consolidated into `"ob-poc"`.
- Semantic OS runtime data is consolidated into `sem_reg`, `sem_reg_authoring`, and `sem_reg_pub`.
- The legacy runtime schemas `stewardship`, `agent`, `teams`, `feedback`, `events`, `sessions`, `ob_ref`, and `ob_kyc` have been retired.
- `rust/config/sem_os_seeds/domain_metadata.yaml` now covers `306/306` live `"ob-poc"` tables and includes remediated SemOS footprint metadata for `sem-reg` and stewardship-backed verbs.

### Reducer + Constellation Runtime State (2026-03-15)

The SemOS-governed CBU/KYC structure layer is now backed by a live reducer and constellation runtime in `ob-poc`:

- reducer state machines are seeded under:
  - `rust/config/sem_os_seeds/state_machines/`
- constellation maps are seeded under:
  - `rust/config/sem_os_seeds/constellation_maps/`
- reducer execution lives under:
  - `rust/src/sem_reg/reducer/`
- constellation loading, hydration, normalization, action-surface, and summary live under:
  - `rust/src/sem_reg/constellation/`

Operationally, this means:

- CBU/slot lifecycle state is now derived from formal reducer YAML rather than ad hoc UI logic.
- Constellation hydration is server-side and REST-addressable:
  - `GET /api/cbu/:cbu_id/constellation`
  - `GET /api/cbu/:cbu_id/constellation/summary`
  - `GET /api/cbu/:cbu_id/cases`
  - `GET /api/constellation/by-name`
  - `GET /api/constellation/search-cbus`
- the chat UI consumes that server-side constellation payload directly, including:
  - reducer-derived `computed_state` / `effective_state`
  - available and blocked verbs
  - override-aware slot status
  - ownership-chain graph node/edge payloads for `entity_graph` slots

This is now the intended boundary:

- SemOS governs the contracts, metadata, and footprints.
- reducer/constellation runtime turns those governed contracts into session-visible operational state.
- the React chat UI is a projection/rendering layer only; it does not recompute reducer state client-side.

### Cross-Border Structure Runtime State (2026-03-15)

The cross-border constellation gap for M17/M18 is now closed without changing the reducer or constellation architecture:

- persisted CBU-to-CBU structure facts now live in:
  - `"ob-poc".cbu_structure_links`
- the active primitive verb surface now includes:
  - `cbu.link-structure`
  - `cbu.list-structure-links`
- the cross-border constellation maps now hydrate child `cbu` slots from persisted links rather than leaving them as permanently structural placeholders
- hydration direction is explicitly downward-only:
  - parent/root CBU -> child linked CBUs
- slot-level disambiguation is data-backed through selectors such as:
  - `feeder:us`
  - `feeder:ie`
  - `parallel:us`
  - `aggregator`

This preserves the intended layering:

- SemOS governs the map and verb contracts
- the runtime persists structure relationships as first-class operational facts
- constellation hydration projects those facts into the server-side graph returned to the UI

No new reducer semantics were introduced for child CBUs in this pass. Child `cbu` slots remain structural slots that surface `filled` when a persisted link exists and `empty` when it does not.

### Runtime Verification State (2026-03-16)

The runtime cutover above is now verified by the live `ob-poc` harness and code-quality gates rather than only by focused unit slices:

- `cargo clippy -p ob-poc -- -D warnings` is green
- `cargo test -p ob-poc --test db_integration` is green
- `cargo test -p ob-poc -- constellation` is green

The concrete fixes required to get there are part of the durable runtime now:

- the DB integration harness explicitly bootstraps verb config lookup in test context, so baseline verbs such as `cbu.create` resolve under `runtime_registry()`
- local harness runs tolerate lagging `cbu_structure_links` schema by provisioning the cross-border structure table and supporting indexes when absent
- reducer evidence overlays now match the live UBO schema:
  - `kyc_ubo_evidence.ubo_id`
  - `ubo_registry.ubo_id`
  - `ubo_registry.subject_entity_id`
- generic `entity.create` / `entity.ensure` now project the canonical `name` field into required extension-table columns such as:
  - `entity_limited_companies.company_name`
  - `entity_partnerships.partnership_name`
  - `entity_trusts.trust_name`

This matters at the architecture boundary because SemOS-governed metadata is only useful if the live runtime, test harness, and schema actually agree on the operational contract.

### Governance Bootstrap + Runtime Remediation State (2026-03-16)

The March 16 remediation closed the gap between the governed Semantic OS object model and the live `ob-poc` runtime/bootstrap path without rerouting initial seeding through the authoring changeset workflow.

Transport-side outcome:

- scanner/bootstrap remains the initial seeding path for governed KYC objects
- taxonomy, taxonomy-node, view, membership-rule, and KYC verb snapshots can now publish at governed tier directly from the existing seed/scanner flow
- `phase_tags` propagation is verified through republish/backfill rather than treated as a missing-schema problem

Model/runtime outcome:

- governed verb contracts now carry:
  - `harm_class`
  - `action_class`
  - `precondition_states`
- ontology-backed lifecycle enforcement is live in runtime mutation paths for normalized entities such as `kyc-case` and `entity-workstream`
- reducer-state persistence is now exercised against the live DB through `sem_reg.reducer_states`

Operational integrity outcome:

- critical Tier 0 data-integrity defects were corrected in the live code/migration path:
  - conflicting `ubo_registry` FK cascade
  - stale `kyc_ubo_evidence` FK target
  - duplicate FK constraint pairs
  - archive table type mismatch
  - duplicate `intent_events` table split
  - DSL parser empty-string / boolean-word-boundary defects
- soft-delete is now the active runtime pattern for `cbus` and `entities`
- high-traffic read paths across session loading, visualization, graph traversal, KYC/workstream views, control/ownership analysis, and related repositories now exclude rows where `deleted_at IS NOT NULL`

Verification state for this remediation slice:

- `cargo check` green against the live DB-backed workspace
- governed bootstrap scan completes through the existing scanner path
- DB regression coverage exists for:
  - soft delete
  - reducer persistence
  - generic lifecycle guard enforcement

This leaves Semantic OS in a more coherent architectural position:

- initial bootstrap uses the fast seeded scanner path
- ongoing governed authoring still belongs to the changeset/stewardship pipeline
- runtime query surfaces now better respect the same lifecycle/governance invariants as the registry layer

### Constellation Macro/SemOS Remediation (2026-03-13)

The March 13 Constellation remediation pack closed a specific metadata-integrity gap between the macro layer and the SemOS overlay:

- broken macro verb references in the CBU Constellation structure, mandate, party, and case macros were reconciled onto live canonical verbs
- parked gaps with no registered runtime verb remain explicitly marked in macro YAML (`cbu.link-structure`, bulk `trading-profile.set-*`, `address.set`)
- orphan `verb_data_footprint` entries were removed from `rust/config/sem_os_seeds/domain_metadata.yaml`
- Tier A SemOS footprint coverage was added for macro-referenced verbs that previously had no overlay entries
- `client-group` replacement footprints were remapped onto the active verb surface

Validation outcome for the pack:

- Registry verbs: `1064`
- Broken macro refs: `0`
- Orphan footprints: `0`
- Total SemOS footprints: `252`
- Tier A missing: `0`

### KYC/UBO Constellation Enablement and Blocker Fixes (2026-03-13)

The March 13 KYC/UBO follow-on work extended the same macro/footprint discipline into the ownership and case-review surface:

- 29 new macro flows were added across `kyc-workstream`, `ubo`, `evidence`, `doc-request`, `screening-ops`, `red-flag`, and `tollgate`
- `case.request-info` was backfilled onto the live case macro surface
- missing primitive verb contracts and SemOS footprints were added only where the live registry still had gaps
- the evidence lifecycle was corrected to avoid macro-to-primitive name collisions by renaming the primitive verbs to action-encoded names:
  - `evidence.create-requirement`
  - `evidence.attach-document`
  - `evidence.mark-verified`
  - `evidence.mark-rejected`
  - `evidence.mark-waived`
- `ubo.discover` now passes through `threshold` and `max-depth` to `ownership.compute`
- generic `OTHER` enum fallbacks were restored for document-request and risk-flag catch-all values

Validation outcome after enablement plus blocker fixes:

- Registry verbs: `1068`
- Broken macro refs: `0`
- Orphan footprints: `0`
- Total SemOS footprints: `281`
- Tier A missing: `0`

### Document Governance Bootstrap (2026-03-12)

Semantic OS now governs the first document-policy control-plane objects needed for document polymorphism:

- `RequirementProfileDef`
- `ProofObligationDef`
- `EvidenceStrategyDef`

These objects now follow the same immutable snapshot and publication path as the rest of SemOS:

- authoring and publish still land in `sem_reg.snapshots`
- active published copies project into `sem_reg_pub.active_requirement_profiles`
- active published copies project into `sem_reg_pub.active_proof_obligations`
- active published copies project into `sem_reg_pub.active_evidence_strategies`

The `ob-poc` runtime no longer has to read mutable authoring state directly to answer document-requirement questions. It now consumes the published SemOS snapshot set for KYC/entity document flows, computes governed requirement matrices through `document.compute-requirements`, and can persist immutable audit bindings through `"ob-poc".policy_version_bindings`.

### Data Management Contract (2026-03-06)

Semantic OS Data Management is a **structure semantics** surface, not a record-content retrieval surface.

- In `semos-data-management` and `semos-data`, noun-only exploratory prompts default to structure-first verbs.
- The default structure target is the entity/domain model: fields, relationships, and available verbs.
- ID-bound content verbs such as `*.get` are not the default in this mode unless the utterance explicitly instance-targets with `*-id`, `id:`, `for id`, or `@handle` syntax.
- The standard structure verbs are now:
  - `schema.domain.describe`
  - `schema.entity.describe`
  - `schema.entity.list-fields`
  - `schema.entity.list-relationships`
  - `schema.entity.list-verbs`

### Unified Input Trace

The SemOS data-management call path remains on the unified session ingress only:

1. UI submits `POST /api/session/:id/input` with `kind=utterance`.
2. `rust/src/api/agent_routes.rs` adapts that request into the chat/session service path.
3. `rust/src/api/agent_service.rs` builds `OrchestratorContext` and calls `handle_utterance()`.
4. `rust/src/agent/orchestrator.rs` applies the structure-first rewrite and data-management candidate policy.
5. The selected `schema.entity.*` verb executes through the standard DSL/custom-op path.

### Chat State Contract (2026-03-09)

The chat path is now explicitly asymmetric:

- If there is doubt on user intent, the system biases toward `read` / `show` / `list`.
- State-changing intents do not execute immediately. They produce a plain-language pending mutation.
- A read utterance while a mutation is pending cancels that mutation and says so in chat.
- A confirmation token such as `yes` only executes when a mutation is still pending.
- A stale confirmation after cancellation is treated as a safe no-op and returns:
  - `There is no pending change to confirm. I am still in read-only mode.`

Verified smoke behavior on the live `ob-poc-web` path:

- `allianz` -> `Now working with client: Allianz Global Investors.`
- `what deals does Allianz have?` -> safe read execution, no `deal.search` vs `deal.list` disambiguation
- `create a new CBU for Allianz UK Fund` -> plain-language pending mutation for `cbu.create`
- `show me the cbus instead` -> cancels pending mutation and returns to read-only execution
- `yes` after that cancellation -> explicit no-op confirmation response

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
│  PostgreSQL (sem_reg │ sem_reg_pub │ sem_reg_authoring │ "ob-poc")   │
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
| `sem_reg` | Core: snapshots, snapshot_sets, outbox_events, changesets, agent plans/decisions, stewardship storage |
| `sem_reg_pub` | Read-optimized projections: active_snapshot_set, projection_watermark |
| `sem_reg_authoring` | Authoring: validation_reports, governance_audit_log, publish_batches, artifacts, archives |
| `"ob-poc"` | Business runtime data after schema consolidation |

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
