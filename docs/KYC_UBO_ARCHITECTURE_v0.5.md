# Computed UBO KYC — Data-Centric Solution Architecture v0.5

**Project:** ob-poc  
**Date:** 2026-02-12  
**Supersedes:** v0.4 (2026-02-12)  
**Audience:** Core runtime + schema + verb/DSL implementers (Claude Code)  
**Goal:** Implementation-ready architecture. Every ambiguity that would force Claude Code to guess has been resolved.

---

## 0. Executive Summary

v0.3 introduced the Skeleton Build pipeline, import runs with rollback, confidence lifecycle, outreach plan generation, and a complete verb catalog grounded in the codebase.

v0.4 fixed schema layering (import runs → structural), added the normalize→apply contract, import-run boundaries in skeleton build, and agent verb classification.

v0.5 closes the remaining implementation ambiguities — the contracts that Claude Code needs to avoid inventing design:

1. **Edge Upsert Contract** — natural key, update semantics, multi-source conflict handling.
2. **Evidence Model Contract** — combined requirement+artifact lifecycle, clear creation and linking semantics.
3. **Coverage Output Contract** — persisted as JSONB snapshots, stable gap identifiers, outreach linkage.
4. **Tollgate/BPMN Integration Contract** — one sentence resolving the execution model.
5. **Observation ↔ Confidence Mapping** — when to use which epistemic representation.
6. **Confidence defaults fixed** — source-dependent, not blanket HIGH.
7. **Document system wiring** — references existing tables/services for evidence linking.

---

## 0.1 Changelog

| Version | Date | Key Changes |
|---------|------|-------------|
| v0.1 | 2026-02-12 | Initial draft: facts/derivations/decisions, 7 prongs, tollgates, verb families |
| v0.2 | 2026-02-12 | 3-layer schema separation, codebase grounding, Deal integration, phased prongs |
| v0.3 | 2026-02-12 | Skeleton Build pipeline, import runs, confidence lifecycle, outreach plans, complete verb catalog |
| v0.4 | 2026-02-12 | FK layering fix, normalize→apply, import-run boundaries, evidence_hint, agent verb classification |
| v0.5 | 2026-02-12 | Edge upsert contract, evidence model contract, coverage/outreach contracts, tollgate/BPMN integration, observation↔confidence mapping, confidence defaults, document system wiring |

### v0.4 → v0.5 Detailed Changes

| # | Change | Rationale |
|---|--------|-----------|
| 1 | Edge upsert natural key + update semantics specified | Prevents Claude inventing idempotency behaviour. Codebase already has `UNIQUE(from, to, type)` but temporal edges need refinement. |
| 2 | Confidence default changed from HIGH to source-dependent | `DEFAULT 'HIGH'` contradicts "claims not truth". GLEIF → MEDIUM, MANUAL → MEDIUM, AGENT_DISCOVERED → LOW. |
| 3 | Evidence model declared as combined requirement+artifact | Eliminates ambiguity about whether `ubo_evidence` is a tracker, an artifact link, or both. |
| 4 | Coverage output persisted as JSONB in determination_runs | Stable gap identifiers via `(edge_id, gap_type)` composite. Outreach items reference gaps. |
| 5 | Tollgate/BPMN integration: one sentence | "BPMN-Lite guards call `tollgate.evaluate`; results persisted in `kyc.tollgate_evaluations`." |
| 6 | Observation ↔ confidence mapping direction declared | Observations/allegations are case-scoped epistemic assertions. Confidence is structural provenance. Allegation substantiation does NOT auto-bump edge confidence. |
| 7 | Document system wiring to existing tables | `document_entity_links`, `document_attribute_mappings`, `document_metadata` — evidence verbs link to these. |
| 8 | Helper functions referenced in skeleton macro clarified | `entity.get-lei`, `entity.get-company-number` are Rust utility functions, not DSL verbs. |

---

## 1. What Exists (Codebase Map)

### 1.1 Entity Model (schema: `ob-poc`) — Solid

- `entities` — canonical entity registry with entity_type discrimination
- `entity_limited_companies`, `entity_partnerships`, `entity_trusts`, `entity_persons` — type-specific extension tables
- `client_groups`, `cbus` — client group structure and Client Business Units
- `cbu_entity_roles` — role-based membership linking entities to CBU contexts
- `role_categories`, `role_types` — config-driven role taxonomy with view applicability flags

### 1.2 Relationship Graph (schema: `ob-poc`) — Fragmented, Unification Designed

Current competing tables: `ownership_relationships`, `control_relationships`, `ubo_edges`, `entity_relationships` (target), `control_edges`.

**Decision (from Dec 2025 review):** `entity_relationships` is the single structural graph table. Migration path designed. See §5.1.

### 1.3 KYC Domain (schema: `kyc`) — Partially Built

**Investor/Holdings register:** `kyc.investors`, `kyc.holdings`, `kyc.movements`, `kyc.ownership_snapshots`, `kyc.special_rights` + views.

**Screening/monitoring:** `screening_batches`, `screening_hit_resolutions`, `monitoring_cases`, `monitoring_reviews`, `risk_ratings`.

**Observation model (exists):** `kyc.observations`, `kyc.allegations`, `kyc.discrepancies` — the existing epistemic layer for "what we think we know" before proving it.

### 1.4 Research Framework (schema: `kyc`) — Designed, Partially Implemented

**Tables:** `kyc.research_decisions`, `kyc.research_actions`, `kyc.research_corrections`, `kyc.research_anomalies`, `kyc.research_confidence_config`.

**Verb YAMLs exist for:**
- `research/gleif.yaml` — import-entity, import-hierarchy, validate-lei, refresh
- `research/companies-house.yaml` — import-company, import-officers, import-psc
- `research/sec-edgar.yaml` — import-company, import-beneficial-owners, import-13f-holders
- `research/generic.yaml` — import-entity, import-hierarchy, import-officers
- `research/screening.yaml` — record-sanctions-check, record-pep-check, record-adverse-media
- `research/workflow.yaml` — record-decision, confirm-decision, reject-decision, record-correction, audit-trail

**Agent verbs:** `agent/agent.yaml` — start, pause, resume, stop, status, respond-checkpoint, resolve-gaps, chain-research, enrich-entity, screen-entities

**Prompt templates:** `/prompts/research/sources/{gleif,companies-house,sec}/*.md` + `/prompts/research/orchestration/*.md`

**Key implementation detail:** Built-in adapters (GLEIF, Companies House, SEC) normalize source responses internally within their Rust handlers. The LLM is only the normalizer for generic/Tier-2/Tier-3 sources.

### 1.5 Infrastructure

- **DSL verb engine** — YAML-configured verbs, custom ops, execution context, template verbs (macro expansion)
- **BPMN-Lite workflow engine** — inclusive gateways, durable verbs, workstream lifecycle
- **Entity resolution pipeline** — sub-50ms, deterministic
- **Document/attribute system** — entity-linked documents with verification
- **REPL session pipeline** — session state machine, run sheets, DAG phases, `(set @symbol ...)` bindings, template expansion

### 1.6 Deal Record (schema: `ob-poc`)

`deals`, `deal_items`, `deal_principal_selections`, `deal_gates` — commercial origination container, upstream trigger for KYC cases.

---

## 2. Architectural Principles

### 2.1 Three Layers, Three Schemas

```
┌──────────────────────────────────────────────────────────────────────────┐
│  STRUCTURAL LAYER (schema: ob-poc)                                      │
│  World-facts about entities, relationships, and their provenance.       │
│  Tables: entities, entity_relationships, cbus, cbu_entity_roles, deals, │
│          graph_import_runs                                               │
│  Verbs: entity.*, edge.*, deal.*, research.import-run.*                 │
└───────────────────────────────┬──────────────────────────────────────────┘
                                │ FK (entity_id, run_id)
                                ▼
┌──────────────────────────────────────────────────────────────────────────┐
│  KYC LAYER (schema: kyc)                                                │
│  Case-scoped workflow: allegations, evidence, proof, review.            │
│  Tables: cases, workstreams, ubo_registry, evidence, case_import_runs,  │
│          observations, research_decisions, tollgate_evaluations,         │
│          outreach_plans                                                  │
│  Verbs: kyc.*, ubo.*, screening.*, tollgate.*, research.workflow.*,     │
│         observation.*, evidence.*, outreach.*                            │
└───────────────────────────────┬──────────────────────────────────────────┘
                                │ FK (regulator_code, threshold_id, etc.)
                                ▼
┌──────────────────────────────────────────────────────────────────────────┐
│  REFERENCE LAYER (schema: ob_ref)                                       │
│  Versioned config and reference data.                                   │
│  Tables: regulators, thresholds, jurisdiction_rules, doc_requirements,  │
│          tollgate_definitions, standards_mappings                        │
│  Verbs: admin.*                                                         │
└──────────────────────────────────────────────────────────────────────────┘
```

