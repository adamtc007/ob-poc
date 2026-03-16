# P3-D: SemOS Metadata & Infrastructure Tables — Schema Review

> **Reviewer:** Claude Opus 4.6
> **Date:** 2026-03-16
> **Scope:** Semantic Registry core (`sem_reg`), authoring (`sem_reg_authoring`), projections (`sem_reg_pub`), stewardship, evidence instances, embedding infrastructure, GLEIF integration, screening tables
> **Migrations reviewed:** 037, 078, 085, 086, 090, 091, 092, 093, 094, 095, 097, 098, 099, 100, 122, 123 + master-schema.sql excerpts

---

## 1. Per-Table Scorecard

### 1.1 sem_reg.snapshots (Migration 078)

| Aspect | Rating | Notes |
|--------|--------|-------|
| **PK** | CLEAN | `snapshot_id UUID PRIMARY KEY DEFAULT gen_random_uuid()` |
| **FK** | CLEAN | `predecessor_id` self-referential FK, `snapshot_set_id` FK to `snapshot_sets` |
| **Indexes** | CLEAN | Partial unique on `(object_type, object_id) WHERE status='active' AND effective_until IS NULL`; temporal index on `(object_type, created_at DESC)`; `(object_type, fqn)` index |
| **Constraints** | CLEAN | CHECK on proof rule (`governance_tier='governed' OR trust_class != 'proof'`); PG enum types for `object_type`, `governance_tier`, `trust_class`, `snapshot_status`, `change_type` |
| **Naming** | CLEAN | Consistent snake_case |
| **Immutability** | CLEAN | Trigger added in migration 090 prevents UPDATE/DELETE on published snapshots |

**Verdict: CLEAN**

---

### 1.2 sem_reg.snapshot_sets (Migration 078)

| Aspect | Rating | Notes |
|--------|--------|-------|
| **PK** | CLEAN | `snapshot_set_id UUID PRIMARY KEY DEFAULT gen_random_uuid()` |
| **FK** | — | No FKs (root table) |
| **Indexes** | MINOR | Only PK index. No index on `created_at` for temporal queries |
| **Constraints** | CLEAN | `created_at NOT NULL DEFAULT now()` |
| **Naming** | CLEAN | Consistent |

**Verdict: CLEAN**

---

### 1.3 sem_reg.agent_plans (Migration 085)

| Aspect | Rating | Notes |
|--------|--------|-------|
| **PK** | CLEAN | `plan_id UUID PRIMARY KEY DEFAULT gen_random_uuid()` |
| **FK** | MINOR | No FK on `context_resolution_ref` (intentional — reference to ephemeral resolution) |
| **Indexes** | CLEAN | `(status, created_at DESC)` |
| **Constraints** | FLAG | Status uses `TEXT CHECK` instead of PG enum — inconsistent with 078's enum pattern |
| **Naming** | CLEAN | Consistent |

**Verdict: MINOR** — TEXT CHECK vs enum inconsistency

---

### 1.4 sem_reg.plan_steps (Migration 085)

| Aspect | Rating | Notes |
|--------|--------|-------|
| **PK** | CLEAN | `step_id UUID PRIMARY KEY DEFAULT gen_random_uuid()` |
| **FK** | CLEAN | `plan_id` FK to `agent_plans` |
| **Indexes** | CLEAN | `(plan_id, step_order)` |
| **Constraints** | FLAG | Status uses TEXT CHECK. `verb_snapshot_id` has no FK to `snapshots` |
| **Naming** | CLEAN | Consistent |

**Verdict: MINOR** — Missing snapshot FK weakens provenance chain

---

### 1.5 sem_reg.decision_records (Migration 085)

| Aspect | Rating | Notes |
|--------|--------|-------|
| **PK** | CLEAN | `decision_id UUID PRIMARY KEY DEFAULT gen_random_uuid()` |
| **FK** | FLAG | `plan_id` FK to `agent_plans`, `step_id` FK to `plan_steps`. `escalation_id` has **no FK** to `escalation_records` despite being a logical reference |
| **Indexes** | CLEAN | `(plan_id, decided_at DESC)` |
| **Constraints** | CLEAN | `snapshot_manifest JSONB NOT NULL` ensures provenance |
| **Naming** | CLEAN | Consistent |

**Verdict: FLAG** — Missing FK on `escalation_id`

---

### 1.6 sem_reg.disambiguation_prompts (Migration 085)

| Aspect | Rating | Notes |
|--------|--------|-------|
| **PK** | CLEAN | `prompt_id UUID PRIMARY KEY DEFAULT gen_random_uuid()` |
| **FK** | CLEAN | `decision_id` FK to `decision_records` |
| **Indexes** | CLEAN | Via FK |
| **Constraints** | CLEAN | Minimal, appropriate |
| **Naming** | CLEAN | Consistent |

**Verdict: CLEAN**

---

### 1.7 sem_reg.escalation_records (Migration 085)

| Aspect | Rating | Notes |
|--------|--------|-------|
| **PK** | CLEAN | `escalation_id UUID PRIMARY KEY DEFAULT gen_random_uuid()` |
| **FK** | MINOR | `plan_id` FK to `agent_plans`. No FK from `decision_records.escalation_id` back to this table |
| **Indexes** | CLEAN | `(plan_id, escalated_at DESC)` |
| **Constraints** | CLEAN | Status CHECK for lifecycle |
| **Naming** | CLEAN | Consistent |

