# Stewardship Agent Architecture — Phase 0 + Phase 1 Implementation Plan

## Overview

Implement **Phase 0 (Changeset Layer)** and **Phase 1 (Show Loop)** from `docs/stewardship-agent-architecture-v1.0.1.md` §16. Phase 0 builds the stewardship data model, guardrails, and MCP tools. Phase 1 adds the FocusState, ShowPacket emission, SSE transport, and draft-aware context resolution.

## Existing Infrastructure (REUSE)

The following already exists and will be extended (not rewritten):

| Asset | Location | Status |
|-------|----------|--------|
| Migration 095 | `migrations/095_sem_reg_changesets.sql` | Basic changeset/entries/reviews tables — needs ALTER TABLE for missing columns |
| `ChangesetStore` trait | `sem_os_core/src/ports.rs` | 8 methods — extend with new stewardship methods |
| `PgChangesetStore` | `sem_os_postgres/src/store.rs` | SQL implementation — extend |
| `CoreServiceImpl` | `sem_os_core/src/service.rs` | `promote_changeset()`, `changeset_diff()`, `changeset_gate_preview()` — fix bugs |
| Stewardship guardrails | `sem_os_core/src/stewardship.rs` | 3 guardrail functions — extend to full G01-G15 |
| Gate framework | `sem_os_core/src/gates/mod.rs` | `ExtendedGateContext` — add `provisional_snapshots` field |
| `SemOsClient` | `sem_os_client/src/lib.rs` | 5 changeset methods — extend |
| 30 MCP tools | `sem_reg/agent/mcp_tools.rs` | Pattern for tool dispatch — add 20 new tools |

## Known Bugs to Fix

1. **`publish_changeset` response**: `InProcessClient` returns `snapshot_set_id: String::new()` → should be `changeset_id.to_string()`
2. **Harness NoopChangesetStore**: `sem_os_harness/src/lib.rs` uses `NoopChangesetStore` → wire `PgChangesetStore`
3. **`changeset_impact()` stub**: Returns fake data → implement real JSONB dependency traversal
4. **Draft UNIQUE constraint missing**: Add partial UNIQUE index in migration

---

## Phase 0: Changeset Layer (0–50%)

### Migration 097 (`migrations/097_stewardship_phase0.sql`)

