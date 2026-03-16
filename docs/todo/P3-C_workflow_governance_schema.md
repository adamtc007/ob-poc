# P3-C: Structural Integrity Audit — Workflow, Governance & Audit Tables

> **Reviewed:** 2026-03-16
> **Scope:** 19 table families across workflow orchestration, BPMN integration, agent telemetry, learning pipeline, DSL sessions, and document governance
> **Source:** `migrations/023`, `032`, `037`, `039`, `040`, `043`, `049`, `050`, `073`, `076`, `087`, `088`, `103`, `117`, `123` + `master-schema.sql`

---

## Severity Legend

| Tag | Meaning |
|-----|---------|
| **CLEAN** | No issues found, well-designed |
| **MINOR** | Cosmetic or low-risk inconsistency |
| **FLAG** | Structural gap that could cause operational issues |
| **CRITICAL** | Data integrity risk or correctness concern |

---

## 1. Per-Table Scorecards

### 1.1 `"ob-poc".workflow_pending_tasks`

| Aspect | Score | Notes |
|--------|-------|-------|
| PK | CLEAN | UUID PK (`task_id`) |
| FK | CLEAN | FK to `documents(document_id)`, FK to workflow instances |
| Indexes | CLEAN | Status + created_at index for queue processing |
| Constraints | CLEAN | CHECK on status, CHECK on task_type |
| Naming | CLEAN | Consistent snake_case |
| Temporals | CLEAN | `created_at`, `completed_at` |

**Verdict: CLEAN**

---

### 1.2 `"ob-poc".task_result_queue`

| Aspect | Score | Notes |
|--------|-------|-------|
| PK | MINOR | BIGSERIAL PK — works for ephemeral queue but diverges from UUID convention elsewhere |
| FK | FLAG | `task_id` has no FK to `workflow_pending_tasks` — intentional for external-origin tasks but undocumented in constraints |
| Indexes | CLEAN | Good partial index for unprocessed rows (`WHERE processed_at IS NULL`) |
| Constraints | CLEAN | UNIQUE on `(task_id, idempotency_key)` prevents duplicate ingest |
| Naming | CLEAN | Consistent |
| Temporals | CLEAN | `received_at`, `processed_at` |

**Verdict: MINOR** — BIGSERIAL PK is acceptable for ephemeral queue semantics. The missing FK on `task_id` is justified by the external-origin design but should be documented with a COMMENT.

---

### 1.3 `"ob-poc".task_result_dlq`

| Aspect | Score | Notes |
|--------|-------|-------|
| PK | MINOR | BIGSERIAL PK — consistent with `task_result_queue` |
| FK | FLAG | No FK to `task_result_queue` — rows are moved, not referenced |
| Indexes | CLEAN | `failure_count` index for retry logic |
| Constraints | CLEAN | `failure_count > 0` CHECK |
| Naming | CLEAN | |
| Temporals | CLEAN | `failed_at`, `last_retry_at` |

**Verdict: MINOR** — DLQ design is standard. No FK is correct since rows are moved between tables.

---

### 1.4 `"ob-poc".workflow_task_events`

| Aspect | Score | Notes |
|--------|-------|-------|
| PK | CLEAN | UUID PK (`event_id`) |
| FK | CLEAN | FK to `workflow_pending_tasks(task_id)` |
| Indexes | CLEAN | `task_id + timestamp` composite index |
| Constraints | CLEAN | CHECK on `event_type` |
| Naming | CLEAN | |
| Temporals | CLEAN | `timestamp` column |
| Append-only | FLAG | No schema-level enforcement of append-only — no trigger preventing UPDATE/DELETE |

**Verdict: FLAG** — Audit trail table should have a trigger or rule preventing UPDATE/DELETE for schema-level append-only guarantee.

---

### 1.5 `"ob-poc".document_requirements`

| Aspect | Score | Notes |
|--------|-------|-------|
| PK | CLEAN | UUID PK (`requirement_id`) |
| FK | CLEAN | FK to entities |
| Indexes | CLEAN | Composite indexes for entity + doc_type lookup |
| Constraints | CLEAN | CHECK on `status` (8 states), `UNIQUE NULLS NOT DISTINCT` on business key |
| Naming | CLEAN | |
| Temporals | CLEAN | `created_at`, `satisfied_at`, `expires_at` |