**Verdict: CLEAN**

---

### 1.8 sem_reg.derivation_edges (Migration 086)

| Aspect | Rating | Notes |
|--------|--------|-------|
| **PK** | CLEAN | `edge_id UUID PRIMARY KEY DEFAULT gen_random_uuid()` |
| **FK** | FLAG | `output_snapshot_id` FK to `snapshots`. `input_snapshot_ids UUID[]` has **no element-level FK enforcement** — PG arrays cannot enforce FKs |
| **Indexes** | CLEAN | GIN on `input_snapshot_ids`, index on `(output_snapshot_id, created_at DESC)` |
| **Constraints** | CLEAN | Append-only by convention |
| **Naming** | CLEAN | Consistent |

**Verdict: FLAG** — UUID array FK gap is a known PG limitation; application-level enforcement required

---

### 1.9 sem_reg.run_records (Migration 086)

| Aspect | Rating | Notes |
|--------|--------|-------|
| **PK** | CLEAN | `run_id UUID PRIMARY KEY DEFAULT gen_random_uuid()` |
| **FK** | FLAG | `plan_id` and `step_id` are optional but have **no FK** declarations despite referencing `agent_plans`/`plan_steps` |
| **Indexes** | CLEAN | `(spec_fqn, started_at DESC)` |
| **Constraints** | CLEAN | Status CHECK |
| **Naming** | CLEAN | Consistent |

**Verdict: FLAG** — Missing FKs on plan_id/step_id

---

### 1.10 sem_reg.embedding_records (Migration 086)

| Aspect | Rating | Notes |
|--------|--------|-------|
| **PK** | CLEAN | `embedding_id UUID PRIMARY KEY DEFAULT gen_random_uuid()` |
| **FK** | CLEAN | `snapshot_id` FK to `snapshots` |
| **Indexes** | CLEAN | UNIQUE on `snapshot_id` |
| **Constraints** | CLEAN | `version_hash TEXT NOT NULL` for staleness tracking |
| **Naming** | CLEAN | Consistent |

**Verdict: CLEAN**

---

### 1.11 sem_reg.outbox_events (Migration 092)

| Aspect | Rating | Notes |
|--------|--------|-------|
| **PK** | CLEAN | `event_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY` — sequential for ordered processing |
| **FK** | MINOR | `snapshot_set_id` has **no FK** to `snapshot_sets`. Intentional for outbox decoupling but weakens referential integrity |
| **Indexes** | CLEAN | `(processed_at) WHERE processed_at IS NULL` for pending claims; `(claimed_at) WHERE claimed_at IS NOT NULL AND processed_at IS NULL` for stale claims |
| **Constraints** | CLEAN | `event_type TEXT NOT NULL`, `payload JSONB NOT NULL` |
| **Naming** | CLEAN | Consistent |

**Verdict: CLEAN** — Missing FK is standard outbox pattern

---

### 1.12 sem_reg.bootstrap_audit (Migration 093)

| Aspect | Rating | Notes |
|--------|--------|-------|
| **PK** | CLEAN | `bundle_hash TEXT PRIMARY KEY` — hash-based lookup, appropriate for content-addressed pattern |
| **FK** | MINOR | `snapshot_set_id` has no FK. Intentional for bootstrap independence |
| **Indexes** | CLEAN | PK sufficient for lookup pattern |
| **Constraints** | CLEAN | Status CHECK (`pending`, `published`, `failed`) |
| **Naming** | CLEAN | Consistent |

**Verdict: CLEAN**

---

### 1.13 sem_reg_pub.active_verb_contracts / active_entity_types / active_taxonomies (Migration 094)

| Aspect | Rating | Notes |
|--------|--------|-------|
| **PK** | CLEAN | Composite `(snapshot_set_id, fqn)` — appropriate for projection tables |
| **FK** | — | **No FKs** — intentional for projection independence (rebuilt from source snapshots) |
| **Indexes** | CLEAN | PK composite index sufficient for read patterns |
| **Constraints** | CLEAN | `payload JSONB NOT NULL`, `published_at TIMESTAMPTZ NOT NULL` |
| **Naming** | CLEAN | Consistent across all three projection tables |

**Verdict: CLEAN** — No-FK projection pattern is correct

---

### 1.14 sem_reg_pub.projection_watermark (Migration 094)

| Aspect | Rating | Notes |
|--------|--------|-------|
| **PK** | CLEAN | `projection_name TEXT PRIMARY KEY` |
| **FK** | — | No FKs needed |
| **Indexes** | CLEAN | PK sufficient |
| **Constraints** | CLEAN | `last_event_id BIGINT NOT NULL`, `updated_at TIMESTAMPTZ NOT NULL` |
| **Naming** | CLEAN | Consistent |

**Verdict: CLEAN**

---

### 1.15 sem_reg_pub.active_snapshot_set (Migration 099)

| Aspect | Rating | Notes |
|--------|--------|-------|
| **PK** | CLEAN | `singleton BOOLEAN PRIMARY KEY DEFAULT true CHECK (singleton = true)` — singleton pattern ensuring exactly one row |
| **FK** | CLEAN | `active_snapshot_set_id` FK to `sem_reg.snapshot_sets` |
| **Indexes** | CLEAN | PK sufficient (single row) |
| **Constraints** | CLEAN | Singleton CHECK is elegant |
| **Naming** | CLEAN | Consistent |

