# REPL Re-Engineering: Pack-Guided Runbook Architecture

## Overview

Implement the v3 REPL re-engineering spec: Journey Packs, 7-state machine, sentence templates, unified runbook model, and pipeline consolidation. This is Phase 0 (foundation) with a path to Phase 1–6.

**Spec files (now in project root):**
- `repl-reengineering-v3.md` — Architecture spec (frozen)
- `repl-reengineering-TODO.md` — Phase-by-phase checklist

---

## Risk Mitigation Strategy

**Feature flag isolation:** All new code behind `#[cfg(feature = "vnext-repl")]`. The default build (`cargo build`) is unchanged. This is non-negotiable — the running system has 90+ tables, active API, React frontend.

**New files alongside old:** New REPL files use `_v2` suffix or live in new `journey/` module. Old files untouched except `mod.rs` declarations (cfg-gated).

**API coexistence:** New endpoints at `/api/repl/v2/*`. Old `/api/repl/*` stays operational. React gets a new `replApiV2.ts` client.

**DB coexistence:** New tables (`runbooks_v2`, `runbook_steps_v2`, `runbook_events_v2`) have zero FK to old tables. Old tables untouched until Phase 6.

---

## Design Rules (from review feedback)

### 1. Canonical Pack Manifest Hashing

`manifest_hash()` must NOT hash serde_yaml re-serialization (map ordering varies across serde versions). Instead:

```rust
impl PackManifest {
    pub fn manifest_hash(raw_yaml_bytes: &[u8]) -> String {
        // Hash the raw file bytes — deterministic, version-independent.
        // Enforce "no reformatting" policy on pack YAML files.
        use sha2::{Sha256, Digest};
        let hash = Sha256::digest(raw_yaml_bytes);
        format!("{:x}", hash)
    }
}
```

The pack loader reads raw bytes first (for hashing), then deserializes. This guarantees: same file bytes = same hash, always.

### 2. Conversation-First, Never Forms

`options_source` on `PackQuestion` is **suggestions vocabulary only** — for optional UI affordances (autocomplete, chips). The orchestrator MUST NOT gate correctness on picker/dropdown selection. All answers are accepted as free-text `Message` input and validated against the vocabulary after the fact. No `UserInputV2` variant should force structured selection for pack questions.

### 3. Explicit Force-Select for Packs

Users can always force-select a pack by name: "Use the onboarding journey". The `PackRouter` must check for explicit pack name/id match **before** semantic routing. This is an acceptance test (see Verification section).

---

## Phase 0 — Lay the Tracks (Immediate Deliverable)

**Goal:** Golden loop end-to-end with stub execution: scope → pack select → Q/A → sentence → confirm → execute → done.

