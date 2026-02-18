# ob-poc Semantic OS — Gap Remediation Implementation Plan

**Claude Code Task Specification**

Version: 3.1 — February 2026
Predecessor: `semantic-os-implementation-todo-v2.md` (architecture spec), `SEMANTIC_OS_GAP_REMEDIATION_PLAN.md` (audit)
Author: Adam / Lead Solution Architect
Auditor: Claude Opus 4.6 (gap audit 2026-02-16, plan review 2026-02-18)
Peer Review: 2026-02-18 — 5 high-impact fixes applied (see Appendix D)

---

## Revision History

| Version | Date | Changes |
|---|---|---|
| 3.0 | 2026-02-18 | Initial 19-session plan from gap audit |
| 3.1 | 2026-02-18 | Peer review fixes: (1) S1 snapshot table agnosticism, (2) S4 immutability triggers + linear chain enforcement + graceful degradation + attribute_snapshot_id, (3) S5 EdgeClass collision warning, (4) Test filter convention standardised, (5) Document_instances lifecycle guard |

---

## How to Use This Document

This document is structured as **19 discrete Claude Code sessions**. Each session is independently completable, testable, and valuable. Sessions must execute in order — each depends on the outputs of its predecessors.

### Claude Code Execution Rules

1. **Read the entire session specification before writing any code.** Do not start coding after reading only the first paragraph.
2. **Every session has a GATE.** Do not consider the session complete until every gate criterion passes. Run the verification commands listed in the gate.
3. **Continuation mandate**: When a session contains multiple subtasks (e.g., S4 has "create tables" then "rewrite MCP tools"), you MUST complete ALL subtasks. After finishing each subtask, emit `"→ Subtask N complete (X%). IMMEDIATELY proceeding to subtask N+1."` and continue. Do NOT stop, summarise, or ask for confirmation between subtasks.
4. **Test locations are explicit.** Every session specifies where tests go. Follow the specification exactly.
5. **The six foundational invariants (Session S1) are regression tests.** Run them at the end of every subsequent session. If any invariant test fails, stop and fix before proceeding.
6. **Do not invent domain content.** Where a session says "placeholder content" or "example content", create structurally correct but obviously placeholder data (e.g., `"PLACEHOLDER: real taxonomy nodes TBD"`). The Lead Architect populates real domain content separately.
7. **Test filter convention**: Gate commands use `cargo test --features vnext-repl -- <module_name>` where `<module_name>` is a substring filter matched against the full test path. If a gate command fails to find tests, verify the actual module path with `cargo test --features vnext-repl -- --list 2>&1 | grep <module_name>` and adjust the filter. The intent is always "run all tests in that module" — adapt the filter to match the actual layout.

### Reference Files

These files MUST be accessible to Claude Code. Verify before starting:

- `docs/semantic-os-v1.1.md` — architecture vision (the "what")
- `docs/semantic-os-implementation-todo-v2.md` — phase spec (the "how")
- `docs/SEMANTIC_OS_GAP_REMEDIATION_PLAN.md` — gap audit (the "where we are")
- This file — session plan (the "execution order")

### Collision Warning: `sem_reg.observations` vs `attribute_observations`

**CRITICAL**: The existing `attribute_observations` table in the ob-poc master schema is an operational data table with its own lifecycle semantics. `sem_reg.observations` (created in Session S4) is a **new, separate table** in the `sem_reg` schema with different semantics: immutable INSERT-only rows, supersession chains via the `supersedes` column, and snapshot-pinned evidence references.

**Do NOT**: modify, migrate, reference, join against, or conflate `attribute_observations` with `sem_reg.observations`. They are unrelated.

---

## Foundational Invariants (NON-NEGOTIABLE)

These six invariants are tested in Session S1 and regressed in every subsequent session.

| # | Invariant | Mechanical Test |
|---|-----------|-----------------|
| 1 | **No in-place updates for registry snapshot tables.** Every change produces a new immutable snapshot. INSERT of new snapshot + UPDATE of predecessor's `effective_until` only. | Attempt SQL UPDATE on snapshot body column → must fail or be rejected |
| 2 | **Execution/decision/derivation records pin snapshot IDs.** `snapshot_manifest` on DecisionRecord is mandatory. | Create DecisionRecord without `snapshot_manifest` → must fail validation |
| 3 | **Proof Rule is mechanically enforced.** `governance_tier_minimum` + `trust_class_minimum` checked on evidence requirements; `predicate_trust_minimum` checked on policy predicates. | Publish governed PolicyRule referencing Operational attribute → must fail with remediation message |
| 4 | **ABAC / security labels apply to both tiers identically.** Governance tier affects workflow rigour, NOT security posture. | ABAC evaluation of PII attribute → identical masking/export result regardless of `governance_tier` |
| 5 | **Operational-tier snapshots do not require governed approval workflows.** Auto-approve semantics (`approved_by = "auto"`), still recorded. | Publish operational snapshot → must succeed without approval gate; `approved_by` = `"auto"` |
| 6 | **Derived/composite attributes require a DerivationSpec.** No ad-hoc derived values. Security inheritance computed from inputs. `evidence_grade = Prohibited` for operational derivations. | Create derived AttributeDef without DerivationSpec → must fail. Operational derivation with `evidence_grade != Prohibited` → must fail |

---

## Session S1 — Invariant Test Harness

**Goal**: Create the regression test suite that mechanically verifies all six foundational invariants. Every subsequent session runs these tests as a gate.

### S1.1 Create invariant test file

Create `rust/tests/sem_reg_invariants.rs` with six test functions, one per invariant:

```rust
// Test locations: rust/tests/sem_reg_invariants.rs
// All tests require DATABASE_URL and the database feature flag
// All tests are #[ignore] (run explicitly, not in default cargo test)

#[tokio::test]
#[ignore]
async fn invariant_1_no_in_place_snapshot_updates() { ... }

#[tokio::test]
#[ignore]
async fn invariant_2_decision_records_require_snapshot_manifest() { ... }

#[tokio::test]
#[ignore]
async fn invariant_3_proof_rule_rejects_operational_in_governed_policy() { ... }

#[tokio::test]
#[ignore]
async fn invariant_4_abac_identical_across_governance_tiers() { ... }

#[tokio::test]
#[ignore]
async fn invariant_5_operational_snapshots_auto_approve() { ... }

#[tokio::test]
#[ignore]
async fn invariant_6_derived_attributes_require_derivation_spec() { ... }
```

### S1.2 Implementation notes per invariant test

**Invariant 1**: The ob-poc codebase uses a central `sem_reg.snapshots` table (with `object_type` discriminator and `body` JSONB column) rather than per-registry tables. If this is the case, the test targets that table. If the codebase instead uses per-registry tables (e.g., `sem_reg.attribute_defs`, `sem_reg.verb_contracts`), target each one.

**Before writing the test, inspect the actual schema:**
```bash
# Discover the real snapshot storage pattern
psql data_designer -c "\dt sem_reg.*"
psql data_designer -c "\d sem_reg.snapshots" 2>/dev/null || echo "No central snapshots table"
psql data_designer -c "\d sem_reg.attribute_defs" 2>/dev/null || echo "No per-registry attribute_defs table"
```

**Then implement whichever applies:**

- **If central `sem_reg.snapshots` table exists**: Use `SnapshotStore` to publish a snapshot. Then attempt `UPDATE sem_reg.snapshots SET body = '{}' WHERE snapshot_id = <id>`. Must fail.
- **If per-registry tables exist**: Test against each registry table (e.g., `UPDATE sem_reg.attribute_defs SET name = 'tampered' WHERE snapshot_id = <id>`). Must fail for Active snapshots.
- **In either case**: `UPDATE ... SET effective_until = <timestamp> WHERE snapshot_id = <predecessor_id>` **must be allowed** — this is how the predecessor gets closed when a new snapshot is published.

**Enforcement**: If neither application layer nor DB trigger currently prevents the UPDATE, create a shared PostgreSQL trigger function and attach it to the relevant table(s):

```sql
CREATE OR REPLACE FUNCTION sem_reg.prevent_snapshot_mutation() RETURNS TRIGGER AS $$
BEGIN
    -- Allow only effective_until updates on Active snapshots (predecessor closure)
    IF OLD.status = 'active' AND (
        NEW.body IS DISTINCT FROM OLD.body OR
        NEW.status IS DISTINCT FROM OLD.status OR
        NEW.snapshot_id IS DISTINCT FROM OLD.snapshot_id OR
        NEW.object_id IS DISTINCT FROM OLD.object_id
    ) THEN
        RAISE EXCEPTION 'Cannot mutate Active snapshot %. Use publish_snapshot() to create a new version.', OLD.snapshot_id;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Attach to the actual snapshot table(s):
CREATE TRIGGER trg_prevent_snapshot_mutation
    BEFORE UPDATE ON sem_reg.snapshots  -- or sem_reg.attribute_defs, etc.
    FOR EACH ROW EXECUTE FUNCTION sem_reg.prevent_snapshot_mutation();
```

The test verifies: (a) UPDATE to body/status column on Active snapshot → exception, (b) UPDATE to `effective_until` on Active snapshot → allowed.

**Invariant 2**: Construct a `DecisionRecord` with `snapshot_manifest = None` or empty. Call the validation/insert path. Assert it returns an error. If the current code does not validate this, add the validation to the `DecisionRecord` insert path and then verify.

**Invariant 3**: Seed a governed `PolicyRule` whose predicate references an `AttributeDef` with `governance_tier = Operational`. Run the Proof Rule gate (`check_proof_rule` in `gates.rs` or equivalent). Assert it produces a `GateFailure`. If the gate is a stub, note this — Session S6 will fix it, but the test should exist now (and can be `#[should_panic]` or assert-on-error as appropriate).

**Invariant 4**: Create two `AttributeDef` snapshots with identical `SecurityLabel` (containing PII, masking requirements, jurisdiction constraints) but different `governance_tier` values (one Governed, one Operational). Run `evaluate_access()` against both with the same `ActorContext`. Assert the `AccessDecision` is identical (same masking plan, same export controls, same residency constraints).

**Invariant 5**: Publish a snapshot with `governance_tier = Operational`. Assert it succeeds without any approval workflow blocking. Assert `approved_by = "auto"` on the resulting snapshot.

**Invariant 6**: Attempt to publish an `AttributeDef` with `kind = Derived` but no corresponding `DerivationSpec`. Assert the publish gate rejects it. Then create a `DerivationSpec` with `governance_tier = Operational` and `evidence_grade != Prohibited`. Assert the publish gate rejects it.