```sql
-- 1. ALTER changeset_entries: add missing columns per spec §9.1
ALTER TABLE sem_reg.changeset_entries
  ADD COLUMN IF NOT EXISTS action VARCHAR(20) DEFAULT 'add'
    CHECK (action IN ('add','modify','promote','deprecate','alias')),
  ADD COLUMN IF NOT EXISTS predecessor_id UUID REFERENCES sem_reg.snapshots(snapshot_id),
  ADD COLUMN IF NOT EXISTS revision INT NOT NULL DEFAULT 1,
  ADD COLUMN IF NOT EXISTS reasoning TEXT,
  ADD COLUMN IF NOT EXISTS guardrail_log JSONB NOT NULL DEFAULT '[]';

-- 2. Draft uniqueness invariant (spec §9.1)
CREATE UNIQUE INDEX IF NOT EXISTS idx_draft_uniqueness
  ON sem_reg.snapshots (snapshot_set_id, object_type, object_id)
  WHERE status = 'draft' AND effective_until IS NULL;

-- 3. Stewardship event log (immutable, append-only)
CREATE TABLE IF NOT EXISTS sem_reg.stewardship_events (
  event_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  changeset_id UUID NOT NULL REFERENCES sem_reg.changesets(changeset_id),
  event_type VARCHAR(60) NOT NULL,
  actor_id VARCHAR(200) NOT NULL,
  payload JSONB NOT NULL DEFAULT '{}',
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_stewardship_events_changeset
  ON sem_reg.stewardship_events (changeset_id, created_at);

-- 4. Basis records (first-class registered entities with claims)
CREATE TABLE IF NOT EXISTS sem_reg.basis_records (
  basis_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  changeset_id UUID NOT NULL REFERENCES sem_reg.changesets(changeset_id),
  entry_id UUID REFERENCES sem_reg.changeset_entries(entry_id),
  kind VARCHAR(40) NOT NULL
    CHECK (kind IN ('regulatory_fact','market_practice','platform_convention','client_requirement','precedent')),
  title TEXT NOT NULL,
  narrative TEXT,
  created_by VARCHAR(200) NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_basis_changeset
  ON sem_reg.basis_records (changeset_id);

-- 5. Basis claims (sub-records on basis)
CREATE TABLE IF NOT EXISTS sem_reg.basis_claims (
  claim_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  basis_id UUID NOT NULL REFERENCES sem_reg.basis_records(basis_id),
  claim_type VARCHAR(40) NOT NULL
    CHECK (claim_type IN ('regulatory_fact','market_practice','platform_convention')),
  reference_uri TEXT,
  excerpt TEXT,
  confidence DOUBLE PRECISION CHECK (confidence BETWEEN 0.0 AND 1.0),
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_claims_basis
  ON sem_reg.basis_claims (basis_id);

-- 6. Conflict records
CREATE TABLE IF NOT EXISTS sem_reg.conflict_records (
  conflict_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  changeset_id UUID NOT NULL REFERENCES sem_reg.changesets(changeset_id),
  conflict_type VARCHAR(40) NOT NULL
    CHECK (conflict_type IN ('fqn_collision','semantic_overlap','breaking_change',
                             'dependency_cycle','governance_tier_mismatch')),
  description TEXT NOT NULL,
  left_snapshot_id UUID,
  right_snapshot_id UUID,
  resolution_strategy VARCHAR(40)
    CHECK (resolution_strategy IN ('merge','supersede','split','manual','defer')),
  resolved_by VARCHAR(200),
  resolved_at TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_conflicts_changeset
  ON sem_reg.conflict_records (changeset_id);

-- 7. Stewardship templates
CREATE TABLE IF NOT EXISTS sem_reg.stewardship_templates (
  template_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  name VARCHAR(200) NOT NULL UNIQUE,
  description TEXT,
  category VARCHAR(60),
  items JSONB NOT NULL DEFAULT '[]',
  guardrail_overrides JSONB NOT NULL DEFAULT '{}',
  created_by VARCHAR(200) NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- 8. Verb implementation bindings
CREATE TABLE IF NOT EXISTS sem_reg.verb_implementation_bindings (
  binding_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  verb_fqn VARCHAR(200) NOT NULL,
  handler_kind VARCHAR(40) NOT NULL
    CHECK (handler_kind IN ('rust_handler','bpmn_process','remote_http','macro_expansion')),
  handler_ref TEXT NOT NULL,
  config JSONB NOT NULL DEFAULT '{}',
  active BOOLEAN NOT NULL DEFAULT true,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_verb_binding_active
  ON sem_reg.verb_implementation_bindings (verb_fqn)
  WHERE active = true;

-- 9. Update changeset status enum to match spec naming
-- (existing CHECK uses 'in_review', spec uses 'under_review' — add both)
ALTER TABLE sem_reg.changesets
  DROP CONSTRAINT IF EXISTS changesets_status_check;
ALTER TABLE sem_reg.changesets
  ADD CONSTRAINT changesets_status_check
    CHECK (status IN ('draft','in_review','under_review','approved','published','rejected'));
```

### New Rust Module: `rust/src/sem_reg/stewardship/`

```
rust/src/sem_reg/stewardship/
├── mod.rs          # Module root, re-exports
├── types.rs        # All stewardship types (ChangesetAction, BasisKind, etc.)
├── store.rs        # StewardshipStore — DB operations for new tables
├── guardrails.rs   # Full G01-G15 guardrail engine
├── templates.rs    # Template CRUD + instantiation
├── impact.rs       # Real changeset impact analysis (replace stub)
└── tools.rs        # 15 Phase 0 MCP tool specs + dispatch
```

### Type Definitions (`types.rs`)

