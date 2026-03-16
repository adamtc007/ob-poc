# P4 Architecture Synthesis

> **Date:** 2026-03-16
> **Scope:** Cross-pillar synthesis of 13 architecture review deliverables
> **Sources:** P1-A through P1-D (SemOS), P2-A through P2-D (DSL Runtime), P3-A through P3-E (DB Schema)

---

## 1. Cross-Pillar Coherence

The three pillars — SemOS Registry, DSL Runtime, and DB Schema — form a governance pipeline where each layer depends on the fidelity of the one below it. The central finding of this synthesis is that **the pipeline is structurally complete but operationally hollow**: every integration point exists in code, but the data flowing through it is either absent, defaulted, or unconstrained.

### 1.1 Governance Data Flow

```
DB Schema (tables, constraints, FKs)
    ↓ scanner bootstrap
SemOS Registry (snapshots, verb contracts, taxonomies)
    ↓ ContextEnvelope
DSL Runtime (verb search, surface filter, execution)
    ↓ ChatResponse
React UI (verb browser, governance badges)
```

**Where the chain breaks:**

| Integration Point | Upstream | Downstream | Status |
|---|---|---|---|
| Scanner → Registry | Verb YAML (1,081 verbs) | `sem_reg.snapshots` (4,875 rows) | `DEGRADED` — all snapshots `Operational/Convenience`, zero `Governed/Proof`. Scanner uses `SnapshotMeta::new_operational()` exclusively. |
| Registry → ContextEnvelope | Active snapshots | `ContextEnvelope.allowed_verbs` | `DEGRADED` — verb contracts have empty `preconditions`, `postconditions`, `writes_to`, `reads_from`. Filter is permissive. |
| ContextEnvelope → SessionVerbSurface | Envelope + ViewDefs + Taxonomies | 8-step governance filter | `DEGRADED` — Steps 4-6 (SemReg CCIR, Lifecycle, Actor) are effectively no-ops because ViewDefs=0, TaxonomyDefs=0, entity_state=None. |
| Surface → Verb Search | Allowed verb set | HybridVerbSearcher pre-constraint | `FUNCTIONAL` — pre-constraint wiring works, but passes ~all verbs through because upstream filters are empty. |
| Verb Search → Execution | Selected verb FQN | DslExecutor dispatch | `DEGRADED` — GraphQuery behavior dead (9 verbs unexecutable, ref P2-B). Sequential if-let dispatch misses new behaviors. |
| Execution → DB | SQL statements | Schema constraints | `MIXED` — newer tables (booking principal, service availability) have strong constraints; older tables (deals, legal contracts) have systematic gaps. |

### 1.2 SemTaxonomy v2 vs Legacy Pipeline

Two parallel utterance→verb paths exist (ref P2-D):

| Path | Coverage | Production Status |
|---|---|---|
| HybridVerbSearcher (10-tier) | 1,081 verbs | Primary. Full production path. |
| SemTaxonomy v2 (3-step) | 3 verbs (CBU slice) | Narrow override. 0.24% of registry. |

The SemTaxonomy v2 pipeline (EntityScope → EntityState → SelectedVerb) is architecturally superior — it integrates entity state into verb selection — but covers only `cbu.create`, `cbu.read`, `cbu.list`. The reducer YAML driving Step 2 has **no persistence tables** (ref P1-D), meaning state computations are ephemeral.

### 1.3 Schema ↔ Runtime Alignment

The DB schema (344 tables, 526 FK edges) and the runtime verb surface (1,081 verbs) are loosely coupled through `domain_metadata.yaml` and the AffinityGraph. However:

- **9 phantom ObjectType variants** exist in the enum (`TaxonomyDef`, `ViewDef`, `MembershipRule`, `PolicyRule`, `EvidenceRequirement`, `DocumentTypeDef`, `ObservationDef`, `DerivationSpec`, `TaxonomyNode`) with zero active snapshots (ref P1-A). These types are required by `resolve_context()` Steps 3-5 but return empty results.
- **49 orphan tables** (ref P3-E) have no FK connections. Most are intentional (ML pipeline, BPMN bridge, telemetry), but some indicate incomplete integration.
- **Entity FSM enforcement** (ref P1-B) has three inconsistent tiers: hardcoded transition maps, YAML-driven lifecycle, and bare DB CHECK constraints. Only 3 entities have full FSM enforcement.

