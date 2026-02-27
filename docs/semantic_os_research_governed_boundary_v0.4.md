# Semantic OS: Research → Governed Change Boundary
**Version:** 0.4  
**Date:** Feb 2026  
**Scope:** OB-POC Semantic OS / SemReg, agentic sessions, DSL verbs, schema + attribute dictionary governance  
**Status:** Implemented (2026-02-25) — standalone server verified (2026-02-26)
**Implementation:** `rust/crates/sem_os_core/src/authoring/`, `rust/crates/sem_os_server/`, migrations 099-102
**Tests:** 60 unit + 26 integration + 10 HTTP integration tests

---

## 1. Intention

We need two truths to coexist without compromise:

1) **Full LLM freedom for research**  
The agent must be able to explore, hypothesize, design, and propose changes across:
- Postgres schema (tables/columns/indexes)
- DSL verbs (new verbs, refactors, new domains)
- Attribute Dictionary (new atomic attributes, derived attributes, constraints, lineage)
- Supporting taxonomies / doc dictionaries

2) **Deterministic, domain-constrained execution**  
Once changes cross into "production reality", the system must be:
- governed (policy + approvals)
- deterministic (repeatable outcomes)
- auditable (why/when/who)
- safe (bounded tool access, bounded verb set)

**Objective:** introduce a first-class boundary that separates *research authoring* from *governed execution*, where the only way to cross the boundary is through a minimal, deterministic SemReg "publish" verb set.

---

## 2. Architectural Approach

### 2.1 Two planes, one system

#### Plane A — Research / Authoring (Wild West)
- LLM has broad capability: inspect, draft, compare, generate patches
- **No direct mutation** of runtime governed state
- Output is always a **ChangeSet** (proposal bundle)

#### Plane B — Governed / Execution (Deterministic)
- LLM is a controller that selects from **allowed verbs**
- Verb availability is filtered by **active SemReg snapshot set + context**
- No raw schema mutation, no free-form patch application
- Every action leaves an audit trail

> The boundary is *not* an informal "mode". It is a deterministic publish pipeline.

---

### 2.2 Core artifact: ChangeSet

A **ChangeSet** is an immutable proposal bundle containing:
- `schema_migrations[]` (SQL migration files; forward-only, *scratch-reversible* required — see §2.7)
- `verb_defs[]` (YAML verb definitions additions/edits)
- `attribute_defs[]` (attribute dictionary additions/edits; types/constraints/lineage)
- optional: `taxonomy_defs[]`, `doc_defs[]`, `notes`, `rationale`

**Key properties:**

1. **Content-addressed** — each ChangeSet is identified by a deterministic hash of its contents.
2. **Immutable** — once created, a ChangeSet cannot be modified. Revisions create a new ChangeSet linked by `supersedes_change_set_id`.
3. **Atomic** — a ChangeSet is published or rejected as a whole. Partial application of a bundle is never permitted. A ChangeSet containing schema migrations, verb defs, and attribute changes either publishes entirely or not at all.

**Atomicity scope (important):** ChangeSet atomicity is guaranteed for **governed semantic state**
(SemReg snapshots + the active snapshot pointer + outbox event emission). Schema migrations are
**governed and validated via dry-run**, but once applied to the real database they are **forward-only**
and are **not** undone by `rollback_snapshot_set`. Pointer rollback reverts semantic meaning and the
allowed execution surface; corrective schema work requires a new forward ChangeSet.

> **Design rule:** if you need independent lifecycle control over schema vs. verbs vs. attributes, author them as separate ChangeSets with an explicit dependency edge (`depends_on_change_set_ids[]`). The publish pipeline enforces topological ordering across dependent ChangeSets.

#### 2.2.1 Content hash canonicalization

`content_hash` must be stable across equivalent ChangeSets. Canonicalization rules:

- Normalize line endings to `\n`
- Sort artifacts by `(artifact_type, ordinal, path)` before hashing
- Hash **artifact content**, not transient metadata (timestamps, author fields, etc.)
- For YAML/JSON artifacts: hash the **parsed, re-serialized canonical form** (stable key ordering)
- Include an explicit `hash_version` prefix (e.g., `v1:`) so future changes don't invalidate history
- Hash algorithm: SHA-256

#### 2.2.2 Bundle format specification

A ChangeSet bundle is a **directory on disk** with a deterministic layout:

```
my-changeset/
├── changeset.yaml           # manifest (required)
├── migrations/
│   ├── 001_create_foo.up.sql
│   ├── 001_create_foo.down.sql
│   ├── 002_add_bar_col.up.sql
│   └── 002_add_bar_col.down.sql
├── verbs/
│   ├── foo.create.yaml
│   └── foo.update.yaml
├── attributes/
│   ├── foo_name.json
│   └── foo_status.json
├── taxonomies/              # optional
│   └── foo_categories.json
└── docs/                    # optional
    └── foo_domain_notes.md
```

**Manifest (`changeset.yaml`):**

```yaml
version: "1"
title: "Add Foo entity with basic verbs"
rationale: "Supports new Foo onboarding workflow per JIRA-1234"
breaking_change: false
depends_on: []               # list of change_set_ids this bundle requires
artifacts:
  migrations:
    - path: migrations/001_create_foo.up.sql
      down: migrations/001_create_foo.down.sql
      ordinal: 1
    - path: migrations/002_add_bar_col.up.sql
      down: migrations/002_add_bar_col.down.sql
      ordinal: 2
  verbs:
    - path: verbs/foo.create.yaml
    - path: verbs/foo.update.yaml
  attributes:
    - path: attributes/foo_name.json
    - path: attributes/foo_status.json
  taxonomies:
    - path: taxonomies/foo_categories.json
  docs:
    - path: docs/foo_domain_notes.md
```

**Wire format for `propose_change_set`:** the MCP tool accepts either a path to a bundle directory or an inline JSON envelope containing the manifest + base64-encoded artifact contents. The CLI wrapper (`cargo x sem-reg propose <bundle-path>`) reads from disk.

---

### 2.3 The boundary: a minimal deterministic governance verb set

This is the "governance DSL" (small surface, heavily validated). These are the *only* verbs allowed to promote research into governed reality:

1. `sem_reg.propose_change_set`
   - inputs: structured bundle (path or inline envelope)
   - output: `change_set_id`, `content_hash`, status = `Draft`
   - **Idempotent:** if an identical `(hash_version, content_hash)` already exists in `Draft/Validated/DryRun*`,
     return the existing `change_set_id` (do not create duplicates)

2. `sem_reg.validate_change_set`
   - **Scope: internal consistency of the ChangeSet in isolation.**
   - Phases 1–3 of the validation pipeline (§2.4): artifact integrity, reference resolution, semantic consistency.
   - Does NOT evaluate against the current active snapshot set.
   - Output: `ValidationReport`, status transitions: `Draft → Validated` or `Draft → Rejected`
   - **Re-runnable:** may be re-executed on the same ChangeSet; each run appends a new `validation_reports` row.

3. `sem_reg.dry_run_change_set`
   - **Scope: prove the ChangeSet applies cleanly against real state.**
   - Applies schema migrations to a scratch schema within a transaction and rolls back.
   - Evaluates verb/attribute compatibility against the current active snapshot set.
   - Runs phases 4–5 of the validation pipeline (§2.4): schema safety, compatibility & policy.
   - Records `evaluated_against_snapshot_set_id` for drift detection.
   - Output: `DryRunReport`, status transitions: `Validated → DryRunPassed` or `Validated → DryRunFailed`
   - **Re-runnable:** may be re-executed (e.g., after environmental changes); each run appends a new report row.