**FK rule:** downward only. KYC references structural entities and import runs. Structural never references KYC.

**v0.4 correction:** `graph_import_runs` is structural provenance — it describes where graph facts came from, independent of any KYC case. Cases *link* to runs via `kyc.case_import_runs`, preserving directionality.

### 2.2 Structural Graph vs KYC Overlay

**Structural:** "Company A owns 60% of Company B" — a world-fact with source, confidence, and import provenance.

**Overlay:** "Within case KYC-2026-001, we allege Company A owns 60% of Company B. We need a share register extract to prove this." — case-scoped epistemic state.

The overlay *reads from* the structural graph but *writes to* KYC tables. Derivation functions consume the graph; decision verbs write to the overlay.

### 2.3 Facts → Derivations → Decisions

Unchanged.

### 2.4 Claims, Not Truth

Edges enter the structural graph as claims with explicit confidence:
- `HIGH` — official register, regulated filing, verified document
- `MEDIUM` — reputable research source, client-provided, GLEIF
- `LOW` — agent-discovered, single-source, unverified

The structural edge carries an optional `evidence_hint` describing what would upgrade confidence. This is a provenance hint, not a requirement. The KYC overlay computes actual proof obligations per jurisdiction and policy, storing them in case-scoped tables. This keeps structural data reusable across cases while letting KYC rules remain case-specific.

### 2.5 Normalize → Apply Contract (v0.4)

All research imports follow a two-phase contract:

**Phase 1 — Normalize:** produce a canonical, schema-validated payload from the source response. For built-in adapters (GLEIF, Companies House, SEC), normalization happens internally within the Rust handler. For generic/LLM-adapted sources, an explicit `research.generic.normalize` verb produces the canonical payload.

**Phase 2 — Apply:** deterministic, idempotent upserts from the normalized payload. Creates/merges entities, upserts edges into `entity_relationships`, binds provenance via `import_run_id`. The apply phase is identical regardless of source.

This separation enables:
- Storing and hashing normalized artifacts for replay
- Testing normalization independently from graph mutation
- Comparing normalized outputs across sources for conflict detection

### 2.6 Use Existing Infrastructure

Tollgates → BPMN-Lite guards. Document requests → existing document/attribute system. Research → existing research framework + agent verbs. Observations → existing observation/allegation model.

---

## 2A. Implementation Contracts (v0.5)

These contracts resolve design ambiguities that would otherwise force Claude Code to invent behaviour.

### 2A.1 Edge Upsert Contract

**Natural key:** `(from_entity_id, to_entity_id, relationship_type, effective_from)`

The codebase already has `UNIQUE(from_entity_id, to_entity_id, relationship_type)` — v0.5 refines this to include `effective_from` to support temporal edge versioning (same pair, same type, different time periods).

```sql
ALTER TABLE "ob-poc".entity_relationships
ADD CONSTRAINT uq_entity_rel_natural_key
UNIQUE (from_entity_id, to_entity_id, relationship_type, effective_from);
```

**Update semantics:** end-and-insert, never update-in-place.

When an edge upsert arrives with the same natural key but different attributes (e.g. different percentage):
1. End the existing edge: set `effective_to = NOW()`.
2. Insert a new edge with the new values and `effective_from = NOW()`.
3. Both edges retain their `import_run_id` for provenance.

This preserves full history for audit. "Updating" a percentage means the old percentage is historically recorded.

**Multi-source conflict handling:** If two sources disagree about the same edge (e.g. GLEIF says 100%, Companies House says 75-100%), record both edges — each with its own `source`, `confidence`, and `import_run_id`. Raise a `research_anomaly` with severity WARNING and type `SOURCE_CONFLICT`. Do NOT auto-resolve; human/analyst review required.

**Idempotency:** Same `(natural_key, source, percentage, import_run_id)` → no-op. The handler checks for exact match before insert.

### 2A.2 Evidence Model Contract

`kyc.ubo_evidence` is a **combined requirement + artifact tracker**. Each row represents one evidence obligation for one UBO entry.

**Lifecycle:**

| Trigger | Creates/Updates | Status |
|---------|----------------|--------|
| `evidence.require` verb | Creates row | → REQUIRED |
| Outreach item sent | Updates row | → REQUESTED |
| Document received + linked via `evidence.link` | Updates `document_id` | → RECEIVED |
| Analyst verifies via `evidence.verify` | Updates `verified_at`, `verified_by` | → VERIFIED |
| Analyst rejects via `evidence.reject` | Clears `document_id` | → REJECTED (requeue as REQUESTED) |
| Override via `evidence.waive` | Updates `notes` with reason | → WAIVED |

**One row per obligation, one document per row.** If the same UBO needs both IDENTITY_DOC and OWNERSHIP_REGISTER, that's two `ubo_evidence` rows.

**Document linking:** `document_id` references the existing `"ob-poc".document_entity_links` system. When `evidence.link` fires, it:
1. Sets `ubo_evidence.document_id` to the uploaded document.
2. Ensures `document_entity_links` row exists for the UBO person entity.
3. If the document has extractable attributes (via `document_attribute_mappings`), triggers extraction.

**Requirements generation:** `evidence.require` is called by `outreach.plan.generate` based on coverage gaps. The gap type maps to evidence type:

| Gap Type | Evidence Type |
|----------|--------------|
| MISSING_PROOF (ownership edge) | OWNERSHIP_REGISTER or SHARE_CERTIFICATE |
| MISSING_PROOF (control edge) | BOARD_RESOLUTION |
| MISSING_PROOF (trust) | TRUST_DEED |
| NOMINEE_UNDISCLOSED | IDENTITY_DOC + OWNERSHIP_REGISTER |
| No identity on file | IDENTITY_DOC |

### 2A.3 Coverage Output Contract

Coverage computation produces a `CoverageResult` persisted as JSONB in `kyc.ubo_determination_runs.coverage_snapshot`.

```rust
struct CoverageResult {
    case_id: Uuid,
    as_of: NaiveDate,
    config_version: String,
    prongs: Vec<ProngCoverage>,
    overall_coverage_pct: f64,
    total_gaps: u32,
    blocking_gaps: u32,        // gaps that would fail a tollgate
}

struct ProngCoverage {
    prong: Prong,              // OWNERSHIP, GOVERNANCE, CONTRACTUAL, etc.
    coverage_pct: f64,
    total_edges: u32,
    evidenced_edges: u32,
    gaps: Vec<CoverageGap>,
}

struct CoverageGap {
    gap_id: String,            // stable: "{edge_id}:{gap_type}"
    edge_id: Uuid,
    entity_from: Uuid,
    entity_to: Uuid,
    gap_type: GapType,
    severity: Severity,        // ERROR, WARNING, INFO
    suggested_doc_type: Option<String>,
    suggested_action: String,
}
```

**Gap identifiers are stable:** `"{edge_id}:{gap_type}"` composite string. This is what `outreach_items.closes_gap_ref` references. When an outreach item is responded to and evidence verified, the gap is resolved and coverage recomputed.

**Storage:** Coverage is always a snapshot — recomputed, not incrementally updated. Each `ubo_determination_runs` row captures the coverage state at computation time. Diffing two snapshots shows progress.

### 2A.4 Tollgate / BPMN-Lite Integration Contract

BPMN-Lite workflow guards call `tollgate.evaluate` (a pure-compute derivation verb). The verb:
1. Loads the tollgate definition from `ob_ref.tollgate_definitions`.
2. Computes pass/fail against current case state (coverage, workstreams, evidence, anomalies).
3. Persists the result in `kyc.tollgate_evaluations`.
4. Returns `{passed: bool, gaps: [...]}` to the BPMN-Lite guard.

The BPMN-Lite guard uses the boolean to allow/block the workflow transition. There is ONE evaluation system, not two.

### 2A.5 Observation ↔ Confidence Mapping

Three epistemic representations serve different purposes:

| Layer | Representation | Scope | Purpose |
|-------|---------------|-------|---------|
| Structural | Edge `confidence` (LOW/MEDIUM/HIGH) | Global (per edge) | Provenance quality — how trustworthy is this source? |
| KYC | `observations` / `allegations` / `discrepancies` | Per case | Case-scoped assertions — what do we believe within this investigation? |
| KYC | `ubo_registry` lifecycle | Per case | What we've committed to proving and have proved |

**Mapping rules:**

- An **allegation** is a case-scoped belief. It may reference a `relationship_id` but does NOT modify edge confidence. Example: "We allege Company A owns 60% of Company B based on client disclosure."
- **Edge confidence** is structural provenance. It reflects source quality, not case-level belief. A GLEIF-sourced edge starts at MEDIUM regardless of what any case alleges.
- **Allegation substantiation** (via `allegation.substantiate` + linking evidence) may trigger a SEPARATE edge confidence upgrade if the evidence is a primary source (e.g. verified share register). This is an explicit analyst/system action, not automatic.
- **Discrepancies** point to two conflicting edges (via relationship_ids) or to an edge vs an allegation conflict. They are case-scoped and must be resolved before REVIEW gate.

