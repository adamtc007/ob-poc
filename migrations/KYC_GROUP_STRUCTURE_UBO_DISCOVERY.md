# KYC, Group Structure & UBO Discovery

## Peer Review Paper — ob-poc Data Model, Verb Catalogue & Computation Engine

**Date:** 2026-02-12
**Version:** 2.0 — Full rewrite against live schema (37 kyc tables, 10 views, 31 functions, 12 triggers)

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [KYC Case Data Model](#2-kyc-case-data-model)
3. [Entity Workstream Model](#3-entity-workstream-model)
4. [Deal–KYC Bridge](#4-dealkeyc-bridge)
5. [Ownership Graph (entity_relationships)](#5-ownership-graph-entity_relationships)
6. [Control Edges & Standards Cross-Reference](#6-control-edges--standards-cross-reference)
7. [Skeleton Build Pipeline](#7-skeleton-build-pipeline)
8. [UBO Determination Engine](#8-ubo-determination-engine)
9. [UBO Registries (Dual Model)](#9-ubo-registries-dual-model)
10. [UBO Evidence & 4-Prong Coverage Model](#10-ubo-evidence--4-prong-coverage-model)
11. [Outreach Plans & Items](#11-outreach-plans--items)
12. [Tollgate Decision Engine](#12-tollgate-decision-engine)
13. [Screening (Sanctions / PEP / Adverse Media)](#13-screening-sanctions--pep--adverse-media)
14. [Outstanding Requests](#14-outstanding-requests)
15. [Research Audit Trail](#15-research-audit-trail)
16. [Red Flags & Risk Indicators](#16-red-flags--risk-indicators)
17. [Capital Structure & Partnership](#17-capital-structure--partnership)
18. [Ownership Snapshots & Reconciliation](#18-ownership-snapshots--reconciliation)
19. [BODS / GLEIF / PSC Standard Alignment](#19-bods--gleif--psc-standard-alignment)
20. [Verb Catalogue (130 Verbs, 19 Domains)](#20-verb-catalogue-130-verbs-19-domains)
21. [Database Functions & Triggers](#21-database-functions--triggers)
22. [Views](#22-views)
23. [Reference Data (ob_ref)](#23-reference-data-ob_ref)
24. [Schema Summary](#24-schema-summary)

---

## 1. Executive Summary

The ob-poc KYC subsystem provides **end-to-end beneficial ownership discovery and verification** for custody onboarding. It spans three PostgreSQL schemas:

| Schema | Purpose | Tables | Views | Functions | Triggers |
|--------|---------|--------|-------|-----------|----------|
| `kyc` | Case management, workstreams, capital structure, research audit | 37 | 10 | 31 | 12 |
| `ob-poc` | Entity graph, ownership relationships, control edges, UBO registry | ~28 UBO-related | 8 UBO views | 11 UBO functions | 3 UBO triggers |
| `ob_ref` | Reference data: tollgate definitions, standards mappings, role types | 5 | — | — | — |

The system implements a **7-step skeleton build pipeline** that orchestrates graph import, validation, UBO chain computation, coverage analysis, outreach planning, and tollgate evaluation — all as real computation with DFS chain traversal, Tarjan cycle detection, and 4-prong coverage checks.

**Key architectural decisions:**

- **Dual UBO registry**: `ob-poc.ubo_registry` (30 columns, original operational registry with supersession chains) and `kyc.ubo_registry` (20 columns, per-case computed registry from determination runs)
- **Entity relationships** (33 columns) as the single ownership graph, with BODS share range fields and import run provenance
- **Control edges** (21 columns) with auto-triggered standards cross-reference (BODS interest type, GLEIF relationship type, PSC category)
- **4-prong coverage model**: OWNERSHIP, IDENTITY, CONTROL, SOURCE_OF_WEALTH — each evaluated per UBO candidate
- **3 tollgate gates**: SKELETON_READY (70% ownership), EVIDENCE_COMPLETE (100% identity + screening), REVIEW_COMPLETE (all UBOs approved)

---

## 2. KYC Case Data Model

### 2.1 State Machine

```
INTAKE → DISCOVERY → ASSESSMENT → REVIEW → APPROVED
                                    ↓           ↓
                                 BLOCKED     REJECTED
                                    ↓
                              REMEDIATION → DISCOVERY (cycle back)
```

Valid transitions enforced by `kyc.is_valid_case_transition()`:

| From | To |
|------|----|
| INTAKE | DISCOVERY |
| DISCOVERY | ASSESSMENT, BLOCKED |
| ASSESSMENT | REVIEW, BLOCKED |
| REVIEW | APPROVED, REJECTED, BLOCKED |
| BLOCKED | DISCOVERY, ASSESSMENT, REVIEW, REMEDIATION |
| REMEDIATION | DISCOVERY |

### 2.2 kyc.cases (25 columns)

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| `case_id` | uuid | NO | uuidv7() | Primary key |
| `cbu_id` | uuid | NO | — | FK → ob-poc.cbus |
| `status` | varchar | NO | 'INTAKE' | Case state (see state machine) |
| `case_ref` | varchar | NO | auto-generated | Human-readable ref (KYC-YYYY-SEQ), trigger-generated |
| `case_type` | varchar | YES | 'NEW_CLIENT' | NEW_CLIENT, PERIODIC_REVIEW, EVENT_DRIVEN, REMEDIATION |
| `escalation_level` | varchar | NO | 'STANDARD' | STANDARD, SENIOR_COMPLIANCE, EXECUTIVE, BOARD |
| `risk_rating` | varchar | YES | — | LOW, MEDIUM, HIGH, VERY_HIGH, PROHIBITED |
| `priority` | varchar | YES | 'NORMAL' | NORMAL, HIGH, URGENT, CRITICAL |
| `assigned_analyst_id` | uuid | YES | — | Analyst working the case |
| `assigned_reviewer_id` | uuid | YES | — | Reviewer for sign-off |
| `deal_id` | uuid | YES | — | FK → ob-poc.deals (Deal–KYC bridge) |
| `client_group_id` | uuid | YES | — | FK → ob-poc.client_group (inferred from deal if not provided) |
| `subject_entity_id` | uuid | YES | — | Primary subject entity |
| `sponsor_cbu_id` | uuid | YES | — | Sponsoring CBU for service context |
| `service_agreement_id` | uuid | YES | — | Service agreement reference |
| `service_context` | varchar | YES | — | Service context identifier |
| `kyc_standard` | varchar | YES | — | Applicable KYC standard |
| `sla_deadline` | timestamptz | YES | — | SLA deadline for case completion |
| `due_date` | date | YES | — | Business due date |
| `escalation_date` | date | YES | — | Date case auto-escalates |
| `notes` | text | YES | — | Free text notes |
| `opened_at` | timestamptz | NO | now() | Case creation timestamp |
| `closed_at` | timestamptz | YES | — | Case closure timestamp |
| `last_activity_at` | timestamptz | YES | now() | Last modification |
| `updated_at` | timestamptz | YES | now() | Row update timestamp |

**Trigger:** `trg_case_ref` (INSERT) — calls `kyc.generate_case_ref()` to produce `KYC-YYYY-NNNNNN` format.

### 2.3 Escalation Levels

| Level | Authority | Trigger |
|-------|-----------|---------|
| STANDARD | Line analyst | Default |
| SENIOR_COMPLIANCE | Senior compliance officer | PEP match, high-risk jurisdiction |
| EXECUTIVE | Executive committee | Sanctions match, prohibited entity |
| BOARD | Board level | Systemic risk, regulatory order |

---

## 3. Entity Workstream Model

### 3.1 State Machine

```
PENDING → COLLECT → VERIFY → SCREEN → ASSESS → COMPLETE
                                ↓           ↓
                          ENHANCED_DD    BLOCKED
                              ↓
                           BLOCKED (if sanctions match)
```

Valid transitions enforced by `kyc.is_valid_workstream_transition()`.

### 3.2 kyc.entity_workstreams (27 columns)

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| `workstream_id` | uuid | NO | uuidv7() | Primary key |
| `case_id` | uuid | NO | — | FK → kyc.cases |
| `entity_id` | uuid | NO | — | FK → ob-poc.entities |
| `status` | varchar | NO | 'PENDING' | Workstream state |
| `discovery_source_workstream_id` | uuid | YES | — | Which workstream discovered this entity |
| `discovery_reason` | varchar | YES | — | Why entity was included |
| `discovery_depth` | integer | YES | 1 | Depth in ownership chain where discovered |
| `inclusion_reason` | varchar | YES | — | Formal inclusion justification |
| `risk_rating` | varchar | YES | — | Per-entity risk rating |
| `risk_factors` | jsonb | YES | '[]' | Structured risk factor array |
| `is_ubo` | boolean | YES | false | Flagged as UBO candidate |
| `ownership_percentage` | numeric | YES | — | Computed ownership percentage |
| `requires_enhanced_dd` | boolean | YES | false | Enhanced due diligence required |
| `identity_verified` | boolean | YES | false | Identity prong satisfied |
| `ownership_proved` | boolean | YES | false | Ownership prong satisfied |
| `screening_cleared` | boolean | YES | false | Screening prong satisfied |
| `evidence_complete` | boolean | YES | false | All evidence collected |
| `blocker_type` | varchar | YES | — | Type of blocking issue |
| `blocker_request_id` | uuid | YES | — | Outstanding request causing block |
| `blocker_message` | varchar | YES | — | Human-readable block reason |
| `blocked_at` | timestamptz | YES | — | When blocked |
| `blocked_reason` | text | YES | — | Detailed block reason |
| `blocked_days_total` | integer | YES | 0 | Cumulative days in blocked state |
| `started_at` | timestamptz | YES | — | Work started timestamp |
| `completed_at` | timestamptz | YES | — | Work completed timestamp |
| `created_at` | timestamptz | NO | now() | Row creation |
| `updated_at` | timestamptz | YES | now() | Row update |

**Trigger:** `trg_workstream_blocked_days` (UPDATE) — increments `blocked_days_total` when status transitions from BLOCKED.

**Unique constraint:** `(case_id, entity_id)` — one workstream per entity per case.

### 3.3 Coverage Flags

The four boolean flags on entity_workstreams map to the 4-prong coverage model:

| Flag | Prong | Set By |
|------|-------|--------|
| `identity_verified` | IDENTITY | doc_request.verify (identity docs) |
| `ownership_proved` | OWNERSHIP | ubo.compute-chains (ownership edges with percentages) |
| `screening_cleared` | SCREENING | case-screening.complete (all clear) |
| `evidence_complete` | ALL | Computed when all three above are true |

---

## 4. Deal–KYC Bridge

KYC cases can be linked to commercial deals via `cases.deal_id`. This enables:

1. **Case creation from deal**: `kyc-case.create` validates deal is in status CONTRACTED, ONBOARDING, or ACTIVE
2. **Client group inference**: If `client_group_id` not provided, inferred from `deals.primary_client_group_id`
3. **Gate completion event**: When case reaches APPROVED, `kyc-case.close` emits `KYC_GATE_COMPLETED` to `ob-poc.deal_events`

```
Deal Pipeline                         KYC Pipeline
───────────                         ───────────
deal.create                         
   ↓
deal.update-status → CONTRACTED
   ↓
kyc-case.create :deal-id @deal ──→ case created (INTAKE)
                                       ↓
                                   skeleton.build
                                       ↓
                                   DISCOVERY → ASSESSMENT → REVIEW → APPROVED
                                       ↓
                                   kyc-case.close ──→ KYC_GATE_COMPLETED event
   ↓                                                    ↓
deal.update-status → ONBOARDING ←── (gate passed)
```

**Key constraint:** Deal status must be in `['CONTRACTED', 'ONBOARDING', 'ACTIVE']` for case creation. This is enforced in `KycCaseCreateOp::execute()`.

---

## 5. Ownership Graph (entity_relationships)

### 5.1 ob-poc.entity_relationships (33 columns)

The single unified ownership/control/trust graph table. Used by skeleton build for DFS chain traversal.

| Column | Type | Description |
|--------|------|-------------|
| `relationship_id` | uuid | PK |
| `from_entity_id` | uuid | Owner/controller/trustee entity |
| `to_entity_id` | uuid | Owned/controlled/trust entity |
| `relationship_type` | varchar | `ownership`, `control`, `trust_role` |
| `percentage` | numeric | Ownership percentage (0-100) |
| `ownership_type` | varchar | `DIRECT`, `INDIRECT`, `NOMINEE` |
| `control_type` | varchar | `BOARD_MAJORITY`, `VOTING_CONTROL`, `VETO_POWER`, etc. |
| `trust_role` | varchar | `SETTLOR`, `TRUSTEE`, `BENEFICIARY`, `PROTECTOR` |
| `interest_type` | varchar | Type of interest in entity |
| `trust_interest_type` | varchar | Type of trust interest |
| `trust_class_description` | text | Description of trust class |
| `direct_or_indirect` | varchar | BODS-aligned direct/indirect flag |
| `share_minimum` | numeric | BODS share range minimum |
| `share_maximum` | numeric | BODS share range maximum |
| `share_exclusive_minimum` | boolean | BODS exclusive minimum flag |
| `share_exclusive_maximum` | boolean | BODS exclusive maximum flag |
| `is_component` | boolean | Part of a composite relationship |
| `component_of_relationship_id` | uuid | Parent composite relationship |
| `is_regulated` | boolean | Regulated entity flag |
| `regulatory_jurisdiction` | varchar | Jurisdiction of regulation |
| `effective_from` | date | Start date |
| `effective_to` | date | End date (NULL = current) |
| `statement_date` | date | Date of formal statement |
| `source` | varchar | Source system (GLEIF, COMPANIES_HOUSE, MANUAL) |
| `source_document_ref` | varchar | Source document reference |
| `confidence` | varchar | Confidence level (HIGH, MEDIUM, LOW, UNVERIFIED) |
| `evidence_hint` | text | Pointer to supporting evidence |
| `replaces_relationship_id` | uuid | Supersession chain |
| `import_run_id` | uuid | FK → graph_import_runs (provenance) |
| `notes` | text | Free text |
| `created_at` | timestamptz | Row creation |
| `created_by` | uuid | Creating user/system |
| `updated_at` | timestamptz | Row update |

### 5.2 Relationship Types

| Type | Use | Key Columns |
|------|-----|-------------|
| `ownership` | Shareholding, capital interest | percentage, ownership_type, share_min/max |
| `control` | Board control, voting rights | control_type, percentage |
| `trust_role` | Trust relationships | trust_role, trust_interest_type |

### 5.3 BODS Share Range Fields

For jurisdictions that report ownership in ranges (e.g., UK PSC "25-50%"):

```
share_minimum = 25.0, share_maximum = 50.0
share_exclusive_minimum = true, share_exclusive_maximum = false
→ Represents: >25% and ≤50%
```

---

## 6. Control Edges & Standards Cross-Reference

### 6.1 ob-poc.control_edges (21 columns)

Specialized control relationship table with automatic standards mapping via trigger.

| Column | Type | Description |
|--------|------|-------------|
| `id` | uuid | PK |
| `from_entity_id` | uuid | Controlling entity |
| `to_entity_id` | uuid | Controlled entity |
| `edge_type` | text | `VOTING_CONTROL`, `BOARD_MAJORITY`, `APPOINTMENT_RIGHT`, etc. |
| `bods_interest_type` | text | Auto-mapped BODS interest type |
| `gleif_relationship_type` | text | Auto-mapped GLEIF relationship type |
| `psc_category` | text | Auto-mapped PSC category |
| `percentage` | numeric | Control percentage |
| `is_direct` | boolean | Direct vs indirect control |
| `is_beneficial` | boolean | Beneficial ownership indicator |
| `is_legal` | boolean | Legal ownership indicator |
| `share_class_id` | uuid | FK → kyc.share_classes |
| `votes_per_share` | numeric | Voting power per share |
| `source_document_id` | uuid | Source document reference |
| `source_register` | text | Source register name |
| `source_reference` | text | Source reference ID |
| `effective_date` | date | Start date |
| `end_date` | date | End date |
| `created_at` | timestamptz | Row creation |
| `updated_at` | timestamptz | Row update |
| `created_by` | text | Creating user |

### 6.2 Standards Auto-Mapping Trigger

`trg_control_edges_set_standards` (INSERT/UPDATE) calls `ob-poc.set_bods_interest_type()` which:

1. Maps `edge_type` → `bods_interest_type` (e.g., VOTING_CONTROL → voting-rights)
2. Maps `edge_type` → `gleif_relationship_type` (e.g., IS_ULTIMATELY_CONSOLIDATED_BY)
3. Maps `edge_type` → `psc_category` (e.g., voting-rights-25-to-50-percent)

This ensures every control edge is automatically classified against all three regulatory standards.

### 6.3 Standards Mapping Table

`ob_ref.standards_mappings` provides the lookup:

| edge_type | bods_interest_type | gleif_relationship_type | psc_category |
|-----------|-------------------|------------------------|--------------|
| VOTING_CONTROL | voting-rights | IS_ULTIMATELY_CONSOLIDATED_BY | voting-rights-... |
| BOARD_MAJORITY | nomination-rights | IS_ULTIMATELY_CONSOLIDATED_BY | right-to-appoint-and-remove-directors |
| APPOINTMENT_RIGHT | nomination-rights | — | right-to-appoint-and-remove-directors |
| VETO_POWER | other-influence-or-control | — | significant-influence-or-control |

---

## 7. Skeleton Build Pipeline

The **skeleton build** is the primary orchestration verb (`skeleton.build`) that chains 7 internal steps to produce a complete ownership analysis for a KYC case.

### 7.1 Pipeline Steps

```
skeleton.build :case-id @case
    │
    ├─► Step 1: import-run.begin
    │       Create graph_import_runs entry, link to case via case_import_runs
    │
    ├─► Step 2: graph.validate
    │       Load edges scoped to case entities from entity_relationships
    │       ├─ Tarjan SCC cycle detection
    │       ├─ Missing percentage check (ownership edges)
    │       ├─ Supply >100% check per target entity
    │       └─ Source conflict detection (cross-source percentage disagreements)
    │       → Persist anomalies to kyc.research_anomalies
    │
    ├─► Step 3: ubo.compute-chains
    │       Load subject entities from entity_workstreams
    │       Load all active ownership edges (entity_relationships)
    │       Build upward adjacency: to_entity → [(from_entity, pct)]
    │       DFS traversal with:
    │       ├─ Cycle detection (path contains check)
    │       ├─ Depth guard (max 20 hops)
    │       ├─ Percentage multiplication along chains
    │       └─ Terminus detection (natural person or no further owners)
    │       Threshold filter (default 5%)
    │       → Persist to kyc.ubo_determination_runs (output_snapshot, chains_snapshot)
    │
    ├─► Step 4: coverage.compute
    │       Read candidates from determination run output_snapshot
    │       Check 4 prongs per candidate:
    │       ├─ OWNERSHIP: entity_relationships with percentages
    │       ├─ IDENTITY: ubo_evidence (IDENTITY_DOC, PROOF_OF_ADDRESS) verified
    │       ├─ CONTROL: entity_relationships with control_type
    │       └─ SOURCE_OF_WEALTH: ubo_evidence (SOURCE_OF_WEALTH, etc.)
    │       → Persist coverage_snapshot to determination run
    │
    ├─► Step 5: outreach.plan-generate
    │       Read gaps from coverage_snapshot
    │       Map gaps → required document types (spec 2A.2):
    │       ├─ OWNERSHIP gap → SHARE_REGISTER
    │       ├─ IDENTITY gap → PASSPORT
    │       ├─ CONTROL gap → BOARD_RESOLUTION
    │       └─ SOURCE_OF_WEALTH gap → SOURCE_OF_WEALTH_DECLARATION
    │       Sort by priority (IDENTITY=1, OWNERSHIP=2, CONTROL=3, SOW=4)
    │       Cap at 8 items per plan
    │       → Insert to kyc.outreach_plans + kyc.outreach_items
    │
    ├─► Step 6: tollgate.evaluate-gate (SKELETON_READY)
    │       Load gate definition from ob_ref.tollgate_definitions
    │       Check ownership_coverage_pct >= 70% threshold
    │       Check all entities have at least one ownership edge
    │       → Persist evaluation to kyc.tollgate_evaluations
    │
    └─► Step 7: import-run.complete
            Mark graph_import_runs as COMPLETED
```

### 7.2 Return Type

```rust
pub struct SkeletonBuildResult {
    pub case_id: Uuid,
    pub import_run_id: Uuid,
    pub determination_run_id: Uuid,
    pub anomalies_found: i64,
    pub ubo_candidates_found: i64,
    pub coverage_pct: f64,
    pub outreach_plan_id: Option<Uuid>,
    pub skeleton_ready: bool,           // Tollgate SKELETON_READY passed?
    pub steps_completed: Vec<String>,   // ["import-run.begin", "graph.validate", ...]
}
```

### 7.3 Graph Validation Detail

The Tarjan SCC cycle detection is a full iterative implementation (not recursive) to avoid stack overflow on deep graphs. Four anomaly types are detected:

| Anomaly Type | Severity | Description |
|-------------|----------|-------------|
| `CYCLE` | ERROR | Ownership/control cycle (SCC with >1 node) |
| `MISSING_PERCENTAGE` | WARNING | Ownership edge without percentage value |
| `SUPPLY_EXCEEDS_100` | ERROR | Total inbound ownership >100% for an entity |
| `SOURCE_CONFLICT` | WARNING/ERROR | Different sources report different percentages |

All anomalies are persisted to `kyc.research_anomalies` via a `research_actions` audit entry.

---

## 8. UBO Determination Engine

### 8.1 Algorithm (DFS Chain Traversal)

For each subject entity in the case's workstreams:

1. **Build upward adjacency list**: `to_entity_id → Vec<(from_entity_id, percentage)>` from active `entity_relationships` where `relationship_type = 'ownership'`
2. **DFS with percentage multiplication**: Starting from subject entity, traverse upward. At each hop, multiply cumulative percentage by edge percentage / 100
3. **Terminus detection**: Stop when reaching a natural person (entity_category = 'PERSON') or entity with no further owners
4. **Cycle guard**: If path already contains the next entity, record cycle chain with 0% effective ownership
5. **Depth guard**: Max 20 hops to prevent runaway traversal
6. **Threshold filter**: Default 5% — candidates below threshold are excluded (unless they're in a cycle)

### 8.2 kyc.ubo_determination_runs (14 columns)

| Column | Type | Description |
|--------|------|-------------|
| `run_id` | uuid | PK (uuidv7) |
| `subject_entity_id` | uuid | Primary subject entity |
| `case_id` | uuid | FK → kyc.cases |
| `as_of` | date | Determination date |
| `config_version` | varchar | Algorithm config version (e.g., 'v1.0') |
| `threshold_pct` | numeric | Threshold used (default 5.0) |
| `candidates_found` | integer | Number of UBO candidates identified |
| `output_snapshot` | jsonb | Full candidate array with entity_id, ownership_pct, chains |
| `chains_snapshot` | jsonb | Flattened chain paths with effective percentages |
| `coverage_snapshot` | jsonb | 4-prong coverage results (written by coverage.compute) |
| `computed_at` | timestamptz | Computation timestamp |
| `computed_by` | varchar | Computing system ('skeleton.build', 'manual', etc.) |
| `computation_ms` | integer | Computation time in milliseconds |
| `created_at` | timestamptz | Row creation |

### 8.3 output_snapshot Structure

```json
[
  {
    "entity_id": "uuid-...",
    "entity_name": "John Smith",
    "total_ownership_pct": 35.5,
    "chain_count": 2,
    "is_terminus": true,
    "chains": [
      { "path": ["subject-uuid", "intermediate-uuid", "john-uuid"], "effective_pct": 20.0 },
      { "path": ["subject-uuid", "other-intermediate-uuid", "john-uuid"], "effective_pct": 15.5 }
    ]
  }
]
```

---

## 9. UBO Registries (Dual Model)

### 9.1 ob-poc.ubo_registry (30 columns) — Operational Registry

The original operational UBO registry with full supersession chain support, regulatory framework tracking, and evidence document references.

| Column | Type | Description |
|--------|------|-------------|
| `ubo_id` | uuid | PK |
| `cbu_id` | uuid | FK → cbus |
| `subject_entity_id` | uuid | Entity being analyzed |
| `ubo_proper_person_id` | uuid | Natural person identified as UBO |
| `relationship_type` | varchar | How UBO is connected |
| `qualifying_reason` | varchar | Why they qualify (ownership, control, trust) |
| `status` | varchar | CANDIDATE → IDENTIFIED → VERIFIED → APPROVED (with REJECTED, EXPIRED, WAIVED) |
| `workflow_type` | varchar | Type of workflow that created this entry |
| `regulatory_framework` | varchar | Applicable regulation (4AMLD, 5AMLD, FinCEN) |
| `ownership_percentage` | numeric | Computed ownership % |
| `is_direct` | boolean | Direct vs indirect ownership |
| `evidence_doc_ids` | uuid[] | Array of evidence document references |
| `superseded_by_id` | uuid | Self-FK for supersession chain |
| `supersedes_id` | uuid | Self-FK for supersession chain |
| `verified_at` | timestamptz | Verification timestamp |
| `verified_by` | uuid | Verifying user |
| `approved_at` | timestamptz | Approval timestamp |
| `approved_by` | uuid | Approving user |
| `waived_at` | timestamptz | Waiver timestamp |
| `waived_by` | uuid | Waiver authority |
| `waiver_reason` | text | Waiver justification |
| ... | ... | + timestamps, notes, created_by |

**State machine** enforced by trigger `trg_ubo_status_transition` → `ob-poc.is_valid_ubo_transition()`:

```
CANDIDATE → IDENTIFIED → VERIFIED → APPROVED
     ↓           ↓           ↓
  REJECTED    REJECTED    REJECTED
                           EXPIRED
                           WAIVED
```

**Verb domain:** `ubo.registry` (5 verbs: promote, advance, waive, reject, expire)

### 9.2 kyc.ubo_registry (20 columns) — Per-Case Computed Registry

The per-case UBO registry populated by determination runs. Links to workstreams and evidence.

| Column | Type | Description |
|--------|------|-------------|
| `ubo_id` | uuid | PK (uuidv7) |
| `case_id` | uuid | FK → kyc.cases |
| `workstream_id` | uuid | FK → kyc.entity_workstreams |
| `subject_entity_id` | uuid | Entity being analyzed |
| `ubo_person_id` | uuid | Natural person identified as UBO |
| `ubo_type` | varchar | OWNERSHIP, CONTROL, TRUST, NOMINEE |
| `status` | varchar | CANDIDATE, IDENTIFIED, VERIFIED, APPROVED, REJECTED, WAIVED, EXPIRED |
| `determination_run_id` | uuid | FK → kyc.ubo_determination_runs |
| `computed_percentage` | numeric | Ownership percentage from chain computation |
| `chain_description` | text | Human-readable chain summary |
| `waiver_authority` | varchar | Who authorized waiver |
| `waiver_reason` | text | Waiver justification |
| `waived_at` | timestamptz | Waiver timestamp |
| `risk_flags` | jsonb | Structured risk indicators |
| `created_at` | timestamptz | Row creation |
| `updated_at` | timestamptz | Row update |

### 9.3 Why Two Registries?

| Aspect | ob-poc.ubo_registry | kyc.ubo_registry |
|--------|---------------------|-------------------|
| Scope | Per-CBU, operational | Per-case, computed |
| Lifecycle | Long-lived, with supersession | Case-bounded |
| Evidence | evidence_doc_ids array | FK → kyc.ubo_evidence |
| Regulatory | regulatory_framework field | Via case.kyc_standard |
| Population | Manual or workflow | Determination runs |

---

## 10. UBO Evidence & 4-Prong Coverage Model

### 10.1 kyc.ubo_evidence (12 columns)

| Column | Type | Description |
|--------|------|-------------|
| `evidence_id` | uuid | PK |
| `ubo_id` | uuid | FK → kyc.ubo_registry |
| `evidence_type` | varchar | IDENTITY_DOC, PROOF_OF_ADDRESS, SOURCE_OF_WEALTH, SOURCE_OF_FUNDS, ANNUAL_RETURN, CHAIN_PROOF |
| `document_id` | uuid | FK → document reference |
| `screening_id` | uuid | FK → kyc.screenings |
| `relationship_id` | uuid | FK → entity_relationships |
| `determination_run_id` | uuid | FK → ubo_determination_runs |
| `status` | varchar | REQUIRED, RECEIVED, VERIFIED, REJECTED, WAIVED |
| `verified_at` | timestamptz | Verification timestamp |
| `verified_by` | varchar | Verifying user |
| `created_at` | timestamptz | Row creation |
| `updated_at` | timestamptz | Row update |

**Verb domain:** `evidence` (5 verbs: require, link, verify, reject, waive)

### 10.2 4-Prong Coverage Model

Each UBO candidate is assessed against four prongs:

| Prong | What Must Be Proved | DB Check | Gap Doc Type |
|-------|--------------------|-----------|----|
| **OWNERSHIP** | Ownership edges with percentages exist | `entity_relationships WHERE percentage IS NOT NULL` | SHARE_REGISTER |
| **IDENTITY** | Identity documents verified | `ubo_evidence (IDENTITY_DOC/PROOF_OF_ADDRESS) WHERE status = VERIFIED` OR `workstreams.identity_verified = true` | PASSPORT |
| **CONTROL** | Control relationships documented | `entity_relationships WHERE control_type IS NOT NULL` | BOARD_RESOLUTION |
| **SOURCE_OF_WEALTH** | SOW evidence received/verified | `ubo_evidence (SOURCE_OF_WEALTH/SOURCE_OF_FUNDS/ANNUAL_RETURN/CHAIN_PROOF) WHERE status IN (VERIFIED, RECEIVED)` | SOURCE_OF_WEALTH_DECLARATION |

Coverage percentage per prong = `(covered_candidates / total_candidates) * 100`. Overall coverage = average of 4 prongs.

---

## 11. Outreach Plans & Items

### 11.1 kyc.outreach_plans (8 columns)

| Column | Type | Description |
|--------|------|-------------|
| `plan_id` | uuid | PK |
| `case_id` | uuid | FK → kyc.cases |
| `determination_run_id` | uuid | FK → ubo_determination_runs |
| `status` | varchar | DRAFT, ACTIVE, COMPLETED, CANCELLED |
| `total_items` | integer | Number of outreach items |
| `created_at` | timestamptz | Row creation |
| `updated_at` | timestamptz | Row update |
| `completed_at` | timestamptz | Plan completion |

### 11.2 kyc.outreach_items (12 columns)

| Column | Type | Description |
|--------|------|-------------|
| `item_id` | uuid | PK |
| `plan_id` | uuid | FK → kyc.outreach_plans |
| `prong` | varchar | OWNERSHIP, IDENTITY, CONTROL, SOURCE_OF_WEALTH |
| `target_entity_id` | uuid | Entity needing evidence |
| `gap_description` | text | Human-readable gap description |
| `request_text` | text | Outreach request message |
| `doc_type_requested` | varchar | Required document type |
| `priority` | integer | 1=IDENTITY, 2=OWNERSHIP, 3=CONTROL, 4=SOW |
| `closes_gap_ref` | varchar | Reference to coverage gap (prong:entity_id) |
| `status` | varchar | PENDING, SENT, RECEIVED, VERIFIED, WAIVED |
| `created_at` | timestamptz | Row creation |
| `updated_at` | timestamptz | Row update |

### 11.3 Gap-to-Document Mapping (Spec 2A.2)

| Coverage Gap | Doc Type | Priority |
|-------------|----------|----------|
| Missing ownership evidence | SHARE_REGISTER | 2 |
| Missing identity docs | PASSPORT | 1 (highest) |
| Missing control documentation | BOARD_RESOLUTION | 3 |
| Missing source of wealth | SOURCE_OF_WEALTH_DECLARATION | 4 |

Plans are capped at **8 items** per plan to avoid overwhelming outreach.

---

## 12. Tollgate Decision Engine

### 12.1 Three Gates

Defined in `ob_ref.tollgate_definitions`:

| Gate ID | Display Name | Applies To | Required Status | Default Thresholds |
|---------|-------------|------------|-----------------|-------------------|
| `SKELETON_READY` | Skeleton Build Complete | CASE | DISCOVERY | ownership_coverage_pct: 70%, minimum_sources_consulted: 1, cycle_anomalies_acknowledged: true, high_severity_conflicts_resolved: true |
| `EVIDENCE_COMPLETE` | Evidence Collection Complete | CASE | ASSESSMENT | identity_docs_verified_pct: 100%, ownership_coverage_pct: 95%, screening_cleared_pct: 100%, outreach_plan_items_max: 0 |
| `REVIEW_COMPLETE` | Review Complete | CASE | REVIEW | all_ubos_approved: true, all_workstreams_closed: true, no_open_discrepancies: true |

### 12.2 Gate Evaluation Flow

```
tollgate.evaluate-gate :case-id @case :gate "SKELETON_READY"
    │
    ├─ Load gate definition from ob_ref.tollgate_definitions
    ├─ Compute current metrics against thresholds
    ├─ Record pass/fail in kyc.tollgate_evaluations
    └─ Return evaluation detail with per-check breakdown
```

### 12.3 kyc.tollgate_evaluations (12 columns)

| Column | Type | Description |
|--------|------|-------------|
| `evaluation_id` | uuid | PK |
| `case_id` | uuid | FK → kyc.cases |
| `workstream_id` | uuid | Optional FK → entity_workstreams |
| `tollgate_id` | varchar | Gate identifier (SKELETON_READY, etc.) |
| `passed` | boolean | Did the gate pass? |
| `evaluation_detail` | jsonb | Full check-by-check breakdown |
| `gaps` | jsonb | Remaining gaps if failed |
| `override_id` | uuid | If overridden |
| `override_reason` | text | Override justification |
| `override_authority` | varchar | Who approved override |
| `config_version` | varchar | Config version used |
| `created_at` | timestamptz | Row creation |

### 12.4 Evaluation Detail Structure

```json
{
  "gate_name": "SKELETON_READY",
  "passed": true,
  "checks": [
    {
      "criterion": "ownership_coverage_pct",
      "passed": true,
      "actual_value": 85.0,
      "threshold_value": 70.0,
      "detail": "Ownership coverage 85.0% (threshold: 70.0%): 34 of 40 entities proved"
    },
    {
      "criterion": "all_entities_have_ownership_edge",
      "passed": true,
      "actual_value": 0,
      "threshold_value": 0,
      "detail": "0 workstream entities without ownership edges"
    }
  ]
}
```

### 12.5 Verb Domain: tollgate (10 verbs)

| Verb | Behavior | Description |
|------|----------|-------------|
| `evaluate` | plugin | Run tollgate evaluation for a case |
| `evaluate-gate` | plugin | Evaluate a named tollgate gate |
| `get-metrics` | plugin | Compute current metrics without recording |
| `set-threshold` | crud | Update a tollgate threshold |
| `override` | plugin | Record management override |
| `get-decision-readiness` | plugin | Summary view of decision readiness |
| `list-evaluations` | crud | List evaluations for a case |
| `list-thresholds` | crud | List configured thresholds |
| `list-overrides` | crud | List overrides for a case |
| `expire-override` | crud | Manually expire an override |

---

## 13. Screening (Sanctions / PEP / Adverse Media)

### 13.1 kyc.screenings (15 columns)

| Column | Type | Description |
|--------|------|-------------|
| `screening_id` | uuid | PK |
| `workstream_id` | uuid | FK → entity_workstreams |
| `screening_type` | varchar | SANCTIONS, PEP, ADVERSE_MEDIA |
| `status` | varchar | PENDING, IN_PROGRESS, COMPLETED, HIT_REVIEW |
| `provider` | varchar | Screening provider name |
| `provider_ref` | varchar | Provider reference ID |
| `hit_count` | integer | Number of hits found |
| `hit_details` | jsonb | Structured hit information |
| `result_summary` | text | Summary of results |
| `reviewed_by` | uuid | Reviewer |
| `reviewed_at` | timestamptz | Review timestamp |
| `review_outcome` | varchar | CLEARED, ESCALATED, BLOCKED |
| `created_at` | timestamptz | Row creation |
| `updated_at` | timestamptz | Row update |
| `completed_at` | timestamptz | Screening completion |

**Idempotency:** Unique on `(workstream_id, screening_type)` — re-running produces a new screening result.

### 13.2 Verb Domain: case-screening (4 verbs)

| Verb | Behavior | Description |
|------|----------|-------------|
| `run` | crud | Initiate a screening |
| `complete` | crud | Record screening completion |
| `review-hit` | crud | Review a screening hit |
| `list-by-workstream` | crud | List screenings for a workstream |

### 13.3 Screening → Workstream Impact

When a screening completes with hits:
- **SANCTIONS match** → workstream transitions to BLOCKED
- **PEP match** → workstream transitions to ENHANCED_DD
- **ADVERSE_MEDIA** → red flag raised, analyst review required
- **All clear** → `screening_cleared = true` on workstream

---

## 14. Outstanding Requests

### 14.1 kyc.outstanding_requests (37 columns)

A polymorphic request tracker supporting document solicitation, information requests, and approvals with escalation, reminders, and fulfillment tracking.

| Column Group | Key Columns | Description |
|-------------|-------------|-------------|
| Identity | request_id (PK), request_ref, request_type | Core identity |
| Context | case_id, workstream_id, entity_id | Links to case/workstream/entity |
| Request | requested_of, requested_by, request_method | Who, by whom, how |
| Content | subject_line, body_text, doc_type_requested | What is requested |
| Dates | due_date, created_at, sent_at, reminded_at | Tracking dates |
| Status | status, priority, reminder_count | PENDING, SENT, OVERDUE, FULFILLED, CANCELLED, ESCALATED |
| Fulfillment | fulfilled_at, fulfilled_by, fulfillment_notes | How it was resolved |
| Escalation | escalated_at, escalated_to, escalation_reason | Escalation tracking |

### 14.2 Verb Domain: request (8 verbs)

| Verb | Behavior | Description |
|------|----------|-------------|
| `create` | plugin | Create an outstanding request |
| `list` | crud | List outstanding requests |
| `overdue` | plugin | List overdue requests |
| `fulfill` | plugin | Mark request as fulfilled |
| `cancel` | plugin | Cancel a pending request |
| `extend` | plugin | Extend request due date |
| `remind` | plugin | Send reminder for pending request |
| `escalate` | plugin | Escalate overdue request |

---

## 15. Research Audit Trail

### 15.1 Four-Table Research Model

The research audit trail tracks every data-gathering action, anomaly detected, and correction applied during KYC investigation.

```
kyc.research_decisions (18 cols)
    │  "What did we decide to research?"
    │
    └──→ kyc.research_actions (22 cols)
              │  "What did we actually do?"
              │
              ├──→ kyc.research_anomalies (12 cols)
              │        "What problems did we find?"
              │
              └──→ kyc.research_corrections (12 cols)
                       "How did we fix them?"
```

### 15.2 kyc.research_decisions (Key Columns)

| Column | Type | Description |
|--------|------|-------------|
| `decision_id` | uuid | PK |
| `case_id` | uuid | FK → kyc.cases |
| `target_entity_id` | uuid | Entity being researched |
| `decision_type` | varchar | INITIAL_RESEARCH, FOLLOW_UP, VERIFICATION, CORRECTION |
| `source_provider` | varchar | GLEIF, COMPANIES_HOUSE, ORBIS, MANUAL |
| `priority` | integer | Research priority |
| `status` | varchar | PENDING, IN_PROGRESS, COMPLETED, FAILED |
| `rationale` | text | Why this research was needed |

### 15.3 kyc.research_actions (Key Columns)

| Column | Type | Description |
|--------|------|-------------|
| `action_id` | uuid | PK |
| `target_entity_id` | uuid | Entity researched |
| `action_type` | varchar | GLEIF_IMPORT, REGISTRY_LOOKUP, GRAPH_VALIDATION, MANUAL_ENTRY |
| `source_provider` | varchar | Source system |
| `source_key` | varchar | External reference key |
| `source_key_type` | varchar | Type of key (LEI, company_number, etc.) |
| `verb_domain` | varchar | DSL verb domain used |
| `verb_name` | varchar | DSL verb name used |
| `verb_args` | jsonb | Arguments passed to verb |
| `success` | boolean | Whether action succeeded |
| `entities_created` | integer | Count of entities created |
| `entities_updated` | integer | Count of entities updated |
| `relationships_created` | integer | Count of relationships created |

### 15.4 kyc.research_anomalies (Key Columns)

| Column | Type | Description |
|--------|------|-------------|
| `anomaly_id` | uuid | PK |
| `action_id` | uuid | FK → research_actions |
| `entity_id` | uuid | Affected entity |
| `rule_code` | varchar | GRAPH_CYCLE_DETECTED, GRAPH_MISSING_PCT, GRAPH_SUPPLY_GT_100, GRAPH_SOURCE_CONFLICT |
| `severity` | varchar | ERROR, WARNING, INFO |
| `description` | text | Human-readable description |
| `status` | varchar | OPEN, ACKNOWLEDGED, RESOLVED, DISMISSED |

### 15.5 kyc.research_corrections (Key Columns)

| Column | Type | Description |
|--------|------|-------------|
| `correction_id` | uuid | PK |
| `action_id` | uuid | FK → research_actions |
| `anomaly_id` | uuid | FK → research_anomalies |
| `correction_type` | varchar | DATA_FIX, SUPERSESSION, MANUAL_OVERRIDE |
| `before_value` | jsonb | State before correction |
| `after_value` | jsonb | State after correction |
| `applied_by` | varchar | Who applied the correction |

---

## 16. Red Flags & Risk Indicators

### 16.1 kyc.red_flags

| Column | Type | Description |
|--------|------|-------------|
| `red_flag_id` | uuid | PK |
| `case_id` | uuid | FK → kyc.cases |
| `workstream_id` | uuid | Optional FK → entity_workstreams |
| `flag_type` | varchar | PEP_MATCH, SANCTIONS_HIT, ADVERSE_MEDIA, HIGH_RISK_JURISDICTION, COMPLEX_STRUCTURE, etc. |
| `severity` | varchar | LOW, MEDIUM, HIGH, CRITICAL |
| `description` | text | Flag description |
| `status` | varchar | OPEN, MITIGATED, WAIVED, DISMISSED |
| `is_blocking` | boolean | Whether flag blocks case progression |
| `mitigated_by` | uuid | Who mitigated |
| `mitigation_notes` | text | Mitigation justification |
| `created_at` | timestamptz | Row creation |
| `updated_at` | timestamptz | Row update |

### 16.2 Verb Domain: red-flag (7 verbs)

| Verb | Behavior | Description |
|------|----------|-------------|
| `raise` | crud | Raise a new red flag |
| `mitigate` | crud | Mark as mitigated |
| `waive` | crud | Waive with justification |
| `dismiss` | crud | Dismiss as false positive |
| `set-blocking` | crud | Set flag as blocking |
| `list-by-case` | crud | List flags for a case |
| `list-by-workstream` | crud | List flags for a workstream |

---

## 17. Capital Structure & Partnership

### 17.1 kyc.share_classes

Corporate share class definitions with voting rights and special features.

| Column | Type | Description |
|--------|------|-------------|
| `share_class_id` | uuid | PK |
| `entity_id` | uuid | Issuing entity |
| `class_name` | varchar | e.g., "Ordinary A", "Preference B" |
| `class_type` | varchar | ORDINARY, PREFERENCE, NON_VOTING, REDEEMABLE |
| `votes_per_share` | numeric | Voting power |
| `issued_shares` | numeric | Total shares issued |
| `par_value` | numeric | Nominal value per share |
| `currency` | varchar | Share currency |

Related tables: `kyc.share_class_supply` (supply tracking), `kyc.share_class_identifiers` (ISIN, SEDOL), `kyc.special_rights` (veto, drag-along, etc.).

### 17.2 kyc.holdings

Shareholding records linking holders to share classes.

| Column | Type | Description |
|--------|------|-------------|
| `holding_id` | uuid | PK |
| `share_class_id` | uuid | FK → share_classes |
| `holder_entity_id` | uuid | FK → entities |
| `shares_held` | numeric | Number of shares |
| `percentage` | numeric | Computed ownership % |
| `holding_type` | varchar | BENEFICIAL, LEGAL, NOMINEE |

**Trigger:** `trg_sync_holding_to_ubo` (INSERT/UPDATE) — calls `kyc.sync_holding_to_ubo_relationship()` to maintain entity_relationships in sync with holdings changes.

### 17.3 Verb Domains

**capital** (9 verbs): define-share-class, allocate, transfer, reconcile, get-ownership-chain, issue-shares, cancel-shares, list-by-issuer, list-shareholders

**partnership** (7 verbs): add-partner, record-contribution, record-distribution, withdraw-partner, list-partners, reconcile, analyze-control

**trust** (8 verbs): record-provision, update-provision, end-provision, list-provisions, list-by-holder, analyze-control, identify-ubos, classify

---

## 18. Ownership Snapshots & Reconciliation

### 18.1 kyc.ownership_snapshots (22 columns)

Point-in-time captures of ownership state for comparison and audit.

| Key Columns | Description |
|-------------|-------------|
| `snapshot_id` | PK |
| `entity_id` | Subject entity |
| `as_of_date` | Snapshot date |
| `ownership_data` | jsonb — full ownership tree |
| `total_identified_pct` | Sum of identified ownership |
| `ubo_count` | Number of UBOs identified |
| `capture_source` | What triggered the snapshot (SKELETON_BUILD, PERIODIC, MANUAL) |

### 18.2 kyc.ownership_reconciliation_runs (16 columns)

Comparison between two snapshots to detect changes.

| Key Columns | Description |
|-------------|-------------|
| `reconciliation_id` | PK |
| `entity_id` | Subject entity |
| `baseline_snapshot_id` | Earlier snapshot |
| `current_snapshot_id` | Later snapshot |
| `changes_detected` | boolean |
| `added_relationships` | integer |
| `removed_relationships` | integer |
| `changed_percentages` | integer |

### 18.3 kyc.ownership_reconciliation_findings (13 columns)

Individual findings from reconciliation.

| Key Columns | Description |
|-------------|-------------|
| `finding_id` | PK |
| `reconciliation_id` | FK → reconciliation_runs |
| `finding_type` | ADDED, REMOVED, PERCENTAGE_CHANGED, STATUS_CHANGED |
| `entity_id` | Affected entity |
| `detail` | jsonb — before/after values |

---

## 19. BODS / GLEIF / PSC Standard Alignment

### 19.1 Three-Standard Cross-Reference

Every ownership/control relationship is mapped to three regulatory standards:

| Standard | Scope | Key Concept |
|----------|-------|-------------|
| **BODS** (Beneficial Ownership Data Standard) | Global open data | Interest types (voting-rights, shareholding, etc.) |
| **GLEIF** (Global LEI Foundation) | LEI-registered entities | Relationship types (IS_DIRECTLY_CONSOLIDATED_BY, etc.) |
| **PSC** (Persons with Significant Control) | UK Companies Act | Categories (ownership-of-shares-25-to-50-percent, etc.) |

### 19.2 ob-poc Tables for Standard Compliance

| Table | Purpose |
|-------|---------|
| `bods_entity_statements` | BODS entity statement records |
| `bods_person_statements` | BODS person statement records |
| `bods_ownership_statements` | BODS ownership/control interest statements |
| `entity_bods_links` | Links ob-poc entities to BODS statement IDs |
| `control_edges` | Control relationships with auto-mapped standards |
| `ob_ref.standards_mappings` | Cross-reference lookup table |

### 19.3 Auto-Mapping Pipeline

```
1. User creates control edge → INSERT INTO ob-poc.control_edges
2. Trigger fires → trg_control_edges_set_standards
3. Trigger calls → ob-poc.set_bods_interest_type()
4. Function looks up → ob_ref.standards_mappings WHERE edge_type = NEW.edge_type
5. Sets → bods_interest_type, gleif_relationship_type, psc_category
```

This ensures compliance classification is always current without manual intervention.

---

## 20. Verb Catalogue (130 Verbs, 19 Domains)

### 20.1 Domain Summary

| Domain | Verb Count | Behavior Mix | Description |
|--------|-----------|-------------|-------------|
| `kyc-case` | 10 | 8 CRUD + 2 Plugin | Case lifecycle |
| `kyc` | 1 | 1 Durable | Orchestrated case (BPMN-Lite) |
| `entity-workstream` | 9 | 8 CRUD + 1 Plugin | Per-entity work items |
| `ubo` | 24 | 12 CRUD + 12 Plugin | Ownership/control graph management |
| `ubo.registry` | 5 | 5 Plugin | UBO registry state machine |
| `evidence` | 5 | 5 Plugin | UBO evidence management |
| `skeleton` | 1 | 1 Plugin | 7-step skeleton build orchestration |
| `tollgate` | 10 | 4 CRUD + 6 Plugin | Decision gate engine |
| `case-screening` | 4 | 4 CRUD | Sanctions/PEP/adverse media |
| `doc-request` | 7 | 7 CRUD | Document collection |
| `red-flag` | 7 | 7 CRUD | Risk indicators |
| `request` | 8 | 8 Plugin | Outstanding request management |
| `board` | 9 | 8 CRUD + 1 Plugin | Board composition & control |
| `capital` | 9 | 6 CRUD + 3 Plugin | Share classes & holdings |
| `partnership` | 7 | 3 CRUD + 4 Plugin | Partnership structure |
| `trust` | 8 | 5 CRUD + 3 Plugin | Trust provisions & control |
| `coverage` | 1 | 1 Plugin | 4-prong coverage computation |
| `case-event` | 2 | 2 CRUD | Audit trail |
| `research.import-run` | 3 | 3 Plugin | Graph import provenance |
| **TOTAL** | **~130** | **~60 CRUD + ~70 Plugin** | |

### 20.2 Key Verb Details by Domain

#### kyc-case (10 verbs)

| Verb | Behavior | Description |
|------|----------|-------------|
| `create` | plugin | Create case with optional deal linkage; validates deal status; case_ref auto-generated by trigger |
| `update-status` | crud | Advance case through state machine |
| `escalate` | crud | Escalate case (STANDARD → SENIOR_COMPLIANCE → EXECUTIVE → BOARD) |
| `assign` | crud | Assign analyst and/or reviewer |
| `set-risk-rating` | crud | Set risk rating (LOW/MEDIUM/HIGH/VERY_HIGH/PROHIBITED) |
| `close` | plugin | Close case with validation; emits KYC_GATE_COMPLETED to deal_events when APPROVED with linked deal |
| `read` | crud | Read case details |
| `list-by-cbu` | crud | List cases for a CBU |
| `reopen` | crud | Reopen a closed case |
| `state` | plugin | Get full case state with workstreams and embedded awaiting requests |

#### ubo (24 verbs)

**Ownership:** add-ownership, update-ownership, end-ownership, delete-ownership
**Control:** add-control, end-control, delete-control
**Trust Roles:** add-trust-role, end-trust-role, delete-trust-role, delete-relationship
**Query:** list-owners, list-owned, list-ubos, list-by-subject, calculate, trace-chains, compute-chains
**Snapshots:** snapshot.capture, snapshot.diff
**Lifecycle:** mark-deceased, convergence-supersede, transfer-control, waive-verification, mark-terminus

#### skeleton (1 verb — 7-step orchestration)

| Verb | Behavior | Description |
|------|----------|-------------|
| `build` | plugin | Orchestrates: import-run.begin → graph.validate → ubo.compute-chains → coverage.compute → outreach.plan-generate → tollgate.evaluate-gate → import-run.complete |

#### research.import-run (3 verbs)

| Verb | Behavior | Description |
|------|----------|-------------|
| `begin` | plugin | Begin import run with optional case linkage; supports SKELETON_BUILD, INCREMENTAL, CORRECTION, FULL_REFRESH run kinds |
| `complete` | plugin | Mark import run as complete with counts |
| `supersede` | plugin | Supersede an import run — soft-ends edges, logs corrections, cascades re-derivation |

---

## 21. Database Functions & Triggers

### 21.1 kyc Schema Functions (31)

| Function | Purpose |
|----------|---------|
| `generate_case_ref()` | Auto-generate KYC-YYYY-NNNNNN case reference |
| `is_valid_case_transition()` | Validate case state machine transitions |
| `is_valid_workstream_transition()` | Validate workstream state machine transitions |
| `is_valid_doc_request_transition()` | Validate document request state transitions |
| `check_case_doc_completion()` | Check if all case documents are complete |
| `generate_doc_requests_from_threshold()` | Auto-generate doc requests from tollgate thresholds |
| `uuid_to_lock_id()` | Convert UUID to advisory lock integer |
| `fn_primary_governance_controller()` | Identify primary governance controller for entity |
| `fn_compute_control_links()` | Compute control links from ownership + board data |
| `fn_compute_economic_exposure()` | Compute economic exposure through chain |
| `fn_economic_exposure_summary()` | Summary of economic exposure |
| `fn_derive_ownership_snapshots()` | Derive ownership snapshot for an entity |
| `fn_bridge_bods_to_holdings()` | Bridge BODS statements to holdings records |
| `fn_bridge_gleif_fund_manager_to_board_rights()` | GLEIF fund manager → board appointment rights |
| `fn_bridge_manco_role_to_board_rights()` | ManCo role → board appointment rights |
| `fn_run_governance_bridges()` | Run all governance bridge functions |
| `fn_holder_control_position()` | Compute control position for a holder |
| `fn_share_class_supply_at()` | Share class supply as of date |
| `fn_diluted_supply_at()` | Diluted supply as of date |
| `fn_update_supply_timestamp()` | Update supply tracking timestamp |
| `sync_holding_to_ubo_relationship()` | Sync holdings changes to entity_relationships |
| `validate_investor_lifecycle_transition()` | Validate investor lifecycle state |
| `log_investor_lifecycle_change()` | Log investor lifecycle changes |
| `update_outstanding_request_timestamp()` | Update request timestamp |
| `update_workstream_blocked_days()` | Compute blocked days on status change |
| `update_fund_vehicle_timestamp()` | Update fund vehicle timestamp |
| `update_issuer_control_config_timestamp()` | Update issuer config timestamp |
| `update_role_profile_timestamp()` | Update role profile timestamp |
| `upsert_role_profile()` | Upsert investor role profile |
| `get_current_role_profile()` | Get current role profile |
| `get_role_profile_as_of()` | Get role profile as of date |

### 21.2 kyc Schema Triggers (12)

| Trigger | Table | Event | Purpose |
|---------|-------|-------|---------|
| `trg_case_ref` | cases | INSERT | Auto-generate case_ref via generate_case_ref() |
| `trg_workstream_blocked_days` | entity_workstreams | UPDATE | Track cumulative blocked days |
| `trg_sync_holding_to_ubo` | holdings | INSERT/UPDATE | Sync holdings → entity_relationships |
| `trg_validate_investor_lifecycle` | investors | UPDATE | Validate investor state transitions |
| `trg_log_investor_lifecycle` | investors | UPDATE | Log investor lifecycle changes |
| `trg_update_outstanding_request_ts` | outstanding_requests | UPDATE | Update timestamps |
| `trg_update_fund_vehicle_ts` | fund_vehicles | UPDATE | Update timestamps |
| `trg_update_issuer_control_ts` | issuer_control_config | UPDATE | Update timestamps |
| `trg_update_role_profile_ts` | investor_role_profiles | UPDATE | Update timestamps |
| `trg_update_supply_ts` | share_class_supply | INSERT/UPDATE | Update supply timestamps |
| `trg_check_case_doc_completion` | doc_requests | UPDATE | Check document completion on status change |
| `trg_generate_doc_requests` | tollgate_evaluations | INSERT | Auto-generate doc requests from tollgate gaps |

### 21.3 ob-poc Schema (UBO-Related Functions)

| Function | Purpose |
|----------|---------|
| `compute_ownership_chains()` | Compute full ownership chain to terminus |
| `check_ubo_completeness()` | Check if UBO identification is complete |
| `capture_ubo_snapshot()` | Capture point-in-time UBO state |
| `can_prove_ubo()` | Check if UBO can be proved with available evidence |
| `ubo_chain_as_of()` | Ownership chain as of specific date |
| `is_valid_ubo_transition()` | Validate UBO registry state transitions |
| `ownership_as_of()` | Ownership relationships as of date |
| `set_bods_interest_type()` | Auto-map control edge → BODS/GLEIF/PSC standards |
| `fn_manco_group_control_chain()` | ManCo group control chain analysis |

### 21.4 ob-poc Schema (UBO-Related Triggers)

| Trigger | Table | Event | Purpose |
|---------|-------|-------|---------|
| `trg_control_edges_set_standards` | control_edges | INSERT/UPDATE | Auto-map → bods_interest_type, gleif_relationship_type, psc_category |
| `trg_ubo_status_transition` | ubo_registry | UPDATE | Validate status transitions via is_valid_ubo_transition() |

---

## 22. Views

### 22.1 kyc Schema Views (10)

| View | Purpose |
|------|---------|
| `v_case_summary` | Case with workstream counts, completion %, risk distribution |
| `v_workstream_detail` | Workstream with entity name, coverage flags, blocking status |
| `v_pending_decisions` | Research decisions in PENDING/IN_PROGRESS status |
| `v_research_activity` | Research actions with anomaly/correction counts |
| `v_share_class_summary` | Share class with holder count, total issued, voting power |
| `v_capital_structure_extended` | Full capital structure with dilution instruments |
| `v_economic_edges_direct` | Direct economic edges from holdings and control links |
| `v_dilution_summary` | Dilution instrument impact summary |
| `v_fund_vehicle_summary` | Fund vehicle with compartment counts |
| `v_current_role_profiles` | Current investor role profiles |

### 22.2 ob-poc Schema Views (UBO-Related, 8)

| View | Purpose |
|------|---------|
| `v_entity_ubos` | Entities with their identified UBOs |
| `v_ubo_chain_summary` | UBO chain summaries with effective percentages |
| `v_ubo_coverage_gaps` | Entities with incomplete UBO identification |
| `v_control_edge_standards` | Control edges with BODS/GLEIF/PSC mappings |
| `v_bods_ownership_summary` | BODS-formatted ownership summary |
| `v_cbu_control_summary` | CBU control structure summary |
| `v_entity_relationship_current` | Current (non-ended) entity relationships |
| `v_ubo_snapshot_comparisons` | Snapshot comparison results |

---

## 23. Reference Data (ob_ref)

### 23.1 ob_ref.tollgate_definitions

| Column | Type | Description |
|--------|------|-------------|
| `tollgate_id` | varchar | PK (SKELETON_READY, EVIDENCE_COMPLETE, REVIEW_COMPLETE) |
| `display_name` | varchar | Human-readable name |
| `description` | text | Gate description |
| `applies_to` | varchar | CASE or WORKSTREAM |
| `required_status` | varchar | Case status required for evaluation |
| `default_thresholds` | jsonb | Threshold configuration |
| `override_permitted` | boolean | Whether overrides are allowed |
| `override_authority` | varchar | Required authority for override |
| `override_max_days` | integer | Max days an override is valid |

### 23.2 ob_ref.standards_mappings

Cross-reference between internal edge types and regulatory standards (BODS, GLEIF, PSC).

### 23.3 ob_ref.regulators

Reference data for regulatory bodies.

### 23.4 ob_ref.request_types

Reference data for outstanding request types.

### 23.5 ob_ref.role_types

Reference data for entity role types.

---

## 24. Schema Summary

### 24.1 Table Counts

| Schema | Tables | Views | Functions | Triggers | Total Objects |
|--------|--------|-------|-----------|----------|--------------|
| `kyc` | 37 | 10 | 31 | 12 | 90 |
| `ob-poc` (UBO-related) | ~28 | 8 | 11 | 3 | ~50 |
| `ob_ref` | 5 | — | — | — | 5 |
| **Total** | **~70** | **18** | **42** | **15** | **~145** |

### 24.2 kyc Schema Tables (37)

| Table | Row Count | Purpose |
|-------|-----------|---------|
| `cases` | 107 | KYC case records |
| `entity_workstreams` | 40 | Per-entity work items |
| `share_classes` | 69 | Corporate share class definitions |
| `share_class_supply` | 58 | Share supply tracking |
| `holdings` | 49 | Shareholding records |
| `special_rights` | 13 | Veto, drag-along, etc. |
| `issuance_events` | 8 | Share issuance history |
| `dilution_instruments` | 4 | Convertibles, options, warrants |
| `holding_control_links` | 4 | Holdings → control links |
| `screenings` | 2 | Sanctions/PEP/adverse media |
| `issuer_control_config` | 1 | Issuer control configuration |
| `case_events` | 0 | Case audit trail |
| `case_import_runs` | 0 | Case ↔ import run links |
| `dilution_exercise_events` | 0 | Dilution exercise tracking |
| `doc_requests` | 0 | Document requests |
| `fund_compartments` | 0 | Fund compartment structure |
| `fund_vehicles` | 0 | Fund vehicle definitions |
| `investor_role_profiles` | 0 | Investor role profiles |
| `investors` | 0 | Investor records |
| `movements` | 0 | Share movements |
| `outreach_items` | 0 | Outreach plan items |
| `outreach_plans` | 0 | Outreach plans |
| `outreach_requests` | 0 | Outreach request tracking |
| `outstanding_requests` | 0 | Outstanding request tracker |
| `ownership_reconciliation_findings` | 0 | Reconciliation findings |
| `ownership_reconciliation_runs` | 0 | Reconciliation runs |
| `ownership_snapshots` | 0 | Point-in-time ownership captures |
| `red_flags` | 0 | Risk indicators |
| `research_actions` | 0 | Research action log |
| `research_anomalies` | 0 | Detected anomalies |
| `research_corrections` | 0 | Applied corrections |
| `research_decisions` | 0 | Research decisions |
| `share_class_identifiers` | 0 | ISIN, SEDOL references |
| `tollgate_evaluations` | 0 | Tollgate evaluation records |
| `ubo_determination_runs` | 0 | UBO chain computation runs |
| `ubo_evidence` | 0 | UBO evidence tracking |
| `ubo_registry` | 0 | Per-case UBO registry |

### 24.3 Verb Totals

| Metric | Count |
|--------|-------|
| Total verbs | ~130 |
| CRUD verbs | ~60 |
| Plugin verbs | ~70 |
| Durable verbs | 1 |
| Domains | 19 |

### 24.4 Key Relationships

```
kyc.cases ──→ kyc.entity_workstreams ──→ kyc.screenings
    │              │                         kyc.doc_requests
    │              │                         kyc.red_flags
    │              └──→ kyc.ubo_registry ──→ kyc.ubo_evidence
    │
    ├──→ kyc.ubo_determination_runs (output_snapshot, chains_snapshot, coverage_snapshot)
    │
    ├──→ kyc.outreach_plans ──→ kyc.outreach_items
    │
    ├──→ kyc.tollgate_evaluations
    │
    ├──→ kyc.outstanding_requests
    │
    ├──→ kyc.research_decisions ──→ kyc.research_actions ──→ kyc.research_anomalies
    │                                                    └──→ kyc.research_corrections
    │
    ├──→ kyc.case_events
    │
    └──→ ob-poc.deals (via deal_id — Deal–KYC bridge)

ob-poc.entity_relationships ←── Ownership graph (33 cols, BODS share ranges)
    │
    └──→ Used by skeleton.build for DFS chain traversal

ob-poc.control_edges ←── Control graph (21 cols, auto-mapped standards)
    │
    └──→ trg_control_edges_set_standards → bods/gleif/psc auto-mapping

ob-poc.ubo_registry ←── Operational UBO registry (30 cols, supersession chains)

ob_ref.tollgate_definitions ←── 3 gate definitions with configurable thresholds
```

---

*End of peer review paper.*
