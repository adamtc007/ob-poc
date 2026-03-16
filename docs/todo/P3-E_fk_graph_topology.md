# P3-E: FK Graph Topology Review

> **Session:** P3-E Schema Cross-Cut (FK Graph Only)
> **Date:** 2026-03-16
> **Scope:** Topological analysis of the FK dependency graph across all ob-poc schemas
> **Method:** Direct SQL queries against `information_schema` on the live `data_designer` database

---

## Executive Summary

The ob-poc database contains **307 tables** in the `ob-poc` schema, **25** in `sem_reg`, **6** in `sem_reg_authoring`, and **4** in `sem_reg_pub` (344 total). The FK graph has **501 edges** connecting **258 of 307** tables in `ob-poc`. The graph is broadly healthy: the expected domain anchors (`entities`, `cbus`, `cases`) are the highest fan-in hubs, no multi-hop cycles exist, and the longest CASCADE chain is 4 hops (rooted at `entity_types`). The primary risk areas are (a) the large blast radius of CASCADE deletes on `cbus` (35 direct cascade children) and `entities` (30), and (b) 49 orphan tables in `ob-poc` that have zero FK participation.

**Severity distribution:** 3 CLEAN, 5 MINOR, 3 FLAG, 0 CRITICAL.

---

## 1. Schema Inventory

| Schema | Tables | FK Edges | Notes |
|--------|--------|----------|-------|
| `ob-poc` | 307 | 501 | Main business schema |
| `sem_reg` | 25 | 24 | Semantic Registry (immutable snapshots) |
| `sem_reg_authoring` | 6 | 1 | Governance authoring pipeline |
| `sem_reg_pub` | 4 | 0 | Read-optimized projections (all orphan by design) |
| **Total** | **344** | **526** | |

**ob-poc FK participation:**
- 258 tables participate in at least one FK (84%)
- 104 tables are referenced as parents
- 224 tables have outbound FKs (are children)
- 34 tables are pure roots (referenced by others but have no outbound FKs themselves)
- 49 tables are orphans (zero FK in or out)

**Delete rule distribution (ob-poc):**

| Rule | Count | Pct |
|------|-------|-----|
| NO ACTION | 338 | 67.5% |
| CASCADE | 154 | 30.7% |
| SET NULL | 11 | 2.2% |

**Verdict: CLEAN** -- The 2:1 ratio of NO ACTION to CASCADE is healthy. CASCADE is used appropriately for parent-child ownership relationships. SET NULL is used sparingly for optional references.

---

## 2. Orphan Tables (No FK In or Out)

### 2a. ob-poc schema: 49 orphan tables

**Reference data tables (populated, intentionally standalone):**

| Table | Rows | Assessment |
|-------|------|------------|
| `currencies` | 16 | [CLEAN] Lookup table, referenced by application code not FK |
| `case_types` | 5 | [MINOR] Should arguably FK from `cases.case_type` |
| `screening_types` | 7 | [MINOR] Should arguably FK from `screenings` |
| `settlement_types` | 4 | [MINOR] Should arguably FK from settlement tables |
| `client_types` | -- | [MINOR] Should arguably FK from `clients` |
| `request_types` | -- | [MINOR] Should arguably FK from request tables |
| `risk_ratings` | -- | [MINOR] Should arguably FK from risk assessment tables |
| `role_categories` | -- | [MINOR] Appears unused by any FK |
| `role_types` | -- | [MINOR] Appears unused by any FK |
| `ssi_types` | -- | [MINOR] Should arguably FK from SSI tables |

**Semantic/ML pipeline tables (standalone by design):**

| Table | Rows | Assessment |
|-------|------|------------|
| `dsl_verbs` | 1,365 | [CLEAN] Verb registry, queried by app code |
| `verb_pattern_embeddings` | 21,047 | [CLEAN] Embedding vectors, queried by pgvector |
| `verb_centroids` | -- | [CLEAN] ML artifact |
| `invocation_phrases` | -- | [CLEAN] ML pipeline |
| `user_learned_phrases` | 26 | [CLEAN] Learning pipeline |
| `phrase_blocklist` | -- | [CLEAN] Learning pipeline |
| `intent_events` | 117 | [CLEAN] Telemetry, append-only |
| `intent_feedback_analysis` | -- | [CLEAN] Analytics |
| `semantic_match_cache` | -- | [CLEAN] Cache table |
| `learning_candidates` | -- | [CLEAN] Promotion pipeline |
| `dictionary` | -- | [CLEAN] NLP support |
| `lexicon_tokens` | -- | [CLEAN] NLP support |