**Verdict: CLEAN** — Singleton pattern is well-implemented

---

### 1.16 sem_reg.changesets (Migration 095 + 097 + 099)

| Aspect | Rating | Notes |
|--------|--------|-------|
| **PK** | CLEAN | `changeset_id UUID PRIMARY KEY DEFAULT gen_random_uuid()` |
| **FK** | CLEAN | `supersedes_change_set_id` and `superseded_by` self-referential FKs (added 099) |
| **Indexes** | CLEAN | `(status, created_at DESC)`, unique content hash index with partial filter |
| **Constraints** | FLAG | Status CHECK constraint **dropped and recreated 3 times** across migrations 095, 097, 099. Final state has 9 values. Risk of intermediate states during migration application |
| **Naming** | MINOR | Column `owner_actor_id` (TEXT) vs archive table's `owner_id` (UUID) — type mismatch |

**Verdict: FLAG** — Triple CHECK rewrite is fragile; archive type mismatch

---

### 1.17 sem_reg.changeset_entries (Migration 095 + 097 + 099)

| Aspect | Rating | Notes |
|--------|--------|-------|
| **PK** | CLEAN | `entry_id UUID PRIMARY KEY DEFAULT gen_random_uuid()` |
| **FK** | CLEAN | `changeset_id` FK to `changesets` |
| **Indexes** | CLEAN | `(changeset_id, entry_order)` |
| **Constraints** | FLAG | `object_type TEXT` has **no CHECK** constraint, unlike `snapshots` which uses a PG enum. Content could diverge from valid enum values |
| **Naming** | CLEAN | Consistent |

**Verdict: FLAG** — Unconstrained `object_type` TEXT column

---

### 1.18 sem_reg.changeset_reviews (Migration 095)

| Aspect | Rating | Notes |
|--------|--------|-------|
| **PK** | CLEAN | `review_id UUID PRIMARY KEY DEFAULT gen_random_uuid()` |
| **FK** | CLEAN | `changeset_id` FK to `changesets` |
| **Indexes** | CLEAN | `(changeset_id, reviewed_at DESC)` |
| **Constraints** | CLEAN | `verdict TEXT NOT NULL CHECK (verdict IN (...))` |
| **Naming** | CLEAN | Consistent |

**Verdict: CLEAN**

---

### 1.19 stewardship.events (Migration 097)

| Aspect | Rating | Notes |
|--------|--------|-------|
| **PK** | CLEAN | `event_id UUID PRIMARY KEY DEFAULT gen_random_uuid()` |
| **FK** | MINOR | `changeset_id` has no FK to `sem_reg.changesets` (cross-schema FK avoidance) |
| **Indexes** | CLEAN | `(changeset_id, ts DESC)`, `(event_type, ts DESC)` |
| **Constraints** | CLEAN | `event_type TEXT NOT NULL`, `ts TIMESTAMPTZ NOT NULL DEFAULT now()` |
| **Naming** | CLEAN | Consistent |

**Verdict: CLEAN** — Cross-schema FK avoidance is a defensible choice

---

### 1.20 stewardship.basis_records (Migration 097)

| Aspect | Rating | Notes |
|--------|--------|-------|
| **PK** | CLEAN | `basis_id UUID PRIMARY KEY DEFAULT gen_random_uuid()` |
| **FK** | MINOR | `changeset_id` no FK (cross-schema), `snapshot_id` no FK to `sem_reg.snapshots` |
| **Indexes** | CLEAN | `(changeset_id)`, `(snapshot_id)` |
| **Constraints** | CLEAN | `basis_type TEXT NOT NULL` |
| **Naming** | CLEAN | Consistent |

**Verdict: CLEAN**

---

### 1.21 stewardship.basis_claims (Migration 097)

| Aspect | Rating | Notes |
|--------|--------|-------|
| **PK** | CLEAN | `claim_id UUID PRIMARY KEY DEFAULT gen_random_uuid()` |
| **FK** | CLEAN | `basis_id` FK to `basis_records` |
| **Indexes** | CLEAN | `(basis_id)` |
| **Constraints** | CLEAN | Minimal, appropriate |
| **Naming** | CLEAN | Consistent |

**Verdict: CLEAN**

---

### 1.22 stewardship.conflict_records (Migration 097)

| Aspect | Rating | Notes |
|--------|--------|-------|
| **PK** | CLEAN | `conflict_id UUID PRIMARY KEY DEFAULT gen_random_uuid()` |
| **FK** | MINOR | `changeset_id` and `competing_changeset_id` — no FKs (cross-schema) |
| **Indexes** | CLEAN | `(changeset_id)`, `(competing_changeset_id)` |
| **Constraints** | CLEAN | Appropriate |
| **Naming** | CLEAN | Consistent |

**Verdict: CLEAN**

---

### 1.23 stewardship.templates (Migration 097)

| Aspect | Rating | Notes |
|--------|--------|-------|
| **PK** | CLEAN | `template_id UUID PRIMARY KEY DEFAULT gen_random_uuid()` |
| **FK** | — | Self-contained |
| **Indexes** | CLEAN | Partial unique on `(template_key) WHERE status = 'active'` — one active per key |
| **Constraints** | CLEAN | Status CHECK |
| **Naming** | CLEAN | Consistent |

**Verdict: CLEAN**

---

### 1.24 stewardship.focus_states (Migration 098)