```rust
// Changeset action (spec §9.1)
pub enum ChangesetAction { Add, Modify, Promote, Deprecate, Alias }

// Basis entity (spec §9.3)
pub enum BasisKind {
    RegulatoryFact, MarketPractice, PlatformConvention,
    ClientRequirement, Precedent,
}

pub struct BasisRecord {
    pub basis_id: Uuid,
    pub changeset_id: Uuid,
    pub entry_id: Option<Uuid>,
    pub kind: BasisKind,
    pub title: String,
    pub narrative: Option<String>,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
}

pub enum ClaimType { RegulatoryFact, MarketPractice, PlatformConvention }

pub struct BasisClaim {
    pub claim_id: Uuid,
    pub basis_id: Uuid,
    pub claim_type: ClaimType,
    pub reference_uri: Option<String>,
    pub excerpt: Option<String>,
    pub confidence: Option<f64>,
}

// Conflict model (spec §9.6)
pub enum ConflictType {
    FqnCollision, SemanticOverlap, BreakingChange,
    DependencyCycle, GovernanceTierMismatch,
}
pub enum ConflictResolution { Merge, Supersede, Split, Manual, Defer }

pub struct ConflictRecord {
    pub conflict_id: Uuid,
    pub changeset_id: Uuid,
    pub conflict_type: ConflictType,
    pub description: String,
    pub left_snapshot_id: Option<Uuid>,
    pub right_snapshot_id: Option<Uuid>,
    pub resolution_strategy: Option<ConflictResolution>,
    pub resolved_by: Option<String>,
    pub resolved_at: Option<DateTime<Utc>>,
}

// Stewardship event log (spec §9.4)
pub enum StewardshipEventType {
    ChangesetCreated, ItemAdded, ItemRefined, ItemRemoved,
    BasisAttached, BasisDetached, GuardrailFired,
    ReviewRequested, ReviewCompleted, ChangesetPublished,
    ChangesetRejected, ConflictDetected, ConflictResolved,
    TemplateInstantiated,
}

pub struct StewardshipEvent {
    pub event_id: Uuid,
    pub changeset_id: Uuid,
    pub event_type: StewardshipEventType,
    pub actor_id: String,
    pub payload: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

// Template (spec §9.5)
pub struct StewardshipTemplate {
    pub template_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub items: Vec<TemplateItem>,
    pub guardrail_overrides: serde_json::Value,
    pub created_by: String,
}

pub struct TemplateItem {
    pub object_type: ObjectType,
    pub fqn_pattern: String,
    pub action: ChangesetAction,
    pub default_payload: Option<serde_json::Value>,
}

// Verb implementation binding (spec §9.7)
pub enum HandlerKind { RustHandler, BpmnProcess, RemoteHttp, MacroExpansion }

pub struct VerbImplementationBinding {
    pub binding_id: Uuid,
    pub verb_fqn: String,
    pub handler_kind: HandlerKind,
    pub handler_ref: String,
    pub config: serde_json::Value,
    pub active: bool,
}

// Guardrail identifiers (spec §8, G01-G15)
pub enum GuardrailId {
    G01ProofRuleViolation,    // Block
    G02MissingBasis,          // Warning
    G03BreakingChangeNoReview,// Block
    G04DraftStaleness,        // Block (>14 days)
    G05FqnCollision,          // Block
    G06SecurityLabelGap,      // Warning
    G07DependencyCycle,       // Block
    G08TierMismatch,          // Block
    G09LargeChangeset,        // Advisory (>25 items)
    G10OrphanedDraft,         // Warning
    G11MissingDescription,    // Warning
    G12IncompleteEvidence,    // Warning
    G13StaleEvidence,         // Warning
    G14RedundantAlias,        // Advisory
    G15UnresolvedConflict,    // Block
}

pub enum GuardrailSeverity { Block, Warning, Advisory }

pub struct GuardrailResult {
    pub guardrail_id: GuardrailId,
    pub severity: GuardrailSeverity,
    pub message: String,
    pub context: serde_json::Value,
}
```

### Store Methods (`store.rs`)