**Verdict: CLEAN** — Well-designed three-layer model anchor. Status CHECK covers all states from the requirement state machine.

---

### 1.6 `"ob-poc".documents`

| Aspect | Score | Notes |
|--------|-------|-------|
| PK | CLEAN | UUID PK (`document_id`) |
| FK | CLEAN | FK to `document_requirements`, FK to entities |
| Indexes | CLEAN | Entity + doc_type index |
| Constraints | CLEAN | CHECK on `status` |
| Naming | CLEAN | |
| Temporals | CLEAN | `created_at`, `updated_at` |

**Verdict: CLEAN**

---

### 1.7 `"ob-poc".document_versions`

| Aspect | Score | Notes |
|--------|-------|-------|
| PK | CLEAN | UUID PK (`version_id`) |
| FK | CLEAN | FK to `documents(document_id)` |
| Indexes | CLEAN | Document + version ordering index |
| Constraints | CLEAN | CHECK on `verification_status`, content CHECK (`content IS NOT NULL OR storage_ref IS NOT NULL`) |
| Naming | CLEAN | |
| Temporals | CLEAN | `uploaded_at`, `verified_at` |
| Triggers | CLEAN | Trigger syncs version status back to requirement status |

**Verdict: CLEAN** — Immutable-per-version design with proper content constraint.

---

### 1.8 `"ob-poc".expansion_reports`

| Aspect | Score | Notes |
|--------|-------|-------|
| PK | CLEAN | UUID PK (`expansion_id`) |
| FK | FLAG | `session_id` has no FK constraint — could reference non-existent session |
| Indexes | CLEAN | 5 indexes: session, source_digest, expanded_digest, batch_policy, created_at DESC |
| Constraints | CLEAN | CHECK on `batch_policy` (`atomic`/`best_effort`) |
| Naming | CLEAN | |
| Temporals | CLEAN | `expanded_at`, `created_at`, no `updated_at` (append-only intent) |
| Append-only | FLAG | No schema-level enforcement — no trigger preventing UPDATE/DELETE |

**Verdict: FLAG** — Missing FK on `session_id`. Append-only design intent not enforced at schema level.

---

### 1.9 `"ob-poc".bpmn_correlations`

| Aspect | Score | Notes |
|--------|-------|-------|
| PK | CLEAN | UUID PK (`correlation_id`) |
| FK | FLAG | No FK on `session_id`, `runbook_id`, `entry_id`, or `process_instance_id` — intentional cross-system boundary |
| Indexes | CLEAN | UNIQUE on `process_instance_id` |
| Constraints | CLEAN | CHECK on `status` |
| Naming | CLEAN | |
| Temporals | CLEAN | `created_at`, `completed_at` |

**Verdict: MINOR** — FK absence is justified (BPMN is a separate service) and documented in CLAUDE.md. Would benefit from table COMMENT explaining the design.

---

### 1.10 `"ob-poc".bpmn_job_frames`

| Aspect | Score | Notes |
|--------|-------|-------|
| PK | MINOR | TEXT PK (`job_key`) — diverges from UUID convention, but matches BPMN's deterministic key derivation |
| FK | FLAG | No FK on `process_instance_id` — same cross-system boundary justification |
| Indexes | CLEAN | Partial index on `status WHERE active` |
| Constraints | CLEAN | CHECK on `status` |
| Naming | CLEAN | |
| Temporals | CLEAN | `activated_at`, `completed_at` |

**Verdict: MINOR** — TEXT PK is appropriate for the deterministic `(instance_id, task_id, pc)` derivation.

---

### 1.11 `"ob-poc".bpmn_parked_tokens`

| Aspect | Score | Notes |
|--------|-------|-------|
| PK | CLEAN | UUID PK (`token_id`) |
| FK | FLAG | No FK on `session_id`, `entry_id`, `process_instance_id`, `correlation_key` |
| Indexes | CLEAN | UNIQUE on `correlation_key` for O(1) signal routing |
| Constraints | CLEAN | CHECK on `status` |
| Naming | CLEAN | |
| Temporals | CLEAN | `created_at`, `resolved_at` |