| Aspect | Rating | Notes |
|--------|--------|-------|
| **PK** | CLEAN | `session_id UUID PRIMARY KEY` — session-scoped, no auto-generated UUID (intentional — one focus per session) |
| **FK** | MINOR | `changeset_id` no FK (cross-schema) |
| **Indexes** | CLEAN | PK sufficient |
| **Constraints** | CLEAN | `updated_at TIMESTAMPTZ NOT NULL DEFAULT now()` |
| **Naming** | CLEAN | Consistent |

**Verdict: CLEAN**

---

### 1.25 stewardship.viewport_manifests (Migration 098)

| Aspect | Rating | Notes |
|--------|--------|-------|
| **PK** | CLEAN | `manifest_id UUID PRIMARY KEY DEFAULT gen_random_uuid()` |
| **FK** | FLAG | `session_id` has **no FK** to `focus_states`. `overlay_changeset_id` has **no FK** to `changesets` |
| **Indexes** | CLEAN | `(session_id, captured_at DESC)` |
| **Constraints** | CLEAN | `viewport_kind TEXT NOT NULL`, `payload JSONB NOT NULL` |
| **Naming** | CLEAN | Consistent |

**Verdict: FLAG** — Missing FK on session_id to focus_states

---

### 1.26 sem_reg_authoring.change_set_artifacts (Migration 099)

| Aspect | Rating | Notes |
|--------|--------|-------|
| **PK** | CLEAN | `artifact_id UUID PRIMARY KEY DEFAULT gen_random_uuid()` |
| **FK** | CLEAN | `change_set_id` FK to `sem_reg.changesets` |
| **Indexes** | CLEAN | `(change_set_id, ordinal)` |
| **Constraints** | CLEAN | `artifact_type TEXT NOT NULL CHECK (...)` with 6 valid values |
| **Naming** | CLEAN | Consistent |

**Verdict: CLEAN**

---

### 1.27 sem_reg_authoring.validation_reports (Migration 099)

| Aspect | Rating | Notes |
|--------|--------|-------|
| **PK** | CLEAN | `report_id UUID PRIMARY KEY DEFAULT gen_random_uuid()` |
| **FK** | CLEAN | `change_set_id` FK to `sem_reg.changesets` |
| **Indexes** | CLEAN | `(change_set_id, stage, ran_at DESC)` |
| **Constraints** | CLEAN | `stage TEXT NOT NULL CHECK (stage IN ('validate','dry_run'))`, `ok BOOLEAN NOT NULL` |
| **Naming** | CLEAN | Consistent |

**Verdict: CLEAN**

---

### 1.28 sem_reg_authoring.governance_audit_log (Migration 099)

| Aspect | Rating | Notes |
|--------|--------|-------|
| **PK** | CLEAN | `entry_id UUID PRIMARY KEY DEFAULT gen_random_uuid()` |
| **FK** | FLAG | `active_snapshot_set_id NOT NULL` but **no FK** to `snapshot_sets`. `change_set_id` and `snapshot_set_id` also have no FKs |
| **Indexes** | CLEAN | `(ts DESC)`, `(change_set_id)` |
| **Constraints** | CLEAN | `verb TEXT NOT NULL`, `result JSONB NOT NULL`, `duration_ms BIGINT NOT NULL` |
| **Naming** | CLEAN | Consistent |

**Verdict: FLAG** — NOT NULL column without FK on `active_snapshot_set_id`

---

### 1.29 sem_reg_authoring.publish_batches (Migration 099)

| Aspect | Rating | Notes |
|--------|--------|-------|
| **PK** | CLEAN | `batch_id UUID PRIMARY KEY DEFAULT gen_random_uuid()` |
| **FK** | MINOR | `snapshot_set_id NOT NULL` — no FK. `change_set_ids UUID[]` — no element-level FK enforcement |
| **Indexes** | CLEAN | PK sufficient for lookup |
| **Constraints** | CLEAN | `publisher TEXT NOT NULL` |
| **Naming** | CLEAN | Consistent |

**Verdict: MINOR** — UUID array FK gap (same PG limitation as derivation_edges)

---

### 1.30 sem_reg_authoring.change_sets_archive (Migration 100)

| Aspect | Rating | Notes |
|--------|--------|-------|
| **PK** | CLEAN | `changeset_id UUID PRIMARY KEY` — no DEFAULT (copied from source) |
| **FK** | — | Archive tables are standalone by design |
| **Indexes** | CLEAN | `(status)`, `(archived_at DESC)` |
| **Constraints** | CRITICAL | `owner_id UUID` but source table has `owner_actor_id TEXT` — **type mismatch**. Archive INSERT will fail for non-UUID actor IDs |
| **Naming** | MINOR | Column name change: `owner_actor_id` → `owner_id` |

**Verdict: CRITICAL** — Archive column type/name mismatch with live table

---

### 1.31 sem_reg.observations (Migration 090)

| Aspect | Rating | Notes |
|--------|--------|-------|
| **PK** | CLEAN | `observation_id UUID PRIMARY KEY DEFAULT gen_random_uuid()` |
| **FK** | CLEAN | `snapshot_id` FK to `snapshots`, `supersedes` FK to self |
| **Indexes** | CLEAN | `(snapshot_id, observed_at DESC)`, partial unique on `(supersedes) WHERE supersedes IS NOT NULL` for linear chain enforcement |
| **Constraints** | CLEAN | Linear chain via partial unique index |
| **Naming** | CLEAN | Consistent |