---

## 2. Systemic Patterns

### 2.1 The Operational-Only Registry

The single most pervasive pattern across all three pillars: **everything is Operational tier, nothing is Governed**.

| Evidence | Source |
|---|---|
| 4,875 snapshots, 0 governed | P1-A |
| `phase_tags` 99% in YAML, 0% in SemReg | P1-C |
| `harm_class` populated on 2.8% of verbs | P1-C |
| SessionVerbSurface Steps 4-6 disabled | P1-C, P2-D |
| No ViewDefs, no TaxonomyDefs, no PolicyRules active | P1-A |
| Changeset immutability enforced in app only, not DB | P3-C |

**Impact:** The entire governance pipeline — ABAC, proof rules, evidence freshness, taxonomy filtering, view-based verb prominence — is structurally present but functionally inert. Every verb is allowed, every attribute is visible, every action is unaudited.

### 2.2 Nullable Temporals

Across all schema sessions, temporal columns (`created_at`, `updated_at`, `effective_from`, `effective_to`) are frequently `DEFAULT NOW()` but nullable or missing `NOT NULL`:

| Table Family | Pattern | Source |
|---|---|---|
| Deal/billing (14 tables) | Systematic `updated_at` gaps, no triggers | P3-B |
| Legal contracts (045) | Pre-dates uuidv7(), `gen_random_uuid()` | P3-B |
| Core entities | `cbu_structure_links` is gold standard (NOT NULL); others inconsistent | P3-A |
| Workflow/agent | `intent_events` has NOT NULL; `learning_candidates` does not | P3-C |

**Impact:** Temporal queries for audit, freshness, and point-in-time resolution are unreliable on older tables.

### 2.3 Enum Strategy Fragmentation

Three competing enum strategies coexist without a clear boundary:

| Strategy | Example | Count | Source |
|---|---|---|---|
| PostgreSQL `CREATE TYPE ... AS ENUM` | `change_set_status` | ~8 types | P3-C, P3-D |
| `TEXT CHECK (col IN (...))` | `snapshot_status` | ~15 tables | P3-A, P3-D |
| Unconstrained `TEXT` / `VARCHAR` | `cbu_ssi.status` | ~10 tables | P3-B |

**Impact:** Schema-level enforcement is inconsistent. Adding a new enum value requires different migration strategies depending on the table.

### 2.4 Append-Only Without Schema Enforcement

Five critical tables are designed as append-only but rely on application code, not DB triggers, for immutability:

| Table | Enforcement | Source |
|---|---|---|
| `sem_reg.snapshots` | Trigger added in migration 090 | P3-D |
| `sem_reg.decision_records` | App code only | P3-C |
| `governance_audit_log` | App code only | P3-C |
| `sem_reg.derivation_edges` | App code only | P3-C |
| `changeset_entries` (after publish) | App code only | P3-C |

### 2.5 Dead Code Accumulation

| Dead Path | What | Source |
|---|---|---|
| GraphQuery dispatch | 9 verbs with `behavior: graph_query` cannot execute | P2-B |
| `ontology/lifecycle.rs` | Entire module `#[allow(dead_code)]` | P1-B |
| ~60 orphaned CustomOps | Registered via inventory but no matching YAML verb | P2-B |
| Duplicate `intent_events` | Table exists in both `agent` schema and `ob-poc` schema | P3-C |
| Stale embedding model default | `'all-MiniLM-L6-v2'` in DB default vs `'bge-small-en-v1.5'` in runtime | P3-C |
| Duplicate IVFFlat indexes | `verb_pattern_embeddings` has lists=10 and lists=100 indexes | P3-D |

---

## 3. Architectural Risks (Top 5)