**Verdict: MINOR** — Same BPMN cross-boundary pattern. UNIQUE on `correlation_key` is essential for the signal relay.

---

### 1.12 `"ob-poc".bpmn_pending_dispatches`

| Aspect | Score | Notes |
|--------|-------|-------|
| PK | CLEAN | UUID PK (`dispatch_id`) |
| FK | FLAG | No FK on `entry_id`, `runbook_id`, `correlation_id` |
| Indexes | CLEAN | UNIQUE partial index on `payload_hash WHERE status = 'pending'` (idempotency), worker scan index on `(status, last_attempted_at) WHERE status = 'pending'` |
| Constraints | CLEAN | CHECK on `status` |
| Naming | CLEAN | |
| Temporals | CLEAN | `created_at`, `last_attempted_at`, `dispatched_at` |
| Queue pattern | CLEAN | Designed for `FOR UPDATE SKIP LOCKED` claim pattern |

**Verdict: CLEAN** — Excellent queue design with idempotency via content-addressed `payload_hash` partial UNIQUE index.

---

### 1.13 `"ob-poc".dsl_sessions`

| Aspect | Score | Notes |
|--------|-------|-------|
| PK | CLEAN | UUID PK (`session_id`) with `uuidv7()` default |
| FK | CLEAN | FK to `cbus(cbu_id)`, FK to `kyc_cases(case_id)`, FK to `onboarding_requests` |
| Indexes | CLEAN | Partial indexes on `cbu_id WHERE NOT NULL`, `expires_at WHERE active`, status |
| Constraints | CLEAN | CHECK on `status` with 5 valid states |
| Naming | CLEAN | |
| Temporals | CLEAN | `created_at`, `last_activity_at`, `expires_at`, `completed_at` |

**Verdict: CLEAN** — Good use of partial indexes for active sessions and nullable foreign keys.

---

### 1.14 `"ob-poc".dsl_generation_log`

| Aspect | Score | Notes |
|--------|-------|-------|
| PK | CLEAN | UUID PK (`log_id`) with `uuidv7()` default |
| FK | CLEAN | FK to `dsl_instances(instance_id)`, FK to `intent_feedback` |
| Indexes | FLAG | No indexes beyond PK — queries by `session_id`, `domain_name`, `created_at` are likely unindexed |
| Constraints | MINOR | Uses PG ENUM `execution_status` — inconsistent with text+CHECK pattern used elsewhere |
| Naming | CLEAN | |
| Temporals | CLEAN | `created_at`, `completed_at`, `executed_at` |

**Verdict: FLAG** — Missing indexes for common query patterns (session_id, domain_name, created_at). PG ENUM for `execution_status` is an inconsistency with the text+CHECK pattern used in 90%+ of other tables.

---

### 1.15 `"ob-poc".intent_events` (migration 117)

| Aspect | Score | Notes |
|--------|-------|-------|
| PK | CLEAN | UUID PK (`event_id`) |
| FK | CLEAN (intentional) | No FKs — telemetry table, append-only, PII-safe |
| Indexes | CLEAN | ts, session_id+ts, utterance_hash, chosen_verb_fqn, dominant_entity_id partial |
| Constraints | MINOR | No CHECK on `outcome` or `semreg_mode` despite being enums |
| Naming | CLEAN | |
| Temporals | CLEAN | `ts` column, no `updated_at` (append-only) |
| Append-only | FLAG | No schema-level enforcement |

**Verdict: FLAG** — See Critical Finding CF-1 below regarding the duplicate table in two schemas.

---

### 1.16 `agent.intent_events` (migration 087)

| Aspect | Score | Notes |
|--------|-------|-------|
| PK | CLEAN | UUID PK (`event_id`) |
| FK | CLEAN (intentional) | No FKs — same design as ob-poc.intent_events |
| Indexes | CLEAN | Same 4 indexes + dominant_entity_id partial |
| Constraints | MINOR | No CHECK on `outcome` or `semreg_mode` |
| Naming | CLEAN | |
| Temporals | CLEAN | |
| Views | CLEAN | 5 monitoring views (clarify hotspots, SemReg overrides/denies, macro denies, failure modes) |

