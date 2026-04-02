# Agent & Semantic Pipeline — Detailed Reference

> This document covers the full agent orchestration pipeline: utterance ingress,
> Sage/Coder pattern, Candle embedder, PolicyGate, actor resolution, MCP tools,
> and the onboarding pipeline.
> For the high-level overview see the root `CLAUDE.md`.

---

## Pipeline Overview

```
POST /api/session/:id/input
  ↓
SessionInputRequest dispatch (utterance | discovery_selection | decision_reply | repl_v2)
  ↓
agent_service::process_chat()
  ↓
orchestrator::handle_utterance()
  ├── Stage 1.5  — Sage intent classification
  ├── Stage 2    — SemOsContextEnvelope (CCIR)
  ├── Stage 2.5  — SessionVerbSurface (multi-layer governance)
  ├── Stage 3    — HybridVerbSearcher (10-tier, pre-constrained)
  ├── Stage 4    — Coder: verb resolution + LLM arg extraction
  ├── Stage 5    — DSL assembly + validation
  └── Stage 6    — TOCTOU recheck + execution
  ↓
OrchestratorOutcome → ChatResponse
```

**Key files:**

| File | Purpose |
|------|---------|
| `rust/src/agent/orchestrator.rs` | Unified entry point (~52k lines) |
| `rust/src/api/agent_routes.rs` | `POST /api/session/:id/input` handler |
| `rust/src/sage/llm_sage.rs` | Sage intent classifier |
| `rust/src/sage/coder.rs` | Coder: verb resolution + arg extraction |
| `rust/src/policy/gate.rs` | PolicyGate + ActorResolver |
| `rust/src/agent/learning/embedder.rs` | CandleEmbedder (BGE-small-en-v1.5) |
| `rust/crates/ob-poc-types/src/chat.rs` | ChatResponse types |
| `rust/crates/ob-agentic/src/planner.rs` | Onboarding RequirementPlanner |

---

## Orchestrator

**Entry:** `handle_utterance(ctx: &OrchestratorContext, utterance: &str) -> OrchestratorOutcome`

**OrchestratorContext carries:**
- Actor context (auth, roles, departments)
- Session ID, case ID, dominant entity ID
- Scope context (CBU IDs in session)
- `Arc<HybridVerbSearcher>`
- PolicyGate reference
- Sage engine for intent classification
- AgentMode (Research vs Governed)
- Workflow focus goals + stage focus

**OrchestratorOutcome:**
```rust
pub struct OrchestratorOutcome {
    pub pipeline_result: PipelineResult,          // DSL, valid flag, errors
    pub context_envelope: SemOsContextEnvelope,   // Allowed verbs + fingerprint
    pub surface: SessionVerbSurface,              // Consolidated governance
    pub journey_decision: DecisionPacket,         // For disambiguation
    pub auto_execute: bool,                       // Read-only verbs skip staging
    pub sage_intent: OutcomeIntent,               // Classified intent
    pub trace_id: Uuid,                           // Persisted utterance trace
}
```

---

## Session Input Flow

**File:** `rust/src/api/agent_routes.rs`

```rust
pub enum SessionInputRequest {
    Utterance { message: String },
    DiscoverySelection { selection: DiscoverySelection },
    DecisionReply { packet_id: String, reply: UserReply },
    ReplV2 { input: serde_json::Value },
}

pub enum SessionInputResponse {
    Chat { response: Box<ChatResponse> },
    Decision { response: DecisionReplyResponse },
    ReplV2 { response: serde_json::Value },
}
```

**Routing:**
- `Utterance` → `process_chat()` → `orchestrator.handle_utterance()`
- `DiscoverySelection` → `apply_discovery_selection()` → generates message
- `DecisionReply` → `handle_decision_reply()` → decision_routes
- `ReplV2` → `repl_routes_v2::handle_repl_input()` (behind `vnext-repl` feature)

**Legacy endpoints (410 Gone):** `POST /api/session/:id/chat`, `POST /api/session/:id/decision/reply`, `POST /api/repl/v2/session/:id/input`

---

## Sage / Coder Pattern

### Sage (Intent Classifier)

**File:** `rust/src/sage/llm_sage.rs`

- Input: raw utterance + `SageContext`
- Output: `OutcomeIntent` (plane, polarity, domain, action, params, confidence)
- **Invariant E-SAGE-1:** Sage observes **before** entity linking
- **Invariant E-SAGE-2:** Sage **never sees verb FQNs**

**5-step deterministic pre-classification:**
1. ObservationPlane from session context
2. IntentPolarity from clue words (read vs write)
3. Domain hints from compound signals
4. Action family classification
5. Confidence scoring

Falls back to `DeterministicSage` on LLM failure. System prompt constrains LLM to business outcome identification (NOT function selection).