### S1.3 Helper utilities

Create a test helper module `rust/tests/sem_reg_test_helpers.rs` (or inline in the test file) with:

- `setup_test_db()` → connection pool to test database, runs pending migrations
- `seed_minimal_registry()` → creates the minimum viable seed data (one attribute, one verb, one entity type, one policy) needed across multiple tests
- `cleanup_test_data()` → truncates `sem_reg.*` tables (NOT operational tables)

### Gate

```bash
# All six invariant tests compile and run
DATABASE_URL="postgresql:///data_designer" \
  cargo test --features database --test sem_reg_invariants -- --ignored --nocapture

# Expected: 6 tests, all pass (some may require fixes to production code to enforce invariants)
```

→ IMMEDIATELY proceed to Session S2.

---

## Session S2 — Fix Stub Gates (P0-2, P0-3)

**Goal**: Make `check_verb_surface_disclosure` and `check_type_correctness` produce real gate failures instead of returning empty vectors.

**Spec reference**: `semantic-os-implementation-todo-v2.md` §6.1
**Gap reference**: `SEMANTIC_OS_GAP_REMEDIATION_PLAN.md` P0-2, P0-3

### S2.1 Fix `check_verb_surface_disclosure` (P0-2)

**File**: `rust/src/sem_reg/gates_technical.rs`

**Current behaviour**: Builds a `declared_surface` HashSet but returns `Vec<GateFailure>` unconditionally empty.

**Required behaviour**: For each attribute referenced in the verb's implementation metadata (scanner output, side-effects declarations), check it appears in the verb contract's `consumes`, `produces`, or `side_effects`. For any undisclosed reference, emit a `GateFailure` with:

```rust
GateFailure {
    gate_type: "verb_surface_disclosure".into(),
    severity: GateSeverity::Error,
    object_ref: verb_snapshot_id,
    snapshot_id: verb_snapshot_id,
    message: format!("Verb '{}' references attribute '{}' not declared in I/O surface", verb_name, undisclosed_attr),
    remediation_hint: format!("Add '{}' to the verb's consumes, produces, or side_effects declaration", undisclosed_attr),
    regulatory_reference: None,
}
```

**Test location**: `rust/src/sem_reg/gates_technical.rs` — add to existing `#[cfg(test)] mod tests` block:

```rust
#[test]
fn test_verb_surface_disclosure_catches_undisclosed() { ... }

#[test]
fn test_verb_surface_disclosure_passes_fully_disclosed() { ... }
```

### S2.2 Fix `check_type_correctness` (P0-3)

**File**: `rust/src/sem_reg/gates_technical.rs`

**Current behaviour**: Only checks `consumes` arm. The `produces` check is a no-op. The `entity_type_fqn` lookup is a no-op (`let _ = entity_type_fqn`).

**Required behaviour**: Three checks, all producing `GateFailure` on mismatch:

1. **`consumes`** (existing, keep): for each consumed attribute FQN, verify it exists in the attribute dictionary. ✓ Already works.
2. **`produces`** (currently no-op, fix): for each produced attribute FQN, verify it exists in the attribute dictionary. Emit `GateFailure` if unregistered.
3. **`entity_type_fqn`** (currently no-op, fix): for each entity type FQN referenced in verb I/O, verify the entity type is registered. Emit `GateFailure` if unregistered.
4. **Type compatibility** (new): where both verb I/O `type_spec` and attribute `type_spec` are specified, check compatibility (exact match or coercible). Emit `GateFailure` with severity `Warning` on mismatch (not Error — type specs may be partially specified during early onboarding).

**Test location**: `rust/src/sem_reg/gates_technical.rs` — add to existing `#[cfg(test)] mod tests`:

```rust
#[test]
fn test_type_correctness_catches_unregistered_produces() { ... }

#[test]
fn test_type_correctness_catches_unregistered_entity_type() { ... }

#[test]
fn test_type_correctness_warns_type_mismatch() { ... }

#[test]
fn test_type_correctness_passes_fully_registered() { ... }
```

### Gate

```bash
# Unit tests for the two fixed gates
# NOTE: cargo test filter is substring-matched against full test path.
# If the module path is sem_reg::gates_technical::tests::*, this matches.
# If tests don't run, try: cargo test --features vnext-repl -- test_verb_surface
cargo test --features vnext-repl -- gates_technical

# Invariant regression
DATABASE_URL="postgresql:///data_designer" \
  cargo test --features database --test sem_reg_invariants -- --ignored --nocapture
```

→ IMMEDIATELY proceed to Session S3.

---

## Session S3 — Fix `tier_allowed()` for Normal Mode (P0-4)

**Goal**: Make `EvidenceMode::Normal` respect the view's `includes_operational` flag.

**Spec reference**: `semantic-os-implementation-todo-v2.md` §7.3
**Gap reference**: `SEMANTIC_OS_GAP_REMEDIATION_PLAN.md` P0-4

### S3.1 Analyse the call chain

**File**: `rust/src/sem_reg/context_resolution.rs`

Before making changes, trace the call chain from `resolve_context()` down to `tier_allowed()`. Identify every function boundary the `includes_operational` flag needs to pass through. Document the chain in a comment at the top of the modified function.

### S3.2 Thread `includes_operational` into `tier_allowed()`

Modify `tier_allowed()` signature to accept the view's `includes_operational: bool` (or the full `ViewDef` reference if other view properties are needed downstream). Update all call sites.

**Required behaviour for Normal mode**:

| Candidate tier | Candidate trust_class | `includes_operational` | Result |
|---|---|---|---|
| Governed | Proof | any | Allow |
| Governed | DecisionSupport | any | Allow |
| Governed | Convenience | any | Allow, tag `usable_for_proof = false` |
| Operational | any | `true` | Allow, tag `usable_for_proof = false` |
| Operational | any | `false` | **Deny** (currently returns `true` — this is the bug) |

**Strict mode** should already be correct (verify — do not assume).

**Exploratory mode** allows all tiers with annotations (verify).

**Governance mode** focuses on coverage metrics (verify).

### S3.3 Tests

**Test location**: `rust/src/sem_reg/context_resolution.rs` — add to existing `#[cfg(test)] mod tests`:

```rust
#[test]
fn test_normal_mode_excludes_operational_when_view_disallows() { ... }

#[test]
fn test_normal_mode_includes_operational_when_view_allows() { ... }

#[test]
fn test_normal_mode_tags_operational_not_usable_for_proof() { ... }

#[test]
fn test_strict_mode_unchanged() { ... }
```

### Gate

```bash
cargo test --features vnext-repl -- context_resolution

# Invariant regression
DATABASE_URL="postgresql:///data_designer" \
  cargo test --features database --test sem_reg_invariants -- --ignored --nocapture
```

→ IMMEDIATELY proceed to Session S4.

---

## Session S4 — Evidence Instance Tables + MCP Tool Realignment (P1-1 + P0-1)

**Goal**: Create the four evidence instance tables AND rewrite the three misaligned evidence MCP tools in a single atomic session. Both must be complete — tables without consumers or consumers without tables are both useless.

**Spec reference**: `semantic-os-implementation-todo-v2.md` §3.2, §8.3
**Gap reference**: `SEMANTIC_OS_GAP_REMEDIATION_PLAN.md` P1-1, P0-1

### ⚠️ Collision Warning (repeated for emphasis)

**`sem_reg.observations` is NOT `attribute_observations`.** The existing `attribute_observations` table in the master schema is unrelated. Do not modify, reference, or join against it.

### S4.1 Create migration

**File**: Next available migration number (check `migrations/` directory). Name: `sem_reg_NNN_evidence_instances.sql`

Create four tables in the `sem_reg` schema:

```sql
-- 1. Observations: immutable INSERT-only, supersession chain
CREATE TABLE sem_reg.observations (
    obs_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    subject_ref UUID NOT NULL,
    attribute_id UUID NOT NULL,
    attribute_snapshot_id UUID,          -- Pin to exact attribute definition version for forensic replay
    value_ref JSONB NOT NULL,
    source TEXT NOT NULL,
    confidence NUMERIC(3,2) CHECK (confidence BETWEEN 0 AND 1),
    observed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    supporting_doc_ids UUID[] DEFAULT '{}',
    governance_tier VARCHAR(20) NOT NULL DEFAULT 'operational',
    security_label JSONB,
    supersedes UUID REFERENCES sem_reg.observations(obs_id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_obs_subject_attr_ts
    ON sem_reg.observations(subject_ref, attribute_id, observed_at DESC);

-- Enforce INSERT-only on observations (immutable evidence records)
CREATE OR REPLACE FUNCTION sem_reg.prevent_observation_mutation() RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'UPDATE' THEN
        RAISE EXCEPTION 'Observations are immutable. Create a new observation with supersedes = % instead.', OLD.obs_id;
    ELSIF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'Observations cannot be deleted. obs_id = %', OLD.obs_id;
    END IF;
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_observations_immutable
    BEFORE UPDATE OR DELETE ON sem_reg.observations
    FOR EACH ROW EXECUTE FUNCTION sem_reg.prevent_observation_mutation();

-- 2. Document instances: lifecycle table — UPDATE allowed for status transitions only
CREATE TABLE sem_reg.document_instances (
    doc_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    doc_type_id UUID NOT NULL,
    storage_ref TEXT,
    extracted_fields JSONB DEFAULT '{}',
    source_actor TEXT NOT NULL,
    received_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    validated_at TIMESTAMPTZ,
    expiry TIMESTAMPTZ,
    retention_until TIMESTAMPTZ,
    security_label JSONB,
    status VARCHAR(20) NOT NULL DEFAULT 'received'
);

-- Document instances allow UPDATE only to lifecycle fields: validated_at, expiry, retention_until, status
-- All other columns are write-once at INSERT
CREATE OR REPLACE FUNCTION sem_reg.guard_document_instance_mutation() RETURNS TRIGGER AS $$
BEGIN
    IF NEW.doc_type_id IS DISTINCT FROM OLD.doc_type_id
       OR NEW.storage_ref IS DISTINCT FROM OLD.storage_ref
       OR NEW.extracted_fields IS DISTINCT FROM OLD.extracted_fields
       OR NEW.source_actor IS DISTINCT FROM OLD.source_actor
       OR NEW.received_at IS DISTINCT FROM OLD.received_at
       OR NEW.security_label IS DISTINCT FROM OLD.security_label
    THEN
        RAISE EXCEPTION 'Only lifecycle fields (validated_at, expiry, retention_until, status) may be updated on document_instances. doc_id = %', OLD.doc_id;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_document_instance_guard
    BEFORE UPDATE ON sem_reg.document_instances
    FOR EACH ROW EXECUTE FUNCTION sem_reg.guard_document_instance_mutation();

-- 3. Provenance edges: immutable append-only lineage
CREATE TABLE sem_reg.provenance_edges (
    edge_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    from_ref UUID NOT NULL,
    to_ref UUID NOT NULL,
    method VARCHAR(20) NOT NULL CHECK (method IN ('Human','OCR','API','Derived','Attested')),
    verb_id UUID,
    edge_timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    confidence NUMERIC(3,2)
);
CREATE INDEX idx_prov_from ON sem_reg.provenance_edges(from_ref);
CREATE INDEX idx_prov_to ON sem_reg.provenance_edges(to_ref);

-- Enforce INSERT-only on provenance edges (immutable lineage)
CREATE TRIGGER trg_provenance_immutable
    BEFORE UPDATE OR DELETE ON sem_reg.provenance_edges
    FOR EACH ROW EXECUTE FUNCTION sem_reg.prevent_observation_mutation();
    -- Reuses the same function — generic "this table is immutable" guard

-- 4. Retention policies: definitional, INSERT-only
CREATE TABLE sem_reg.retention_policies (
    retention_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    doc_type_id UUID NOT NULL,
    retention_window_days INTEGER NOT NULL,
    jurisdiction VARCHAR(10),
    regulatory_reference JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Enforce INSERT-only on retention policies (definitional records)
CREATE TRIGGER trg_retention_immutable
    BEFORE UPDATE OR DELETE ON sem_reg.retention_policies
    FOR EACH ROW EXECUTE FUNCTION sem_reg.prevent_observation_mutation();
```