**DSL infrastructure tables (standalone by design):**

| Table | Rows | Assessment |
|-------|------|------------|
| `dsl_verb_categories` | -- | [CLEAN] Verb metadata |
| `dsl_verb_sync_log` | -- | [CLEAN] Sync audit |
| `dsl_workflow_phases` | -- | [CLEAN] Workflow metadata |
| `expansion_reports` | 1,176 | [CLEAN] Audit trail, append-only |
| `rule_field_dictionary` | -- | [CLEAN] Eligibility rule metadata |

**BPMN integration tables (standalone by design):**

| Table | Rows | Assessment |
|-------|------|------------|
| `bpmn_correlations` | -- | [CLEAN] BPMN-Lite bridge, keyed by correlation_id |
| `bpmn_job_frames` | -- | [CLEAN] Job dedupe cache |
| `bpmn_parked_tokens` | -- | [CLEAN] Waiting REPL entries |
| `bpmn_pending_dispatches` | -- | [CLEAN] Dispatch queue |

**BODS tables (unused):**

| Table | Rows | Assessment |
|-------|------|------------|
| `bods_entity_types` | 7 | [FLAG] Reference data, populated but no FK consumers |
| `bods_interest_types` | 23 | [FLAG] Reference data, populated but no FK consumers |
| `bods_ownership_statements` | 0 | [FLAG] Empty, likely placeholder from BODS import |
| `bods_person_statements` | 0 | [FLAG] Empty, likely placeholder from BODS import |

**Other orphans:**

| Table | Assessment |
|-------|------------|
| `events` | [MINOR] Generic event log, 0 rows -- possibly vestigial |
| `edge_types` | [MINOR] Graph metadata, no FK consumers |
| `node_types` | [MINOR] Graph metadata, no FK consumers |
| `view_modes` | [MINOR] Deprecated (ViewMode is now a unit struct) |
| `cbu_layout_overrides` | [MINOR] Likely vestigial from removed esper_* crates |
| `layout_cache` | [MINOR] Likely vestigial from removed esper_* crates |
| `layout_config` | [MINOR] Likely vestigial from removed esper_* crates |
| `entity_relationships_history` | [MINOR] History table, append-only |
| `entity_type_dependencies` | [MINOR] Graph metadata |
| `standards_mappings` | [MINOR] Reference data |
| `threshold_factors` | [MINOR] KYC config |
| `workflow_definitions` | [MINOR] Workflow metadata |
| `schema_consolidation_table_map` | [CLEAN] Migration bookkeeping |
| `rate_cards` | [MINOR] 0 rows, possibly superseded by `deal_rate_cards` |

### 2b. sem_reg schemas: 13 orphan tables

| Schema | Table | Assessment |
|--------|-------|------------|
| `sem_reg` | `bootstrap_audit` | [CLEAN] Idempotency tracking, standalone by design |
| `sem_reg` | `classification_levels` | [MINOR] Should arguably FK from snapshots |
| `sem_reg` | `idempotency_keys` | [CLEAN] Dedup cache |
| `sem_reg` | `outbox_events` | [CLEAN] Outbox pattern, standalone by design |
| `sem_reg` | `templates` | [MINOR] Workflow templates, no FK consumers |
| `sem_reg` | `verb_implementation_bindings` | [MINOR] Standalone lookup |
| `sem_reg` | `viewport_manifests` | [CLEAN] Stewardship show loop |
| `sem_reg_authoring` | `governance_audit_log` | [CLEAN] Append-only audit, standalone by design |
| `sem_reg_authoring` | `publish_batches` | [CLEAN] Batch publish records |
| `sem_reg_pub` | `active_entity_types` | [CLEAN] Projection table, populated by outbox |
| `sem_reg_pub` | `active_taxonomies` | [CLEAN] Projection table |
| `sem_reg_pub` | `active_verb_contracts` | [CLEAN] Projection table |
| `sem_reg_pub` | `projection_watermark` | [CLEAN] Outbox watermark |