**Verdict: FLAG** — Duplicate of `"ob-poc".intent_events`. See CF-1.

---

### 1.17 `"ob-poc".learning_candidates` (via agent schema, consolidated)

| Aspect | Score | Notes |
|--------|-------|-------|
| PK | CLEAN | BIGSERIAL PK — appropriate for learning pipeline |
| FK | MINOR | `example_events` is `BIGINT[]` referencing `agent.events(id)` — no FK enforcement on array elements |
| Indexes | CLEAN | Status, learning_type, auto_applicable partial index |
| Constraints | FLAG | `status` and `risk_level` are TEXT with no CHECK constraint — app-only enforcement |
| Naming | CLEAN | |
| Temporals | CLEAN | `first_seen`, `last_seen`, `reviewed_at`, `applied_at`, `created_at`, `updated_at` |

**Verdict: FLAG** — Missing CHECK constraints on `status` (should be `pending|approved|rejected|applied`) and `risk_level` (should be `low|medium|high`). The `BIGINT[]` array for event references cannot be FK-enforced.

---

### 1.18 `"ob-poc".phrase_blocklist`

| Aspect | Score | Notes |
|--------|-------|-------|
| PK | MINOR | BIGSERIAL PK — consistent with learning pipeline |
| FK | CLEAN | No FK needed (self-contained negative examples) |
| Indexes | CLEAN | IVFFlat vector index for embedding, UNIQUE index on `(phrase, blocked_verb, COALESCE(user_id, ...))` |
| Constraints | MINOR | No CHECK constraints on any field |
| Naming | CLEAN | |
| Temporals | MINOR | `created_at` only, no `updated_at` — appropriate if append-only |
| Legacy | MINOR | `embedding_model` defaults to `'all-MiniLM-L6-v2'` — stale, system now uses `bge-small-en-v1.5` |

**Verdict: MINOR** — Stale default on `embedding_model`. The COALESCE-based UNIQUE index is a clever pattern for nullable user_id deduplication.

---

### 1.19 `"ob-poc".user_learned_phrases`

| Aspect | Score | Notes |
|--------|-------|-------|
| PK | CLEAN | BIGSERIAL PK |
| FK | MINOR | No FK on `user_id` to any users table (none exists) |
| Indexes | CLEAN | UNIQUE on `(user_id, phrase)`, IVFFlat vector index, user_id index |
| Constraints | MINOR | No CHECK on `source` or `confidence` range |
| Naming | CLEAN | |
| Temporals | CLEAN | `created_at`, `updated_at` |
| Legacy | MINOR | `embedding_model` defaults to `'all-MiniLM-L6-v2'` — same stale default |

**Verdict: MINOR** — Same stale embedding model default. `confidence` NUMERIC(3,2) naturally constrains to 0.00-9.99, but a CHECK for 0.00-1.00 would be more precise.

---

### 1.20 `"ob-poc".sessions` (migration 023)

| Aspect | Score | Notes |
|--------|-------|-------|
| PK | CLEAN | UUID PK with `gen_random_uuid()` |
| FK | MINOR | `user_id` has no FK (no users table) |
| Indexes | CLEAN | user_id partial, expires_at, updated_at DESC |
| Constraints | MINOR | No CHECK on any field — relies on trigger for expiry management |
| Naming | CLEAN | |
| Temporals | CLEAN | `created_at`, `updated_at`, `expires_at` with auto-extend trigger |
| Triggers | CLEAN | `extend_session_expiry()` trigger auto-extends on UPDATE |
| Data types | MINOR | `cbu_ids UUID[]` — array type makes FK enforcement impossible |

**Verdict: MINOR** — `UUID[]` for `cbu_ids` prevents referential integrity checking. This is documented as intentional ("memory is truth, DB is backup").

---

### 1.21 `"ob-poc".policy_version_bindings` (migration 123)