```rust
pub struct StewardshipStore;

impl StewardshipStore {
    // Events
    pub async fn append_event(pool: &PgPool, event: &StewardshipEvent) -> Result<()>;
    pub async fn list_events(pool: &PgPool, changeset_id: Uuid, limit: i64) -> Result<Vec<StewardshipEvent>>;

    // Basis
    pub async fn insert_basis(pool: &PgPool, basis: &BasisRecord) -> Result<()>;
    pub async fn list_basis(pool: &PgPool, changeset_id: Uuid) -> Result<Vec<BasisRecord>>;
    pub async fn insert_claim(pool: &PgPool, claim: &BasisClaim) -> Result<()>;
    pub async fn list_claims(pool: &PgPool, basis_id: Uuid) -> Result<Vec<BasisClaim>>;

    // Conflicts
    pub async fn insert_conflict(pool: &PgPool, conflict: &ConflictRecord) -> Result<()>;
    pub async fn list_conflicts(pool: &PgPool, changeset_id: Uuid) -> Result<Vec<ConflictRecord>>;
    pub async fn resolve_conflict(pool: &PgPool, conflict_id: Uuid, strategy: ConflictResolution, actor: &str) -> Result<()>;

    // Templates
    pub async fn insert_template(pool: &PgPool, template: &StewardshipTemplate) -> Result<()>;
    pub async fn get_template(pool: &PgPool, template_id: Uuid) -> Result<Option<StewardshipTemplate>>;
    pub async fn list_templates(pool: &PgPool) -> Result<Vec<StewardshipTemplate>>;

    // Verb bindings
    pub async fn insert_binding(pool: &PgPool, binding: &VerbImplementationBinding) -> Result<()>;
    pub async fn get_binding(pool: &PgPool, verb_fqn: &str) -> Result<Option<VerbImplementationBinding>>;
    pub async fn list_bindings(pool: &PgPool) -> Result<Vec<VerbImplementationBinding>>;
}
```

### Guardrails Engine (`guardrails.rs`)

Full G01-G15 implementation. Each guardrail is a pure function taking changeset context and returning `Option<GuardrailResult>`.

```rust
pub fn evaluate_all_guardrails(
    changeset: &ChangesetRow,
    entries: &[ChangesetEntryRow],
    conflicts: &[ConflictRecord],
    basis_records: &[BasisRecord],
    active_snapshots: &[SnapshotMeta],
) -> Vec<GuardrailResult>;

pub fn has_blocking_guardrails(results: &[GuardrailResult]) -> bool;
```

Extends existing `sem_os_core/src/stewardship.rs` guardrails (reuse `validate_role_constraints`, `check_proof_chain_compatibility`, `detect_stale_drafts`).

### 15 Phase 0 MCP Tools (`tools.rs`)

| Tool | Category | Description |
|------|----------|-------------|
| `stew_create_changeset` | Stewardship | Create new changeset with scope and owner |
| `stew_add_item` | Stewardship | Add draft item (object type + FQN + payload + action) |
| `stew_refine_item` | Stewardship | Modify existing draft item (bumps revision) |
| `stew_remove_item` | Stewardship | Remove draft item from changeset |
| `stew_attach_basis` | Stewardship | Attach basis record with claims to entry |
| `stew_detach_basis` | Stewardship | Remove basis from entry |
| `stew_gate_preview` | Stewardship | Run all guardrails + publish gates without committing |
| `stew_submit_review` | Stewardship | Transition changeset to under_review |
| `stew_approve_changeset` | Stewardship | Approve changeset (requires reviewer role) |
| `stew_publish_changeset` | Stewardship | Promote all drafts to active (idempotent via client_request_id) |
| `stew_reject_changeset` | Stewardship | Reject changeset with reason |
| `stew_list_conflicts` | Stewardship | List detected conflicts for changeset |
| `stew_resolve_conflict` | Stewardship | Apply resolution strategy to conflict |
| `stew_changeset_diff` | Stewardship | Show diff between draft and active versions |
| `stew_changeset_impact` | Stewardship | Analyze downstream impact of changeset items |

All mutating tools accept `client_request_id: Option<String>` for idempotency. Each tool emits a `StewardshipEvent` to the append-only log.

### Integration Points (Phase 0)

1. **`rust/src/sem_reg/mod.rs`**: Add `pub mod stewardship;` after existing phase modules
2. **`rust/src/sem_reg/agent/mcp_tools.rs`**: Register 15 new tools in `all_tool_specs()` + `dispatch_tool()`
3. **`rust/src/mcp/tools_sem_reg.rs`**: Bridge new tools to MCP server surface
4. **`sem_os_core/src/gates/mod.rs`**: Add `provisional_snapshots: Vec<SnapshotMeta>` to `ExtendedGateContext` for intra-changeset resolution
5. **Fix `InProcessClient::publish_changeset`**: Return `changeset_id.to_string()` as `snapshot_set_id`

### Phase 0 Bug Fixes

