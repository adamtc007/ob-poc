# Domain Features — Detailed Annex

> This annex covers contracts, deals, billing, client groups, entity linking, lexicon,
> lookup service, documents, inspector/projection, transactional execution, KYC skeleton,
> onboarding state view, playbooks, macros, and the CustomOp pattern.
> For the high-level overview see the root `CLAUDE.md`.

---

## Contracts

**Config:** `rust/config/verbs/contract.yaml`

Manages legal master service agreements (MSAs) with product-level rate cards.

| Verb | Behavior | Purpose |
|------|----------|---------|
| `contract.create` | crud | Create new contract → returns UUID |
| `contract.get` | crud | Read contract details |
| `contract.list` | crud | List contracts, optionally by client |
| `contract.update-status` | crud | DRAFT → ACTIVE → TERMINATED/EXPIRED |
| `contract.create-rate-card` | plugin | Define pricing tier within contract |
| `contract.agree-rate-card` | plugin | Move rate card to AGREED (precondition for billing) |
| `contract.list-rate-cards` | crud | List rate cards for a contract |

**Data model:** `legal_contracts`, `deal_rate_cards` (rate_card_id, deal_id, contract_id, status: DRAFT|AGREED|SUPERSEDED)

---

## Deals

**File:** `rust/src/domain_ops/deal_ops.rs` (76KB)
**Config:** `rust/config/verbs/deal.yaml`

Commercial origination hub linking sales → contracting → KYC → onboarding → billing.

| Verb | Purpose |
|------|---------|
| `deal.create` | Create sales opportunity → deal_id |
| `deal.read` | Fetch deal record |
| `deal.list` | List deals optionally by status |
| `deal.update-status` | State machine transitions |
| `deal.initiate-kyc-clearance` | Transition to KYC phase (requires approved KYC case) |
| `deal.propose-rate-card` | Create initial rate card |
| `deal.agree-rate-card` | Both parties agree (precondition for billing) |
| `deal.update-rate-card` | Adjust pricing during negotiation |

**Deal status state machine:**
```
PROSPECT → QUALIFYING → NEGOTIATING → KYC_CLEARANCE → CONTRACTED
         → ONBOARDING → ACTIVE → WINDING_DOWN → OFFBOARDED
```

Transitions are strictly enforced. Reverse moves only under specific conditions.

**Result types:**
```rust
DealCreateResult { deal_id, deal_name, deal_status }
DealStatusUpdateResult { deal_id, old_status, new_status }
```

**Data model:** `deals` (deal_id, primary_client_group_id, deal_name, status, sales_owner, estimated_revenue, currency_code), `deal_rate_cards`

---

## Billing

**File:** `rust/src/domain_ops/billing_ops.rs` (36KB)
**Config:** `rust/config/verbs/billing.yaml`

Bridges commercial deals to operational billing cycles.

| Verb | Purpose |
|------|---------|
| `billing.create-profile` | Create billing profile from AGREED rate card |
| `billing.calculate-charges` | Compute fee lines for a billing period |
| `billing.create-invoice` | Generate invoice from calculated charges |
| `billing.list-profiles` | List billing profiles for a deal |
| `billing.get-profile-status` | Check profile lifecycle state |

**`billing.create-profile` preconditions:** `requires_prior: deal.create`, `requires_prior: deal.agree-rate-card`

Maps: deal → contract → rate-card → CBU → product → invoice-entity. Supports billing-frequency (MONTHLY|QUARTERLY|ANNUAL), invoice-currency, payment method.

**Billing period states:** PENDING → OPEN → BILLED → COLLECTED → CLOSED

**Result type:**
```rust
BillingCalculationResult { period_id: Uuid, line_count: i32, gross_amount: f64, status: String }
```

**Data model:** `billing_profiles`, `billing_periods`, `billing_line_items` (base|usage|tiered fee lines)

---

## Client Groups

**File:** `rust/src/domain_ops/client_group_ops.rs` (70KB)
**Resolver:** `rust/crates/ob-semantic-matcher/src/client_group_resolver.rs`

Virtual commercial umbrella entities. Two-stage resolution: alias → group → anchor entity.

### Stage 1: Alias → ClientGroupId (Semantic)

Uses Candle embeddings (BGE-small-en-v1.5) for fuzzy matching against `ClientGroupAlias` table. `ClientGroupEmbedderAdapter` bridges `CandleEmbedder` to the `ob-semantic-matcher` `Embedder` trait.