4. `sem_reg.plan_publish`
   - computes a publish plan + diff from current active snapshot set
   - lists impacted verbs/entities/attrs; flags breaking changes
   - produces human-readable and machine-readable impact summary
   - **Read-only** — does not mutate ChangeSet status
   - **Distinct from DryRunReport:** plan_publish additionally computes downstream verb availability
     changes (verbs that become allowed/disallowed), entity resolution impact, and attribute lineage
     effects. DryRunReport proves *applicability*; plan_publish shows *consequences*.

5. `sem_reg.publish_snapshot_set`
   - atomic publish: creates new snapshot set, emits one outbox event
   - projection updates active watermark (single `active_snapshot_set` projection)
   - **Requires status = `DryRunPassed`** (non-negotiable gate)
   - supports single ChangeSet or **batch publish** of a dependency graph (see §2.10)

6. `sem_reg.rollback_snapshot_set`
   - reverts active snapshot pointer to a prior snapshot set (with audit + outbox event)
   - **Pointer-only rollback** — does not reverse schema migrations (see §2.7)
   - corrective schema changes require a new forward ChangeSet

7. `sem_reg.diff_change_sets`
   - compare two ChangeSets, producing a structural diff
   - supports `diff(a, b)` and `diff_against_active(change_set_id)`
   - output: machine-readable diff (added/removed/modified verbs, entities, attributes, migrations)
   - essential for research iteration loops

> Verbs 1–6 are the "cross the boundary" mechanism. Verb 7 is a read-only research tool.

---

### 2.4 Deterministic validation pipeline (ordered phases)

Validation must be boring and exact. Split into **two stages** with different verbs and a well-defined error taxonomy.

#### 2.4.1 Error taxonomy

All errors produced by the validation pipeline use a structured code:

```
{stage}:{category}:{code}
```

**Stages:** `V` (validate, Stage 1), `D` (dry-run, Stage 2)

**Categories and codes:**

| Category | Code | Meaning | Stage |
|----------|------|---------|-------|
| `HASH` | `MISMATCH` | Artifact content hash does not match declared hash | V |
| `HASH` | `MISSING_ARTIFACT` | Declared artifact not found in bundle | V |
| `PARSE` | `SQL_SYNTAX` | SQL migration failed to parse | V |
| `PARSE` | `YAML_SYNTAX` | YAML verb definition failed to parse | V |
| `PARSE` | `YAML_SCHEMA` | YAML verb definition does not conform to schema | V |
| `PARSE` | `JSON_SYNTAX` | JSON attribute definition failed to parse | V |
| `PARSE` | `JSON_SCHEMA` | JSON attribute definition does not conform to schema | V |
| `REF` | `MISSING_ENTITY` | Verb references an entity kind not declared or external | V |
| `REF` | `MISSING_DOMAIN` | Verb references a domain not declared or external | V |
| `REF` | `MISSING_ATTRIBUTE` | Derived attribute references a non-existent input | V |
| `REF` | `MISSING_DEPENDENCY` | `depends_on` references a malformed or unknown ChangeSet ID | V |
| `REF` | `CIRCULAR_DEPENDENCY` | Dependency graph contains a cycle | V |
| `TYPE` | `ATTRIBUTE_MISMATCH` | Attribute type inconsistency (e.g., derived from wrong type) | V |
| `TYPE` | `CONTRACT_INCOMPLETE` | Verb contract missing required params/outputs | V |
| `TYPE` | `LINEAGE_BROKEN` | Derived attribute lineage chain has gaps | V |
| `SCHEMA` | `APPLY_FAILED` | Migration failed to apply to scratch schema | D |
| `SCHEMA` | `NON_TRANSACTIONAL_DDL` | Migration contains non-transactional ops (e.g., `CONCURRENTLY`) | D |
| `SCHEMA` | `FORBIDDEN_DDL` | Migration uses forbidden operations (e.g., `DROP TABLE` without `breaking_change`) | D |
| `SCHEMA` | `DOWN_MISSING` | Migration has no corresponding `down.sql` for scratch cleanup | D |
| `SCHEMA` | `DOWN_FAILED` | `down.sql` failed to apply during scratch cleanup | D |
| `COMPAT` | `BREAKING_UNDECLARED` | Breaking change detected but `breaking_change=true` not set | D |
| `COMPAT` | `ATTR_CONFLICT` | Attribute conflicts with active snapshot set definition | D |
| `COMPAT` | `VERB_CONFLICT` | Verb conflicts with active snapshot set definition | D |
| `COMPAT` | `DEPENDENCY_UNPUBLISHED` | Required dependency ChangeSet is not yet published | D |
| `COMPAT` | `DEPENDENCY_FAILED` | Required dependency ChangeSet is in Rejected/DryRunFailed state | D |
| `POLICY` | `APPROVAL_REQUIRED` | Human approval required for this change category | D |
| `POLICY` | `ROLE_INSUFFICIENT` | Publisher does not have required role | D |

**Error structure:**

```rust
pub struct ValidationError {
    pub code: String,           // e.g., "V:REF:MISSING_ENTITY"
    pub severity: ErrorSeverity,// Error | Warning
    pub message: String,        // human-readable description
    pub artifact_path: Option<String>,  // which artifact caused it
    pub context: serde_json::Value,     // structured remediation context
}

pub enum ErrorSeverity {
    Error,   // blocks status transition
    Warning, // informational, does not block
}
```

The `context` field carries structured data the agent can use for automated remediation (e.g., the missing entity name, the expected type, the conflicting attribute definition).

#### 2.4.2 Stage 1: Structural validation (`validate_change_set`)

These phases evaluate the ChangeSet **in isolation** — no dependency on the active snapshot set.

**Phase 1 — Artifact integrity**
- Verify content hashes match (using canonicalization rules from §2.2.1)
- Verify all declared artifacts are present in the bundle
- Parse SQL migrations (syntax-level, using `sqlparser-rs`)
- Parse YAML verb definitions; validate against verb definition schema
- Parse attribute defs; validate against attribute JSON schema

**Phase 2 — Reference resolution (internal)**
- Resolve internal references within the bundle:
  - verbs referencing entity kinds/domains
  - attributes referencing other attributes for derivations
  - `depends_on_change_set_ids` are well-formed UUIDs
- Detect circular dependencies within `depends_on` graph
- Ensure all declared IDs are present internally or explicitly marked as external

**Phase 3 — Semantic consistency (internal)**
- Type-check attributes (atomic + derived)
- Validate derived attribute inputs exist and type-check within the bundle
- Validate verb contracts are complete and consistent (args, lookup bindings, outputs)
- Produce `ValidationReport`

#### 2.4.3 Stage 2: Environmental validation (`dry_run_change_set`)

These phases evaluate the ChangeSet **against the current active snapshot set and live DB state**.

**Phase 4 — Schema safety (mandatory)**
- Create a scratch schema from current DB state within the same connection
- Apply migrations in order within a transaction
- Verify success, then rollback the transaction
- Apply `down.sql` files in reverse order during scratch cleanup (validate they work)
- Verify no forbidden DDL operations (e.g., `DROP TABLE` without `breaking_change=true`)
- **DDL policy:** ChangeSet migrations must be **transactional**. Disallow non-transactional
  operations such as `CREATE INDEX CONCURRENTLY` in governed ChangeSets. If needed, run
  them via a separate maintenance pipeline outside `publish_snapshot_set`.
- Verify migration idempotence where applicable
- Capture apply timing and result in `DryRunReport`

**Phase 5 — Compatibility & policy**
- Resolve all attribute references against active snapshot set + ChangeSet bundle combined
- Breaking changes require explicit `breaking_change=true` + rationale
- Dependent ChangeSets must already be published, or co-batched in correct topological order
- Verify declared invariants (e.g., required attributes exist for verb groups)
- Evaluate role/policy gates (human approval requirements)
- Record `evaluated_against_snapshot_set_id` for drift detection at publish time

#### 2.4.4 Report structures

