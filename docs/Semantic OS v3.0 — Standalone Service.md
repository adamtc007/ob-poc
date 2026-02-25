# Semantic OS v3.0 — Standalone Service Boundary + Stewardship (Implementation TODO)

**Status:** Draft — for Opus peer review → then Claude Code implementation  
**Date:** Feb 2026  
**Scope:** Refactor Semantic OS v2 first (freeze semantics), then add Standalone boundary, then Stewardship Agent + Workbench.

---

## 1) Strategy (state so far)

1) **Refactor Semantic OS v2 first** (boundary + layering + tests), **before** Stewardship Agent + Workbench.  
2) **Goal:** freeze execution semantics, then add stewardship as an additive authoring/workflow layer.  
3) **v3.0 direction:** Semantic OS becomes optionally standalone:
   - **Service boundary:** all external access via API (gRPC/REST) + events; **no direct DB reads/writes** by consumers.
   - **Storage boundary:** Semantic OS owns its store (Postgres initially) and supports “enterprise DB” via snapshot export/projections.
   - **Snapshot contract:** runtime consumers pin to immutable `snapshot_id` + stable FQNs.

---

## 2) Repo reality (what exists today)

Semantic OS already exists in the repo as a `sem_reg` implementation with:

- A full semantic registry module under `rust/src/sem_reg/` including:
  - snapshot types + IDs (`types.rs`, `ids.rs`)
  - publish/store logic (`store.rs`)
  - gates (`gates.rs`, `gates_governance.rs`, `gates_technical.rs`)
  - context resolution (`context_resolution.rs`)
  - agent control plane (`rust/src/sem_reg/agent/*`)
  - projections (`rust/src/sem_reg/projections/*`)
  - onboarding/scanning (`scanner.rs`, `onboarding/*`, `seeds/*`)  [oai_citation:0‡CLAUDE.md](sediment://file_0000000049f8724681607a289a05045a)

- Tests already present:
  - `rust/tests/sem_reg_integration.rs`
  - `rust/tests/sem_reg_invariants.rs`  [oai_citation:1‡CLAUDE.md](sediment://file_0000000049f8724681607a289a05045a)

- Dedicated migrations for `sem_reg` schema (Phase 0+ tables, agent tables, projections, evidence instances) under `migrations/078_sem_reg_phase0.sql` … `090_sem_reg_evidence_instances.sql`.  [oai_citation:2‡CLAUDE.md](sediment://file_0000000049f8724681607a289a05045a)

- Recent hardening already done (atomic publish, ABAC enforcement, deterministic IDs, drift detection, pagination fixes, immutable backfill patterns).  [oai_citation:3‡CLAUDE.md](sediment://file_0000000049f8724681607a289a05045a)

**What’s missing for v3.0 (standalone):**
- A true **service boundary** (API-only access + events); consumers must not read `sem_reg.*` directly.
- A “semantic kernel” layering: core logic must be buildable without SQLx/web/MCP/ob-poc couplings.
- **Publish → outbox event** invariant and an outbox table + dispatcher.
- Snapshot export / projection contract for enterprise DB consumers.
- Stewardship primitives: changesets, drafts, review/approval, workbench APIs (additive, not semantic drift).

---

## 3) v3.0 non-negotiables

### 3.1 Freeze semantics before stewardship
- Refactors must not drift behavior.  
- Golden/invariant tests become the contract.

### 3.2 No consumer DB coupling
- No direct DB reads/writes by ob-poc into `sem_reg.*` in “standalone mode”.
- Integration is via **API + snapshot export/events**.

### 3.3 Snapshot immutability
- Active snapshots are immutable and append-only.
- “Draft” work must not mutate active snapshots in-place.

---

## 4) Delivery plan (rip/replace safe)

### Stage 0 — Confirm “separate schema” baseline is correct (fast verification)
> This is largely already in place (schema + migrations exist). The v3 task is to *enforce* the boundary.

**TODO**
- [ ] Verify `sem_reg` schema is fully owned by Semantic OS migrations only.
- [ ] Verify there are **no cross-schema foreign keys** from ob-poc schemas into `sem_reg.*` (integration must be `snapshot_id/FQN` at app layer).
- [ ] Add DB roles/privileges plan (even if not yet enforced until Stage 2):
  - `sem_os_owner` owns `sem_reg`
  - `sem_os_app` runtime role for the semantic service
  - `ob_app` cannot write (ideally cannot read) `sem_reg.*`
- [ ] Optional transitional: `sem_reg_published` read-only views if you need bridging during cutover.

**Acceptance**
- `ob_app` can run ob-poc without any direct queries to `sem_reg.*` (once Stage 2 client cutover lands).

---

### Stage 1 — Refactor tranche (Semantic kernel + seams + tests)
**Outcome:** Semantic OS becomes a layered module suite with clean ports; still runnable in-process initially, but now swappable behind an API boundary.

#### S1.1 Extract/confirm `sem_reg_core` (“semantic kernel”)
**Goal:** Core compiles with **no SQLx/web/MCP deps**.

**Move/shape responsibilities**
- Core must contain:
  - canonical types
  - snapshot model (Draft/Active, immutability rules)
  - gates + error model + remediation payloads
  - diff/impact primitives
  - context resolution algorithm (pure)
- Core must not contain:
  - SQLx row types / `PgPool`
  - MCP tool surface (`rust/src/mcp/*`)
  - ob-poc config scanning dependencies (verb YAML scanning belongs in an adapter)

**File-path anchored tasks (starting point)**
- [ ] Split `rust/src/sem_reg/types.rs` into:
  - `rust/src/sem_reg_core/types.rs` (pure types; remove sqlx-specific derives)
  - `rust/src/sem_reg_postgres/sqlx_types.rs` (if needed: sqlx types + row mappings)
- [ ] Move pure logic to core:
  - `rust/src/sem_reg/gates*.rs` → `rust/src/sem_reg_core/gates/*.rs`
  - `rust/src/sem_reg/abac.rs` → `rust/src/sem_reg_core/abac.rs`
  - `rust/src/sem_reg/security.rs` → `rust/src/sem_reg_core/security.rs`
  - `rust/src/sem_reg/context_resolution.rs` → `rust/src/sem_reg_core/context_resolution.rs`
- [ ] Ensure `evaluate_publish_gates()` stays pure and testable (no DB calls). (`rust/src/sem_reg/gates.rs`)  [oai_citation:4‡CLAUDE.md](sediment://file_0000000049f8724681607a289a05045a)

**Acceptance**
- Core builds without SQLx/web/MCP imports.
- Existing integration points still compile via adapters (next steps).

#### S1.2 Introduce explicit storage ports (traits) and isolate Postgres adapter
**Goal:** Core depends on ports; Postgres implements ports.

**Ports to define (minimum)**
- `SnapshotStore` (resolve/publish/list as-of)
- `ObjectStore` (load typed objects by snapshot)
- `ChangesetStore` (stub now; real Stage 3)
- `AuditStore` (append-only)
- `OutboxStore` (stub now; real Stage 2)
- `EvidenceInstanceStore` (observations/document instances/provenance if needed)

**File-path anchored tasks**
- [ ] Convert `rust/src/sem_reg/store.rs` into `rust/src/sem_reg_postgres/store.rs` implementing the ports.
- [ ] Refactor `rust/src/sem_reg/registry.rs` into core service logic that takes ports.
- [ ] Keep “atomic publish” invariant intact when refactoring store boundaries.  [oai_citation:5‡CLAUDE.md](sediment://file_0000000049f8724681607a289a05045a)

**Acceptance**
- Existing tests (`rust/tests/sem_reg_integration.rs`, `rust/tests/sem_reg_invariants.rs`) still pass unchanged.

#### S1.3 Move scanning/onboarding into an explicit “ob-poc adapter”
**Goal:** Remove ob-poc coupling from core.

**Current files**
- `rust/src/sem_reg/scanner.rs` (verb YAML bootstrap + drift detection)
- `rust/src/sem_reg/onboarding/*`
- `rust/src/sem_reg/seeds/*`  [oai_citation:6‡CLAUDE.md](sediment://file_0000000049f8724681607a289a05045a)

**TODO**
- [ ] Define pure DTO seeds in core:
  - `VerbContractSeed`, `AttributeSeed`, `EntityTypeSeed`, `TaxonomySeed`, `PolicySeed`, `ViewSeed`, etc.
- [ ] Move:
  - `scanner.rs` → `rust/src/sem_reg_obpoc_adapter/scanner.rs`
  - `onboarding/*` → `rust/src/sem_reg_obpoc_adapter/onboarding/*`
  - `seeds/*` remain as pure DTO builders in core OR move into adapter if they read ob-poc config directly.
- [ ] Adapter outputs seeds → core publishes snapshots via ports.

**Acceptance**
- Core has no dependency on ob-poc YAML config structures.
- Running the bootstrap scan still populates the registry (via adapter path).

#### S1.4 Lock behavior with golden/invariant tests (refactor safety net)
**Goal:** prevent semantic drift while rearranging boundaries.

**TODO**
- [ ] Expand/strengthen golden tests for:
  - gate suite outcomes (governance + technical)
  - publish invariants (append-only; successor chain)
  - context resolution determinism (same inputs → same outputs/candidate ordering)
- [ ] Add at least one “point-in-time” scenario per major registry type, aligned with v2 spec.  [oai_citation:7‡semantic-os-implementation-todo-v2.md](sediment://file_00000000a8d07243a7ed4d414ec35964)

**Acceptance**
- Refactors can proceed without behavior drift.
- Tests become the semantic contract for later standalone work.

---

### Stage 2 — Standalone Semantic OS boundary (API + outbox events)
**Outcome:** Semantic OS runs as its own process; ob-poc consumes via a client; DB access is owned by Semantic OS.

#### S2.1 Add Semantic OS server (REST or gRPC-first)
**TODO**
- [ ] Create a new server surface (recommended naming):
  - `rust/src/semantic_os_server/*` (or a workspace crate if the repo is multi-crate)
- [ ] Minimal endpoints first:
  - `GET /health`
  - `POST /resolve_context`
  - `GET /snapshots/{snapshot_id}` (typed read or manifest)
  - `POST /publish` (admin-only)
  - `GET /snapshot_sets/{id}/manifest`
  - `POST /exports/snapshot_set` (export contract for consumers)

**Acceptance**
- Server can:
  - publish a minimal snapshot set
  - resolve context deterministically
  - export a snapshot manifest keyed by immutable IDs

#### S2.2 Publish → Outbox event invariant (first-class)
**TODO**
- [ ] Add migration: `migrations/091_sem_reg_outbox.sql`:
  - `sem_reg.outbox_events` table (append-only)
    - `event_id`, `event_type`, `snapshot_set_id`, `payload jsonb`, `created_at`, `processed_at`, `attempt_count`, `last_error`
- [ ] Modify publish path in Postgres adapter so that publish transaction also inserts outbox event(s).
- [ ] Add a simple dispatcher (poll + mark processed) runnable by the semantic service.

**Acceptance**
- Every publish produces an outbox event (no silent publish).
- Consumers can poll and checkpoint.

#### S2.3 Enforce DB boundary with roles/privileges
**TODO**
- [ ] Create SQL scripts:
  - `sql/sem_os_roles.sql`
  - `sql/sem_os_privileges.sql`
- [ ] Enforce:
  - `ob_app` cannot read/write `sem_reg.*` (or at minimum cannot write)
  - only the semantic service role can access the schema

**Acceptance**
- Attempted `SELECT` from `sem_reg.*` using `ob_app` fails (once enforcement is enabled).

#### S2.4 Replace in-process calls with a semantic-os client in ob-poc
**TODO**
- [ ] Create a `semantic-os-client` interface (trait) used by ob-poc:
  - `resolve_context()`
  - `get_manifest()`
  - `export_snapshot_set()`
- [ ] Feature flag / env switch:
  - `SEM_OS_MODE=inprocess|remote`
- [ ] Ensure ob-poc no longer links to sem_reg DB directly in `remote` mode.

**Acceptance**
- ob-poc runs end-to-end in `remote` mode.
- Candidate selection + gating semantics preserved (within agreed ordering tolerance).

---

### Stage 3 — Stewardship Agent + Workbench (additive authoring/workflow layer)
**Outcome:** Governance workflow exists without altering runtime semantics.

#### S3.1 Changesets + Draft snapshots + Review/Approval
**Key constraint:** no mutation of active snapshots.

**TODO**
- [ ] Add schema:
  - `sem_reg.changesets`
  - `sem_reg.changeset_entries`
  - `sem_reg.changeset_reviews` (approvals, actors, timestamps, verdicts)
- [ ] Draft snapshots are created as normal snapshots with `status=draft`.
- [ ] On publish:
  - create successor active snapshots (insert-only)
  - link published snapshots back to source draft/changeset

**Acceptance**
- Governed changesets cannot publish without approval.
- Operational changesets are still audited and traceable.

#### S3.2 Workbench APIs (capability first; UI later)
**TODO**
- [ ] Endpoints:
  - list changesets (status/owner/scope)
  - diff (draft vs active)
  - impact analysis
  - gate preview (run gates on draft set)
  - publish changeset

**Acceptance**
- Diff + impact are deterministic and snapshot-pinned.

#### S3.3 Stewardship agent guardrails (safe policy builder primitives)
**TODO**
- [ ] Role constraints, templates, classification/labels helpers
- [ ] Proof-chain compatibility checks
- [ ] Conflict handling (stale draft detection, merge strategy)

**Acceptance**
- Agent cannot bypass publish boundary.
- Publish remains the single trust boundary.

---

### Stage 4 — Cutover / rollback (make rip-and-replace safe)
**Outcome:** You can switch modes without fear.

**TODO**
- [ ] Compatibility harness runs same scenarios against:
  - `SEM_OS_MODE=inprocess`
  - `SEM_OS_MODE=remote`
- [ ] Compare:
  - gate outcomes identical
  - manifest stability
  - resolve_context determinism (same inputs → same outputs, within ordering tolerance)
- [ ] Rollback plan:
  - env var switch only (no DB changes required)

**Acceptance**
- One-command rollback to in-process mode.

---

## 5) Claude Code execution order (minimize blast radius)
1) Stage 1.1 core extraction  
2) Stage 1.2 ports + Postgres adapter  
3) Stage 1.3 move scanner/onboarding into adapter  
4) Stage 1.4 golden/invariant tests lock semantics  
5) Stage 2.1 server + API  
6) Stage 2.2 outbox events + publish invariant  
7) Stage 2.3 roles/privileges enforce DB boundary  
8) Stage 2.4 ob-poc client cutover (`remote` mode)  
9) Stage 3 stewardship changesets + workbench APIs  