### Stage 2: ClientGroupId → AnchorEntityId (Policy)

Maps group to anchor based on verb domain context:

| Anchor Role | Domains |
|-------------|---------|
| `UltimateParent` | ownership, ubo |
| `GovernanceController` | session, cbu, view |
| `BookController` | book management |
| `OperatingController` | contract, service |
| `RegulatoryAnchor` | kyc, screening, regulatory |

**Key verbs:** `client-group.entity-add/remove`, `client-group.assign-role/remove-role`, `client-group.add-ownership-source`, `client-group.verify-ownership`, `client-group.tag-add/remove`, `client-group.search-entities`, `client-group.discover-entities`

**Data model:** `client_group`, `client_group_alias` (with normalized aliases + confidence), `client_group_entity`, `client_group_relationship`, `client_group_relationship_sources`, `client_group_anchor` (group_id, anchor_entity_id, anchor_role, jurisdiction, confidence, priority)

---

## Entity Linking

**Files:** `rust/src/entity_linking/` (5 files)

Fast in-memory entity resolution. Extracts mention spans from utterances and resolves to canonical entity IDs with kind constraints.

**Architecture:**
```
DB tables → compiler → entity.snapshot.bin (bincode)
         → load at runtime → Arc<EntitySnapshot> → EntityLinkingServiceImpl
```

**Components:**
- `mention.rs` — Extracts mention spans (start/end indices)
- `resolver.rs` — Multi-mention resolution; returns candidates sorted by score
- `compiler.rs` — Build-time snapshot generation
- `snapshot.rs` — Bincode serialization/deserialization

**Evidence types for scoring:**
- `AliasExact` — Perfect alias match (score 1.0)
- `AliasTokenOverlap` — Partial token match
- `KindMatchBoost` — Entity kind matches expected constraint
- `KindMismatchPenalty` — Kind constraint mismatch
- `ConceptOverlapBoost` — Concept overlap with context

**EntityCandidate:**
```rust
pub struct EntityCandidate {
    pub entity_id: Uuid,
    pub entity_kind: String,
    pub canonical_name: String,
    pub score: f32,
    pub evidence: Vec<Evidence>,
}
```

**Performance target:** p95 < 5ms (all in-memory, no DB access at query time)

**Usage:**
```rust
let snapshot = EntitySnapshot::load(Path::new("rust/assets/entity.snapshot.bin"))?;
let service = EntityLinkingServiceImpl::new(Arc::new(snapshot));
let resolutions = service.resolve_mentions(
    "Set up Goldman Sachs for OTC trading",
    Some(&["company".to_string()]),  // kind constraints
    None,
    5,  // top-k
);
```

---

## Lexicon

**Files:** `rust/src/lexicon/` (5 files)

Fast lexical search lane. Runs **before** semantic embedding in `HybridVerbSearcher`. Recognizes known vocabulary (verb synonyms, entity types, domain keywords) with explainable evidence.

**Architecture:**
```
YAML → LexiconCompiler → lexicon.snapshot.bin
                       → load at runtime → Arc<LexiconSnapshot> → LexiconServiceImpl
```

**Key methods:**
- `search_verbs(phrase, target_type, limit)` → `Vec<VerbCandidate>` — <100µs
- `search_entity_types(phrase, limit)` → `Vec<EntityTypeCandidate>`
- `verb_target_types(dsl_verb)` — Entity types accepted by verb
- `verb_domain(dsl_verb)` — Domain string (e.g., "cbu")
- `verb_produces_type(dsl_verb)` — Output type (for chaining)
- `infer_domain(phrase)` — Keyword-based domain detection

**Critical rules:**
1. Hot path = in-memory only (NO DB, NO YAML parsing at query time)
2. Scores clamped to `[0, 1]` to preserve ambiguity thresholds
3. Evidence maps to `VerbEvidence` trait

**Files:** `service.rs` (9KB), `compiler.rs` (19KB), `snapshot.rs` (9KB), `types.rs` (9KB)

---

## Lookup Service

**Files:** `rust/src/lookup/` — `service.rs` (12KB)

Consolidates verb search and entity linking into a single pass. Implements **verb-first** ordering: verbs → expected_kinds → entities.