**ValidationReport (Stage 1):**
```rust
pub struct ValidationReport {
    pub ok: bool,
    pub errors: Vec<ValidationError>,   // code prefix "V:*"
    pub warnings: Vec<ValidationError>,
    pub internal_diff_summary: Option<DiffSummary>,
}
```

**DryRunReport (Stage 2):**
```rust
pub struct DryRunReport {
    pub ok: bool,
    pub errors: Vec<ValidationError>,   // code prefix "D:*"
    pub warnings: Vec<ValidationError>,
    pub diff_summary: DiffSummary,
    pub breaking_flags: Vec<BreakingFlag>,
    pub impacted_verbs: Vec<String>,
    pub impacted_entities: Vec<String>,
    pub impacted_attributes: Vec<String>,
    pub scratch_schema_apply_ms: u64,
    pub evaluated_against_snapshot_set_id: Uuid,
}
```

---

### 2.5 Session "agent mode" as a policy gate

Introduce an explicit **Agent Mode** bound to the session:

#### `Mode::Research`
- **Allowed:** introspection tools (full surface — see §2.6), SemReg read tools, ChangeSet authoring verbs, diff tools
- **Disallowed:** governed business verbs that mutate real state (unless explicitly whitelisted)

#### `Mode::Governed` (default)
- **Allowed:** governed business verbs + governance verbs + read-only DB introspection (limited surface — see §2.6)
- **Disallowed:** free-form authoring, full schema introspection, direct patch generation

**Enforcement mechanism:** tool/verb allowlists driven by mode + SemReg context resolution. The agent does not "choose" compliance — the available verb surface is physically constrained by mode. If strict SemReg is enabled, governed mode fails closed when no allowed verbs exist.

---

### 2.6 Schema & Dictionary visibility (what the agent can see)

DB introspection is available in **both modes**, with different surfaces:

#### Research mode (full introspection)
- `db_introspect.list_schemas()` — enumerate schemas
- `db_introspect.list_tables(schema)` — tables in a schema
- `db_introspect.describe_table(schema, table)` — columns, types, nullability, PK/FK, indexes, check constraints
- `db_introspect.table_stats(schema, table)` — `reltuples` estimate, table size, index sizes (policy-gated)
- `db_introspect.sample_rows(schema, table, limit)` — behind a privacy/policy gate, default off

#### Governed mode (read-only health check)
- `db_introspect.verify_table_exists(schema, table)` — confirm expected structure matches SemReg declarations
- `db_introspect.describe_table(schema, table)` — read-only, same as Research (needed for post-publish health checks)

> Governed mode introspection exists so the agent can verify that a published migration landed correctly. It is strictly read-only and cannot be used to generate patches or authoring proposals.

#### SemReg dictionary tools (both modes, read-only)
- list/describe verbs, entity types, attributes from the active snapshot set
- show lineage for derived attributes
- show per-verb lookup bindings (schema/table/pk/search key)

**Policy rule:** sampling of real row values (`sample_rows`) is never available in Governed mode by default and must be explicitly enabled per-session with justification (privacy / PII constraints).

**SemReg integration:** the scanner can optionally publish `TableDef/ColumnDef` snapshots into SemReg, creating a governed "physical data dictionary" that maps logical attributes to physical `(schema, table, column)` locations.

---

### 2.7 Rollback semantics & migration reversibility

Rollback is **pointer-only**: `sem_reg.rollback_snapshot_set` reverts the active snapshot pointer to a prior snapshot set. It does **not** reverse schema migrations.

**Rationale:** down-migrations in a production compliance system are dangerous. A reversed migration may drop columns containing live data, violate audit retention requirements, or break concurrent connections. Forward-only migrations are the safer default.

**Corrective workflow:**
1. Rollback the snapshot pointer (verbs/attributes revert immediately)
2. Author a new ChangeSet containing a corrective migration (forward-only)
3. Publish the corrective ChangeSet through the standard pipeline

**Migration reversibility requirement:** all schema migrations in a ChangeSet **should** include a `down.sql` or equivalent reverse operation, documented in the artifact metadata. This is used exclusively for scratch-schema cleanup during `dry_run_change_set` — never applied to the governed database. The `down.sql` is validated for syntactic correctness during dry-run but is not a production rollback mechanism.

---

### 2.8 Outbox & projection guarantees

Publishing a snapshot set emits exactly one outbox event per `publish_snapshot_set` invocation:

**Event:** `snapshot_set_published`
```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct SnapshotSetPublished {
    pub snapshot_set_id: Uuid,
    pub change_set_id: Uuid,          // or batch_id for batch publishes
    pub published_at: DateTime<Utc>,
    pub publisher: String,
    pub content_hash: String,
    pub prior_snapshot_set_id: Option<Uuid>,  // for rollback chain traversal
    pub sequence_number: i64,                 // monotonic, for ordering guarantee
}
```

**Projection:** a single `active_snapshot_set` projection (one row) updated by the projection writer.

**Guarantees:**
- **Idempotent:** the projection writer uses `snapshot_set_id` as a deduplication key. Replaying the same outbox event is a no-op.
- **Ordered:** outbox events carry a monotonic `sequence_number`; the projection writer rejects out-of-order events.
- **Failure recovery:** if the projection writer fails mid-update, the outbox event remains unconsumed. The next projection run replays from the last committed watermark. No partial state is visible.
- **Single writer:** only one projection writer instance processes `snapshot_set_published` events, eliminating write contention.

> This aligns with the existing ob-poc outbox infrastructure. The key constraint is: one publish → one event → one projection update. No HOL blocking across projection types.

---

### 2.9 Publish ordering & lock discipline

`sem_reg.publish_snapshot_set` must be deterministic and safe under concurrency. Required ordering:

1. **Acquire a global publish lock** (Postgres advisory lock) to ensure single publisher.
2. Re-check ChangeSet status is `DryRunPassed` and record the **active snapshot set id** the publish is based on.
3. **Drift detection:** compare `evaluated_against_snapshot_set_id` (from the DryRunReport) against the current active snapshot set. If they differ, **fail-fast** with a `PUBLISH:DRIFT_DETECTED` error. The ChangeSet must be re-dry-run against the new active state.
4. **Apply migrations to the governed database** (forward-only, transactional DDL only; see §2.4).
5. Write the new SemReg snapshots + snapshot set row.
6. Emit exactly one outbox event `snapshot_set_published`.
7. Commit.
8. Release advisory lock.

**Concurrency rule:** drift detection is the default and only mode. There is no "force publish" bypass.

---

### 2.10 Batch publish (dependency graph)

When a ChangeSet has `depends_on_change_set_ids`, and those dependencies are not yet published, a **batch publish** can atomically publish the entire dependency graph in one transaction:

**Batch publish rules:**
1. All ChangeSets in the batch must be in `DryRunPassed` status.
2. The batch is topologically sorted by `depends_on_change_set_ids`. Cycles are rejected.
3. All migrations across the batch are applied in topological order within one transaction.
4. One snapshot set is created covering all ChangeSets in the batch.
5. One outbox event is emitted (`change_set_id` is replaced by `batch_id` referencing all included ChangeSets).
6. Drift detection applies to the batch as a whole: if any ChangeSet's `evaluated_against_snapshot_set_id` is stale, the batch fails.

**Batch record:**

```sql
CREATE TABLE sem_reg_authoring.publish_batches (
    batch_id        uuid PRIMARY KEY,
    change_set_ids  uuid[] NOT NULL,
    snapshot_set_id uuid NOT NULL,
    published_at    timestamptz NOT NULL,
    publisher       text NOT NULL
);
```

**Alternative:** if batch complexity is too high for the initial implementation, require dependencies to be published sequentially (each acquiring/releasing the advisory lock). Document this as a known limitation with a clear upgrade path to batch publish.

---

### 2.11 ChangeSet dependency graph semantics