**Verdict: MINOR** -- Most orphans are intentionally standalone (ML pipeline, BPMN bridge, telemetry, projections). The reference data tables (`case_types`, `screening_types`, `settlement_types`, etc.) represent missing FK constraints that should ideally exist for referential integrity. The BODS tables are unused placeholders. The layout/graph tables are vestigial from removed crates.

---

## 3. Cycle Analysis

### 3a. Self-Referencing FKs (1-hop cycles): 12 tables

All are intentional hierarchy/versioning patterns:

| Table | FK Column | Pattern | Assessment |
|-------|-----------|---------|------------|
| `entity_types` | `parent_type_id -> entity_type_id` | Type hierarchy | [CLEAN] |
| `instrument_classes` | `parent_class_id -> class_id` | Class hierarchy | [CLEAN] |
| `entity_workstreams` | `discovery_source_workstream_id -> workstream_id` | Source tracking | [CLEAN] |
| `crud_operations` | `parent_operation_id -> operation_id` | Op hierarchy | [CLEAN] |
| `share_classes` | `converts_to_share_class_id -> id` | Conversion chain | [CLEAN] |
| `ubo_registry` | `replacement_ubo_id -> ubo_id` | Supersession | [CLEAN] |
| `ubo_registry` | `superseded_by -> ubo_id` | Supersession | [CLEAN] |
| `deal_rate_cards` | `superseded_by -> rate_card_id` | Versioning | [CLEAN] |
| `attribute_observations` | `superseded_by -> observation_id` | Versioning | [CLEAN] |
| `graph_import_runs` | `superseded_by -> run_id` | Versioning | [CLEAN] |
| `ownership_snapshots` | `superseded_by -> snapshot_id` | Versioning | [CLEAN] |
| `client_group_relationship_sources` | `verifies_source_id -> id` | Cross-verification | [CLEAN] |

Also in `sem_reg`:
- `sem_reg.changesets` self-references (supersession chain)
- `sem_reg.snapshots` self-references (supersession chain)

**Verdict: CLEAN** -- All self-references follow well-known patterns (hierarchy, supersession, versioning). No DELETE CASCADE on any self-reference.

### 3b. Mutual FK References (2-hop cycles): 2 pairs

**Pair 1: `cbu_resource_instances` <-> `provisioning_requests`**
- `provisioning_requests.instance_id -> cbu_resource_instances.instance_id` (NO ACTION)
- `cbu_resource_instances.last_request_id -> provisioning_requests.request_id` (NO ACTION)
- **Pattern:** Resource tracks its latest provisioning request; request references the resource. This is a standard "latest pointer" pattern.
- **Risk:** None -- both sides are NO ACTION. Insert order requires either a nullable column or deferred constraints. `last_request_id` is likely nullable.

**Pair 2: `entity_workstreams` <-> `outstanding_requests`**
- `outstanding_requests.workstream_id -> entity_workstreams.workstream_id` (NO ACTION)
- `entity_workstreams.blocker_request_id -> outstanding_requests.request_id` (NO ACTION)
- **Pattern:** Workstream tracks its blocking request; request belongs to a workstream. Standard "current blocker" pattern.
- **Risk:** None -- both sides are NO ACTION. `blocker_request_id` is likely nullable.

**No 3-hop or longer cycles detected.**

**Verdict: CLEAN** -- Both mutual references are well-understood patterns with NO ACTION delete rules. No cascade risk.

---

## 4. Hub Tables (Top-10 by FK Fan-In)