**Verdict: CLEAN** — Linear chain enforcement pattern is well-implemented

---

### 1.32 sem_reg.attribute_observations (Migration 091)

| Aspect | Rating | Notes |
|--------|--------|-------|
| **PK** | CLEAN | `observation_id UUID PRIMARY KEY DEFAULT gen_random_uuid()` |
| **FK** | CLEAN | `supersedes` FK to self |
| **Indexes** | CLEAN | `(subject_ref, attribute_fqn, observed_at DESC)`, partial unique for linear chain |
| **Constraints** | CLEAN | `subject_ref TEXT NOT NULL` — intentionally generic |
| **Naming** | CLEAN | Consistent |

**Verdict: CLEAN** — Entity-centric observation model matching the domain design

---

### 1.33 sem_reg.document_instances (Migration 090)

| Aspect | Rating | Notes |
|--------|--------|-------|
| **PK** | CLEAN | `instance_id UUID PRIMARY KEY DEFAULT gen_random_uuid()` |
| **FK** | FLAG | `entity_id UUID NOT NULL` has **no FK** to `"ob-poc".entities`. Cross-schema, but this is a strong entity reference |
| **Indexes** | CLEAN | `(entity_id, document_type_fqn)`, `(status)` |
| **Constraints** | CLEAN | Lifecycle guard trigger (migration 091) protects immutable fields |
| **Naming** | CLEAN | Consistent |

**Verdict: FLAG** — Missing cross-schema FK on entity_id

---

### 1.34 sem_reg.provenance_edges (Migration 090)

| Aspect | Rating | Notes |
|--------|--------|-------|
| **PK** | CLEAN | `edge_id UUID PRIMARY KEY DEFAULT gen_random_uuid()` |
| **FK** | — | No FKs — polymorphic references via `source_type/source_id` and `target_type/target_id` |
| **Indexes** | CLEAN | `(source_type, source_id)`, `(target_type, target_id)` |
| **Constraints** | MINOR | `source_type` and `target_type` are unconstrained TEXT — no CHECK or enum |
| **Naming** | CLEAN | Consistent |

**Verdict: MINOR** — Polymorphic FK pattern is acceptable but unconstrained type columns are a data quality risk

---

### 1.35 sem_reg.retention_policies (Migration 090)

| Aspect | Rating | Notes |
|--------|--------|-------|
| **PK** | CLEAN | `policy_id UUID PRIMARY KEY DEFAULT gen_random_uuid()` |
| **FK** | — | Self-contained reference data |
| **Indexes** | CLEAN | Partial unique `(object_type_fqn) WHERE active = true` |
| **Constraints** | CLEAN | `retain_days INT NOT NULL CHECK (retain_days > 0)`, `active BOOLEAN NOT NULL DEFAULT true` |
| **Naming** | CLEAN | Consistent |

**Verdict: CLEAN**

---

### 1.36 sem_reg_pub.active_requirement_profiles / active_proof_obligations / active_evidence_strategies (Migration 123)

| Aspect | Rating | Notes |
|--------|--------|-------|
| **PK** | CLEAN | Composite `(snapshot_set_id, fqn)` — consistent with other projection tables |
| **FK** | — | No FKs — projection pattern |
| **Indexes** | CLEAN | PK composite index |
| **Constraints** | CLEAN | All columns NOT NULL |
| **Naming** | CLEAN | Consistent with existing sem_reg_pub tables |

**Verdict: CLEAN**

---

### 1.37 "ob-poc".policy_version_bindings (Migration 123)

| Aspect | Rating | Notes |
|--------|--------|-------|
| **PK** | CLEAN | `binding_id UUID PRIMARY KEY DEFAULT gen_random_uuid()` |
| **FK** | CLEAN | `semos_snapshot_set_id` FK to `sem_reg.snapshot_sets`; 3 optional `*_snapshot_id` FKs to `sem_reg.snapshots` |
| **Indexes** | CLEAN | `(subject_kind, subject_id, computed_at DESC)`, `(semos_snapshot_set_id, computed_at DESC)`, partial index on `requirement_profile_snapshot_id WHERE NOT NULL` |
| **Constraints** | CLEAN | `subject_kind TEXT NOT NULL`, `subject_id UUID NOT NULL`, metadata defaults |
| **Naming** | CLEAN | Consistent |

**Verdict: CLEAN** — Well-designed runtime binding table with proper provenance FKs

---

### 1.38 "ob-poc".dsl_verbs (master-schema.sql)

| Aspect | Rating | Notes |
|--------|--------|-------|
| **PK** | CLEAN | UUID (uuidv7) |
| **FK** | — | Root table, no FKs |
| **Indexes** | CLEAN | UNIQUE `(domain, verb_name)`, GIN on `yaml_intent_patterns`, `intent_patterns`, tsvector search index |
| **Constraints** | CLEAN | Generated column `full_name` from domain + verb_name |
| **Naming** | CLEAN | Consistent |

**Verdict: CLEAN**

---

### 1.39 "ob-poc".verb_pattern_embeddings (master-schema.sql + migration 037)

| Aspect | Rating | Notes |
|--------|--------|-------|
| **PK** | CLEAN | UUID (uuidv7) |
| **FK** | — | No FK to `dsl_verbs` (verb_name is denormalized text, not a UUID ref) |
| **Indexes** | FLAG | **Two overlapping IVFFlat indexes**: `idx_pattern_embed_vector` (lists=10) and `idx_vpe_ivfflat` (lists=100). Redundant — only one should exist |
| **Constraints** | MINOR | No UNIQUE on `(verb_name, pattern_phrase)` — duplicate patterns are possible |
| **Naming** | CLEAN | Consistent |