| Aspect | Score | Notes |
|--------|-------|-------|
| PK | CLEAN | UUID PK (`binding_id`) with `gen_random_uuid()` |
| FK | CLEAN | FK to `sem_reg.snapshot_sets`, FKs to `sem_reg.snapshots` for 3 optional snapshot refs |
| Indexes | CLEAN | 3 indexes: subject (kind+id+computed_at DESC), snapshot_set, requirement_profile partial |
| Constraints | MINOR | No CHECK on `subject_kind` |
| Naming | CLEAN | |
| Temporals | CLEAN | `computed_at`, `policy_effective_at`, no `updated_at` (append-only binding) |

**Verdict: CLEAN** — Well-designed immutable binding table with proper FK relationships to the SemOS registry.

---

### 1.22 `sem_reg_pub.active_requirement_profiles` / `active_proof_obligations` / `active_evidence_strategies` (migration 123)

| Aspect | Score | Notes |
|--------|-------|-------|
| PK | CLEAN | Composite PK `(snapshot_set_id, fqn)` — correct for published read models |
| FK | FLAG | No FK on `snapshot_set_id` or `snapshot_id` to `sem_reg` tables |
| Indexes | CLEAN | PK provides the lookup index |
| Constraints | CLEAN | All columns NOT NULL |
| Naming | CLEAN | |
| Temporals | CLEAN | `published_at` |

**Verdict: FLAG** — Missing FK to `sem_reg.snapshot_sets(snapshot_set_id)` and `sem_reg.snapshots(snapshot_id)`. These are published projections that should reference their source snapshots. The `policy_version_bindings` table in the same migration correctly has these FKs, suggesting an oversight.

---

## 2. Cross-Table Consistency Findings

### CT-1: PK Strategy Divergence (MINOR)

| Pattern | Tables |
|---------|--------|
| UUID PK | workflow_pending_tasks, workflow_task_events, document_requirements, documents, document_versions, expansion_reports, bpmn_correlations, bpmn_parked_tokens, bpmn_pending_dispatches, dsl_sessions, dsl_generation_log, intent_events (both), policy_version_bindings |
| BIGSERIAL PK | task_result_queue, task_result_dlq, learning_candidates, phrase_blocklist, user_learned_phrases, learning_audit, agent.events |
| TEXT PK | bpmn_job_frames |

**Assessment:** The split is intentional and follows a coherent pattern: domain/audit tables use UUID; ephemeral queues and learning pipeline use BIGSERIAL; BPMN job frames use deterministic TEXT keys. This is acceptable.

---

### CT-2: Enum Strategy Inconsistency (FLAG)

| Strategy | Tables |
|----------|--------|
| Text + CHECK | dsl_sessions, bpmn_correlations, bpmn_job_frames, bpmn_parked_tokens, bpmn_pending_dispatches, expansion_reports, document_requirements, document_versions |
| PG native ENUM | dsl_generation_log (`execution_status`) |
| Text without CHECK | learning_candidates (`status`, `risk_level`), intent_events (`outcome`, `semreg_mode`), phrase_blocklist, user_learned_phrases |

**Assessment:** The codebase predominantly uses text+CHECK, but `dsl_generation_log.execution_status` uses a PG ENUM type. PG ENUMs are problematic because adding values requires `ALTER TYPE ... ADD VALUE` which cannot be rolled back in a transaction. The text-without-CHECK tables lack basic validation.

**Recommendation:** Standardize on text+CHECK. Add CHECK constraints to `learning_candidates.status`, `learning_candidates.risk_level`, and `intent_events.outcome`.

---

### CT-3: Append-Only Tables Without Schema Enforcement (FLAG)

The following tables are designed as append-only audit trails but lack schema-level enforcement:

| Table | Evidence of Append-Only Intent |
|-------|-------------------------------|
| `workflow_task_events` | Named "events", no `updated_at` column |
| `expansion_reports` | No `updated_at`, documented as audit trail |
| `intent_events` (both schemas) | No `updated_at`, described as "append-only telemetry" |
| `learning_audit` | Named "audit", FK to candidates, `can_rollback` flag |
| `policy_version_bindings` | Documented as "immutable audit bindings" |