**Phase 0 cut-line (what's in vs out):**
- IN: SentenceGenerator works and is deterministic (String allocation per sentence is fine)
- IN: Runbook add/remove/reorder + sentence-only display
- IN: Packs load + canonical hash + route via hardcoded/substring selection
- IN: One integration test proving: scope → pack select → Q/A → playback → confirm → run (stub)
- IN: Template provenance (template_id, template_hash, slot sources) asserted in unit tests
- OUT: Performance benchmarks (Phase 1)
- OUT: Candle BGE semantic pack routing (Phase 1)
- OUT: `sentences.step[]` fields on verb YAML (Phase 1–2, fallback to invocation_phrases is fine)
- OUT: DB persistence of runbooks (Phase 5, in-memory is fine)

### Build Order (dependency-first)

#### Step 1: Feature Flag + Module Scaffolding

**Files modified:**
- `rust/Cargo.toml` — Add `vnext-repl = []` feature
- `rust/src/lib.rs` — Add `#[cfg(feature = "vnext-repl")] pub mod journey;`
- `rust/src/repl/mod.rs` — Add cfg-gated `mod` declarations for v2 modules

**New directories:**
- `rust/src/journey/` — Pack manifest, template, routing, playback, handoff
- `rust/config/packs/` — Starter pack YAML files

#### Step 2: Runbook Model (`rust/src/repl/runbook.rs`) — NEW

Core data model that everything else depends on. No internal deps (leaf node).

```rust
// Key types (from spec §7):
pub struct Runbook {
    pub id: Uuid,
    pub session_id: Uuid,
    pub client_group_id: Option<Uuid>,
    pub pack_id: Option<String>,
    pub pack_version: Option<String>,
    pub pack_manifest_hash: Option<String>,
    pub template_id: Option<String>,
    pub template_hash: Option<String>,
    pub status: RunbookStatus,
    pub entries: Vec<RunbookEntry>,
    pub audit: Vec<RunbookEvent>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub struct RunbookEntry {
    pub id: Uuid,
    pub sequence: i32,
    pub sentence: String,           // "Add IRS product to Allianz Lux"
    pub labels: HashMap<String, String>,
    pub dsl: String,                // "(cbu.assign-product :cbu-name ...)"
    pub verb: String,
    pub args: HashMap<String, String>,
    pub slot_provenance: SlotProvenance,
    pub arg_extraction_audit: Option<ArgExtractionAudit>,
    pub status: EntryStatus,
    pub execution_mode: ExecutionMode,
    pub confirm_policy: ConfirmPolicy,
    pub unresolved_refs: Vec<UnresolvedRef>,
    pub depends_on: Vec<Uuid>,
    pub result: Option<serde_json::Value>,
}

pub enum RunbookStatus { Draft, Building, Ready, Executing, Completed, Parked, Aborted }
pub enum EntryStatus { Proposed, Confirmed, Resolved, Executing, Completed, Failed, Parked }
pub enum ExecutionMode { Sync, Durable, HumanGate }
pub enum ConfirmPolicy { Always, QuickConfirm, PackConfigured }
pub struct SlotProvenance { pub slots: HashMap<String, SlotSource> }
pub enum SlotSource { UserProvided, TemplateDefault, InferredFromContext, CopiedFromPrevious }
pub struct ArgExtractionAudit { model_id, prompt_hash, user_input, extracted_args, confidence, timestamp }
pub enum RunbookEvent { /* event-sourced audit trail variants */ }
```

Methods: `add_entry()`, `remove_entry(id)`, `reorder(ids)`, `entry_by_id()`, `entries_by_status()`, `display_sentences()`.

**Tests:** add/remove/reorder entries, status transitions, provenance tracking, serialization roundtrip.

#### Step 3: Pack Manifest Types (`rust/src/journey/pack.rs`) — NEW

Pack manifest types + YAML loader. No internal deps (leaf node).

```rust
pub struct PackManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub invocation_phrases: Vec<String>,
    pub required_context: Vec<String>,
    pub optional_context: Vec<String>,
    pub allowed_verbs: Vec<String>,
    pub forbidden_verbs: Vec<String>,
    pub risk_policy: RiskPolicy,
    pub required_questions: Vec<PackQuestion>,
    pub optional_questions: Vec<PackQuestion>,
    pub stop_rules: Vec<String>,
    pub templates: Vec<PackTemplate>,
    pub pack_summary_template: Option<String>,
    pub section_layout: Vec<SectionLayout>,
    pub definition_of_done: Vec<String>,
    pub progress_signals: Vec<ProgressSignal>,
}

pub struct PackQuestion {
    pub field: String,
    pub prompt: String,
    pub answer_kind: AnswerKind,        // list | boolean | string | entity_ref | enum
    pub options_source: Option<String>, // SUGGESTIONS ONLY — never gates correctness
    pub default: Option<serde_json::Value>,
    pub ask_when: Option<String>,
}

pub struct PackTemplate { template_id, when_to_use, steps: Vec<TemplateStep> }
pub struct TemplateStep { verb, args, repeat_for, when, execution_mode }
pub struct RiskPolicy { require_confirm_before_execute, max_steps_without_confirm }
```

**Pack loader (two-pass for canonical hashing):**
```rust
pub fn load_pack_from_file(path: &Path) -> Result<(PackManifest, String)> {
    let raw_bytes = std::fs::read(path)?;
    let hash = PackManifest::manifest_hash(&raw_bytes);
    let manifest: PackManifest = serde_yaml::from_slice(&raw_bytes)?;
    Ok((manifest, hash))
}

pub fn load_packs_from_dir(dir: &Path) -> Result<Vec<(PackManifest, String)>>;
```

**Tests:** Deserialize all 3 starter pack YAMLs, verify all fields, hash stability (same file = same hash), hash changes when file changes.

#### Step 4: Pack Handoff Types (`rust/src/journey/handoff.rs`) — NEW

Small file. Types only.

```rust
pub struct PackHandoff {
    pub source_runbook_id: Uuid,
    pub target_pack_id: String,
    pub forwarded_context: HashMap<String, String>,
    pub forwarded_outcomes: Vec<Uuid>,
}
```

#### Step 5: Sentence Generator (`rust/src/repl/sentence_gen.rs`) — NEW

Template-based, deterministic, LLM-free. Depends on `dsl-core/config/phrase_gen.rs` for fallback.

```rust
pub struct SentenceGenerator;

impl SentenceGenerator {
    pub fn generate(&self, verb: &str, args: &HashMap<String, String>, verb_config: &VerbConfig) -> String;
    fn best_template(verb_config: &VerbConfig, args: &HashMap<String, String>) -> Option<String>;
    fn substitute(template: &str, args: &HashMap<String, String>) -> String;
    pub fn format_list(values: &[String]) -> String; // Oxford comma
}
```

**Fallback chain:** `sentences.step[]` (when added in Phase 1–2) → `invocation_phrases` (best match by arg coverage) → `phrase_gen::generate_phrases()` output.

Phase 0 performance target: deterministic and correct. No allocation micro-optimization yet — that's Phase 1.

**Tests:** 20+ verb/arg combos covering all fallback sources, edge cases (no templates, no args, list of 1/2/5+), Oxford comma formatting.

#### Step 6: State Machine Types (`rust/src/repl/types_v2.rs`) — NEW

New 7-state machine from spec §8.3.

```rust
pub enum ReplStateV2 {
    ScopeGate { pending_input: Option<String> },
    JourneySelection { candidates: Option<Vec<PackCandidate>> },
    InPack { pack_id: String, required_slots_remaining: Vec<String>, last_proposal_id: Option<Uuid> },
    Clarifying { question: String, candidates: Vec<VerbCandidate>, original_input: String },
    SentencePlayback { sentence: String, verb: String, dsl: String, args: HashMap<String, String> },
    RunbookEditing,
    Executing { runbook_id: Uuid, progress: ExecutionProgress },
}

// Simplified UserInput - conversational model, not typed forms
pub enum UserInputV2 {
    Message { content: String },
    Confirm,
    Reject,
    Edit { step_id: Uuid, field: String, value: String },
    Command { command: ReplCommandV2 },
    SelectPack { pack_id: String },
    SelectVerb { verb_fqn: String, original_input: String },
    SelectEntity { ref_id: String, entity_id: Uuid, entity_name: String },
    SelectScope { group_id: Uuid, group_name: String },
}

pub enum ReplCommandV2 { Run, Undo, Redo, Clear, Cancel, Info, Help, Remove(Uuid), Reorder(Vec<Uuid>) }
```

#### Step 7: Session V2 (`rust/src/repl/session_v2.rs`) — NEW

Session that owns a Runbook instead of a ledger.

```rust
pub struct ReplSessionV2 {
    pub id: Uuid,
    pub state: ReplStateV2,
    pub client_context: Option<ClientContext>,
    pub journey_context: Option<JourneyContext>,
    pub runbook: Runbook,
    pub messages: Vec<ChatMessage>,  // conversation history for display
    pub created_at: DateTime<Utc>,
    pub last_active_at: DateTime<Utc>,
}

pub struct ClientContext {
    pub client_group_id: Uuid,
    pub client_group_name: String,
    pub default_cbu: Option<Uuid>,
    pub default_book: Option<Uuid>,
}

pub struct JourneyContext {
    pub pack: Arc<PackManifest>,
    pub pack_manifest_hash: String,
    pub answers: HashMap<String, serde_json::Value>,
    pub template_id: Option<String>,
    pub progress: PackProgress,
    pub handoff_source: Option<PackHandoff>,
}
```

#### Step 8: Response V2 (`rust/src/repl/response_v2.rs`) — NEW

Sentence-first responses.

```rust
pub struct ReplResponseV2 {
    pub state: ReplStateV2,
    pub kind: ReplResponseKindV2,
    pub message: String,
    pub runbook_summary: Option<String>,   // pack-level playback
    pub step_count: usize,
}

pub enum ReplResponseKindV2 {
    ScopeRequired { prompt: String },
    JourneyOptions { packs: Vec<PackCandidate> },
    Question { field: String, prompt: String, answer_kind: String },
    SentencePlayback { sentence: String, verb: String, step_sequence: i32 },
    RunbookSummary { chapters: Vec<ChapterView>, summary: String },
    Executed { results: Vec<StepResult> },
    Error { error: String, recoverable: bool },
}
```

#### Step 9: Template Instantiation (`rust/src/journey/template.rs`) — NEW

Expand pack template skeleton into runbook entries.

```rust
pub fn instantiate_template(
    template: &PackTemplate,
    context: &ClientContext,
    answers: &HashMap<String, serde_json::Value>,
    sentence_gen: &SentenceGenerator,
    verb_config: &VerbsConfig,
) -> Result<Vec<RunbookEntry>>;
```

Handles: `{context.*}` / `{answers.*}` substitution, `repeat_for` expansion, `when` conditionals, `execution_mode` per step, `SlotProvenance` on each arg.

**Tests MUST assert template provenance:**
- `template_id` and `template_hash` set on parent Runbook
- Each entry's `slot_provenance` correctly tags `UserProvided` vs `TemplateDefault` vs `InferredFromContext`
- `repeat_for` entries each get distinct provenance

#### Step 10: Pack Router (`rust/src/journey/router.rs`) — NEW

Phase 0: substring + exact-name matching. Phase 1: Candle BGE semantic search.

```rust
pub struct PackRouter {
    packs: Vec<(Arc<PackManifest>, String)>,  // (manifest, hash)
}

impl PackRouter {
    pub fn load(config_dir: &Path) -> Result<Self>;
    pub fn route(&self, input: &str, context: Option<&ClientContext>) -> PackRouteOutcome;
    pub fn list_packs(&self) -> Vec<PackCandidate>;
    pub fn get_pack(&self, pack_id: &str) -> Option<&(Arc<PackManifest>, String)>;
}

pub enum PackRouteOutcome {
    Matched(Arc<PackManifest>, String),  // manifest + hash
    Ambiguous(Vec<PackCandidate>),
    NoMatch,
}
```

**Routing priority (spec §4.2):**
1. **Explicit name match** — "use the onboarding journey" / "use onboarding-request" → exact pack id/name match (HIGHEST PRIORITY, always wins)
2. **Substring match on invocation_phrases** — Phase 0 implementation
3. **Fallback** — list available packs and ask user

#### Step 11: Pack Playback (`rust/src/journey/playback.rs`) — NEW

```rust
pub struct PackPlayback;

impl PackPlayback {
    pub fn summarize(pack: &PackManifest, runbook: &Runbook, answers: &HashMap<String, serde_json::Value>) -> String;
    pub fn chapter_view(pack: &PackManifest, runbook: &Runbook) -> Vec<ChapterView>;
}

pub struct ChapterView {
    pub chapter: String,
    pub steps: Vec<(i32, String)>,  // (sequence, sentence)
}
```

#### Step 12: Orchestrator V2 (`rust/src/repl/orchestrator_v2.rs`) — NEW

The heart of the new system. Depends on everything above.

```rust
pub struct ReplOrchestratorV2 {
    pack_router: PackRouter,
    sentence_gen: SentenceGenerator,
    sessions: Arc<RwLock<HashMap<Uuid, ReplSessionV2>>>,
    executor: Option<Arc<dyn DslExecutor>>,
}

impl ReplOrchestratorV2 {
    pub async fn process(&self, session_id: Uuid, input: UserInputV2) -> Result<ReplResponseV2>;
}
```

**State Machine Dispatch:**

| Current State | Input | Handler | Next State |
|---|---|---|---|
| ScopeGate | Message | try_resolve_scope() | JourneySelection (if resolved) or ScopeGate (ask again) |
| ScopeGate | SelectScope | set_scope() | JourneySelection |
| JourneySelection | Message | route_pack() | InPack (if matched) or JourneySelection (ask) |
| JourneySelection | SelectPack | activate_pack() | InPack |
| InPack | Message | match_verb_in_pack() | SentencePlayback or Clarifying |
| InPack | Command(Run) | validate_and_execute() | Executing |
| Clarifying | Message/SelectVerb/SelectEntity | resolve_clarification() | SentencePlayback or Clarifying |
| SentencePlayback | Confirm | add_to_runbook() | RunbookEditing (or InPack for more) |
| SentencePlayback | Reject | discard_proposal() | InPack |
| RunbookEditing | Command(Run) | execute_runbook() | Executing |
| RunbookEditing | Message | match_verb_in_pack() | SentencePlayback |
| RunbookEditing | Command(Remove/Reorder) | edit_runbook() | RunbookEditing |
| Executing | (completion) | record_outcomes() | RunbookEditing or InPack |

#### Step 13: Starter Pack YAMLs — NEW

- `rust/config/packs/onboarding-request.yaml` — Full schema from spec §4.1
- `rust/config/packs/book-setup.yaml` — Lux SICAV + UK OEIC templates
- `rust/config/packs/kyc-case.yaml` — New case + renewal templates

#### Step 14: Integration Test

`rust/tests/repl_v2_golden_loop.rs` — Full end-to-end test:
1. Create session
2. Set scope (Allianz)
3. Route to Onboarding pack
4. Answer required questions (products, trading matrix)
5. Confirm sentence playback
6. Execute runbook (stub)
7. Verify runbook entries have sentences, DSL, provenance
8. Verify template provenance fields populated (template_id, template_hash, slot sources)

---

## Module Dependency Graph (Phase 0)

```
dsl-core/config/phrase_gen.rs (existing, unchanged)
    │
    ▼
repl/sentence_gen.rs ◄─── journey/pack.rs
    │                         │
    ▼                         ▼
repl/runbook.rs        journey/template.rs
    │                         │
    ▼                         ▼
repl/session_v2.rs     journey/router.rs
    │                         │
    ▼                         ▼
repl/types_v2.rs       journey/playback.rs
    │                         │
    └────────┬────────────────┘
             ▼
    repl/orchestrator_v2.rs
             │
             ▼
    repl/response_v2.rs
```

---

## What NOT to Touch (Phase 0)

- `mcp/verb_search.rs` — HybridVerbSearcher (wrap in Phase 1)
- `mcp/intent_pipeline.rs` — IntentPipeline (deprecate in Phase 6)
- `mcp/scope_resolution.rs` — ScopeResolver (wrap in Phase 1)
- `repl/orchestrator.rs` — Existing orchestrator (coexists)
- `repl/types.rs` — Existing types (coexists)
- `repl/intent_matcher.rs` — Existing matcher (coexists)
- `repl/session.rs` — Existing session (coexists)
- `session/unified.rs` — UnifiedSession (coexists)
- `session/dsl_sheet.rs` — Already deprecated
- `api/repl_routes.rs` — Existing routes (add v2 routes alongside)
- Any verb YAML files (sentence fields added in Phase 1–2)

---

## Phase 1 — Real Wiring: Packs + Verbs + Execution (Week 1–2)

**Goal:** Replace stubs with real infrastructure. Candle BGE pack routing, real DSL execution, ArgExtractionAudit, sentence templates on all pack verbs.

**Phase 0 is DONE** (132 tests, clippy clean). Phase 1 builds on the scaffolding.

### Phase 1 Quality Gates (non-negotiable)

1. **No pack verbs fall back to search_phrases** — enforced by test. All verbs used by starter packs have `sentences.step[]` and the sentence generator uses them.
2. **Pack shorthand test (Scenario D)** — "Onboard Allianz Lux CBU book" → Onboarding pack → template → slot-filling questions → one-paragraph playback.
3. **Force-select test** — "use onboarding journey" → immediate pack activation.

### Step 1.1: VerbConfigIndex — Read-Only Verb Projection

**File:** `rust/src/repl/verb_config_index.rs` (NEW)

The orchestrator_v2 and sentence_gen currently receive `invocation_phrases` and `description` as loose `HashMap<String, Vec<String>>` and `HashMap<String, String>`. This step creates a proper read-only index.

```rust
pub struct VerbConfigIndex {
    entries: HashMap<String, VerbIndexEntry>,
}

pub struct VerbIndexEntry {
    pub fqn: String,
    pub description: String,
    pub invocation_phrases: Vec<String>,
    pub sentences: Option<VerbSentences>,  // None until Phase 2 YAML migration
    pub args: Vec<ArgSummary>,
    pub confirm_policy: ConfirmPolicy,
}

pub struct ArgSummary {
    pub name: String,
    pub arg_type: String,
    pub required: bool,
    pub description: Option<String>,
}

impl VerbConfigIndex {
    pub fn from_verbs_config(config: &VerbsConfig) -> Self;
    pub fn get(&self, verb_fqn: &str) -> Option<&VerbIndexEntry>;
    pub fn verbs_for_domain(&self, domain: &str) -> Vec<&VerbIndexEntry>;
    pub fn all_verbs(&self) -> impl Iterator<Item = &VerbIndexEntry>;
}
```

**Depends on:** `dsl-core/src/config/types.rs` (VerbsConfig, VerbConfig, ArgConfig)
**Used by:** SentenceGenerator, template instantiation, orchestrator_v2

### Step 1.2: Candle BGE Pack Routing

**File:** `rust/src/journey/router.rs` (MODIFY)

Add semantic scoring between substring match and fallback. Insert `PackSemanticScorer` trait for testability.

```rust
/// Trait for semantic scoring — decouples Candle from PackRouter for testing.
#[async_trait]
pub trait PackSemanticScorer: Send + Sync {
    async fn score(&self, query: &str, phrases: &[String]) -> Vec<f32>;
}

/// Real implementation using CandleEmbedder (BGE-small-en-v1.5).
pub struct CandlePackScorer {
    embedder: SharedEmbedder,
}

impl PackSemanticScorer for CandlePackScorer {
    async fn score(&self, query: &str, phrases: &[String]) -> Vec<f32> {
        let query_vec = self.embedder.embed_query(query).await?;
        phrases.iter().map(|p| {
            let target_vec = self.embedder.embed_target(p).await?;
            cosine_similarity(&query_vec, &target_vec)
        }).collect()
    }
}
```

**Routing priority update:**
1. Force-select (exact name/id) — unchanged
2. Substring match — unchanged
3. **NEW: Candle BGE semantic match** — embed query, score against pack invocation_phrases
4. Fallback — list packs

**Integration point:** `SharedEmbedder` from `rust/src/agent/learning/embedder.rs`
**Threshold:** 0.65 for semantic pack match (same as verb search)

### Step 1.3: RealDslExecutor — Bridge to Existing Pipeline

**File:** `rust/src/repl/executor_bridge.rs` (NEW)

Replace `StubExecutor` with a bridge to the real dsl-core parse → compile → execute pipeline.

```rust
pub struct RealDslExecutor {
    pool: PgPool,
    verb_config: Arc<VerbsConfig>,
    runtime_registry: Arc<RuntimeVerbRegistry>,
}

#[async_trait]
impl DslExecutor for RealDslExecutor {
    async fn execute(&self, dsl: &str) -> Result<serde_json::Value, String> {
        // 1. Parse DSL via dsl_core::parser::parse_program()
        // 2. Compile via dsl_core::compiler
        // 3. Execute via dsl_v2::executor with pool + verb_config
        // 4. Return JSON result
    }
}
```

**Depends on:** `rust/src/dsl_v2/executor.rs`, `dsl-core/src/parser.rs`
**Reuses:** Existing `execute_plan_best_effort()` / `execute_plan_atomic_with_locks()` from `dsl_v2/executor.rs`

### Step 1.4: Wire HybridIntentMatcher into OrchestratorV2

**File:** `rust/src/repl/orchestrator_v2.rs` (MODIFY)

Currently `handle_in_pack_msg()` uses a stub verb matching approach (substring on `verb_phrases`). Replace with real `HybridIntentMatcher` from the v1 system.

```rust
pub struct ReplOrchestratorV2 {
    pack_router: PackRouter,
    sentence_gen: SentenceGenerator,
    verb_config_index: Arc<VerbConfigIndex>,
    intent_matcher: Arc<dyn IntentMatcher>,  // NEW — replaces verb_phrases HashMap
    sessions: Arc<RwLock<HashMap<Uuid, ReplSessionV2>>>,
    executor: Arc<dyn DslExecutor>,
}
```

**Changes:**
- `handle_in_pack_msg()` calls `intent_matcher.match_intent()` with `MatchContext` built from session
- Filter results by pack's `allowed_verbs` / `forbidden_verbs`
- Map `MatchOutcome::Matched` → SentencePlayback, `Ambiguous` → Clarifying, `NoMatch` → ask again
- Use `VerbConfigIndex` for sentence generation instead of loose HashMaps

### Step 1.5: ArgExtractionAudit Capture

**File:** `rust/src/repl/orchestrator_v2.rs` (MODIFY), `rust/src/repl/runbook.rs` (MODIFY)

When `IntentMatcher` returns a match with LLM-extracted args, capture the audit trail.

```rust
// In orchestrator_v2, after match_intent returns:
let audit = ArgExtractionAudit {
    model_id: result.debug.model_id.unwrap_or_default(),
    prompt_hash: hash_prompt(&result.debug.prompt_used),
    user_input: input.to_string(),
    extracted_args: result.args.clone(),
    confidence: result.debug.confidence.unwrap_or(0.0),
    timestamp: Utc::now(),
};
entry.arg_extraction_audit = Some(audit);
```

**Test:** Verify audit fields populated on LLM-derived entries, None on template-derived entries.

### Step 1.6: Sentence Templates for Pack Verbs + ConfirmPolicy

**File:** `rust/src/repl/sentence_templates.rs` (NEW)

Hardcoded sentence templates for the ~15 verbs used by starter packs. This is a bridge until Phase 2 adds `sentences` to VerbConfig YAML.

```rust
/// Hardcoded sentence templates for pack verbs (Phase 1 bridge).
/// Replaced by VerbConfig.sentences field in Phase 2.
pub fn pack_verb_sentence_templates() -> HashMap<String, Vec<String>> {
    let mut m = HashMap::new();
    m.insert("cbu.assign-product".into(), vec![
        "Assign {product} product to {cbu-name}".into(),
        "Add {product} to {cbu-name} product list".into(),
    ]);
    m.insert("trading-profile.create-matrix".into(), vec![
        "Create trading matrix for {cbu-name}".into(),
    ]);
    // ... ~15 verbs total
    m
}

/// Hardcoded confirm policies for pack verbs.
pub fn pack_verb_confirm_policies() -> HashMap<String, ConfirmPolicy> {
    let mut m = HashMap::new();
    m.insert("session.load-galaxy".into(), ConfirmPolicy::QuickConfirm);
    m.insert("session.load-cbu".into(), ConfirmPolicy::QuickConfirm);
    // All data-modifying verbs default to ConfirmPolicy::Always
    m
}
```

**Modify `SentenceGenerator::generate()`** to check `pack_verb_sentence_templates()` FIRST, before invocation_phrases fallback.

**Verbs to cover (~15):** `cbu.assign-product`, `cbu.create`, `cbu.assign-manco`, `trading-profile.create-matrix`, `trading-profile.add-counterparty`, `trading-profile.add-instrument`, `entity.ensure-or-create`, `kyc.open-case`, `kyc.request-docs`, `kyc.review-gate`, `onboarding.create-request`, `session.load-galaxy`, `session.load-cbu`, `contract.subscribe`, `contract.add-product`

### Step 1.7: Phase 1 Integration Tests

**File:** `rust/tests/repl_v2_integration.rs` (NEW)

```rust
#[tokio::test]
async fn test_candle_pack_routing() { /* semantic route → correct pack */ }

#[tokio::test]
async fn test_real_execution_bridge() { /* stub parse + execute pipeline */ }

#[tokio::test]
async fn test_pack_verb_filtering() { /* InPack filters by allowed_verbs */ }

#[tokio::test]
async fn test_arg_extraction_audit_populated() { /* LLM entry has audit */ }

#[tokio::test]
async fn test_sentence_templates_no_fallback() { /* pack verbs use templates, not search_phrases */ }

#[tokio::test]
async fn test_confirm_policy_quick_confirm() { /* nav verbs get QuickConfirm */ }

#[tokio::test]
async fn test_scenario_d_pack_shorthand() { /* "Onboard Allianz Lux" → full pipeline */ }
```

### Phase 1 File Inventory

| File | Action | Purpose |
|---|---|---|
| `rust/src/repl/verb_config_index.rs` | NEW | Read-only verb projection |
| `rust/src/repl/executor_bridge.rs` | NEW | Bridge DslExecutor to real pipeline |
| `rust/src/repl/sentence_templates.rs` | NEW | Hardcoded templates for pack verbs |
| `rust/src/journey/router.rs` | MODIFY | Add PackSemanticScorer + Candle BGE |
| `rust/src/repl/orchestrator_v2.rs` | MODIFY | Wire IntentMatcher, VerbConfigIndex, ConfirmPolicy |
| `rust/src/repl/sentence_gen.rs` | MODIFY | Check sentence_templates before invocation_phrases |
| `rust/src/repl/runbook.rs` | MODIFY | Populate ArgExtractionAudit |
| `rust/src/repl/mod.rs` | MODIFY | Add new module declarations |
| `rust/tests/repl_v2_integration.rs` | NEW | Phase 1 integration tests |

---

## Phase 2 — IntentService + Verb Sentences + Deprecation (Week 3–4)

**Goal:** Unified IntentService pipeline. `sentences` field on VerbConfig YAML. Deprecate old pipeline entry points.

### Step 2.1: VerbSentences on VerbConfig

**File:** `rust/crates/dsl-core/src/config/types.rs` (MODIFY)

Add new fields to VerbConfig:

```rust
// Add to VerbConfig struct:
/// Sentence templates for human-readable playback (§9).
#[serde(default)]
pub sentences: Option<VerbSentences>,

/// Confirm policy for this verb (default: Always).
#[serde(default)]
pub confirm_policy: Option<ConfirmPolicyConfig>,

/// Journey tags — which packs are allowed to use this verb.
#[serde(default)]
pub journey_tags: Vec<String>,

// New types:
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct VerbSentences {
    /// Step templates: "Assign {product} to {cbu-name}"
    #[serde(default)]
    pub step: Vec<String>,
    /// Summary templates: "assigned {product}"
    #[serde(default)]
    pub summary: Vec<String>,
    /// Clarification prompts per arg: {"product": "Which product?"}
    #[serde(default)]
    pub clarify: HashMap<String, String>,
    /// Completed sentence: "{product} is now assigned to {cbu-name}"
    #[serde(default)]
    pub completed: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfirmPolicyConfig {
    Always,
    QuickConfirm,
    PackConfigured,
}
```

**Backwards compat:** All fields are `Option` or `#[serde(default)]`. Existing YAML loads unchanged.

### Step 2.2: IntentService — Unified 5-Phase Pipeline

**File:** `rust/src/repl/intent_service.rs` (NEW)

Single service that orchestrates the entire intent → sentence pipeline.

```rust
pub struct IntentService {
    scope_resolver: Arc<dyn ScopeResolver>,
    pack_router: PackRouter,
    verb_searcher: Arc<HybridVerbSearcher>,
    llm_client: Option<Arc<dyn LlmClient>>,
    sentence_gen: SentenceGenerator,
    verb_config_index: Arc<VerbConfigIndex>,
}

impl IntentService {
    /// Phase 0: Scope resolution
    pub async fn resolve_scope(&self, input: &str, ctx: &SessionContext) -> ScopeOutcome;

    /// Phase 1: Pack routing (if in JourneySelection state)
    pub async fn route_pack(&self, input: &str, ctx: &SessionContext) -> PackRouteOutcome;

    /// Phase 2: Verb matching (filtered by pack allowed_verbs if active)
    pub async fn match_verb(&self, input: &str, ctx: &VerbMatchContext) -> VerbMatchOutcome;

    /// Phase 3: Arg extraction (LLM call)
    pub async fn extract_args(&self, input: &str, verb: &str, ctx: &ArgContext) -> ArgExtractionOutcome;

    /// Phase 4: Sentence generation (deterministic, no LLM)
    pub fn generate_sentence(&self, verb: &str, args: &HashMap<String, String>) -> String;

    /// Phase 5: DSL assembly (deterministic)
    pub fn assemble_dsl(&self, verb: &str, args: &HashMap<String, String>) -> Result<String>;
}

pub enum ScopeOutcome { Resolved(ClientContext), NeedsSelection(Vec<ClientGroupOption>), NoScope }
pub enum VerbMatchOutcome { Matched(VerbMatch), Ambiguous(Vec<VerbCandidate>), NoMatch }
pub enum ArgExtractionOutcome { Complete(HashMap<String, String>, ArgExtractionAudit), NeedsClarification(Vec<String>) }
```

**Key principle:** Each phase is a pure function that returns an outcome enum. No side effects on session state. The orchestrator calls phases sequentially and drives state transitions.

**Depends on:**
- `rust/src/mcp/verb_search.rs` (HybridVerbSearcher) — wrap, don't rewrite
- `rust/src/mcp/scope_resolution.rs` (ScopeResolver) — wrap, don't rewrite
- `rust/src/repl/intent_matcher.rs` (LlmClient) — reuse for arg extraction

### Step 2.3: Wire IntentService into OrchestratorV2

**File:** `rust/src/repl/orchestrator_v2.rs` (MODIFY)

Replace `IntentMatcher` dependency with `IntentService`. The orchestrator now calls phases explicitly:

```rust
// In handle_in_pack_msg():
let verb_outcome = self.intent_service.match_verb(input, &verb_ctx).await?;
match verb_outcome {
    VerbMatchOutcome::Matched(m) => {
        let arg_outcome = self.intent_service.extract_args(input, &m.verb, &arg_ctx).await?;
        match arg_outcome {
            ArgExtractionOutcome::Complete(args, audit) => {
                let sentence = self.intent_service.generate_sentence(&m.verb, &args);
                // → SentencePlayback state
            }
            ArgExtractionOutcome::NeedsClarification(missing) => {
                // → Clarifying state with sentences.clarify prompts
            }
        }
    }
    VerbMatchOutcome::Ambiguous(candidates) => { /* → Clarifying */ }
    VerbMatchOutcome::NoMatch => { /* ask again */ }
}
```

### Step 2.4: Sentence Templates for ALL Exposed Verbs

**Files:** `rust/config/verbs/*.yaml` (MODIFY — ~20 YAML files for pack-used verbs)

Add `sentences` field to every verb used by starter packs AND commonly used verbs:

```yaml
# Example: config/verbs/cbu.yaml
assign-product:
  description: "Assign a product to a CBU"
  behavior: plugin
  sentences:
    step:
      - "Assign {product} product to {cbu-name}"
      - "Add {product} to the {cbu-name} product list"
    summary:
      - "assigned {product}"
    clarify:
      product: "Which product should be assigned? (e.g., CUSTODY, TA, EQUITY)"
      cbu-name: "Which structure should receive this product?"
    completed: "{product} is now assigned to {cbu-name}"
  confirm_policy: always
  # existing fields unchanged...
```

**Coverage target:** All ~15 pack verbs MUST have `sentences.step[]`. Additional high-traffic verbs (~30 more) should also get templates. Total: ~45 verbs with sentence templates.

**Test:** `sentence_generator_no_fallback_for_pack_verbs` — asserts that pack verbs use `sentences.step[]` and never reach the invocation_phrases fallback.

### Step 2.5: Deprecation Markers + V2 API Routes

**File:** `rust/src/repl/intent_matcher.rs` (MODIFY), `rust/src/mcp/intent_pipeline.rs` (MODIFY)

```rust
// Mark old entry points deprecated:
#[deprecated(since = "0.2.0", note = "Use IntentService instead")]
pub trait IntentMatcher { ... }

#[deprecated(since = "0.2.0", note = "Use IntentService::match_verb() instead")]
pub struct IntentPipeline { ... }
```

**File:** `rust/src/api/repl_routes_v2.rs` (NEW)

V2 API routes that use the new orchestrator:

```rust
pub fn repl_v2_routes() -> Router<AppState> {
    Router::new()
        .route("/api/repl/v2/session", post(create_session_v2))
        .route("/api/repl/v2/session/:id", get(get_session_v2))
        .route("/api/repl/v2/session/:id/input", post(input_v2))
        .route("/api/repl/v2/session/:id", delete(delete_session_v2))
}
```

**File:** `ob-poc-ui-react/src/api/replV2.ts` (NEW)

TypeScript client for v2 API (mirrors `repl.ts` structure).

### Phase 2 File Inventory

| File | Action | Purpose |
|---|---|---|
| `rust/crates/dsl-core/src/config/types.rs` | MODIFY | Add VerbSentences, ConfirmPolicyConfig, journey_tags |
| `rust/src/repl/intent_service.rs` | NEW | Unified 5-phase pipeline |
| `rust/src/repl/orchestrator_v2.rs` | MODIFY | Wire IntentService, remove IntentMatcher dep |
| `rust/config/verbs/*.yaml` (~20 files) | MODIFY | Add sentences + confirm_policy |
| `rust/src/repl/intent_matcher.rs` | MODIFY | Add #[deprecated] |
| `rust/src/mcp/intent_pipeline.rs` | MODIFY | Add #[deprecated] |
| `rust/src/api/repl_routes_v2.rs` | NEW | V2 HTTP routes |
| `ob-poc-ui-react/src/api/replV2.ts` | NEW | V2 TypeScript client |
| `rust/src/repl/sentence_templates.rs` | DELETE | Replaced by VerbConfig.sentences |
| `rust/src/repl/mod.rs` | MODIFY | Add intent_service module |

---

## Phase 3 — Proposal Engine (Week 5)

**Goal:** Deterministic step proposals from runbook state + user message + pack context.

### Step 3.1: ProposalEngine Trait + PackScopedProposalEngine

**File:** `rust/src/repl/proposal.rs` (NEW)

```rust
#[async_trait]
pub trait ProposalEngine: Send + Sync {
    async fn propose(
        &self,
        input: &str,
        runbook: &Runbook,
        journey_ctx: &JourneyContext,
        client_ctx: &ClientContext,
    ) -> Result<Vec<StepProposal>>;
}

pub struct StepProposal {
    pub verb: String,
    pub args: HashMap<String, String>,
    pub sentence: String,
    pub evidence: String,        // "matches template 'standard-onboarding' step 3"
    pub confidence: f32,
    pub slot_provenance: SlotProvenance,
    pub confirm_policy: ConfirmPolicy,
    pub execution_mode: ExecutionMode,
}

pub enum ProposalOutcome {
    Ready(StepProposal),
    NeedsClarification { question: String, candidates: Vec<VerbCandidate> },
    NeedsEntityResolution { refs: Vec<UnresolvedRef> },
    NoMatch { message: String },
}
```

### Step 3.2: PackScopedProposalEngine Implementation

```rust
pub struct PackScopedProposalEngine {
    intent_service: Arc<IntentService>,
    verb_config_index: Arc<VerbConfigIndex>,
}

impl ProposalEngine for PackScopedProposalEngine {
    async fn propose(&self, input: &str, runbook: &Runbook, journey_ctx: &JourneyContext, client_ctx: &ClientContext) -> Result<Vec<StepProposal>> {
        // 1. Template fast path: check if input matches a template step
        //    by comparing against pack template step descriptions
        if let Some(template_match) = self.try_template_match(input, journey_ctx) {
            return Ok(vec![template_match]);
        }

        // 2. Verb search within pack's allowed_verbs
        let verb_ctx = VerbMatchContext {
            allowed_verbs: Some(&journey_ctx.pack.allowed_verbs),
            forbidden_verbs: Some(&journey_ctx.pack.forbidden_verbs),
            ..Default::default()
        };
        let verb_outcome = self.intent_service.match_verb(input, &verb_ctx).await?;

        // 3. Build StepProposal from match + extracted args
        // 4. Tag provenance as UserProvided for all args
        // 5. Generate sentence via intent_service.generate_sentence()
    }
}
```

### Step 3.3: Wire ProposalEngine into OrchestratorV2

**File:** `rust/src/repl/orchestrator_v2.rs` (MODIFY)

Replace `handle_in_pack_msg()` internals with `ProposalEngine::propose()` call.

```rust
// In InPack state, Message input:
let proposals = self.proposal_engine.propose(input, &session.runbook, journey_ctx, client_ctx).await?;
match proposals.as_slice() {
    [single] if single.confidence >= 0.8 => {
        // → SentencePlayback with single proposal
    }
    multiple if !multiple.is_empty() => {
        // → Clarifying with ranked proposals
    }
    _ => {
        // → ask user to rephrase
    }
}
```

### Step 3.4: Proposal Evidence and Reproducibility

**Invariant:** Same (input, runbook_state, journey_ctx) → same proposals. No randomness, no non-deterministic LLM in ranking. LLM is only used for arg extraction AFTER verb is selected.

Evidence strings enable audit: "Template 'standard-onboarding' step 3", "Semantic match to cbu.assign-product (score 0.87)".

### Phase 3 File Inventory

| File | Action | Purpose |
|---|---|---|
| `rust/src/repl/proposal.rs` | NEW | ProposalEngine trait + PackScopedProposalEngine |
| `rust/src/repl/orchestrator_v2.rs` | MODIFY | Wire ProposalEngine |
| `rust/src/repl/mod.rs` | MODIFY | Add proposal module |
| `rust/tests/repl_v2_proposal.rs` | NEW | Proposal engine tests |

---

## Phase 4 — Runbook Editing UX + Conversational Clarification (Week 6)

**Goal:** Co-authoring. Runbook is the artifact, chat is the assistant.

### Step 4.1: EditIntent Detection

**File:** `rust/src/repl/edit_intent.rs` (NEW)

Keyword-based detection of editing intent (no LLM needed):

```rust
pub enum EditIntent {
    RemoveStep { step_id: Uuid },
    ReorderSteps { new_order: Vec<Uuid> },
    ReplaceStepArgs { step_id: Uuid, new_args: HashMap<String, String> },
    InsertBefore { before_id: Uuid, proposal: StepProposal },
    InsertAfter { after_id: Uuid, proposal: StepProposal },
    DisableStep { step_id: Uuid },
    EnableStep { step_id: Uuid },
}

pub fn detect_edit_intent(input: &str, runbook: &Runbook) -> Option<EditIntent> {
    // Pattern matching:
    // "remove step 3" → RemoveStep
    // "swap steps 2 and 4" → ReorderSteps
    // "change the product on step 1 to EQUITY" → ReplaceStepArgs
    // "add a step before step 3" → InsertBefore
    // "skip step 5" → DisableStep
}
```

### Step 4.2: Runbook Mutation Methods

**File:** `rust/src/repl/runbook.rs` (MODIFY)

Add missing methods for editing operations:

```rust
impl Runbook {
    pub fn insert_entry_at(&mut self, index: usize, entry: RunbookEntry) -> Result<()>;
    pub fn replace_entry(&mut self, id: Uuid, entry: RunbookEntry) -> Result<()>;
    pub fn disable_entry(&mut self, id: Uuid) -> Result<()>;
    pub fn enable_entry(&mut self, id: Uuid) -> Result<()>;

    // All mutations log RunbookEvent for audit trail
    fn log_event(&mut self, event: RunbookEvent);
}
```

### Step 4.3: Conversational Clarification via sentences.clarify

**File:** `rust/src/repl/orchestrator_v2.rs` (MODIFY)

When `ArgExtractionOutcome::NeedsClarification` returns missing args, use `VerbSentences.clarify` templates:

```rust
// In Clarifying state handler:
let clarify_prompt = verb_config_index
    .get(&verb)
    .and_then(|v| v.sentences.as_ref())
    .and_then(|s| s.clarify.get(&missing_arg))
    .unwrap_or(&format!("Please provide the {} value", missing_arg));
// → Respond with clarify_prompt, stay in Clarifying state
```

### Step 4.4: Entity Resolution via Conversation

Reuse existing `EntityArgResolver` from `rust/src/repl/resolver.rs` for entity resolution within the v2 orchestrator. When an entity ref is ambiguous, present candidates using conversation (not a picker modal).

### Step 4.5: RunbookEditing State Enrichment

In `RunbookEditing` state:
- `Message` input → check `detect_edit_intent()` first, then fall through to `ProposalEngine`
- `Command(Remove/Reorder)` → direct runbook mutation
- All edits regenerate sentences and re-validate DAG order
- Show updated runbook summary after each edit

### Phase 4 File Inventory

| File | Action | Purpose |
|---|---|---|
| `rust/src/repl/edit_intent.rs` | NEW | Keyword edit detection |
| `rust/src/repl/runbook.rs` | MODIFY | insert_entry_at, replace_entry, disable/enable |
| `rust/src/repl/orchestrator_v2.rs` | MODIFY | Clarification + edit routing in RunbookEditing |
| `rust/src/repl/mod.rs` | MODIFY | Add edit_intent module |
| `rust/tests/repl_v2_editing.rs` | NEW | Edit + clarification tests |

---

## Phase 5 — Durable Execution + DB Persistence + API (Week 7–8)

**Goal:** Real execution modes, DB persistence, full API surface, pack handoff.

### Step 5.1: DB Migration — V2 Tables

**File:** `migrations/070_repl_v2_runbooks.sql` (NEW)

Six new tables with ZERO foreign keys to legacy tables:

```sql
-- Session container
CREATE TABLE "ob-poc".repl_sessions_v2 (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    client_group_id UUID,
    client_group_name TEXT,
    state JSONB NOT NULL,          -- Serialized ReplStateV2
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_active_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Runbook (one per session, replaceable)
CREATE TABLE "ob-poc".repl_runbooks_v2 (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session_id UUID NOT NULL REFERENCES "ob-poc".repl_sessions_v2(id),
    pack_id TEXT,
    pack_version TEXT,
    pack_manifest_hash TEXT,
    template_id TEXT,
    template_hash TEXT,
    status TEXT NOT NULL DEFAULT 'draft',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Runbook entries (ordered steps)
CREATE TABLE "ob-poc".repl_runbook_entries_v2 (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    runbook_id UUID NOT NULL REFERENCES "ob-poc".repl_runbooks_v2(id) ON DELETE CASCADE,
    sequence INTEGER NOT NULL,
    sentence TEXT NOT NULL,
    dsl TEXT NOT NULL,
    verb TEXT NOT NULL,
    args JSONB NOT NULL DEFAULT '{}',
    labels JSONB NOT NULL DEFAULT '{}',
    slot_provenance JSONB NOT NULL DEFAULT '{}',
    arg_extraction_audit JSONB,
    status TEXT NOT NULL DEFAULT 'proposed',
    execution_mode TEXT NOT NULL DEFAULT 'sync',
    confirm_policy TEXT NOT NULL DEFAULT 'always',
    unresolved_refs JSONB NOT NULL DEFAULT '[]',
    depends_on UUID[] NOT NULL DEFAULT '{}',
    result JSONB,
    executed_at TIMESTAMPTZ,
    UNIQUE(runbook_id, sequence)
);

-- Event-sourced audit trail
CREATE TABLE "ob-poc".repl_runbook_events_v2 (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    runbook_id UUID NOT NULL REFERENCES "ob-poc".repl_runbooks_v2(id) ON DELETE CASCADE,
    event_type TEXT NOT NULL,
    payload JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Conversation messages for display
CREATE TABLE "ob-poc".repl_session_messages_v2 (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session_id UUID NOT NULL REFERENCES "ob-poc".repl_sessions_v2(id) ON DELETE CASCADE,
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    metadata JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Journey context (pack answers, progress)
CREATE TABLE "ob-poc".repl_journey_context_v2 (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session_id UUID NOT NULL REFERENCES "ob-poc".repl_sessions_v2(id) ON DELETE CASCADE,
    pack_id TEXT NOT NULL,
    pack_manifest_hash TEXT NOT NULL,
    answers JSONB NOT NULL DEFAULT '{}',
    template_id TEXT,
    progress JSONB NOT NULL DEFAULT '{}',
    handoff_source JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_repl_sessions_v2_active ON "ob-poc".repl_sessions_v2(last_active_at DESC);
CREATE INDEX idx_repl_runbooks_v2_session ON "ob-poc".repl_runbooks_v2(session_id);
CREATE INDEX idx_repl_runbook_entries_v2_runbook ON "ob-poc".repl_runbook_entries_v2(runbook_id, sequence);
```

### Step 5.2: Repository Layer

**File:** `rust/src/repl/repository_v2.rs` (NEW)

```rust
pub struct RunbookRepositoryV2 { pool: PgPool }

impl RunbookRepositoryV2 {
    pub async fn save_session(&self, session: &ReplSessionV2) -> Result<()>;
    pub async fn load_session(&self, id: Uuid) -> Result<Option<ReplSessionV2>>;
    pub async fn save_runbook(&self, runbook: &Runbook) -> Result<()>;
    pub async fn load_runbook(&self, session_id: Uuid) -> Result<Option<Runbook>>;
    pub async fn append_event(&self, runbook_id: Uuid, event: &RunbookEvent) -> Result<()>;
    pub async fn save_message(&self, session_id: Uuid, msg: &ChatMessage) -> Result<()>;
    pub async fn load_messages(&self, session_id: Uuid) -> Result<Vec<ChatMessage>>;
    pub async fn save_journey_context(&self, session_id: Uuid, ctx: &JourneyContext) -> Result<()>;
    pub async fn load_journey_context(&self, session_id: Uuid) -> Result<Option<JourneyContext>>;
}
```

### Step 5.3: Execution Modes

**File:** `rust/src/repl/executor_v2.rs` (NEW)

```rust
pub struct RunbookExecutorV2 {
    dsl_executor: Arc<dyn DslExecutor>,
    dag_analyzer: DagAnalyzer,
    repository: Arc<RunbookRepositoryV2>,
}

impl RunbookExecutorV2 {
    pub async fn execute(&self, runbook: &mut Runbook) -> Result<ExecutionReport> {
        // 1. Validate all entries are Confirmed or Resolved
        // 2. Build DAG via dag_analyzer (reuse existing)
        // 3. Execute in topological order, phase by phase
        // 4. Per entry:
        //    - Sync: execute immediately, record result
        //    - Durable: spawn async task, mark entry as Executing, park runbook
        //    - HumanGate: mark entry as Parked, emit notification, await signal
        // 5. Persist results after each entry
    }

    pub async fn resume(&self, runbook: &mut Runbook, entry_id: Uuid, signal: ResumeSignal) -> Result<()>;
}
```

**DAG adapter:** Convert `RunbookEntry` ↔ `StagedCommand` for `DagAnalyzer` compatibility:

```rust
fn runbook_entry_to_staged_command(entry: &RunbookEntry) -> StagedCommand {
    // Map fields: dsl, verb, args, depends_on
}
```

### Step 5.4: Pack Handoff

**File:** `rust/src/journey/handoff.rs` (MODIFY)

Add handoff execution logic:

```rust
impl PackHandoff {
    pub fn create_from_completed_runbook(
        runbook: &Runbook,
        target_pack_id: &str,
        forward_fields: &[String],
    ) -> Self;

    pub fn apply_to_session(
        &self,
        session: &mut ReplSessionV2,
        pack_router: &PackRouter,
    ) -> Result<()>;
}
```

Forward context (client_group, entity IDs, CBU IDs) and outcomes from completed runbook entries.

### Step 5.5: V2 API Routes (Full Surface)

**File:** `rust/src/api/repl_routes_v2.rs` (MODIFY — expand from Phase 2 stub)

```rust
Router::new()
    .route("/api/repl/v2/session", post(create_session))
    .route("/api/repl/v2/session/:id", get(get_session))
    .route("/api/repl/v2/session/:id/input", post(input))
    .route("/api/repl/v2/session/:id", delete(delete_session))
    .route("/api/repl/v2/session/:id/runbook", get(get_runbook))
    .route("/api/repl/v2/session/:id/runbook/execute", post(execute_runbook))
    .route("/api/repl/v2/session/:id/runbook/entry/:entry_id/resume", post(resume_entry))
    .route("/api/repl/v2/session/:id/handoff", post(initiate_handoff))
    .route("/api/repl/v2/packs", get(list_packs))
    .route("/api/repl/v2/packs/:id", get(get_pack))
```

### Step 5.6: React V2 API Client

**File:** `ob-poc-ui-react/src/api/replV2.ts` (MODIFY — expand from Phase 2 stub)

Full TypeScript client matching all v2 endpoints. Types mirror Rust `ReplResponseV2` / `ReplStateV2` / `UserInputV2`.

### Phase 5 File Inventory

| File | Action | Purpose |
|---|---|---|
| `migrations/070_repl_v2_runbooks.sql` | NEW | 6 new tables |
| `rust/src/repl/repository_v2.rs` | NEW | DB persistence layer |
| `rust/src/repl/executor_v2.rs` | NEW | Phased execution with Sync/Durable/HumanGate |
| `rust/src/journey/handoff.rs` | MODIFY | Handoff execution logic |
| `rust/src/api/repl_routes_v2.rs` | MODIFY | Full API surface |
| `ob-poc-ui-react/src/api/replV2.ts` | MODIFY | Full TypeScript client |
| `rust/src/repl/mod.rs` | MODIFY | Add repository_v2, executor_v2 modules |
| `rust/tests/repl_v2_execution.rs` | NEW | Execution mode tests |
| `rust/tests/repl_v2_persistence.rs` | NEW | DB round-trip tests |

---

## Phase 6 — Decommission Old Paths (Week 9)

**Goal:** Parity verified, old code deleted, feature flags removed.

### Step 6.1: Parity Matrix

Before any deletion, verify ALL acceptance scenarios work on the new path:

| Scenario | Description | Test |
|---|---|---|
| A | Golden loop: scope → pack → Q/A → sentence → confirm → execute | `repl_v2_golden_loop` |
| B | Direct DSL input → sentence generated → runbook entry | `repl_v2_direct_dsl` |
| C | Ambiguous verb → clarification → correct selection | `repl_v2_disambiguation` |
| D | Pack shorthand: "Onboard Allianz Lux" → full pipeline | `repl_v2_scenario_d` |
| E | Runbook editing: remove, reorder, replace args | `repl_v2_editing` |
| F | Pack handoff: Book Setup → Onboarding Request | `repl_v2_handoff` |
| G | Durable execution: park on HumanGate, resume on signal | `repl_v2_durable` |
| H | Force-select: "use onboarding journey" → immediate activation | `repl_v2_force_select` |

### Step 6.2: Delete Legacy Files

| File to Delete | Replaced By |
|---|---|
| `rust/src/repl/types.rs` (old ReplState) | `rust/src/repl/types_v2.rs` |
| `rust/src/repl/orchestrator.rs` (v1) | `rust/src/repl/orchestrator_v2.rs` |
| `rust/src/repl/session.rs` (v1 ReplSession) | `rust/src/repl/session_v2.rs` |
| `rust/src/repl/response.rs` (v1) | `rust/src/repl/response_v2.rs` |
| `rust/src/repl/intent_matcher.rs` | `rust/src/repl/intent_service.rs` |
| `rust/src/mcp/intent_pipeline.rs` | `rust/src/repl/intent_service.rs` |
| `rust/src/session/dsl_sheet.rs` | Already deprecated |
| `rust/src/repl/service.rs` | `rust/src/repl/executor_v2.rs` |

### Step 6.3: Rename V2 Files (Drop Suffix)

| Old Name | New Name |
|---|---|
| `types_v2.rs` | `types.rs` |
| `orchestrator_v2.rs` | `orchestrator.rs` |
| `session_v2.rs` | `session.rs` |
| `response_v2.rs` | `response.rs` |
| `repository_v2.rs` | `repository.rs` |
| `executor_v2.rs` | `executor.rs` |
| `repl_routes_v2.rs` | `repl_routes.rs` |
| `replV2.ts` | `repl.ts` |

### Step 6.4: Remove Feature Flag

- Remove `vnext-repl = []` from `rust/Cargo.toml`
- Remove all `#[cfg(feature = "vnext-repl")]` guards from `rust/src/repl/mod.rs` and `rust/src/lib.rs`
- All v2 modules become the only modules

### Step 6.5: Update Imports and Re-exports

- Update `rust/src/repl/mod.rs` re-exports to point at renamed files
- Update all `use` statements across codebase (imports from repl module)
- Update `rust/src/api/repl_routes.rs` → the renamed v2 routes
- Update React imports from `replV2` → `repl`

### Step 6.6: Clean API Surface

- Remove `/api/repl/session/*` old routes
- Promote `/api/repl/v2/*` to `/api/repl/*`
- Update React to use non-v2 paths

### Step 6.7: Dead Code Scan

```bash
cargo clippy --all-targets -- -W dead_code
cargo test   # All tests pass
```

### Phase 6 File Inventory

| File | Action | Purpose |
|---|---|---|
| 8 files | DELETE | Legacy v1 files (see Step 6.2) |
| 8 files | RENAME | Drop _v2 suffix (see Step 6.3) |
| `rust/Cargo.toml` | MODIFY | Remove vnext-repl feature |
| `rust/src/lib.rs` | MODIFY | Remove cfg guards |
| `rust/src/repl/mod.rs` | MODIFY | Update imports, remove cfg guards |
| `rust/src/api/repl_routes.rs` | MODIFY | Promote v2 routes |
| `ob-poc-ui-react/src/api/repl.ts` | MODIFY | Remove v2 indirection |

---

## Full Dependency Graph (Phases 1–6)

```
Phase 1 (Week 1-2) ─── parallelizable steps:
  ├── Step 1.1: VerbConfigIndex (leaf)
  ├── Step 1.2: Candle BGE routing (depends on SharedEmbedder)
  └── Step 1.3: RealDslExecutor (depends on dsl_v2/executor)
      │
      ▼
  Step 1.4: Wire IntentMatcher (depends on 1.1, 1.2, 1.3)
  Step 1.5: ArgExtractionAudit (depends on 1.4)
  Step 1.6: Sentence templates (depends on 1.1)
  Step 1.7: Integration tests (depends on all above)

Phase 2 (Week 3-4) ─── depends on Phase 1:
  ├── Step 2.1: VerbSentences on VerbConfig (leaf)
  ├── Step 2.2: IntentService (depends on 2.1 + Phase 1 components)
  ├── Step 2.3: Wire IntentService into orchestrator (depends on 2.2)
  ├── Step 2.4: YAML sentence templates (depends on 2.1)
  └── Step 2.5: Deprecation + v2 routes (depends on 2.2)

Phase 3 (Week 5) ─── depends on Phase 2:
  └── ProposalEngine (depends on IntentService from Phase 2)

Phase 4 (Week 6) ─── depends on Phase 3:
  └── EditIntent + Clarification (depends on ProposalEngine)

Phase 5 (Week 7-8) ─── depends on Phase 4:
  ├── DB migration (leaf)
  ├── Repository (depends on migration)
  ├── ExecutorV2 (depends on repository + DagAnalyzer)
  ├── Pack handoff (depends on repository)
  └── API + React (depends on all above)

Phase 6 (Week 9) ─── depends on Phase 5:
  └── Delete + rename + remove flags (depends on parity matrix)
```

---

## Verification (All Phases)

```bash
# After each phase:
cd rust

# All existing tests pass (no regression)
cargo test

# V2 tests pass
cargo test --features vnext-repl

# Clippy clean
cargo clippy --features vnext-repl -- -W clippy::all

# Phase-specific:
# Phase 1: cargo test --features vnext-repl --test repl_v2_integration
# Phase 2: cargo test --features vnext-repl --test repl_v2_intent_service
# Phase 3: cargo test --features vnext-repl --test repl_v2_proposal
# Phase 4: cargo test --features vnext-repl --test repl_v2_editing
# Phase 5: cargo test --features vnext-repl,database --test repl_v2_persistence
# Phase 6: cargo test (no feature flag needed — it's the default)
```

**Acceptance tests across all phases:**

1. **Golden loop** — scope → pack → Q/A → sentence → confirm → execute (Phase 0 ✓, extended each phase)
2. **Template provenance** — template_id, template_hash, slot_provenance correct (Phase 0 ✓)
3. **Pack hash stability** — same file = same hash (Phase 0 ✓)
4. **Sentence quality** — no search_phrase fallback for pack verbs (Phase 1)
5. **Force-select** — "use onboarding journey" works (Phase 0 ✓)
6. **Scenario D** — "Onboard Allianz Lux" → full pipeline (Phase 1)
7. **Parity matrix** — all 8 scenarios pass (Phase 6)

---

## Complete File Inventory (Phases 1–6)

### New Files (14)
| File | Phase | Purpose |
|---|---|---|
| `rust/src/repl/verb_config_index.rs` | 1 | Read-only verb projection |
| `rust/src/repl/executor_bridge.rs` | 1 | Bridge to real DSL pipeline |
| `rust/src/repl/sentence_templates.rs` | 1 | Hardcoded templates (deleted in Phase 2) |
| `rust/tests/repl_v2_integration.rs` | 1 | Phase 1 integration tests |
| `rust/src/repl/intent_service.rs` | 2 | Unified 5-phase pipeline |
| `rust/src/api/repl_routes_v2.rs` | 2 | V2 HTTP routes |
| `ob-poc-ui-react/src/api/replV2.ts` | 2 | V2 TypeScript client |
| `rust/src/repl/proposal.rs` | 3 | Proposal engine |
| `rust/tests/repl_v2_proposal.rs` | 3 | Proposal tests |
| `rust/src/repl/edit_intent.rs` | 4 | Edit detection |
| `rust/tests/repl_v2_editing.rs` | 4 | Edit tests |
| `migrations/070_repl_v2_runbooks.sql` | 5 | 6 DB tables |
| `rust/src/repl/repository_v2.rs` | 5 | DB persistence |
| `rust/src/repl/executor_v2.rs` | 5 | Phased execution |

### Modified Files (across all phases)
| File | Phases | Change |
|---|---|---|
| `rust/src/repl/orchestrator_v2.rs` | 1,2,3,4 | Wire real services progressively |
| `rust/src/repl/sentence_gen.rs` | 1,2 | Sentence template priority chain |
| `rust/src/repl/runbook.rs` | 1,4 | ArgExtractionAudit, edit methods |
| `rust/src/repl/mod.rs` | 1,2,3,4,5 | Module declarations |
| `rust/src/journey/router.rs` | 1 | Candle BGE semantic scoring |
| `rust/src/journey/handoff.rs` | 5 | Handoff execution logic |
| `rust/crates/dsl-core/src/config/types.rs` | 2 | VerbSentences, ConfirmPolicyConfig |
| `rust/config/verbs/*.yaml` (~20 files) | 2 | Add sentences + confirm_policy |
| `rust/src/repl/intent_matcher.rs` | 2 | #[deprecated] |
| `rust/src/mcp/intent_pipeline.rs` | 2 | #[deprecated] |

### Deleted Files (Phase 6)
| File | Replaced By |
|---|---|
| `rust/src/repl/types.rs` | `types_v2.rs` (renamed) |
| `rust/src/repl/orchestrator.rs` | `orchestrator_v2.rs` (renamed) |
| `rust/src/repl/session.rs` | `session_v2.rs` (renamed) |
| `rust/src/repl/response.rs` | `response_v2.rs` (renamed) |
| `rust/src/repl/intent_matcher.rs` | `intent_service.rs` |
| `rust/src/mcp/intent_pipeline.rs` | `intent_service.rs` |
| `rust/src/session/dsl_sheet.rs` | Already deprecated |
| `rust/src/repl/service.rs` | `executor_v2.rs` |

### Renamed Files (Phase 6)
| Old | New |
|---|---|
| `types_v2.rs` | `types.rs` |
| `orchestrator_v2.rs` | `orchestrator.rs` |
| `session_v2.rs` | `session.rs` |
| `response_v2.rs` | `response.rs` |
| `repository_v2.rs` | `repository.rs` |
| `executor_v2.rs` | `executor.rs` |
| `repl_routes_v2.rs` | `repl_routes.rs` |
| `replV2.ts` | `repl.ts` |