**Verdict: FLAG** — Duplicate IVFFlat indexes waste space and confuse the query planner

---

### 1.40 "ob-poc".gleif_relationships (master-schema.sql)

| Aspect | Rating | Notes |
|--------|--------|-------|
| **PK** | CLEAN | UUID (uuidv7) |
| **FK** | FLAG | `parent_entity_id` and `child_entity_id` are entity references with **no declared FKs** to `entities` table |
| **Indexes** | CLEAN | Indexes on both entity ID columns |
| **Constraints** | MINOR | No staleness detection mechanism beyond `fetched_at` timestamp. No `valid_until` or `stale_after` column |
| **Naming** | CLEAN | Consistent |

**Verdict: FLAG** — Missing entity FKs; no staleness detection for external data

---

### 1.41 "ob-poc".gleif_sync_log (master-schema.sql)

| Aspect | Rating | Notes |
|--------|--------|-------|
| **PK** | CLEAN | UUID (uuidv7) |
| **FK** | FLAG | `entity_id` has **no FK** to `entities` |
| **Indexes** | MINOR | No index on `entity_id` for entity-scoped sync queries |
| **Constraints** | CLEAN | CHECK on `sync_status` |
| **Naming** | CLEAN | Consistent |

**Verdict: FLAG** — Missing entity FK and entity_id index

---

### 1.42 "ob-poc".screening_lists (master-schema.sql)

| Aspect | Rating | Notes |
|--------|--------|-------|
| **PK** | CLEAN | UUID |
| **FK** | — | Reference table |
| **Indexes** | MINOR | No UNIQUE on `list_code` — could allow duplicate list registrations |
| **Constraints** | CLEAN | Basic NOT NULLs |
| **Naming** | CLEAN | Consistent |

**Verdict: MINOR** — Missing unique constraint on list_code

---

### 1.43 "ob-poc".screening_requirements (master-schema.sql)

| Aspect | Rating | Notes |
|--------|--------|-------|
| **PK** | CLEAN | Composite `(risk_band, screening_type)` |
| **FK** | MINOR | `screening_type` has no FK to `screening_types.code` |
| **Indexes** | CLEAN | Composite PK index |
| **Constraints** | CLEAN | Appropriate |
| **Naming** | CLEAN | Consistent |

**Verdict: MINOR** — Missing FK to screening_types reference table

---

### 1.44 "ob-poc".screenings (master-schema.sql)

| Aspect | Rating | Notes |
|--------|--------|-------|
| **PK** | CLEAN | UUID |
| **FK** | FLAG | `workstream_id` has no visible FK. `screening_type` CHECK constraint duplicates `screening_types` reference data |
| **Indexes** | CLEAN | Status and type indexes |
| **Constraints** | CLEAN | CHECK on `status` and `screening_type` |
| **Naming** | CLEAN | Consistent |

**Verdict: MINOR** — Duplicated screening_type values in CHECK vs reference table

---

## 2. Cross-Table Consistency Findings

### 2.1 Enum Strategy Inconsistency

**Severity: FLAG**

The schema uses **two different enum strategies** without a clear boundary:

| Strategy | Used In | Examples |
|----------|---------|---------|
| **PG CREATE TYPE** | `sem_reg.snapshots` (078) | `governance_tier`, `trust_class`, `snapshot_status`, `change_type`, `object_type` |
| **TEXT + CHECK** | Agent tables (085), changesets (095), stewardship (097) | plan status, step status, review verdict, changeset status |

This creates risk:
- PG enums are type-safe at the column level but require `ALTER TYPE` to extend (cumbersome in migrations)
- TEXT CHECK is flexible but allows typos at the application layer if CHECK is temporarily dropped
- The changeset status CHECK has been **dropped and recreated 3 times** (095 → 097 → 099), each time widening the allowed values

**Recommendation:** Standardize on one approach. TEXT CHECK is pragmatically better for evolving status fields. PG enums are fine for stable classifications (governance_tier, trust_class). Document the boundary.

### 2.2 Cross-Schema FK Avoidance

**Severity: MINOR**

Tables in `stewardship` schema consistently avoid FKs to `sem_reg` schema tables (changesets, snapshots). This is defensible for schema independence but means:
- `stewardship.events.changeset_id` could reference a non-existent changeset
- `stewardship.basis_records.snapshot_id` could reference a non-existent snapshot
- `stewardship.viewport_manifests.overlay_changeset_id` could dangle

Application-level enforcement must be rigorous. Consider adding COMMENT statements documenting the intentional FK omission on these columns.

### 2.3 Archive Table Schema Drift

**Severity: CRITICAL**

`sem_reg_authoring.change_sets_archive` (migration 100) has `owner_id UUID` while the source table `sem_reg.changesets` has `owner_actor_id TEXT` (originally named in 095, no rename visible). This means:
- Non-UUID actor IDs cannot be archived (INSERT would fail or silently lose data)
- Column name changed from `owner_actor_id` to `owner_id`

**Recommendation:** Align archive table column type and name with the live table.

### 2.4 Projection Table Consistency

**Severity: CLEAN**