1. **`publish_changeset` snapshot_set_id**: In `sem_os_client/src/inprocess.rs`, change `snapshot_set_id: String::new()` to `snapshot_set_id: changeset_id.to_string()`
2. **Harness wiring**: In `sem_os_harness/src/lib.rs`, replace `NoopChangesetStore` with `PgChangesetStore::new(pool.clone())`
3. **`changeset_impact()` stub**: Replace with real implementation in `stewardship/impact.rs` that traverses JSONB definition dependencies

### Phase 0 Test Scenarios

| # | Scenario | Verifies |
|---|----------|----------|
| T-S0-1 | Create changeset, add 3 items, gate preview, publish | Happy path end-to-end |
| T-S0-2 | Add Proof-class item to Operational tier → G01 blocks | Proof rule guardrail |
| T-S0-3 | Add item with duplicate FQN → G05 blocks | FQN collision |
| T-S0-4 | Create changeset, wait 15 days → G04 blocks | Draft staleness |
| T-S0-5 | Attach basis with regulatory claim, verify in gate preview | Basis flow |
| T-S0-6 | Detect conflict, attempt publish → G15 blocks, resolve, re-publish | Conflict lifecycle |
| T-S0-7 | Instantiate template → changeset pre-populated | Template instantiation |
| T-S0-8 | Concurrent publish with same client_request_id → idempotent | Idempotency |

---

## Phase 1: Show Loop (50–100%)

### New Types (extend `stewardship/types.rs`)

```rust
// FocusState — server-side shared truth (spec §9.14)
pub struct FocusState {
    pub session_id: Uuid,
    pub changeset_id: Option<Uuid>,
    pub selected_entry_id: Option<Uuid>,
    pub selected_object_fqn: Option<String>,
    pub overlay_mode: OverlayMode,
    pub viewport_filter: ViewportFilter,
    pub updated_at: DateTime<Utc>,
}

pub enum OverlayMode {
    ActiveOnly,
    DraftOverlay { changeset_id: Uuid },
}

pub struct ViewportFilter {
    pub object_types: Option<Vec<ObjectType>>,
    pub governance_tiers: Option<Vec<GovernanceTier>>,
    pub domains: Option<Vec<String>>,
    pub search_text: Option<String>,
}

// ShowPacket — emitted on every meaningful state change (spec §9.14)
pub struct ShowPacket {
    pub packet_id: Uuid,
    pub session_id: Uuid,
    pub sequence: u64,
    pub focus: FocusState,
    pub viewports: Vec<ViewportPayload>,
    pub deltas: Vec<DeltaEntry>,
    pub narrative: Option<String>,
    pub next_actions: Vec<SuggestedAction>,
    pub emitted_at: DateTime<Utc>,
}

pub struct ViewportPayload {
    pub viewport_id: ViewportId,
    pub data: serde_json::Value,
}

// Phase 1 delivers 4 of 8 viewports
pub enum ViewportId {
    Focus,          // A: Current changeset + selected entry summary
    ObjectInspector,// C: Full snapshot detail for selected object
    Diff,           // D: Side-by-side draft vs active
    Gates,          // G: Guardrail results + publish gate status
    // Phase 2+: Lineage (B), Timeline (E), Impact (F), Coverage (H)
}

pub struct DeltaEntry {
    pub path: String,
    pub old_value: Option<serde_json::Value>,
    pub new_value: Option<serde_json::Value>,
}

pub struct SuggestedAction {
    pub tool_name: String,
    pub label: String,
    pub params: serde_json::Value,
}

// WorkbenchPacket — transport envelope (spec §9.16)
pub struct WorkbenchPacket {
    pub frame_type: FrameType,
    pub payload: serde_json::Value,
    pub sequence: u64,
    pub session_id: Uuid,
    pub emitted_at: DateTime<Utc>,
}

pub enum FrameType {
    Show,      // ShowPacket
    Decision,  // DecisionPacket (existing)
}

// ViewportManifest — audit record (spec §9.4)
pub struct ViewportManifest {
    pub manifest_id: Uuid,
    pub session_id: Uuid,
    pub changeset_id: Option<Uuid>,
    pub viewport_hashes: HashMap<String, String>, // viewport_id → SHA-256 of RFC 8785 canonical JSON
    pub created_at: DateTime<Utc>,
}
```

### New Module: `rust/src/sem_reg/stewardship/show_loop.rs`