**Decision:** These three layers are intentionally separate. Do not auto-synchronize them. The analyst decides when structural confidence should be upgraded based on case-level evidence.

### 2A.6 Confidence Defaults by Source

Edge confidence is NOT defaulted at the schema level. Each source adapter sets confidence explicitly:

| Source | Default Confidence | Rationale |
|--------|--------------------|-----------|
| GLEIF | MEDIUM | Reputable but self-reported by entities |
| COMPANIES_HOUSE | HIGH | Official government register (UK) |
| SEC_EDGAR | HIGH | Official government register (US) |
| CLIENT_PROVIDED | MEDIUM | Client disclosure, not yet verified |
| AGENT_DISCOVERED | LOW | Single source, unverified |
| MANUAL | MEDIUM | Analyst-entered, presumably has reason |
| BODS | MEDIUM | Open data, reputable but aggregated |

The schema column is `NOT NULL` with no default — the source adapter MUST set it.

### 2A.7 Document System Wiring

The evidence lifecycle wires into the existing document system. Relevant existing tables:

| Table | Schema | Purpose |
|-------|--------|---------|
| `document_entity_links` | ob-poc | Links a document to an entity |
| `document_attribute_mappings` | ob-poc | Maps document types → extractable attributes |
| `document_metadata` | ob-poc | Stores extracted attribute values per document |
| `document_catalog` | ob-poc | Document type definitions |

When `evidence.link` binds a `document_id` to a `ubo_evidence` row:
1. The document must already exist in the document system (uploaded via existing upload flow).
2. `document_entity_links` must have a row linking the document to the relevant entity.
3. If `document_attribute_mappings` defines extractable attributes for this document type, extraction runs via existing `document_extraction_service`.

No new document tables are needed. The `ubo_evidence.document_id` is a logical FK to the document system (not a hard FK, since documents are in `ob-poc` schema and may predate the KYC case).

### 2A.8 Helper Functions in Skeleton Template

`entity.get-lei` and `entity.get-company-number` referenced in the skeleton build template (§7.7) are **Rust utility functions** exposed as DSL inline expressions, not standalone verbs. They query `entity_identifiers` by entity_id and identifier type. They already exist in the codebase.

If an entity has no LEI or company number, the skeleton template skips that source adapter (the `sources` arg controls which adapters run).

---

## 3. Domain Model

### 3.1 Ownership & Control Taxonomy (Phased Prongs)

**Phase 1:** Shares/Voting, Officers/Governance, Contractual control  
**Phase 2:** Trust/Foundation, Partnership/GP  
**Phase 3:** Nominee/Custody disclosure, Fallback control persons (SMO)

### 3.2 Core Domain Objects

**Structural layer (exist):** Entity, Entity Relationship, CBU, Deal, Import Run  
**KYC layer (new + existing):** Case, Workstream, UBO Registry Entry, UBO Evidence, Determination Run, Outreach Plan, Case Import Run (join), Observation, Allegation, Discrepancy  
**Reference layer (partially exist):** Threshold Config, Document Requirements, Jurisdiction Rules, Tollgate Definitions, Standards Mappings

---

## 4. Initial Skeleton Build & Quality Gate

### 4.1 What "Skeleton" Produces

Five artifacts:

1. **Structural Graph Baseline** — `entity_relationships` populated with `source`, `confidence`, `import_run_id`. Every edge traceable to its origin.
2. **Conflict & Uncertainty Set** — anomalies in `kyc.research_anomalies` linked to import runs.
3. **Initial Derived Results** — candidate UBO list + chains (CANDIDATE only). Per-prong coverage snapshot.
4. **Targeted Outreach Plan** — minimal document requests closing biggest gaps, ranked by impact.
5. **Skeleton Quality Report** — pass/fail against SKELETON_READY gate.

### 4.2 Pipeline Steps

```
1. SCOPE    ──► Choose subject roots (client group root + key CBUs)
2. IMPORT   ──► Run source adapters, each within an import-run boundary
3. RESOLVE  ──► Entity resolution to canonical IDs (existing resolver)
4. MAP      ──► Normalize → Apply: source payloads → entity_relationships
5. VALIDATE ──► Cycles, missing %, supply inconsistency, conflicts, terminus rules
6. DERIVE   ──► Compute candidate sets + coverage + gaps
7. PLAN     ──► Generate outreach/doc requests from gap list, ranked by impact
```

### 4.3 Import Run Model (v0.4 — Layer-Correct)

Import runs are **structural provenance objects**: they describe the origin of graph facts independent of any KYC case. KYC cases link to runs via a join table, preserving FK directionality.

#### Structural provenance (schema: `ob-poc`)

```sql
CREATE TABLE "ob-poc".graph_import_runs (
    run_id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Kind / scope
    run_kind               VARCHAR(30) NOT NULL DEFAULT 'SKELETON_BUILD',
    -- SKELETON_BUILD, MANUAL_RESEARCH, REFRESH, CORRECTION_REPLAY
    scope_root_entity_id   UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    as_of                  DATE,

    -- Source
    source                 VARCHAR(30) NOT NULL,
    -- GLEIF, COMPANIES_HOUSE, SEC_EDGAR, CLIENT_PROVIDED,
    -- INTERNAL_KYC, BODS, MANUAL, AGENT_DISCOVERED
    source_query           TEXT,
    source_ref             TEXT,               -- external ID (LEI, company number, CIK)

    -- Payload audit
    payload_hash           VARCHAR(64),        -- SHA-256 of raw source response
    normalized_hash        VARCHAR(64),        -- SHA-256 of normalized canonical JSON

    -- Counts
    entities_created       INTEGER DEFAULT 0,
    entities_updated       INTEGER DEFAULT 0,
    edges_created          INTEGER DEFAULT 0,

    -- Status
    status                 VARCHAR(20) NOT NULL DEFAULT 'ACTIVE',
    -- ACTIVE, SUPERSEDED, ROLLED_BACK, PARTIAL
    superseded_by          UUID REFERENCES "ob-poc".graph_import_runs(run_id),
    superseded_reason      TEXT,

    -- Audit
    imported_at            TIMESTAMPTZ DEFAULT NOW(),
    imported_by            VARCHAR(80) NOT NULL DEFAULT 'SYSTEM'
);

CREATE INDEX idx_gir_scope_root ON "ob-poc".graph_import_runs(scope_root_entity_id);
CREATE INDEX idx_gir_source_ref ON "ob-poc".graph_import_runs(source, source_ref);
CREATE INDEX idx_gir_status     ON "ob-poc".graph_import_runs(status);
```

#### Case linkage (schema: `kyc`)

```sql
CREATE TABLE kyc.case_import_runs (
    case_id      UUID NOT NULL REFERENCES kyc.cases(case_id),
    run_id       UUID NOT NULL REFERENCES "ob-poc".graph_import_runs(run_id),
    decision_id  UUID REFERENCES kyc.research_decisions(decision_id),
    linked_at    TIMESTAMPTZ DEFAULT NOW(),
    PRIMARY KEY (case_id, run_id)
);

CREATE INDEX idx_cir_case ON kyc.case_import_runs(case_id);
```

#### Structural graph traceability

`entity_relationships` carries:
- `import_run_id UUID REFERENCES "ob-poc".graph_import_runs(run_id)` — provenance link
- `source VARCHAR(30)` — denormalized for query convenience
- `source_ref TEXT` — denormalized
- `confidence VARCHAR(10) NOT NULL` — set by source adapter per §2A.6, never defaulted
- `evidence_hint TEXT` — optional: what would upgrade confidence (hint, not obligation)

#### Rollback mechanics

When a run is marked `SUPERSEDED` or `ROLLED_BACK`:
1. All edges with that `import_run_id` get `effective_to = NOW()` (soft-end, never delete).
2. A `research.workflow.record-correction` entry is created in KYC for case(s) linked to the run.
3. Downstream derivations re-run to update candidate sets and coverage.

### 4.4 Confidence Lifecycle

```
LOW  ──► MEDIUM ──► HIGH
│              │          │
│              │          └── Official register extract verified
│              └── Client-provided doc received, not yet verified
└── Agent-discovered, single source, unverified
```

The `evidence_hint` field on an edge is a provenance-level suggestion: "what would make me more confident about this edge." Examples: "Share register extract from Company X registrar", "Board resolution confirming appointment".

Actual proof **obligations** are case/jurisdiction-scoped. The KYC layer computes what's required for a given entity in a given case, based on `ob_ref.document_type_requirements` and jurisdiction rules. Outreach plan generation and tollgate evaluation consume obligations, not hints.

