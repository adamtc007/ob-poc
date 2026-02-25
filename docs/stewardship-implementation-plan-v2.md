# Stewardship Agent — Phase 0 + Phase 1 Implementation Plan (v2)

**Source spec:** `docs/stewardship-agent-architecture-v1.0.1.md`
**Scope:** Phase 0 (Changeset Layer) + Phase 1 (Show Loop) per §16
**Constraint:** No kernel DB schema deltas; stewardship-layer tables only

---

## Existing Infrastructure (REUSE)

The following already exists and will be extended (not rewritten):

| Asset | Location | Status |
|-------|----------|--------|
| Migration 095 | `migrations/095_sem_reg_changesets.sql` | Basic changeset/entries/reviews tables — needs ALTER for missing columns |
| `ChangesetStore` trait | `sem_os_core/src/ports.rs` | 8 methods — extend with new stewardship methods |
| `PgChangesetStore` | `sem_os_postgres/src/store.rs` | SQL implementation — extend |
| `CoreServiceImpl` | `sem_os_core/src/service.rs` | `promote_changeset()`, `changeset_diff()`, `changeset_gate_preview()` — fix bugs |
| Stewardship guardrails | `sem_os_core/src/stewardship.rs` | 3 guardrail functions — extend to full G01–G15 per §8 |
| Gate framework | `sem_os_core/src/gates/mod.rs` | `ExtendedGateContext` — add `overlay_mode` field |
| `SemOsClient` | `sem_os_client/src/lib.rs` | 5 changeset methods — extend |
| 30 MCP tools | `sem_reg/agent/mcp_tools.rs` | Pattern for tool dispatch — add new tools |

## Known Bugs to Fix

1. **`publish_changeset` response**: `InProcessClient` returns `snapshot_set_id: String::new()` → should be `changeset_id.to_string()` (changeset_id == snapshot_set_id per §9.1)
2. **Harness NoopChangesetStore**: `sem_os_harness/src/lib.rs` uses `NoopChangesetStore` → wire `PgChangesetStore`
3. **`changeset_impact()` stub**: Returns fake data → implement real JSONB dependency traversal
4. **Draft UNIQUE constraint missing**: Add partial UNIQUE index in migration

---

## Phase 0: Changeset Layer (0–50%)

### Migration 097 (`migrations/097_stewardship_phase0.sql`)