### S4.2 Create Rust types

**File**: `rust/src/sem_reg/evidence_instances.rs` (new file)

```rust
// Structs: Observation, DocumentInstance, ProvenanceEdge, RetentionPolicy
// Each with Serialize/Deserialize derives
// Observation must enforce: supersedes chain, INSERT-only semantics
// ProvenanceMethod enum: Human, OCR, API, Derived, Attested

pub struct Observation {
    pub obs_id: Uuid,
    pub subject_ref: Uuid,
    pub attribute_id: Uuid,
    pub attribute_snapshot_id: Option<Uuid>,  // Pin to exact AttributeDef version for forensic replay
    pub value_ref: serde_json::Value,
    pub source: String,
    pub confidence: Option<f64>,
    pub observed_at: DateTime<Utc>,
    pub supporting_doc_ids: Vec<Uuid>,
    pub governance_tier: GovernanceTier,
    pub security_label: Option<SecurityLabel>,
    pub supersedes: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}
```

The `attribute_snapshot_id` is optional but recommended: when populated, it pins the exact `AttributeDef` version that was active when the observation was recorded. This enables forensic replay without needing a separate `resolve_at(attribute_id, observed_at)` call during audits. The MCP handler (S4.5) should resolve and populate this automatically when recording an observation.

Add `mod evidence_instances;` to `rust/src/sem_reg/mod.rs`.

### S4.3 Implement observation queries

In `evidence_instances.rs`, implement:

**Linear chain invariant**: The supersession chain for `(subject_ref, attribute_id)` is strictly linear — each new observation supersedes the current latest. `insert_observation()` enforces this by checking that if `supersedes` is `Some(id)`, then `id` is the most recent observation for the pair. This invariant keeps the point-in-time query simple.

```rust
/// Insert a new observation. Enforces linear supersession chain:
/// if superseding, the superseded obs_id MUST be the current latest
/// for (subject_ref, attribute_id). Rejects out-of-order supersession.
/// Automatically resolves attribute_snapshot_id from the registry if not provided.
pub async fn insert_observation(pool: &PgPool, obs: &NewObservation) -> Result<Observation> {
    // 1. If supersedes is Some: verify it matches the latest obs for the pair
    // 2. If supersedes is None but observations exist for the pair: error —
    //    caller must explicitly supersede (prevents silent parallel chains)
    // 3. If no prior observations exist: insert as chain root (supersedes = None OK)
    // 4. Resolve attribute_snapshot_id from registry if not provided
}

/// Point-in-time query: returns the active observation for (subject_ref, attribute_id)
/// at the given timestamp. Because we enforce a linear chain, this is simply:
///   SELECT * FROM sem_reg.observations
///   WHERE subject_ref = $1 AND attribute_id = $2 AND observed_at <= $3
///   ORDER BY observed_at DESC LIMIT 1
/// No recursive chain traversal needed.
pub async fn resolve_observation_at(
    pool: &PgPool,
    subject_ref: Uuid,
    attribute_id: Uuid,
    as_of: DateTime<Utc>,
) -> Result<Option<Observation>>

/// Returns the full supersession chain for (subject_ref, attribute_id),
/// ordered newest-first.
pub async fn observation_chain(
    pool: &PgPool,
    subject_ref: Uuid,
    attribute_id: Uuid,
) -> Result<Vec<Observation>>
```

### S4.4 Rename existing misplaced function (preserve-and-rename)

**CRITICAL**: The current `handle_record_observation()` body in `rust/src/sem_reg/agent/mcp_tools.rs` writes to `sem_reg.derivation_edges`. This behaviour is **correct for lineage recording** — it is only misnamed.

1. Copy the current function body to a new function `record_lineage_edge()` (or `handle_record_lineage_edge()`) in the same file or in a lineage-specific module.
2. Verify the lineage function still works after the rename.
3. Then rewrite `handle_record_observation()` (see S4.5).

**Do NOT delete the existing logic.** It is correct code in the wrong function.

### S4.5 Rewrite three evidence MCP tool handlers

**File**: `rust/src/sem_reg/agent/mcp_tools.rs`

**`handle_record_observation()`** — Rewrite to:
1. Parse params: `subject_ref`, `attribute_id`, `value`, `source`, `confidence`, `supporting_docs`, `governance_tier`
2. If `governance_tier = Governed`, verify `actor_type` has steward-level authority (from ActorContext)
3. Resolve the current active `AttributeDef` snapshot for `attribute_id` → extract its `snapshot_id` for `attribute_snapshot_id` (forensic pinning)
4. Look up the latest observation for `(subject_ref, attribute_id)` — if exists, set `supersedes` to its `obs_id` (linear chain enforcement from S4.3)
5. Call `insert_observation()` from S4.3 (which enforces linear chain and validates supersession)
6. Return the created `Observation` as JSON

**`handle_check_freshness()`** — Rewrite to:
1. Parse params: `subject_ref`, optional `attribute_id`
2. Query active observations for the subject (all attributes, or specific attribute)
3. For each observation, attempt to look up the applicable `EvidenceRequirement` (from `sem_reg.evidence` / policy rules) to find `freshness_window`
4. **Graceful degradation**: If no `EvidenceRequirement` exists for the attribute (policies not yet seeded), return the observation with `freshness_window: null, is_fresh: "unknown", reason: "no evidence requirement registered for this attribute"`. Do NOT return `is_fresh: true` — that would create false confidence.
5. Where `freshness_window` is available: compare observation `observed_at` + `freshness_window` against `now()`
6. Return freshness status per observation: `{ attribute_id, observed_at, freshness_window, is_fresh, expires_at }` where `is_fresh` is `true | false | "unknown"`

**`handle_identify_gaps()`** — Rewrite to:
1. Parse params: `case_id`, optional `view_name`
2. Attempt to resolve applicable `PolicyRule`s for the case (from context resolution or direct lookup)
3. **Graceful degradation**: If no applicable `PolicyRule`s exist (policies not yet seeded), return `{ gaps: [], policies_found: 0, status: "no_policies_registered", message: "No policy rules found for this case. Evidence gaps cannot be assessed until policies are seeded." }`. Do NOT return an empty gaps list without the signal — an empty list must mean "all evidence satisfied", not "we didn't check".
4. Where policies exist: extract `EvidenceRequirement`s from those policies
5. For each requirement: query `sem_reg.observations` for `(subject_ref, required_attribute)` — check if a fresh observation exists
6. Return gaps: `{ policy_id, requirement, attribute_id, status: "missing" | "stale" | "present" }` plus `policies_found` count

### S4.6 Tests

**Unit tests** in `rust/src/sem_reg/evidence_instances.rs` `#[cfg(test)] mod tests`:
```rust
#[test]
fn test_observation_serialization() { ... }
```

**Integration tests** in `rust/tests/sem_reg_integration.rs` (add new test functions):
```rust
#[tokio::test]
#[ignore]
async fn test_observation_insert_and_supersession_chain() { ... }

#[tokio::test]
#[ignore]
async fn test_observation_point_in_time_resolution() { ... }

#[tokio::test]
#[ignore]
async fn test_observation_linear_chain_enforcement() {
    // Insert obs1 for (subject, attr). Insert obs2 superseding obs1 (OK).
    // Attempt to insert obs3 superseding obs1 (not the latest) → must fail.
    // This enforces the linear chain invariant from S4.3.
}

#[tokio::test]
#[ignore]
async fn test_observation_immutability_trigger() {
    // Insert an observation. Attempt UPDATE → must fail with trigger exception.
    // Attempt DELETE → must fail with trigger exception.
}

#[tokio::test]
#[ignore]
async fn test_provenance_edge_immutability_trigger() {
    // Insert a provenance edge. Attempt UPDATE → must fail.
    // Attempt DELETE → must fail.
}

#[tokio::test]
#[ignore]
async fn test_document_instance_lifecycle_guard() {
    // Insert a document instance. UPDATE status → OK.
    // UPDATE doc_type_id → must fail (non-lifecycle column).
}

#[tokio::test]
#[ignore]
async fn test_mcp_record_observation_writes_to_observations_table() { ... }

#[tokio::test]
#[ignore]
async fn test_mcp_record_observation_populates_attribute_snapshot_id() {
    // Record an observation via MCP. Verify attribute_snapshot_id is populated
    // with the current active AttributeDef snapshot for that attribute.
}

#[tokio::test]
#[ignore]
async fn test_mcp_check_freshness_graceful_without_policies() {
    // Call check_freshness when no EvidenceRequirements exist.
    // Must return is_fresh: "unknown", not is_fresh: true.
}

#[tokio::test]
#[ignore]
async fn test_mcp_check_freshness_queries_observations() { ... }

#[tokio::test]
#[ignore]
async fn test_mcp_identify_gaps_graceful_without_policies() {
    // Call identify_gaps when no PolicyRules exist.
    // Must return policies_found: 0 and status: "no_policies_registered",
    // NOT an empty gaps list (which would imply "all satisfied").
}

#[tokio::test]
#[ignore]
async fn test_mcp_identify_gaps_compares_policy_vs_observations() { ... }

#[tokio::test]
#[ignore]
async fn test_lineage_edge_still_works_after_rename() { ... }
```