### 4.5 Outreach Plan Generation

Coverage gaps feed outreach plans. Goal: "ask once, ask well."

**Rules:**
1. Prefer structure packs over one-off documents.
2. Bundle asks by prong (ownership, governance, contractual).
3. If nominee/custody detected: ask for beneficial owner disclosure + evidence, not random registers.
4. Cap items per entity (configurable, default ≤ 8). More items needed → skeleton quality is too low.

```sql
CREATE TABLE kyc.outreach_plans (
    plan_id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    case_id             UUID NOT NULL REFERENCES kyc.cases(case_id),
    workstream_id       UUID REFERENCES kyc.entity_workstreams(workstream_id),
    determination_run_id UUID REFERENCES kyc.ubo_determination_runs(run_id),
    generated_at        TIMESTAMPTZ DEFAULT NOW(),
    status              VARCHAR(20) DEFAULT 'DRAFT',
    -- DRAFT, APPROVED, SENT, PARTIALLY_RESPONDED, CLOSED
    total_items         INTEGER NOT NULL DEFAULT 0,
    items_responded     INTEGER DEFAULT 0
);

CREATE TABLE kyc.outreach_items (
    item_id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    plan_id             UUID NOT NULL REFERENCES kyc.outreach_plans(plan_id),
    prong               VARCHAR(30) NOT NULL,
    target_entity_id    UUID REFERENCES "ob-poc".entities(entity_id),
    gap_description     TEXT NOT NULL,
    request_text        TEXT NOT NULL,
    doc_type_requested  VARCHAR(50),
    priority            INTEGER DEFAULT 5,
    closes_gap_ref      TEXT,
    status              VARCHAR(20) DEFAULT 'PENDING',
    -- PENDING, SENT, RESPONDED, VERIFIED, WAIVED
    responded_at        TIMESTAMPTZ,
    document_id         UUID,
    created_at          TIMESTAMPTZ DEFAULT NOW()
);
```

### 4.6 SKELETON_READY Gate

BPMN-Lite guard before DISCOVERY → ASSESSMENT.

```json
{
  "tollgate_id": "SKELETON_READY",
  "default_thresholds": {
    "ownership_coverage_pct": 60,
    "governance_controller_identified": true,
    "high_severity_conflicts_resolved": true,
    "outreach_plan_items_max": 15,
    "cycle_anomalies_acknowledged": true,
    "minimum_sources_consulted": 2
  }
}
```

---

## 5. Schema Specification

### 5.1 Structural Graph — Unified Edge Table

```sql
CREATE TABLE "ob-poc".entity_relationships (
    relationship_id     UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- The directed edge
    from_entity_id      UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    to_entity_id        UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),

    -- Classification
    relationship_type   VARCHAR(30) NOT NULL,
    -- OWNERSHIP, CONTROL, TRUST_ROLE, MANAGEMENT, PARTNERSHIP_ROLE

    -- Ownership-specific
    percentage          DECIMAL(7,4),
    ownership_basis     VARCHAR(20),        -- DIRECT, INDIRECT, BENEFICIAL, VOTING
    share_class_id      UUID,

    -- Control-specific
    control_type        VARCHAR(30),

    -- Trust/Partnership-specific
    role_in_structure   VARCHAR(30),

    -- Temporality
    effective_from      DATE NOT NULL DEFAULT CURRENT_DATE,
    effective_to        DATE,

    -- Provenance (v0.4: layer-correct)
    source              VARCHAR(30) NOT NULL DEFAULT 'MANUAL',
    source_ref          TEXT,
    confidence          VARCHAR(10) NOT NULL,       -- set by source adapter, NOT defaulted
    import_run_id       UUID REFERENCES "ob-poc".graph_import_runs(run_id),
    evidence_hint       TEXT,               -- what would upgrade confidence (hint, not obligation)

    -- Standards xref
    bods_interest_type  VARCHAR(50),
    gleif_rel_type      VARCHAR(50),
    psc_category        VARCHAR(50),

    -- Terminus
    is_terminus         BOOLEAN DEFAULT FALSE,
    terminus_reason     VARCHAR(50),

    -- Audit
    created_at          TIMESTAMPTZ DEFAULT NOW(),
    updated_at          TIMESTAMPTZ DEFAULT NOW(),
    created_by          UUID,

    CONSTRAINT chk_rel_type CHECK (
        relationship_type IN ('OWNERSHIP','CONTROL','TRUST_ROLE','MANAGEMENT','PARTNERSHIP_ROLE')
    ),
    CONSTRAINT chk_no_self_ref CHECK (from_entity_id != to_entity_id),
    CONSTRAINT uq_entity_rel_natural_key UNIQUE (from_entity_id, to_entity_id, relationship_type, effective_from)
);

CREATE INDEX idx_er_from ON "ob-poc".entity_relationships(from_entity_id) WHERE effective_to IS NULL;
CREATE INDEX idx_er_to ON "ob-poc".entity_relationships(to_entity_id) WHERE effective_to IS NULL;
CREATE INDEX idx_er_import_run ON "ob-poc".entity_relationships(import_run_id);
```

### 5.2 KYC Case and Workstream

```sql
CREATE TABLE kyc.cases (
    case_id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    case_ref            VARCHAR(30) NOT NULL UNIQUE,
    client_group_id     UUID NOT NULL REFERENCES "ob-poc".client_groups(group_id),
    deal_id             UUID REFERENCES "ob-poc".deals(deal_id),
    case_type           VARCHAR(20) NOT NULL DEFAULT 'ONBOARDING',
    -- ONBOARDING, PERIODIC_REVIEW, EVENT_DRIVEN, REMEDIATION
    status              VARCHAR(20) NOT NULL DEFAULT 'INTAKE',
    -- INTAKE → DISCOVERY → ASSESSMENT → REVIEW → APPROVED | REJECTED | BLOCKED | WITHDRAWN | DO_NOT_ONBOARD
    risk_rating         VARCHAR(10),
    priority            VARCHAR(10) DEFAULT 'NORMAL',
    assigned_analyst_id UUID,
    assigned_reviewer_id UUID,
    due_date            DATE,
    escalation_date     DATE,
    created_at          TIMESTAMPTZ DEFAULT NOW(),
    updated_at          TIMESTAMPTZ DEFAULT NOW(),
    closed_at           TIMESTAMPTZ,
    CONSTRAINT chk_case_status CHECK (
        status IN ('INTAKE','DISCOVERY','ASSESSMENT','REVIEW',
                   'APPROVED','REJECTED','BLOCKED','WITHDRAWN','DO_NOT_ONBOARD')
    )
);

CREATE TABLE kyc.entity_workstreams (
    workstream_id       UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    case_id             UUID NOT NULL REFERENCES kyc.cases(case_id),
    entity_id           UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    inclusion_reason    VARCHAR(30) NOT NULL,
    -- UBO_CANDIDATE, CONTROLLER, ACCOUNT_HOLDER, SIGNATORY, BENEFICIAL_OWNER,
    -- NOMINEE_DISCLOSED, SMO_FALLBACK
    status              VARCHAR(20) NOT NULL DEFAULT 'OPEN',
    -- OPEN → IN_PROGRESS → BLOCKED → READY_FOR_REVIEW → CLOSED
    identity_verified   BOOLEAN DEFAULT FALSE,
    ownership_proved    BOOLEAN DEFAULT FALSE,
    screening_cleared   BOOLEAN DEFAULT FALSE,
    evidence_complete   BOOLEAN DEFAULT FALSE,
    created_at          TIMESTAMPTZ DEFAULT NOW(),
    updated_at          TIMESTAMPTZ DEFAULT NOW(),
    CONSTRAINT uq_case_entity UNIQUE (case_id, entity_id)
);
```

### 5.3 UBO Registry and Evidence