```rust
// ShowLoop engine — computes ShowPacket from FocusState
pub struct ShowLoop;

impl ShowLoop {
    // Main entry: compute full ShowPacket from current focus
    pub async fn compute_show_packet(
        pool: &PgPool,
        focus: &FocusState,
        actor: &ActorContext,
    ) -> Result<ShowPacket>;

    // Viewport A: Focus summary
    async fn render_focus_viewport(pool: &PgPool, focus: &FocusState) -> Result<ViewportPayload>;

    // Viewport C: Object inspector (full snapshot detail)
    async fn render_object_inspector(pool: &PgPool, focus: &FocusState) -> Result<ViewportPayload>;

    // Viewport D: Diff (draft vs active)
    async fn render_diff_viewport(pool: &PgPool, focus: &FocusState) -> Result<ViewportPayload>;

    // Viewport G: Gates (guardrails + publish gates)
    async fn render_gates_viewport(
        pool: &PgPool,
        focus: &FocusState,
        actor: &ActorContext,
    ) -> Result<ViewportPayload>;

    // Compute ViewportManifest (SHA-256 hashes for audit)
    pub fn compute_manifest(
        session_id: Uuid,
        changeset_id: Option<Uuid>,
        viewports: &[ViewportPayload],
    ) -> ViewportManifest;
}
```

### FocusState Storage (in-memory + DB persistence)

```rust
// In-memory focus state per session (for fast reads)
// Persisted to DB on mutation for durability
pub struct FocusStore;

impl FocusStore {
    pub async fn get(pool: &PgPool, session_id: Uuid) -> Result<Option<FocusState>>;
    pub async fn set(pool: &PgPool, focus: &FocusState) -> Result<()>;
    pub async fn delete(pool: &PgPool, session_id: Uuid) -> Result<()>;
}
```

Add to migration 097:
```sql
-- 10. Focus state (session-scoped, mutable)
CREATE TABLE IF NOT EXISTS sem_reg.focus_states (
  session_id UUID PRIMARY KEY,
  changeset_id UUID REFERENCES sem_reg.changesets(changeset_id),
  selected_entry_id UUID,
  selected_object_fqn VARCHAR(300),
  overlay_mode VARCHAR(20) NOT NULL DEFAULT 'active_only'
    CHECK (overlay_mode IN ('active_only','draft_overlay')),
  viewport_filter JSONB NOT NULL DEFAULT '{}',
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- 11. Viewport manifests (immutable audit records)
CREATE TABLE IF NOT EXISTS sem_reg.viewport_manifests (
  manifest_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  session_id UUID NOT NULL,
  changeset_id UUID,
  viewport_hashes JSONB NOT NULL DEFAULT '{}',
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_viewport_manifests_session
  ON sem_reg.viewport_manifests (session_id, created_at DESC);
```

### SSE Endpoint

New endpoint in `rust/src/api/stewardship_routes.rs`:

```
GET /api/session/:id/workbench-events
Accept: text/event-stream

→ SSE stream of WorkbenchPacket events
```

Implementation uses `tokio::sync::broadcast` channel per session. ShowPacket emission triggers broadcast to all connected SSE clients.

### Draft-Aware Context Resolution

Extend `resolve_context()` in `rust/src/sem_reg/context_resolution.rs` to support `OverlayMode::DraftOverlay`:

- When `overlay_mode = DraftOverlay { changeset_id }`, extend snapshot resolution queries:
  ```sql
  WHERE (status = 'active' AND effective_until IS NULL)
     OR (snapshot_set_id = $changeset_id AND status = 'draft' AND effective_until IS NULL)
  ```
- Draft snapshots override active snapshots for same `(object_type, object_id)`
- Gate pre-check uses same overlay for intra-changeset validation

### 5 Phase 1 MCP Tools

| Tool | Category | Description |
|------|----------|-------------|
| `stew_set_focus` | Visualisation | Set FocusState (changeset, entry, overlay mode) |
| `stew_get_focus` | Visualisation | Get current FocusState |
| `stew_show` | Visualisation | Trigger ShowPacket computation and emission |
| `stew_navigate` | Visualisation | Navigate to specific object within changeset |
| `stew_viewport_manifest` | Visualisation | Capture viewport hashes for audit |

### Integration Points (Phase 1)