| Rank | Table | Referencing Tables | Total FK Refs | CASCADE Children | Assessment |
|------|-------|--------------------|---------------|------------------|------------|
| 1 | `entities` | 88 | 119 | 30 | [FLAG] Very high fan-in; expected as core domain anchor |
| 2 | `cbus` | 64 | 68 | 35 | [FLAG] High fan-in; expected as atomic business unit |
| 3 | `cases` | 22 | 22 | 4 | [CLEAN] KYC case hub |
| 4 | `instrument_classes` | 13 | 13 | 0 | [CLEAN] Reference data |
| 5 | `products` | 13 | 13 | 1 | [CLEAN] Product catalog |
| 6 | `entity_workstreams` | 13 | 13 | 4 | [CLEAN] KYC workstream hub |
| 7 | `document_catalog` | 12 | 12 | 0 | [CLEAN] Document registry |
| 8 | `markets` | 12 | 13 | 0 | [CLEAN] Reference data |
| 9 | `deals` | 11 | 11 | 9 | [CLEAN] Commercial deal hub |
| 10 | `attribute_registry` | 10 | 11 | 3 | [CLEAN] Attribute metadata |

**Observations:**
- `entities` (88 referencing tables) and `cbus` (64) are the clear domain pillars, consistent with the CBU-centric architecture.
- `cases` (22) is the KYC hub, as expected.
- `deals` (11) is the commercial hub -- its 9 CASCADE children are all deal sub-tables (events, participants, documents, rate cards, etc.), which is correct ownership semantics.
- `instrument_classes` and `markets` are pure reference data with zero CASCADE children -- correct.

**Verdict: CLEAN** -- Hub distribution matches the expected domain architecture. The two primary hubs (`entities`, `cbus`) are the correct domain anchors.

---

## 5. CASCADE Chain Risk Map

### 5a. Maximum CASCADE chain depth: 4 hops

The deepest CASCADE chains all root at `entity_types`:

```
entity_types (26 rows)
  -> entities (CASCADE)
     -> client_group_entity (CASCADE)
        -> client_group_entity_roles (CASCADE)

entity_types
  -> entities (CASCADE)
     -> client_group_entity_tag (CASCADE)
        -> client_group_entity_tag_embedding (CASCADE)

entity_types
  -> entities (CASCADE)
     -> entity_trusts (CASCADE)
        -> trust_parties (CASCADE)

entity_types
  -> entities (CASCADE)
     -> ubo_registry (CASCADE)
        -> ubo_evidence (CASCADE)
```

**Risk assessment:** `entity_types` contains only 26 rows (reference data). Deleting an entity type would cascade through `entities` and then 2 more levels. In practice, entity types are never deleted (they are reference data), so this is a theoretical risk only.

### 5b. Blast Radius: `cbus` DELETE CASCADE

Deleting a single CBU cascades to **35 direct child tables** and up to **8 additional grandchild tables** (2-hop):

```
cbus (root)
  |-- cbu_board_controller -> board_control_evidence
  |-- cbu_product_subscriptions -> cbu_matrix_product_overlay
  |-- cbu_settlement_chains -> settlement_chain_hops
  |-- cbu_sla_commitments -> sla_measurements
  |-- cbu_ssi -> cbu_ssi_agent_override
  |-- cbu_ssi -> ssi_booking_rules
  |-- cbu_trading_profiles -> trading_profile_materializations
  |-- ubo_registry -> ubo_evidence
  |-- (27 more direct CASCADE children with no further cascade)
```

**Total blast radius of a CBU delete:** ~43 tables affected (35 direct + 8 grandchild).

**Verdict: FLAG** -- This is the largest blast radius in the schema. It is architecturally correct (a CBU owns all its sub-structures), but any accidental or bulk CBU deletion would be devastating. Recommend:
1. Application-level soft-delete rather than hard DELETE on `cbus`
2. If hard delete is needed, wrap in explicit transaction with row count assertions
3. Consider adding a `deleted_at` column for tombstone pattern

### 5c. Blast Radius: `entities` DELETE CASCADE

Deleting a single entity cascades to **30 direct child tables** and up to **4 additional grandchild tables**:

```
entities (root)
  |-- client_group_entity -> client_group_entity_roles
  |-- client_group_entity_tag -> client_group_entity_tag_embedding
  |-- entity_trusts -> trust_parties
  |-- ubo_registry -> ubo_evidence
  |-- (26 more direct CASCADE children)
```

**Total blast radius:** ~34 tables affected.

### 5d. Other Notable CASCADE Trees