```sql
CREATE TABLE kyc.ubo_registry (
    ubo_id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    case_id             UUID NOT NULL REFERENCES kyc.cases(case_id),
    workstream_id       UUID NOT NULL REFERENCES kyc.entity_workstreams(workstream_id),
    subject_entity_id   UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    ubo_person_id       UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    ubo_type            VARCHAR(20) NOT NULL,
    -- OWNERSHIP, CONTROL, TRUST_ROLE, SMO_FALLBACK, NOMINEE_BENEFICIARY
    status              VARCHAR(20) NOT NULL DEFAULT 'CANDIDATE',
    -- CANDIDATE → IDENTIFIED → PROVABLE → PROVED → REVIEWED → APPROVED
    -- also: WAIVED, REJECTED, EXPIRED
    determination_run_id UUID REFERENCES kyc.ubo_determination_runs(run_id),
    computed_percentage  DECIMAL(7,4),
    chain_description    TEXT,
    waiver_reason       TEXT,
    waiver_authority     VARCHAR(50),
    waiver_expiry        DATE,
    risk_flags          JSONB DEFAULT '[]',
    identified_at       TIMESTAMPTZ,
    proved_at           TIMESTAMPTZ,
    reviewed_at         TIMESTAMPTZ,
    approved_at         TIMESTAMPTZ,
    created_at          TIMESTAMPTZ DEFAULT NOW(),
    updated_at          TIMESTAMPTZ DEFAULT NOW(),
    CONSTRAINT chk_ubo_status CHECK (
        status IN ('CANDIDATE','IDENTIFIED','PROVABLE','PROVED',
                   'REVIEWED','APPROVED','WAIVED','REJECTED','EXPIRED')
    )
);

CREATE TABLE kyc.ubo_evidence (
    evidence_id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    ubo_id              UUID NOT NULL REFERENCES kyc.ubo_registry(ubo_id),
    evidence_type       VARCHAR(30) NOT NULL,
    -- IDENTITY_DOC, OWNERSHIP_REGISTER, BOARD_RESOLUTION, TRUST_DEED,
    -- PARTNERSHIP_AGREEMENT, SCREENING_CLEAR, SPECIAL_RIGHTS_DOC,
    -- ANNUAL_RETURN, SHARE_CERTIFICATE, CHAIN_PROOF
    document_id         UUID,
    screening_id        UUID,
    relationship_id     UUID REFERENCES "ob-poc".entity_relationships(relationship_id),
    determination_run_id UUID,
    status              VARCHAR(20) DEFAULT 'REQUIRED',
    -- REQUIRED, REQUESTED, RECEIVED, VERIFIED, REJECTED, WAIVED, EXPIRED
    verified_at         TIMESTAMPTZ,
    verified_by         UUID,
    notes               TEXT,
    created_at          TIMESTAMPTZ DEFAULT NOW(),
    CONSTRAINT chk_evidence_status CHECK (
        status IN ('REQUIRED','REQUESTED','RECEIVED','VERIFIED','REJECTED','WAIVED','EXPIRED')
    )
);
```

### 5.4 Determination Runs

```sql
CREATE TABLE kyc.ubo_determination_runs (
    run_id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    subject_entity_id   UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    case_id             UUID REFERENCES kyc.cases(case_id),
    as_of               DATE NOT NULL,
    config_version      VARCHAR(20) NOT NULL,
    threshold_pct       DECIMAL(5,2) NOT NULL,
    code_hash           VARCHAR(64),
    candidates_found    INTEGER NOT NULL DEFAULT 0,
    output_snapshot     JSONB NOT NULL,
    chains_snapshot     JSONB,
    coverage_snapshot   JSONB,
    computed_at         TIMESTAMPTZ DEFAULT NOW(),
    computed_by         VARCHAR(50) DEFAULT 'SYSTEM',
    computation_ms      INTEGER
);
```

### 5.5 Tollgate Configuration and Results

```sql
-- Schema: ob_ref
CREATE TABLE ob_ref.tollgate_definitions (
    tollgate_id         VARCHAR(30) PRIMARY KEY,
    display_name        VARCHAR(100) NOT NULL,
    description         TEXT,
    applies_to          VARCHAR(20) NOT NULL,       -- CASE or WORKSTREAM
    required_status     VARCHAR(20),
    default_thresholds  JSONB NOT NULL,
    override_permitted  BOOLEAN DEFAULT TRUE,
    override_authority  VARCHAR(30),
    override_max_days   INTEGER
);

-- Schema: kyc
CREATE TABLE kyc.tollgate_evaluations (
    evaluation_id       UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    case_id             UUID NOT NULL REFERENCES kyc.cases(case_id),
    workstream_id       UUID REFERENCES kyc.entity_workstreams(workstream_id),
    tollgate_id         VARCHAR(30) NOT NULL REFERENCES ob_ref.tollgate_definitions(tollgate_id),
    passed              BOOLEAN NOT NULL,
    evaluation_detail   JSONB NOT NULL,
    gaps                JSONB,
    overridden          BOOLEAN DEFAULT FALSE,
    override_by         UUID,
    override_reason     TEXT,
    override_expiry     DATE,
    evaluated_at        TIMESTAMPTZ DEFAULT NOW(),
    config_version      VARCHAR(20) NOT NULL
);
```

### 5.6 Standards Mappings

```sql
CREATE TABLE ob_ref.standards_mappings (
    mapping_id          SERIAL PRIMARY KEY,
    standard            VARCHAR(20) NOT NULL,       -- BODS, GLEIF, PSC
    our_value           VARCHAR(50) NOT NULL,
    standard_value      VARCHAR(100) NOT NULL,
    standard_version    VARCHAR(20) NOT NULL,
    notes               TEXT,
    effective_from      DATE NOT NULL DEFAULT CURRENT_DATE,
    effective_to        DATE
);
```

---

## 6. Computation Engine

### 6.1 Ownership Chain Traversal

**Input:** `(subject_entity_id, as_of, threshold_pct, config_version)`