```sql
-- ============================================================
-- Phase 0: Stewardship tables
-- Constraint: no changes to sem_reg.snapshots or other kernel tables
-- (only the partial UNIQUE index on snapshots, which is additive)
-- ============================================================

-- 0. Schema: stewardship tables in their own schema for boundary visibility
CREATE SCHEMA IF NOT EXISTS stewardship;

-- 1. ALTER changeset_entries: add missing columns per spec §9.1
ALTER TABLE sem_reg.changeset_entries
  ADD COLUMN IF NOT EXISTS action VARCHAR(20) NOT NULL DEFAULT 'add'
    CHECK (action IN ('add','modify','promote','deprecate','alias')),
  ADD COLUMN IF NOT EXISTS predecessor_id UUID REFERENCES sem_reg.snapshots(snapshot_id),
  ADD COLUMN IF NOT EXISTS revision INT NOT NULL DEFAULT 1,
  ADD COLUMN IF NOT EXISTS reasoning TEXT,
  ADD COLUMN IF NOT EXISTS guardrail_log JSONB NOT NULL DEFAULT '[]';

-- 2. Draft uniqueness invariant (spec §9.1)
--    object_id (not fqn) — kernel has no fqn column; fqn is derived from object_id
CREATE UNIQUE INDEX IF NOT EXISTS idx_draft_uniqueness
  ON sem_reg.snapshots (snapshot_set_id, object_type, object_id)
  WHERE status = 'draft' AND effective_until IS NULL;

-- 3. Update changeset status to spec naming (§9.1)
--    Migrate existing 'in_review' rows then drop old value
UPDATE sem_reg.changesets SET status = 'under_review' WHERE status = 'in_review';
ALTER TABLE sem_reg.changesets
  DROP CONSTRAINT IF EXISTS changesets_status_check;
ALTER TABLE sem_reg.changesets
  ADD CONSTRAINT changesets_status_check
    CHECK (status IN ('draft','under_review','approved','published','rejected'));

-- 4. Stewardship event log — immutable, append-only (spec §9.4)
CREATE TABLE IF NOT EXISTS stewardship.events (
  event_id       UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  changeset_id   UUID NOT NULL REFERENCES sem_reg.changesets(changeset_id),
  event_type     VARCHAR(60) NOT NULL
    CHECK (event_type IN (
      'changeset_created',
      'item_added', 'item_removed', 'item_refined',
      'basis_attached',
      'guardrail_fired',
      'gate_prechecked',
      'submitted_for_review',
      'review_note_added',
      'review_decision_recorded',
      'focus_changed',
      'published',
      'rejected'
    )),
  actor_id       VARCHAR(200) NOT NULL,
  payload        JSONB NOT NULL DEFAULT '{}',
  viewport_manifest_id UUID,  -- optional FK to viewport_manifests for audit
  created_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_stew_events_changeset
  ON stewardship.events (changeset_id, created_at);

-- 5. Basis records (spec §9.3)
CREATE TABLE IF NOT EXISTS stewardship.basis_records (
  basis_id       UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  changeset_id   UUID NOT NULL REFERENCES sem_reg.changesets(changeset_id),
  entry_id       UUID REFERENCES sem_reg.changeset_entries(entry_id),
  kind           VARCHAR(40) NOT NULL
    CHECK (kind IN ('regulatory_fact','market_practice','platform_convention',
                    'client_requirement','precedent')),
  title          TEXT NOT NULL,
  narrative      TEXT,
  created_by     VARCHAR(200) NOT NULL,
  created_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_basis_changeset
  ON stewardship.basis_records (changeset_id);

-- 6. Basis claims (spec §9.3)
CREATE TABLE IF NOT EXISTS stewardship.basis_claims (
  claim_id       UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  basis_id       UUID NOT NULL REFERENCES stewardship.basis_records(basis_id),
  claim_text     TEXT NOT NULL,
  reference_uri  TEXT,
  excerpt        TEXT,
  confidence     DOUBLE PRECISION CHECK (confidence BETWEEN 0.0 AND 1.0),
  flagged_as_open_question BOOLEAN NOT NULL DEFAULT false,
  created_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_claims_basis
  ON stewardship.basis_claims (basis_id);

-- 7. Conflict records (spec §9.6)
CREATE TABLE IF NOT EXISTS stewardship.conflict_records (
  conflict_id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  changeset_id            UUID NOT NULL REFERENCES sem_reg.changesets(changeset_id),
  competing_changeset_id  UUID NOT NULL REFERENCES sem_reg.changesets(changeset_id),
  fqn                     VARCHAR(300) NOT NULL,
  detected_at             TIMESTAMPTZ NOT NULL DEFAULT now(),
  resolution_strategy     VARCHAR(20)
    CHECK (resolution_strategy IN ('merge','rebase','supersede')),
  resolution_rationale    TEXT,
  resolved_by             VARCHAR(200),
  resolved_at             TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS idx_conflicts_changeset
  ON stewardship.conflict_records (changeset_id);

-- 8. Templates (spec §9.5) — versioned stewardship objects
CREATE TABLE IF NOT EXISTS stewardship.templates (
  template_id    UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  fqn            VARCHAR(300) NOT NULL,
  display_name   VARCHAR(200) NOT NULL,
  version_major  INT NOT NULL DEFAULT 1,
  version_minor  INT NOT NULL DEFAULT 0,
  version_patch  INT NOT NULL DEFAULT 0,
  domain         VARCHAR(100) NOT NULL,
  scope          JSONB NOT NULL DEFAULT '[]',    -- Vec<EntityType>
  items          JSONB NOT NULL DEFAULT '[]',    -- Vec<TemplateItem>
  steward        VARCHAR(200) NOT NULL,
  basis_ref      UUID,
  status         VARCHAR(20) NOT NULL DEFAULT 'draft'
    CHECK (status IN ('draft','active','deprecated')),
  created_by     VARCHAR(200) NOT NULL,
  created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);
-- Uniqueness: one active version per FQN
CREATE UNIQUE INDEX IF NOT EXISTS idx_template_fqn_active
  ON stewardship.templates (fqn)
  WHERE status = 'active';

-- 9. Verb implementation bindings (spec §9.7)
CREATE TABLE IF NOT EXISTS stewardship.verb_implementation_bindings (
  binding_id     UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  verb_fqn       VARCHAR(300) NOT NULL,
  binding_kind   VARCHAR(40) NOT NULL
    CHECK (binding_kind IN ('rust_handler','bpmn_process','remote_http','macro_expansion')),
  binding_ref    TEXT NOT NULL,
  exec_modes     JSONB NOT NULL DEFAULT '[]',    -- Vec<ExecMode>
  status         VARCHAR(20) NOT NULL DEFAULT 'draft'
    CHECK (status IN ('draft','active','deprecated')),
  last_verified_at TIMESTAMPTZ,
  notes          TEXT,
  created_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_verb_binding_active
  ON stewardship.verb_implementation_bindings (verb_fqn)
  WHERE status = 'active';

-- 10. Idempotency tracking for mutating tools (spec §6.2)
CREATE TABLE IF NOT EXISTS stewardship.idempotency_keys (
  client_request_id UUID PRIMARY KEY,
  tool_name         VARCHAR(100) NOT NULL,
  result            JSONB NOT NULL,
  created_at        TIMESTAMPTZ NOT NULL DEFAULT now()
);
-- Auto-expire old keys (optional: pg_cron or application-level)
CREATE INDEX IF NOT EXISTS idx_idempotency_created
  ON stewardship.idempotency_keys (created_at);
```

### New Rust Module: `rust/src/sem_reg/stewardship/`

```
rust/src/sem_reg/stewardship/
├── mod.rs            # Module root, re-exports
├── types.rs          # All stewardship types matching spec §8, §9.1–9.7, §9.14
├── store.rs          # StewardshipStore — DB operations for stewardship.* tables
├── guardrails.rs     # Full G01–G15 guardrail engine (spec §8.2 verbatim)
├── templates.rs      # Template CRUD + instantiation (spec §9.5)
├── impact.rs         # Real changeset impact analysis (replace stub)
├── idempotency.rs    # client_request_id dedup layer
├── tools_phase0.rs   # Phase 0 MCP tool definitions + dispatch
├── show_loop.rs      # Phase 1: ShowPacket engine
├── focus.rs          # Phase 1: FocusState store + transitions
└── tools_phase1.rs   # Phase 1 MCP tool definitions + dispatch
```

### Type Definitions (`types.rs`)

All types match the spec's data model sections exactly. Spec section references in comments.

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ─── Changeset Action (§9.1) ───

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "varchar", rename_all = "snake_case")]
pub enum ChangesetAction {
    Add,
    Modify,
    Promote,
    Deprecate,
    Alias,
}