### RISK-1: Governance Pipeline is a No-Op

| Attribute | Value |
|---|---|
| **Severity** | `CRITICAL` |
| **Pillars** | P1 (SemOS) + P2 (Runtime) |
| **Sources** | P1-A, P1-C, P1-D, P2-D |
| **Description** | The SessionVerbSurface 8-step pipeline, ContextEnvelope CCIR, ABAC evaluation, proof rules, taxonomy filtering, and evidence freshness checks all exist in code but produce permissive results because the registry contains zero governed objects, zero ViewDefs, zero TaxonomyDefs, and zero PolicyRules. The system cannot distinguish between "explicitly allowed" and "not yet governed." |
| **Blast Radius** | Every user session. Every verb invocation. Every audit claim. |
| **Remediation** | Promote a pilot domain (e.g., KYC) from Operational to Governed tier end-to-end: scanner creates governed snapshots → ViewDefs populated → TaxonomyDefs seeded → PolicyRules active → SessionVerbSurface Steps 4-6 produce real filtering. |

### RISK-2: Conflicting FK Policies on Critical Tables

| Attribute | Value |
|---|---|
| **Severity** | `CRITICAL` |
| **Pillars** | P3 (Schema) |
| **Sources** | P3-A (CR-1, CR-2) |
| **Description** | `ubo_registry.cbu_id` has conflicting delete rules: `SET NULL` from one FK and `CASCADE` from another. This means deleting a CBU produces non-deterministic behavior depending on execution order. Separately, `kyc_ubo_evidence` references the stale `kyc_ubo_registry` table instead of the live `ubo_registry` — evidence lookups return empty results or fail. |
| **Blast Radius** | KYC/UBO pipeline integrity. Any CBU deletion touching UBO data. |
| **Remediation** | Unify CASCADE policy on `ubo_registry.cbu_id` (recommend SET NULL for audit preservation). Retarget `kyc_ubo_evidence` FK to `ubo_registry`. |

### RISK-3: CBU/Entity CASCADE Blast Radius

| Attribute | Value |
|---|---|
| **Severity** | `HIGH` |
| **Pillars** | P3 (Schema) |
| **Sources** | P3-E (F-09, F-10) |
| **Description** | Deleting a row from `cbus` cascades through 43 tables (35 direct children + 8 grandchildren). Deleting from `entities` cascades through 34 tables. There are no soft-delete patterns or archive-before-delete triggers. A single `DELETE FROM cbus WHERE cbu_id = ...` is irreversible and destroys data across the entire entity graph. |
| **Blast Radius** | All downstream tables for the affected CBU/entity. |
| **Remediation** | Add `deleted_at` soft-delete column + application-level "archive then mark" pattern. Consider converting direct CASCADE to NO ACTION + explicit cleanup in application code. |

### RISK-4: KYC Case Dual-FSM Conflict

