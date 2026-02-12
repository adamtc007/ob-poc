# KYC/UBO Implementation TODO — v0.5

**Source spec:** `KYC_UBO_ARCHITECTURE_v0.5.md`  
**Execution:** Claude Code (Zed integration)  
**Date:** 2026-02-12  

---

## Execution Directives

> **CRITICAL:** This TODO has 4 phases with 45+ tasks. After completing each numbered task, IMMEDIATELY proceed to the next. Do NOT stop after completing a phase — continue to the next phase. Report progress as `[X/N]` at each task boundary.
>
> **E-INVARIANT:** At every phase boundary, run `cargo test` and report pass count. All existing tests (~160) must continue to pass. New tests must be added per task. A phase is not complete until its test gate passes.
>
> **CONTINUATION RULE:** If you complete Phase N and there are more phases, you MUST say "Phase N complete. → IMMEDIATELY proceeding to Phase N+1" and continue. Do NOT ask "shall I continue?" — the answer is always yes.

---

## Phase 0 — Schema Foundations (prerequisite)

**Goal:** Create all new tables and alter existing ones. No Rust code yet. Pure SQL migrations.

**Test gate:** `cargo test` passes (no regressions). Tables exist in test DB.

### 0.1 — Create `"ob-poc".graph_import_runs` table

**File:** `rust/migrations/YYYYMMDD_graph_import_runs.sql`

Create per spec §4.3:
- All columns from DDL in spec
- 3 indexes: `idx_gir_scope_root`, `idx_gir_source_ref`, `idx_gir_status`
- `status` CHECK constraint: `ACTIVE, SUPERSEDED, ROLLED_BACK, PARTIAL`
- Self-referential FK on `superseded_by`
- **No `payload_store_uri`, no `response_meta` JSONB** — spec explicitly defers these

→ IMMEDIATELY proceed to 0.2

### 0.2 — Create `kyc.case_import_runs` join table

**File:** same migration or next sequential

Create per spec §4.3:
- `(case_id, run_id)` composite PK
- FK to `kyc.cases(case_id)` and `"ob-poc".graph_import_runs(run_id)`
- Optional FK to `kyc.research_decisions(decision_id)`
- Index on `case_id`

→ IMMEDIATELY proceed to 0.3

### 0.3 — Alter `entity_relationships` for provenance + natural key

**File:** same migration or next sequential