| Root | Direct CASCADE Children | Max Depth | Notes |
|------|------------------------|-----------|-------|
| `client_group` | 5 | 3 | Alias -> embedding, entity -> roles, etc. |
| `deals` | 9 | 2 | rate_cards -> rate_card_lines |
| `legal_contracts` | 1 | 3 | contract_products -> cbu_subscriptions |
| `cases` | 4 | 2 | entity_workstreams -> (case_events, doc_requests, red_flags, screenings) |
| `dsl_sessions` | 5 | 1 | Session cleanup |
| `staged_runbook` | 1 | 2 | staged_command -> (staged_command_candidate, staged_command_entity) |

---

## 6. Schema Domain Boundaries

### 6a. Domains Identified from FK Graph

The FK graph reveals **12 natural domain clusters** plus reference data and infrastructure:

| Domain | Core Tables | Table Count | Key Hub | FK Isolation |
|--------|------------|-------------|---------|--------------|
| **Entity** | `entities`, `entity_*` | ~25 | `entities` (88 refs) | Moderate -- heavy cross-domain references |
| **CBU** | `cbus`, `cbu_*` | ~37 | `cbus` (64 refs) | Low -- entities, markets, instruments all flow in |
| **KYC/Case** | `cases`, `entity_workstreams`, `screenings`, `red_flags` | ~15 | `cases` (22 refs) | Good -- primarily internal FKs |
| **UBO** | `ubo_registry`, `ubo_evidence`, `ubo_assertion_log` | ~6 | `ubo_registry` | Coupled to entity + cbu |
| **Deal/Commercial** | `deals`, `deal_*` | ~11 | `deals` (11 refs) | Good -- self-contained cluster |
| **Billing** | `fee_billing_*` | ~4 | `fee_billing_profiles` | Good -- only FKs to deals + cbus |
| **Contract** | `legal_contracts`, `contract_*`, `cbu_subscriptions` | ~5 | `legal_contracts` (6 refs) | Good |
| **Client Group** | `client_group*` | ~8 | `client_group` (7 refs) | Good -- clean tree structure |
| **Service/Product** | `services`, `products`, `service_*`, `product_*` | ~10 | `products` (13 refs) | Good |
| **Share Class** | `share_classes`, `share_class_*` | ~4 | `share_classes` (10 refs) | Good |
| **DSL/Session** | `dsl_*`, `sessions`, `sheet_*` | ~14 | `dsl_sessions` (5 refs) | Good -- infrastructure |
| **ISDA** | `isda_agreements`, `csa_agreements`, `isda_product_coverage` | 3 | `isda_agreements` | Good |

### 6b. Cross-Domain FK Edge Counts (Top 10)

| From Domain | To Domain | Edge Count | Assessment |
|-------------|-----------|------------|------------|
| entity | other | 67 | Expected -- entities are universal anchors |
| cbu | other | 27 | Expected -- CBUs reference diverse sub-structures |
| entity | cbu | 16 | Expected -- cbu_entity_roles, control_anchors, etc. |
| case | other | 10 | Expected -- cases reference workstreams, events |
| market | cbu | 10 | Expected -- cbu instrument/market universe |
| entity | client | 10 | Expected -- client groups reference entities |
| document | other | 7 | Moderate coupling |
| entity | ubo | 6 | Expected -- UBO discovery from entities |
| cbu | ubo | 5 | Expected -- UBO registry per CBU |

### 6c. Domain Boundary Quality

**Well-isolated domains (clean internal FK structure, minimal external coupling):**
- Deal/Commercial (9 internal CASCADE edges, 4 cross-domain)
- Client Group (7 internal CASCADE edges, clean tree)
- Billing (4 tables, coupled only to deals + cbus)
- ISDA (3 tables, self-contained)
- Contract (clean parent-child)

**Highly coupled domains (many cross-domain edges):**
- Entity <-> CBU (16 cross-edges) -- by design, entities populate CBU roles
- Entity <-> Client Group (10 cross-edges) -- by design, client groups reference entities
- Entity <-> UBO (6 cross-edges) -- by design, UBO discovery from entities

**Verdict: CLEAN** -- Domain boundaries align with the expected CBU-centric architecture. The high coupling between `entity` and `cbu` domains is architectural (CBU roles require entity references). No unexpected cross-domain coupling detected.