All `sem_reg_pub` projection tables follow a consistent pattern:
- Composite PK `(snapshot_set_id, fqn)`
- No FKs (rebuilt from source)
- `payload JSONB NOT NULL`
- `published_at TIMESTAMPTZ NOT NULL`

The 3 tables from migration 123 (`active_requirement_profiles`, `active_proof_obligations`, `active_evidence_strategies`) correctly follow this pattern, including the additional `snapshot_id UUID NOT NULL` column for per-row provenance.

### 2.5 Temporal Column Patterns

**Severity: MINOR**

Most tables use `created_at TIMESTAMPTZ NOT NULL DEFAULT now()`, but there is inconsistency in update tracking:
- `snapshot_sets` — only `created_at`, no `updated_at` (correct: immutable)
- `focus_states` — has `updated_at` (correct: mutable)
- `changesets` — has `created_at` but no explicit `updated_at` (status changes are tracked via review records)
- `outbox_events` — uses `claimed_at` and `processed_at` instead of generic `updated_at` (correct: domain-specific)

The inconsistency is mostly justified by domain semantics. No action needed.

### 2.6 GLEIF External ID Handling

**Severity: FLAG**

GLEIF data (`gleif_relationships`, `gleif_sync_log`) lacks:
- **FK enforcement** on entity references (entity_id, parent_entity_id, child_entity_id)
- **Staleness detection** beyond raw `fetched_at` timestamp — no `stale_after` interval, no `valid_until`, no TTL policy
- **External ID deduplication** — `gleif_relationships` has no UNIQUE constraint preventing duplicate relationship entries for the same parent/child pair
- **Sync log indexing** — `gleif_sync_log.entity_id` lacks an index for per-entity sync history queries

### 2.7 Document Polymorphism Boundaries

**Severity: CLEAN**

The three-plane document governance model is cleanly separated:

| Plane | Tables | Schema |
|-------|--------|--------|
| **Definition (artifact)** | `snapshots` with types `requirement_profile_def`, `proof_obligation_def`, `evidence_strategy_def` | `sem_reg` |
| **Published projections** | `active_requirement_profiles`, `active_proof_obligations`, `active_evidence_strategies` | `sem_reg_pub` |
| **Runtime bindings** | `policy_version_bindings` | `"ob-poc"` |

Cross-plane integrity is maintained via:
- FKs from `policy_version_bindings` to `sem_reg.snapshot_sets` and `sem_reg.snapshots`
- Expression indexes on `definition->>'fqn'` for active snapshot lookup (migration 122)

The boundary between governed definitions and runtime consumption is well-drawn.

### 2.8 Self-Referential Integrity

**Severity: CLEAN**

Self-referential patterns are consistently implemented:
- `snapshots.predecessor_id` → `snapshots.snapshot_id` (supersession chain)
- `changesets.supersedes_change_set_id` → `changesets.changeset_id`
- `changesets.superseded_by` → `changesets.changeset_id`
- `observations.supersedes` → `observations.observation_id` (linear chain with partial unique)
- `attribute_observations.supersedes` → `attribute_observations.observation_id`

All use FK constraints with appropriate indexes. The linear chain enforcement via partial unique indexes (`WHERE supersedes IS NOT NULL`) is a strong pattern preventing branching.

---

## 3. Severity-Tagged Issue List

### CRITICAL

| # | Table | Issue | Impact |
|---|-------|-------|--------|
| C-1 | `sem_reg_authoring.change_sets_archive` | `owner_id UUID` vs source `owner_actor_id TEXT` — type and name mismatch | Archive INSERT will fail for non-UUID actor IDs. Data loss risk during retention cleanup |

### FLAG

| # | Table | Issue | Impact |
|---|-------|-------|--------|
| F-1 | `sem_reg.changesets` | Status CHECK dropped/recreated 3 times (095, 097, 099) | Migration ordering sensitivity. Concurrent migration application risk |
| F-2 | `sem_reg.changeset_entries` | `object_type TEXT` unconstrained — no CHECK or enum | Invalid object_type values can be inserted without detection |
| F-3 | `sem_reg.decision_records` | `escalation_id` has no FK to `escalation_records` | Dangling escalation references possible |
| F-4 | `sem_reg.derivation_edges` | `input_snapshot_ids UUID[]` — no element-level FK | Stale/invalid snapshot IDs can accumulate in array |
| F-5 | `sem_reg.run_records` | `plan_id` and `step_id` have no FKs to `agent_plans`/`plan_steps` | Broken lineage references |
| F-6 | `stewardship.viewport_manifests` | `session_id` has no FK to `focus_states` | Orphan manifests for non-existent sessions |
| F-7 | `sem_reg_authoring.governance_audit_log` | `active_snapshot_set_id NOT NULL` but no FK | NOT NULL constraint implies strong reference but no DB enforcement |
| F-8 | `verb_pattern_embeddings` | Two overlapping IVFFlat indexes (lists=10 and lists=100) | Wasted disk space, potential query planner confusion |
| F-9 | `gleif_relationships` | `parent_entity_id` and `child_entity_id` have no FKs to `entities` | Orphan GLEIF relationships when entities are cleaned up |
| F-10 | `gleif_sync_log` | `entity_id` has no FK to `entities`, no index | No referential integrity, slow entity-scoped queries |
| F-11 | `sem_reg.document_instances` | `entity_id` has no FK to `"ob-poc".entities` | Cross-schema entity reference unenforceable |
| F-12 | Agent tables (085) vs core tables (078) | TEXT CHECK vs PG enum for status fields | Inconsistent enum strategy across the sem_reg schema family |