None of these have triggers or rules preventing `UPDATE` or `DELETE`. The `sem_reg.snapshots` table (migration 090) does have an immutability trigger — this pattern should be extended to these tables.

---

### CT-4: Missing FK on `session_id` (MINOR)

Multiple tables reference `session_id` without FK constraints:

| Table | Column | FK Present? |
|-------|--------|:-----------:|
| `expansion_reports` | `session_id` | No |
| `bpmn_correlations` | `session_id` | No |
| `bpmn_parked_tokens` | `session_id` | No |
| `intent_events` | `session_id` | No |
| `agent.events` | `session_id` | No |

**Assessment:** For telemetry tables (`intent_events`, `agent.events`), FK absence is justified — telemetry should never fail due to session lifecycle. For `expansion_reports`, a FK to `dsl_sessions` or `sessions` would improve referential integrity. For BPMN tables, FK absence is justified by the cross-system boundary.

---

### CT-5: Stale Default Values (MINOR)

| Table | Column | Default | Current Value |
|-------|--------|---------|---------------|
| `phrase_blocklist` | `embedding_model` | `'all-MiniLM-L6-v2'` | `bge-small-en-v1.5` |
| `user_learned_phrases` | `embedding_model` | `'all-MiniLM-L6-v2'` | `bge-small-en-v1.5` |

These defaults reference the deprecated MiniLM model. New rows inserted without explicit model values will have incorrect metadata.

---

## 3. Critical and Notable Findings

### CF-1: Duplicate `intent_events` Table (CRITICAL)

**Two identical tables exist:**
- `agent.intent_events` — created in migration 087, extended in 088 and 103
- `"ob-poc".intent_events` — created in migration 117 with all columns from 087+088+103 baked in

Migration 117 creates a complete copy of the `agent.intent_events` structure in the `"ob-poc"` schema. The table has the same columns, same indexes (with different names prefixed `ob_poc_` instead of bare names), but no migration drops the original or migrates data.

**Risks:**
1. Dual-write ambiguity — which table does the application write to?
2. Query confusion — monitoring views in migration 087 reference `agent.intent_events`
3. Storage waste if both are written
4. Schema drift if only one gets future ALTERs

**Recommendation:** Determine which table is authoritative. The schema consolidation (migrations 115-121) moved business tables to `"ob-poc"`, suggesting `agent.intent_events` should have been dropped. Either add a migration to drop `agent.intent_events` (after verifying no active references) or add a migration to drop `"ob-poc".intent_events` if it was created in error.

---

### CF-2: Changeset Immutability — App-Only Enforcement (FLAG)

The changeset lifecycle (Draft -> UnderReview -> Approved -> Validated -> DryRunPassed -> Published) is enforced entirely in application code (`GovernanceVerbService` in Rust). The database has:
- `sem_reg.changesets` with a CHECK constraint on `status` (9 valid values, fixed in migration 101)
- No trigger preventing invalid status transitions (e.g., Published -> Draft)
- No trigger preventing content changes after publish

The `sem_reg.snapshots` table has an immutability trigger (migration 090), but `sem_reg.changesets` does not.

**Recommendation:** Add a transition guard trigger on `sem_reg.changesets` that prevents:
1. Reverse transitions (Published -> any earlier state)
2. Content field changes after status = 'published'

---

### CF-3: Two-Stage Validation Pipeline Not Reflected in Schema (MINOR)

The two-stage validation pipeline (Stage 1: artifact integrity, Stage 2: scratch schema) stores results in `sem_reg_authoring.validation_reports` with a `stage` column. However:
- No CHECK constraint on `stage` (should be `'stage1'|'stage2'`)
- No constraint ensuring Stage 1 passes before Stage 2 can be recorded
- The ordering is enforced purely in `GovernanceVerbService`

This is acceptable for a POC but would benefit from a CHECK constraint on `stage`.

---

### CF-4: Advisory Lock Error Codes Not in Schema (MINOR)