| Attribute | Value |
|---|---|
| **Severity** | `HIGH` |
| **Pillars** | P1 (SemOS) + P2 (Runtime) |
| **Sources** | P1-B |
| **Description** | KYC Case has two incompatible state machines: a YAML-driven 7-state lifecycle and a hardcoded 11-state transition map in `kyc_case_ops.rs`. The `EXPIRED` state has no inbound transition in either FSM — cases can never reach it. The YAML lifecycle permits transitions that the hardcoded map rejects, and vice versa. |
| **Blast Radius** | All KYC case workflows. State corruption on edge transitions. |
| **Remediation** | Designate one FSM as authoritative (recommend hardcoded map, since it's enforced at runtime). Align YAML lifecycle to match. Add `EXPIRED` inbound transition (likely timer-driven from `IN_REVIEW` or `REMEDIATION`). |

### RISK-5: Archive Table Type Mismatch

| Attribute | Value |
|---|---|
| **Severity** | `HIGH` |
| **Pillars** | P3 (Schema) |
| **Sources** | P3-D (C-1) |
| **Description** | `change_sets_archive.owner_id` is `UUID` but the source `sem_reg.changesets.owner_actor_id` is `TEXT`. The archival process (`INSERT INTO archive SELECT ... FROM changesets`) will fail with a type cast error, meaning no changeset can ever be archived. The retention/cleanup pipeline is broken at the schema level. |
| **Blast Radius** | Changeset archival. Long-term storage growth in `sem_reg.changesets`. |
| **Remediation** | ALTER `change_sets_archive.owner_id` to `TEXT` (or add explicit `::text` cast in archive query). |

---

## 4. Remediation Backlog

### P0 — Fix Before Next Release

| ID | Title | Pillar | Source | Effort |
|---|---|---|---|---|
| P0-01 | Unify CASCADE policy on `ubo_registry.cbu_id` | P3 | P3-A CR-1 | 1 migration |
| P0-02 | Retarget `kyc_ubo_evidence` FK to `ubo_registry` | P3 | P3-A CR-2 | 1 migration |
| P0-03 | Fix `change_sets_archive.owner_id` UUID→TEXT mismatch | P3 | P3-D C-1 | 1 migration |
| P0-04 | Resolve duplicate `intent_events` tables (agent vs ob-poc) | P3 | P3-C CF-1 | 1 migration + app code |
| P0-05 | Reconcile KYC Case dual FSM (YAML vs hardcoded) | P1+P2 | P1-B | 2-3 days |
| P0-06 | Fix empty-string parse failure in DSL parser | P2 | P2-A F-01 | 1 hour |
| P0-07 | Fix boolean/null prefix matching (no word boundary) | P2 | P2-A F-02 | 2 hours |
| P0-08 | Remove duplicate FK constraint pairs (3 tables, 6 pairs) | P3 | P3-A CR-3 | 1 migration |

### P1 — Fix Within 2 Sprints

| ID | Title | Pillar | Source | Effort |
|---|---|---|---|---|
| P1-01 | Restore GraphQuery dispatch path (9 dead verbs) | P2 | P2-B | 1 day |
| P1-02 | Add `EXPIRED` inbound transition to KYC Case FSM | P1 | P1-B | 1 day |
| P1-03 | Add immutability triggers on 4 append-only tables | P3 | P3-C CT-3 | 1 migration |
| P1-04 | Add missing `status` CHECK on `cbu_ssi` | P3 | P3-B | 1 migration |
| P1-05 | Add `updated_at` trigger on deal/billing tables | P3 | P3-B | 1 migration |
| P1-06 | Drop duplicate IVFFlat index on `verb_pattern_embeddings` | P3 | P3-D F-8 | 1 migration |
| P1-07 | Fix HashMap in `EnvelopeCore.snapshot_manifest` (breaks content-addressing) | P2 | P2-C F-1 | 2 hours |
| P1-08 | Add TTL to `PendingMutation` | P2 | P2-D FM-2 | 2 hours |
| P1-09 | Add GLEIF table FKs to `entities` | P3 | P3-D F-9/F-10 | 1 migration |
| P1-10 | Soft-delete pattern for `cbus` table (mitigate 43-table CASCADE) | P3 | P3-E F-09 | 2-3 days |
| P1-11 | Clean up ~60 orphaned CustomOps | P2 | P2-B | 1 day |
| P1-12 | Update stale embedding model default in DB column | P3 | P3-C | 1 migration |

### P2 — Fix Within Quarter

| ID | Title | Pillar | Source | Effort |
|---|---|---|---|---|
| P2-01 | Populate governed-tier snapshots for pilot domain (KYC) | P1 | P1-A | 1-2 weeks |
| P2-02 | Seed ViewDefs and TaxonomyDefs to activate SessionVerbSurface Steps 4-6 | P1 | P1-A, P1-C | 1 week |
| P2-03 | Propagate `phase_tags` from YAML through scanner to SemReg snapshots | P1 | P1-C | 2-3 days |
| P2-04 | Populate verb contract metadata (`harm_class`, `action_class`, `produces`) | P1 | P1-C | 1 week |
| P2-05 | Add reducer state persistence tables | P1 | P1-D | 3-5 days |
| P2-06 | Expand SemTaxonomy v2 beyond CBU slice | P1+P2 | P1-D, P2-D | 2-3 weeks |
| P2-07 | Normalize enum strategy (choose TEXT CHECK as standard) | P3 | P3-C CT-2, P3-D F-12 | 2-3 migrations |
| P2-08 | Add cryptographic ConfirmToken to mutation confirmation path | P2 | P2-D CB-1 | 3-5 days |
| P2-09 | Improve DSL parser error locality (multiple cut points) | P2 | P2-A F-03 | 1 week |
| P2-10 | Replace sequential if-let dispatch with exhaustive match in DslExecutor | P2 | P2-B | 2-3 days |
| P2-11 | Delete dead `ontology/lifecycle.rs` module | P1 | P1-B | 1 hour |
| P2-12 | Add FKs on `sem_reg_pub` published tables back to source snapshots | P3 | P3-C CF-5 | 1 migration |

---

## 5. SemOS Completeness Assessment

### 5.1 Component Maturity Matrix

| Component | Code Exists | Tests Pass | Data Populated | Production Active | Maturity |
|---|:---:|:---:|:---:|:---:|---|
| Snapshot store (INSERT-only) | Yes | Yes | 4,875 rows | Yes | `COMPLETE` |
| Scanner (YAML → snapshots) | Yes | Yes | All verbs scanned | Yes | `COMPLETE` (but Operational-only) |
| ABAC evaluation | Yes | Yes | No governed objects to gate | No (permissive) | `STRUCTURAL` |
| Security label inheritance | Yes | Yes | Labels present but unchecked | No | `STRUCTURAL` |
| Publish gates (proof rule, etc.) | Yes | Yes | Nothing to gate (all Operational) | No | `STRUCTURAL` |
| Context resolution (12-step) | Yes | Yes | Steps 3-5 return empty | Partial | `DEGRADED` |
| Agent control plane (plans/decisions) | Yes | Yes | No production plans created | No | `STRUCTURAL` |
| Projections (lineage/embeddings) | Yes | Yes | Lineage edges empty | No | `STRUCTURAL` |
| Stewardship (changeset workflow) | Yes | Yes | Changesets used for authoring | Partial | `FUNCTIONAL` |
| Governed authoring (7 verbs) | Yes | Yes | Pipeline tested end-to-end | Yes | `COMPLETE` |
| Standalone server (REST+JWT) | Yes | Yes | 10 HTTP integration tests | Deployable | `COMPLETE` |
| db_introspect MCP tool | Yes | Yes | Schema queries work | Yes | `COMPLETE` |
| AttributeSource resolution | Yes | Yes | Real triples resolved | Yes | `COMPLETE` |

### 5.2 Object Type Population

| Object Type | Enum Exists | Body Struct | Active Snapshots | Status |
|---|:---:|:---:|---:|---|
| `attribute_def` | Yes | `AttributeDefBody` | ~2,400 | `POPULATED` |
| `entity_type_def` | Yes | `EntityTypeDefBody` | ~120 | `POPULATED` |
| `verb_contract` | Yes | `VerbContractBody` | ~1,081 | `POPULATED` (metadata hollow) |
| `relationship_type_def` | Yes | JSONB | ~50 | `POPULATED` |
| `taxonomy_def` | Yes | `TaxonomyDefBody` | 0 | `PHANTOM` |
| `taxonomy_node` | Yes | `TaxonomyNodeBody` | 0 | `PHANTOM` |
| `membership_rule` | Yes | `MembershipRuleBody` | 0 | `PHANTOM` |
| `view_def` | Yes | `ViewDefBody` | 0 | `PHANTOM` |
| `policy_rule` | Yes | `PolicyRuleBody` | 0 | `PHANTOM` |
| `evidence_requirement` | Yes | `EvidenceRequirementBody` | 0 | `PHANTOM` |
| `document_type_def` | Yes | `DocumentTypeDefBody` | 0 | `PHANTOM` |
| `observation_def` | Yes | `ObservationDefBody` | 0 | `PHANTOM` |
| `derivation_spec` | Yes | `DerivationSpecBody` | 0 | `PHANTOM` |

**7 of 13 object types populated. 6 phantom types block the governance pipeline.**

### 5.3 Governance Tier Distribution

| Tier | Count | Percentage |
|---|---:|---:|
| Governed | 0 | 0.0% |
| Operational | 4,875 | 100.0% |

### 5.4 What "Complete" Means

SemOS has three layers of completeness:

1. **Structural completeness** (`~90%`): Types exist, traits implemented, ports defined, tests pass, migrations applied, REST routes wired, MCP tools registered. The code is architecturally sound.

2. **Data completeness** (`~30%`): Only 4 of 13 object types have meaningful data. Verb contracts exist but with empty metadata fields. Zero governed objects means the proof rule, ABAC, taxonomy filtering, and evidence freshness checks never fire on real data.

3. **Operational completeness** (`~15%`): The stewardship changeset workflow and governed authoring pipeline are functional. Everything else — context resolution, agent planning, lineage tracking, view-based filtering — is structurally present but produces no governance effect because the data layer is hollow.

### 5.5 Critical Path to Operational Governance

The minimum work to make the governance pipeline produce real filtering (not just pass-through):

```
Step 1: Seed TaxonomyDefs for KYC domain (4 canonical taxonomies exist as code seeds)
     → Enables resolve_context() Step 3 (taxonomy-based view selection)

Step 2: Create ViewDefs linking KYC taxonomies to verb surfaces
     → Enables resolve_context() Step 4 (verb surface extraction)
     → Enables SessionVerbSurface Step 4 (SemReg CCIR produces real filtering)

Step 3: Promote KYC verb contracts to Governed tier
     → Enables proof rule enforcement
     → Enables ABAC gating on sensitive attributes

Step 4: Propagate phase_tags from YAML → scanner → snapshots
     → Enables goals-based verb filtering in Semantic OS Tab

Step 5: Populate verb contract metadata (harm_class, action_class, produces)
     → Enables utterance pipeline metadata-quality improvements
     → Lifts exact-hit rate above current 7.95% floor
```

Estimated effort: 2-3 weeks for Steps 1-4 on a single pilot domain.

---

## Appendix: Source Document Index

| ID | Title | Pillar | Key Findings |
|---|---|---|---|
| P1-A | Registry Structure | SemOS | 4,875 operational snapshots, 0 governed, 9 phantom types |
| P1-B | Entity FSM Audit | SemOS | Dual KYC FSM conflict, EXPIRED unreachable, 3-tier enforcement |
| P1-C | Verb Surface Profiles | SemOS | 1,081 verbs, hollow metadata, SessionVerbSurface Steps 4-6 disabled |
| P1-D | StateGraph Gating | SemOS | 0.24% CBU slice live, no reducer persistence |
| P2-A | Parser Audit | Runtime | Empty-string failure, boolean prefix matching, single cut point |
| P2-B | Dispatch & Execution | Runtime | GraphQuery dead (9 verbs), ~60 orphan CustomOps, non-transactional plugins |
| P2-C | Macro & Runbook Compilation | Runtime | HashMap breaks content-addressing, SemReg fail-open, write-set heuristic gaps |
| P2-D | Utterance Pipeline | Runtime | 7.95% exact-hit rate, parallel pipelines, no ConfirmToken, no PendingMutation TTL |
| P3-A | Core Entity Schema | Schema | Conflicting CASCADE, stale FK target, 6 duplicate FK pairs |
| P3-B | Service & Trading Schema | Schema | Strong newer tables, systematic NOT NULL gaps in deals/billing |
| P3-C | Workflow & Governance Schema | Schema | Duplicate intent_events, app-only immutability, enum fragmentation |
| P3-D | Metadata & Infrastructure Schema | Schema | Archive type mismatch, duplicate indexes, GLEIF FK gaps |
| P3-E | FK Graph Topology | Schema | 344 tables, 526 edges, 43-table CASCADE from cbus, 49 orphans |