**Routes:**
- Read intent → `ServeIntent` (can auto-execute)
- Write intent → `DelegateIntent` (stage for confirmation)

### Coder (Action Resolver)

**File:** `rust/src/sage/coder.rs`

- Input: `OutcomeIntent` from Sage (NOT raw utterance)
- Output: `CoderResult` with verb_fqn, DSL, missing_args, unresolved_refs
- **Invariant E-CODER-1:** Coder **never interprets natural language**
- Verb resolution via `StructuredVerbScorer` against allowed verb set
- Missing args extracted via LLM tool calls (`LlmClient::call_tool()`)

### LLM Usage

**Model:** `claude-sonnet-4-20250514` (via `ANTHROPIC_MODEL` env var)
**Endpoint:** `https://api.anthropic.com/v1/messages`
**Headers:** `x-api-key`, `anthropic-version: 2023-06-01`

**LLM is used ONLY for:**
1. Sage intent classification (observation plane, ~200ms)
2. Coder arg extraction (tool calls, ~200-500ms)

**LLM is NEVER used for:** verb selection, verb search, scoring, governance decisions.

**Latency:**
- Verb search (Candle): 5–15ms
- LLM arg extraction: 200–500ms

---

## Candle Embedder

**File:** `rust/src/agent/learning/embedder.rs`

| Property | Value |
|----------|-------|
| Model | `BGE-small-en-v1.5` |
| Dimensions | 384 |
| Engine | Candle (local, no API) |
| Cache | `~/.cache/huggingface/` (~130MB) |
| Speed | 5–15ms per embedding |
| Mode | Asymmetric (queries get instruction prefix) |

```rust
pub async fn embed_query(&self, text: &str) -> Result<Embedding>   // with prefix
pub async fn embed_target(&self, text: &str) -> Result<Embedding>  // no prefix
pub async fn embed_batch_queries(&self, texts: &[&str]) -> Result<Vec<Embedding>>
pub async fn embed_batch_targets(&self, texts: &[&str]) -> Result<Vec<Embedding>>
// Blocking variants also available
```

**`CachedEmbedder`** — read/write cache wrapper to avoid redundant computation.

**Pattern sources:**
- `dsl_verbs.yaml_intent_patterns` — from YAML `invocation_phrases` (overwritten on startup)
- `dsl_verbs.intent_patterns` — learned from user feedback (preserved)
- View `v_verb_intent_patterns` = UNION → `verb_pattern_embeddings` table (pgvector, 384-dim)

**Populate after YAML changes:**
```bash
cargo x verbs compile && \
DATABASE_URL="postgresql:///data_designer" \
  cargo run --release -p ob-semantic-matcher --bin populate_embeddings
```

---

## PolicyGate

**File:** `rust/src/policy/gate.rs`

```rust
pub struct PolicyGate {
    pub strict_single_pipeline: bool,  // OBPOC_STRICT_SINGLE_PIPELINE (default: true)
    pub allow_raw_execute: bool,        // OBPOC_ALLOW_RAW_EXECUTE (default: false)
    pub strict_semreg: bool,            // OBPOC_STRICT_SEMREG (default: true)
    pub allow_legacy_generate: bool,    // OBPOC_ALLOW_LEGACY_GENERATE (default: false)
}
```

**Methods:**
- `can_execute_raw_dsl(actor)` — operator/admin only if `allow_raw_execute=true`
- `can_use_legacy_generate(actor)` — legacy endpoint if enabled + actor is operator/admin
- `semreg_fail_closed()` — returns `strict_semreg` (~30 safe-harbor verbs if true)

Every bypass/privilege decision flows through `PolicyGate`. `SemOsContextEnvelope` replaced flat `SemRegVerbPolicy`.

---

## Actor Resolution

**File:** `rust/src/policy/gate.rs`

**Three entry points:**

| Method | Context | Key Headers/Env | Default Role |
|--------|---------|-----------------|--------------|
| `from_headers(headers)` | HTTP requests | `x-obpoc-actor-id`, `x-obpoc-roles`, `x-obpoc-department`, `x-obpoc-clearance`, `x-obpoc-jurisdictions` | `analyst` |
| `from_env()` | MCP tools | `MCP_ACTOR_ID`, `MCP_ROLES`, `MCP_DEPARTMENT`, `MCP_CLEARANCE`, `MCP_JURISDICTIONS` | `analyst` |
| `from_session_id(id)` | REPL | `REPL_ROLES` env var | `viewer` |

```rust
pub struct ActorContext {
    pub actor_id: String,
    pub roles: Vec<String>,
    pub department: Option<String>,
    pub clearance: Classification,  // Public | Internal | Confidential | Restricted
    pub jurisdictions: Vec<String>,
}
```