---

## 7. sem_reg FK Structure

The `sem_reg` schema has **24 FK edges**, all with `NO ACTION` delete rules. This is correct for an immutable snapshot registry.

**Key structure:**
```
snapshot_sets -> snapshots (set membership)
snapshots -> snapshots (supersession self-ref)
snapshots -> changeset_entries, derivation_edges, embedding_records
changesets -> changeset_entries, changeset_reviews, conflict_records, events, focus_states
changesets -> changesets (supersession self-ref)
agent_plans -> plan_steps, decision_records, disambiguation_prompts
plan_steps -> decision_records
decision_records -> disambiguation_prompts, escalation_records
run_records -> derivation_edges
changeset_entries -> basis_records
basis_records -> basis_claims
```

**Verdict: CLEAN** -- Fully NO ACTION (immutable data), correct supersession self-references, clean hierarchy.

---

## 8. Findings Summary

### CLEAN (No Action Required)

| ID | Finding |
|----|---------|
| F-01 | Hub table distribution matches expected architecture (`entities` > `cbus` > `cases`) |
| F-02 | Self-referencing FKs are all hierarchy/supersession patterns with NO ACTION |
| F-03 | sem_reg schema is fully NO ACTION with correct immutable structure |

### MINOR (Low Priority, Track)

| ID | Finding | Recommendation |
|----|---------|----------------|
| F-04 | 10+ reference data tables (`case_types`, `screening_types`, `settlement_types`, `client_types`, `risk_ratings`, `role_categories`, `role_types`, `ssi_types`, `request_types`, `currencies`) are orphan tables with no FK consumers | Add FK constraints from consuming tables to enforce referential integrity, or document as application-enforced |
| F-05 | 5 vestigial tables (`view_modes`, `cbu_layout_overrides`, `layout_cache`, `layout_config`, `events`) appear unused post-React migration | Consider dropping in a future migration |
| F-06 | 4 BODS tables (`bods_entity_types`, `bods_interest_types`, `bods_ownership_statements`, `bods_person_statements`) are orphaned; 2 are empty | Either wire into `entity_bods_links` chain or drop if BODS import is not planned |
| F-07 | `rate_cards` table (0 rows) may be superseded by `deal_rate_cards` | Confirm and drop if redundant |
| F-08 | `sem_reg.classification_levels` and `sem_reg.templates` have no FK consumers | Wire into snapshot/changeset structure or document as application-enforced |

### FLAG (Review Required)

| ID | Finding | Risk | Recommendation |
|----|---------|------|----------------|
| F-09 | `cbus` DELETE CASCADE blast radius: 35 direct + 8 grandchild tables (~43 total) | Accidental hard delete of a CBU wipes ~43 related tables | Implement soft-delete pattern (`deleted_at` column); wrap hard deletes in assertion-guarded transactions |
| F-10 | `entities` DELETE CASCADE blast radius: 30 direct + 4 grandchild tables (~34 total) | Same risk as F-09 for entities | Same recommendation as F-09 |
| F-11 | `entity_types -> entities` CASCADE is the root of the deepest chain (4 hops) | Deleting an entity type cascades through entities and 2 more levels | Entity types are reference data (26 rows) and should never be deleted; add application-level guard or remove CASCADE in favor of RESTRICT |

### CRITICAL

None identified. The schema topology is sound.

---

## 9. Statistics Summary

| Metric | Value |
|--------|-------|
| Total tables (all schemas) | 344 |
| Total FK edges (all schemas) | 526 |
| ob-poc FK edges | 501 |
| ob-poc connected tables | 258 / 307 (84%) |
| ob-poc orphan tables | 49 (16%) |
| Self-referencing FKs | 12 tables |
| Mutual FK pairs (2-hop cycles) | 2 pairs |
| 3+ hop cycles | 0 |
| Max CASCADE chain depth | 4 hops |
| Largest CASCADE fan-out (single table) | `cbus` (35 direct children) |
| DELETE CASCADE edges | 154 (30.7%) |
| NO ACTION edges | 338 (67.5%) |
| SET NULL edges | 11 (2.2%) |