1. **`rust/src/api/stewardship_routes.rs`** (NEW): SSE endpoint + REST routes for focus/show
2. **`rust/crates/ob-poc-web/src/main.rs`**: Mount stewardship routes
3. **`rust/src/sem_reg/context_resolution.rs`**: Add `overlay_mode` parameter to `resolve_context()`
4. **`rust/src/sem_reg/stewardship/tools.rs`**: Add 5 visualisation tools

### Phase 1 Test Scenarios

| # | Scenario | Verifies |
|---|----------|----------|
| T-S1-1 | Set focus to changeset → compute ShowPacket → 4 viewports present | Happy path |
| T-S1-2 | Add item to changeset → ShowPacket diff viewport shows delta | Diff rendering |
| T-S1-3 | DraftOverlay context resolution → draft overrides active | Overlay mode |
| T-S1-4 | Gate preview in overlay mode → intra-changeset validation | Draft-aware gates |
| T-S1-5 | Capture viewport manifest → SHA-256 hashes match content | Audit trail |
| T-S1-6 | SSE endpoint delivers WorkbenchPacket on state change | Transport |

---

## File Summary

### New Files

| File | Phase | Purpose |
|------|-------|---------|
| `migrations/097_stewardship_phase0.sql` | 0 | Migration: ALTER + 8 new tables + 2 indexes |
| `rust/src/sem_reg/stewardship/mod.rs` | 0 | Module root |
| `rust/src/sem_reg/stewardship/types.rs` | 0+1 | All type definitions |
| `rust/src/sem_reg/stewardship/store.rs` | 0 | DB operations |
| `rust/src/sem_reg/stewardship/guardrails.rs` | 0 | G01-G15 engine |
| `rust/src/sem_reg/stewardship/templates.rs` | 0 | Template CRUD |
| `rust/src/sem_reg/stewardship/impact.rs` | 0 | Impact analysis |
| `rust/src/sem_reg/stewardship/tools.rs` | 0+1 | 20 MCP tools |
| `rust/src/sem_reg/stewardship/show_loop.rs` | 1 | ShowPacket engine |
| `rust/src/api/stewardship_routes.rs` | 1 | SSE + REST routes |

### Modified Files

| File | Phase | Change |
|------|-------|--------|
| `rust/src/sem_reg/mod.rs` | 0 | Add `pub mod stewardship;` |
| `rust/src/sem_reg/agent/mcp_tools.rs` | 0+1 | Register 20 new tools |
| `rust/src/mcp/tools_sem_reg.rs` | 0+1 | Bridge new tools |
| `rust/src/sem_reg/context_resolution.rs` | 1 | Add overlay_mode |
| `rust/crates/ob-poc-web/src/main.rs` | 1 | Mount stewardship routes |
| `rust/crates/sem_os_client/src/inprocess.rs` | 0 | Fix publish_changeset bug |
| `rust/crates/sem_os_harness/src/lib.rs` | 0 | Wire PgChangesetStore |
| `rust/crates/sem_os_core/src/gates/mod.rs` | 0 | Add provisional_snapshots |

### Test Files

| File | Phase | Scenarios |
|------|-------|-----------|
| `rust/tests/stewardship_phase0_test.rs` | 0 | T-S0-1 through T-S0-8 |
| `rust/tests/stewardship_phase1_test.rs` | 1 | T-S1-1 through T-S1-6 |

---

## Implementation Order

1. Migration 097 (all tables at once)
2. `stewardship/types.rs` (all type definitions)
3. `stewardship/store.rs` (DB operations)
4. `stewardship/guardrails.rs` (G01-G15)
5. `stewardship/templates.rs` (template CRUD)
6. `stewardship/impact.rs` (impact analysis)
7. `stewardship/tools.rs` (15 Phase 0 MCP tools)
8. `stewardship/mod.rs` + wire into `sem_reg/mod.rs`
9. Wire tools into `mcp_tools.rs` + `tools_sem_reg.rs`
10. Fix bugs (publish_changeset, harness, changeset_impact)
11. Phase 0 tests
12. `stewardship/show_loop.rs` (ShowPacket engine)
13. Add FocusState/ShowPacket/WorkbenchPacket types
14. Add 5 Phase 1 MCP tools to `tools.rs`
15. `api/stewardship_routes.rs` (SSE endpoint)
16. Draft-aware context resolution
17. Mount routes in main.rs
18. Phase 1 tests