`depends_on_change_set_ids[]` declares explicit ordering constraints:

| Dependent state | Effect on `validate` (Stage 1) | Effect on `dry_run` (Stage 2) | Effect on `publish` |
|-----------------|-------------------------------|-------------------------------|---------------------|
| `Draft` | Warning: dependency not yet validated | Error: `D:COMPAT:DEPENDENCY_UNPUBLISHED` | Blocked (unless co-batched) |
| `Validated` | OK (structural deps resolved) | Error: `D:COMPAT:DEPENDENCY_UNPUBLISHED` | Blocked (unless co-batched) |
| `DryRunPassed` | OK | OK (co-batch allowed) | OK (co-batch or sequential) |
| `DryRunFailed` | Warning | Error: `D:COMPAT:DEPENDENCY_FAILED` | Blocked |
| `Rejected` | Warning | Error: `D:COMPAT:DEPENDENCY_FAILED` | Blocked |
| `Published` | OK | OK | OK |
| `Superseded` | OK (treat as published) | OK (treat as published) | OK |

**Stage 1 is lenient:** validation checks structural completeness. Dependencies in early states produce warnings, not errors, to enable parallel authoring.

**Stage 2 is strict:** dry-run must prove environmental applicability. Unpublished or failed dependencies are errors unless co-batched.

---

### 2.12 Superseded status transitions

A Published ChangeSet transitions to `Superseded` **automatically** during `publish_snapshot_set` when the newly published ChangeSet declares `supersedes_change_set_id` pointing to it.

**Rules:**
- Only `Published` ChangeSets can be superseded. If `supersedes_change_set_id` points to a ChangeSet in any other state, it is treated as a lineage annotation only (no status change on the target).
- The `Superseded` transition is recorded with an audit entry (`superseded_by`, `superseded_at`).
- Superseded ChangeSets remain queryable for audit purposes. They are never deleted.
- Supersession chains are walkable: `change_set_id → supersedes_change_set_id → ...` for full lineage.

---

### 2.13 Concurrent research sessions

Multiple agents (or the same agent in multiple sessions) may author ChangeSets against the same active snapshot set simultaneously. This is explicitly supported:

**Authoring:** no coordination required. Each session produces independent ChangeSets. Content-addressing means identical proposals are deduplicated by `propose_change_set`.

**Dry-run:** each dry-run records `evaluated_against_snapshot_set_id`. If another session publishes between dry-run and publish, drift detection catches it.

**Staleness signal:** when the active snapshot set advances, any ChangeSet with `DryRunPassed` whose `evaluated_against_snapshot_set_id` no longer matches the active pointer is flagged as `stale_dry_run` in query results. This is a **query-time computed flag**, not a status transition — the ChangeSet remains `DryRunPassed` but the agent is warned that re-dry-run is required before publish.

**No pessimistic locking of research:** the advisory lock in §2.9 only applies at publish time. Research and dry-run are fully concurrent.

---

### 2.14 Retention & cleanup policy

`sem_reg_authoring` state is retained indefinitely by default (compliance audit trail). Specific policies:

| Status | Retention | Rationale |
|--------|-----------|-----------|
| `Published` | Permanent | Audit trail: what was published, when, by whom |
| `Superseded` | Permanent | Lineage: what was replaced and why |
| `Rejected` / `DryRunFailed` | 90 days (configurable) | Research artifacts; useful for debugging but not permanent |
| `Draft` / `Validated` (orphaned) | 30 days since last activity (configurable) | Abandoned proposals; clean up to reduce noise |

**Cleanup mechanism:** a scheduled job (`sem_reg_authoring.cleanup`) runs daily, archiving expired ChangeSets to `sem_reg_authoring.change_sets_archive` (same schema, cold table). Archived ChangeSets are excluded from default queries but remain accessible via explicit archive queries.

**Validation reports:** follow the retention of their parent ChangeSet.

---

## 3. Capabilities

### 3.1 Research / Authoring capabilities
- **Discover** current schema shape (tables/columns/FKs/indexes) via full `db_introspect` surface
- **Discover** current governed semantics (active verbs/entities/attrs) via SemReg read tools
- **Propose** new schema + new verbs + new attributes as a ChangeSet
- **Diff** ChangeSets against each other or against the active snapshot set
- **Iterate**: version ChangeSets via `supersedes_change_set_id`, compare diffs, annotate rationale
- **Validate** (Stage 1) — internal consistency check, actionable remediation output with structured error codes
- **Dry-run** (Stage 2) — prove applicability against real state, get environmental compatibility report
- **Plan** publish and understand blast radius + downstream verb availability changes before committing

### 3.2 Governed / Execution capabilities
- Run only **SemReg-allowed verbs** for the current context, constrained via `ContextEnvelope`
- **Pre-constrained verb search**: allowed verb set threaded into `HybridVerbSearcher` (not just post-filtered)
- **dsl_execute SemReg gate**: every verb FQN in parsed AST checked against envelope before execution
- "Fail closed" when SemReg is strict and no allowed verbs exist
- **Post-publish health check** via limited `db_introspect` surface (verify migrations landed correctly)
- **TOCTOU recheck**: deterministic fingerprint comparison between resolution and execution time
- Strong audit: every decision and execution recorded with:
  - active snapshot set id
  - evidence / policy gates invoked
  - `AllowedVerbSetFingerprint` (SHA-256 of sorted allowed verb FQNs)
  - pruned verb count + structured `PruneReason`
  - TOCTOU recheck result (if performed)
  - input params (redacted where required)

### 3.3 Governance capabilities
- Publish is atomic (single or batch) and produces:
  - `snapshot_set_id` + `published_at` + `publisher`
  - one outbox event → one idempotent projection update
- Rollback is pointer-only with full audit trail
- ChangeSet dependency graph enforces topological publish ordering
- Supersession chain provides full lineage for governed changes

---

## 4. ChangeSet lifecycle

```
Draft
  │
  ▼  sem_reg.validate_change_set
Validated ──────────► Rejected
  │
  ▼  sem_reg.dry_run_change_set
DryRunPassed ───────► DryRunFailed
  │
  ▼  sem_reg.publish_snapshot_set
Published
  │
  └──► Superseded  (automatic: when a ChangeSet declaring supersedes_change_set_id is published)
```

**Status transitions are forward-only.** Validation and dry-run may be **re-executed** on the same
immutable ChangeSet (to capture new reports or re-evaluate after environmental changes), but structural
fixes require authoring a new ChangeSet (linked via `supersedes_change_set_id`). Every execution
writes a new `validation_reports` row.

**Query-time flags (not status transitions):**
- `stale_dry_run`: `DryRunPassed` but `evaluated_against_snapshot_set_id != active_snapshot_set_id`

---

## 5. Observability & metrics

### 5.1 Instrumentation points

Every governance verb emits structured metrics and traces. The following instrumentation hooks are required:

#### ChangeSet lifecycle metrics

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `semreg.changeset.proposed_total` | Counter | `status=draft` | ChangeSets proposed |
| `semreg.changeset.validated_total` | Counter | `result={ok,rejected}` | Validation completions |
| `semreg.changeset.dryrun_total` | Counter | `result={passed,failed}` | Dry-run completions |
| `semreg.changeset.published_total` | Counter | `mode={single,batch}` | Successful publishes |
| `semreg.changeset.rollback_total` | Counter | — | Snapshot pointer rollbacks |
| `semreg.changeset.superseded_total` | Counter | — | ChangeSets superseded |
| `semreg.changeset.active_draft_count` | Gauge | — | Currently open Drafts (cleanup signal) |
| `semreg.changeset.stale_dryrun_count` | Gauge | — | DryRunPassed with stale evaluation pointer |