Add columns to `"ob-poc".entity_relationships`:
- `import_run_id UUID REFERENCES "ob-poc".graph_import_runs(run_id)` — nullable (existing edges won't have one)
- `evidence_hint TEXT` — nullable
- Ensure `source VARCHAR(30) NOT NULL DEFAULT 'MANUAL'` exists
- Ensure `source_ref TEXT` exists
- Change `confidence` to `VARCHAR(10) NOT NULL` — **remove any DEFAULT 'HIGH'** if present. Existing rows: backfill as 'MEDIUM'.

Add constraint:
```sql
ALTER TABLE "ob-poc".entity_relationships
ADD CONSTRAINT uq_entity_rel_natural_key
UNIQUE (from_entity_id, to_entity_id, relationship_type, effective_from);
```

**Note:** If existing data violates this unique constraint (duplicate `(from, to, type, effective_from)` tuples), deduplicate first: keep the most recent `created_at` row, soft-end others.

Add index:
```sql
CREATE INDEX idx_er_import_run ON "ob-poc".entity_relationships(import_run_id);
```

→ IMMEDIATELY proceed to 0.4

### 0.4 — Create `kyc.cases` table

**File:** next sequential migration

Create per spec §5.2. Include:
- All columns from DDL
- CHECK constraint on `status`
- UNIQUE on `case_ref`

→ IMMEDIATELY proceed to 0.5

### 0.5 — Create `kyc.entity_workstreams` table

**File:** same migration

Create per spec §5.2. Include:
- All columns from DDL
- UNIQUE on `(case_id, entity_id)`
- FK to `kyc.cases` and `"ob-poc".entities`

→ IMMEDIATELY proceed to 0.6

### 0.6 — Create `kyc.ubo_determination_runs` table

**File:** same migration

Create per spec §5.4. Include `coverage_snapshot JSONB` column per §2A.3.

→ IMMEDIATELY proceed to 0.7

### 0.7 — Create `kyc.ubo_registry` table

**File:** same migration

Create per spec §5.3. Include:
- All columns, all CHECK constraints
- FK to `kyc.cases`, `kyc.entity_workstreams`, `"ob-poc".entities`, `kyc.ubo_determination_runs`

→ IMMEDIATELY proceed to 0.8

### 0.8 — Create `kyc.ubo_evidence` table

**File:** same migration

Create per spec §5.3. Include:
- All columns, CHECK constraint on `status`
- FK to `kyc.ubo_registry`
- `document_id UUID` — **logical FK, no hard constraint** (documents are in ob-poc schema, may predate case). See spec §2A.7.

→ IMMEDIATELY proceed to 0.9

### 0.9 — Create `kyc.outreach_plans` and `kyc.outreach_items` tables

**File:** same migration

Create per spec §4.5.

→ IMMEDIATELY proceed to 0.10

### 0.10 — Create `kyc.tollgate_evaluations` table

**File:** same migration

Create per spec §5.5. Also seed `ob_ref.tollgate_definitions` with at least `SKELETON_READY` definition per spec §4.6.

→ IMMEDIATELY proceed to 0.11

### 0.11 — Run migration and verify

Run all new migrations against test DB. Run `cargo test`. All existing tests must pass.

**Verification queries:**
```sql
SELECT count(*) FROM information_schema.tables 
WHERE table_schema IN ('ob-poc', 'kyc', 'ob_ref')
AND table_name IN ('graph_import_runs', 'case_import_runs', 'cases', 
    'entity_workstreams', 'ubo_determination_runs', 'ubo_registry', 
    'ubo_evidence', 'outreach_plans', 'outreach_items', 'tollgate_evaluations');
-- Expected: 10
```

→ Phase 0 complete. → IMMEDIATELY proceed to Phase 1.

---

## Phase 1 — Core Verbs: Import Runs + Case + UBO Registry

**Goal:** Implement the verb handlers and YAML configs for import runs, case lifecycle, and UBO registry. These are the building blocks everything else depends on.

**Test gate:** `cargo test` passes. Each new verb has at least one happy-path test + one error-path test.

### 1.1 — Import Run Verbs (3 verbs)

**Files:**
- YAML: extend `config/verbs/research/workflow.yaml` with `import-run` domain
- Handler: new file `rust/src/dsl/custom_ops/import_run_ops.rs`
- Register in `rust/src/dsl/custom_ops/mod.rs`

**Verbs:**

| Verb | Handler | Behaviour |
|------|---------|-----------|
| `research.import-run.begin` | `ImportRunBeginOp` | INSERT into `graph_import_runs` + INSERT into `case_import_runs` (if `:case-id` provided). Return `run_id`. |
| `research.import-run.complete` | `ImportRunCompleteOp` | UPDATE `graph_import_runs` SET `status`, update counts (`entities_created`, etc.). |
| `research.import-run.supersede` | `ImportRunSupersedeOp` | UPDATE run SET `status='SUPERSEDED'`, `superseded_by`, `superseded_reason`. UPDATE all `entity_relationships` WHERE `import_run_id = run_id` SET `effective_to = NOW()`. Log `research.workflow.record-correction` for linked cases. |

**Args for `begin`:**
- `:run-kind` (required, VARCHAR(30))
- `:scope-root-entity-id` (required, UUID)
- `:as-of` (optional, DATE)
- `:source` (required, VARCHAR(30))
- `:source-ref` (optional, TEXT)
- `:source-query` (optional, TEXT)
- `:case-id` (optional, UUID — if provided, creates case_import_runs link)
- `:decision-id` (optional, UUID)

**Tests:**
- Begin creates row in `graph_import_runs`
- Begin with `:case-id` also creates `case_import_runs` row
- Complete updates status and counts
- Supersede soft-ends all edges from that run
- Supersede creates correction record for linked cases
- Begin is idempotent (same params → same run, not duplicate)

→ IMMEDIATELY proceed to 1.2

### 1.2 — Case Lifecycle Verbs (6 verbs)

**Files:**
- YAML: `config/verbs/kyc/kyc-case.yaml` (create or update)
- Handler: new file `rust/src/dsl/custom_ops/kyc_case_ops.rs`
- Register in mod.rs

**Verbs:** `kyc.create-case`, `kyc.update-status`, `kyc.assign-analyst`, `kyc.assign-reviewer`, `kyc.escalate`, `kyc.close-case`

**Key behaviours:**
- `create-case`: INSERT into `kyc.cases`. Generate `case_ref` (format: `KYC-{YYYY}-{SEQ}`). Return `case_id`.
- `update-status`: Validate transition per §9.1 state machine. Reject invalid transitions. Set `updated_at`.
- `close-case`: Set `closed_at`, validate terminal state (APPROVED, REJECTED, BLOCKED, WITHDRAWN, DO_NOT_ONBOARD).

**Tests:**
- Create case returns valid UUID and case_ref
- Valid status transitions succeed
- Invalid transitions rejected (e.g. INTAKE → REVIEW)
- Assign sets analyst/reviewer IDs
- Close-case sets closed_at timestamp

→ IMMEDIATELY proceed to 1.3

### 1.3 — Workstream Verbs (3 verbs)

**Files:**
- YAML: `config/verbs/kyc/entity-workstream.yaml`
- Handler: `rust/src/dsl/custom_ops/kyc_workstream_ops.rs`

**Verbs:** `kyc.add-workstream`, `kyc.workstream.update-status`, `kyc.workstream.close`

**Key behaviours:**
- `add-workstream`: INSERT with `UNIQUE(case_id, entity_id)` — idempotent via ON CONFLICT DO NOTHING.
- `update-status`: Validate workstream state machine per spec §9 (OPEN → IN_PROGRESS → BLOCKED → READY_FOR_REVIEW → CLOSED).

**Tests:**
- Add workstream is idempotent
- Status transitions validated
- Close sets appropriate flags

→ IMMEDIATELY proceed to 1.4

### 1.4 — UBO Registry Verbs (5 verbs)

**Files:**
- YAML: extend `config/verbs/ubo.yaml`
- Handler: `rust/src/dsl/custom_ops/ubo_registry_ops.rs`

**Verbs:** `ubo.registry.promote`, `ubo.registry.advance`, `ubo.registry.waive`, `ubo.registry.reject`, `ubo.registry.expire`

**Key behaviours:**
- `promote`: CANDIDATE → IDENTIFIED. Sets `identified_at`.
- `advance`: General-purpose transition validator. IDENTIFIED → PROVABLE → PROVED → REVIEWED → APPROVED. Sets corresponding timestamp.
- `waive`: Any → WAIVED. Requires `:reason` and `:authority`. Sets `waiver_reason`, `waiver_authority`, optional `waiver_expiry`.
- `reject` / `expire`: Terminal states.

**State machine validation:** Reject any transition not in the allowed graph (§9.2).

**Tests:**
- Full happy path: CANDIDATE → IDENTIFIED → PROVABLE → PROVED → REVIEWED → APPROVED
- Waive from any non-terminal state
- Reject invalid transition (e.g. CANDIDATE → PROVED)
- Timestamps set exactly once per transition

→ IMMEDIATELY proceed to 1.5

### 1.5 — Evidence Verbs (5 verbs)

**Files:**
- YAML: new `config/verbs/kyc/evidence.yaml`
- Handler: `rust/src/dsl/custom_ops/evidence_ops.rs`

**Verbs:** `evidence.require`, `evidence.link`, `evidence.verify`, `evidence.reject`, `evidence.waive`

**Key behaviours per §2A.2:**
- `require`: INSERT `ubo_evidence` with status=REQUIRED. Args: `:ubo-id`, `:type` (evidence_type).
- `link`: UPDATE SET `document_id`, `status='RECEIVED'`. Validate document exists in document system (query `document_entity_links`).
- `verify`: UPDATE SET `status='VERIFIED'`, `verified_at=NOW()`, `verified_by`. Requires `:evidence-id`, `:verifier-id`.
- `reject`: UPDATE SET `status='REJECTED'`, clear `document_id`. Evidence returns to REQUESTED state for re-submission.
- `waive`: UPDATE SET `status='WAIVED'`, `notes=reason`.

**Tests:**
- Require creates REQUIRED row
- Link updates document_id and status
- Verify sets timestamp and verifier
- Reject clears document and requeues
- Waive records reason
- Link with non-existent document_id fails gracefully

→ Phase 1 complete. → IMMEDIATELY proceed to Phase 2.

---

## Phase 2 — Skeleton Build Pipeline

**Goal:** Implement the derivation verbs, outreach generation, tollgate evaluation, and the skeleton build template verb.

**Test gate:** `cargo test` passes. Skeleton build runs end-to-end on a golden fixture.

### 2.1 — Edge Upsert: Implement End-and-Insert Semantics

**File:** Update existing `edge.yaml` handler in `rust/src/dsl/custom_ops/`

Update `edge.upsert` per §2A.1:
1. Check natural key `(from, to, type, effective_from)`.
2. If exact match with same `(source, percentage, import_run_id)` → no-op.
3. If natural key match with different attributes → end old edge, insert new.
4. Accept optional `:import-run-id`, `:confidence`, `:evidence-hint` args.
5. **Confidence is required if `:source` is provided** — no default.

**Tests:**
- Idempotent: same upsert twice → one row
- Changed percentage: old edge ended, new inserted
- Multi-source conflict: two edges coexist, both active
- Missing confidence with explicit source → error

→ IMMEDIATELY proceed to 2.2

### 2.2 — Graph Validate Verb

**File:** new `rust/src/dsl/custom_ops/graph_validate_ops.rs`
**YAML:** `config/verbs/graph.yaml` (extend)

Implements §6.4 validation checks:
1. Cycle detection (Tarjan's SCC on ownership subgraph)
2. Missing percentages where expected
3. Supply inconsistency (> 100% per share class)
4. Terminus integrity
5. Source conflicts (same pair, different %, different source)
6. Orphan entities (import-created with no edges)

**Output:** INSERT anomalies into `kyc.research_anomalies` with `case_id`, severity, anomaly_type.

**Tests:**
- Fixture with known cycle → detected
- Fixture with 110% ownership → flagged
- Clean fixture → no anomalies

→ IMMEDIATELY proceed to 2.3

### 2.3 — UBO Compute Chains Verb

**File:** update existing `ubo.yaml` handler or create `rust/src/dsl/custom_ops/ubo_compute_ops.rs`

Implements §6.1 algorithm:
- Load ownership edges for `(subject_entity_id, as_of)` temporal window
- Build in-memory directed graph
- Traverse upward, multiply percentages
- Check terminus flags
- Output: candidate list with `(person_entity_id, cumulative_pct, chain_path, prong)`
- INSERT into `kyc.ubo_determination_runs` with `output_snapshot` JSONB and `chains_snapshot` JSONB

**Binding:** Results available via `(... :as @candidates)` session variable.

**Tests:**
- Simple chain: A (100%) → B (60%) → Person X = UBO at 60%
- Below threshold: A (20%) → B (20%) → Person Y = 4%, not UBO
- Terminus: stops at regulated entity
- Cycle: detected and reported, traversal continues on non-cycled paths
- Performance: < 100ms for 10-level chain with < 500 entities

→ IMMEDIATELY proceed to 2.4

### 2.4 — Coverage Compute Verb

**File:** new `rust/src/dsl/custom_ops/coverage_compute_ops.rs`
**YAML:** new `config/verbs/kyc/coverage.yaml`

Implements §6.3 + §2A.3:
- For each prong, count edges and evidenced edges
- Generate gap list with stable `"{edge_id}:{gap_type}"` identifiers
- Determine `blocking_at_gate` by checking against tollgate thresholds
- Persist `CoverageResult` as JSONB in `determination_runs.coverage_snapshot`

**Binding:** Results available via `(... :as @coverage)` session variable.

**Tests:**
- 0% coverage: no evidence → all gaps
- 100% coverage: all edges evidenced → no gaps
- Gap identifiers are stable across re-computations (same edge, same gap type → same ID)

→ IMMEDIATELY proceed to 2.5

### 2.5 — Outreach Plan Generate Verb

**File:** new `rust/src/dsl/custom_ops/outreach_ops.rs`
**YAML:** new `config/verbs/kyc/outreach.yaml`

Implements §4.5:
- Takes `@coverage` ref (gap list) + `@candidates` ref
- Groups gaps by prong
- Maps gap types to document request types per §2A.2 mapping table
- Bundles by entity (max 8 items per entity)
- INSERT into `kyc.outreach_plans` + `kyc.outreach_items`
- Each item's `closes_gap_ref` = the gap ID from coverage

**Tests:**
- 3 gaps → 3 outreach items
- Bundling: 10 gaps for one entity → capped at 8
- Gap-to-doc-type mapping is correct
- Plan status starts as DRAFT

→ IMMEDIATELY proceed to 2.6

### 2.6 — Tollgate Evaluate Verb

**File:** new `rust/src/dsl/custom_ops/tollgate_ops.rs`
**YAML:** new `config/verbs/kyc/tollgate.yaml`

Implements §2A.4:
1. Load `ob_ref.tollgate_definitions` by `:gate` arg
2. Compute pass/fail against current case state:
   - `ownership_coverage_pct` from latest coverage snapshot
   - `governance_controller_identified` from control derivation
   - `high_severity_conflicts_resolved` from anomalies
   - `outreach_plan_items_max` from outreach plan
   - `cycle_anomalies_acknowledged` from anomalies
   - `minimum_sources_consulted` from import runs count
3. INSERT into `kyc.tollgate_evaluations`
4. Return `{passed: bool, evaluation_detail: {...}, gaps: [...]}`

**Tests:**
- All thresholds met → passed
- One threshold failed → not passed, gap identified
- SKELETON_READY gate with default thresholds

→ IMMEDIATELY proceed to 2.7

### 2.7 — Skeleton Build Template Verb

**File:** `config/verbs/kyc/skeleton-build.yaml`

Create per spec §7.7. This is a DSL template verb (macro expansion).

Verify the DSL session pipeline correctly:
1. Expands template args (`$case-id`, `$subject-id`, etc.)
2. Handles `(set @symbol ...)` bindings
3. Handles inline expressions `(entity.get-lei $subject-id)`
4. Expands sequential verb calls

**Test (integration):**

Create a golden fixture:
- Entity hierarchy: ManCo → Sub1 (70%) → Sub2 (50%) → Person A
- ManCo has LEI in entity_identifiers
- Sub1 has company_number in entity_identifiers
- Mock GLEIF and Companies House adapters that return known data

Run `kyc.skeleton.build` with fixture. Verify:
- 2 import runs created (one GLEIF, one CH)
- Edges created with `import_run_id` set
- Each edge has explicit `confidence` (not defaulted)
- `graph.validate` ran (check for anomaly records)
- `ubo.compute-chains` ran (check determination_runs)
- `coverage.compute` ran (check coverage_snapshot)
- `outreach.plan.generate` ran (check outreach_plans + items)
- `tollgate.evaluate` ran (check tollgate_evaluations)
- All in correct order

→ Phase 2 complete. → IMMEDIATELY proceed to Phase 3.

---

## Phase 3 — Derivation Engine Completion + Agent Verb Classification

**Goal:** Implement remaining derivation verbs, snapshot/diff, governance controllers, and agent verb YAML metadata.

**Test gate:** `cargo test` passes. Case can go from INTAKE → APPROVED with all gates evaluated.

### 3.1 — Control Compute Controllers Verb

**File:** new `rust/src/dsl/custom_ops/control_compute_ops.rs`

Implements §6.2:
- Load CONTROL and MANAGEMENT edges
- Apply priority ordering from config
- Bridge to `cbu_entity_roles` for officer data
- Output: `(controller_entity_id, control_type, basis_description)`

**Tests:**
- Board appointment power outranks management contract
- Veto/golden share detected from special_rights
- Controller linked to entity via cbu_entity_roles

→ IMMEDIATELY proceed to 3.2

### 3.2 — UBO Snapshot Capture + Diff Verbs

**File:** extend UBO ops

**Verbs:** `ubo.snapshot.capture`, `ubo.snapshot.diff`

- `capture`: Serialize current determination results to `output_snapshot` + `chains_snapshot` JSONB. Record `code_hash`, `config_version`.
- `diff`: Compare two snapshots by `run_id`. Output: added candidates, removed candidates, changed percentages, changed chains.

**Tests:**
- Capture produces reproducible JSONB (sorted keys, deterministic)
- Diff detects added/removed/changed candidates
- Diff on identical snapshots → empty diff

→ IMMEDIATELY proceed to 3.3

### 3.3 — Agent Verb YAML: Add Classification Metadata

**File:** `config/verbs/agent/agent.yaml`

Add to each verb definition:

```yaml
agent.start:
  category: agent_control
  context: interactive_only
  side_effects: runtime_state
  # ... existing fields
  
agent.resolve-gaps:
  category: agent_task
  context: scripted_ok
  side_effects: facts_only
  # ... existing fields
```

Apply per spec §7.5:
- All 6 control verbs: `agent_control / interactive_only / runtime_state`
- All 4 task verbs: `agent_task / scripted_ok / facts_only` (except `agent.screen-entities` which is `mixed`)

**Tests:**
- YAML parses correctly with new fields
- Verb registry loads classification metadata

→ IMMEDIATELY proceed to 3.4

### 3.4 — Agent Verb Linter Rule

**File:** extend DSL linter (create if not exists: `rust/src/dsl/linting/`)

Add rule: reject any verb with `context: interactive_only` appearing inside:
- Template verb body (behaviour: template)
- BPMN-Lite task definition
- Non-interactive execution mode

**Tests:**
- Template containing `agent.start` → lint error
- Template containing `agent.resolve-gaps` → OK
- Interactive REPL session calling `agent.start` → OK

→ IMMEDIATELY proceed to 3.5

### 3.5 — Research Generic Normalize Verb

**File:** extend `config/verbs/research/generic.yaml` + handler

**Verb:** `research.generic.normalize`

Takes `:source-name` + raw payload (TEXT or JSONB). Produces:
- Canonical JSON (sorted keys, schema-validated)
- `normalized_hash` (SHA-256)
- Returns `normalized_payload_ref` for downstream `import-*` verbs

For v0.5 implementation: the handler validates the payload against a schema registry (config-driven) and computes the hash. LLM integration deferred.

**Tests:**
- Valid payload → normalized + hashed
- Invalid payload → schema validation error
- Same payload → same hash (deterministic)

→ IMMEDIATELY proceed to 3.6

### 3.6 — End-to-End Integration Test: Full Case Lifecycle

**File:** `rust/tests/integration/kyc_full_lifecycle.rs`

Execute the case pack runbook from spec §13:
1. `kyc.create-case` → DISCOVERY
2. `kyc.skeleton.build` (with golden fixture from 2.7)
3. SKELETON_READY gate passes (or assert specific gaps if fixture is incomplete)
4. `kyc.update-status` → ASSESSMENT
5. `ubo.registry.promote` candidates
6. `evidence.require` + `evidence.link` + `evidence.verify`
7. `tollgate.evaluate EVIDENCE_COMPLETE`
8. `kyc.update-status` → REVIEW
9. `kyc.assign-reviewer`
10. `ubo.registry.advance` → REVIEWED → APPROVED
11. `tollgate.evaluate REVIEW_COMPLETE`
12. `kyc.close-case` → APPROVED

**Assert:** All intermediate state machines respected, all timestamps set, all tollgate evaluations persisted.

→ Phase 3 complete. → IMMEDIATELY proceed to Phase 4.

---

## Phase 4 — Deal Integration + Rollback Hardening

**Goal:** Wire KYC to Deal lifecycle, implement import run rollback end-to-end, harden research correction workflow.

**Test gate:** `cargo test` passes. Deal → Onboarding → KYC → Approved end-to-end.

### 4.1 — Wire `kyc.create-case` to Accept `deal_id`

**File:** Update case creation handler

`kyc.create-case` already has `:deal-id` as optional arg (spec §5.2). Ensure:
- FK `kyc.cases.deal_id REFERENCES "ob-poc".deals(deal_id)` is enforced
- If deal_id provided, validate deal exists and is in AGREED or later status
- `client_group_id` can be inferred from deal's client group if not explicitly provided

**Test:**
- Create case with deal_id → linked
- Create case with invalid deal_id → error
- Create case without deal_id → works (periodic review case)

→ IMMEDIATELY proceed to 4.2

### 4.2 — Import Run Rollback: End-to-End

**File:** Extend `ImportRunSupersedeOp`

Implement full rollback chain:
1. `research.import-run.supersede :run-id X :reason "incorrect hierarchy" :superseded-by Y`
2. All edges with `import_run_id = X` get `effective_to = NOW()`
3. For each case linked via `case_import_runs`:
   - Log `research.workflow.record-correction`
   - Trigger re-derivation: `ubo.compute-chains` + `coverage.compute`
   - Generate updated outreach plan if coverage changed

**Test (golden fixture: bad import rollback):**
1. Run skeleton build with golden fixture
2. Verify candidates computed
3. Supersede one import run (e.g. GLEIF was wrong)
4. Verify: edges soft-ended, correction logged
5. Re-derive: candidates updated, coverage recalculated
6. Outreach plan regenerated with new gaps (if any)
7. Previously computed tollgate result is now stale (new evaluation needed)

→ IMMEDIATELY proceed to 4.3

### 4.3 — Research Correction Audit Trail

**File:** Extend `research/workflow.yaml` handlers

Ensure `research.workflow.record-correction` captures:
- Which run was superseded
- Why (reason text)
- Which cases were affected
- What changed (diff of before/after candidate sets)

Ensure `research.workflow.audit-trail` can query the full decision + correction history for a case.

**Test:**
- Record correction → persisted
- Audit trail query returns ordered history (decisions + corrections)

→ IMMEDIATELY proceed to 4.4

### 4.4 — Deal Gate Completion Event

**File:** new handler or extend case close handler

When `kyc.close-case` sets status to APPROVED:
- If `deal_id` is set, emit a gate completion event to the deal lifecycle
- This event should mark the KYC gate as passed on the associated deal

**Note:** The exact deal gate mechanism depends on existing deal lifecycle implementation. If `deal_gates` table exists, INSERT/UPDATE the KYC gate row.

**Test:**
- Close case with deal → deal gate updated
- Close case without deal → no deal gate interaction

→ IMMEDIATELY proceed to 4.5

### 4.5 — Final Integration Test: Deal → KYC → Approved

**File:** `rust/tests/integration/deal_to_kyc_lifecycle.rs`

1. Create a deal (or use existing fixture)
2. `kyc.create-case :deal-id $DEAL`
3. Full skeleton build + assessment + review lifecycle
4. Case approved → deal gate completed
5. Assert deal reflects KYC approval

→ Phase 4 complete. All phases done.

---

## Summary: Task Count by Phase

| Phase | Tasks | Description |
|-------|-------|-------------|
| 0 | 11 | Schema migrations (SQL only) |
| 1 | 5 | Core verbs: import runs, case, workstream, UBO registry, evidence |
| 2 | 7 | Skeleton build pipeline: edge upsert, validate, compute chains, coverage, outreach, tollgate, template |
| 3 | 6 | Derivation completion: controllers, snapshots, agent classification, normalize, integration test |
| 4 | 5 | Deal integration: deal wiring, rollback, correction audit, gate events, final integration |
| **Total** | **34** | |

## Files Created/Modified (Expected)

### New Files
- `rust/migrations/YYYYMMDD_kyc_ubo_schema.sql` (or split across multiple)
- `rust/src/dsl/custom_ops/import_run_ops.rs`
- `rust/src/dsl/custom_ops/kyc_case_ops.rs`
- `rust/src/dsl/custom_ops/kyc_workstream_ops.rs`
- `rust/src/dsl/custom_ops/ubo_registry_ops.rs`
- `rust/src/dsl/custom_ops/evidence_ops.rs`
- `rust/src/dsl/custom_ops/graph_validate_ops.rs`
- `rust/src/dsl/custom_ops/ubo_compute_ops.rs`
- `rust/src/dsl/custom_ops/coverage_compute_ops.rs`
- `rust/src/dsl/custom_ops/outreach_ops.rs`
- `rust/src/dsl/custom_ops/tollgate_ops.rs`
- `rust/src/dsl/custom_ops/control_compute_ops.rs`
- `rust/src/dsl/linting/` (new module)
- `config/verbs/kyc/kyc-case.yaml`
- `config/verbs/kyc/entity-workstream.yaml`
- `config/verbs/kyc/evidence.yaml`
- `config/verbs/kyc/coverage.yaml`
- `config/verbs/kyc/outreach.yaml`
- `config/verbs/kyc/tollgate.yaml`
- `config/verbs/kyc/skeleton-build.yaml`
- `rust/tests/integration/kyc_full_lifecycle.rs`
- `rust/tests/integration/deal_to_kyc_lifecycle.rs`

### Modified Files
- `config/verbs/research/workflow.yaml` (import-run verbs added)
- `config/verbs/research/generic.yaml` (normalize verb added)
- `config/verbs/ubo.yaml` (registry + snapshot verbs)
- `config/verbs/agent/agent.yaml` (classification metadata)
- `config/verbs/edge.yaml` (import_run_id + confidence args)
- `config/verbs/graph.yaml` (validate verb)
- `rust/src/dsl/custom_ops/mod.rs` (register all new ops)
- Existing edge upsert handler (end-and-insert semantics)