The advisory locking system (`execute_runbook_with_pool`) uses `pg_advisory_xact_lock` with deterministic lock keys derived from `(entity_type, entity_id)`. The error taxonomy (`LockContention`, `EntityDeleted`, `VersionConflict`, `ConstraintViolation`) is entirely Rust-side. No database enum or reference table captures these error codes.

This is acceptable — advisory lock errors are transient runtime conditions, not persisted state.

---

### CF-5: `sem_reg_pub` Published Tables Missing FKs (FLAG)

The three published read model tables in migration 123 (`active_requirement_profiles`, `active_proof_obligations`, `active_evidence_strategies`) lack FK constraints to their source tables:
- `snapshot_set_id` should FK to `sem_reg.snapshot_sets(snapshot_set_id)`
- `snapshot_id` should FK to `sem_reg.snapshots(snapshot_id)`

The `policy_version_bindings` table in the same migration correctly has both FKs, confirming this is likely an oversight.

---

## 4. Severity Summary

| Severity | Count | Items |
|----------|-------|-------|
| CRITICAL | 1 | CF-1 (duplicate intent_events) |
| FLAG | 7 | CT-2 (enum inconsistency), CT-3 (append-only not enforced), CF-2 (changeset immutability), CF-5 (sem_reg_pub missing FKs), scorecard flags on expansion_reports, workflow_task_events, learning_candidates |
| MINOR | 11 | CT-1 (PK divergence), CT-4 (session_id FK), CT-5 (stale defaults), CF-3 (validation stage CHECK), CF-4 (advisory lock codes), various per-table items |
| CLEAN | 8 | workflow_pending_tasks, document_requirements, documents, document_versions, bpmn_pending_dispatches, dsl_sessions, policy_version_bindings, sem_reg_pub tables (structure) |

---

## 5. Index Recommendation List

### High Priority

| Table | Recommended Index | Rationale |
|-------|-------------------|-----------|
| `dsl_generation_log` | `CREATE INDEX idx_dsl_gen_log_session ON "ob-poc".dsl_generation_log(session_id);` | Session-scoped queries for generation history |
| `dsl_generation_log` | `CREATE INDEX idx_dsl_gen_log_domain ON "ob-poc".dsl_generation_log(domain_name, created_at DESC);` | Domain-filtered audit queries |
| `dsl_generation_log` | `CREATE INDEX idx_dsl_gen_log_created ON "ob-poc".dsl_generation_log(created_at DESC);` | Time-ordered audit trail |

### Medium Priority

| Table | Recommended Index | Rationale |
|-------|-------------------|-----------|
| `sem_reg_pub.active_requirement_profiles` | `CREATE INDEX idx_arp_snapshot_id ON sem_reg_pub.active_requirement_profiles(snapshot_id);` | Join queries to source snapshots |
| `sem_reg_pub.active_proof_obligations` | `CREATE INDEX idx_apo_snapshot_id ON sem_reg_pub.active_proof_obligations(snapshot_id);` | Same pattern |
| `sem_reg_pub.active_evidence_strategies` | `CREATE INDEX idx_aes_snapshot_id ON sem_reg_pub.active_evidence_strategies(snapshot_id);` | Same pattern |

### Low Priority (Nice to Have)

| Table | Recommended Index | Rationale |
|-------|-------------------|-----------|
| `expansion_reports` | `CREATE INDEX idx_expansion_session ON "ob-poc".expansion_reports(session_id);` | Already exists per master-schema, but confirm coverage |
| `learning_candidates` | `CREATE INDEX idx_learning_candidates_risk ON "ob-poc".learning_candidates(risk_level) WHERE risk_level = 'high';` | Quick lookup for high-risk candidates needing review |

---

## 6. Constraint Addition Recommendations

### CHECK Constraints to Add