#### Validation & dry-run metrics

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `semreg.validate.duration_ms` | Histogram | `stage={validate,dryrun}` | Time per validation/dry-run |
| `semreg.validate.errors_total` | Counter | `stage, category, code` | Errors by taxonomy code |
| `semreg.validate.warnings_total` | Counter | `stage, category, code` | Warnings by taxonomy code |
| `semreg.dryrun.scratch_apply_ms` | Histogram | — | Scratch schema migration apply time |
| `semreg.dryrun.scratch_cleanup_ms` | Histogram | — | Scratch schema down-migration time |

#### Publish pipeline metrics

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `semreg.publish.duration_ms` | Histogram | `mode={single,batch}` | Total publish wall time |
| `semreg.publish.lock_wait_ms` | Histogram | — | Advisory lock acquisition time |
| `semreg.publish.migration_apply_ms` | Histogram | — | DDL migration apply time (governed DB) |
| `semreg.publish.drift_detected_total` | Counter | — | Drift failures (stale dry-run at publish) |
| `semreg.publish.lock_contention_total` | Counter | — | Advisory lock contention events |
| `semreg.outbox.emit_total` | Counter | `event_type` | Outbox events emitted |
| `semreg.projection.lag_ms` | Gauge | `projection_name` | Time between outbox emit and projection commit |

#### Mode & session metrics

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `semreg.session.mode_switches_total` | Counter | `from, to` | Mode transitions |
| `semreg.session.verb_denied_total` | Counter | `mode, verb` | Verb access denied by mode gating |
| `semreg.session.fail_closed_total` | Counter | — | Governed mode fail-closed events |

### 5.2 Structured audit log

Every governance verb invocation writes a structured audit entry:

```rust
pub struct GovernanceAuditEntry {
    pub entry_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub verb: String,                       // e.g., "sem_reg.publish_snapshot_set"
    pub agent_session_id: Uuid,
    pub agent_mode: AgentMode,
    pub change_set_id: Option<Uuid>,
    pub snapshot_set_id: Option<Uuid>,
    pub active_snapshot_set_id: Uuid,       // at time of invocation
    pub result: AuditResult,                // Success | Failure { code, message }
    pub duration_ms: u64,
    pub metadata: serde_json::Value,        // verb-specific context (redacted where required)
}
```

Audit entries are written to `sem_reg_authoring.governance_audit_log` and are **permanent** (no retention expiry).

### 5.3 Health check endpoints

For operational monitoring:

- `GET /health/semreg/active` — current active snapshot set id, published_at, publisher
- `GET /health/semreg/projection-lag` — time since last projection update
- `GET /health/semreg/pending-changesets` — count by status
- `GET /health/semreg/stale-dryruns` — count of DryRunPassed with stale evaluation pointer

### 5.4 Alerting thresholds (recommended)

| Condition | Severity | Action |
|-----------|----------|--------|
| `projection.lag_ms > 5000` | Warning | Investigate projection writer |
| `projection.lag_ms > 30000` | Critical | Projection writer likely stalled |
| `publish.drift_detected_total` spike | Info | High concurrency; normal but worth monitoring |
| `publish.lock_contention_total` sustained | Warning | Consider publish queue or backoff |
| `session.fail_closed_total` spike | Critical | SemReg may be misconfigured or empty |
| `validate.errors_total` by `D:SCHEMA:APPLY_FAILED` spike | Warning | Schema drift or migration quality issue |
| `active_draft_count > 50` | Info | Cleanup may be overdue or agents are abandoning proposals |

---

## 6. Claude Code TODO (Implementation Plan)

> Goal: implement Research→Governed boundary with ChangeSets + publish pipeline + mode gating.  
> **Ordering rationale:** storage model first (everything depends on it), then validation pipeline incrementally, then publish, then mode gating, then observability, then integration tests.  
> **Claude Code note:** add "→ IMMEDIATELY proceed to Phase N" at every gate + progress % + E-invariant making continuation mandatory.

---

### Phase 1: Foundation (0–15%)

#### A) ChangeSet storage model (DB)
Create a new authoring area (same DB, separate schema: `sem_reg_authoring`):

- [ ] Migration: `sem_reg_authoring.change_sets`
  - `change_set_id uuid PK`
  - `status enum { Draft, UnderReview, Approved, Validated, Rejected, DryRunPassed, DryRunFailed, Published, Superseded }` (9-state; UnderReview/Approved added for stewardship workflow)
  - `content_hash text NOT NULL`
  - `hash_version text NOT NULL DEFAULT 'v1'`
  - `UNIQUE(hash_version, content_hash)`
  - `title text NOT NULL`, `rationale text`
  - `created_at timestamptz`, `created_by text`
  - `supersedes_change_set_id uuid NULL FK → change_sets`
  - `superseded_by uuid NULL FK → change_sets`
  - `superseded_at timestamptz NULL`
  - `depends_on_change_set_ids uuid[] NULL`
  - `evaluated_against_snapshot_set_id uuid NULL` (set by dry-run)

- [ ] Migration: `sem_reg_authoring.change_set_artifacts`
  - `artifact_id uuid PK`, `change_set_id FK`
  - `artifact_type enum { MigrationSql, MigrationDownSql, VerbYaml, AttributeJson, TaxonomyJson, DocJson }`
  - `ordinal int NOT NULL` (for migration ordering)
  - `path text`, `content text NOT NULL`
  - `content_hash text NOT NULL`
  - `metadata jsonb`

- [ ] Migration: `sem_reg_authoring.validation_reports`
  - `report_id uuid PK`, `change_set_id FK`
  - `stage enum { Validate, DryRun }`
  - `ran_at timestamptz`, `ok bool`
  - `report jsonb` (errors/warnings/diff/breaking flags/impact summary)

- [ ] Migration: `sem_reg_authoring.governance_audit_log`
  - `entry_id uuid PK`
  - `timestamp timestamptz`, `verb text`, `agent_session_id uuid`, `agent_mode text`
  - `change_set_id uuid NULL`, `snapshot_set_id uuid NULL`, `active_snapshot_set_id uuid`
  - `result jsonb`, `duration_ms bigint`, `metadata jsonb`

- [ ] Migration: `sem_reg_authoring.publish_batches`
  - `batch_id uuid PK`
  - `change_set_ids uuid[] NOT NULL`
  - `snapshot_set_id uuid NOT NULL`
  - `published_at timestamptz`, `publisher text`

- [ ] Rust: `ChangeSet`, `ChangeSetArtifact`, `ValidationReport`, `GovernanceAuditEntry` structs + SQLx CRUD
- [ ] Rust: content hash canonicalization module (`hash_version=v1`, SHA-256)
- [ ] Rust: error taxonomy types (`ValidationError`, `ErrorSeverity`)

**Acceptance:** ChangeSet can be created, versioned, queried, and re-validated without mutating governed SemReg state.

→ IMMEDIATELY proceed to Phase 2. Progress: 15%.

---

### Phase 2: Validation pipeline — incremental (15–40%)

#### B1) Stage 1 — Artifact integrity + reference resolution + semantic consistency
- [ ] Phase 1: hash verification (canonicalization rules from §2.2.1), SQL parse check (`sqlparser-rs`), YAML parse, JSON schema validation
- [ ] Phase 2: internal reference resolution (verbs ↔ entities, attributes ↔ lineage, dependency IDs, cycle detection)
- [ ] Phase 3: semantic consistency (type checking, derived attribute inputs, verb contract completeness)
- [ ] `sem_reg.validate_change_set` verb handler: runs phases 1–3, writes `ValidationReport` with structured error codes, transitions status
- [ ] Emit metrics: `semreg.changeset.validated_total`, `semreg.validate.duration_ms`, `semreg.validate.errors_total`

**Acceptance:** `validate_change_set` produces a structured report with taxonomy-coded errors; invalid bundles are rejected with actionable remediation context.

→ IMMEDIATELY proceed to B2. Progress: 25%.