// ─── Basis (§9.3) ───

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "varchar", rename_all = "snake_case")]
pub enum BasisKind {
    RegulatoryFact,
    MarketPractice,
    PlatformConvention,
    ClientRequirement,
    Precedent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BasisClaim {
    pub claim_id: Uuid,
    pub basis_id: Uuid,
    pub claim_text: String,
    pub reference_uri: Option<String>,
    pub excerpt: Option<String>,
    pub confidence: Option<f64>,
    pub flagged_as_open_question: bool,
}

// ─── Conflict Model (§9.6) ───

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "varchar", rename_all = "snake_case")]
pub enum ConflictStrategy {
    Merge,
    Rebase,
    Supersede,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictRecord {
    pub conflict_id: Uuid,
    pub changeset_id: Uuid,
    pub competing_changeset_id: Uuid,
    pub fqn: String,
    pub detected_at: DateTime<Utc>,
    pub resolution_strategy: Option<ConflictStrategy>,
    pub resolution_rationale: Option<String>,
    pub resolved_by: Option<String>,
    pub resolved_at: Option<DateTime<Utc>>,
}

// ─── Stewardship Events (§9.4) — matches spec enum exactly ───

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StewardshipEventType {
    ChangesetCreated,
    ItemAdded,
    ItemRemoved,
    ItemRefined,
    BasisAttached,
    GuardrailFired {
        guardrail_id: GuardrailId,
        severity: GuardrailSeverity,
        resolution: String,
    },
    GatePrechecked {
        result: serde_json::Value, // GateResult serialized
    },
    SubmittedForReview,
    ReviewNoteAdded,
    ReviewDecisionRecorded {
        disposition: ReviewDisposition,
    },
    FocusChanged {
        from: serde_json::Value, // FocusState serialized
        to: serde_json::Value,
        source: FocusUpdateSource,
    },
    Published,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StewardshipRecord {
    pub event_id: Uuid,
    pub changeset_id: Uuid,
    pub event_type: StewardshipEventType,
    pub actor_id: String,
    pub payload: serde_json::Value,
    pub viewport_manifest_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewDisposition {
    Approve,
    RequestChange,
    Reject,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FocusUpdateSource {
    Agent,
    UserNavigation,
}

// ─── Template (§9.5) — stewardship-layer object ───

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StewardshipTemplate {
    pub template_id: Uuid,
    pub fqn: String,
    pub display_name: String,
    pub version: SemanticVersion,
    pub domain: String,
    pub scope: Vec<String>,      // entity types
    pub items: Vec<TemplateItem>,
    pub steward: String,
    pub basis_ref: Option<Uuid>,
    pub status: TemplateStatus,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateItem {
    pub object_type: String,
    pub fqn_pattern: String,
    pub action: ChangesetAction,
    pub default_payload: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "varchar", rename_all = "snake_case")]
pub enum TemplateStatus {
    Draft,
    Active,
    Deprecated,
}

// ─── VerbImplementationBinding (§9.7) — stewardship-layer ───

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "varchar", rename_all = "snake_case")]
pub enum BindingKind {
    RustHandler,
    BpmnProcess,
    RemoteHttp,
    MacroExpansion,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "varchar", rename_all = "snake_case")]
pub enum BindingStatus {
    Draft,
    Active,
    Deprecated,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbImplementationBinding {
    pub binding_id: Uuid,
    pub verb_fqn: String,
    pub binding_kind: BindingKind,
    pub binding_ref: String,
    pub exec_modes: Vec<String>, // ExecMode serialized
    pub status: BindingStatus,
    pub last_verified_at: Option<DateTime<Utc>>,
    pub notes: Option<String>,
}

// ─── Guardrails (§8.2) — VERBATIM from spec ───

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum GuardrailId {
    G01RolePermission,
    G02NamingConvention,
    G03TypeConstraint,
    G04ProofChainCompatibility,
    G05ClassificationRequired,
    G06SecurityLabelRequired,
    G07SilentMeaningChange,
    G08DeprecationWithoutReplacement,
    G09AIKnowledgeBoundary,
    G10ConflictDetected,
    G11StaleTemplate,
    G12ObservationImpact,
    G13ResolutionMetadataMissing,
    G14CompositionHintStale,
    G15DraftUniquenessViolation,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum GuardrailSeverity {
    Block,
    Warning,
    Advisory,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuardrailResult {
    pub guardrail_id: GuardrailId,
    pub severity: GuardrailSeverity,
    pub message: String,
    pub remediation: String,
    pub context: serde_json::Value,
}
```

### Guardrail Severity Map (`guardrails.rs`)

Each guardrail maps to its spec-defined severity. This is the enforcement contract.

```rust
impl GuardrailId {
    pub fn default_severity(&self) -> GuardrailSeverity {
        match self {
            // Block — edit cannot be saved
            Self::G01RolePermission => GuardrailSeverity::Block,
            Self::G03TypeConstraint => GuardrailSeverity::Block,
            Self::G04ProofChainCompatibility => GuardrailSeverity::Block,
            Self::G05ClassificationRequired => GuardrailSeverity::Block,
            Self::G06SecurityLabelRequired => GuardrailSeverity::Block,
            Self::G07SilentMeaningChange => GuardrailSeverity::Block,
            Self::G08DeprecationWithoutReplacement => GuardrailSeverity::Block,
            Self::G15DraftUniquenessViolation => GuardrailSeverity::Block,
            // Warning — must be acknowledged before submit
            Self::G02NamingConvention => GuardrailSeverity::Warning,
            Self::G10ConflictDetected => GuardrailSeverity::Warning,
            Self::G11StaleTemplate => GuardrailSeverity::Warning,
            Self::G12ObservationImpact => GuardrailSeverity::Warning,
            Self::G13ResolutionMetadataMissing => GuardrailSeverity::Warning,
            // Advisory — informational only
            Self::G09AIKnowledgeBoundary => GuardrailSeverity::Advisory,
            Self::G14CompositionHintStale => GuardrailSeverity::Advisory,
        }
    }
}

/// Evaluate all applicable guardrails for the current changeset state.
/// Each guardrail is a pure function returning Option<GuardrailResult>.
pub fn evaluate_all_guardrails(
    changeset: &ChangesetRow,
    entries: &[ChangesetEntryRow],
    conflicts: &[ConflictRecord],
    basis_records: &[BasisRecord],
    active_snapshots: &[SnapshotMeta],
    templates_used: &[StewardshipTemplate],
) -> Vec<GuardrailResult> {
    let mut results = Vec::new();

    // G01: RolePermission — field-level ABAC check
    results.extend(check_role_permissions(changeset, entries));

    // G02: NamingConvention — FQN pattern matching
    results.extend(check_naming_conventions(entries));

    // G03: TypeConstraint — data type vs governance tier compatibility
    results.extend(check_type_constraints(entries, active_snapshots));

    // G04: ProofChainCompatibility — attr in policy predicate but tier < Proof
    results.extend(check_proof_chain_compatibility(entries, active_snapshots));

    // G05: ClassificationRequired — regulated domain missing taxonomy membership
    results.extend(check_classification_required(entries));

    // G06: SecurityLabelRequired — PII/tax semantics missing security label
    results.extend(check_security_label_required(entries));

    // G07: SilentMeaningChange — type change without migration note
    results.extend(check_silent_meaning_change(entries, active_snapshots));

    // G08: DeprecationWithoutReplacement
    results.extend(check_deprecation_replacement(entries));

    // G09: AIKnowledgeBoundary — low-confidence Basis claims
    results.extend(check_ai_knowledge_boundary(basis_records));

    // G10: ConflictDetected — FQN modified in another open changeset
    results.extend(check_conflicts_detected(conflicts));

    // G11: StaleTemplate — template below current version
    results.extend(check_stale_template(changeset, templates_used));

    // G12: ObservationImpact — promotion affects existing observations
    results.extend(check_observation_impact(entries, active_snapshots));

    // G13: ResolutionMetadataMissing — VerbContract missing usage examples etc.
    results.extend(check_resolution_metadata(entries));

    // G14: CompositionHintStale — VerbContract composition hints reference non-Active
    results.extend(check_composition_hints(entries, active_snapshots));

    // G15: DraftUniquenessViolation — duplicate Draft head per (object_type, object_id)
    // Note: also enforced by DB UNIQUE constraint; guardrail provides friendly message
    results.extend(check_draft_uniqueness(entries));

    results
}

pub fn has_blocking_guardrails(results: &[GuardrailResult]) -> bool {
    results.iter().any(|r| r.severity == GuardrailSeverity::Block)
}
```

### Store Methods (`store.rs`)

```rust
pub struct StewardshipStore;

impl StewardshipStore {
    // ─── Events (§9.4) ───
    pub async fn append_event(pool: &PgPool, record: &StewardshipRecord) -> Result<()>;
    pub async fn list_events(pool: &PgPool, changeset_id: Uuid, limit: i64) -> Result<Vec<StewardshipRecord>>;

    // ─── Basis (§9.3) ───
    pub async fn insert_basis(pool: &PgPool, basis: &BasisRecord) -> Result<()>;
    pub async fn list_basis(pool: &PgPool, changeset_id: Uuid) -> Result<Vec<BasisRecord>>;
    pub async fn insert_claim(pool: &PgPool, claim: &BasisClaim) -> Result<()>;
    pub async fn list_claims(pool: &PgPool, basis_id: Uuid) -> Result<Vec<BasisClaim>>;

    // ─── Conflicts (§9.6) ───
    pub async fn insert_conflict(pool: &PgPool, conflict: &ConflictRecord) -> Result<()>;
    pub async fn list_conflicts(pool: &PgPool, changeset_id: Uuid) -> Result<Vec<ConflictRecord>>;
    pub async fn resolve_conflict(
        pool: &PgPool,
        conflict_id: Uuid,
        strategy: ConflictStrategy,
        rationale: &str,
        actor: &str,
    ) -> Result<()>;

    // ─── Templates (§9.5) ───
    pub async fn insert_template(pool: &PgPool, template: &StewardshipTemplate) -> Result<()>;
    pub async fn get_template_by_fqn(pool: &PgPool, fqn: &str) -> Result<Option<StewardshipTemplate>>;
    pub async fn get_active_template(pool: &PgPool, fqn: &str) -> Result<Option<StewardshipTemplate>>;
    pub async fn list_templates(pool: &PgPool, status: Option<TemplateStatus>) -> Result<Vec<StewardshipTemplate>>;

    // ─── Verb Bindings (§9.7) ───
    pub async fn insert_binding(pool: &PgPool, binding: &VerbImplementationBinding) -> Result<()>;
    pub async fn get_active_binding(pool: &PgPool, verb_fqn: &str) -> Result<Option<VerbImplementationBinding>>;
    pub async fn list_bindings(pool: &PgPool, status: Option<BindingStatus>) -> Result<Vec<VerbImplementationBinding>>;

    // ─── Idempotency (§6.2) ───
    pub async fn check_idempotency(pool: &PgPool, client_request_id: Uuid) -> Result<Option<serde_json::Value>>;
    pub async fn record_idempotency(pool: &PgPool, client_request_id: Uuid, tool_name: &str, result: &serde_json::Value) -> Result<()>;
}
```

### Phase 0 MCP Tools (`tools_phase0.rs`)

Tool names match spec §6.1 + §6.2. All mutating tools accept `client_request_id: Option<Uuid>`.

| Tool | Spec Name (§6) | Category | Description |
|------|----------------|----------|-------------|
| `stew_compose_changeset` | ComposeChangeset | Stewardship | Create new changeset with intent + optional template |
| `stew_suggest` | Suggest | Stewardship | Agent-driven refinement of changeset items |
| `stew_add_item` | AddItem | Stewardship | Add draft item (writes Draft snapshot to sem_reg) |
| `stew_remove_item` | RemoveItem | Stewardship | Remove draft item from changeset |
| `stew_refine_item` | RefineItem | Stewardship | Modify existing draft (supersedes prior Draft, bumps revision) |
| `stew_attach_basis` | AttachBasis | Stewardship | Attach basis record with claims |
| `stew_gate_precheck` | GatePrecheck | Stewardship | Run all guardrails G01–G15 + publish gates |
| `stew_submit_for_review` | SubmitForReview | Stewardship | Transition to `under_review` |
| `stew_record_review_decision` | RecordReviewDecision | Stewardship | Approve/RequestChange/Reject with ReviewNote |
| `stew_publish` | Publish | Stewardship | Draft→Active flip + predecessor supersede + publish event (one TX) |
| `stew_apply_template` | ApplyTemplate | Stewardship | Pre-populate changeset from template |
| `stew_validate_edit` | ValidateEdit | Stewardship | Run guardrails on a single item |
| `stew_resolve_conflict` | ResolveConflict | Stewardship | Apply merge/rebase/supersede strategy |
| `stew_describe_object` | DescribeObject | Query | Snapshot + memberships + consumers (extends existing) |
| `stew_cross_reference` | CrossReference | Query | Conflicts, duplicates, promotable candidates |
| `stew_impact_analysis` | ImpactAnalysis | Query | Blast radius for changeset items |
| `stew_coverage_report` | CoverageReport | Query | Orphans, drift, intent resolution readiness |

Each mutating tool emits a `StewardshipRecord` to the append-only event log.

### Integration Points (Phase 0)

1. **`rust/src/sem_reg/mod.rs`**: Add `pub mod stewardship;`
2. **`rust/src/sem_reg/agent/mcp_tools.rs`**: Register Phase 0 tools in `all_tool_specs()` + `dispatch_tool()`
3. **`rust/src/mcp/tools_sem_reg.rs`**: Bridge new tools to MCP server surface
4. **`sem_os_core/src/gates/mod.rs`**: Add `overlay_mode: OverlayMode` to `ExtendedGateContext` for draft-aware gate evaluation
5. **Fix `InProcessClient::publish_changeset`**: Return `changeset_id.to_string()` as `snapshot_set_id`
6. **Fix harness wiring**: Replace `NoopChangesetStore` with `PgChangesetStore`

### Phase 0 Test Scenarios

| # | Scenario | Verifies | Success Criteria (§15) |
|---|----------|----------|----------------------|
| T-P0-1 | Create changeset → add 3 items → gate precheck → publish → snapshots Active | End-to-end lifecycle | SC-1, SC-4 |
| T-P0-2 | Add item to Governed tier without proof chain → G04 blocks | ProofChainCompatibility | SC-3 |
| T-P0-3 | Add item with FQN modified in another open changeset → G10 warns | ConflictDetected | SC-6 |
| T-P0-4 | Add two items with same (object_type, object_id) → G15 blocks | DraftUniquenessViolation | SC-15 |
| T-P0-5 | Attach basis with low-confidence claim → G09 advisory | AIKnowledgeBoundary | SC-3 |
| T-P0-6 | Detect conflict → attempt publish → blocks → resolve → re-publish | Conflict lifecycle | SC-6 |
| T-P0-7 | Instantiate template → changeset pre-populated with items | Template instantiation | SC-1 |
| T-P0-8 | Publish with same client_request_id twice → idempotent (second returns cached) | Idempotency | SC-3 |
| T-P0-9 | Deprecate item without replacement_fqn → G08 blocks | DeprecationWithoutReplacement | SC-5 |
| T-P0-10 | Submit for review → approve → publish → all stewardship events in audit chain | Audit completeness | SC-3 |

---

## Phase 1: Show Loop (50–100%)

### Migration 098 (`migrations/098_stewardship_phase1.sql`)

```sql
-- Phase 1: Show Loop tables

-- 1. Focus state — server-side shared truth (spec §9.14.1)
CREATE TABLE IF NOT EXISTS stewardship.focus_states (
  session_id         UUID PRIMARY KEY,
  changeset_id       UUID REFERENCES sem_reg.changesets(changeset_id),
  overlay_mode       VARCHAR(20) NOT NULL DEFAULT 'active_only'
    CHECK (overlay_mode IN ('active_only','draft_overlay')),
  overlay_changeset_id UUID,  -- populated when overlay_mode = 'draft_overlay'
  object_refs        JSONB NOT NULL DEFAULT '[]',   -- Vec<ObjectRef> (multiple selection)
  taxonomy_focus     JSONB,                          -- Optional TaxonomyFocus
  resolution_context JSONB,                          -- Optional ResolutionContext
  updated_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_by         VARCHAR(20) NOT NULL DEFAULT 'agent'
    CHECK (updated_by IN ('agent','user_navigation'))
);

-- 2. Viewport manifests — immutable audit records (spec §9.4)
CREATE TABLE IF NOT EXISTS stewardship.viewport_manifests (
  manifest_id        UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  session_id         UUID NOT NULL,
  changeset_id       UUID,
  focus_state        JSONB NOT NULL,              -- snapshot of FocusState at capture time
  overlay_mode       VARCHAR(20) NOT NULL,
  assumed_principal   VARCHAR(200),                -- ABAC impersonation context (§2.3.4)
  viewport_refs      JSONB NOT NULL DEFAULT '[]', -- Vec<ViewportRef> with data_hash + registry_version
  created_at         TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_viewport_manifests_session
  ON stewardship.viewport_manifests (session_id, created_at DESC);
```

### Phase 1 Types (extend `types.rs`)

```rust
// ─── FocusState (§9.14.1) — server-side shared truth ───

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FocusState {
    pub session_id: Uuid,
    pub changeset_id: Option<Uuid>,
    pub overlay_mode: OverlayMode,
    pub object_refs: Vec<ObjectRef>,           // multiple selection (not single)
    pub taxonomy_focus: Option<TaxonomyFocus>,
    pub resolution_context: Option<serde_json::Value>, // ResolutionContext
    pub updated_at: DateTime<Utc>,
    pub updated_by: FocusUpdateSource,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectRef {
    pub object_type: String,
    pub object_id: Uuid,
    pub fqn: String,
    pub snapshot_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaxonomyFocus {
    pub taxonomy_fqn: String,
    pub node_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum OverlayMode {
    ActiveOnly,
    DraftOverlay { changeset_id: Uuid },
}

// ─── ShowPacket (§9.14.3) ───

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShowPacket {
    pub focus: FocusState,
    pub viewports: Vec<ViewportSpec>,
    pub deltas: Option<Vec<ViewportDelta>>,
    pub narrative: Option<String>,
    pub next_actions: Vec<SuggestedAction>,
}

// ─── SuggestedAction (§9.14.4) — closes the Refine step ───

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuggestedAction {
    pub action_type: ActionType,
    pub label: String,
    pub target: ActionTarget,
    pub enabled: bool,
    pub disabled_reason: Option<String>,
    pub keyboard_hint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionType {
    AcceptItem,
    EditItem,
    RunGates,
    SubmitForReview,
    RecordReview,
    Publish,
    ResolveConflict,
    AddEvidence,
    ToggleOverlay,
    NavigateToItem,
    Remediate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionTarget {
    pub changeset_id: Option<Uuid>,
    pub item_id: Option<Uuid>,
    pub viewport_id: Option<String>,
    pub guardrail_id: Option<String>,
}

// ─── Viewport Types (§9.14.5, §9.14.6) ───

/// The 8 viewport kinds from spec §9.14.6
/// Phase 1 implements: Focus, Object, Diff, Gates
/// Phase 2 adds: Taxonomy, Impact, ActionSurface, Coverage
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ViewportKind {
    Focus,
    Taxonomy,
    Object,
    Diff,
    Impact,
    ActionSurface,
    Gates,
    Coverage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RenderHint {
    Tree,
    Graph,
    Table,
    Diff,
    Cards,
}

/// ViewportSpec is the request: "compute this viewport"
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewportSpec {
    pub id: String,
    pub kind: ViewportKind,
    pub title: String,
    pub params: serde_json::Value,
    pub render_hint: RenderHint,
}

/// ViewportStatus (§9.14.5) — lifecycle state per viewport
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ViewportStatus {
    Ready,
    Loading { progress: Option<f32> },
    Error { message: String },
    Stale,
}

/// ViewportModel is the response: "here is the computed viewport data"
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewportModel {
    pub id: String,
    pub kind: ViewportKind,
    pub status: ViewportStatus,     // ← critical: carries lifecycle state
    pub data: serde_json::Value,    // typed by kind (see viewport data shapes)
    pub meta: ViewportMeta,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewportMeta {
    pub updated_at: DateTime<Utc>,
    pub sources: Vec<String>,       // tool call IDs that produced this data
    pub overlay_mode: OverlayMode,
}

/// ViewportDelta (§9.14.7) — incremental viewport updates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewportDelta {
    pub viewport_id: String,
    pub op: PatchOp,
    pub path: String,               // JSON Pointer into viewport data
    pub value: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PatchOp {
    Add,
    Remove,
    Replace,
    Move,
}

// ─── WorkbenchPacket Transport (§9.16) ───

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkbenchPacket {
    pub packet_id: Uuid,
    pub session_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub frame_type: String,         // "workbench" (vs "decision" for execution-side)
    pub kind: WorkbenchPacketKind,
    pub payload: WorkbenchPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkbenchPacketKind {
    Show,
    DeltaUpdate,
    StatusUpdate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WorkbenchPayload {
    ShowPayload { show_packet: ShowPacket },
    DeltaPayload { deltas: Vec<ViewportDelta> },
    StatusPayload { viewport_id: String, status: ViewportStatus },
}

// ─── ViewportManifest (§9.4) — audit record ───

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewportManifest {
    pub manifest_id: Uuid,
    pub captured_at: DateTime<Utc>,
    pub focus_state: FocusState,
    pub rendered_viewports: Vec<ViewportRef>,
    pub overlay_mode: OverlayMode,
    pub assumed_principal: Option<String>,  // ABAC impersonation (§2.3.4)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewportRef {
    pub viewport_id: String,
    pub kind: ViewportKind,
    pub data_hash: String,               // SHA-256 of RFC 8785 canonical JSON
    pub registry_version: Option<Uuid>,  // snapshot_set_id viewport was computed from
    pub tool_call_ref: Option<String>,
}
```

### FocusState Store (`focus.rs`)

```rust
/// Server-side focus state: same record updated by agent and UI.
/// FocusChanged events emitted to audit chain on every mutation.
pub struct FocusStore;

impl FocusStore {
    pub async fn get(pool: &PgPool, session_id: Uuid) -> Result<Option<FocusState>>;

    /// Set focus — records FocusChanged event in audit chain.
    /// `source` determines whether this was agent-driven or user navigation.
    pub async fn set(
        pool: &PgPool,
        focus: &FocusState,
        source: FocusUpdateSource,
        changeset_id: Option<Uuid>, // for audit event linkage
    ) -> Result<()>;

    pub async fn delete(pool: &PgPool, session_id: Uuid) -> Result<()>;
}
```

### ShowLoop Engine (`show_loop.rs`)

```rust
/// ShowLoop engine — computes ShowPacket from FocusState.
///
/// Show Loop Latency Invariant (§2.3.5):
///   FocusState + Diff viewport MUST be Ready within one interaction cycle.
///   Gates/Impact/Coverage viewports MAY be Loading initially.
pub struct ShowLoop;

impl ShowLoop {
    /// Compute full ShowPacket from current focus.
    /// Returns Focus + Diff as Ready, Gates as Loading (computed async).
    pub async fn compute_show_packet(
        pool: &PgPool,
        focus: &FocusState,
        actor: &str,
        assume_principal: Option<&str>, // ABAC impersonation for Draft Overlay (§2.3.4)
    ) -> Result<ShowPacket>;

    /// Viewport A: Focus summary — always Ready
    async fn render_focus_viewport(pool: &PgPool, focus: &FocusState) -> Result<ViewportModel>;

    /// Viewport C: Object inspector — Ready (uses existing sem_reg_describe_* tools)
    async fn render_object_inspector(pool: &PgPool, focus: &FocusState) -> Result<ViewportModel>;

    /// Viewport D: Diff (predecessor Active vs Draft successor) — always Ready
    /// Server-side diff: field-level typed diff + human-readable summary
    async fn render_diff_viewport(pool: &PgPool, focus: &FocusState) -> Result<ViewportModel>;

    /// Viewport G: Gates — may return Loading initially, then StatusUpdate when done
    async fn render_gates_viewport(
        pool: &PgPool,
        focus: &FocusState,
        actor: &str,
    ) -> Result<ViewportModel>;

    /// Compute ViewportManifest for audit (SHA-256 hashes per RFC 8785)
    pub fn compute_manifest(
        focus: &FocusState,
        viewports: &[ViewportModel],
        assumed_principal: Option<&str>,
    ) -> ViewportManifest;

    /// Compute SuggestedActions from current changeset state
    fn compute_suggested_actions(
        changeset: &ChangesetRow,
        gate_results: &[GuardrailResult],
        focus: &FocusState,
    ) -> Vec<SuggestedAction>;
}
```

### SSE Endpoint (Phase 1 transport)

New endpoint in `rust/src/api/stewardship_routes.rs`:

```
GET /api/session/:id/workbench-events
Accept: text/event-stream

→ SSE stream of WorkbenchPacket events (JSON, one per SSE event)
  Each event carries frame_type: "workbench" for UI routing
```

Implementation uses `tokio::sync::broadcast` channel per session. ShowPacket emission triggers broadcast to all connected SSE clients.

**Note on transport divergence:** The spec (§9.16) says WorkbenchPackets ride on the "same WebSocket channel" as DecisionPackets. Phase 1 uses SSE as a pragmatic first implementation. Phase 2 should migrate to the shared WebSocket channel if the existing app already uses WebSocket for DecisionPackets, adding the `frame_type: "workbench" | "decision"` discriminator at the envelope level.

### Draft-Aware Context Resolution

Extend `resolve_context()` in `rust/src/sem_reg/context_resolution.rs`:

```rust
/// When overlay_mode = DraftOverlay { changeset_id }, extend snapshot resolution:
///
///   WHERE (status = 'active' AND effective_until IS NULL)
///      OR (snapshot_set_id = $changeset_id AND status = 'draft' AND effective_until IS NULL)
///
/// Both branches require effective_until IS NULL to exclude:
///   - superseded Active history
///   - superseded Draft refinements
///
/// Draft snapshots override Active snapshots for same (object_type, object_id).
///
/// When assume_principal is set, ABAC evaluation uses that principal's roles
/// instead of the caller's. The assumed identity is recorded in ViewportManifest. (§2.3.4)

pub async fn resolve_context_with_overlay(
    pool: &PgPool,
    context: &ResolutionContext,
    overlay_mode: &OverlayMode,
    assume_principal: Option<&str>,
) -> Result<ResolvedContext>;
```

### Phase 1 MCP Tools (`tools_phase1.rs`)

| Tool | Spec Name (§6.4) | Category | Description |
|------|-------------------|----------|-------------|
| `stew_get_focus` | GetFocusState | Visualisation | Get current FocusState for session |
| `stew_set_focus` | (UI navigation API) | Visualisation | Set FocusState (emits FocusChanged event) |
| `stew_show` | (ShowPacket emission) | Visualisation | Trigger ShowPacket computation and broadcast via SSE |
| `stew_get_viewport` | GetViewportModel | Visualisation | Compute single viewport model by kind |
| `stew_get_diff` | GetDiffModel | Visualisation | Structured diff: predecessor vs draft |
| `stew_capture_manifest` | (Audit capture) | Visualisation | Compute + persist ViewportManifest with SHA-256 hashes |

### Integration Points (Phase 1)

1. **`rust/src/api/stewardship_routes.rs`** (NEW): SSE endpoint + REST routes for focus/show
2. **`rust/crates/ob-poc-web/src/main.rs`**: Mount stewardship routes
3. **`rust/src/sem_reg/context_resolution.rs`**: Add `resolve_context_with_overlay()` supporting `OverlayMode` + `assume_principal`
4. **`rust/src/sem_reg/stewardship/tools_phase1.rs`**: 6 visualisation tools
5. **`rust/src/sem_reg/agent/mcp_tools.rs`**: Register Phase 1 tools

### Phase 1 Test Scenarios

| # | Scenario | Verifies | Success Criteria (§15) |
|---|----------|----------|----------------------|
| T-P1-1 | Set focus to changeset → compute ShowPacket → Focus + Diff viewports are Ready | Latency invariant | SC-12 |
| T-P1-2 | Add item → emit ShowPacket → Diff viewport shows field-level delta | Diff rendering | SC-12 |
| T-P1-3 | DraftOverlay resolution → draft snapshots override active for same (type, id) | Overlay correctness | SC-13 |
| T-P1-4 | DraftOverlay with assume_principal → ABAC computed as execution agent | Impersonation | SC-13 |
| T-P1-5 | Gate precheck in overlay mode → validates intra-changeset references | Draft-aware gates | SC-4 |
| T-P1-6 | Capture ViewportManifest → SHA-256 hashes match RFC 8785 canonical JSON | Audit integrity | SC-14 |
| T-P1-7 | ViewportManifest includes assumed_principal when impersonation used | Audit impersonation | SC-14 |
| T-P1-8 | SSE endpoint delivers WorkbenchPacket with frame_type="workbench" | Transport | SC-12 |
| T-P1-9 | Gates viewport returns Loading initially → StatusUpdate when complete | ViewportStatus lifecycle | SC-12 |
| T-P1-10 | Focus change emits FocusChanged event to audit chain | Audit navigation | SC-14 |
| T-P1-11 | ShowPacket includes SuggestedActions with correct enabled/disabled state | Action surface | SC-12 |

---

## File Summary

### New Files

| File | Phase | Purpose |
|------|-------|---------|
| `migrations/097_stewardship_phase0.sql` | 0 | stewardship schema + 8 tables + indexes + constraints |
| `migrations/098_stewardship_phase1.sql` | 1 | focus_states + viewport_manifests tables |
| `rust/src/sem_reg/stewardship/mod.rs` | 0 | Module root |
| `rust/src/sem_reg/stewardship/types.rs` | 0+1 | All types (spec §8, §9.1–9.7, §9.14–9.16) |
| `rust/src/sem_reg/stewardship/store.rs` | 0 | DB operations for stewardship.* tables |
| `rust/src/sem_reg/stewardship/guardrails.rs` | 0 | G01–G15 engine (spec §8.2 verbatim) |
| `rust/src/sem_reg/stewardship/templates.rs` | 0 | Template CRUD + instantiation |
| `rust/src/sem_reg/stewardship/impact.rs` | 0 | Real impact analysis (replace stub) |
| `rust/src/sem_reg/stewardship/idempotency.rs` | 0 | client_request_id dedup layer |
| `rust/src/sem_reg/stewardship/tools_phase0.rs` | 0 | 17 Phase 0 MCP tools |
| `rust/src/sem_reg/stewardship/show_loop.rs` | 1 | ShowPacket engine (4 viewports) |
| `rust/src/sem_reg/stewardship/focus.rs` | 1 | FocusState store + FocusChanged events |
| `rust/src/sem_reg/stewardship/tools_phase1.rs` | 1 | 6 Phase 1 MCP tools |
| `rust/src/api/stewardship_routes.rs` | 1 | SSE endpoint + REST routes |

### Modified Files

| File | Phase | Change |
|------|-------|--------|
| `rust/src/sem_reg/mod.rs` | 0 | Add `pub mod stewardship;` |
| `rust/src/sem_reg/agent/mcp_tools.rs` | 0+1 | Register 23 new tools |
| `rust/src/mcp/tools_sem_reg.rs` | 0+1 | Bridge new tools |
| `rust/src/sem_reg/context_resolution.rs` | 1 | Add `resolve_context_with_overlay()` |
| `rust/crates/ob-poc-web/src/main.rs` | 1 | Mount stewardship routes |
| `rust/crates/sem_os_client/src/inprocess.rs` | 0 | Fix publish_changeset bug |
| `rust/crates/sem_os_harness/src/lib.rs` | 0 | Wire PgChangesetStore |
| `rust/crates/sem_os_core/src/gates/mod.rs` | 0 | Add overlay_mode to gate context |

### Test Files

| File | Phase | Scenarios |
|------|-------|-----------|
| `rust/tests/stewardship_phase0_test.rs` | 0 | T-P0-1 through T-P0-10 |
| `rust/tests/stewardship_phase1_test.rs` | 1 | T-P1-1 through T-P1-11 |

---

## Implementation Order

### Phase 0 (steps 1–11)

1. Migration 097 (stewardship schema + all tables)
2. `stewardship/types.rs` — all type definitions (both phases, since Phase 1 types are just more structs in the same file)
3. `stewardship/idempotency.rs` — client_request_id layer
4. `stewardship/store.rs` — DB operations
5. `stewardship/guardrails.rs` — G01–G15 (extend existing 3 guardrails)
6. `stewardship/templates.rs` — template CRUD + instantiation
7. `stewardship/impact.rs` — real impact analysis (replace stub)
8. `stewardship/tools_phase0.rs` — 17 MCP tools
9. `stewardship/mod.rs` + wire into `sem_reg/mod.rs`
10. Wire tools into `mcp_tools.rs` + `tools_sem_reg.rs`
11. Fix bugs (publish_changeset, harness, changeset_impact) + Phase 0 tests

→ **IMMEDIATELY proceed to Phase 1. Progress: 50%.**

### Phase 1 (steps 12–18)

12. Migration 098 (focus_states + viewport_manifests)
13. `stewardship/focus.rs` — FocusState store + FocusChanged audit events
14. `stewardship/show_loop.rs` — ShowPacket engine (4 viewports)
15. `stewardship/tools_phase1.rs` — 6 visualisation tools
16. `api/stewardship_routes.rs` — SSE endpoint
17. Draft-aware context resolution (`resolve_context_with_overlay()`)
18. Mount routes + Phase 1 tests

→ **Progress: 100%. Phase 0 + Phase 1 complete.**

---

## Spec Cross-Reference Index

Every implementation artifact traces to a spec section:

| Artifact | Spec Section |
|----------|-------------|
| Changeset identity (id == snapshot_set_id) | §9.1 |
| Draft uniqueness constraint (object_id) | §9.1 |
| Draft mutability rule | §9.1 |
| Stewardship events enum | §9.4 |
| ViewportManifest + SHA-256 hashing | §9.4 |
| Template table (versioned, lifecycle) | §9.5 |
| Conflict model (competing_changeset_id) | §9.6 |
| VerbImplementationBinding (lifecycle status) | §9.7 |
| Guardrails G01–G15 | §8.2 |
| FocusState (server-side, updated_by, resolution_context) | §9.14.1 |
| OverlayMode | §9.14.2 |
| ShowPacket | §9.14.3 |
| SuggestedAction + ActionType + ActionTarget | §9.14.4 |
| ViewportStatus (Ready/Loading/Error/Stale) | §9.14.5 |
| ViewportSpec / ViewportModel / ViewportKind | §9.14.6 |
| ViewportDelta | §9.14.7 |
| WorkbenchPacket (Show/DeltaUpdate/StatusUpdate) | §9.16 |
| Draft Overlay WHERE clause (both branches) | §9.2 |
| ABAC impersonation (assume_principal) | §2.3.4 |
| Show Loop Latency Invariant | §2.3.5 |
| Idempotency (client_request_id) | §6.2 |
| Tool names | §6.1, §6.2, §6.4 |