**Flow:**
```
"Set up ISDA with Goldman Sachs"
  → 1. Verb search (lexicon + semantic)        → "isda.create" (0.88)
  → 2. Derive expected_kinds from verb schema  → ["company", "counterparty"]
  → 3. Entity linking with kind constraints    → "Goldman Sachs" → entity_id
  → LookupResult { verbs, entities, dominant_entity, expected_kinds }
```

```rust
pub struct LookupResult {
    pub verbs: Vec<VerbSearchResult>,
    pub entities: Vec<EntityResolution>,
    pub dominant_entity: Option<DominantEntity>,  // Highest confidence, kind-matched
    pub expected_kinds: Vec<String>,
    pub concepts: Vec<String>,
    pub verb_matched: bool,
    pub entities_resolved: bool,
}
```

**Why verb-first:** Verb schema defines valid entity types, enabling kind-constrained disambiguation ("Apple" as company vs person depends on verb context).

---

## Documents

**Files:** `rust/src/domain_ops/document_ops.rs`, `request_ops.rs`
**Config:** `rust/config/verbs/document.yaml`, `doc-request.yaml`

Document solicitation with durable BPMN-Lite workflow integration.

| Verb | Behavior | Purpose |
|------|----------|---------|
| `document.solicit` | plugin (durable) | Request document via BPMN-Lite workflow |
| `document.upload-version` | crud | Upload new version |
| `document.verify` | plugin | Verify uploaded document |
| `document.reject` | plugin | Reject → returns to requester |
| `document.extract` | plugin | AI-assisted data extraction |
| `document.missing-for-entity` | plugin | Find missing documents |
| `document.solicit-set` | plugin | Solicit multiple at once |

**`document.solicit` durable config:**
- Process key: `kyc-document-request`
- Tasks: send_request_notification, validate_uploaded_document, update_requirement_status
- Timeout: P14D (14 days)
- Correlation via workflow-instance-id

**Request task queue verbs:** `request.create/fulfill/remind/escalate/extend/waive/cancel`

**Request states:** CREATED → SENT → RECEIVED → REJECTED → FULFILLED → CANCELLED

**Data model:** `document_requirements`, `requests` (task queue), `document_uploads`, `document_requests` (BPMN-Lite instances)

---

## Inspector / Projection

**Crate:** `rust/crates/inspector-projection/src/`

Generates deterministic JSON projections of complex entity graphs for React tree UI rendering.

### Core Types

```rust
pub struct InspectorProjection {
    pub snapshot: SnapshotMeta,
    pub render_policy: RenderPolicy,
    pub ui_hints: UiHints,
    pub root: BTreeMap<String, RefValue>,    // Entry points by chamber
    pub nodes: BTreeMap<NodeId, Node>,       // Flat node map
}
```

**NodeId** — stable path-based identifiers:
- Format: `{kind}:{qualifier}[:{subpath}]`
- Examples: `cbu:allianz-ie-funds`, `entity:uuid:fund_001`, `matrix:focus:mic:XLON`
- Kind prefix (before first `:`) is lowercase; subsequent segments allow uppercase (MIC, ISIN)
- O(1) kind lookup by prefix

**RefValue** — `$ref` linking (`{ "$ref": "node_id" }`):
- Flat node map with O(1) lookup
- No recursive descent required
- Supports cycle detection
- Deterministic serialization

**20 node type variants** including Entity, CBU, Holding, Relationship, Matrix, Register, Agreement.

**Chambers (entry points):** `"cbu"`, `"entity"`, `"holdings"`, `"relationships"` + custom domain-specific.

**RenderPolicy:** level of detail (summary/detail/full), depth limit, field filters, node kind filters.

**Files:** `model.rs` (19KB), `node_id.rs`, `ref_value.rs`, `validate.rs`, `policy.rs`, `generator/`

---

## Transactional Execution

**File:** `rust/src/domain_ops/mod.rs` (lines 362–384)

Multi-step operations with PostgreSQL advisory locks and `sqlx::Transaction`.

### execute_in_tx()

```rust
#[cfg(feature = "database")]
async fn execute_in_tx(
    &self,
    verb_call: &VerbCall,
    ctx: &mut ExecutionContext,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<ExecutionResult> {
    // Default: returns error forcing implementors to opt-in
}
```

### Advisory Lock Pattern

```rust
// Acquire transaction-scoped lock
sqlx::query("SELECT pg_advisory_xact_lock($1)")
    .bind(lock_key)
    .execute(&mut *tx)
    .await?;
// ... multi-step operation ...
tx.commit().await?;  // releases lock
```