---

## ChatResponse

**File:** `rust/crates/ob-poc-types/src/chat.rs`

```rust
pub struct ChatResponse {
    pub message: String,
    pub dsl: Option<DslState>,                         // Single source of truth
    pub session_state: SessionStateEnum,               // New|Scoped|PendingValidation|ReadyToExecute|Executing|Executed|Closed
    pub bindings: Vec<BoundReference>,
    pub confidence: Option<f32>,
    pub journey_decision: Option<DecisionPacket>,      // Disambiguation
    pub pending_mutation: Option<PendingMutation>,     // Confirmation request
    pub verb_surface: Option<SessionVerbSurface>,      // All governance layers
    pub onboarding_state: Option<OnboardingStateView>, // Forward/revert verbs
    pub disambiguation: Option<DisambiguationRequest>,
    pub debug: Option<ChatDebugInfo>,                  // OB_CHAT_DEBUG=1
}
```

**SessionStateEnum:** `New → Scoped → PendingValidation → ReadyToExecute → Executing → Executed → Closed`

**VerbMatchSource variants:**
`UserLearnedExact`, `UserLearnedSemantic`, `LearnedExact`, `GlobalLearned`,
`PatternEmbedding`, `Phonetic`, `Macro`, `LexiconExact`, `LexiconToken`,
`ConstellationIndex`, `MacroIndex`, `ScenarioIndex`, `DirectDsl`

---

## MCP Tools (~102 tools)

**File:** `rust/src/mcp/handlers/core.rs`

**Tool categories:**

| Category | Key Tools |
|----------|-----------|
| Semantic/Discovery | `verb_search`, `dsl_generate`, `intent_feedback`, `session_verb_surface` |
| Session management | `session_load_cbu`, `session_load_jurisdiction`, `session_load_galaxy`, `session_unload_cbu`, `session_clear`, `session_undo`, `session_redo`, `session_info`, `session_list` |
| Execution | `dsl_validate`, `dsl_execute`, `dsl_plan`, `dsl_bind` |
| Entity/Registry | `entity_search`, `entity_get`, `cbu_get`, `cbu_list`, `schema_info`, `db_introspect` |
| Learning/Taxonomy | `learning_import`, `learning_list`, `learning_approve`, `learning_reject`, `teach_phrase`, `unteach_phrase`, `taxonomy_get`, `taxonomy_drill_in` |
| SemReg/Stewardship | `sem_reg.*` tools (~32 total) |

**ToolHandlers config struct:**
```rust
pub struct ToolHandlers {
    pub pool: PgPool,
    pub verb_searcher: Arc<Mutex<Option<HybridVerbSearcher>>>,
    pub learned_data: Option<SharedLearnedData>,
    pub embedder: Option<SharedEmbedder>,
    pub sessions: Option<SessionStore>,
    pub cbu_sessions: Option<CbuSessionStore>,
    pub gateway_client: Arc<Mutex<Option<EntityGatewayClient<Channel>>>>,
}
```

---

## Onboarding Pipeline (ob-agentic)

**Files:** `rust/crates/ob-agentic/src/`

**RequirementPlanner** — deterministic, no AI:
- Input: `OnboardingIntent` (client info, counterparties, instruments, settlements)
- Output: `OnboardingPlan`

```rust
pub struct OnboardingPlan {
    pub pattern: OnboardingPattern,
    pub cbu: CbuPlan,
    pub entities: Vec<EntityPlan>,
    pub universe: Vec<UniverseEntry>,
    pub ssis: Vec<SsiPlan>,
    pub booking_rules: Vec<BookingRulePlan>,
    pub isdas: Vec<IsdaPlan>,
}
```

**Workflow:**
1. User provides onboarding intent (NL or structured)
2. `LexiconAgent` extracts `OnboardingIntent`
3. `RequirementPlanner` deterministically expands to full plan
4. Plan drives DSL generation or step-by-step UI

**AnthropicClient** (`ob-agentic` crate):
- `LlmClient` trait (pluggable backend)
- `AnthropicClient` implementation with tool call support
- Model configurable via `ANTHROPIC_MODEL` env var

---

## Key Invariants

| ID | Rule |
|----|------|
| E-SAGE-1 | Sage observes before entity linking |
| E-SAGE-2 | Sage never sees verb FQNs |
| E-CODER-1 | Coder never interprets natural language |
| PolicyGate | All bypass decisions flow through `PolicyGate` |
| Single Pipeline | `OBPOC_STRICT_SINGLE_PIPELINE=true` — no side doors |
| FailClosed | SemReg unavailable → ~30 safe-harbor verbs only |
| TOCTOU | Envelope recheck before executing stale DSL |