### MINOR

| # | Table | Issue | Impact |
|---|-------|-------|--------|
| M-1 | `sem_reg.plan_steps` | `verb_snapshot_id` has no FK to `snapshots` | Provenance chain relies on application enforcement |
| M-2 | `sem_reg.provenance_edges` | `source_type`/`target_type` are unconstrained TEXT | Invalid type values possible |
| M-3 | `screening_lists` | No UNIQUE on `list_code` | Duplicate list registrations possible |
| M-4 | `screening_requirements` | `screening_type` has no FK to `screening_types.code` | Reference table not enforced |
| M-5 | `screenings` | `screening_type` CHECK duplicates `screening_types` reference data | Maintenance burden if types change |
| M-6 | `verb_pattern_embeddings` | No UNIQUE on `(verb_name, pattern_phrase)` | Duplicate embeddings waste storage and can skew search results |
| M-7 | `gleif_relationships` + `gleif_sync_log` | No staleness detection beyond raw `fetched_at` | Stale external data not systematically detected |
| M-8 | `sem_reg.snapshot_sets` | No index on `created_at` | Temporal range queries must seq-scan |

### CLEAN (Notable Positives)

| # | Table/Pattern | What's Well Done |
|---|---------------|------------------|
| P-1 | `sem_reg_pub.active_snapshot_set` | Singleton pattern (boolean PK with CHECK true) is elegant |
| P-2 | `sem_reg.observations` / `attribute_observations` | Linear chain enforcement via partial unique index on `supersedes` prevents branching |
| P-3 | `document_instances` lifecycle guard trigger | Immutable field protection while allowing status updates — correct pattern |
| P-4 | `sem_reg_pub.*` projection tables | Consistent no-FK pattern, composite PKs, uniform column set |
| P-5 | `policy_version_bindings` | Proper FKs to both `snapshot_sets` and `snapshots` — strong provenance |
| P-6 | `sem_reg.snapshots` immutability trigger | Added in 090, prevents UPDATE/DELETE on published rows |
| P-7 | Content-addressed idempotency | `UNIQUE INDEX WHERE content_hash IS NOT NULL AND status NOT IN (...)` on changesets |
| P-8 | Expression indexes on `definition->>'fqn'` | Targeted indexes for active snapshot resolution by FQN (migration 122) |

---

## 4. Index Recommendation List

| Priority | Table | Recommended Index | Rationale |
|----------|-------|-------------------|-----------|
| **HIGH** | `verb_pattern_embeddings` | **DROP** `idx_pattern_embed_vector` (lists=10), keep `idx_vpe_ivfflat` (lists=100) | Duplicate IVFFlat index. lists=100 is better for 15K+ rows |
| **HIGH** | `gleif_sync_log` | `CREATE INDEX idx_gleif_sync_entity ON "ob-poc".gleif_sync_log(entity_id, synced_at DESC)` | Entity-scoped sync history queries currently require full scan |
| **MEDIUM** | `verb_pattern_embeddings` | `CREATE UNIQUE INDEX uq_vpe_verb_phrase ON "ob-poc".verb_pattern_embeddings(verb_name, pattern_phrase)` | Prevent duplicate embeddings for the same verb+phrase pair |
| **MEDIUM** | `screening_lists` | `CREATE UNIQUE INDEX uq_screening_list_code ON "ob-poc".screening_lists(list_code)` | Prevent duplicate list registrations |
| **MEDIUM** | `sem_reg.snapshot_sets` | `CREATE INDEX idx_snapshot_sets_created ON sem_reg.snapshot_sets(created_at DESC)` | Support temporal range queries on snapshot sets |
| **LOW** | `gleif_relationships` | `CREATE UNIQUE INDEX uq_gleif_rel_pair ON "ob-poc".gleif_relationships(parent_entity_id, child_entity_id, relationship_type)` | Prevent duplicate GLEIF relationship entries |
| **LOW** | `sem_reg.provenance_edges` | Add CHECK on `source_type` and `target_type` (e.g., `CHECK (source_type IN ('observation', 'document_instance', 'snapshot', 'external'))`) | Prevent invalid polymorphic type values |

---

## 5. Summary

| Severity | Count |
|----------|-------|
| CRITICAL | 1 |
| FLAG | 12 |
| MINOR | 8 |
| CLEAN | 30+ tables/patterns |

**Overall Assessment:** The SemOS metadata and infrastructure schema is well-designed with strong patterns (immutable snapshots, linear chains, projection independence, singleton pointers, content-addressed idempotency). The primary concerns are:

1. **One CRITICAL issue** — archive table type mismatch (C-1) that will cause runtime failures during retention cleanup
2. **Enum strategy inconsistency** (F-12) — PG enums vs TEXT CHECK mixed without documented boundary
3. **Changeset status CHECK fragility** (F-1) — dropped/recreated 3 times across migrations
4. **GLEIF integration gaps** (F-9, F-10, M-7) — missing FKs, indexes, and staleness detection for external data
5. **Duplicate IVFFlat index** (F-8) — wastes disk and confuses query planner

The document governance three-plane model (migration 122-123) is cleanly implemented with proper FK provenance chains. The self-referential integrity patterns (supersession chains, linear chain enforcement) are consistently applied and robust.