**Examples:** stock split in `capital_ops.rs` (acquire lock → read distribution → validate ratio → update holders → audit trail → commit), batch control operations in `batch_control_ops.rs`.

**Batch control verbs:** `BatchContinueOp`, `BatchPauseOp`, `BatchSkipOp`, `BatchAbortOp`

---

## KYC Skeleton Build

**Files:** `rust/src/domain_ops/kyc_case_ops.rs`, `skeleton_build_ops.rs`

### KYC Case Lifecycle

```
DRAFT → ASSESSMENT → REVIEW → APPROVED → ACTIVE → CLOSED
```

**Plugin operations:**
- `KycCaseCreateOp` — Create case (DRAFT status)
- `KycCaseStateOp` — Get current state
- `KycCaseCloseOp` — Close case
- `WorkstreamStateOp` — Get workstream status (entity assessment, screening)

### Skeleton Build Pipeline (5 operations)

```
run_ubo_compute         → Compute UBO chain from import data
run_graph_validate      → Validate ownership/control graph integrity
run_coverage_compute    → Calculate required KYC coverage
run_outreach_plan       → Generate document solicitation plan
run_tollgate_evaluate   → Evaluate readiness gates
```

**Data model:** `cases` (case_id, client_group_id, status, cbu_id), `case_workstreams`, `case_transition_history`

---

## OnboardingStateView

**File:** `rust/src/agent/onboarding_state_view.rs` (~200 lines)

Projects `GroupCompositeState` into a UI-facing 6-layer DAG with verb suggestions. Returned on every `ChatResponse`.

### Invariants

1. **Undo is composite-level** — Revert verbs move case/screening state backward; factual attributes are corrected, not undone
2. **Utterance alignment** — Every `suggested_utterance` resolves through `HybridVerbSearcher` (tested via loopback calibration)
3. **Pruned by composite state** — Only verbs relevant to current state appear

### Structure

```rust
pub struct OnboardingStateView {
    pub group_name: Option<String>,
    pub overall_progress_pct: u8,
    pub active_layer_index: usize,
    pub layers: Vec<OnboardingLayer>,
    pub cbu_cards: Vec<CbuStateCard>,
    pub context_reset_hint: Option<String>,
}

pub struct OnboardingLayer {
    pub index: usize,
    pub name: String,
    pub state: LayerState,              // NotStarted | InProgress | Complete
    pub progress_pct: u8,
    pub forward_verbs: Vec<SuggestedVerbHint>,
    pub revert_verbs: Vec<SuggestedVerbHint>,
    pub summary: Option<String>,
}
```

### 6-Layer DAG

| Layer | Name | Forward Verbs | Revert |
|-------|------|---------------|--------|
| 1 | Group Identity | ubo.discover, ownership.trace-chain, gleif.import-tree | n/a |
| 2 | CBU Identification | cbu.create, entity.identify, discovery.run | cbu.delete |
| 3 | KYC Case | case.open, case.add-workstream, screening.run | case.revert-state |
| 4 | Screening | screening.sanctions, screening.pep, screening.adverse-media | screening.clear |
| 5 | Documents | document.solicit, document.verify | document.waive |
| 6 | Approval | tollgate.approve, governance.publish | n/a |

**Integration:** Computed in `agent_routes.rs` after `process_chat()`. Source data: `GroupCompositeState` from live DB.

---

## Playbooks

**Crates:** `rust/crates/playbook-core/`, `rust/crates/playbook-lower/`

Declarative YAML templates for multi-step workflows with DAG dependencies and slot-based parameter binding.

```rust
pub struct PlaybookSpec {
    pub id: String,
    pub version: u32,
    pub slots: HashMap<String, SlotSpec>,    // Typed parameters
    pub steps: Vec<StepSpec>,                // DAG operations
}

pub struct SlotSpec {
    pub slot_type: String,                   // "cbu_ref", "entity_id", "string"
    pub required: bool,
    pub default: Option<serde_yaml::Value>,
    pub autofill_from: Option<String>,       // e.g., "session.client_id"
}

pub struct StepSpec {
    pub id: String,
    pub verb: String,
    pub args: HashMap<String, serde_yaml::Value>, // Templated
    pub after: Vec<String>,                  // DAG dependencies
}
```