#### B2) Stage 2 — Schema safety + environmental compatibility
- [ ] Phase 4: scratch schema validation
  - create temporary schema from current DB state
  - apply migrations in order within a transaction
  - apply `down.sql` in reverse for scratch cleanup
  - verify success, then rollback the transaction
  - reject non-transactional operations (e.g., `CONCURRENTLY`) per DDL policy
  - capture apply timing in `DryRunReport.scratch_schema_apply_ms`
- [ ] Phase 5: compatibility & policy
  - resolve all references against active snapshot set + ChangeSet combined
  - flag breaking changes, enforce `breaking_change=true` requirement
  - evaluate dependency graph (dependent ChangeSets published or co-batched — see §2.11)
  - record `evaluated_against_snapshot_set_id`
- [ ] `sem_reg.dry_run_change_set` verb handler: runs phases 4–5, writes `DryRunReport` with structured error codes, transitions status
- [ ] Emit metrics: `semreg.changeset.dryrun_total`, `semreg.dryrun.scratch_apply_ms`, `semreg.validate.errors_total`

**Acceptance:** `dry_run_change_set` fails early on DB/schema incompatibilities and blocks publish unless passed.

→ IMMEDIATELY proceed to B3. Progress: 35%.

#### B3) Diff tooling
- [ ] `sem_reg.diff_change_sets(a, b)` — structural diff between two ChangeSets
- [ ] `sem_reg.diff_against_active(change_set_id)` — diff ChangeSet against current active snapshot set
- [ ] Output: machine-readable diff (added/removed/modified verbs, entities, attributes, migrations)
- [ ] Human-readable summary for agent consumption

**Acceptance:** agent can compare iterations and understand blast radius during research.

→ IMMEDIATELY proceed to Phase 3. Progress: 40%.

---

### Phase 3: Publish pipeline (40–60%)

#### C) Governance verbs + publish
- [ ] `sem_reg.propose_change_set` — create Draft ChangeSet from bundle path or inline envelope (idempotent by `hash_version + content_hash`)
- [ ] `sem_reg.plan_publish` — read-only impact summary: diff against active, downstream verb availability changes, entity resolution impact, attribute lineage effects
- [ ] `sem_reg.publish_snapshot_set` — atomic publish with full ordering (§2.9):
  - status must be `DryRunPassed`
  - acquire advisory lock; fail if locked
  - detect drift: `evaluated_against_snapshot_set_id != active_snapshot_set_id` → fail-fast
  - apply transactional DDL migrations to governed DB (forward-only)
  - create new snapshot set from active + deltas
  - handle supersession: transition target ChangeSet to `Superseded` if applicable (§2.12)
  - emit one `snapshot_set_published` outbox event (with `prior_snapshot_set_id` + `sequence_number`)
  - projection writer updates `active_snapshot_set` watermark
  - outbox event is idempotent (dedup by `snapshot_set_id`)
  - write `GovernanceAuditEntry`
- [ ] Batch publish support (§2.10): topological sort, single transaction, single outbox event, `publish_batches` record
- [ ] `sem_reg.rollback_snapshot_set` — pointer-only revert with audit + outbox event
- [ ] list/get/status query APIs for ChangeSets (include `stale_dry_run` computed flag)
- [ ] Emit metrics: `semreg.changeset.published_total`, `semreg.publish.duration_ms`, `semreg.publish.lock_wait_ms`, `semreg.publish.drift_detected_total`, `semreg.outbox.emit_total`

**Acceptance:** agent can author → validate → dry-run → publish entirely through deterministic verbs. Publish is atomic. Batch publish handles dependency graphs.

→ IMMEDIATELY proceed to Phase 4. Progress: 60%.

---

### Phase 4: Mode gating & introspection (60–75%)

#### D) Agent Mode plumbing
- [ ] Add `AgentMode { Research, Governed }` to session context (persist per agent session)
- [ ] Default mode = `Governed`
- [ ] Add verb: `agent.set_mode(mode)` with policy gating (e.g., require explicit confirmation)
- [ ] Thread mode through orchestrator → intent pipeline → tool router
- [ ] Implement tool/verb allowlists per mode (§2.5)
- [ ] Emit metrics: `semreg.session.mode_switches_total`, `semreg.session.verb_denied_total`, `semreg.session.fail_closed_total`

**Acceptance:** tools/verbs are filtered deterministically based on mode. No accidental cross-boundary mutations.

→ IMMEDIATELY proceed to D2. Progress: 65%.

#### D2) DB introspection tools
- [ ] Research mode surface:
  - `db_introspect.list_schemas()`
  - `db_introspect.list_tables(schema)`
  - `db_introspect.describe_table(schema, table)` — columns, types, constraints, indexes, FKs
  - `db_introspect.table_stats(schema, table)` — reltuples, sizes (policy-gated)
  - `db_introspect.sample_rows(schema, table, limit)` — policy-gated, default off
- [ ] Governed mode surface:
  - `db_introspect.verify_table_exists(schema, table)`
  - `db_introspect.describe_table(schema, table)` — read-only, for post-publish health checks
- [ ] Mode-based gating enforcement in tool router

**Acceptance:** research agent can ground proposals in real DB structure; governed agent can verify post-publish state.

→ IMMEDIATELY proceed to Phase 5. Progress: 75%.

---

### Phase 5: SemReg scanner enhancements (75–85%)

#### E) Schema + dictionary integration
- [ ] Extend scanner to include CRUD mapping in VerbContracts:
  - `crud.schema`, `crud.table`, `crud.operation` in contract metadata
- [ ] Implement explicit `maps_to` → `(schema, table, column)` mapping:
  - prefer `schema.table.column` form OR explicit structured YAML fields
  - populate `AttributeSource { schema, table, column }` deterministically
- [ ] Ensure scanner output is stable (canonical ordering) to reduce hash churn
- [ ] Optional: publish `TableDef/ColumnDef` snapshots into SemReg for governed physical data dictionary

**Acceptance:** SemReg "data dictionary" objects explain where attributes and verbs live in the physical schema.

→ IMMEDIATELY proceed to Phase 6. Progress: 85%.

---

### Phase 6: Observability & operational infrastructure (85–92%)

#### F) Metrics & health checks
- [ ] Instrument all governance verbs with metrics from §5.1
- [ ] Implement health check endpoints (§5.3):
  - `/health/semreg/active`
  - `/health/semreg/projection-lag`
  - `/health/semreg/pending-changesets`
  - `/health/semreg/stale-dryruns`
- [ ] Wire metrics to Prometheus / metrics sink (existing ob-poc infra)
- [ ] Implement `stale_dry_run` computed flag on ChangeSet queries

#### G) Retention & cleanup
- [ ] Implement `sem_reg_authoring.change_sets_archive` table (same schema as `change_sets`)
- [ ] Implement daily cleanup job: archive `Rejected`/`DryRunFailed` > 90 days, orphaned `Draft`/`Validated` > 30 days
- [ ] Validation reports follow parent ChangeSet retention

**Acceptance:** operational visibility into the governance pipeline; stale state is cleaned up automatically.

→ IMMEDIATELY proceed to Phase 7. Progress: 92%.

---

### Phase 7: Integration tests & CLI (92–100%)