```sql
-- learning_candidates: status enum
ALTER TABLE "ob-poc".learning_candidates
  ADD CONSTRAINT learning_candidates_status_check
  CHECK (status IN ('pending', 'approved', 'rejected', 'applied', 'promoted'));

-- learning_candidates: risk_level enum
ALTER TABLE "ob-poc".learning_candidates
  ADD CONSTRAINT learning_candidates_risk_check
  CHECK (risk_level IN ('low', 'medium', 'high'));

-- intent_events: outcome enum (apply to whichever table is authoritative)
-- Values from orchestrator.rs outcome_label():
ALTER TABLE "ob-poc".intent_events
  ADD CONSTRAINT intent_events_outcome_check
  CHECK (outcome IN (
    'ready', 'needs_clarification', 'no_match', 'no_allowed_verbs',
    'scope_resolved', 'direct_dsl_denied', 'macro_expanded', 'error'
  ));

-- intent_events: semreg_mode enum
ALTER TABLE "ob-poc".intent_events
  ADD CONSTRAINT intent_events_semreg_mode_check
  CHECK (semreg_mode IN ('strict', 'permissive', 'fail_open'));

-- user_learned_phrases: confidence range
ALTER TABLE "ob-poc".user_learned_phrases
  ADD CONSTRAINT user_learned_phrases_confidence_check
  CHECK (confidence >= 0.00 AND confidence <= 1.00);
```

### FK Constraints to Add

```sql
-- sem_reg_pub tables: FK to source snapshots
ALTER TABLE sem_reg_pub.active_requirement_profiles
  ADD CONSTRAINT fk_arp_snapshot_set
  FOREIGN KEY (snapshot_set_id) REFERENCES sem_reg.snapshot_sets(snapshot_set_id);

ALTER TABLE sem_reg_pub.active_proof_obligations
  ADD CONSTRAINT fk_apo_snapshot_set
  FOREIGN KEY (snapshot_set_id) REFERENCES sem_reg.snapshot_sets(snapshot_set_id);

ALTER TABLE sem_reg_pub.active_evidence_strategies
  ADD CONSTRAINT fk_aes_snapshot_set
  FOREIGN KEY (snapshot_set_id) REFERENCES sem_reg.snapshot_sets(snapshot_set_id);
```

### Append-Only Triggers to Add

```sql
-- Template: apply to workflow_task_events, expansion_reports, intent_events, policy_version_bindings
CREATE OR REPLACE FUNCTION prevent_mutation() RETURNS TRIGGER AS $$
BEGIN
  RAISE EXCEPTION 'Table % is append-only: % not permitted',
    TG_TABLE_NAME, TG_OP;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER enforce_append_only_workflow_task_events
  BEFORE UPDATE OR DELETE ON "ob-poc".workflow_task_events
  FOR EACH ROW EXECUTE FUNCTION prevent_mutation();

CREATE TRIGGER enforce_append_only_expansion_reports
  BEFORE UPDATE OR DELETE ON "ob-poc".expansion_reports
  FOR EACH ROW EXECUTE FUNCTION prevent_mutation();

-- For intent_events: apply to whichever table is authoritative after CF-1 resolution
```

---

## 7. Additional Focus Area Findings

### Temporal Integrity for Audit Trail Tables

**Finding:** Audit trail tables (`workflow_task_events`, `expansion_reports`, `learning_audit`, `intent_events`, `policy_version_bindings`) all follow the pattern of having `created_at`/`timestamp` without `updated_at`, signaling append-only intent. However, none enforce this at schema level. The `sem_reg.snapshots` immutability trigger (migration 090) is the only example of schema-level enforcement in the codebase. **Severity: FLAG.**

### Changeset Immutability

**Finding:** The compose-to-publish lifecycle is enforced purely in `GovernanceVerbService` (application code). The database permits any status transition and content modification. The changeset `content_hash` provides detection of content drift but not prevention. **Severity: FLAG** — acceptable for POC, needs trigger guards for production.

### Two-Stage Validation Pipeline Reflection

**Finding:** `sem_reg_authoring.validation_reports` stores both stages but has no CHECK on `stage` and no ordering constraint. The `change_set_id` + `stage` combination is not UNIQUE, allowing multiple reports per stage (which is valid for re-runs but could be confusing). **Severity: MINOR.**

### Advisory Locking Schema Support

**Finding:** The advisory locking system is entirely runtime (Rust `DerivedLock` + `pg_advisory_xact_lock`). No schema support needed — PostgreSQL advisory locks are ephemeral by design. Error codes are transient and appropriately handled in application code. **Severity: CLEAN.**