**Templating:** `${slots.X}` (slot reference), `${steps.Y.result.Z}` (prior step output), `${scope.client_id}` (session context)

**Execution:** topological order, supports dry-run. Playbooks are compiled from macro sequences. Narration: "Step 4 of 13: Lux UCITS SICAV Setup"

**LSP support:** `rust/crates/dsl-lsp/src/handlers/playbook.rs`

---

## Macros — Operator Vocabulary & Constraint Cascade

**Files:** `rust/src/macros/definition.rs`, `registry.rs`
**Config:** `rust/config/verb_schemas/macros/*.yaml`

User-friendly operator terminology that expands to deterministic DSL with DAG-based constraint cascade.

### Macro Definition

```rust
pub struct OperatorMacroDef {
    pub fqn: String,                        // "structure.setup"
    pub ui: MacroUi,                        // label, description
    pub routing: MacroRouting,
    pub target: MacroTarget,                // operates_on, produces
    pub args: MacroArgs,
    pub prereqs: Vec<MacroPrereq>,
    pub expands_to: Vec<MacroExpansion>,    // Multi-step DSL
    pub sets_state: Vec<MacroStateSet>,     // Flags set after execution
    pub unlocks: Vec<String>,               // Next available macros
}
```

### Key Rules

1. **UI masking** — Macro labels never show internal types (cbu, entity_ref, trading-profile). Use operator terms: "Structure", "Party", "Role", "Mandate"
2. **Argument translation** — Operator types (structure_ref, party_ref) → internal entity/CBU references
3. **Expansion template** — Expands to multi-step verb DSL (`structure.setup` → [cbu.create, cbu.activate, trading-profile.create-draft])
4. **State machine unlocking** — `unlocks` gates next macro candidates; prevents out-of-order operations
5. **Mode tagging** — Macros tagged by mode (onboarding, kyc, maintenance)

**Macro YAML files:**
`structure.yaml`, `case.yaml`, `mandate.yaml`, `party.yaml`, `screening-ops.yaml`, `kyc-workflow.yaml`, `kyc-workstream.yaml`, `attribute.yaml`, `evidence.yaml`, `red-flag.yaml`

---

## CustomOp Pattern — Auto-Registration

**Files:** `rust/src/domain_ops/mod.rs` (lines 322–450), `rust/crates/ob-poc-macros/src/`

~300+ custom operations, all auto-registered at link time via `inventory` crate.

### Trait

```rust
#[async_trait]
pub trait CustomOperation: Send + Sync {
    fn domain(&self) -> &'static str;
    fn verb(&self) -> &'static str;
    fn rationale(&self) -> &'static str;  // Why this needs custom code

    #[cfg(feature = "database")]
    async fn execute(&self, verb_call: &VerbCall, ctx: &mut ExecutionContext, pool: &PgPool)
        -> Result<ExecutionResult>;

    // Optional: transactional override
    #[cfg(feature = "database")]
    async fn execute_in_tx(&self, ..., tx: &mut sqlx::Transaction<'_, sqlx::Postgres>)
        -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("Operation does not support transactional execution"))
    }
}
```

### Usage

```rust
#[register_custom_op]
pub struct MyDomainCreateOp;

#[async_trait]
impl CustomOperation for MyDomainCreateOp {
    fn domain(&self) -> &'static str { "my-domain" }
    fn verb(&self) -> &'static str { "create" }
    fn rationale(&self) -> &'static str { "Complex validation + external API" }

    #[cfg(feature = "database")]
    async fn execute(&self, verb_call: &VerbCall, ctx: &mut ExecutionContext, pool: &PgPool)
        -> Result<ExecutionResult>
    {
        Ok(ExecutionResult::Uuid(uuid))
    }
}
```

**`#[register_custom_op]`** expands to `inventory::submit!()` — adds operation to linker-collected inventory at compile time. No manual registry file. Duplicate detection: panics at startup if same (domain, verb) registered twice.

**When to use CustomOp:**
- External API calls (screening, GLEIF, research)
- Complex business logic (UBO calculation, graph traversal)
- Multi-step transactions
- Side effects (notifications)

**When NOT to use:**
- Simple insert/select/update/delete → `behavior: crud`
- Templated multi-statement DSL → `behavior: template`

**Verify coverage:**
```bash
cargo test --lib -- test_plugin_verb_coverage
```