---

## 6) What Opus should peer review (checklist)
Please review and challenge:

1) **Layering correctness**: core vs adapters vs server vs client  
2) **Immutability correctness**: draft/changeset model does not mutate active snapshots  
3) **Outbox contract**: event payload, idempotency, checkpointing  
4) **Enterprise DB integration**: snapshot export/projections strategy  
5) **Cutover safety**: feature flag, compatibility harness, rollback plan  
6) **Hidden coupling risks**: scanner/DSL/YAML dependencies, MCP surfaces, ABAC enforcement points

---

## 7) Appendices

### A) Primary reference docs in repo
- `docs/semantic-os-implementation-todo-v2.md` (baseline v2 spec + invariants + test scenarios).  [oai_citation:8‡semantic-os-implementation-todo-v2.md](sediment://file_00000000a8d07243a7ed4d414ec35964)  
- `CLAUDE.md` (current implementation map + recent hardening notes).   

### B) Key current file map (starting points)
- Semantic registry core module: `rust/src/sem_reg/*`  [oai_citation:9‡CLAUDE.md](sediment://file_0000000049f8724681607a289a05045a)  
- MCP tools: `rust/src/mcp/tools_sem_reg.rs`  [oai_citation:10‡CLAUDE.md](sediment://file_0000000049f8724681607a289a05045a)  
- Tests: `rust/tests/sem_reg_integration.rs`, `rust/tests/sem_reg_invariants.rs`  [oai_citation:11‡CLAUDE.md](sediment://file_0000000049f8724681607a289a05045a)  
- Migrations: `migrations/078_sem_reg_phase0.sql` … `090_sem_reg_evidence_instances.sql`  [oai_citation:12‡CLAUDE.md](sediment://file_0000000049f8724681607a289a05045a)  

---