#### H) Tests
- [ ] E2E: create ChangeSet → validate → dry-run → publish → new verb appears as allowed
- [ ] E2E: ChangeSet with dependencies → batch publish in topological order → all verbs active
- [ ] E2E: supersession chain → publish new ChangeSet → predecessor marked Superseded
- [ ] Negative: strict SemReg + empty registry fails closed with clear error
- [ ] Negative: publish with status != `DryRunPassed` is rejected
- [ ] Negative: ChangeSet with failing migration is caught at dry-run, not at publish
- [ ] Negative: publish fails fast on drift (active snapshot set changed since dry-run)
- [ ] Negative: migration containing `CONCURRENTLY` is rejected by dry-run/policy gate
- [ ] Negative: circular dependency in `depends_on_change_set_ids` is rejected at validation
- [ ] Regression: publish is atomic, watermark advances once, outbox event emitted once
- [ ] Regression: replay of outbox event is idempotent (no double-publish)
- [ ] Regression: `propose_change_set` is idempotent by `(hash_version, content_hash)`
- [ ] Regression: batch publish — single transaction, single outbox event, single snapshot set
- [ ] Mode: Research mode cannot execute governed business verbs
- [ ] Mode: Governed mode cannot access full introspection or authoring verbs
- [ ] Mode: Governed mode CAN access limited introspection for health checks
- [ ] Observability: governance verbs emit expected metrics
- [ ] Cleanup: archived ChangeSets excluded from default queries, accessible via archive query

#### I) CLI wrappers
- [ ] `cargo x sem-reg propose <bundle-path>`
- [ ] `cargo x sem-reg validate <change-set-id>`
- [ ] `cargo x sem-reg dry-run <change-set-id>`
- [ ] `cargo x sem-reg plan <change-set-id>`
- [ ] `cargo x sem-reg publish <change-set-id>`
- [ ] `cargo x sem-reg publish-batch <change-set-id,...>`
- [ ] `cargo x sem-reg rollback <snapshot-set-id>`
- [ ] `cargo x sem-reg diff <a> <b>`
- [ ] `cargo x sem-reg diff-active <change-set-id>`
- [ ] `cargo x sem-reg status <change-set-id>`
- [ ] `cargo x sem-reg list [--status=<status>] [--stale]`
- [ ] `cargo x sem-reg health`

**Acceptance:** the workflow is safe, testable, repeatable, and fully observable.

→ Progress: 100%. Implementation complete.

---

## 7. Definition of Done

- Research mode can propose schema + verbs + attribute dictionary updates as a ChangeSet.
- Governed mode can only execute deterministic verbs allowed by the active snapshot set.
- The only way to promote proposals into governed reality is via:
  `propose → validate → dry-run → plan → publish`.
- Validation is split: internal consistency (Stage 1) vs. environmental compatibility (Stage 2).
- All validation errors use the structured taxonomy (`{stage}:{category}:{code}`) with remediation context.
- Dry-run against a scratch schema is **mandatory**, not optional.
- Publish requires `DryRunPassed` status — non-negotiable gate.
- Publish is atomic for semantic state and produces a single idempotent outbox event driving the active projection.
- Batch publish handles dependency graphs in a single transaction.
- Drift detection prevents stale dry-runs from reaching publish.
- Rollback is pointer-only; schema corrections require a new forward ChangeSet.
- ChangeSet atomicity: a bundle publishes entirely or not at all.
- ChangeSet dependencies are explicitly declared and topologically ordered at publish time.
- Supersession is automatic and auditable.
- Diff tooling enables meaningful research iteration.
- All governance verbs emit structured metrics and audit entries.
- Health check endpoints provide operational visibility.
- Retention policy keeps audit-critical state permanently and cleans up abandoned proposals.

---

## Appendix A: ChangeSet status enum (9-state)

```rust
/// Implemented in: rust/crates/sem_os_core/src/authoring/types.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeSetStatus {
    Draft,
    UnderReview,     // Added for stewardship workflow
    Approved,        // Added for stewardship workflow
    Validated,
    Rejected,
    DryRunPassed,
    DryRunFailed,
    Published,
    Superseded,
}
```

> **Note:** `UnderReview` and `Approved` states were added to support the stewardship changeset workflow (migrations 097, 101). Migration 101 fixes the CHECK constraint to include all 9 values.

---

## Appendix B: Outbox event schema

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct SnapshotSetPublished {
    pub snapshot_set_id: Uuid,
    pub change_set_id: Option<Uuid>,        // None for batch publish
    pub batch_id: Option<Uuid>,             // Some for batch publish
    pub published_at: DateTime<Utc>,
    pub publisher: String,
    pub content_hash: String,
    pub prior_snapshot_set_id: Option<Uuid>, // for rollback chain traversal
    pub sequence_number: i64,                // monotonic, for ordering guarantee
}
```

---

## Appendix C: Key design decisions log

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Two-plane model | Research vs Governed | Authoring produces ChangeSets; execution is constrained by snapshot set |
| Canonical boundary | Publish pipeline | Only governance verbs promote changes; no informal "modes" |
| ChangeSet atomicity | All-or-nothing | Partial publish creates ambiguous governed state |
| Validation split | `validate` vs `dry_run` | Stage 1 is fast/stateless (internal); Stage 2 is environmental (against live state) |
| Schema migrations | Forward-only | Down-migrations in production compliance systems are dangerous |
| Migration reversibility | Scratch-reversible only | `down.sql` used for scratch cleanup during dry-run; never applied to governed DB |
| Rollback scope | Pointer-only | Reverts semantic state; schema corrections require forward ChangeSet |
| Publish semantics | Single outbox event | One publish → one event → one projection run; no HOL blocking |
| Governed mode introspection | Limited read-only surface | Post-publish health checks need DB verification; no patch generation |
| ChangeSet identity | Content-addressed | Stable hash with canonicalization + `hash_version` prefix |
| Propose idempotency | By `(hash_version, content_hash)` | Prevent duplicate proposals from concurrent sessions |
| DDL policy | Transactional only | Disallow `CONCURRENTLY` in governed ChangeSets; maintenance pipeline for non-transactional ops |
| Drift handling | Fail-fast at publish | If active snapshot changed since dry-run, require re-dry-run; no force bypass |
| Batch publish | Single transaction, single event | Dependency graphs publish atomically; alternative is sequential with upgrade path |
| Dependency graph | Lenient at Stage 1, strict at Stage 2 | Parallel authoring supported; environmental proof required before publish |
| Superseded trigger | Automatic at publish time | Publisher walks `supersedes_change_set_id` chain; only targets `Published` status |
| Concurrent research | No pessimistic locking | Advisory lock only at publish; `stale_dry_run` flag for query-time warning |
| Retention | Permanent for Published/Superseded; TTL for failures/drafts | Audit compliance + noise reduction |
| Error taxonomy | `{stage}:{category}:{code}` | Structured codes enable automated agent remediation loops |
| Bundle format | Directory with `changeset.yaml` manifest | Simple, file-system friendly, works with CLI and MCP inline envelope |
| Observability | Metrics + audit log + health endpoints | Governance pipeline must be fully observable in production |

---

## Appendix D: Error code quick reference

```
V:HASH:MISMATCH              V:HASH:MISSING_ARTIFACT
V:PARSE:SQL_SYNTAX            V:PARSE:YAML_SYNTAX
V:PARSE:YAML_SCHEMA           V:PARSE:JSON_SYNTAX
V:PARSE:JSON_SCHEMA
V:REF:MISSING_ENTITY          V:REF:MISSING_DOMAIN
V:REF:MISSING_ATTRIBUTE        V:REF:MISSING_DEPENDENCY
V:REF:CIRCULAR_DEPENDENCY
V:TYPE:ATTRIBUTE_MISMATCH      V:TYPE:CONTRACT_INCOMPLETE
V:TYPE:LINEAGE_BROKEN

D:SCHEMA:APPLY_FAILED          D:SCHEMA:NON_TRANSACTIONAL_DDL
D:SCHEMA:FORBIDDEN_DDL         D:SCHEMA:DOWN_MISSING
D:SCHEMA:DOWN_FAILED
D:COMPAT:BREAKING_UNDECLARED   D:COMPAT:ATTR_CONFLICT
D:COMPAT:VERB_CONFLICT         D:COMPAT:DEPENDENCY_UNPUBLISHED
D:COMPAT:DEPENDENCY_FAILED
D:POLICY:APPROVAL_REQUIRED     D:POLICY:ROLE_INSUFFICIENT