### Gate

```bash
# Migration applies cleanly
sqlx migrate run

# Unit tests
cargo test --features vnext-repl -- evidence_instances

# Integration tests (all six new + existing)
DATABASE_URL="postgresql:///data_designer" \
  cargo test --features database --test sem_reg_integration -- --ignored --nocapture

# Verify the renamed lineage function still works
DATABASE_URL="postgresql:///data_designer" \
  cargo test --features database --test sem_reg_integration -- test_lineage_edge_still_works --ignored --nocapture

# Invariant regression
DATABASE_URL="postgresql:///data_designer" \
  cargo test --features database --test sem_reg_invariants -- --ignored --nocapture
```

→ IMMEDIATELY proceed to Session S5.

---

## Session S5 — RelationshipTypeDefBody (P1-2)

**Goal**: Make `RelationshipTypeDef` a first-class registry object with body struct, registry methods, and CLI support.

**Spec reference**: `semantic-os-implementation-todo-v2.md` §1.2
**Gap reference**: `SEMANTIC_OS_GAP_REMEDIATION_PLAN.md` P1-2

### S5.1 Create the body struct

**File**: `rust/src/sem_reg/relationship_type_def.rs` (new file)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationshipTypeDefBody {
    pub name: String,
    pub description: Option<String>,
    pub source_entity_type: String,   // FQN of source EntityTypeDef
    pub target_entity_type: String,   // FQN of target EntityTypeDef
    pub edge_class: EdgeClass,
    pub directionality: Directionality,
    pub cardinality: Cardinality,
    pub constraints: Vec<serde_json::Value>,
    pub semantics: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EdgeClass {
    Structural,   // ownership / hierarchy (FK to parent)
    Derivation,   // computed from inputs
    Reference,    // FK to document / evidence
    Association,  // FK to lookup / taxonomy
    Temporal,     // time-bounded relationship
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Directionality { Unidirectional, Bidirectional }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Cardinality { OneToOne, OneToMany, ManyToMany }
```

Add `mod relationship_type_def;` to `mod.rs`. Wire `ObjectType::RelationshipTypeDef` to deserialise this body.

### ⚠️ EdgeClass Naming Collision Warning

The codebase may already have `EdgeClass` enums in other modules (e.g., `view_def.rs` for layout edge classes, `derivation.rs` for lineage edge classes, `sem_reg.derivation_edges` table). Before creating this enum:

1. `grep -rn "EdgeClass" rust/src/sem_reg/` to find existing definitions
2. If an identical `EdgeClass` enum already exists with the same variants → **reuse it** (import from the shared location, do not duplicate)
3. If a different `EdgeClass` exists with different variants → **namespace this one** as `RelationshipEdgeClass` to avoid confusion. Update the `RelationshipTypeDefBody` field accordingly.
4. Ensure all `EdgeClass` variants serialize with the same casing convention (e.g., `"Structural"` not `"structural"`) across all usages. Mixed serialization will cause silent JSONB mismatches.

### S5.2 Registry service methods

Add to `RegistryService` (or the `registry.rs` facade):

- `publish_relationship_type_def(fqn, body, governance_tier, security_label) -> SnapshotId`
- `resolve_relationship_type_def(fqn, as_of?) -> Option<(SnapshotMeta, RelationshipTypeDefBody)>`

### S5.3 CLI command

**File**: `rust/xtask/src/sem_reg.rs`

Add `rel-describe <fqn>` subcommand. Output: formatted display of the relationship type definition including source/target entity types, edge class, directionality, cardinality, and constraints.

### S5.4 Tests

**Unit tests** in `rust/src/sem_reg/relationship_type_def.rs`:
```rust
#[test]
fn test_relationship_type_def_body_roundtrip() { ... }

#[test]
fn test_edge_class_serialization() { ... }
```

**Integration tests** in `rust/tests/sem_reg_integration.rs`:
```rust
#[tokio::test]
#[ignore]
async fn test_publish_and_resolve_relationship_type_def() { ... }
```

### Gate

```bash
cargo test --features vnext-repl -- relationship_type_def
DATABASE_URL="postgresql:///data_designer" \
  cargo test --features database --test sem_reg_integration -- test_publish_and_resolve_relationship -- --ignored --nocapture
cargo x sem-reg rel-describe "test.relationship" || echo "Expected: no data yet, command runs without panic"

# Invariant regression
DATABASE_URL="postgresql:///data_designer" \
  cargo test --features database --test sem_reg_invariants -- --ignored --nocapture
```

→ IMMEDIATELY proceed to Session S6.

---

## Session S6 — Five Missing Publish Gates (P1-3)

**Goal**: Implement the five publish gates that the gap audit identified as missing.

**Spec reference**: `semantic-os-implementation-todo-v2.md` §6.1, §6.2
**Gap reference**: `SEMANTIC_OS_GAP_REMEDIATION_PLAN.md` P1-3

### S6.1 Technical gates (both tiers)

**File**: `rust/src/sem_reg/gates_technical.rs`

**Gate 1: `check_continuation_completeness`**
- For verbs with `exec_mode = DurableStart`: query the registry for a corresponding `DurableResume` verb
- Matching criteria: same correlation key structure, compatible I/O types
- Missing resume → `GateFailure` with severity `Error`

**Gate 2: `check_macro_expansion_integrity`**
- For verbs with an `ExpansionContract`: resolve all expansion target verbs
- Each target must be registered with compatible I/O (outputs of step N feed inputs of step N+1)
- Missing or incompatible target → `GateFailure` with severity `Error`

### S6.2 Governance gates (governed tier only, report-only initially)

**File**: `rust/src/sem_reg/gates_governance.rs`

**Gate 3: `check_regulatory_linkage`**
- For governed `PolicyRule` snapshots: verify at least one `RegulatoryReference` is present in the body
- Missing reference → `GateFailure` with severity `Warning` (report-only — see §6 rollout strategy)

**Gate 4: `check_review_cycle_compliance`**
- For governed objects: verify `last_reviewed` timestamp (if the field exists on the body) is within the configured review cycle window
- Default review cycle: 180 days if not specified
- Stale review → `GateFailure` with severity `Warning` (report-only)

**Gate 5: `check_version_consistency`**
- For snapshots with `change_type = Breaking` (major version bump): verify the predecessor snapshot exists and a compatibility analysis field is populated in `change_rationale`
- Missing analysis → `GateFailure` with severity `Warning` (report-only)

### S6.3 Wire all five gates into the publish pipeline

Locate the publish pipeline entry point (likely in `gates.rs` where `run_all_gates()` or equivalent orchestrates gate execution). Add the five new gates to the appropriate execution groups:

- Technical gates (1, 2): run for both tiers, fail on Error
- Governance gates (3, 4, 5): run only for `governance_tier = Governed`, report-only (emit Warning, do not block publish)

### S6.4 Tests

**Test location**: `rust/src/sem_reg/gates_technical.rs` and `gates_governance.rs`, in their respective `#[cfg(test)] mod tests`:

```rust
// gates_technical.rs
#[test]
fn test_continuation_completeness_missing_resume() { ... }
#[test]
fn test_continuation_completeness_with_matching_resume() { ... }
#[test]
fn test_macro_expansion_integrity_missing_target() { ... }
#[test]
fn test_macro_expansion_integrity_valid() { ... }

// gates_governance.rs
#[test]
fn test_regulatory_linkage_missing() { ... }
#[test]
fn test_review_cycle_stale() { ... }
#[test]
fn test_version_consistency_breaking_without_analysis() { ... }
```

### Gate

```bash
cargo test --features vnext-repl -- gates_technical
cargo test --features vnext-repl -- gates_governance

# Invariant regression
DATABASE_URL="postgresql:///data_designer" \
  cargo test --features database --test sem_reg_invariants -- --ignored --nocapture
```

→ IMMEDIATELY proceed to Session S7.

---

## Session S7 — Taxonomy + View Seed Infrastructure (P3-1, P3-2)

**Goal**: Build the seeding infrastructure for taxonomies and views. Populate with structurally correct placeholder content. The Lead Architect will replace placeholder content with real domain data separately.

**Spec reference**: `semantic-os-implementation-todo-v2.md` §2.1, §2.2
**Gap reference**: `SEMANTIC_OS_GAP_REMEDIATION_PLAN.md` P3-1, P3-2

### ⚠️ Content vs Infrastructure Split

This session builds **infrastructure** (YAML format, loading pipeline, CLI commands). It creates **placeholder content** for the four canonical taxonomies and six canonical views. Real KYC domain content is populated by the Lead Architect.

### S7.1 YAML seed file format for taxonomies

**File**: `rust/src/sem_reg/seeds/taxonomies.yaml` (new directory `seeds/`)

Define a YAML schema for taxonomy seed files:

```yaml
taxonomies:
  - name: "KYC Review Navigation"
    description: "PLACEHOLDER: Navigation taxonomy for KYC review workflows"
    governance_tier: Governed
    nodes:
      - path: "ownership-and-control"
        name: "Ownership & Control"
        description: "PLACEHOLDER: UBO discovery and ownership structure"
        children:
          - path: "ownership-and-control/direct"
            name: "Direct Ownership"
          - path: "ownership-and-control/indirect"
            name: "Indirect Ownership"
      - path: "evidence-and-proofs"
        name: "Evidence & Proofs"
        description: "PLACEHOLDER: Document evidence and proof collection"
      # ... more nodes
  - name: "Regulatory Domain"
    # ...
  - name: "Data Sensitivity"
    # ...
  - name: "Execution Semantics"
    # ...
```

### S7.2 YAML seed file format for views

**File**: `rust/src/sem_reg/seeds/views.yaml`

```yaml
views:
  - name: "UBO Discovery"
    description: "PLACEHOLDER: View for UBO resolution workflows"
    taxonomy_slices:
      - taxonomy: "KYC Review Navigation"
        node_path: "ownership-and-control"
    primary_edge_class: Structural
    layout_strategy_hint: Hierarchical
    includes_operational: false
    verb_surface: []  # PLACEHOLDER: populated after verb seeding
    attribute_prominence: []  # PLACEHOLDER
  - name: "Sanctions Screening"
    # ...
  - name: "Proof Collection"
    # ...
  - name: "Case Overview"
    # ...
  - name: "Governance Review"
    # ...
  - name: "Operational Setup"
    includes_operational: true
    # ...
```

### S7.3 Seed loading pipeline

**File**: `rust/src/sem_reg/seeds/loader.rs` (new file)

Implement:
- `load_taxonomy_seeds(path: &Path) -> Result<Vec<TaxonomySeed>>`
- `apply_taxonomy_seeds(pool: &PgPool, seeds: Vec<TaxonomySeed>) -> Result<SeedReport>`
- `load_view_seeds(path: &Path) -> Result<Vec<ViewSeed>>`
- `apply_view_seeds(pool: &PgPool, seeds: Vec<ViewSeed>) -> Result<SeedReport>`

`SeedReport` should include: objects created, objects skipped (already exist), errors.

The loader should be idempotent: if a taxonomy/view with the same name already exists as an Active snapshot, skip it (don't create a duplicate).

### S7.4 CLI commands

**File**: `rust/xtask/src/sem_reg.rs`

Add commands:
- `cargo x sem-reg seed-taxonomies [--file path]` — load and apply taxonomy seeds
- `cargo x sem-reg seed-views [--file path]` — load and apply view seeds
- `cargo x sem-reg taxonomy-tree <name>` — display a taxonomy's node hierarchy as a tree
- `cargo x sem-reg taxonomy-members <taxonomy> <node_path>` — list objects classified under a node

### S7.5 Tests

**Unit tests** in `rust/src/sem_reg/seeds/loader.rs`:
```rust
#[test]
fn test_parse_taxonomy_yaml() { ... }

#[test]
fn test_parse_view_yaml() { ... }
```

**Integration tests** in `rust/tests/sem_reg_integration.rs`:
```rust
#[tokio::test]
#[ignore]
async fn test_seed_taxonomies_creates_snapshots() { ... }

#[tokio::test]
#[ignore]
async fn test_seed_views_creates_snapshots() { ... }

#[tokio::test]
#[ignore]
async fn test_seed_idempotency() { ... }
```

### Gate

```bash
cargo test --features vnext-repl -- seeds::loader

DATABASE_URL="postgresql:///data_designer" \
  cargo test --features database --test sem_reg_integration -- test_seed -- --ignored --nocapture

# CLI verification
DATABASE_URL="postgresql:///data_designer" cargo x sem-reg seed-taxonomies
DATABASE_URL="postgresql:///data_designer" cargo x sem-reg seed-views
DATABASE_URL="postgresql:///data_designer" cargo x sem-reg taxonomy-tree "KYC Review Navigation"

# Should show 4 taxonomies, 6 views
DATABASE_URL="postgresql:///data_designer" cargo x sem-reg stats

# Invariant regression
DATABASE_URL="postgresql:///data_designer" \
  cargo test --features database --test sem_reg_invariants -- --ignored --nocapture
```

→ IMMEDIATELY proceed to Session S8.

---

## Sessions S8–S12 — Onboarding Pipeline Steps 2–6 (P1-4)

**Goal**: Complete the verb-first onboarding pipeline. Each step is a separate Claude Code session because they have fundamentally different concerns and I/O.

**Spec reference**: `semantic-os-implementation-todo-v2.md` §1.4.2–1.4.7
**Gap reference**: `SEMANTIC_OS_GAP_REMEDIATION_PLAN.md` P1-4

### Session S8 — Step 2: Attribute Extraction from Verb Surfaces

**File**: `rust/src/sem_reg/onboarding/attr_extract.rs` (new)

**Input**: Verb inventory from Step 1 (existing `scanner.rs` output — list of `(verb_canonical_name, declared_inputs[], declared_outputs[], exec_mode, expansion?, continuation?)`).

**Logic**:
- For each verb, extract the attributes it reads (`declared_inputs[]`) and writes (`declared_outputs[]`)
- For verbs with side-effects declarations, extract implicit attributes from the side-effects
- Deduplicate across verbs: build a map of `attribute_name → { producing_verbs[], consuming_verbs[], inferred_type? }`
- Infer type from usage patterns where possible (e.g., an attribute consumed by a verb whose contract specifies `type_spec: String` → the attribute is likely String)

**Output**: `VerbConnectedAttributeSet` — the set of attributes that matter because something in the platform reads or writes them.

**Test location**: `rust/src/sem_reg/onboarding/attr_extract.rs` `#[cfg(test)] mod tests`

**Gate**:
```bash
cargo test --features vnext-repl -- onboarding::attr_extract
```

→ IMMEDIATELY proceed to Session S9.

### Session S9 — Step 3: Schema Cross-Reference

**File**: `rust/src/sem_reg/onboarding/schema_xref.rs` (new)

**Input**: `VerbConnectedAttributeSet` from Step 2.

**Logic**:
- Query `information_schema.columns` for all tables in the operational schema (92+ tables)
- Query `information_schema.table_constraints` and `information_schema.key_column_usage` for FK relationships and CHECK constraints
- For each verb-connected attribute, match against column names using these heuristics (in priority order):
  1. **Exact match**: attribute name == column name
  2. **Table-qualified match**: attribute name == `table_name.column_name`
  3. **Prefix/suffix strip**: strip common prefixes/suffixes (`fk_`, `_id`, `_ref`, `_code`) and match
  4. **Snake-case normalisation**: normalise both sides to snake_case and match
  5. Do NOT use fuzzy string distance — false positives are worse than missing matches in a compliance context
- For matched columns, extract: SQL type → `type_spec`, NOT NULL → `required` constraint, CHECK expressions → `AttributeConstraint`, FK references → relationship edge candidates
- Extract table context → infer entity type membership (columns in `client_entity` table → belong to ClientEntity type)

**Output**: Enriched attribute records: `{ attribute_name, type_spec, constraints[], inferred_entity_type, producing_verbs[], consuming_verbs[], schema_column_ref? }`

**Test location**: `rust/src/sem_reg/onboarding/schema_xref.rs` `#[cfg(test)] mod tests` (unit tests with mock schema results) + integration test that runs against real DB.

**Gate**:
```bash
cargo test --features vnext-repl -- onboarding::schema_xref

DATABASE_URL="postgresql:///data_designer" \
  cargo test --features database --test sem_reg_integration -- test_schema_xref -- --ignored --nocapture
```

→ IMMEDIATELY proceed to Session S10.

### Session S10 — Step 4: Entity Type Inference

**File**: `rust/src/sem_reg/onboarding/entity_infer.rs` (new)

**Input**: Enriched attribute records from Step 3.

**Logic**:
- Group attributes by their `inferred_entity_type` (from table membership)
- For each FK relationship detected in Step 3, classify the edge:
  - FK to a table that looks like a parent/owner (same prefix, broader scope) → `EdgeClass::Structural`
  - FK to a table named `*_document*`, `*_evidence*`, `*_proof*` → `EdgeClass::Reference`
  - FK to a table named `*_lookup*`, `*_type*`, `*_code*`, `*_category*` → `EdgeClass::Association`
  - FK where both tables participate in time-bounded joins → `EdgeClass::Temporal`
  - Ambiguous → `EdgeClass::Association` (safest default)
- Infer entity lifecycle states from `status` / `state` columns and their CHECK constraints

**Output**: `EntityTypeDef` candidates with attribute memberships, `RelationshipTypeDef` candidates with edge classes.

**Test location**: `rust/src/sem_reg/onboarding/entity_infer.rs` `#[cfg(test)] mod tests`

**Gate**:
```bash
cargo test --features vnext-repl -- onboarding::entity_infer
```

→ IMMEDIATELY proceed to Session S11.

### Session S11 — Step 5: Seed Registries

**File**: `rust/src/sem_reg/onboarding/seed.rs` (new)

**Input**: All outputs from Steps 2–4.

**Logic**:
- Verb-connected attributes → `AttributeDef` snapshots. Set `governance_tier = Operational`, `trust_class = Convenience`, `approved_by = "auto"`.
- `kind` classification:
  - Raw schema column → `Primitive`
  - Extracted from document (based on verb context) → `Captured`
  - From external API (based on verb context) → `ExternalSourced`
  - (Derived/Composite handled in Session S13)
- Verb definitions → `VerbContract` snapshots. Wire I/O attribute references to seeded `AttributeDef` IDs.
- Entity types → `EntityTypeDef` snapshots with attribute memberships.
- Relationships → `RelationshipTypeDef` snapshots (uses S5 infrastructure).

**Output**: `SeedReport` with counts per object type and wiring completeness percentage.

**CLI**: `cargo x sem-reg onboard apply`

**Test location**: `rust/tests/sem_reg_integration.rs`

**Gate**:
```bash
DATABASE_URL="postgresql:///data_designer" \
  cargo test --features database --test sem_reg_integration -- test_onboard_seed -- --ignored --nocapture

DATABASE_URL="postgresql:///data_designer" cargo x sem-reg stats
# Should show increased counts for attributes, verbs, entity types, relationships
```

→ IMMEDIATELY proceed to Session S12.

### Session S12 — Step 6: Orphan Classification

**File**: `rust/src/sem_reg/onboarding/orphans.rs` (new)

**Input**: Schema columns from Step 3, seeded attribute set from Step 5.

**Logic**: For every column in the operational schema NOT referenced by any verb:

1. **Framework columns** — match against patterns: `created_at`, `updated_at`, `created_by`, `updated_by`, `version`, `audit_id`, `id` (primary key), `_uuid` suffix. **Action**: skip silently, do not seed.

2. **UI / reporting / export convenience** — match against patterns: `*_display`, `*_cache`, `*_formatted`, `*_composite`, `*_denorm`, columns in tables named `*_view_*` or `*_report_*`. **Action**: seed ONLY if clearly domain-meaningful. Tag `origin = projection`, `verb_orphan = true`. When in doubt, skip.

3. **Genuine operational fields** — columns in operational tables (servicing, routing, config) not matching Framework or UI patterns. **Action**: seed as `Operational / Convenience` with `verb_orphan = true`.

4. **Dead schema** — columns where grep across the entire Rust codebase + SQL views finds zero reads, zero writes, zero references. **Action**: flag for schema cleanup review, do NOT seed.

**Output**: Onboarding report showing N seeded (verb-connected), M operational orphans, K dead schema, entity types/relationships inferred, wiring completeness.

**CLI commands**:
- `cargo x sem-reg onboard scan` — execute Steps 1–4, produce report, write nothing
- `cargo x sem-reg onboard report` — display current onboarding status
- `cargo x sem-reg onboard verify` — verify every verb's I/O maps to registered AttributeDefs

**Test location**: `rust/src/sem_reg/onboarding/orphans.rs` `#[cfg(test)] mod tests`

**Gate**:
```bash
cargo test --features vnext-repl -- onboarding::orphans

DATABASE_URL="postgresql:///data_designer" cargo x sem-reg onboard scan
DATABASE_URL="postgresql:///data_designer" cargo x sem-reg onboard report
DATABASE_URL="postgresql:///data_designer" cargo x sem-reg onboard verify
# Wiring completeness should be >= 80% for existing verbs

# Invariant regression
DATABASE_URL="postgresql:///data_designer" \
  cargo test --features database --test sem_reg_invariants -- --ignored --nocapture
```

→ IMMEDIATELY proceed to Session S13.

---

## Session S13 — Policy + Derivation Seed Infrastructure (P3-3, P3-4)

**Goal**: Build seeding infrastructure for policy rules and derivation specs. Populate with placeholder content referencing existing codebase patterns.

**Spec reference**: `semantic-os-implementation-todo-v2.md` §3.1, §5.4
**Gap reference**: `SEMANTIC_OS_GAP_REMEDIATION_PLAN.md` P3-3, P3-4

### ⚠️ Content vs Infrastructure Split

This session builds the YAML seed format and loading pipeline. Real policy predicates and derivation function_refs are populated by the Lead Architect from domain knowledge.

### S13.1 Policy seed infrastructure

**File**: `rust/src/sem_reg/seeds/policies.yaml` + loader extension

YAML format for policy rules:
```yaml
policies:
  - name: "PLACEHOLDER: CDD document requirement"
    description: "PLACEHOLDER: Client must provide identity documentation"
    scope: "entity_type:ClientEntity"
    predicate: { "kind": "attribute_present", "attribute": "identity_document" }
    predicate_trust_minimum: Proof
    enforcement: Hard
    evidence_requirements:
      - required_doc_type: "identity_document"
        freshness_window_days: 365
        governance_tier_minimum: Governed
        trust_class_minimum: Proof
    governance_tier: Governed
    # ...
```

### S13.2 Derivation seed infrastructure

**File**: `rust/src/sem_reg/seeds/derivations.yaml` + loader extension

YAML format for derivation specs. Note the `ExpressionKind` enum for future-proofing:

```yaml
derivations:
  - output_attribute: "risk_score"
    inputs:
      - attribute: "jurisdiction"
        role: "context"
        required: true
      - attribute: "entity_type"
        role: "context"
        required: true
    expression:
      kind: function_ref       # MVP: function_ref only
      ref: "compute_risk_score"
    # Future: kind: expression_ast / kind: query_plan
    null_semantics: "propagate"
    freshness_rule: "inherit_oldest_input"
    security_inheritance: Strict
    evidence_grade: Prohibited   # Operational derivation — always Prohibited
    governance_tier: Operational
```

### S13.3 ExpressionKind enum (future-proofing)

In the `DerivationSpec` body (or the derivation module), ensure the expression field uses a discriminated enum:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum ExpressionKind {
    #[serde(rename = "function_ref")]
    FunctionRef { ref_name: String },
    #[serde(rename = "expression_ast")]
    ExpressionAst { ast: serde_json::Value },  // Future
    #[serde(rename = "query_plan")]
    QueryPlan { plan: serde_json::Value },      // Future
}
```

If the current implementation stores `expression` as a plain JSONB object with `{ "kind": "function_ref", "ref": "..." }`, migrate to this typed enum. The evaluation path should match on `ExpressionKind::FunctionRef` for now; `ExpressionAst` and `QueryPlan` arms return `Err("not yet implemented")`.

### S13.4 CLI + tests

**CLI**: `cargo x sem-reg seed-policies`, `cargo x sem-reg seed-derivations`

**Test location**: `rust/src/sem_reg/seeds/loader.rs` (extend existing tests)

### Gate

```bash
cargo test --features vnext-repl -- seeds::loader

DATABASE_URL="postgresql:///data_designer" cargo x sem-reg seed-policies
DATABASE_URL="postgresql:///data_designer" cargo x sem-reg seed-derivations
DATABASE_URL="postgresql:///data_designer" cargo x sem-reg stats

# Invariant regression
DATABASE_URL="postgresql:///data_designer" \
  cargo test --features database --test sem_reg_invariants -- --ignored --nocapture
```

→ IMMEDIATELY proceed to Session S14.

---

## Session S14 — Membership Wiring (P3-5)

**Goal**: Auto-classify seeded registry objects into taxonomy nodes based on attribute patterns, verb exec_mode, and entity types.

**Spec reference**: `semantic-os-implementation-todo-v2.md` §2.3
**Gap reference**: `SEMANTIC_OS_GAP_REMEDIATION_PLAN.md` P3-5

### S14.1 Auto-classification rules

**File**: `rust/src/sem_reg/onboarding/classify.rs` (new)

Implement classification rules for each canonical taxonomy:

**Data Sensitivity** (auto-assign, high confidence):
- Attributes matching PII name patterns (`name`, `address`, `date_of_birth`, `ssn`, `passport`, `email`, `phone`) → PII node
- Attributes in sanctions/screening verbs → Sanctions node
- Financial data patterns (`amount`, `balance`, `price`, `rate`, `fee`) → Financial node

**Execution Semantics** (auto-assign from verb metadata):
- `exec_mode = Sync` → Synchronous node
- `exec_mode = Research` → Research node
- `exec_mode = DurableStart | DurableResume` → Durable node
- Verbs with `expansion` → Macro node

**KYC Review Navigation** (propose, lower confidence):
- Entity types related to ownership (UBO, shareholder, beneficial_owner patterns) → Ownership & Control
- Document-related entity types → Evidence & Proofs
- Screening-related verbs → Sanctions / Adverse Media

**Regulatory Domain** (propose, lower confidence):
- Verbs with CDD/EDD-flavoured names or preconditions → CDD/EDD nodes
- Sanctions workflow participants → Sanctions node

### S14.2 CLI

- `cargo x sem-reg onboard classify --taxonomy <name> --mode auto` — auto-assign high-confidence classifications
- `cargo x sem-reg onboard classify --taxonomy <name> --mode propose` — generate proposals for steward review (JSON output)

### Gate

```bash
DATABASE_URL="postgresql:///data_designer" cargo x sem-reg onboard classify --taxonomy "Data Sensitivity" --mode auto
DATABASE_URL="postgresql:///data_designer" cargo x sem-reg onboard classify --taxonomy "Execution Semantics" --mode auto
DATABASE_URL="postgresql:///data_designer" cargo x sem-reg taxonomy-members "Data Sensitivity" "pii"
# Should return non-empty results

# Invariant regression
DATABASE_URL="postgresql:///data_designer" \
  cargo test --features database --test sem_reg_invariants -- --ignored --nocapture
```

→ IMMEDIATELY proceed to Session S15.

---

## Session S15 — Execution Snapshot Pinning (P2-1)

**Goal**: Make existing verb execution automatically create DecisionRecords with snapshot manifests.

**Spec reference**: `semantic-os-implementation-todo-v2.md` §8.5
**Gap reference**: `SEMANTIC_OS_GAP_REMEDIATION_PLAN.md` P2-1

### S15.1 Integration point

**File**: `rust/src/agent/orchestrator.rs` (or the unified execution pipeline)

After successful verb execution via the pipeline:

1. Look up the executed verb's `VerbContract` from the semantic registry → get `snapshot_id`
2. For non-trivial decisions (verb execution, not navigation/UI actions), create a `DecisionRecord`:
   - `snapshot_manifest`: at minimum `{ verb_contract: snapshot_id }`. If attribute definitions were resolved during execution, include those too.
   - `chosen_action`: the executed verb canonical name
   - `policy_verdicts`: from the most recent `resolve_context()` call, if available
3. Insert via the existing `DecisionRecord` insert path

### S15.2 Feature gate

Gate behind `sem-reg-decision-audit` feature flag. When disabled, no DecisionRecords are created and no latency is added.

### S15.3 Implementation strategy

- Recording is **async, best-effort**: spawn a tokio task for the INSERT. Do not block the execution pipeline.
- If the semantic registry is unavailable (feature disabled, DB error), log a warning and continue. Never fail a verb execution because of a recording failure.
- Start with verb_contract snapshot_id only. Expand the manifest contents in a future iteration.

### S15.4 Tests

**Test location**: `rust/tests/sem_reg_integration.rs`

```rust
#[tokio::test]
#[ignore]
async fn test_verb_execution_creates_decision_record() { ... }

#[tokio::test]
#[ignore]
async fn test_decision_record_has_snapshot_manifest() { ... }

#[tokio::test]
#[ignore]
async fn test_recording_failure_does_not_block_execution() { ... }
```

### Gate

```bash
DATABASE_URL="postgresql:///data_designer" \
  cargo test --features "database,sem-reg-decision-audit" --test sem_reg_integration -- test_verb_execution_creates -- --ignored --nocapture

# Invariant regression (especially Invariant 2 — snapshot_manifest required)
DATABASE_URL="postgresql:///data_designer" \
  cargo test --features database --test sem_reg_invariants -- --ignored --nocapture
```

→ IMMEDIATELY proceed to Session S16.

---

## Session S16 — MCP Resource Surface (P2-2)

**Goal**: Expose registry objects as MCP resources via `resources/list` and `resources/read`.

**Spec reference**: `semantic-os-implementation-todo-v2.md` §8.4
**Gap reference**: `SEMANTIC_OS_GAP_REMEDIATION_PLAN.md` P2-2

### S16.1 Resource URI templates

Nine resource URI patterns:

| URI Pattern | Resolves To |
|---|---|
| `sem_reg://attributes/{name_or_id}` | Active AttributeDef snapshot |
| `sem_reg://verbs/{canonical_name}` | Active VerbContract snapshot |
| `sem_reg://entities/{name}` | Active EntityTypeDef snapshot |
| `sem_reg://policies/{name_or_id}` | Active PolicyRule snapshot |
| `sem_reg://views/{name}` | Active ViewDef snapshot |
| `sem_reg://taxonomies/{name}` | Active TaxonomyDef snapshot |
| `sem_reg://observations/{subject_ref}/{attribute_id}` | Current observation chain |
| `sem_reg://decisions/{decision_id}` | DecisionRecord with snapshot manifest |
| `sem_reg://plans/{plan_id}` | AgentPlan with steps |

All support `?as_of=<ISO8601 timestamp>` for point-in-time resolution.

### S16.2 MCP server handlers

Add to the MCP server:
- `resources/list` → returns the 9 URI templates with descriptions
- `resources/read` → dispatches by URI prefix, resolves the object, applies ABAC enforcement via `enforce_read()`, returns JSON

### S16.3 ABAC enforcement

All resource reads pass through `enforce_read()`. ABAC denial → return a redacted stub (same pattern as existing tool enforcement) rather than an error, so the agent knows the resource exists but cannot see its content.

### S16.4 Tests

**Test location**: `rust/tests/sem_reg_integration.rs`

```rust
#[tokio::test]
#[ignore]
async fn test_mcp_resources_list_returns_nine_templates() { ... }

#[tokio::test]
#[ignore]
async fn test_mcp_resource_read_attribute() { ... }

#[tokio::test]
#[ignore]
async fn test_mcp_resource_read_with_as_of() { ... }

#[tokio::test]
#[ignore]
async fn test_mcp_resource_read_abac_denial_returns_redacted() { ... }
```

### Gate

```bash
DATABASE_URL="postgresql:///data_designer" \
  cargo test --features database --test sem_reg_integration -- test_mcp_resource -- --ignored --nocapture

# Invariant regression
DATABASE_URL="postgresql:///data_designer" \
  cargo test --features database --test sem_reg_invariants -- --ignored --nocapture
```

→ IMMEDIATELY proceed to Session S17.

---

## Session S17 — Agent Prompt Grounding (P2-3)

**Goal**: Wire Semantic OS instructions into agent system prompts so the LLM knows to use the registry.

**Spec reference**: `semantic-os-implementation-todo-v2.md` §8.6
**Gap reference**: `SEMANTIC_OS_GAP_REMEDIATION_PLAN.md` P2-3

### S17.1 Prompt constant

**File**: `rust/src/sem_reg/agent/prompt_grounding.rs` (new)

```rust
pub const SEMANTIC_OS_INSTRUCTIONS: &str = r#"
## Semantic Registry Instructions

The Semantic Registry is the authoritative source for what actions are available,
what they require, and what they produce.

### Required Workflow
1. Before proposing actions: call `sem_reg_resolve_context` to get registry-backed
   recommendations for the current case/subject.
2. After every non-trivial decision: call `sem_reg_record_decision` with the
   action taken, alternatives considered, and evidence.
3. Before relying on evidence: call `sem_reg_check_evidence_freshness` to verify
   observations are within freshness windows.
4. When planning proof collection: call `sem_reg_identify_evidence_gaps` to find
   what evidence is missing vs. what policies require.

### Proof Rule (NON-NEGOTIABLE)
Never treat operational-tier or convenience-trust-class attributes as evidence
for compliance decisions. Always verify `governance_tier` and `trust_class`
before using an attribute as proof.

### Evidence Recording
When extracting data from documents, call `sem_reg_record_observation` to create
an immutable observation record in the evidence chain.
"#;
```

### S17.2 Wire into system prompt

Locate the agent system prompt construction (in `agent/orchestrator.rs` or a prompt module). Append `SEMANTIC_OS_INSTRUCTIONS` to the system prompt when the `sem-reg-prompt-grounding` feature flag is enabled.

**Feature gate**: `sem-reg-prompt-grounding` — disabled by default for gradual rollout.

### S17.3 Tests

**Test location**: `rust/src/sem_reg/agent/prompt_grounding.rs` `#[cfg(test)] mod tests`

```rust
#[test]
fn test_prompt_contains_resolve_context_instruction() {
    assert!(SEMANTIC_OS_INSTRUCTIONS.contains("sem_reg_resolve_context"));
}

#[test]
fn test_prompt_contains_record_decision_instruction() {
    assert!(SEMANTIC_OS_INSTRUCTIONS.contains("sem_reg_record_decision"));
}

#[test]
fn test_prompt_contains_proof_rule() {
    assert!(SEMANTIC_OS_INSTRUCTIONS.contains("governance_tier"));
    assert!(SEMANTIC_OS_INSTRUCTIONS.contains("trust_class"));
}
```

### Gate

```bash
cargo test --features vnext-repl -- prompt_grounding

# Verify grep
grep -r "sem_reg_resolve_context" rust/src/sem_reg/agent/prompt_grounding.rs
# Should return at least one match

# Invariant regression
DATABASE_URL="postgresql:///data_designer" \
  cargo test --features database --test sem_reg_invariants -- --ignored --nocapture
```

→ IMMEDIATELY proceed to Session S18.

---

## Session S18 — Embedding Ranking in Context Resolution (P2-4)

**Goal**: Wire embedding-based semantic similarity as a secondary ranking signal in `resolve_context()`.

**Spec reference**: `semantic-os-implementation-todo-v2.md` §9.2
**Gap reference**: `SEMANTIC_OS_GAP_REMEDIATION_PLAN.md` P2-4

### S18.1 Embedding lookup integration

**File**: `rust/src/sem_reg/context_resolution.rs`

When `req.intent` is `Some(text)`:
1. Embed the intent text using the local Candle/BGE embedder (existing infrastructure)
2. For each candidate verb/attribute returned by taxonomy-based filtering, look up its embedding from `sem_reg.embedding_records`
3. Compute cosine similarity between the intent embedding and each candidate's embedding
4. Blend: `final_score = taxonomy_score * 0.8 + embedding_similarity * 0.2`

When `req.intent` is `None`: use taxonomy-only ranking (no change from current behaviour).

### S18.2 Graceful degradation

- If an embedding record is missing for a candidate → use `embedding_similarity = 0.0` (neutral, falls back to taxonomy-only)
- If an embedding record is stale (`stale_since IS NOT NULL`) → use `embedding_similarity = 0.0` with a governance signal noting the staleness
- If the embedder is unavailable (model not loaded) → fall back entirely to taxonomy-only ranking, no error

### S18.3 NoLLMExternal enforcement

Attributes with `handling_requirements` containing `NoLLMExternal`:
- Must only be embedded using the internal/local model
- If only an external model is available, exclude these attributes from embedding-based ranking
- Track enforcement on the `EmbeddingRecord`: verify the `model_id` matches an internal model

### S18.4 Tests

**Test location**: `rust/src/sem_reg/context_resolution.rs` `#[cfg(test)] mod tests`

```rust
#[test]
fn test_intent_based_ranking_changes_order() { ... }

#[test]
fn test_missing_embeddings_graceful_fallback() { ... }

#[test]
fn test_stale_embeddings_ignored_with_signal() { ... }

#[test]
fn test_no_intent_uses_taxonomy_only() { ... }
```

### Gate

```bash
cargo test --features vnext-repl -- context_resolution

# Invariant regression
DATABASE_URL="postgresql:///data_designer" \
  cargo test --features database --test sem_reg_invariants -- --ignored --nocapture
```

→ IMMEDIATELY proceed to Session S19.

---

## Session S19 — CLI Polish, Publish Command, Label Templates (P4-1, P4-2, P4-3)

**Goal**: Complete all remaining CLI commands, create the user-facing publish command, and implement security label templates.

**Spec reference**: `semantic-os-implementation-todo-v2.md` §4.4, §6
**Gap reference**: `SEMANTIC_OS_GAP_REMEDIATION_PLAN.md` P4-1, P4-2, P4-3

### S19.1 Missing CLI commands (P4-1)

**File**: `rust/xtask/src/sem_reg.rs`

Add all missing commands:

| Command | Purpose |
|---|---|
| `sem-reg publish <object_type> <fqn>` | Gate-checked publish (see S19.2) |
| `sem-reg classify <object_type> <fqn> <taxonomy> <node_path>` | Create draft membership |
| `sem-reg coverage [--tier all\|governed\|operational]` | Coverage metrics report |

Note: `taxonomy-tree`, `taxonomy-members`, `onboard scan/apply/report/verify`, `rel-describe` should already exist from previous sessions. Verify they all work.

### S19.2 Publish command (P4-2)

`cargo x sem-reg publish <object_type> <fqn>` should:

1. Load the draft snapshot for `(object_type, fqn)`
2. Determine the governance tier from the snapshot
3. Run all applicable gates:
   - Technical gates (both tiers): type correctness, verb surface disclosure, dependency correctness, security label presence, snapshot integrity, continuation completeness, macro expansion integrity
   - Governance gates (governed tier only): taxonomy membership, stewardship, policy attachment, regulatory linkage, review-cycle compliance, version consistency
   - Proof Rule gate (both tiers)
4. On all gates pass → promote snapshot status to Active, set `effective_from`, update predecessor's `effective_until`
5. On any gate failure → print structured output:
   ```
   PUBLISH FAILED: 3 errors, 2 warnings

   [ERROR] verb_surface_disclosure: Verb 'kyc.verify' references attribute 'ssn' not declared in I/O
     → Add 'ssn' to the verb's consumes declaration

   [ERROR] proof_rule: PolicyRule 'cdd_doc_requirement' references Operational attribute 'client_name'
     → Promote 'client_name' to Governed tier or remove from policy predicate

   [WARN] review_cycle: AttributeDef 'jurisdiction' last reviewed 210 days ago (cycle: 180 days)
     → Schedule governance review
   ```
6. Exit code: 0 on success, 1 on error failures, 0 on warnings-only

### S19.3 Security label templates (P4-3)

**File**: `rust/src/sem_reg/seeds/label_templates.rs` (new) or `rust/src/sem_reg/seeds/label_templates.yaml`

Define at least five label templates:

| Template Name | Confidentiality | Data Category | Handling Requirements | Purpose Tags | Residency Class |
|---|---|---|---|---|---|
| `standard_pii_uk` | Confidential | PII | MaskByDefault, NoExport | KYC, AML | UK |
| `standard_pii_eu` | Confidential | PII | MaskByDefault, NoExport, GDPRControlled | KYC, AML | EU |
| `standard_financial_global` | Internal | Financial | AuditTrail | Reporting, Operations | Global |
| `sanctions_restricted` | Restricted | Sanctions | MaskByDefault, NoLLMExternal | Sanctions | Global |
| `operational_internal` | Internal | None | [] | [] | Global |

Update the `backfill-labels` CLI command to:
1. Accept a `--template-mode` flag (in addition to existing default behaviour)
2. In template mode: match attributes against classification patterns (PII name patterns → `standard_pii_*`, sanctions taxonomy membership → `sanctions_restricted`, etc.)
3. Cross-validate against Data Sensitivity taxonomy membership from S14

### S19.4 Tests

**Test location**: Various — see above modules

### Gate (Final)

```bash
# Full test suite
cargo test --features vnext-repl -- sem_reg

# Gate tests
cargo test --features vnext-repl -- gates_technical
cargo test --features vnext-repl -- gates_governance

# Integration tests
DATABASE_URL="postgresql:///data_designer" \
  cargo test --features database --test sem_reg_integration -- --ignored --nocapture

# Invariant regression
DATABASE_URL="postgresql:///data_designer" \
  cargo test --features database --test sem_reg_invariants -- --ignored --nocapture

# CLI verification
DATABASE_URL="postgresql:///data_designer" cargo x sem-reg stats
DATABASE_URL="postgresql:///data_designer" cargo x sem-reg validate --enforce
DATABASE_URL="postgresql:///data_designer" cargo x sem-reg taxonomy-tree "KYC Review Navigation"
DATABASE_URL="postgresql:///data_designer" cargo x sem-reg coverage --tier all
DATABASE_URL="postgresql:///data_designer" cargo x sem-reg onboard scan
DATABASE_URL="postgresql:///data_designer" cargo x sem-reg onboard verify

# Publish flow test
DATABASE_URL="postgresql:///data_designer" cargo x sem-reg publish attribute_def "test.attribute"
```

→ Gap remediation complete.

---

## Session Dependency Summary

```
S1:  Invariant test harness                          ← Foundation
  ↓
S2:  Fix stub gates (P0-2, P0-3)                     ← No dependencies
  ↓
S3:  Fix tier_allowed (P0-4)                          ← No dependencies
  ↓
S4:  Evidence tables + MCP tools (P1-1 + P0-1)       ← Atomic: tables + consumers
  ↓
S5:  RelationshipTypeDefBody (P1-2)                   ← Independent
  ↓
S6:  Five missing gates (P1-3)                        ← Independent, benefits from S5
  ↓
S7:  Taxonomy + view seed infra (P3-1, P3-2)          ← Content infrastructure
  ↓
S8:  Onboarding Step 2: attr extraction               ← Requires existing scanner
  ↓
S9:  Onboarding Step 3: schema cross-ref              ← Requires S8 output
  ↓
S10: Onboarding Step 4: entity inference              ← Requires S9 output
  ↓
S11: Onboarding Step 5: seed registries               ← Requires S5, S10
  ↓
S12: Onboarding Step 6: orphan classification         ← Requires S9, S11
  ↓
S13: Policy + derivation seed infra (P3-3, P3-4)     ← Content infrastructure
  ↓
S14: Membership wiring (P3-5)                         ← Requires S7, S12
  ↓
S15: Execution snapshot pinning (P2-1)                ← Requires registry content
  ↓
S16: MCP resource surface (P2-2)                      ← Requires S4
  ↓
S17: Agent prompt grounding (P2-3)                    ← Integration
  ↓
S18: Embedding ranking in resolution (P2-4)           ← Requires populated embeddings
  ↓
S19: CLI polish + publish + labels (P4-1-3)           ← Polish, all prior sessions
```

---

## Post-Remediation: Integration Test Scenarios

After all 19 sessions complete, run the five canonical integration test scenarios from `semantic-os-implementation-todo-v2.md` Phase 10:

1. **UBO Discovery end-to-end** (§10.1)
2. **Sanctions Screening end-to-end** (§10.2)
3. **Proof Collection end-to-end** (§10.3)
4. **Governance Review** (§10.4)
5. **Point-in-time Audit** (§10.5)
6. **Proof Rule Enforcement Validation** (§10.6)

These are NOT separate Claude Code sessions — they are the final acceptance criteria that validate the entire remediation. If any scenario fails, trace back to the responsible session and fix.

---

## Appendix A: Verification Commands (run after every session)

```bash
# Invariant regression (MANDATORY after every session)
DATABASE_URL="postgresql:///data_designer" \
  cargo test --features database --test sem_reg_invariants -- --ignored --nocapture

# Full unit test suite
cargo test --features vnext-repl -- sem_reg

# Static checks
cargo test --features vnext-repl -- test_no_unwrap_in_runbook

# Quick stats
DATABASE_URL="postgresql:///data_designer" cargo x sem-reg stats
```

## Appendix B: Files Created or Modified Per Session

| Session | New Files | Modified Files |
|---|---|---|
| S1 | `rust/tests/sem_reg_invariants.rs`, `rust/tests/sem_reg_test_helpers.rs` | Possibly `sem_reg/store.rs` (trigger enforcement) |
| S2 | — | `gates_technical.rs` |
| S3 | — | `context_resolution.rs` |
| S4 | `sem_reg/evidence_instances.rs`, migration SQL | `agent/mcp_tools.rs`, `mod.rs` |
| S5 | `sem_reg/relationship_type_def.rs` | `mod.rs`, `registry.rs`, `xtask/sem_reg.rs` |
| S6 | — | `gates_technical.rs`, `gates_governance.rs`, `gates.rs` |
| S7 | `sem_reg/seeds/` directory (4+ files) | `xtask/sem_reg.rs`, `mod.rs` |
| S8 | `sem_reg/onboarding/attr_extract.rs` | `onboarding/mod.rs` |
| S9 | `sem_reg/onboarding/schema_xref.rs` | `onboarding/mod.rs` |
| S10 | `sem_reg/onboarding/entity_infer.rs` | `onboarding/mod.rs` |
| S11 | `sem_reg/onboarding/seed.rs` | `onboarding/mod.rs`, `xtask/sem_reg.rs` |
| S12 | `sem_reg/onboarding/orphans.rs`, `onboarding/report.rs` | `onboarding/mod.rs`, `xtask/sem_reg.rs` |
| S13 | `seeds/policies.yaml`, `seeds/derivations.yaml` | `seeds/loader.rs`, `derivation_spec.rs` |
| S14 | `sem_reg/onboarding/classify.rs` | `onboarding/mod.rs`, `xtask/sem_reg.rs` |
| S15 | — | `agent/orchestrator.rs` |
| S16 | — | MCP server handler file |
| S17 | `sem_reg/agent/prompt_grounding.rs` | `agent/mod.rs`, orchestrator |
| S18 | — | `context_resolution.rs` |
| S19 | `seeds/label_templates.rs` or `.yaml` | `xtask/sem_reg.rs` |

## Appendix C: Feature Flags

| Flag | Purpose | Default | Sessions |
|---|---|---|---|
| `vnext-repl` | Existing: enables sem_reg compilation | Enabled | All |
| `database` | Existing: enables DB-dependent tests | Test only | All integration tests |
| `sem-reg-decision-audit` | Auto-create DecisionRecords on verb execution | Disabled | S15 |
| `sem-reg-prompt-grounding` | Include Semantic OS instructions in agent prompt | Disabled | S17 |

## Appendix D: Peer Review Changes (v3.0 → v3.1)

Five high-impact fixes from peer review, plus two upgrade items.

### Fix 1: S1 Snapshot Table Agnosticism

**Problem**: v3.0 assumed a central `sem_reg.snapshots` table, but v2 describes per-registry tables (`attribute_defs`, `verb_contracts`, etc.). The test would target a table that may not exist.

**Fix**: S1 Invariant 1 now includes a discovery step (`\dt sem_reg.*`) before writing the test, and provides implementation paths for both the central-table and per-registry-table patterns. The trigger function is shared and attachable to whichever tables exist.

### Fix 2: Evidence Table Immutability Enforcement

**Problem**: v3.0 created `sem_reg.observations`, `provenance_edges`, and `retention_policies` as INSERT-only by convention, but nothing prevented `UPDATE`/`DELETE` at the DB level. This contradicts the immutable-records principle enforced everywhere else.

**Fix**: S4.1 migration now includes PostgreSQL triggers:
- `observations`: INSERT-only (UPDATE and DELETE raise exceptions)
- `provenance_edges`: INSERT-only (same trigger function)
- `retention_policies`: INSERT-only (same trigger function)
- `document_instances`: lifecycle guard — UPDATE allowed only on lifecycle fields (`validated_at`, `expiry`, `retention_until`, `status`); all other columns are write-once

### Fix 3: Linear Supersession Chain Enforcement

**Problem**: v3.0's point-in-time observation query used complex recursive chain logic ("not superseded by any later observation"). This is correct but slow and bug-prone.

**Fix**: S4.3 `insert_observation()` now enforces a linear chain invariant: `supersedes` must point to the current latest observation for `(subject_ref, attribute_id)`. Out-of-order or forking supersession is rejected at insert time. This simplifies `resolve_observation_at()` to a single `ORDER BY observed_at DESC LIMIT 1` query — no chain traversal needed.

### Fix 4: Freshness/Gaps Graceful Degradation

**Problem**: S4.5's `handle_check_freshness()` and `handle_identify_gaps()` assumed `PolicyRule` and `EvidenceRequirement` data exists. But policies aren't seeded until S13 (9 sessions later). During early bring-up, these tools would either error or return misleadingly empty results.

**Fix**: Both tools now degrade gracefully by contract:
- `check_freshness` without policies → `is_fresh: "unknown"` (not `true`)
- `identify_gaps` without policies → `{ policies_found: 0, status: "no_policies_registered" }` (not empty gaps list)
This prevents "false confidence" where the absence of checks looks like the absence of problems.

### Fix 5: Test Filter Convention

**Problem**: Gate commands used patterns like `cargo test -- gates_technical::tests` which may not match actual test paths depending on module layout, causing gates to silently pass with zero tests run.

**Fix**: All gate commands now use module-name-only filters (`-- gates_technical` instead of `-- gates_technical::tests`). Execution Rule 7 added: verify filters match actual tests with `--list` if zero tests run.

### Upgrade 1: `attribute_snapshot_id` on Observations

`sem_reg.observations` now includes an optional `attribute_snapshot_id` column. When populated (by the MCP handler in S4.5), it pins the exact `AttributeDef` version active at observation time. This enables forensic audit replay without a separate point-in-time registry lookup.

### Upgrade 2: EdgeClass Naming Collision Warning

S5 now includes a pre-implementation check: `grep -rn "EdgeClass" rust/src/sem_reg/` to detect existing enums with the same name. If a compatible enum exists, reuse it. If an incompatible one exists, namespace the new one as `RelationshipEdgeClass`. This prevents silent JSONB serialization mismatches between different `EdgeClass` types.