**Algorithm:**
1. Load current edges from `entity_relationships` where `relationship_type = 'OWNERSHIP'` and within `as_of` temporal window.
2. Build directed graph in-memory (Rust `petgraph` or custom adjacency list).
3. Detect cycles (Tarjan's or DFS-based). Mark cycled nodes for human review.
4. Traverse from subject upward. At each hop, multiply percentages.
5. Accumulate all paths to natural persons. Person is UBO candidate if cumulative % ≥ threshold.
6. Check terminus flags — stop traversal on terminus branches, record terminus entity as potential controller.
7. Output: list of `(person_entity_id, cumulative_pct, chain_path, prong)` tuples.

**Performance target:** < 100ms for chains up to 10 levels deep, < 500 entities in scope.

### 6.2 Governance Controller Derivation

**Input:** `(subject_entity_id, as_of, config_version)`

**Algorithm:**
1. Load control edges from `entity_relationships` where `relationship_type IN ('CONTROL', 'MANAGEMENT')`.
2. Apply deterministic priority ordering (from config): board appointment power → executive control → management contract → veto/golden share.
3. Bridge to `cbu_entity_roles` for officer/director data.
4. Output: list of `(controller_entity_id, control_type, basis_description)`.

### 6.3 Per-Prong Coverage Computation

**Input:** `(subject_entity_id, case_id, as_of, config_version)`

**Output per prong:**
```rust
struct ProngCoverage {
    prong: Prong,
    coverage_pct: f64,
    total_edges: u32,
    evidenced_edges: u32,
    gaps: Vec<CoverageGap>,
    blocking_at_gate: Option<String>,
}

struct CoverageGap {
    edge_id: Uuid,
    entity_from: Uuid,
    entity_to: Uuid,
    gap_type: GapType,          // MISSING_PROOF, MISSING_PERCENTAGE,
                                // NOMINEE_UNDISCLOSED, EXPIRED_EVIDENCE
    suggested_action: String,
    doc_type_required: Option<String>,
}
```

Gap list feeds directly into outreach plan generation (§4.5). Full output contract in §2A.3.

**Gap identifiers:** Each gap has a stable ID: `"{edge_id}:{gap_type}"`. This is referenced by `outreach_items.closes_gap_ref` to track which outreach items resolve which gaps.

### 6.4 Skeleton Validation

Post-import validation checks:
1. **Cycle detection** — Tarjan's SCC on ownership subgraph
2. **Missing denominators** — edges with `percentage IS NULL` where percentage expected
3. **Inconsistent supply** — `SUM(holdings) > 100%` for a share class + as_of
4. **Terminus integrity** — `is_terminus` entities with upstream edges
5. **Source conflict** — same entity pair, different percentages, different sources
6. **Orphan entities** — import-created entities with no edges

Results in `kyc.research_anomalies` with severity (ERROR, WARNING, INFO).

---

## 7. Complete Verb Catalog

### 7.1 Tier A — Fact Verbs (Idempotent Upserts)

#### 7.1.1 Structural Graph

| Verb | YAML | Handler | Target |
|------|------|---------|--------|
| `edge.upsert` | edge.yaml | EdgeUpsertOp | entity_relationships |
| `edge.end` | edge.yaml | EdgeEndOp | entity_relationships |
| `edge.mark-terminus` | edge.yaml | EdgeMarkTerminusOp | entity_relationships |

#### 7.1.2 Holdings / Capital Structure

| Verb | YAML | Handler | Target |
|------|------|---------|--------|
| `investor.upsert` | registry/investor.yaml | InvestorUpsertOp | kyc.investors |
| `investor.update-status` | registry/investor.yaml | InvestorStatusOp | kyc.investors |
| `holding.upsert` | registry/holding.yaml | HoldingUpsertOp | kyc.holdings |
| `holding.movement` | registry/holding.yaml | HoldingMovementOp | kyc.movements |
| `shareclass.supply.upsert` | registry/shareclass.yaml | ShareClassSupplyOp | kyc.share_class_supply |
| `boardcomposition.upsert` | — (needs YAML) | — | board_compositions |
| `special-right.upsert` | — (needs YAML) | — | kyc.special_rights |

#### 7.1.3 Entity Management

| Verb | YAML | Handler | Target |
|------|------|---------|--------|
| `entity.create` | entity.yaml | EntityCreateOp | entities |
| `entity.update` | entity.yaml | EntityUpdateOp | entities |
| `entity.merge` | entity.yaml | EntityMergeOp | entities |

#### 7.1.4 CBU / Client Group

| Verb | YAML | Handler | Target |
|------|------|---------|--------|
| `cbu.create` | cbu.yaml | CbuCreateOp | cbus |
| `cbu.assign-role` | cbu.yaml | CbuAssignRoleOp | cbu_entity_roles |
| `client-group.load` | client.yaml | ClientGroupLoadOp | client_groups |

#### 7.1.5 Import Run Lifecycle (v0.4)

| Verb | YAML | Handler | Target |
|------|------|---------|--------|
| `research.import-run.begin` | research/workflow.yaml (extend) | ImportRunBeginOp | graph_import_runs + case_import_runs |
| `research.import-run.complete` | research/workflow.yaml (extend) | ImportRunCompleteOp | graph_import_runs |
| `research.import-run.supersede` | research/workflow.yaml (extend) | ImportRunSupersedeOp | graph_import_runs (+ soft-end edges) |

### 7.2 Tier B — Derivation Verbs (Pure Compute)

| Verb | YAML | Handler | Reads From | Output |
|------|------|---------|-----------|--------|
| `ubo.compute-chains` | ubo.yaml | UboComputeChainsOp | entity_relationships | candidates + chains |
| `ubo.list-candidates` | ubo.yaml | UboListCandidatesOp | entity_relationships | candidate list |
| `control.compute-controllers` | — (new) | ControlComputeOp | entity_relationships + cbu_entity_roles | controller list |
| `coverage.compute` | — (new) | CoverageComputeOp | entity_relationships + kyc.ubo_evidence | per-prong coverage |
| `graph.detect-cycles` | graph.yaml | GraphDetectCyclesOp | entity_relationships | cycle list |
| `graph.validate` | graph.yaml | GraphValidateOp | entity_relationships | anomaly list |
| `ubo.snapshot.capture` | ubo.yaml | UboSnapshotCaptureOp | computed results | snapshot record |
| `ubo.snapshot.diff` | ubo.yaml | UboSnapshotDiffOp | two snapshots | diff report |
| `tollgate.evaluate` | — (new) | TollgateEvalOp | case + workstreams + evidence | pass/fail + detail |
| `outreach.plan.generate` | — (new) | OutreachPlanGenOp | coverage gaps | outreach plan |
| `ownership.identify-gaps` | graph.yaml | OwnershipGapsOp | entity_relationships | gap list |

### 7.3 Tier C — Decision Verbs (State Transitions)

#### 7.3.1 Case Lifecycle

| Verb | YAML | Handler |
|------|------|---------|
| `kyc.create-case` | kyc/kyc-case.yaml | KycCreateCaseOp |
| `kyc.update-status` | kyc/kyc-case.yaml | KycUpdateStatusOp |
| `kyc.assign-analyst` | kyc/kyc-case.yaml | KycAssignOp |
| `kyc.assign-reviewer` | kyc/kyc-case.yaml | KycAssignOp |
| `kyc.escalate` | kyc/kyc-case.yaml | KycEscalateOp |
| `kyc.close-case` | kyc/kyc-case.yaml | KycCloseCaseOp |

#### 7.3.2 Workstream Lifecycle

| Verb | YAML | Handler |
|------|------|---------|
| `kyc.add-workstream` | kyc/entity-workstream.yaml | KycAddWorkstreamOp |
| `kyc.workstream.update-status` | kyc/entity-workstream.yaml | KycWsStatusOp |
| `kyc.workstream.close` | kyc/entity-workstream.yaml | KycWsCloseOp |

#### 7.3.3 UBO Registry Lifecycle

| Verb | YAML | Handler | Transition |
|------|------|---------|-----------|
| `ubo.registry.promote` | ubo.yaml | UboPromoteOp | CANDIDATE → IDENTIFIED |
| `ubo.registry.advance` | ubo.yaml | UboAdvanceOp | IDENTIFIED → PROVABLE → PROVED → REVIEWED → APPROVED |
| `ubo.registry.waive` | ubo.yaml | UboWaiveOp | any → WAIVED |
| `ubo.registry.reject` | ubo.yaml | UboRejectOp | any → REJECTED |
| `ubo.registry.expire` | ubo.yaml | UboExpireOp | any → EXPIRED |

#### 7.3.4 Evidence Lifecycle

| Verb | YAML | Handler | Transition |
|------|------|---------|-----------|
| `evidence.require` | — (new) | EvidenceRequireOp | → REQUIRED |
| `evidence.link` | — (new) | EvidenceLinkOp | REQUIRED → RECEIVED |
| `evidence.verify` | — (new) | EvidenceVerifyOp | RECEIVED → VERIFIED |
| `evidence.reject` | — (new) | EvidenceRejectOp | RECEIVED → REJECTED |
| `evidence.waive` | — (new) | EvidenceWaiveOp | any → WAIVED |

#### 7.3.5 Document Requests

| Verb | YAML | Handler |
|------|------|---------|
| `doc-request.create` | kyc/doc-request.yaml | DocRequestCreateOp |
| `doc-request.send` | kyc/doc-request.yaml | DocRequestSendOp |
| `doc-request.respond` | kyc/doc-request.yaml | DocRequestRespondOp |
| `doc-request.verify` | kyc/doc-request.yaml | DocRequestVerifyOp |
| `doc-request.reject` | kyc/doc-request.yaml | DocRequestRejectOp |

#### 7.3.6 Screening

| Verb | YAML | Handler |
|------|------|---------|
| `screening.run` | kyc/case-screening.yaml | ScreeningRunOp |
| `screening.review-hit` | kyc/case-screening.yaml | ScreeningReviewOp |
| `screening.bulk-refresh` | kyc/case-screening.yaml | ScreeningBulkRefreshOp |

#### 7.3.7 Red Flags

| Verb | YAML | Handler |
|------|------|---------|
| `red-flag.raise` | kyc/red-flag.yaml | RedFlagRaiseOp |
| `red-flag.resolve` | kyc/red-flag.yaml | RedFlagResolveOp |
| `red-flag.escalate` | kyc/red-flag.yaml | RedFlagEscalateOp |

### 7.4 Research Verbs (Source Adapters)

#### 7.4.1 GLEIF

| Verb | Handler | Key Input | Output |
|------|---------|----------|--------|
| `research.gleif.import-entity` | GleifImportEntityOp | LEI | entity + LEI link |
| `research.gleif.import-hierarchy` | GleifImportHierarchyOp | LEI + direction + max-depth | entities + edges |
| `research.gleif.validate-lei` | GleifValidateLeiOp | LEI | validity status |
| `research.gleif.refresh` | GleifRefreshOp | entity-id | updated fields |

All GLEIF verbs accept optional `:import-run-id`. Normalization is handler-internal.

#### 7.4.2 Companies House (UK)

| Verb | Handler | Key Input | Output |
|------|---------|----------|--------|
| `research.companies-house.import-company` | ChImportCompanyOp | company-number | entity |
| `research.companies-house.import-officers` | ChImportOfficersOp | company-number | persons + control edges |
| `research.companies-house.import-psc` | ChImportPscOp | company-number | ownership + control edges |

All CH verbs accept optional `:import-run-id`. Normalization is handler-internal.

#### 7.4.3 SEC EDGAR (US)

| Verb | Handler | Key Input | Output |
|------|---------|----------|--------|
| `research.sec.import-company` | SecImportCompanyOp | CIK | entity |
| `research.sec.import-beneficial-owners` | SecImportBenOwnersOp | CIK | ownership edges (13D/13G) |
| `research.sec.import-13f-holders` | SecImport13FOp | CIK | institutional holdings |

All SEC verbs accept optional `:import-run-id`. Normalization is handler-internal.

#### 7.4.4 Generic (LLM-Adapted Sources)

| Verb | Handler | Key Input | Output |
|------|---------|----------|--------|
| `research.generic.normalize` | GenericNormalizeOp | source-name + raw payload | normalized_payload_ref + hash |
| `research.generic.import-entity` | GenericImportEntityOp | source-name + normalized payload | entity |
| `research.generic.import-hierarchy` | GenericImportHierarchyOp | source-name + normalized payload | entities + edges |
| `research.generic.import-officers` | GenericImportOfficersOp | source-name + normalized payload | persons + control edges |

**Normalize → Apply contract (v0.4):** For generic/LLM-adapted sources, normalization is explicit via `research.generic.normalize`. For built-in sources (GLEIF, CH, SEC), normalization is handler-internal but follows the same contract: normalize source response → hash → idempotent upserts.

#### 7.4.5 Research Workflow (Decision Audit)

| Verb | Handler | Purpose |
|------|---------|---------|
| `research.workflow.record-decision` | ResearchDecisionOp | Log candidate selection |
| `research.workflow.confirm-decision` | ResearchConfirmOp | User confirms agent choice |
| `research.workflow.reject-decision` | ResearchRejectOp | User rejects, try next |
| `research.workflow.record-correction` | ResearchCorrectionOp | Rollback / supersede |
| `research.workflow.audit-trail` | ResearchAuditOp | Query decision history |

#### 7.4.6 Screening (Record Results)

| Verb | Handler | Purpose |
|------|---------|---------|
| `research.screening.record-sanctions-check` | ScreeningSanctionsOp | Sanctions result |
| `research.screening.record-pep-check` | ScreeningPepOp | PEP classification |
| `research.screening.record-adverse-media` | ScreeningAdverseOp | Adverse media |

### 7.5 Agent Verbs (Orchestration)

Flat `agent.*` namespace, but with **declarative classification** in the verb YAML to enforce the boundary between runtime control and deterministic work.

**Why this matters:** Agent control verbs mutate the agent runtime/session state (start, pause, stop, checkpoints). Agent task verbs produce artifacts and facts and can be replayed deterministically. If both are treated as equivalent "do stuff" verbs, scripted case packs will eventually embed `agent.start` into a reproducible runbook, or LLMs will insert `agent.start` "to begin" because it's semantically adjacent to `agent.resolve-gaps`. The classification is a guardrail against both failure modes.

#### 7.5.1 Verb Classification (YAML metadata)

Every agent verb carries three classification fields:

```yaml
# Example: agent.start
agent.start:
  category: agent_control        # agent_control | agent_task
  context: interactive_only      # interactive_only | scripted_ok
  side_effects: runtime_state    # runtime_state | facts_only | mixed
```

**Linter rule:** reject any verb with `context: interactive_only` appearing inside a non-interactive execution mode (case pack, template verb body, BPMN-Lite task). This is a compile-time check, not a runtime check.

**Future migration path:** If agent verb count exceeds ~20 or durable execution introduces job-level lifecycle verbs (start-job, cancel-job, checkpoint-job), split into `agent.control.*` and `agent.task.*` namespaces at that point. The YAML metadata makes this a mechanical rename.

#### 7.5.2 Agent Control Verbs (runtime, interactive-only)

| Verb | Purpose | Triggers | category | context | side_effects |
|------|---------|----------|----------|---------|--------------|
| `agent.start` | Enter agent mode | "start the agent" | agent_control | interactive_only | runtime_state |
| `agent.pause` | Pause execution | "pause", "hold on" | agent_control | interactive_only | runtime_state |
| `agent.resume` | Resume execution | "continue", "carry on" | agent_control | interactive_only | runtime_state |
| `agent.stop` | Stop, return to manual | "stop", "cancel" | agent_control | interactive_only | runtime_state |
| `agent.status` | Get agent progress | "what's the agent doing" | agent_control | interactive_only | runtime_state |
| `agent.respond-checkpoint` | Answer forced checkpoint | "select the first" | agent_control | interactive_only | runtime_state |

#### 7.5.3 Agent Task Verbs (work, scripted-ok)

| Verb | Purpose | Triggers | category | context | side_effects |
|------|---------|----------|----------|---------|--------------|
| `agent.resolve-gaps` | Resolve ownership gaps | "resolve the gaps", "who owns" | agent_task | scripted_ok | facts_only |
| `agent.chain-research` | Build full chain | "complete the chain" | agent_task | scripted_ok | facts_only |
| `agent.enrich-entity` | Enrich single entity | "enrich this entity" | agent_task | scripted_ok | facts_only |
| `agent.screen-entities` | Run screening | "screen for sanctions" | agent_task | scripted_ok | mixed |

**Confidence thresholds:** ≥ 0.90 auto-proceed, 0.70–0.90 user checkpoint, < 0.70 try next source. Forced checkpoints on screening hits, high-stakes context, corrections.

### 7.6 Observation / Allegation Verbs (Existing)

| Verb | YAML | Purpose |
|------|------|---------|
| `observation.record` | observation/observation.yaml | Record an observed fact |
| `allegation.record` | observation/allegation.yaml | Record an unproven claim |
| `allegation.substantiate` | observation/allegation.yaml | Upgrade with evidence |
| `allegation.refute` | observation/allegation.yaml | Mark as disproven |
| `discrepancy.raise` | observation/discrepancy.yaml | Flag conflicting data |
| `discrepancy.resolve` | observation/discrepancy.yaml | Close with resolution |

### 7.7 Skeleton Build Macro Verb (v0.4 — Import-Run Boundaries)

```yaml
# config/verbs/kyc/skeleton-build.yaml
domains:
  kyc.skeleton:
    verbs:
      build:
        description: "Build initial ownership/control skeleton for a subject"
        behavior: template
        template:
          args:
            - name: case-id
              type: uuid
              required: true
            - name: subject-id
              type: uuid
              required: true
            - name: as-of
              type: date
              default: today
            - name: sources
              type: list
              default: [GLEIF, COMPANIES_HOUSE]
            - name: config-version
              type: string
              default: current

          body: |
            ; Phase 1: Import from sources
            ; Each source adapter is wrapped in an import-run boundary
            ; so every import is a discrete, rollbackable graph patch.

            ; --- GLEIF import run ---
            (set @gleif_decision (research.workflow.record-decision
              :case-id $case-id :source GLEIF :action IMPORT_HIERARCHY))

            (set @gleif_run (research.import-run.begin
              :run-kind SKELETON_BUILD
              :scope-root-entity-id $subject-id
              :as-of $as-of
              :source GLEIF
              :source-ref (entity.get-lei $subject-id)
              :source-query "hierarchy BOTH max-depth=5"
              :case-id $case-id
              :decision-id @gleif_decision))

            (research.gleif.import-hierarchy
              :lei (entity.get-lei $subject-id)
              :direction BOTH :max-depth 5
              :import-run-id @gleif_run)

            (research.import-run.complete :run-id @gleif_run :status ACTIVE)

            ; --- Companies House import run ---
            (set @ch_decision (research.workflow.record-decision
              :case-id $case-id :source COMPANIES_HOUSE :action IMPORT_PSC))

            (set @ch_run (research.import-run.begin
              :run-kind SKELETON_BUILD
              :scope-root-entity-id $subject-id
              :as-of $as-of
              :source COMPANIES_HOUSE
              :source-ref (entity.get-company-number $subject-id)
              :source-query "psc + officers"
              :case-id $case-id
              :decision-id @ch_decision))

            (research.companies-house.import-psc
              :company-number (entity.get-company-number $subject-id)
              :import-run-id @ch_run)

            (research.companies-house.import-officers
              :company-number (entity.get-company-number $subject-id)
              :import-run-id @ch_run)

            (research.import-run.complete :run-id @ch_run :status ACTIVE)

            ; Phase 2: Validate
            (graph.validate :subject-id $subject-id :as-of $as-of)

            ; Phase 3: Derive
            (ubo.compute-chains
              :subject-id $subject-id :as-of $as-of
              :threshold 25.0 :config-version $config-version
              :as @candidates)

            (coverage.compute
              :case-id $case-id :as-of $as-of
              :as @coverage)

            ; Phase 4: Plan
            (outreach.plan.generate
              :case-id $case-id
              :coverage-ref @coverage
              :candidates-ref @candidates)

            ; Phase 5: Gate check
            (tollgate.evaluate :case-id $case-id :gate SKELETON_READY)
```

**Key v0.4 changes:**
- `research.import-run.begin` creates the structural `graph_import_runs` row AND the `case_import_runs` link (accepts `:case-id` + `:decision-id`)
- Adapter verbs accept `:import-run-id` and stamp it on every edge/entity
- `research.import-run.complete` finalizes status and counts
- Later corrections use `research.import-run.supersede` to soft-end all edges from a run

---

## 8. Lifecycle Integration: Deal → Onboarding → KYC

```
Deal (AGREED)
    │
    ├──► Onboarding Runbook generated from Deal scope
    │       │
    │       ├──► KYC Case created (kyc.create-case)
    │       │       │
    │       │       ├──► DISCOVERY
    │       │       │       ├── kyc.skeleton.build (§4, §7.7)
    │       │       │       ├── SKELETON_READY gate (§4.6)
    │       │       │       └── Human review of skeleton + outreach plan
    │       │       │
    │       │       ├──► ASSESSMENT
    │       │       │       ├── Document requests from outreach plan
    │       │       │       ├── Screening runs
    │       │       │       ├── Evidence linking + verification
    │       │       │       ├── EVIDENCE_COMPLETE gate
    │       │       │       └── Loop until gates pass
    │       │       │
    │       │       ├──► REVIEW
    │       │       │       ├── Reviewer assignment
    │       │       │       ├── UBO registry → REVIEWED → APPROVED
    │       │       │       ├── REVIEW_COMPLETE gate
    │       │       │       └── Risk assessment finalized
    │       │       │
    │       │       └──► APPROVED / REJECTED / BLOCKED
    │       │
    │       ├──► Legal documentation (parallel)
    │       ├──► Service activation (parallel)
    │       └──► All gates clear → Client active
    │
    └──► Deal status updated from onboarding progress
```

---

## 9. State Machines

### 9.1 Case Lifecycle

```
INTAKE ──► DISCOVERY ──► ASSESSMENT ──► REVIEW ──► APPROVED
   │            │              │           │
   │            │              │           └──► REJECTED
   │            │              │
   │            │              └──► BLOCKED ──► ASSESSMENT
   │            │
   │            └──► WITHDRAWN
   │
   └──► DO_NOT_ONBOARD
```

DISCOVERY → ASSESSMENT guarded by SKELETON_READY tollgate.

### 9.2 UBO Determination Lifecycle

```
CANDIDATE ──► IDENTIFIED ──► PROVABLE ──► PROVED ──► REVIEWED ──► APPROVED
                  │               │          │
                  │               │          └──► WAIVED
                  │               └──► REJECTED
                  └──► EXPIRED
```

### 9.3 Evidence Lifecycle

```
REQUIRED ──► REQUESTED ──► RECEIVED ──► VERIFIED
                               │
                               └──► REJECTED ──► REQUESTED
                                                      │
                                                      └──► WAIVED
                                                      └──► EXPIRED
```

### 9.4 Import Run Lifecycle (v0.4)

```
(begin) ──► ACTIVE ──► SUPERSEDED
                  │
                  └──► ROLLED_BACK
                  │
                  └──► PARTIAL (incomplete import)
```

---

## 10. Standards Alignment

Mapping tables in `ob_ref`. Deterministic, versioned. Edges carry `bods_interest_type`, `gleif_rel_type`, `psc_category` populated by mapping function at edge-upsert time.

---

## 11. Migration Path

### Phase 0 — Graph Unification (prerequisite)

1. Create `entity_relationships` table (if not present from Dec 2025 migration).
2. Migrate data from `ownership_relationships`, `control_relationships`, `ubo_edges`, `control_edges`.
3. Create `cbu_relationship_verification` junction for CBU-scoped KYC overlay.
4. Deprecate legacy tables (do not drop).
5. Update DSL verb YAML configs.
6. Update graph builder, UBO ops, visualization queries.
7. **Test:** all 160+ existing tests pass.

### Phase 1 — KYC Case & Registry + Import Run Infrastructure

1. Create `kyc.cases`, `kyc.entity_workstreams` tables.
2. Create `kyc.ubo_registry`, `kyc.ubo_evidence` tables.
3. Create `kyc.ubo_determination_runs` table.
4. Create `"ob-poc".graph_import_runs` table (structural provenance).
5. Create `kyc.case_import_runs` join table.
6. Add `import_run_id REFERENCES "ob-poc".graph_import_runs`, `evidence_hint` columns to `entity_relationships`.
7. Implement case lifecycle verbs, UBO registry verbs, import run verbs.
8. **Test:** case can be created, import run logged, edges traced, UBO promoted through lifecycle.

### Phase 2 — Skeleton Build Pipeline

1. Implement `graph.validate` verb.
2. Implement `coverage.compute` verb (per-prong with gap identification).
3. Implement `outreach.plan.generate` verb.
4. Create `kyc.outreach_plans` and `kyc.outreach_items` tables.
5. Implement `kyc.skeleton.build` template verb.
6. Create `SKELETON_READY` tollgate definition in `ob_ref`.
7. Wire research adapters to open import runs, pass `import_run_id` into every upsert, complete/supersede runs deterministically.
8. **Test:** skeleton build runs end-to-end on golden fixture, produces quality report, outreach plan.

### Phase 3 — Derivation Engine + Tollgates

1. Implement `ubo.compute-chains` using `entity_relationships` graph.
2. Implement `control.compute-controllers`.
3. Implement snapshot capture + diff.
4. Implement tollgate evaluation as BPMN-Lite workflow guards.
5. Wire evidence requirements to document request system.
6. **Test:** case can go from INTAKE → APPROVED with all gates evaluated.

### Phase 4 — Deal Integration & Research Hardening

1. Wire `kyc.create-case` to accept `deal_id` from onboarding runbook.
2. Implement KYC approval as onboarding gate completion event.
3. Implement import run rollback/supersede mechanics end-to-end.
4. Harden research workflow with correction audit trail.
5. **Test:** end-to-end Deal → Onboarding → KYC → Approved.

---

## 12. Test Strategy

### 12.1 Golden Fixtures

1. **Simple corporate chain** — Company A (100%) → Company B (60%) → Person X = UBO at 60%.
2. **Fund umbrella** — ManCo → Sub-Fund → Share Classes → Investors.
3. **Trust chain** — Settlor → Trust → Trustee + Beneficiaries.
4. **SPV chain with nominee** — Investor → Nominee → SPV → Operating Co.
5. **Circular ownership** — A (30%) → B (40%) → C (50%) → A.
6. **Regulated entity terminus** — Pension Fund (regulated) → Company.
7. **Multi-source conflict** — GLEIF says 100%, Companies House says 75-100%.
8. **Bad import rollback** — wrong hierarchy imported, corrected, re-derived.

### 12.2 Property Tests

- Idempotent edge upserts
- Cycle detection catches all cycles
- Snapshot reproducibility (byte-for-byte JSONB match)
- Coverage monotonicity (adding evidence never decreases coverage)
- Tollgate determinism
- Import run rollback ends all edges from that run (no orphans)
- Outreach plan items ≤ configured maximum
- Skeleton quality monotonically non-decreasing as evidence accumulates

### 12.3 State Machine Tests

- Every valid transition succeeds
- Every invalid transition rejected (DB constraint + verb layer)
- Lifecycle timestamp columns populated exactly once per transition

### 12.4 Integration Tests

- Deal → Case → Skeleton Build → SKELETON_READY gate → Assessment → Evidence → Review → Approved
- Periodic review case (no deal) with existing graph + snapshot diff
- Import rollback mid-case: correction → re-derivation → updated candidates

---

## 13. Appendix — Case Pack Runbook

```lisp
; === Phase 1: Case Setup ===
(kyc.create-case client-group-id: $GROUP deal-id: $DEAL type: ONBOARDING)
(kyc.update-status case-id: $CASE status: DISCOVERY)

; === Phase 2: Skeleton Build ===
(kyc.skeleton.build case-id: $CASE subject-id: $SUBJECT as-of: 2025-01-01
  sources: [GLEIF COMPANIES_HOUSE] config-version: v2.1)

; === Phase 3: Human Review ===
; Analyst reviews graph, conflicts, outreach plan
; Agent may resolve gaps:
(agent.resolve-gaps case-id: $CASE)

; === Phase 4: Transition to Assessment ===
(kyc.update-status case-id: $CASE status: ASSESSMENT)
; guarded by SKELETON_READY tollgate

; === Phase 5: UBO Promotion ===
(ubo.registry.promote candidate-ref: $CAND_1 case-id: $CASE)
(ubo.registry.promote candidate-ref: $CAND_2 case-id: $CASE)

; === Phase 6: Evidence Collection ===
(evidence.require ubo-id: $UBO_1 type: IDENTITY_DOC)
(evidence.require ubo-id: $UBO_1 type: OWNERSHIP_REGISTER)
(evidence.link ubo-id: $UBO_1 document-id: $DOC_1 type: IDENTITY_DOC)
(evidence.verify evidence-id: $EV_1)
(screening.run entity-id: $UBO_PERSON_1 type: SANCTIONS_PEP)
(screening.review-hit hit-id: $HIT_1 decision: FALSE_POSITIVE reason: "Common name")

; === Phase 7: Gate Evaluation ===
(tollgate.evaluate case-id: $CASE gate: EVIDENCE_COMPLETE)
(kyc.update-status case-id: $CASE status: REVIEW)

; === Phase 8: Review & Approve ===
(kyc.assign-reviewer case-id: $CASE reviewer-id: $REVIEWER)
(ubo.registry.advance ubo-id: $UBO_1 to-status: REVIEWED)
(ubo.registry.advance ubo-id: $UBO_1 to-status: APPROVED)
(tollgate.evaluate case-id: $CASE gate: REVIEW_COMPLETE)
(kyc.update-status case-id: $CASE status: APPROVED)
```