PUBLISH:DRIFT_DETECTED         PUBLISH:LOCK_CONTENTION
PUBLISH:STATUS_INVALID         PUBLISH:BATCH_CYCLE_DETECTED
```

---

## Appendix E: Metric names quick reference

```
semreg.changeset.proposed_total
semreg.changeset.validated_total
semreg.changeset.dryrun_total
semreg.changeset.published_total
semreg.changeset.rollback_total
semreg.changeset.superseded_total
semreg.changeset.active_draft_count
semreg.changeset.stale_dryrun_count

semreg.validate.duration_ms
semreg.validate.errors_total
semreg.validate.warnings_total
semreg.dryrun.scratch_apply_ms
semreg.dryrun.scratch_cleanup_ms

semreg.publish.duration_ms
semreg.publish.lock_wait_ms
semreg.publish.migration_apply_ms
semreg.publish.drift_detected_total
semreg.publish.lock_contention_total
semreg.outbox.emit_total
semreg.projection.lag_ms

semreg.session.mode_switches_total
semreg.session.verb_denied_total
semreg.session.fail_closed_total
```

---

## Appendix F: Implementation notes (2026-02-26)

### Standalone server deployment

The `sem_os_server` crate is a fully standalone Axum REST server (port 4100) with JWT authentication. It is deployable independently of the `ob-poc-web` monolith.

**Verified capabilities:**
- All 10 authoring REST routes operational
- 3 health/observability routes (no auth required)
- JWT authentication enforced on all protected routes
- AgentMode gating: `publish` requires Governed mode + admin role
- Outbox dispatcher runs as background task
- Cleanup store wired for retention/archival

**Deferred:** `/tools/call` and `/tools/list` routes removed pending finalized tool schemas.

### Migrations 101-102 (standalone remediation)

| Migration | Purpose |
|-----------|---------|
| 101 | Fix `sem_reg.changesets.status` CHECK constraint to include all 9 ChangeSetStatus values (`under_review`, `approved` added from stewardship workflow) |
| 102 | Fix `change_sets_archive` and `change_set_artifacts_archive` to use `sem_reg_authoring` schema |

### Implementation deviations from spec

| Spec | Implementation | Rationale |
|------|----------------|-----------|
| 7-state ChangeSetStatus | 9-state (added UnderReview, Approved) | Stewardship workflow needs intermediate review states |
| `sem_reg_authoring` schema for change_sets | `sem_reg.changesets` table (existing) | Reused existing stewardship table with extended columns |
| `/tools/*` REST routes | Removed (TODO) | Tool schemas not yet finalized |
| `SnapshotStore` as direct trait | Port-trait isolation via `CoreServiceImpl` | All stores injected as `Arc<dyn Port>` for testability |

### CCIR — Context-Constrained Intent Resolution (2026-02-26)

**Migration 103:** Adds 5 CCIR telemetry columns to `agent.intent_events`.

**Problem:** The `SemRegVerbPolicy` enum (`AllowedSet`/`DenyAll`/`Unavailable`) was a flat discriminator that lost all resolution detail — why a verb was denied, what evidence was missing, whether the allowed set drifted between intent resolution and execution.

**Solution:** `ContextEnvelope` replaces `SemRegVerbPolicy` as the structured output of SemReg context resolution in the intent orchestrator:

| Field | Type | Purpose |
|-------|------|---------|
| `allowed_verbs` | `HashSet<String>` | Verbs passing ABAC + tier + preconditions |
| `pruned_verbs` | `Vec<PrunedVerb>` | Verbs rejected with structured `PruneReason` |
| `fingerprint` | `AllowedVerbSetFingerprint` | SHA-256 of sorted FQNs (`v1:<hex>`) — deterministic |
| `evidence_gaps` | `Vec<String>` | Missing evidence identified during resolution |
| `governance_signals` | `Vec<GovernanceSignalSummary>` | Staleness, unowned objects, etc. |
| `snapshot_set_id` | `Option<String>` | Which snapshot set was resolved against |

**PruneReason (7 variants):** `AbacDenied`, `EntityKindMismatch`, `TierExcluded`, `TaxonomyNoOverlap`, `PreconditionFailed`, `AgentModeBlocked`, `PolicyDenied`

**Key changes:**

1. **Pre-constrained verb search (Phase 3):** Allowed verbs threaded into `IntentPipeline.with_allowed_verbs()` → `HybridVerbSearcher.search()`. Disallowed verbs are filtered at the search tier, not just post-filtered. Post-filter retained as safety net.

2. **dsl_execute SemReg gate:** MCP `dsl_execute` now parses the AST, extracts verb FQNs from each `VerbCall`, and checks against the `ContextEnvelope`. Denied verbs block execution.

3. **TOCTOU recheck:** `ContextEnvelope::toctou_recheck()` compares an original resolution against a fresh one, yielding `StillAllowed` / `AllowedButDrifted` / `Denied`.

4. **Direct DSL bypass removed:** The `dsl:` prefix path and `allow_direct_dsl` flag are deleted. All DSL flows through the SemReg-filtered pipeline.

5. **Legacy V1 cleanup:** 4 modules deleted (~2,700 LOC): `session/agent_context.rs`, `session/enhanced_context.rs`, `session/verb_discovery.rs`, `api/verb_discovery_routes.rs`, `lint/agent_context_lint.rs`.

**Key files:**
- `rust/src/agent/context_envelope.rs` — `ContextEnvelope`, `PruneReason`, `AllowedVerbSetFingerprint`, `TocTouResult` (16 unit tests)
- `rust/src/agent/orchestrator.rs` — Uses `ContextEnvelope` instead of `SemRegVerbPolicy`
- `rust/src/mcp/intent_pipeline.rs` — `with_allowed_verbs()` builder
- `rust/src/mcp/verb_search.rs` — `search()` accepts `allowed_verbs` parameter
- `rust/src/mcp/handlers/core.rs` — `dsl_execute` verb validation + `verb_search` pre-constrained
- `migrations/103_ccir_intent_fingerprint.sql` — Telemetry columns

### DSL Verb Domains for SemReg (2026-02-27)

The SemReg/Stewardship MCP tools are now discoverable as first-class DSL verbs across 7 new domains (69 verbs total) plus 4 new agent introspection verbs. Verb YAML lives in `rust/config/verbs/sem-reg/` with corresponding `CustomOperation` handlers in `rust/src/domain_ops/sem_reg_*_ops.rs`.

| Domain | Verbs | YAML File | Purpose |
|--------|-------|-----------|---------|
| `registry` | 20 | `sem-reg/registry.yaml` | Object CRUD: snapshots, attributes, entity types, verb contracts, taxonomies, views, policies |
| `changeset` | 14 | `sem-reg/changeset.yaml` | Changeset authoring: propose, validate, dry-run, publish, diff, review workflow |
| `governance` | 9 | `sem-reg/governance.yaml` | Governance verbs: publish gates, impact analysis, rollback, audit log |
| `audit` | 8 | `sem-reg/audit.yaml` | Governance audit trail: decision records, intent events, bootstrap audit |
| `maintenance` | 7 | `sem-reg/maintenance.yaml` | Registry maintenance: cleanup, retention, archival, health checks |
| `focus` | 6 | `sem-reg/focus.yaml` | Stewardship focus/show loop: viewport management, manifest capture |
| `schema` | 5 | `sem-reg/schema.yaml` | Schema introspection and attribute source mapping |
| `agent` (ext) | 4 | `agent.yaml` | Mode/policy introspection, tool listing, telemetry summary |

All 73 new verbs pass tiering lint (`cargo x verbs lint` — 0 errors) and have been embedded via `populate_embeddings` (15,465 total patterns, 1,158 verbs, 100% coverage).

---
