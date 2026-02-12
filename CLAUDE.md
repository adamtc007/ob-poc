# CLAUDE.md

> **Last reviewed:** 2026-02-12
> **Frontend:** React/TypeScript (`ob-poc-ui-react/`) - Chat UI with scope panel, Inspector
> **Backend:** Rust/Axum (`rust/crates/ob-poc-web/`) - Serves React + REST API
> **Crates:** 18 active Rust crates (esper_* crates deprecated after React migration)
> **Verbs:** 1,083 canonical verbs, 14,593 intent patterns (DB-sourced)
> **Migrations:** 77 schema migrations (+ 072b seed)
> **Schema Overview:** `migrations/OB_POC_SCHEMA_ENTITY_OVERVIEW.md` — living doc, 14 sections, ~185 tables (ob-poc + kyc), 13 mermaid ER diagrams
> **Embeddings:** Candle local (384-dim, BGE-small-en-v1.5) - 14,593 patterns vectorized
> **React Migration (077):** ✅ Complete - egui/WASM replaced with React/TypeScript, 3-panel chat layout
> **Verb Phrase Generation:** ✅ Complete - V1 YAML auto-generates phrases on load (no V2 registry)
> **Navigation:** ✅ Unified - All prompts go through intent matching (view.*/session.* verbs)
> **Multi-CBU Viewport:** ✅ Complete - Scope graph endpoint, execution refresh
> **REPL Session/Phased Execution:** ⚠️ Superseded by V2 REPL Architecture
> **REPL Pipeline Redesign (077):** ⚠️ Superseded by V2 REPL Architecture — V1 types retained for reference
> **V2 REPL Architecture (TODO-2):** ✅ Complete - Pack-scoped intent resolution, 7-state machine, ContextStack fold, preconditions engine, 320 tests
> **Candle Semantic Pipeline:** ✅ Complete - DB source of truth, populate_embeddings binary
> **Agent Pipeline:** ✅ Unified - One path, all input → LLM intent → DSL (no special cases)
> **Solar Navigation (038):** ✅ Complete - ViewState, NavigationHistory, orbit navigation
> **Nav UX Messages:** ✅ Complete - NavReason codes, NavSuggestion, standardized error copy
> **Promotion Pipeline (043):** ✅ Complete - Quality-gated pattern promotion with collision detection
> **Teaching Mechanism (044):** ✅ Complete - Direct phrase→verb mapping for trusted sources
> **Verb Search Test Harness:** ✅ Complete - Full pipeline sweep, safety-first policy, learning accelerator workflow
> **Client Group Resolver (048):** ✅ Complete - Two-stage alias→group→anchor resolution for session scope
> **Workflow Task Queue (049):** ✅ Complete - Async task return path, document entity, requirement guards
> **Transactional Execution (050):** ✅ Complete - Atomic execution, advisory locks, expansion audit
> **CustomOp Auto-Registration (051):** ✅ Complete - `#[register_custom_op]` macro, inventory-based registration
> **Staged Runbook REPL (054):** ✅ Complete - Anti-hallucination staging, entity resolution, DAG ordering
> **Client Group Research Integration (055):** ✅ Complete - GLEIF import → client_group_entity staging → CBU creation with role mapping
> **REPL Viewport Feedback Loop (056):** ✅ Complete - Scope propagation in execute_runbook triggers UI refresh
> **Verb Disambiguation UI (057):** ✅ Complete - Ambiguous verb selection with gold-standard learning signals
> **Unified Architecture (058):** ✅ Complete - Operator vocabulary, macro lint, constraint cascade, DAG navigation, phonetic matching
> **Playbook System (059):** ✅ Complete - YAML playbooks, marked-yaml source mapping, LSP validation, xtask CLI
> **LSP Alignment (060):** ✅ Complete - UTF-16 encoding, multiline context, tree-sitter binding rule, rename handler
> **LSP Test Harness (063):** ✅ Complete - 150+ parser tests, golden file validation, syntax edge cases
> **Macro Vocabulary (063):** ✅ Complete - party.yaml macros, macro/implementation separation documented
> **CBU Structure Macros (064):** ✅ Complete - M1-M18 jurisdiction macros, document bundles, placeholder entities, role cardinality, wizard UI
> **ESPER Navigation Crates (065):** ⚠️ Deprecated - esper_* crates retained for reference, replaced by React UI
> **Unified Lookup Service (074):** ✅ Complete - Verb-first dual search combining verb discovery + entity linking
> **Lexicon Service (072):** ✅ Complete - In-memory verb/domain/concept lookup with bincode snapshots
> **Entity Linking Service (073):** ✅ Complete - In-memory entity resolution with mention extraction, token overlap matching
> **Clarification UX Wiring (075):** ✅ Complete - Unified DecisionPacket system for verb/scope/group disambiguation with confirm tokens
> **Inspector-First Visualization (076):** ✅ Complete - Deterministic tree/table Inspector UI, projection schema with $ref linking, 82 tests
> **Deal Record & Fee Billing (067):** ✅ Complete - Commercial origination hub, rate card negotiation, closed-loop billing
> **BPMN-Lite Runtime (Phase A):** ✅ Complete - Standalone durable orchestration service, 23-opcode VM, gRPC API (verified over the wire), 123 core tests + gRPC smoke test
> **BPMN-Lite Phase 2 (Race/Boundary):** ✅ Complete - Race semantics (Msg vs Timer arms), boundary timer events, interrupting + non-interrupting fire modes
> **BPMN-Lite Phase 2A (Non-Interrupting + Cycles):** ✅ Complete - Non-interrupting boundary spawns child fiber (main stays in Race), ISO 8601 timer cycles (R<n>/PT<dur>), cycle exhaustion revert
> **BPMN-Lite Phase 3 (Cancel/Ghost):** ✅ Complete - Ghost signal protection, WaitCancelled/SignalIgnored events, job purge on cancel, completion ownership in engine
> **BPMN-Lite Phase 5 (Terminate/Error/Loops):** ✅ Complete - EndTerminate (kill all sibling fibers), error boundary routing (BusinessRejection → catch path), bounded retry loops (IncCounter/BrCounterLt)
> **BPMN-Lite Phase 5A (Inclusive Gateway):** ✅ Complete - OR-gateway ForkInclusive (condition_flag evaluation, dynamic branch count), JoinDynamic (join_expected set at fork time)
> **BPMN-Lite Integration (Phase B):** ✅ Complete - ob-poc ↔ bpmn-lite wiring: WorkflowDispatcher (queue-based resilience), JobWorker, EventBridge, SignalRelay, PendingDispatchWorker, correlation stores, 41 unit tests + 13 integration tests + 15 E2E choreography tests
> **BPMN-Lite Phase 4 (PostgresProcessStore):** ✅ Complete - Feature-gated (`postgres`) PostgreSQL-backed ProcessStore, 12 migrations, 29 async methods, `--database-url` CLI arg with MemoryStore fallback, 15 integration tests
> **BPMN-Lite Authoring (Phases B-D):** ✅ Complete - Verb contracts + lint rules (5 rules), BPMN 2.0 XML export + IR↔DTO round-trip, template registry + atomic publish pipeline, PostgresTemplateStore, 123 core tests + 6 integration tests
> **KYC/UBO Skeleton Build Pipeline:** ✅ Complete - 7-step build (import-run → graph validate → UBO compute → coverage → outreach plan → tollgate → complete), real computation in all ops, 12 integration tests with assertions
> **KYC Skeleton Build Post-Audit (S1):** ✅ Complete - Decimal conversion (F-5), coverage ownership scoping (F-2), transaction boundary (F-1), configurable outreach cap (F-4), shared function extraction (F-3a-e)
> **KYC Skeleton Build Post-Audit (S2):** ✅ Complete - Import run case linkage on idempotent hit (F-7), as_of date for import runs (F-8a-c), case status state machine with KycCaseUpdateStatusOp plugin (F-6a-e), 4 transition tests

This is the root project guide for Claude Code. Domain-specific details are in annexes.

---

## Quick Start

```bash
cd rust/

# Pre-commit (fast)
cargo x pre-commit          # Format + clippy + unit tests

# Full check
cargo x check --db          # Include database integration tests

# Deploy (Full stack: React frontend + Rust backend)
cargo x deploy              # Build React + server + start
cargo x deploy --skip-frontend  # Skip React rebuild (backend only)

# Run server directly (serves React from ob-poc-ui-react/dist/)
DATABASE_URL="postgresql:///data_designer" cargo run -p ob-poc-web

# React development (hot reload)
cd ob-poc-ui-react && npm run dev  # Runs on port 5173, proxies API to :3000

# BPMN-Lite service (standalone workspace at bpmn-lite/)
cargo x bpmn-lite build            # Build bpmn-lite workspace
cargo x bpmn-lite build --release  # Release build
cargo x bpmn-lite test             # Run all 71 tests (+ 1 ignored gRPC smoke test)
cargo x bpmn-lite clippy           # Lint
cargo x bpmn-lite start            # Build release + start native (port 50051, MemoryStore)
cargo x bpmn-lite start --database-url postgresql:///data_designer  # Start with PostgresProcessStore
cargo x bpmn-lite stop             # Stop native server
cargo x bpmn-lite status           # Show native + Docker status
cargo x bpmn-lite docker-build     # Build Docker image
cargo x bpmn-lite deploy           # Docker build + compose up

# Schema overview (living doc with mermaid ER diagrams)
# Regenerate PDF after schema changes:
npx md-to-pdf migrations/OB_POC_SCHEMA_ENTITY_OVERVIEW.md
```

---

## React Frontend (ob-poc-ui-react)

> **UI Migration:** The UI has been migrated from egui/WASM to React/TypeScript. The old `ob-poc-ui` and `esper_egui` crates are deprecated.

### Architecture

```
ob-poc-ui-react/
├── src/
│   ├── api/              # API client (chat.ts, scope.ts)
│   ├── features/
│   │   ├── chat/         # Agent chat UI with scope panel
│   │   ├── inspector/    # Projection inspector (tree + detail)
│   │   └── settings/     # App settings
│   ├── stores/           # Zustand state management
│   ├── types/            # TypeScript types
│   └── lib/              # Utilities, query client
├── dist/                 # Production build (served by Rust)
└── package.json
```

### Key Endpoints (Backend → React)

| Endpoint | Purpose |
|----------|---------|
| `POST /api/session` | Create agent session |
| `GET /api/session/:id` | Get session with messages |
| `POST /api/session/:id/chat` | Send chat message |
| `GET /api/session/:id/scope-graph` | Get loaded CBUs (scope) |
| `GET /api/cbu/:id/graph` | Get single CBU's entity graph |
| `GET /api/projections/:id` | Get Inspector projection |

### Development

```bash
cd ob-poc-ui-react

# Install dependencies
npm install

# Development server (hot reload, proxies to :3000)
npm run dev

# Production build
npm run build

# Type check
npm run typecheck

# Lint
npm run lint
```

### Chat Page Layout

```
┌─────────────────────────────────────────────────────────────────────┐
│  [Sessions]  │  [Chat Messages]                    │  [Scope Panel] │
│              │                                     │                │
│  Session 1   │  User: load allianz book            │  Scope (100)   │
│  Session 2   │  Agent: 100 CBUs in scope           │  ├─ Allianz A  │
│  + New       │                                     │  ├─ Allianz B  │
│              │  [Input: Type a message...]         │  └─ ...        │
└─────────────────────────────────────────────────────────────────────┘
```

### Scope Panel

The right-side panel shows loaded CBUs and supports drill-down:
- **CBU List**: Click any CBU to see its entities
- **Entity View**: Shows entities within the CBU (persons, organizations)
- **Auto-refresh**: Updates every 5 seconds to reflect DSL execution

---

## Non-Negotiable Implementation Rules

These rules are **mandatory** for all code changes. No exceptions.

### 1. Type Safety First

**Never use untyped JSON (`serde_json::json!`) for structured data.** Always define typed structs.

```rust
// ❌ WRONG - Untyped, no compile-time guarantees
Ok(ExecutionResult::Record(serde_json::json!({
    "groups_created": row.0,
    "memberships_created": row.1,
})))

// ✅ CORRECT - Typed struct with Serialize/Deserialize
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeriveGroupsResult {
    pub groups_created: i32,
    pub memberships_created: i32,
}

let result = DeriveGroupsResult {
    groups_created: row.0,
    memberships_created: row.1,
};
Ok(ExecutionResult::Record(serde_json::to_value(result)?))
```

**Where to define types:**
- Domain result types → `ob-poc-types` crate (e.g., `manco_group.rs`, `trading_matrix.rs`)
- DSL config types → `dsl-core/src/config/types.rs`
- API response types → Near the route handler or in a shared `types.rs`

**Benefits:**
- Compile-time field name checking
- Refactoring safety (rename field → compiler finds all uses)
- IDE autocomplete and documentation
- Consistent serialization (snake_case via serde)

### 2. Consistent Return Types

YAML verb definitions use `ReturnTypeConfig` enum. Map these to typed Rust structs:

| YAML `returns.type` | Rust Pattern |
|---------------------|--------------|
| `uuid` | `ExecutionResult::Uuid(uuid)` |
| `record` | `ExecutionResult::Record(serde_json::to_value(typed_struct)?)` |
| `record_set` | `ExecutionResult::RecordSet(vec_of_typed_structs.iter().map(serde_json::to_value).collect())` |
| `affected` | `ExecutionResult::Affected(count)` |
| `void` | `ExecutionResult::Void` |

### 3. Option<T> for Nullable/Optional Values

Use `Option<T>` consistently:

```rust
// ✅ CORRECT - Explicit optionality
pub struct ControlChainNode {
    pub entity_id: Uuid,                      // Required
    pub controlled_by_entity_id: Option<Uuid>, // Optional (None for root)
    pub voting_pct: Option<Decimal>,          // Optional
}

// ❌ WRONG - Sentinel values or silent nulls
pub voting_pct: Decimal,  // What does 0.0 mean? Missing or zero?
```

### 4. Error Types over Panics

Use `Result<T, E>` and `?` operator. Never `.unwrap()` in production code paths.

```rust
// ❌ WRONG
let value = map.get("key").unwrap();

// ✅ CORRECT
let value = map.get("key").ok_or_else(|| anyhow!("Missing key"))?;
```

### 5. Re-export Types at Module Boundary

When a module uses types from another crate, re-export them for consumers:

```rust
// In domain_ops/manco_ops.rs
pub use ob_poc_types::manco_group::{
    DeriveGroupsResult, BridgeRolesResult, ControlChainNode, // ...
};
```

---

## Agent Chat `/commands` Help

The agent chat includes a built-in help system. Type `/commands` to see available operations with natural language examples.

**Chat Commands:**
| Command | Description |
|---------|-------------|
| `/commands` | Full command reference with natural language examples |
| `/verbs` | List all DSL verbs |
| `/verbs <domain>` | Verbs for specific domain (e.g., `/verbs kyc`, `/verbs session`) |
| `/help` | Quick help |

**Categories in `/commands`:**

| Section | Content |
|---------|---------|
| **DSL Operations** | `dsl_validate`, `dsl_execute`, `dsl_generate` with NL examples |
| **Session & Navigation** | Load/unload CBUs, undo/redo, scope management |
| **CBU & Entity** | Create, search, roles, products |
| **View & Zoom (ESPER)** | "enhance", "drill", "surface", "follow the money" |
| **KYC & UBO** | Case management, ownership discovery, screening |
| **Trading Profile & Custody** | Instruments, markets, SSIs |
| **Research Macros** | GLEIF lookup, UBO research, approval flow |
| **Learning & Feedback** | Pattern learning, corrections, semantic status |
| **Promotion Pipeline** | Quality-gated auto-promotion with thresholds |
| **Teaching** | Direct phrase→verb mapping for trusted sources |
| **Workflow & Templates** | Workflow instances, template expansion |
| **Batch Operations** | Multi-entity template application |

**Example Natural Language → DSL:**

```
User: "create a fund for Acme Corp"
→ dsl_generate
→ (cbu.create :name "Acme Corp" :type FUND)

User: "load the Allianz book"  
→ (session.load-galaxy :apex-name "Allianz")

User: "who owns this company?"
→ (ubo.discover :entity-id @entity)

User: "teach: 'spin up a fund' = cbu.create"
→ (agent.teach :phrase "spin up a fund" :verb "cbu.create")
```

**Key file:** `rust/src/api/agent_routes.rs` - `generate_commands_help()`

---

## Core Architecture: CBU-Centric Model

**CBU (Client Business Unit) is the atomic unit.** Everything resolves to sets of CBUs.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         SESSION = Set<CBU>                                   │
│                                                                              │
│   Universe (all CBUs)                                                        │
│     └── Book (commercial client's CBUs: Allianz, BlackRock)                 │
│          └── CBU (single trading unit)                                       │
│               ├── TRADING view (default) - instruments, counterparties      │
│               └── UBO view (KYC mode) - ownership/control taxonomy          │
│                    └── Entity drill-down (within UBO context only)          │
│                                                                              │
│   Group structure cross-links CBUs via ownership/control edges              │
│   Clusters/galaxies are DERIVED from these edges, not stored                │
└─────────────────────────────────────────────────────────────────────────────┘
```

### ViewMode (Simplified)

`ViewMode` is a unit struct - always TRADING (CBU view). The old multi-mode enum (`KYC_UBO`, `SERVICE_DELIVERY`, `PRODUCTS_ONLY`, `BOARD_CONTROL`) was removed.

```rust
// Current implementation (graph/mod.rs)
pub struct ViewMode;  // Always "TRADING"
```

For KYC/UBO work, use `view.cbu :mode ubo` which switches to ownership taxonomy within the CBU.

### GraphScope (How Sessions Resolve)

```rust
pub enum GraphScope {
    Empty,                                    // Initial state
    SingleCbu { cbu_id, cbu_name },          // Single CBU focus
    Book { apex_entity_id, apex_name },      // All CBUs under apex
    Jurisdiction { code },                    // All CBUs in jurisdiction
    EntityNeighborhood { entity_id, hops },  // N hops from entity (UBO context)
    Custom { description },                   // Custom filter
}
```

### Astro Scale Levels (Zoom/Layout)

These are **UI zoom levels using CBU and group structures**, not session scope changes:

| Level | What You See | Zoom Action |
|-------|--------------|-------------|
| Universe | All CBUs as dots | Zoom out to see everything |
| Cluster/Galaxy | Segment view (by apex/jurisdiction) | Group CBUs by ownership |
| System | Single CBU expanded | Focus on one CBU |
| Planet | Entity within CBU | Drill into CBU's entities |
| Surface/Core | Deep detail | Max zoom on entity attributes |

---

## Domain Annexes

| When working on... | Read this annex | Contains |
|--------------------|-----------------|----------|
| **Semantic pipeline** | `docs/agent-semantic-pipeline.md` | Candle embeddings, 7-tier search, latency analysis |
| **Agent/MCP pipeline** | `docs/agent-architecture.md` | Intent extraction, MCP tools, learning loop |
| **Session & navigation** | `docs/session-visualization-architecture.md` | Scopes, filters, ESPER verbs, history |
| **Data model (CBU/Entity/UBO)** | `docs/strategy-patterns.md` §1 | Why CBU is a lens, UBO discovery, holdings |
| **Verb authoring** | `docs/verb-definition-spec.md` | YAML structure, valid values, common errors |
| **React frontend** | CLAUDE.md §React Frontend | Chat UI, scope panel, API endpoints |
| **Entity model & schema** | `docs/entity-model-ascii.md` | Full ERD, table relationships |
| **Schema overview (living doc)** | `migrations/OB_POC_SCHEMA_ENTITY_OVERVIEW.md` | ob-poc schema: 14 domain sections, ~150 tables, 10 mermaid ER diagrams, DSL verb domain cross-ref |
| **DSL pipeline** | `docs/dsl-verb-flow.md` | Parser, compiler, executor, plugins |
| **Research workflows** | `docs/research-agent-annex.md` | GLEIF, agent mode, invocation phrases |
| **V2 REPL invariants** | `docs/INVARIANT-VERIFICATION.md` | P-1 through P-5, E-1 through E-8, enforcing code citations |

### AI-Thoughts (Design Decisions)

| Topic | Document | Status |
|-------|----------|--------|
| Group/UBO ownership | `ai-thoughts/019-group-taxonomy-intra-company-ownership.md` | ✅ Done |
| Research workflows | `ai-thoughts/020-research-workflows-external-sources.md` | ✅ Done |
| Source loaders | `ai-thoughts/021-pluggable-research-source-loaders.md` | ✅ Done |
| Event infrastructure | `ai-thoughts/023a-event-infrastructure.md` | ✅ Done |
| Feedback inspector | `ai-thoughts/023b-feedback-inspector.md` | ✅ Done |
| Entity disambiguation | `ai-thoughts/025-entity-disambiguation-ux.md` | ✅ Done |
| Trading matrix pivot | `ai-thoughts/027-trading-matrix-canonical-pivot.md` | ✅ Done |
| Verb governance | `ai-thoughts/028-verb-lexicon-governance.md` | ✅ Done |
| Entity resolution wiring | `ai-thoughts/033-entity-resolution-wiring-plan.md` | ✅ Done |
| REPL state model | `ai-thoughts/034-repl-state-model-dsl-agent-protocol.md` | ✅ Done |
| Session-runsheet-viewport | `ai-thoughts/035-session-runsheet-viewport-integration.md` | ✅ Done |
| Session rip-and-replace | `ai-thoughts/036-session-rip-and-replace.md` | ✅ Done |
| Solar navigation | `ai-thoughts/038-solar-navigation-unified-design.md` | ✅ Done |
| Lexicon service | `ai-thoughts/072-lexicon-service-implementation-plan.md` | ✅ Done |
| Entity linking | `ai-thoughts/073-entity-linking-implementation-plan.md` | ✅ Done |

---

## DSL Pipeline (Single Path)

**ALL DSL generation goes through this pipeline. No bypass paths.**

```
User says: "spin up a fund for Acme"
                    ↓
            verb_search tool
                    ↓
    ┌───────────────┴───────────────┐
    │     Search Priority (6-tier)   │
    │  1. User learned (exact)       │
    │  2. Global learned (exact)     │
    │  3. User semantic (pgvector)   │
    │  4. Global semantic (pgvector) │
    │  5. Blocklist check            │
    │  6. Global semantic fallback   │
    └───────────────┬───────────────┘
                    ↓
            Top match: cbu.create
                    ↓
            dsl_generate tool
                    ↓
    LLM extracts args as JSON (NOT DSL)
                    ↓
    Deterministic DSL assembly
                    ↓
            (cbu.create :name "Acme")
                    ↓
            dsl_execute tool
```

### Why This Matters: LLM Removed from Semantic Loop

**Before Candle:** Verb discovery required LLM call (200-500ms, network, API cost).
**After Candle:** Verb discovery is pure Rust (5-15ms, local, free).

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  VERB DISCOVERY (100% Rust, NO LLM)                                         │
│                                                                              │
│  User: "set up an ISDA"                                                     │
│      ↓                                                                       │
│  Candle embed (local)          5-15ms   ← Pure Rust, no network             │
│      ↓                                                                       │
│  pgvector similarity           <1ms     ← In-memory index                   │
│      ↓                                                                       │
│  Result: isda.create           ────────── Total: 6-16ms                     │
│                                                                              │
├─────────────────────────────────────────────────────────────────────────────┤
│  ARG EXTRACTION (LLM still needed)                                          │
│                                                                              │
│  LLM extracts JSON args        200-500ms  ← Only LLM call in pipeline       │
│      ↓                                                                       │
│  DSL builder (Rust)            1-5ms                                        │
│      ↓                                                                       │
│  (isda.create :counterparty "Goldman" :governing-law "NY")                  │
└─────────────────────────────────────────────────────────────────────────────┘
```

| Metric | Before (OpenAI) | After (Candle) | Improvement |
|--------|-----------------|----------------|-------------|
| Embedding latency | 100-300ms | 5-15ms | **10-20x faster** |
| Network required | Yes | No | **Offline capable** |
| Cost per embed | $0.00002 | $0 | **100% savings** |
| LLM calls per query | 2 (discover + extract) | 1 (extract only) | **50% reduction** |

> **Full details:** `docs/agent-semantic-pipeline.md`

### Intent Pipeline Fixes (042)

> ✅ **IMPLEMENTED (2026-01-21)**: Full correctness fixes for verb search, entity resolution, and ambiguity detection.

**Issues Fixed:**

| Issue | Problem | Fix |
|-------|---------|-----|
| **C** | Unresolved refs lacked metadata | Parse → enrich → canonical walker with `entity_type`, `search_column`, `ref_id` |
| **K** | List/map commit broke (shared span) | Commit by `ref_id` (includes `:list_index` suffix), `dsl_hash` guard |
| **J/D** | LIMIT 1 prevented ambiguity detection | Top-k semantic search + `normalize_candidates()` + `AMBIGUITY_MARGIN = 0.05` |
| **I** | Two competing global sources | Union `agent.invocation_phrases` + `verb_pattern_embeddings`, dedupe by verb |
| **G** | Embedding computed 4x per search | Compute once at top of `search()`, pass through |
| **H** | Hardcoded 0.5 threshold | Added `fallback_threshold` (0.70), `semantic_threshold` (0.78) |

**Search Priority (Updated):**

```
1. User-specific learned (exact) - score 1.0
2. Global learned (exact) - score 1.0
3. User-specific learned (semantic, top-k=3)
4. [REMOVED] - merged into step 6
5. Blocklist filter
6. Global semantic - UNION of learned + cold-start patterns (top-k)
   → normalize_candidates(): dedupe by verb, sort desc, truncate
   → Final blocklist filter across all candidates
```

**Ambiguity Detection:**

```rust
const AMBIGUITY_MARGIN: f32 = 0.05;

pub enum VerbSearchOutcome {
    Matched(VerbSearchResult),           // Clear winner
    Ambiguous { top, runner_up, margin }, // Need clarification
    NoMatch,                              // Below threshold
}

// Pipeline returns NeedsClarification early, does NOT call LLM
```

**Entity Resolution (ref_id for Lists/Maps):**

```rust
// Each list item gets unique ref_id: "stmt_idx:start-end:list_index"
// Commit endpoint uses ref_id, not span, to target exact EntityRef

POST /api/dsl/resolve-by-ref-id
{
  "session_id": "...",
  "ref_id": "0:40-80:0",        // First item in list
  "resolved_key": "uuid-...",
  "dsl_hash": "a1b2c3d4..."     // Optimistic concurrency
}
```

**Key Files:**
- `rust/src/mcp/intent_pipeline.rs` - Pipeline with `IntentArgValue`, fail-early
- `rust/src/mcp/verb_search.rs` - `normalize_candidates()`, top-k, single embed
- `rust/crates/dsl-core/src/ast.rs` - `find_unresolved_ref_locations()` uses stored `ref_id`
- `rust/src/api/agent_routes.rs` - `resolve_by_ref_id` endpoint
- `intent-pipeline-fixes-todo.md` - Full spec with 12-point PR review rubric

### Embeddings: Candle Local (BGE Retrieval Model)

| Component | Value |
|-----------|-------|
| Framework | HuggingFace Candle (pure Rust) |
| Model | BAAI/bge-small-en-v1.5 |
| Dimensions | 384 |
| Latency | 5-15ms |
| Storage | pgvector (IVFFlat) |
| API Key | Not required |
| Mode | Asymmetric retrieval (query→target) |

**BGE vs MiniLM:**
- **BGE** is retrieval-optimized: user queries search against stored verb patterns
- Uses CLS token pooling (not mean pooling)
- Queries prefixed with instruction: `"Represent this sentence for searching relevant passages: "`
- Targets (verb patterns) stored without prefix
- Scores cluster higher (0.6-1.0) than MiniLM (0.3-1.0), thresholds recalibrated

**API Distinction:**
```rust
// User input - searching for verbs
embedder.embed_query("load the allianz book")

// Verb patterns - stored in DB
embedder.embed_target("load galaxy by apex name")
```

### Candle Semantic Pipeline - DB as Source of Truth

> ✅ **IMPLEMENTED (2026-01-19)**: Full pipeline operational with DB-sourced patterns.

**Architecture:**

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    SOURCE OF TRUTH: dsl_verbs.intent_patterns               │
│                                                                              │
│   YAML invocation_phrases                                                    │
│         │                                                                    │
│         ▼                                                                    │
│   VerbSyncService.sync_all_with_phrases() [server startup]                  │
│         │                                                                    │
│         ▼                                                                    │
│   dsl_verbs.intent_patterns (923 verbs, 7334 patterns)                      │
│         │                                                                    │
│         ▼                                                                    │
│   populate_embeddings [binary]                                               │
│   - Reads from v_verb_intent_patterns view                                  │
│   - BGE embeds each pattern (384-dim, embed_target mode)                    │
│   - Inserts to verb_pattern_embeddings with phonetic codes                  │
│         │                                                                    │
│         ▼                                                                    │
│   verb_pattern_embeddings (7500+ patterns with vectors)                     │
│         │                                                                    │
│         ▼                                                                    │
│   HybridVerbSearcher.search_global_semantic()                               │
│   - pgvector cosine similarity                                               │
│   - Returns ranked verb matches                                              │
│                                                                              │
│   Learning loop:                                                             │
│   PatternLearner → add_learned_pattern() → dsl_verbs.intent_patterns        │
│                         ↑                                                    │
│   (Re-run populate_embeddings to pick up new patterns)                      │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Key Files:**

| File | Purpose |
|------|---------|
| `rust/src/database/verb_service.rs` | `VerbService` - centralized verb DB access |
| `rust/src/mcp/verb_search.rs` | `HybridVerbSearcher` - semantic search (uses VerbService) |
| `rust/src/session/verb_sync.rs` | `VerbSyncService` - syncs YAML to DB |
| `rust/crates/ob-semantic-matcher/src/bin/populate_embeddings.rs` | Populates verb_pattern_embeddings |
| `rust/crates/ob-semantic-matcher/src/feedback/learner.rs` | Learning loop |
| `migrations/037_candle_pipeline_complete.sql` | DB schema |

**Database Tables:**

| Table | Purpose | Records |
|-------|---------|---------|
| `ob-poc.dsl_verbs` | Verb definitions + intent_patterns | 923 verbs |
| `ob-poc.verb_pattern_embeddings` | Patterns with Candle embeddings | 7500+ |
| `ob-poc.v_verb_intent_patterns` | View that flattens intent_patterns array | — |
| `agent.user_learned_phrases` | Per-user learned patterns | Runtime |
| `agent.phrase_blocklist` | Blocked verb mappings | Runtime |

**Startup Sync:**

On server startup (`ob-poc-web/src/main.rs`):
1. Load verb YAML config
2. `VerbSyncService.sync_all_with_phrases()` syncs verbs AND invocation_phrases to `dsl_verbs.yaml_intent_patterns`
3. `populate_embeddings` must be run separately to create vectors
4. Background learning task spawns (30s delay, then 6hr interval)

**Populating Embeddings:**

> ⚠️ **CRITICAL:** After adding/modifying verb YAML, you MUST run `populate_embeddings` or the new verbs won't be discoverable via semantic search. Server startup syncs YAML → DB but does NOT create embeddings.

```bash
# Run after YAML changes or first setup
DATABASE_URL="postgresql:///data_designer" \
  cargo run --release --package ob-semantic-matcher --bin populate_embeddings

# Use --force if patterns exist but need re-embedding (e.g., after model change)
DATABASE_URL="postgresql:///data_designer" \
  cargo run --release --package ob-semantic-matcher --bin populate_embeddings -- --force
```

Options:
- `--bootstrap`: Generate patterns for verbs without intent_patterns (use sparingly)
- `--force`: Re-embed all patterns even if already present (use after model change)

**Delta Loading (Fast Incremental):**

`populate_embeddings` uses delta loading by default - only processes patterns WHERE embedding IS NULL. This makes teaching and incremental updates very fast:

```
Full re-embed (7,414 patterns): ~74 seconds
Delta embed (6 new patterns):   ~0.07 seconds
```

After teaching new phrases, just run `populate_embeddings` without `--force` to pick up only the new patterns.

**Search Priority (HybridVerbSearcher):**

All DB access goes through `VerbService` (no direct sqlx calls).

1. User learned exact → `agent.user_learned_phrases`
2. Global learned exact → In-memory LearnedData
3. User semantic → pgvector on user_learned_phrases
4. Global semantic → pgvector on `verb_pattern_embeddings`
5. Blocklist check → Filter blocked verbs
6. **Global semantic fallback** ← Lower threshold, wider net

**Pattern Sources (v_verb_intent_patterns view):**
- `yaml_intent_patterns` - YAML invocation_phrases, overwritten on startup
- `intent_patterns` - Learned patterns, preserved across restarts

**Learning Loop:**

Automatic learning via background task (or MCP tools):
1. `learning_analyze` identifies high-frequency unmatched phrases
2. `learning_apply` promotes candidates to `dsl_verbs.intent_patterns`
3. `populate_embeddings` creates vectors for new patterns
4. Future queries match the learned phrases

**MCP Tools:**
- `learning_analyze` - Find patterns worth learning (days_back, min_occurrences)
- `learning_apply` - Apply a specific pattern → verb mapping
- `embeddings_status` - Check embedding coverage stats

**Coverage Stats:**

```sql
SELECT * FROM "ob-poc".v_verb_embedding_stats;
-- total_verbs: 938
-- verbs_with_patterns: 938
-- verbs_with_yaml_patterns: 935   -- From YAML invocation_phrases
-- verbs_with_learned_patterns: 15 -- From learning loop
-- total_embeddings: 7928
-- unique_verbs_embedded: 977
```

---

## Structured Onboarding Pipeline (ob-agentic)

The `ob-agentic` crate provides a **three-layer pipeline** for converting natural language custody onboarding requests into validated DSL. This is distinct from the single-verb MCP pipeline above—it handles complex multi-entity onboarding workflows.

**Crate:** `rust/crates/ob-agentic/` (no database dependency)

### Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    STRUCTURED ONBOARDING PIPELINE                            │
│                                                                              │
│  User: "Set up a PE fund trading US equities with IRS with Morgan Stanley"  │
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │  LAYER 1: INTENT EXTRACTION (LLM)                                   │    │
│  │  IntentExtractor.extract() → OnboardingIntent                       │    │
│  │                                                                     │    │
│  │  Output: {                                                          │    │
│  │    client: { name: "PE Fund", type: "fund", jurisdiction: "US" },  │    │
│  │    instruments: [EQUITY, OTC_IRS],                                 │    │
│  │    markets: [{ code: XNYS, currencies: [USD] }],                   │    │
│  │    otc_counterparties: [{ name: "Morgan Stanley", law: "NY" }]     │    │
│  │  }                                                                  │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                              │                                               │
│                              ▼                                               │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │  LAYER 2: REQUIREMENT PLANNING (Deterministic Rust)                 │    │
│  │  RequirementPlanner::plan(intent) → OnboardingPlan                  │    │
│  │                                                                     │    │
│  │  • Classify pattern (SimpleEquity / MultiMarket / WithOtc)         │    │
│  │  • Plan CBU creation                                                │    │
│  │  • Plan entity lookups (counterparties)                            │    │
│  │  • Derive universe (instruments × markets × currencies)            │    │
│  │  • Derive SSIs (settlement routes)                                 │    │
│  │  • Derive booking rules (priority-ordered)                         │    │
│  │  • Plan ISDAs + CSAs (if OTC)                                      │    │
│  │                                                                     │    │
│  │  NO LLM - pure business logic expansion                            │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                              │                                               │
│                              ▼                                               │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │  LAYER 3: DSL GENERATION (LLM)                                      │    │
│  │  DslGenerator.generate(plan) → DSL source string                    │    │
│  │                                                                     │    │
│  │  • System prompt: DSL syntax, verb schemas, pattern example        │    │
│  │  • User prompt: structured plan (CBU, entities, universe, rules)   │    │
│  │  • LLM renders plan as s-expression DSL                            │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                              │                                               │
│                              ▼                                               │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │  VALIDATION & RETRY (FeedbackLoop)                                  │    │
│  │  FeedbackLoop.generate_valid_dsl(plan)                              │    │
│  │                                                                     │    │
│  │  • AgentValidator checks syntax                                    │    │
│  │  • If invalid: collect errors → ask LLM to fix                     │    │
│  │  • Retry up to max_retries                                         │    │
│  │  • Return ValidatedDsl { source, attempts, validation }            │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                              │                                               │
│                              ▼                                               │
│                    dsl-core parser → execution                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Key Types

| Type | Module | Purpose |
|------|--------|---------|
| `OnboardingIntent` | `ob_agentic::intent` | Structured NL extraction (client, instruments, markets, counterparties) |
| `IntentResult` | `ob_agentic::intent` | Either `Clear(intent)` or `NeedsClarification(request)` |
| `OnboardingPlan` | `ob_agentic::planner` | Complete requirements (CBU, entities, universe, SSIs, rules, ISDAs) |
| `OnboardingPattern` | `ob_agentic::patterns` | Classification: `SimpleEquity`, `MultiMarket`, `WithOtc` |
| `DslGenerator` | `ob_agentic::generator` | LLM-based DSL rendering from plan |
| `FeedbackLoop` | `ob_agentic::feedback` | Retry loop with error correction |
| `ValidatedDsl` | `ob_agentic::feedback` | Final output with attempt count |

### Onboarding Patterns

| Pattern | Criteria | Complexity |
|---------|----------|------------|
| `SimpleEquity` | Single market, single currency, no OTC | CBU + profile + 1 SSI + basic rules |
| `MultiMarket` | Multiple markets or cross-currency | Multiple SSIs + complex routing rules |
| `WithOtc` | Has OTC counterparties | Adds entities + ISDAs + CSAs + collateral SSI |

### OnboardingPlan Structure

```rust
pub struct OnboardingPlan {
    pub pattern: OnboardingPattern,       // Classification
    pub cbu: CbuPlan,                     // CBU creation spec
    pub entities: Vec<EntityPlan>,        // Counterparty lookups
    pub universe: Vec<UniverseEntry>,     // What client trades
    pub ssis: Vec<SsiPlan>,               // Settlement identifiers  
    pub booking_rules: Vec<BookingRulePlan>, // Routing (priority-ordered)
    pub isdas: Vec<IsdaPlan>,             // OTC agreements
}

// Universe entry: instruments × markets × currencies
pub struct UniverseEntry {
    pub instrument_class: String,         // EQUITY, OTC_IRS
    pub market: Option<String>,           // XNYS, None for OTC
    pub currencies: Vec<String>,          // USD, EUR
    pub settlement_types: Vec<String>,    // DVP, FOP
    pub counterparty_var: Option<String>, // @morgan for OTC
}

// Booking rules derived from universe
pub struct BookingRulePlan {
    pub priority: u32,                    // 10, 15, 50, 100
    pub instrument_class: Option<String>,
    pub market: Option<String>,
    pub currency: Option<String>,
    pub counterparty_var: Option<String>,
    pub ssi_variable: String,             // @ssi-xnys-usd
}
```

### Call Stack

```rust
// 1. Extract intent from NL (async, LLM)
let extractor = IntentExtractor::from_env()?;
let result = extractor.extract("Set up PE fund...").await?;

match result {
    IntentResult::NeedsClarification(req) => {
        // Show req.ambiguity.question to user
        // Call extract_with_clarification() with their choice
    }
    IntentResult::Clear(intent) => {
        // 2. Plan requirements (sync, deterministic)
        let plan = RequirementPlanner::plan(&intent);
        
        // 3. Generate validated DSL (async, LLM with retry)
        let feedback = FeedbackLoop::from_env(3)?;
        let validated = feedback.generate_valid_dsl(&plan).await?;
        
        // validated.source contains DSL ready for dsl-core
    }
}
```

### Lexicon Pipeline (Alternative Path)

The `ob_agentic::lexicon` module provides a **formal grammar** approach as an alternative to LLM-based intent extraction:

```
User Input → Tokenizer (lexicon + EntityGateway) → Nom Parser → IntentAst → Plan → DSL
```

| Module | Purpose |
|--------|---------|
| `lexicon::Tokenizer` | Classifies words against YAML lexicon + DB entities |
| `lexicon::parse_tokens` | Nom grammar parser → `IntentAst` |
| `lexicon::intent_to_plan` | AST → `Plan` with `SemanticAction` |
| `lexicon::render_plan` | Plan → DSL source string |

This path is deterministic end-to-end (no LLM) but requires the lexicon to cover the input vocabulary.

### Key Files

| File | Purpose |
|------|---------|
| `rust/crates/ob-agentic/src/intent.rs` | `OnboardingIntent`, `IntentResult`, `ClarificationRequest` |
| `rust/crates/ob-agentic/src/planner.rs` | `RequirementPlanner::plan()`, `OnboardingPlan` |
| `rust/crates/ob-agentic/src/generator.rs` | `DslGenerator`, `IntentExtractor` |
| `rust/crates/ob-agentic/src/feedback.rs` | `FeedbackLoop`, `ValidatedDsl` |
| `rust/crates/ob-agentic/src/patterns.rs` | `OnboardingPattern` enum |
| `rust/crates/ob-agentic/src/validator.rs` | `AgentValidator` for DSL syntax |
| `rust/crates/ob-agentic/src/lexicon/` | Formal grammar tokenizer + parser |
| `rust/crates/ob-agentic/src/schemas/` | Verb schemas + reference data for prompts |

### When to Use Which Pipeline

| Pipeline | Use Case |
|----------|----------|
| **MCP Pipeline** (verb_search → dsl_generate) | Single-verb commands, navigation, CRUD operations |
| **ob-agentic Pipeline** (Intent → Plan → DSL) | Complex custody onboarding with multiple entities, SSIs, booking rules |
| **Lexicon Pipeline** | Deterministic parsing when vocabulary is constrained |

---

## Verb Phrase Generation (Auto-Generated)

> ✅ **IMPLEMENTED (2026-02-05)**: Invocation phrases auto-generated on V1 YAML load. No separate V2 registry needed.

The verb loader automatically generates invocation phrases for all verbs without manual curation. This enables semantic search discovery for new verbs immediately after adding them to YAML.

### Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    V1 VERB LOADING WITH AUTO-GENERATED PHRASES               │
│                                                                              │
│  V1 YAML (config/verbs/*.yaml)                                              │
│         │   - Full verb definitions (behavior, handlers, CRUD, lifecycle)   │
│         │   - Optional manual invocation_phrases (preserved)                │
│         │                                                                    │
│         ▼                                                                    │
│  ConfigLoader.load_verbs()                                                  │
│         │   - Loads verb definitions                                        │
│         │   - Calls enrich_with_generated_phrases()                         │
│         │   - Auto-generates phrases from domain + action synonyms          │
│         │                                                                    │
│         ▼                                                                    │
│  Server startup (ob-poc-web)                                                │
│         │   - VerbSyncService.sync_all_with_phrases()                       │
│         │   - Syncs to dsl_verbs.yaml_intent_patterns                       │
│         │                                                                    │
│         ▼                                                                    │
│  populate_embeddings                                                        │
│         │   - Delta loading (only new patterns)                             │
│         │                                                                    │
│         ▼                                                                    │
│  HybridVerbSearcher (semantic search ready)                                 │
└─────────────────────────────────────────────────────────────────────────────┘
```

### How It Works

When verbs are loaded from YAML, the `ConfigLoader` automatically generates invocation phrases by combining:
1. **Verb action synonyms** (create → add, new, make, register)
2. **Domain nouns** (cbu → client business unit, trading unit)

```rust
// dsl-core/src/config/phrase_gen.rs
generate_phrases("cbu", "create", &[])
// Returns: ["create cbu", "add cbu", "new client business unit", ...]

generate_phrases("deal", "create", &[])
// Returns: ["create deal", "add deal record", "new client deal", ...]
```

### Key Benefits

- **No manual step required** - New verbs are discoverable immediately
- **Single source of truth** - V1 YAML is the only verb format
- **Existing phrases preserved** - Manual phrases merged with generated
- **No build artifacts** - No registry.json to rebuild

### Adding New Verbs

Simply add the verb to V1 YAML and restart the server:

```yaml
# config/verbs/deal.yaml
domains:
  deal:
    verbs:
      create:
        description: "Create a new deal record"
        behavior: plugin
        # invocation_phrases auto-generated: ["create deal", "add deal", ...]
```

Then run embeddings to enable semantic search:

```bash
DATABASE_URL="postgresql:///data_designer" \
  cargo run --release --package ob-semantic-matcher --bin populate_embeddings
```

### Phrase Synonym Dictionaries

| Action | Synonyms |
|--------|----------|
| create | add, new, make, register |
| list | show, get all, display, enumerate |
| get | show, fetch, retrieve, read |
| update | edit, modify, change, set |
| delete | remove, drop, terminate |
| record | log, capture, enter |
| generate | create, produce, build |

| Domain | Nouns |
|--------|-------|
| cbu | client business unit, trading unit |
| entity | company, person |
| deal | deal record, client deal, sales deal |
| billing | billing profile, fee billing, invoice |

### Key Files

| File | Purpose |
|------|---------|
| `rust/crates/dsl-core/src/config/phrase_gen.rs` | Phrase generation with synonym dictionaries |
| `rust/crates/dsl-core/src/config/loader.rs` | `enrich_with_generated_phrases()` on load |
| `rust/src/session/verb_sync.rs` | `sync_all_with_phrases()` to DB |
| `rust/crates/ob-poc-web/src/main.rs` | Server startup verb sync |

---

## Promotion Pipeline (Staged Pattern Learning)

The **PromotionService** provides quality-gated pattern promotion with collision detection. Runs as part of the background learning task (every 6 hours).

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    PROMOTION PIPELINE                                        │
│                                                                              │
│  User interaction → outcome recorded                                        │
│      │                                                                       │
│      ▼                                                                       │
│  FeedbackService.record_outcome_with_dsl()                                  │
│      │                                                                       │
│      └── Triggers agent.record_learning_signal() for strong signals         │
│              │                                                               │
│              ▼                                                               │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │  QUALITY GATES (SQL function)                                        │    │
│  │  • Word count: 3-15 words                                            │    │
│  │  • Stopword ratio: <70%                                              │    │
│  │  • Not already in verb_pattern_embeddings                           │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│              │                                                               │
│              ▼                                                               │
│  agent.learning_candidates (tracks success_count, total_count)              │
│              │                                                               │
│              │ (background job every 6 hours)                               │
│              ▼                                                               │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │  PROMOTION THRESHOLDS                                                │    │
│  │  • occurrence_count >= 5                                             │    │
│  │  • success_rate >= 0.80                                              │    │
│  │  • age >= 24 hours (cool-down)                                       │    │
│  │  • collision_safe = true (semantic check)                            │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│              │                                                               │
│              ▼                                                               │
│  agent.apply_promotion() → dsl_verbs.intent_patterns                        │
│              │                                                               │
│              ▼                                                               │
│  (Run populate_embeddings to enable semantic matching)                      │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Key Files:**

| File | Purpose |
|------|---------|
| `rust/crates/ob-semantic-matcher/src/feedback/promotion.rs` | PromotionService |
| `rust/crates/ob-semantic-matcher/src/feedback/service.rs` | FeedbackService (triggers learning signals) |
| `rust/src/agent/learning/background.rs` | Background job integration |
| `migrations/043_feedback_loop_promotion.sql` | Schema, functions, views |

**Database Tables:**

| Table | Purpose |
|-------|---------|
| `agent.learning_candidates` | Staged candidates with success tracking |
| `agent.stopwords` | Words filtered from patterns |
| `agent.phrase_blocklist` | Rejected patterns |
| `agent.learning_audit` | Audit trail of promotions/rejections |

**Views:**

| View | Purpose |
|------|---------|
| `agent.v_top_pending_candidates` | Top 100 pending by occurrence |
| `agent.v_candidate_pipeline` | Pipeline status summary |
| `agent.v_learning_health_weekly` | Weekly health metrics |

**MCP Tools:**

| Tool | Description |
|------|-------------|
| `promotion_run_cycle` | Run full promotion pipeline manually |
| `promotion_candidates` | List candidates ready for auto-promotion |
| `promotion_review_queue` | List candidates needing manual review |
| `promotion_approve` | Manually approve a candidate |
| `promotion_reject` | Reject and add to blocklist |
| `promotion_health` | Weekly health metrics |
| `promotion_pipeline_status` | Pipeline summary by status |

**Thresholds:**

| Parameter | Value | Rationale |
|-----------|-------|-----------|
| `min_occurrences` | 5 | Enough signal, not one-off |
| `min_success_rate` | 0.80 | 4/5 successful uses |
| `min_age_hours` | 24 | Cool-down for burst patterns |
| `collision_threshold` | 0.92 | Prevent verb confusion |

---

## Teaching Mechanism (Direct Pattern Learning)

The **Teaching Tools** allow trusted sources (admin, Claude) to directly add phrase→verb mappings without going through the candidate promotion pipeline. This bypasses quality gates for immediate effect.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    TEACHING vs PROMOTION                                     │
│                                                                              │
│  TEACHING (trusted, immediate)           PROMOTION (earned, gated)          │
│  ─────────────────────────────           ──────────────────────────         │
│  teach_phrase("spin up", "cbu.create")   User interactions → signals        │
│      │                                       │                               │
│      ▼                                       ▼                               │
│  agent.teach_phrase() SQL function       learning_candidates (staged)       │
│      │                                       │                               │
│      ├── Validates verb exists               ├── Quality gates              │
│      ├── Normalizes phrase                   ├── Success rate >= 80%        │
│      ├── Adds to intent_patterns             ├── Age >= 24 hours            │
│      └── Audits to teaching_audit            └── Collision check            │
│      │                                       │                               │
│      ▼                                       ▼                               │
│  dsl_verbs.intent_patterns              dsl_verbs.intent_patterns           │
│      │                                       │                               │
│      └──────────────┬────────────────────────┘                              │
│                     ▼                                                        │
│           populate_embeddings                                                │
│                     │                                                        │
│                     ▼                                                        │
│           verb_pattern_embeddings (semantic search enabled)                  │
└─────────────────────────────────────────────────────────────────────────────┘
```

**MCP Tools:**

| Tool | Description |
|------|-------------|
| `teach_phrase` | Directly add phrase→verb mapping |
| `unteach_phrase` | Remove a taught mapping (with audit) |
| `teaching_status` | View recently taught patterns and stats |

**Usage Examples:**

```json
// Teach a new phrase
{"tool": "teach_phrase", "phrase": "spin up a fund", "verb": "cbu.create"}

// Teach with custom source
{"tool": "teach_phrase", "phrase": "who owns this", "verb": "ubo.discover", "source": "admin_manual"}

// Remove a taught phrase
{"tool": "unteach_phrase", "phrase": "spin up a fund", "reason": "too_generic"}

// Check teaching status
{"tool": "teaching_status", "limit": 20, "include_stats": true}
```

**Key Files:**

| File | Purpose |
|------|---------|
| `migrations/044_agent_teaching.sql` | Schema, functions, views |
| `rust/src/mcp/handlers/core.rs` | MCP tool handlers |
| `rust/src/mcp/tools.rs` | Tool definitions |

**Database Functions:**

| Function | Purpose |
|----------|---------|
| `agent.teach_phrase(phrase, verb, source)` | Add pattern with validation |
| `agent.teach_phrases_batch(json_array)` | Bulk teaching |
| `agent.unteach_phrase(phrase, verb, reason)` | Remove with audit |
| `agent.get_taught_pending_embeddings()` | Patterns needing embedding |

**Views:**

| View | Purpose |
|------|---------|
| `agent.v_recently_taught` | Recent patterns with source |
| `agent.v_teaching_stats` | Totals, today, this week |

**After Teaching:**

Always run `populate_embeddings` after teaching to enable semantic matching:

```bash
DATABASE_URL="postgresql:///data_designer" \
  cargo run --release --package ob-semantic-matcher --bin populate_embeddings
```

---

## Verb Disambiguation UI (057)

> ✅ **IMPLEMENTED (2026-01-27)**: Interactive disambiguation when verb search returns multiple high-confidence matches.

When the semantic verb search returns multiple verbs with similar scores (within `AMBIGUITY_MARGIN = 0.05`), the system presents an interactive disambiguation UI instead of guessing.

**Flow:**

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  User: "load the book"                                                      │
│      │                                                                       │
│      ▼                                                                       │
│  HybridVerbSearcher.search() → VerbSearchOutcome::Suggest                   │
│      │   (top: session.load-galaxy @ 0.82, runner_up: session.load-cbu @ 0.79)
│      │                                                                       │
│      ▼                                                                       │
│  ChatResponse.verb_disambiguation = Some(VerbDisambiguationRequest {        │
│      original_input: "load the book",                                       │
│      options: [session.load-galaxy, session.load-cbu, ...]                  │
│  })                                                                          │
│      │                                                                       │
│      ▼                                                                       │
│  UI shows disambiguation card with clickable verb buttons                   │
│      │                                                                       │
│      ├─► User clicks verb → POST /select-verb                               │
│      │       → Gold-standard learning signal (confidence=0.95)              │
│      │       → Phrase variants generated for robust learning                │
│      │       → Continue with selected verb                                  │
│      │                                                                       │
│      └─► User cancels/times out (30s) → POST /abandon-disambiguation        │
│              → Negative signals for all candidates                          │
│              → Clear disambiguation state                                   │
└─────────────────────────────────────────────────────────────────────────────┘
```

**API Endpoints:**

| Endpoint | Purpose |
|----------|---------|
| `POST /api/session/:id/select-verb` | User selected a verb from disambiguation |
| `POST /api/session/:id/abandon-disambiguation` | User cancelled or timed out |

**Types (`ob-poc-types`):**

```rust
pub struct VerbDisambiguationRequest {
    pub original_input: String,
    pub options: Vec<VerbOption>,
}

pub struct VerbOption {
    pub verb_fqn: String,      // e.g., "session.load-galaxy"
    pub description: String,
    pub score: f32,
    pub example: Option<String>,
}

pub struct VerbSelectionRequest {
    pub selected_verb: String,
    pub original_input: String,
    pub all_candidates: Vec<VerbCandidate>,
}

pub enum AbandonReason {
    Cancelled,      // User clicked cancel
    Timeout,        // 30-second timeout
    NewInput,       // User started typing new input
}
```

**UI State (`VerbDisambiguationState`):**

```rust
pub struct VerbDisambiguationState {
    pub active: bool,
    pub request: Option<VerbDisambiguationRequest>,
    pub original_input: String,
    pub shown_at: Option<f64>,  // Timestamp for timeout
    pub loading: bool,
}
```

**Key Files:**

| File | Purpose |
|------|---------|
| `rust/crates/ob-poc-types/src/lib.rs` | Type definitions |
| `rust/src/api/agent_routes.rs` | `/select-verb`, `/abandon-disambiguation` endpoints |
| `rust/src/api/agent_service.rs` | `handle_verb_selection()`, wires to ChatResponse |
> **Note:** The egui UI for disambiguation (`ob-poc-ui`) has been removed. Disambiguation is now handled by the React frontend and V2 REPL API.

**Learning Signal Generation:**

When user selects a verb:
1. **Gold-standard signal** (confidence=0.95) for selected phrase→verb
2. **Phrase variants** generated via `generate_phrase_variants()`:
   - Original phrase
   - Lowercase normalized
   - Common prefix variations ("please", "can you", "I want to")
3. **Negative signals** for rejected candidates

---

## Intent Tier Disambiguation (065)

> ✅ **IMPLEMENTED (2026-02-01)**: Higher-level intent clarification before verb disambiguation.

**Problem Solved:** Verb disambiguation showed low-level technical verb options (e.g., `session.load-galaxy`, `session.load-cbu`, `cbu.create`) which overwhelmed users. They need to first answer a simpler question: "What are you trying to do?"

**Key Insight:** Intent tier disambiguation happens **BEFORE** verb disambiguation. It's a funnel:

```
User Input → Intent Tier (action type) → Verb Disambiguation (specific verb) → Entity Resolution → Execute
```

### Two-Tier Clarification Model

| Tier | Question | Example Options |
|------|----------|-----------------|
| **Tier 1** | "What are you trying to do?" | Navigate, Create, Modify, Analyze, Workflow |
| **Tier 2** | "What kind of [action]?" | (for Navigate): Single Structure, Client Book, Jurisdiction |

### Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  INTENT TIER DISAMBIGUATION                                                  │
│                                                                              │
│  User: "load the book"                                                       │
│      │                                                                       │
│      ▼                                                                       │
│  HybridVerbSearcher.search() → Multiple verbs across different tiers        │
│      │   session.load-galaxy (navigate), cbu.create (create), ...           │
│      │                                                                       │
│      ▼                                                                       │
│  IntentTierTaxonomy.analyze_candidates()                                    │
│      │   Detects verbs span multiple action categories                      │
│      │   navigate: 3 verbs, create: 2 verbs, modify: 1 verb                 │
│      │                                                                       │
│      ▼                                                                       │
│  should_use_tiers() = true (multiple tiers with candidates)                 │
│      │                                                                       │
│      ▼                                                                       │
│  ChatResponse.intent_tier = Some(IntentTierRequest {                        │
│      tier_number: 1,                                                        │
│      prompt: "What are you trying to do?",                                  │
│      options: [Navigate, Create, Modify, ...]                               │
│  })                                                                          │
│      │                                                                       │
│      ▼                                                                       │
│  UI shows intent tier card with high-level action buttons                   │
│      │                                                                       │
│      ├─► User selects "Navigate & View"                                     │
│      │       → Filter to navigate verbs only                                │
│      │       → If 1 verb remains: proceed to execute                        │
│      │       → If multiple remain: show Tier 2 or verb disambiguation       │
│      │                                                                       │
│      └─► User cancels → Clear state, no action                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Taxonomy Configuration

Defined in `config/verb_schemas/intent_tiers.yaml`:

```yaml
tiers:
  tier1:
    navigate:
      id: navigate
      label: "Navigate & View"
      description: "Load, view, or explore structures and data"
      hint: "session.load-*, view.*, session.info"
    create:
      id: create
      label: "Create New"
      description: "Create new structures, entities, or relationships"
      hint: "*.create, *.add, *.register"
    modify:
      id: modify
      label: "Modify Existing"
      description: "Update, edit, or change existing data"
      hint: "*.update, *.set, *.assign"
    analyze:
      id: analyze
      label: "Analyze & Report"
      description: "Run analysis, generate reports, or query data"
      hint: "*.analyze, *.report, *.query"
    workflow:
      id: workflow
      label: "Workflow & Process"
      description: "Manage cases, approvals, or multi-step processes"
      hint: "kyc.*, workflow.*, case.*"

  tier2:
    navigate:
      single_structure:
        label: "Single Structure"
        description: "Focus on one CBU/fund"
      client_book:
        label: "Client Book"
        description: "All structures for a client"
      jurisdiction:
        label: "By Jurisdiction"
        description: "Structures in a region"

verb_tier_mapping:
  session.load-galaxy: navigate
  session.load-cbu: navigate
  session.load-jurisdiction: navigate
  cbu.create: create
  entity.create: create
  # ... etc
```

### Types (`ob-poc-types`)

```rust
/// Request for intent tier selection (before verb disambiguation)
pub struct IntentTierRequest {
    pub request_id: String,
    pub tier_number: u32,           // 1 or 2
    pub original_input: String,
    pub options: Vec<IntentTierOption>,
    pub prompt: String,             // "What are you trying to do?"
    pub selected_path: Vec<IntentTierSelection>,  // Previous selections
}

pub struct IntentTierOption {
    pub id: String,         // "navigate", "create", etc.
    pub label: String,      // "Navigate & View"
    pub description: String,
    pub hint: Option<String>,
    pub verb_count: usize,  // How many verbs in this category
}

pub struct IntentTierSelection {
    pub tier: u32,
    pub selected_id: String,
}
```

### Key Files

| File | Purpose |
|------|---------|
| `config/verb_schemas/intent_tiers.yaml` | Tier definitions and verb→tier mapping |
| `rust/src/dsl_v2/intent_tiers.rs` | `IntentTierTaxonomy` loader and analysis |
| `rust/src/api/agent_service.rs` | Integration in `process_chat()`, builds tier requests |
| `rust/crates/ob-poc-types/src/lib.rs` | `IntentTierRequest`, `IntentTierOption` types |

> **Note:** The egui UI for intent tiers (`ob-poc-ui`) has been removed. Intent tier selection is now handled by the React frontend and V2 REPL API.

### Decision Flow in agent_service.rs

```rust
// In process_chat():
// 1. Run verb search
let search_result = verb_searcher.search(&input).await?;

// 2. Check if we need intent tier disambiguation FIRST
if let VerbSearchOutcome::Ambiguous { candidates, .. } = &search_result {
    let tier_analysis = taxonomy.analyze_candidates(candidates);
    
    if taxonomy.should_use_tiers(&tier_analysis) {
        // Multiple action categories detected - ask high-level question first
        let tier_request = taxonomy.build_tier1_request(&input, &tier_analysis);
        return Ok(AgentChatResponse {
            intent_tier: Some(tier_request),
            verb_disambiguation: None,  // Don't show verb options yet
            ..
        });
    }
}

// 3. If single tier or user already selected tier, proceed to verb disambiguation
```

### Why This Matters

| Without Intent Tiers | With Intent Tiers |
|---------------------|-------------------|
| "load the book" → 8 verb buttons | "load the book" → "What are you trying to do?" |
| User sees: `session.load-galaxy`, `session.load-cbu`, `cbu.create`, ... | User sees: Navigate, Create, Modify |
| Cognitive overload, technical jargon | Simple, action-oriented choices |
| User may pick wrong verb | Guided to correct category first |

### Learning from Selections

User tier selections generate **gold-standard training data**:

1. Input phrase + selected tier → train intent classifier
2. If tier leads to single verb → implicit phrase→verb mapping
3. Aggregate tier selection patterns → improve taxonomy weights

---

## Operator Macro Vocabulary (058)

> ✅ **IMPLEMENTED (2026-01-28)**: Business-friendly vocabulary layer over technical DSL.

The Operator Macro system provides business-friendly terms (structure, case, mandate) that map to technical DSL verbs (cbu, kyc-case, trading-profile). This enables the UI verb picker to show operator-friendly labels.

### Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  OPERATOR VOCABULARY LAYER                                                   │
│                                                                              │
│  User types: "set up structure"                                             │
│      │                                                                       │
│      ▼                                                                       │
│  HybridVerbSearcher.search() - Tier 0: Macro Search (HIGHEST PRIORITY)      │
│      │                                                                       │
│      ├─► Exact FQN match (structure.setup) → score 1.0                      │
│      ├─► Exact label match ("Set up Structure") → score 1.0                 │
│      └─► Fuzzy label/desc match → score 0.95                                │
│      │                                                                       │
│      ▼                                                                       │
│  Macro found → Expands to DSL: (session.set-structure :structure-id ...)    │
│                                                                              │
│  Display Noun Translation:                                                   │
│  - cbu → structure                                                          │
│  - kyc-case → case                                                          │
│  - trading-profile → mandate                                                │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Components

| Component | File | Purpose |
|-----------|------|---------|
| Macro Definitions | `rust/src/macros/definition.rs` | Type definitions for operator macros |
| Macro Registry | `rust/src/macros/registry.rs` | Load/index macros from YAML, build taxonomy |
| Display Nouns | `rust/src/api/display_nouns.rs` | Translate internal→operator vocabulary |
| Verb Search Integration | `rust/src/mcp/verb_search.rs` | Tier 0 macro search in `HybridVerbSearcher` |

### Session Context Verbs

New primitive verbs for macro expansion targets:

| Verb | Purpose | Handler |
|------|---------|---------|
| `session.set-structure` | Set current CBU context | `SessionSetStructureOp` |
| `session.set-case` | Set current KYC case context | `SessionSetCaseOp` |
| `session.set-mandate` | Set current trading profile context | `SessionSetMandateOp` |

### API Endpoints

| Endpoint | Purpose |
|----------|---------|
| `GET /api/verbs/taxonomy` | Hierarchical macro taxonomy for UI verb picker |
| `GET /api/verbs/:fqn/schema` | Full macro definition (args, prereqs, expansion) |

### Verb Search Priority (Updated)

```
0. Operator macros (business vocabulary) - score 1.0 exact, 0.95 fuzzy  ← NEW
1. User-specific learned (exact) - score 1.0
2. Global learned (exact) - score 1.0
3. User-specific learned (semantic) - score 0.8-0.99
4. [REMOVED]
5. Blocklist filter
6. Global semantic (cold start) - score 0.55-0.95
7. Phonetic fallback (typo handling)
```

### Display Noun Mapping

| Internal Term | Operator Term |
|---------------|---------------|
| `cbu` | `structure` |
| `cbu_id` | `structure_id` |
| `kyc-case` | `case` |
| `kyc_case_id` | `case_id` |
| `trading-profile` | `mandate` |
| `trading_profile_id` | `mandate_id` |

### Macro Lint Rules

Key lint rules enforced by `cargo x verbs lint-macros`:

| Rule | Severity | Description |
|------|----------|-------------|
| `MACRO011` | Error | UI fields required (label, description, target_label) |
| `MACRO012` | Error | Forbidden tokens in UI (cbu, entity_ref, etc.) |
| `MACRO042` | Error | No raw `entity_ref` - use `structure_ref`, `party_ref` |
| `MACRO043` | Error | `kinds:` only under `internal:` block |
| `MACRO044` | Error | Enums must use `${arg.X.internal}` in expansion |
| `MACRO063` | Error | Variable grammar validation (only `${arg.*}`, `${scope.*}`, `${session.*}`) |
| `MACRO080a-c` | Warning | UX friction (missing autofill, picker, short description) |

### Operator Enum Keys vs Internal Tokens

UI keys are not the same as internal tokens - expansion MUST use `${arg.X.internal}`:

| UI Key | Label (user sees) | Internal Token |
|--------|-------------------|----------------|
| `pe` | "Private Equity" | `private-equity` |
| `sicav` | "SICAV" | `sicav` |
| `gp` | "General Partner" | `general-partner` |
| `manco` | "Management Company" | `manco` |

---

## Legal Contracts & Onboarding Gate (045)

> ✅ **IMPLEMENTED (2026-01-22)**: Legal contracts with product-level rate cards and CBU subscription gate.

**Data Model:**

```
┌─────────────────────────────────────────────────────────────────────┐
│                     legal_contracts                                  │
│  contract_id, client_label, effective_date, status                  │
└─────────────────────────────────────────────────────────────────────┘
                              │
                              │ 1:N
                              ▼
┌─────────────────────────────────────────────────────────────────────┐
│                contract_products                                     │
│  (contract_id, product_code) = PK                                    │
│  rate_card_id                                                        │
└─────────────────────────────────────────────────────────────────────┘
                              │
                              │ 1:N (FK enforced onboarding gate)
                              ▼
┌─────────────────────────────────────────────────────────────────────┐
│                cbu_subscriptions                                     │
│  (cbu_id, contract_id, product_code) = PK                           │
│  CBU can only subscribe if contract covers that product             │
└─────────────────────────────────────────────────────────────────────┘
```

**Key Concept:** CBU onboarding requires contract+product subscription. No contract = no onboarding.

**client_label:**
- Added to `entities` and `cbus` tables as searchable client shorthand
- "allianz" → all Allianz entities/CBUs, "blackrock" → all BlackRock entities/CBUs
- Used by `session.load-cluster :client "allianz"` for client-scoped loading

**Contract Verbs:**

| Verb | Purpose |
|------|---------|
| `contract.create` | Create legal contract for client |
| `contract.add-product` | Add product with rate card |
| `contract.subscribe` | Subscribe CBU to contract+product (onboarding gate) |
| `contract.list-subscriptions` | List CBU subscriptions |
| `contract.for-client` | Get active contract by client label |

**DSL Examples:**

```clojure
;; Create contract with products
(contract.create :client "allianz" :reference "MSA-2024-001" :effective-date "2024-01-01" :as @contract)
(contract.add-product :contract-id @contract :product "CUSTODY" :rate-card-id @rate)

;; Subscribe CBU (onboarding)
(contract.subscribe :cbu-id @cbu :contract-id @contract :product "CUSTODY")

;; Load client's CBUs
(session.load-cluster :client "allianz" :jurisdiction "LU")
```

**Key Files:**

| File | Purpose |
|------|---------|
| `migrations/045_legal_contracts.sql` | Schema, views, seed data |
| `rust/config/verbs/contract.yaml` | 14 contract verbs |
| `rust/src/domain_ops/session_ops.rs` | `load-cluster` uses client_label |

---

## Deal Record & Fee Billing (067)

> ✅ **IMPLEMENTED (2026-02-05)**: Deal record as commercial origination hub with rate card negotiation and closed-loop fee billing.

**Problem Solved:** Sales, contracting, onboarding, servicing, and billing were disconnected silos. Deal Record is the hub entity that links them all in a closed commercial loop.

### Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    DEAL RECORD - COMMERCIAL HUB                              │
│                                                                              │
│  Deal Record                                                                 │
│       │                                                                       │
│       ├─► Participants (sales owner, relationship manager, legal counsel)   │
│       │                                                                       │
│       ├─► Contracts (links to legal_contracts)                              │
│       │                                                                       │
│       ├─► Rate Cards (with negotiation workflow)                            │
│       │       └─► Rate Card Lines (BPS, FLAT, TIERED, PER_TRANSACTION)     │
│       │                                                                       │
│       ├─► SLAs (service level commitments)                                  │
│       │                                                                       │
│       ├─► Documents (proposals, agreements)                                  │
│       │                                                                       │
│       ├─► UBO Assessments (links to KYC cases)                              │
│       │                                                                       │
│       └─► Onboarding Requests (handoff to CBU creation)                     │
│                   │                                                          │
│                   ▼                                                          │
│           Fee Billing Profile                                                │
│                   │                                                          │
│                   ├─► Account Targets (which CBUs to bill)                  │
│                   │                                                          │
│                   └─► Billing Periods → Period Lines → Invoices             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Deal Status State Machine

```
PROSPECT → QUALIFYING → NEGOTIATING → CONTRACTED → ONBOARDING → ACTIVE → WINDING_DOWN → OFFBOARDED
     ↓           ↓            ↓             ↓            ↓
 CANCELLED   CANCELLED    CANCELLED    CANCELLED    CANCELLED
```

### Rate Card Status State Machine

```
DRAFT → PROPOSED → COUNTER_OFFERED ←→ REVISED → AGREED → SUPERSEDED
   ↓        ↓            ↓               ↓
CANCELLED REJECTED    REJECTED       REJECTED
```

### Pricing Models

| Model | Description | Example |
|-------|-------------|---------|
| `BPS` | Basis points on AUM | 15 bps on $1B = $1.5M |
| `FLAT` | Fixed fee | $50,000/year |
| `TIERED` | Volume-based tiers | 20 bps ≤$100M, 15 bps >$100M |
| `PER_TRANSACTION` | Per-trade fee | $5/trade |

### Database Tables (14 new)

| Table | Purpose |
|-------|---------|
| `deals` | Deal record hub entity |
| `deal_participants` | Sales team members with roles |
| `deal_contracts` | Links deals to legal contracts |
| `deal_rate_cards` | Negotiable fee schedules |
| `deal_rate_card_lines` | Individual fee line items |
| `deal_slas` | Service level agreements |
| `deal_documents` | Attached proposals/agreements |
| `deal_ubo_assessments` | KYC case references |
| `deal_onboarding_requests` | Handoff to CBU onboarding |
| `deal_events` | Audit trail of all deal changes |
| `fee_billing_profiles` | Billing configuration per deal |
| `fee_billing_account_targets` | Which CBUs get billed |
| `fee_billing_periods` | Monthly/quarterly billing cycles |
| `fee_billing_period_lines` | Calculated fee amounts |

### Key Verbs

**Deal Verbs (30):**

| Verb | Purpose |
|------|---------|
| `deal.create` | Create new deal record |
| `deal.update-status` | Advance deal through pipeline |
| `deal.add-participant` | Add sales team member |
| `deal.link-contract` | Attach legal contract |
| `deal.create-rate-card` | Start rate card negotiation |
| `deal.add-rate-card-line` | Add fee line item |
| `deal.propose-rate-card` | Submit for client review |
| `deal.counter-offer` | Client counter-proposal |
| `deal.agree-rate-card` | Finalize negotiation |
| `deal.create-onboarding-request` | Handoff to CBU creation |
| `deal.summary` | Get deal with all related data |

**Billing Verbs (14):**

| Verb | Purpose |
|------|---------|
| `billing.create-profile` | Create billing configuration |
| `billing.add-account-target` | Add CBU to billing scope |
| `billing.open-period` | Start billing cycle |
| `billing.calculate-fees` | Run fee calculation |
| `billing.close-period` | Finalize billing cycle |
| `billing.revenue-summary` | Get revenue analytics |

### DSL Examples

```clojure
;; Create deal and rate card
(deal.create :deal-name "Allianz Custody 2024" 
             :client-group-id <Allianz>
             :sales-owner "john.smith@bny.com"
             :as @deal)

(deal.create-rate-card :deal-id @deal :version "1.0" :as @rate-card)

(deal.add-rate-card-line :rate-card-id @rate-card
                         :fee-type "CUSTODY"
                         :pricing-model "BPS"
                         :rate-value 15.0
                         :currency-code "USD")

(deal.propose-rate-card :rate-card-id @rate-card)

;; After negotiation completes
(deal.agree-rate-card :rate-card-id @rate-card)
(deal.update-status :deal-id @deal :new-status "CONTRACTED")

;; Create billing profile
(billing.create-profile :deal-id @deal
                        :billing-frequency "MONTHLY"
                        :invoice-currency "USD"
                        :as @profile)

(billing.add-account-target :profile-id @profile :cbu-id @cbu)
(billing.open-period :profile-id @profile
                     :period-start "2024-01-01"
                     :period-end "2024-01-31")
```

### Key Files

| File | Purpose |
|------|---------|
| `migrations/067_deal_record_fee_billing.sql` | Schema (14 tables, 2 views) |
| `rust/config/verbs/deal.yaml` | 30 deal verbs |
| `rust/config/verbs/billing.yaml` | 14 billing verbs |
| `rust/src/domain_ops/deal_ops.rs` | Deal custom operations |
| `rust/src/domain_ops/billing_ops.rs` | Billing custom operations |
| `rust/src/api/deal_types.rs` | Deal API types (DealSummary, DealGraphResponse) |
| `rust/src/api/deal_routes.rs` | Deal REST API endpoints |
| `rust/src/database/deal_repository.rs` | Deal database queries |
| `rust/src/graph/deal_graph_builder.rs` | Deal taxonomy graph construction |
| `docs/DEAL_RECORD_IMPLEMENTATION_PLAN.md` | Implementation details |

### Deal Graph API

| Endpoint | Purpose |
|----------|---------|
| `GET /api/deal/:id/graph?view_mode=COMMERCIAL` | Get deal taxonomy graph |
| `GET /api/deal/:id/products` | List deal products |
| `GET /api/deal/:id/rate-cards` | List deal rate cards |
| `GET /api/deal/rate-card/:id/lines` | Get rate card fee lines |
| `GET /api/deal/rate-card/:id/history` | Rate card supersession chain |

### Deal Taxonomy Hierarchy

```
Deal (root)
├── Products (commercial scope)
│   └── Rate Cards
│       └── Rate Card Lines (fee definitions)
├── Participants (sales/relationship team)
├── Contracts (legal agreements)
├── Onboarding Requests
│   └── CBU (if onboarded)
└── Billing Profiles
    └── Account Targets
```

### Test Harness

```bash
# Create complete deal for Aviva Investors (idempotent)
cargo x aviva-deal-harness

# Dry run to see DSL without execution
cargo x aviva-deal-harness --dry-run

# Verbose mode shows all DSL statements
cargo x aviva-deal-harness --verbose
```

The harness creates:
- Deal for Aviva Investors (MSA 2024)
- 2 Contracts (Core Services + Ancillary Services)
- 9 Products with rate cards
- Fee lines with BPS, FLAT, PER_TRANSACTION pricing

---

## BPMN-Lite Integration (Phase B)

> ✅ **IMPLEMENTED (2026-02-09)**: ob-poc ↔ bpmn-lite gRPC wiring with WorkflowDispatcher (queue-based resilience), JobWorker, EventBridge, SignalRelay, PendingDispatchWorker, correlation stores, 15 E2E choreography tests.

**Problem Solved:** Verbs that represent long-running processes (days/weeks — document solicitation, KYC reviews, approvals) were fire-and-forget. Phase B wires ob-poc to the standalone bpmn-lite gRPC service so orchestrated verbs park the REPL runbook and resume on external signals. When the BPMN service is temporarily unavailable, dispatch requests are queued locally and retried automatically.

### Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  BPMN-LITE INTEGRATION PIPELINE                                             │
│                                                                              │
│  REPL V2 Orchestrator                                                        │
│       │                                                                       │
│       ▼                                                                       │
│  WorkflowDispatcher (implements DslExecutorV2)                              │
│       │                                                                       │
│       ├─► Direct verb → inner executor → DslExecutionOutcome::Completed     │
│       │                                                                       │
│       └─► Orchestrated verb:                                                │
│               1. canonical_json_with_hash(payload)                          │
│               2. Generate correlation_id + correlation_key (stable)         │
│               3. Try gRPC StartProcess → process_instance_id               │
│                  ├─ OK → process_instance_id = real PID                     │
│                  └─ ERR → queue to bpmn_pending_dispatches                  │
│                           process_instance_id = dispatch_id (placeholder)   │
│               4. CorrelationStore.insert() (session ↔ process link)         │
│               5. ParkedTokenStore.insert() (waiting entry)                  │
│               6. → DslExecutionOutcome::Parked (always, even if queued)    │
│                                                                              │
│  PendingDispatchWorker (background retry, 10s poll loop)                    │
│       │   1. claim_pending(5, backoff) — FOR UPDATE SKIP LOCKED             │
│       │   2. Retry StartProcess with same correlation_id (idempotent)      │
│       │   3. On success: patch correlation.process_instance_id              │
│       │   4. On failure: record_failure (max 50 → failed_permanent)        │
│       │                                                                      │
│  JobWorker (background task, long-poll loop)                                │
│       │   1. ActivateJobs via gRPC (30s timeout)                            │
│       │   2. Dedupe via JobFrameStore                                       │
│       │   3. Execute ob-poc verb for each job                               │
│       │   4. CompleteJob / FailJob via gRPC                                 │
│       │                                                                      │
│  EventBridge (per-process subscription)                                     │
│       │   1. SubscribeEvents via gRPC (tailing stream)                      │
│       │   2. Translate lifecycle events → OutcomeEvent                      │
│       │   3. Update CorrelationStore / ParkedTokenStore                     │
│       │   4. Forward OutcomeEvent to SignalRelay via channel                │
│                                                                              │
│  SignalRelay (decoupled orchestrator bridge)                                │
│       │   1. Consume OutcomeEvent from EventBridge channel                  │
│       │   2. Look up CorrelationRecord → correlation_key                    │
│       │   3. Call orchestrator.signal_completion()                          │
│       │   4. Runbook entry transitions Parked → Completed/Failed           │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Module Structure

```
rust/src/bpmn_integration/
├── mod.rs                      # Module root, re-exports
├── types.rs                    # CorrelationRecord, JobFrame, ParkedToken, PendingDispatch,
│                               # OutcomeEvent, WorkflowBinding, TaskBinding, ExecutionRoute
├── canonical.rs                # canonical_json_with_hash(), sha256_bytes()
├── client.rs                   # BpmnLiteConnection — typed gRPC client wrapper
├── config.rs                   # WorkflowConfigIndex — verb→route mapping from YAML
├── correlation.rs              # CorrelationStore — session ↔ process instance links
├── job_frames.rs               # JobFrameStore — dedupe for at-least-once delivery
├── parked_tokens.rs            # ParkedTokenStore — waiting REPL entries
├── pending_dispatches.rs       # PendingDispatchStore — durable queue for BPMN resilience
├── pending_dispatch_worker.rs  # PendingDispatchWorker — background retry (10s poll)
├── dispatcher.rs               # WorkflowDispatcher — DslExecutorV2 impl, routes Direct/Orchestrated
├── worker.rs                   # JobWorker — long-poll job activation + verb execution
├── event_bridge.rs             # EventBridge — lifecycle event subscription + translation
└── signal_relay.rs             # SignalRelay — bounces OutcomeEvents to orchestrator
```

### Key Types

| Type | Purpose |
|------|---------|
| `ExecutionRoute` | `Direct` (standard executor) or `Orchestrated` (BPMN-Lite) |
| `WorkflowBinding` | Maps verb FQN → route + process_key + task_bindings |
| `TaskBinding` | Maps BPMN task_type → ob-poc verb FQN + timeout + retries |
| `CorrelationRecord` | Links process_instance_id ↔ session/runbook/entry with payload hash |
| `JobFrame` | Tracks job activation for dedupe (job_key, status, attempts) |
| `ParkedToken` | Waiting REPL entry with correlation_key for O(1) signal routing |
| `PendingDispatch` | Queued BPMN dispatch request with correlation_id, payload_hash, retry state |
| `PendingDispatchStatus` | `Pending` → `Dispatched` or `FailedPermanent` (after max retries) |
| `OutcomeEvent` | Translated BPMN events: StepCompleted, StepFailed, ProcessCompleted, ProcessCancelled, IncidentCreated |
| `BpmnLiteConnection` | Typed gRPC client (connect_lazy, 8 RPC methods) |
| `WorkflowConfigIndex` | In-memory verb→route index loaded from `config/workflows.yaml` |
| `SignalRelay` | Consumes EventBridge outcomes, relays to orchestrator for runbook resume |
| `PendingDispatchWorker` | Background retry worker — 10s poll, claims pending dispatches, retries gRPC |

### Control Verbs

| Verb | Purpose |
|------|---------|
| `bpmn.compile` | Compile BPMN XML → bytecode via gRPC |
| `bpmn.start` | Start a process instance |
| `bpmn.signal` | Send signal to waiting process |
| `bpmn.cancel` | Cancel running process |
| `bpmn.inspect` | Inspect process state (fiber positions, waits) |

### Server Wiring

BPMN integration is **conditionally enabled** on the `BPMN_LITE_GRPC_URL` environment variable in `ob-poc-web/src/main.rs`. The BPMN init block runs **before** the REPL V2 router so the `WorkflowDispatcher` can be wired as the V2 executor.

```bash
# Enable BPMN-Lite integration (WorkflowDispatcher routes orchestrated verbs)
BPMN_LITE_GRPC_URL=http://localhost:50052 cargo run -p ob-poc-web

# Without env var — RealDslExecutor used directly, no parking, no BPMN
cargo run -p ob-poc-web
```

**Startup sequence (when BPMN enabled):**
1. Create `BpmnLiteConnection` (lazy — no network call until first RPC)
2. Load `WorkflowConfigIndex` from `config/workflows.yaml`
3. Auto-register `behavior: durable` verbs into WorkflowConfigIndex
4. Create `WorkflowDispatcher` wrapping `RealDslExecutor` + all stores
5. Spawn `JobWorker` (long-poll job activation)
6. Spawn `PendingDispatchWorker` (retry queued dispatches)
7. Wire `WorkflowDispatcher` as REPL V2 `executor_v2`
8. Log "REPL V2 executor: WorkflowDispatcher (BPMN-routed)"

**Startup sequence (without BPMN):**
1. Wire `RealDslExecutor` as REPL V2 `executor_v2`
2. Log "REPL V2 executor: RealDslExecutor (direct, no BPMN)"

### Database Tables (migrations 073, 076)

| Table | Migration | Purpose |
|-------|-----------|---------|
| `bpmn_correlations` | 073 | PK `correlation_id`, UNIQUE on `process_instance_id` |
| `bpmn_job_frames` | 073 | PK `job_key`, indexed on `status WHERE active` |
| `bpmn_parked_tokens` | 073 | PK `token_id`, UNIQUE on `correlation_key` |
| `bpmn_pending_dispatches` | 076 | PK `dispatch_id`, UNIQUE on `payload_hash WHERE pending` (idempotency), indexed on `(status, last_attempted_at) WHERE pending` |

### Key Files

| File | Purpose |
|------|---------|
| `rust/src/bpmn_integration/dispatcher.rs` | `WorkflowDispatcher` — routes verbs, queues on gRPC failure |
| `rust/src/bpmn_integration/pending_dispatches.rs` | `PendingDispatchStore` — durable queue (insert, claim, mark, fail) |
| `rust/src/bpmn_integration/pending_dispatch_worker.rs` | `PendingDispatchWorker` — background retry (10s poll, max 50 attempts) |
| `rust/src/bpmn_integration/worker.rs` | `JobWorker` — background job activation loop |
| `rust/src/bpmn_integration/event_bridge.rs` | `EventBridge` — lifecycle event translation |
| `rust/src/bpmn_integration/signal_relay.rs` | `SignalRelay` — orchestrator bridge |
| `rust/src/bpmn_integration/client.rs` | `BpmnLiteConnection` — typed gRPC wrapper |
| `rust/src/bpmn_integration/config.rs` | `WorkflowConfigIndex` — verb routing config |
| `rust/src/domain_ops/bpmn_lite_ops.rs` | 5 control verb CustomOps |
| `rust/config/workflows.yaml` | Verb → route mappings |
| `rust/config/verbs/bpmn-lite.yaml` | Control verb definitions |
| `rust/proto/bpmn_lite/v1/bpmn_lite.proto` | gRPC service definition (8 RPCs) |
| `rust/crates/ob-poc-web/src/main.rs` | BPMN init before REPL V2, wires WorkflowDispatcher + workers |
| `migrations/073_bpmn_integration.sql` | 3 tables: correlations, job_frames, parked_tokens |
| `migrations/076_bpmn_pending_dispatches.sql` | Pending dispatch queue table with idempotency index |
| `rust/tests/bpmn_integration_test.rs` | 13 integration tests (all `#[ignore]`) |
| `rust/tests/bpmn_e2e_harness_test.rs` | 15 E2E choreography tests (all `#[ignore]`) |
| `rust/tests/models/kyc-open-case.bpmn` | Test BPMN model (7-node KYC workflow) |

### Environment Variables

| Variable | Default | Purpose |
|----------|---------|---------|
| `BPMN_LITE_GRPC_URL` | (none — disabled) | gRPC endpoint for bpmn-lite service |

### Queue-Based Resilience (Pending Dispatch)

When the bpmn-lite gRPC service is temporarily unavailable, the `WorkflowDispatcher` queues the dispatch request locally in `bpmn_pending_dispatches` and still returns `Parked` to the REPL. The `PendingDispatchWorker` retries queued dispatches every 10 seconds until the service recovers.

**Idempotency chain:**

```
Verb dispatch → canonical_json_with_hash(payload) → (json, hash)
  ├─ gRPC OK → correlation(correlation_id, real_pid) + parked_token → Parked
  └─ gRPC ERR → pending_dispatch(dispatch_id, correlation_id, hash)
               + correlation(correlation_id, placeholder_pid) + parked_token → Parked
                    ↓
               PendingDispatchWorker retries with same correlation_id
                    ↓
               start_process(correlation_id) — bpmn-lite is idempotent on correlation_id
                    ↓
               On success: update correlation.process_instance_id, mark dispatched
```

**Key guarantees:**
- `correlation_id` generated once at dispatch time, stable across retries
- `payload_hash` UNIQUE index prevents duplicate pending entries
- `FOR UPDATE SKIP LOCKED` prevents concurrent workers claiming the same row
- Max 50 attempts (~8 minutes at 10s intervals) before `failed_permanent`
- Only fails immediately if BOTH gRPC AND queue insert fail (true infra failure)

**PendingDispatchStore methods:**

| Method | Purpose |
|--------|---------|
| `insert()` | Queue dispatch (ON CONFLICT DO NOTHING for idempotency) |
| `claim_pending()` | Pop pending rows with backoff (FOR UPDATE SKIP LOCKED) |
| `mark_dispatched()` | Transition to dispatched state with timestamp |
| `record_failure()` | Increment attempts, promote to failed_permanent after max |

---

## Client Group Resolver (048)

> ✅ **IMPLEMENTED (2026-01-23)**: Two-stage alias→group→anchor resolution for session scope selection.

The Client Group Resolver enables natural language scope selection (e.g., "allianz" → load all Allianz CBUs) through a two-stage resolution process:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    TWO-STAGE RESOLUTION                                      │
│                                                                              │
│  User: "allianz"                                                            │
│      │                                                                       │
│      ▼                                                                       │
│  Stage 1: Alias → ClientGroupId                                             │
│      │   "allianz" → exact/semantic match → Allianz Global Investors        │
│      │   (group_id: 11111111-1111-1111-1111-111111111111)                   │
│      │                                                                       │
│      ▼                                                                       │
│  Stage 2: ClientGroupId → AnchorEntityId                                    │
│      │   group_id + role (governance_controller) + jurisdiction (optional) │
│      │   → Allianz Global Investors Holdings GmbH (entity_id)              │
│      │                                                                       │
│      ▼                                                                       │
│  session.load-cluster uses anchor_entity_id to find all CBUs               │
│  under the client's ownership hierarchy                                     │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Data Model:**

| Table | Purpose |
|-------|---------|
| `client_group` | Virtual client groups (Allianz, Aviva, BlackRock) |
| `client_group_alias` | Searchable aliases with embeddings |
| `client_group_alias_embedding` | Versioned embeddings per alias |
| `client_group_anchor` | Role-based anchors per jurisdiction |

**Anchor Roles:**

| Role | Use Case | Default For |
|------|----------|-------------|
| `ultimate_parent` | UBO discovery, ownership tracing | `ubo.*` verbs |
| `governance_controller` | Session scope, CBU loading | `session.*`, `cbu.*` verbs |
| `book_controller` | Regional operations | - |
| `operating_controller` | Day-to-day operations | `contract.*` verbs |
| `regulatory_anchor` | Compliance, KYC | `kyc.*` verbs |

**DSL Usage:**

```clojure
;; Load by client name (two-stage resolution)
(session.load-cluster :client <Allianz>)

;; With jurisdiction filter
(session.load-cluster :client <Allianz> :jurisdiction "LU")

;; Direct entity (bypasses client group resolution)
(session.load-cluster :apex-entity-id "uuid-...")
```

**Key Files:**

| File | Purpose |
|------|---------|
| `migrations/048_client_group_seed.sql` | Schema + bootstrap data |
| `rust/crates/ob-semantic-matcher/src/client_group_resolver.rs` | Resolution logic |
| `rust/src/domain_ops/session_ops.rs` | `SessionLoadClusterOp` handler |
| `rust/config/verbs/session.yaml` | `:client` arg with lookup config |
| `rust/tests/client_group_integration.rs` | Full integration tests |

**Bootstrap Data:**

| Group | Canonical Name | Anchors |
|-------|----------------|---------|
| Allianz | Allianz Global Investors | Allianz SE (UP), AGI Holdings GmbH (GC) |
| Aviva | Aviva Investors | Aviva plc (UP), Aviva Investors Global (GC) |
| BlackRock | BlackRock | BlackRock Inc (UP), BLK Fund Advisors (GC) |
| Aberdeen | Aberdeen Standard Investments | abrdn plc (UP, GC) |

**Populate Embeddings:**

After adding client groups or aliases:

```bash
DATABASE_URL="postgresql:///data_designer" \
  cargo run --release --package ob-semantic-matcher --bin populate_embeddings -- --client-groups
```

---

## Workflow Task Queue & Document Entity (049)

> ✅ **IMPLEMENTED (2026-01-24)**: Queue-based async task return path with document as first-class entity.

**Problem Solved:** Workflows can emit tasks (e.g., "solicit passport") but had no mechanism to receive results and resume. The task queue provides the return path.

### Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  OUTBOUND: Workflow emits task                                              │
│  Blocker detected → resolution DSL → workflow_pending_tasks                 │
│  → External system (Camunda, portal, email) receives task_id + callback URL │
└─────────────────────────────────────────────────────────────────────────────┘
                              │
                    External system works...
                              │
┌─────────────────────────────────────────────────────────────────────────────┐
│  INBOUND: Task completion webhook                                           │
│  POST /api/workflow/task-complete → task_result_queue                       │
│  TaskQueueListener → try_advance(workflow_instance)                         │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Three-Layer Document Model

| Layer | Table | Purpose |
|-------|-------|---------|
| A: Requirement | `document_requirements` | What we NEED from entity (exists before upload) |
| B: Document | `documents` | Logical identity (stable reference) |
| C: Version | `document_versions` | Each submission (immutable, verification status) |

### Requirement State Machine

```
missing → requested → received → in_qa → verified
                                    ↓
                               rejected → (retry with new version)
                                    
waived (manual override)    expired (validity lapsed)
```

### Rejection Reason Codes

Standardized codes for document QA rejection with client messaging:

| Category | Codes | Next Action |
|----------|-------|-------------|
| Quality | `UNREADABLE`, `CUTOFF`, `GLARE`, `LOW_RESOLUTION` | Re-upload |
| Mismatch | `WRONG_DOC_TYPE`, `WRONG_PERSON`, `SAMPLE_DOC` | Upload correct doc |
| Validity | `EXPIRED`, `NOT_YET_VALID`, `UNDATED` | Upload valid doc |
| Data | `DOB_MISMATCH`, `NAME_MISMATCH`, `ADDRESS_MISMATCH` | Verify details |
| Authenticity | `SUSPECTED_ALTERATION`, `INCONSISTENT_FONTS` | Escalate |

### CargoRef URI Scheme

Typed reference URIs for document/entity tracking:

```
document://ob-poc/{uuid}     # Document identity
version://ob-poc/{uuid}      # Specific version (preferred for callbacks)
entity://ob-poc/{uuid}       # Entity reference
screening://ob-poc/{uuid}    # Screening result
external://{system}/{id}     # Vendor passthrough
```

### Bundle Payload Format

External systems return bundles (even for single documents):

```json
{
  "task_id": "uuid",
  "status": "completed",
  "idempotency_key": "vendor-event-12345",
  "items": [
    { "cargo_ref": "version://ob-poc/...", "doc_type": "passport", "status": "completed" }
  ]
}
```

Listener uses atomic CTE-based queue pop (`FOR UPDATE SKIP LOCKED`) with deduplication by `(task_id, idempotency_key)`.

### Key Tables

| Table | Purpose |
|-------|---------|
| `rejection_reason_codes` | Reference data for QA rejection reasons |
| `document_requirements` | What documents are needed per entity/workflow |
| `documents` | Logical document identity |
| `document_versions` | Immutable submissions with verification status |
| `workflow_pending_tasks` | Outbound task tracking |
| `task_result_queue` | Inbound results (ephemeral, deleted after processing) |
| `task_result_dlq` | Dead letter queue for failed processing |
| `workflow_task_events` | Permanent audit trail |

### API Endpoints

| Endpoint | Purpose |
|----------|---------|
| `POST /api/workflow/task-complete` | External callback webhook (bundle payload) |
| `POST /api/documents` | Create document |
| `POST /api/documents/:id/versions` | Upload version, returns `cargo_ref` |
| `GET /api/documents/:id` | Get document with status |
| `POST /api/documents/:doc_id/versions/:version_id/verify` | QA approve |
| `POST /api/documents/:doc_id/versions/:version_id/reject` | QA reject with code |
| `GET /api/requirements` | List requirements (with filters) |

### DSL Verbs

| Verb | Purpose |
|------|---------|
| `document.solicit` | Request document from entity |
| `document.solicit-set` | Request multiple documents |
| `document.verify` | QA approves version |
| `document.reject` | QA rejects with reason code |
| `requirement.create` | Initialize requirement |
| `requirement.waive` | Manual override (skip requirement) |

### Workflow Guard Integration

Guards check **requirement status**, not raw document existence:

```yaml
states:
  awaiting_identity:
    requirements:
      - type: requirement_satisfied
        doc_type: passport
        min_state: verified       # Must be QA-approved
        subject: $entity_id
        max_age_days: 90          # Recency check on satisfied_at
```

### Key Files

| File | Purpose |
|------|---------|
| `migrations/049_workflow_task_queue_documents.sql` | Schema (all tables) |
| `rust/crates/ob-workflow/src/listener.rs` | Queue listener with retry/DLQ |
| `rust/crates/ob-workflow/src/cargo_ref.rs` | CargoRef URI scheme |
| `rust/crates/ob-workflow/src/document.rs` | Document types, RequirementState |
| `rust/src/api/workflow_routes.rs` | HTTP endpoints |
| `rust/config/verbs/document.yaml` | 7 document verbs |
| `rust/config/verbs/requirement.yaml` | 5 requirement verbs |

---

## Transactional Execution & Advisory Locking (050)

> ✅ **IMPLEMENTED (2026-01-24)**: Atomic execution with PostgreSQL advisory locks, deterministic template expansion, and audit trail.

**Problem Solved:** Multi-statement DSL batches need transactional guarantees. Without locking, concurrent sessions can corrupt shared entities. The expansion stage derives locks deterministically from DSL structure.

### Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  DSL EXECUTION PIPELINE WITH EXPANSION                                       │
│                                                                              │
│  Source DSL: (cbu.create ...) (entity.create ...) (trading-profile.create)  │
│      │                                                                       │
│      ▼                                                                       │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │  EXPANSION STAGE (deterministic)                                     │    │
│  │  • Expand template verbs (behavior: template)                        │    │
│  │  • Derive lock set from entity references                            │    │
│  │  • Determine batch_policy: atomic vs best_effort                     │    │
│  │  • Generate ExpansionReport for audit                                │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│      │                                                                       │
│      ├─── batch_policy = atomic ──────────────────────────┐                 │
│      │                                                     ▼                 │
│      │    ┌───────────────────────────────────────────────────────────┐     │
│      │    │  ATOMIC EXECUTOR                                          │     │
│      │    │  • Acquire advisory locks (sorted, no deadlock)           │     │
│      │    │  • Single transaction wraps all statements                │     │
│      │    │  • Any failure → full rollback                            │     │
│      │    │  • Result: Committed | RolledBack | LockContention        │     │
│      │    └───────────────────────────────────────────────────────────┘     │
│      │                                                                       │
│      └─── batch_policy = best_effort ─────────────────────┐                 │
│                                                            ▼                 │
│           ┌───────────────────────────────────────────────────────────┐     │
│           │  BEST-EFFORT EXECUTOR                                     │     │
│           │  • No locking required                                    │     │
│           │  • Execute statements independently                       │     │
│           │  • Continue on failure, aggregate errors                  │     │
│           │  • Result: { succeeded: [...], failed: [...] }            │     │
│           └───────────────────────────────────────────────────────────┘     │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Batch Policy

| Policy | Lock Acquisition | Failure Behavior | Use Case |
|--------|------------------|------------------|----------|
| `atomic` | Advisory locks (sorted by entity_id) | Full rollback | Multi-entity operations needing consistency |
| `best_effort` | None | Continue, aggregate errors | Independent operations, bulk imports |

**Policy Determination:**
- `atomic`: Template verbs with `batch_policy: atomic`, or DSL touching multiple related entities
- `best_effort`: Independent statements, bulk operations, read-only queries

### Advisory Locks

PostgreSQL `pg_advisory_xact_lock` provides session-scoped locking:

```rust
pub struct DerivedLock {
    pub entity_type: String,  // "cbu", "entity", "trading_profile"
    pub entity_id: Uuid,      // Target entity
    pub access: LockAccess,   // Read or Write
}

// Lock key derivation: deterministic i64 from (entity_type, entity_id) using DefaultHasher
// Locks acquired in sorted order (entity_id) to prevent deadlocks
// Released automatically when transaction commits/rolls back
```

### Error Aggregation

Errors grouped by root cause for clear diagnostics:

| ErrorCause | Description |
|------------|-------------|
| `EntityDeleted` | Entity was deleted mid-execution |
| `EntityNotFound` | Referenced entity doesn't exist |
| `VersionConflict` | Optimistic lock failure |
| `ConstraintViolation` | FK/unique constraint failed |

`FailureTiming` distinguishes `PreExisting` (bad input) vs `MidExecution` (race condition).

### Expansion Safety Rules

- **Expansion output is UNTRUSTED** - validate expanded macros like hostile input
- **Max expansion steps: 100** - prevents runaway recursive macros
- **No macro recursion** - primitives only in expansion output

### Execution Results

```rust
pub enum AtomicExecutionResult {
    Committed {
        results: Vec<ExecutionResult>,
        lock_stats: LockStats,
    },
    RolledBack {
        cause: RollbackCause,
        partial_results: Vec<ExecutionResult>,
    },
    LockContention {
        contested_locks: Vec<DerivedLock>,
        retry_after_ms: Option<u64>,
    },
}

pub struct BestEffortExecutionResult {
    pub succeeded: Vec<(usize, ExecutionResult)>,  // (statement_index, result)
    pub failed: Vec<(usize, ExecutionError)>,       // (statement_index, error)
    pub skipped: Vec<usize>,                        // Skipped due to dependency
}
```

### Expansion Report (Audit Trail)

Every DSL execution produces an `ExpansionReport` persisted to `ob-poc.expansion_reports`:

| Field | Description |
|-------|-------------|
| `expansion_id` | Unique ID for this expansion |
| `session_id` | Session that triggered execution |
| `source_digest` | SHA-256 of canonical source DSL |
| `expanded_dsl_digest` | SHA-256 of expanded DSL |
| `batch_policy` | `atomic` or `best_effort` |
| `derived_lock_set` | JSON array of locks derived |
| `template_digests` | Templates used (name, version, hash) |
| `invocations` | Template invocation details |
| `diagnostics` | Warnings/errors during expansion |

### Key Files

| File | Purpose |
|------|---------|
| `migrations/050_expansion_audit.sql` | Schema for expansion_reports |
| `rust/src/dsl_v2/expansion/engine.rs` | Template expansion, lock derivation |
| `rust/src/dsl_v2/expansion/policy.rs` | Batch policy determination |
| `rust/src/dsl_v2/executor.rs` | `execute_plan_atomic_with_locks`, `execute_plan_best_effort` |
| `rust/src/dsl_v2/locking.rs` | Advisory lock acquisition/release |
| `rust/src/database/expansion_audit.rs` | ExpansionAuditRepository |
| `rust/src/api/agent_routes.rs` | Session execution with expansion |
| `rust/src/mcp/handlers/core.rs` | MCP dsl_execute with expansion |

### Database Table

```sql
CREATE TABLE "ob-poc".expansion_reports (
    expansion_id UUID PRIMARY KEY,
    session_id UUID NOT NULL,
    source_digest VARCHAR(64) NOT NULL,
    expanded_dsl_digest VARCHAR(64) NOT NULL,
    expanded_statement_count INTEGER NOT NULL,
    batch_policy VARCHAR(20) NOT NULL CHECK (batch_policy IN ('atomic', 'best_effort')),
    derived_lock_set JSONB NOT NULL DEFAULT '[]',
    template_digests JSONB NOT NULL DEFAULT '[]',
    invocations JSONB NOT NULL DEFAULT '[]',
    diagnostics JSONB NOT NULL DEFAULT '[]',
    expanded_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

---


---

## Playbook System (059)

> ✅ **IMPLEMENTED (2026-01-28)**: YAML-based playbook specs with LSP validation, source mapping, and CLI tooling.

**Purpose:** Define reusable multi-step workflows as YAML playbooks that lower to DSL statements with slot substitution.

### Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    PLAYBOOK PIPELINE                                         │
│                                                                              │
│  .playbook.yaml file                                                        │
│         │                                                                    │
│         ▼                                                                    │
│  playbook-core::parse_playbook()                                            │
│         │   - Pass 1: serde_yaml → PlaybookSpec (typed)                     │
│         │   - Pass 2: marked-yaml → SourceMap (line:col positions)          │
│         │                                                                    │
│         ▼                                                                    │
│  playbook-lower::lower_playbook()                                           │
│         │   - Substitutes ${slot} references                                │
│         │   - Reports missing required slots                                │
│         │   - Generates DSL statements                                      │
│         │                                                                    │
│         ▼                                                                    │
│  DSL statements ready for execution                                         │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Playbook Spec Format

```yaml
id: setup-pe-fund
version: 1
name: "Set Up PE Fund Structure"
slots:
  fund_name:
    type: string
    required: true
  fund_type:
    type: string
    required: false
    default: "private-equity"
steps:
  - id: create-fund
    verb: cbu.create
    args:
      name: "${fund_name}"
      kind: "${fund_type}"
  - id: assign-gp
    verb: cbu-role.assign
    after: [create-fund]
    args:
      cbu_id: "${create-fund.result}"
      role: general-partner
```

### Crates

| Crate | Purpose |
|-------|---------|
| `playbook-core` | Spec types, parser with marked-yaml source mapping |
| `playbook-lower` | Slot state management, DSL lowering |

### Parser (Two-Pass with marked-yaml)

```rust
// Pass 1: Typed deserialization
let spec: PlaybookSpec = serde_yaml::from_str(source)?;

// Pass 2: AST walk with source positions
let node = marked_yaml::parse_yaml(0, source)?;
let marker = verb_node.span().start()?;  // Marker{line, col}
```

No string scanning or regex - pure AST traversal with exact source positions.

### LSP Support

- **File detection:** `.playbook.yaml`, `.playbook.yml`
- **Diagnostics:** Parse errors, missing slots reported with exact positions
- **Debouncing:** 100ms delay before analysis (avoids thrashing)
- **Completion:** Verb completions from macro registry

### CLI Tool

```bash
# Check playbook(s)
cargo run -p xtask -- playbook-check <files> [-v|--verbose]

# Example
cargo run -p xtask -- playbook-check data/playbooks/*.playbook.yaml -v
```

### Zed Extension

Extension updated to recognize `.playbook.yaml` files:
- Syntax highlighting via tree-sitter-yaml
- Language server integration for diagnostics
- Verb completion in `verb:` context

### Key Files

| File | Purpose |
|------|---------|
| `rust/crates/playbook-core/src/spec.rs` | PlaybookSpec, SlotSpec, StepSpec |
| `rust/crates/playbook-core/src/parser.rs` | Two-pass parser with marked-yaml |
| `rust/crates/playbook-core/src/source_map.rs` | SourceSpan, StepSpan |
| `rust/crates/playbook-lower/src/lower.rs` | lower_playbook(), MissingSlot |
| `rust/crates/playbook-lower/src/slots.rs` | SlotState, SlotValue |
| `rust/crates/dsl-lsp/src/handlers/playbook.rs` | LSP playbook analysis |
| `rust/crates/dsl-lsp/zed-extension/languages/playbook/` | Zed language config |
| `rust/xtask/src/main.rs` | PlaybookCheck command |

---

## Zed Extension: DSL Tree-sitter Grammar (Dev Install)

> **Critical for local development** - Installing the custom DSL grammar in Zed.
> This was painful to get working. Follow these steps EXACTLY.

### Directory Structure

```
rust/crates/dsl-lsp/
├── zed-extension/           # Extension root (select THIS for dev install)
│   ├── extension.toml       # Must be at root of selected directory
│   ├── Cargo.toml           # Rust extension for LSP launch
│   ├── src/lib.rs           # LSP command provider
│   ├── languages/
│   │   ├── dsl/
│   │   │   ├── config.toml      # grammar = "dsl" (must match [grammars.dsl])
│   │   │   ├── highlights.scm
│   │   │   ├── brackets.scm
│   │   │   ├── indents.scm
│   │   │   ├── outline.scm
│   │   │   ├── textobjects.scm
│   │   │   ├── overrides.scm
│   │   │   └── runnables.scm
│   │   └── playbook/
│   │       └── config.toml      # grammar = "yaml"
│   └── snippets/
│       └── dsl.json
└── tree-sitter-dsl/         # Grammar source
    ├── grammar.js
    ├── package.json         # tree-sitter-cli devDependency
    ├── src/parser.c         # Generated - run `npx tree-sitter generate`
    └── tree-sitter-dsl.wasm # Generated - run `npx tree-sitter build --wasm`
```

### Installation Steps

1. **Upgrade tree-sitter CLI** (0.22+ required for Docker-free WASM build):
   ```bash
   cd rust/crates/dsl-lsp/tree-sitter-dsl
   npm install tree-sitter-cli@latest --save-dev
   ```

2. **Generate grammar artifacts:**
   ```bash
   npx tree-sitter generate
   npx tree-sitter build --wasm   # Downloads wasi-sdk automatically, no Docker
   ```

3. **Verify config alignment:**
   - `languages/dsl/config.toml`: `grammar = "dsl"` (NOT "clojure")
   - `extension.toml`: `[grammars.dsl]` section exists with proper config

4. **Clean any stale grammar cache** (if reinstalling):
   ```bash
   rm -rf rust/crates/dsl-lsp/zed-extension/grammars/
   ```

5. **Install in Zed:**
   - Open Extensions panel (Cmd+Shift+X or `zed: extensions`)
   - Click **"Install Dev Extension"**
   - Select: `rust/crates/dsl-lsp/zed-extension/` (the directory with `extension.toml`)

6. **Verify:**
   - Open any `.dsl` file
   - Status bar should show "DSL" (not Clojure/Plain Text)
   - Syntax highlighting should work (verb names, keywords, symbols)

### extension.toml Grammar Config (EXACT FORMAT)

```toml
# For local development - file:// URL to REPO ROOT + path to grammar
# BOTH repository AND rev are REQUIRED (even for file://)
[grammars.dsl]
repository = "file:///Users/adamtc007/Developer/ob-poc"
rev = "c50cd396ac861ee21c8db56de664ed55b3a9b1f0"  # Must be actual commit SHA
path = "rust/crates/dsl-lsp/tree-sitter-dsl"

# For publishing - use https:// with commit SHA + path
[grammars.dsl]
repository = "https://github.com/your-org/ob-poc"
rev = "abc123def456"  # Must be commit SHA, NOT "main" or "HEAD"
path = "rust/crates/dsl-lsp/tree-sitter-dsl"
```

**To get current commit SHA:**
```bash
git rev-parse HEAD
```

### Common Failure Modes (Battle-Tested)

| Problem | Cause | Fix |
|---------|-------|-----|
| `.dsl` shows as Clojure | `grammar = "clojure"` in config.toml | Change to `grammar = "dsl"` |
| No highlighting | Grammar name mismatch | `config.toml` grammar must match `[grammars.X]` key |
| "missing field `rev`" | `rev` not specified | Add `rev = "<commit-sha>"` (required even for file://) |
| "pathspec 'HEAD' did not match" | `rev = "HEAD"` doesn't work | Use actual commit SHA from `git rev-parse HEAD` |
| "grammar directory already exists" | Stale cache | Delete `zed-extension/grammars/` and reinstall |
| "Failed to fetch grammar" | Wrong file:// path | Point to repo root, use `path` for subdirectory |
| "parser.c missing" | Grammar not generated | Run `npx tree-sitter generate` |
| WASM build needs Docker | Old tree-sitter CLI (<0.22) | `npm install tree-sitter-cli@latest` |
| Extension not found | Wrong folder selected | Must select folder containing `extension.toml` |
| TOML parse error with curly quotes | Copy-paste from docs | Replace `""` with `"`, `''` with `'` |
| "expected newline" in TOML | Malformed string escaping | Check `autoclose_before` and bracket strings |

### config.toml Gotchas

**NEVER use curly/smart quotes** - they break TOML parsing:
```toml
# WRONG - curly quotes from copy-paste
autoclose_before = "]"'"
brackets = [{ start = """, end = """ }]

# CORRECT - straight quotes
autoclose_before = "]'\""
brackets = [{ start = "\"", end = "\"" }]
```

### Debug

1. Open Zed log: `zed: open log` command
2. Search for "grammar", "extension", "TOML", or "dsl"
3. Look for specific error messages

Common log patterns:
- `compiled grammar dsl` = SUCCESS
- `TOML parse error at line X` = config.toml syntax error
- `missing field 'rev'` = extension.toml needs rev
- `failed to checkout revision` = wrong rev or stale cache

### Key Files

| File | Purpose |
|------|---------|
| `zed-extension/extension.toml` | Extension manifest + grammar registration |
| `zed-extension/languages/dsl/config.toml` | Language config (grammar binding) |
| `zed-extension/languages/playbook/config.toml` | Playbook YAML config |
| `zed-extension/languages/dsl/highlights.scm` | Syntax highlighting queries |
| `tree-sitter-dsl/grammar.js` | Grammar definition |
| `tree-sitter-dsl/package.json` | tree-sitter-cli version (keep at 0.22+) |
| `tree-sitter-dsl/src/parser.c` | Generated parser (commit this) |
| `tree-sitter-dsl/tree-sitter-dsl.wasm` | Compiled WASM (Zed compiles this itself) |

### After Making Changes

If you modify the grammar or extension config:
1. Re-run `npx tree-sitter generate` (if grammar.js changed)
2. Delete `zed-extension/grammars/` to clear cache
3. Re-run "Install Dev Extension" in Zed
4. Check `zed: open log` for errors

---

## DSL Language Server (dsl-lsp)

> ✅ **IMPLEMENTED (060-063)**: Full LSP with completions, hover, rename, diagnostics, code actions, and EntityGateway integration.

**Crate:** `rust/crates/dsl-lsp/` — Language Server Protocol implementation for the Onboarding DSL.

### Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    DSL LANGUAGE SERVER                                       │
│                                                                              │
│  Editor (Zed/VS Code)         LSP Server (tower-lsp)        External        │
│  ───────────────────          ──────────────────────        ────────        │
│                                                                              │
│  .dsl file opened    ───►    did_open()                                     │
│                               │                                              │
│                               ├─► parse_with_v2() (dsl-core)                │
│                               ├─► Extract symbols (@bindings)               │
│                               ├─► LspValidator.validate()                   │
│                               ├─► analyse_and_plan() (DAG)    ◄─── ob-poc  │
│                               └─► publish_diagnostics()                     │
│                                                                              │
│  User types          ───►    did_change() [debounced 100ms]                 │
│                               └─► Re-analyze + re-publish                   │
│                                                                              │
│  Ctrl+Space          ───►    completion()                                   │
│                               ├─► detect_completion_context()               │
│                               ├─► complete_verbs / keywords / symbols       │
│                               └─► EntityGateway lookup  ◄─── gRPC          │
│                                                                              │
│  Hover               ───►    hover()                                        │
│                               └─► Verb docs, symbol info, error suggestions │
│                                                                              │
│  F2 Rename           ───►    rename()                                       │
│                               └─► Find all symbol refs, apply edits         │
│                                                                              │
│  Cmd+.               ───►    code_action()                                  │
│                               ├─► Implicit creates (PlanningOutput)         │
│                               ├─► Reorder suggestions                       │
│                               └─► Entity "did you mean?" fixes              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### LSP Capabilities

| Capability | Handler | Features |
|------------|---------|----------|
| **Diagnostics** | `diagnostics.rs` | Syntax errors, semantic validation, DAG warnings |
| **Completion** | `completion.rs` | Verbs, `:keywords`, `@symbols`, entity lookups, next-step suggestions |
| **Hover** | `hover.rs` | Verb documentation, symbol info, error suggestions |
| **Go to Definition** | `goto_definition.rs` | Jump to `@symbol` definitions |
| **Find References** | `goto_definition.rs` | Find all uses of `@symbol` |
| **Rename** | `rename.rs` | Rename symbols across document |
| **Signature Help** | `signature.rs` | Verb argument hints while typing |
| **Document Symbols** | `symbols.rs` | Outline view of verbs and bindings |
| **Code Actions** | `code_actions.rs` | Quick fixes, implicit creates, reordering |

### Completion Contexts

The LSP detects context and routes to appropriate completer:

| Context | Trigger | Completions |
|---------|---------|-------------|
| `VerbName` | After `(` | All verbs from registry |
| `Keyword` | After verb name | Valid `:args` for that verb |
| `KeywordValue` | After `:keyword` | Enum values, entity lookups |
| `SymbolRef` | After `@` | Defined `@bindings` in document |
| `None` | Empty line | DAG-based next-step suggestions |

### Entity Gateway Integration

For entity completion (e.g., `:counterparty <...>`), the LSP connects to EntityGateway via gRPC:

```bash
# Environment variables
ENTITY_GATEWAY_URL=http://localhost:50051   # Default: [::1]:50051
DSL_CONFIG_DIR=/path/to/config/verbs/       # Verb YAML directory
```

If EntityGateway is unavailable, LSP falls back to syntax-only mode.

### Code Actions

Three sources of code actions from `PlanningOutput`:

| Source | Example Action |
|--------|----------------|
| `synthetic_steps` | "Create @cbu with cbu.create" (implicit binding) |
| `was_reordered` | "Reorder statements for dependencies" |
| `SemanticDiagnostic.suggestions` | "Did you mean 'John Smith'?" |

### Document Analysis Pipeline

```
DSL Source
    │
    ▼
parse_with_v2() ──► DocumentState { text, expressions, symbol_defs, symbol_refs }
    │
    ▼
LspValidator.validate() ──► SemanticDiagnostics (entity resolution errors)
    │
    ▼
analyse_and_plan() ──► PlanningOutput { phases, synthetic_steps, was_reordered }
    │
    ▼
publish_diagnostics() ──► Editor shows errors/warnings
```

### Shared Validation with Server

The LSP uses the **same validation code** as the server:

```rust
// Both LSP and server use:
ob_poc::parse_program()           // Parser
ob_poc::LspValidator              // Semantic validation
ob_poc::planning_facade           // DAG analysis
ob_poc::RuntimeVerbRegistry       // Verb metadata
```

This ensures LSP diagnostics match server behavior 100%.

### Playbook Support

Files matching `*.playbook.yaml` / `*.playbook.yml` get specialized handling:

- Parse with `playbook-core`
- Lower with `playbook-lower`
- Report missing required slots as warnings
- Validate verb references

### Running the LSP

```bash
# Direct execution
cargo run --release -p dsl-lsp

# Zed auto-launches via extension config
# VS Code: configure in settings.json
```

### Logging

LSP logs to file (stdout reserved for protocol):

```bash
tail -f /tmp/dsl-lsp.log

# Control verbosity
DSL_LSP=trace cargo run -p dsl-lsp   # Max verbosity
DSL_LSP=warn cargo run -p dsl-lsp    # Errors only
```

### Key Files

| File | Purpose |
|------|---------|
| `src/main.rs` | Entry point, stdio setup |
| `src/server.rs` | Core LSP state machine, lifecycle |
| `src/analysis/document.rs` | `DocumentState` - parsed document representation |
| `src/analysis/context.rs` | `detect_completion_context()` |
| `src/handlers/completion.rs` | Completion logic with sub-completers |
| `src/handlers/diagnostics.rs` | Validation pipeline |
| `src/handlers/code_actions.rs` | Code action generation |
| `src/handlers/hover.rs` | Hover information |
| `src/handlers/rename.rs` | Symbol rename |
| `src/encoding.rs` | UTF-16/UTF-8 position conversion |
| `src/entity_client.rs` | gRPC client for EntityGateway |

### Performance

| Optimization | Implementation |
|--------------|----------------|
| Debouncing | 100ms delay on `did_change` |
| Incremental sync | Clients send deltas, not full text |
| Lazy EntityGateway | Only connects when completion needs entities |
| Lazy planning | Only runs if no parse errors |
| Caching | Verb registry and macro registry cached in `OnceLock` |

---

## Staged Runbook REPL (054)

> ✅ **IMPLEMENTED (2026-01-25)**: Anti-hallucination execution model with staged commands, entity resolution, DAG ordering.

**Problem Solved:** Agent could hallucinate entity UUIDs or execute commands without user confirmation. The staged runbook provides a deterministic bridge from natural language → resolved UUIDs → safe execution.

### Architecture

```
User prompt: "Show me Irish funds"
    │
    ▼
┌─────────────────────────────────────────────────────────────────┐
│  STAGE (no execution)                                           │
│  runbook_stage tool                                             │
│  • Parse DSL                                                    │
│  • Resolve entity arguments → UUIDs via DB search               │
│  • Stage command in staged_runbook table                        │
│  • Emit CommandStaged / ResolutionAmbiguous / StageFailed       │
└─────────────────────────────────────────────────────────────────┘
    │
    ▼ (user reviews, may pick from ambiguous, may remove commands)
    │
    ▼ (explicit "run" / "execute" / "commit")
┌─────────────────────────────────────────────────────────────────┐
│  RUN (server-side ready gate)                                   │
│  runbook_run tool                                               │
│  • Validate all commands resolved                               │
│  • Compute DAG order                                            │
│  • Execute in order                                             │
│  • Emit per-command results                                     │
└─────────────────────────────────────────────────────────────────┘
```

### Non-Negotiable Invariants

1. **No side-effects** unless user explicitly says `run/execute/commit`
2. **No invented UUIDs** - All UUIDs from DB resolution or picker validation
3. **Picker validation** - `runbook_pick` entity_ids must match stored candidates
4. **Server-side ready gate** - `runbook_run` rejects if not all resolved
5. **DAG ordering** - Dependencies detected and reordered transparently

### MCP Tools

| Tool | Description |
|------|-------------|
| `runbook_stage` | Stage a command (parse + resolve, no execute) |
| `runbook_pick` | Select from ambiguous candidates |
| `runbook_show` | Show current runbook state |
| `runbook_preview` | Preview with readiness check |
| `runbook_remove` | Remove a staged command |
| `runbook_run` | Execute (explicit user confirmation required) |
| `runbook_abort` | Clear all staged commands |

### Key Files

| File | Purpose |
|------|---------|
| `migrations/054_staged_runbook.sql` | Schema (staged_runbook, staged_command, etc.) |
| `rust/src/repl/staged_runbook.rs` | Rust types (StagedRunbook, ResolutionStatus) |
| `rust/src/repl/repository.rs` | DB access layer |
| `rust/src/repl/resolver.rs` | EntityArgResolver (shorthand → UUID) |
| `rust/src/repl/dag_analyzer.rs` | Dependency detection, topological sort |
| `rust/src/repl/service.rs` | RunbookService (stage/pick/run/show/abort) |
| `rust/src/mcp/handlers/runbook.rs` | MCP tool handlers |
| `rust/config/verbs/runbook.yaml` | Verb definitions |

### Database Tables

| Table | Purpose |
|-------|---------|
| `staged_runbook` | Session-scoped runbook container |
| `staged_command` | Individual DSL commands with resolution status |
| `staged_command_entity` | Resolved entity footprint |
| `staged_command_candidate` | Picker candidates for ambiguous resolution |

---

## Client Group Research Integration (055)

> ✅ **IMPLEMENTED (2026-01-26)**: GLEIF import populates client_group_entity staging tables, then CBU creation with GLEIF role mapping.

**Problem Solved:** Research (GLEIF import) and onboarding (CBU creation) were coupled. Now they are separate concerns:
1. **Research phase**: GLEIF import → entities + `client_group_entity` staging
2. **Onboarding phase**: `cbu.create-from-client-group` → CBUs with role mapping

### Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  PHASE 1: RESEARCH (GLEIF Import)                                           │
│                                                                              │
│  (gleif.import-tree :entity-id <Allianz SE> :depth 3)                       │
│      │                                                                       │
│      ├─► entities table (all LEI entities)                                  │
│      ├─► entity_funds (with gleif_category: FUND, GENERAL)                  │
│      ├─► entity_parent_relationships (corporate hierarchy)                  │
│      └─► client_group_entity + client_group_entity_roles                    │
│              │                                                               │
│              └─► Roles: SUBSIDIARY, ULTIMATE_PARENT (corporate hierarchy)   │
└─────────────────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│  PHASE 2: ONBOARDING (CBU Creation)                                         │
│                                                                              │
│  (cbu.create-from-client-group :group-id <Allianz> :gleif-category "FUND")  │
│      │                                                                       │
│      ├─► Query client_group_entity WHERE gleif_category = 'FUND'            │
│      │                                                                       │
│      ├─► Create CBU per entity                                               │
│      │                                                                       │
│      └─► Assign CBU entity roles via GLEIF→CBU role mapping:                │
│              • GLEIF category FUND → ASSET_OWNER                            │
│              • Group role ULTIMATE_PARENT → HOLDING_COMPANY                 │
│              • Group role SUBSIDIARY → SUBSIDIARY                           │
│              • Optional: MANAGEMENT_COMPANY, INVESTMENT_MANAGER             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### GLEIF Role → CBU Role Mapping

| GLEIF Source | CBU Entity Role | Rationale |
|--------------|-----------------|-----------|
| `gleif_category = 'FUND'` | `ASSET_OWNER` | Fund entity owns its trading unit |
| `group_role = 'ULTIMATE_PARENT'` | `HOLDING_COMPANY` | Top of corporate hierarchy |
| `group_role = 'SUBSIDIARY'` | `SUBSIDIARY` | Corporate subsidiary |
| (default) | `ASSET_OWNER` | Safe default for fund onboarding |

### Key Verbs

| Verb | Description |
|------|-------------|
| `gleif.import-tree` | Import corporate hierarchy from GLEIF into entities + client_group_entity |
| `cbu.create-from-client-group` | Create CBUs from staged entities with role mapping |

### DSL Usage

```clojure
;; Phase 1: Research - import corporate hierarchy
(gleif.import-tree :entity-id <Allianz SE> :depth 3 :as @import)

;; Phase 2: Onboard - create CBUs for Luxembourg FUNDs
(cbu.create-from-client-group 
  :group-id <Allianz>
  :gleif-category "FUND"
  :jurisdiction-filter "LU"
  :manco-entity-id <AGI Holdings GmbH>)

;; Dry-run to preview
(cbu.create-from-client-group 
  :group-id <Allianz>
  :gleif-category "FUND"
  :dry-run true
  :limit 10)
```

### Key Tables

| Table | Purpose |
|-------|---------|
| `client_group_entity` | Staging: entities discovered via research |
| `client_group_entity_roles` | Staging: GLEIF roles (SUBSIDIARY, ULTIMATE_PARENT) |
| `entity_funds.gleif_category` | GLEIF entity classification (FUND, GENERAL) |
| `cbu_entity_roles` | Production: CBU membership roles |

### Key Files

| File | Purpose |
|------|---------|
| `migrations/055_client_group_research.sql` | Schema for client_group_entity_roles |
| `rust/src/gleif/enrichment.rs` | GLEIF tree import with relationship tracking |
| `rust/src/domain_ops/gleif_ops.rs` | `GleifImportTreeOp` - links to client_group_entity |
| `rust/src/domain_ops/cbu_ops.rs` | `CbuCreateFromClientGroupOp` - CBU creation with role mapping |
| `rust/config/verbs/cbu.yaml` | `create-from-client-group` verb definition |

---

## Verb Search Test Harness & Learning Accelerator

Comprehensive test harness for verifying semantic matching after teaching new phrases or tuning thresholds. Includes a **learning accelerator workflow** that uses LLM-generated phrases to rapidly improve verb discovery accuracy.

### Quick Start

```bash
# Run all verb search tests
cargo x test-verbs

# Specific test categories
cargo x test-verbs --taught       # Only taught phrase tests
cargo x test-verbs --hard-negatives  # Dangerous confusion detection
cargo x test-verbs --sweep        # Threshold calibration sweep

# Explore a specific query
cargo x test-verbs --explore "load the allianz book"
```

### Test Categories

| Category | Purpose |
|----------|---------|
| `taught` | Verify taught phrases match expected verbs |
| `session` | Session management verbs (load, unload, undo) |
| `cbu` | CBU lifecycle verbs |
| `hard_negative` | Dangerous confusions (delete vs archive) |
| `safety_first` | Verbs where ambiguity is acceptable |
| `edge` | Edge cases (garbage input, ambiguous queries) |

### Learning Accelerator Workflow

The test harness enables a closed-loop learning accelerator:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  LEARNING ACCELERATOR WORKFLOW                                               │
│                                                                              │
│  1. Run test harness → Identify failing phrases                             │
│         │                                                                    │
│         ▼                                                                    │
│  2. Harness outputs SQL for phrases that need teaching                      │
│         │                                                                    │
│         ▼                                                                    │
│  3. Execute SQL via agent.teach_phrases_batch()                             │
│         │                                                                    │
│         ▼                                                                    │
│  4. Run populate_embeddings (delta loading - fast)                          │
│         │                                                                    │
│         ▼                                                                    │
│  5. Re-run test harness → Validate improvement                              │
│         │                                                                    │
│         └──► Repeat until target accuracy achieved                          │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Commands:**

```bash
# Step 1: Run extended CBU scenarios to find gaps
DATABASE_URL="postgresql:///data_designer" \
  cargo test --features database --test verb_search_integration test_cbu_extended -- --ignored --nocapture

# Step 2: Harness outputs SQL like:
# SELECT * FROM agent.teach_phrases_batch('[{"phrase": "create a fund", "verb": "cbu.create"}, ...]'::jsonb, 'accelerated_learning');

# Step 3: Execute the SQL (or use scripts/teach_cbu_phrases.sql)
psql -d data_designer -f scripts/teach_cbu_phrases.sql

# Step 4: Populate embeddings (delta loading - only new patterns)
DATABASE_URL="postgresql:///data_designer" \
  cargo run --release --package ob-semantic-matcher --bin populate_embeddings

# Step 5: Re-run to validate
DATABASE_URL="postgresql:///data_designer" \
  cargo test --features database --test verb_search_integration test_cbu_extended -- --ignored --nocapture
```

**Example Results (CBU domain):**

| Round | Pass Rate | Top-1 Correct | Top-3 Contains | Ambiguity Rate |
|-------|-----------|---------------|----------------|----------------|
| Baseline | 15.2% | 13.2% | 32.5% | 66.9% |
| Round 1 | 61.6% | 58.9% | 88.7% | 40.4% |
| Round 2 | **78.1%** | **75.5%** | **95.4%** | **23.8%** |

Remaining failures are typically **legitimate disambiguation cases** where the system correctly asks for clarification.

### Extended Test Scenarios

The `cbu_phrase_scenarios.rs` module provides ~150 test scenarios across CBU verbs:

```rust
// rust/tests/cbu_phrase_scenarios.rs
pub fn all_cbu_scenarios() -> Vec<TestScenario> {
    let mut all = Vec::new();
    all.extend(cbu_create_scenarios());      // 34 scenarios
    all.extend(cbu_list_scenarios());        // 21 scenarios
    all.extend(cbu_assign_role_scenarios()); // 34 scenarios
    all.extend(cbu_parties_scenarios());     // 13 scenarios
    // ... etc
    all
}
```

**Adding scenarios for other domains:**
1. Create `<domain>_phrase_scenarios.rs` in `rust/tests/`
2. Define `TestScenario::matched()` for expected phrase→verb mappings
3. Use `TestScenario::safety_first()` for destructive operations
4. Add module to `verb_search_integration.rs`

### Threshold Sweep (Full Pipeline)

The sweep tests both retrieval AND decision thresholds:

```bash
# Quick sweep (3 combinations)
cargo x test-verbs --sweep

# Full sweep (180 combinations) - comprehensive exploration
DATABASE_URL="postgresql:///data_designer" \
  cargo test --features database --test verb_search_integration test_threshold_sweep_full -- --ignored --nocapture
```

**Output:**
```
sem_thr  fall_thr margin top1_hit%   ambig%    wrong no_match
   0.85     0.78   0.05      85.0       5.0        0        2
   0.88     0.78   0.05      80.0       8.0        0        4
   ...
✓ BEST (0 wrong): semantic=0.85 fallback=0.78 margin=0.05 → 85.0% top-1
```

### Safety-First Policy

For dangerous verb pairs (delete/archive, approve/submit), the harness uses `ExpectedOutcome::MatchedOrAmbiguous`:

- **Matched(correct)** → Pass (correct verb selected)
- **Ambiguous** → Pass (disambiguation UI triggered - safe)
- **Matched(wrong)** → FAIL (dangerous confusion)

This prevents optimizing for overconfident behavior on safety-critical verbs.

### Key Files

| File | Purpose |
|------|---------|
| `rust/tests/verb_search_integration.rs` | Main test harness with `VerbSearchTestHarness` |
| `rust/tests/cbu_phrase_scenarios.rs` | Extended CBU test scenarios (~150 phrases) |
| `rust/xtask/src/main.rs` | `cargo x test-verbs` command |
| `scripts/teach_cbu_phrases.sql` | Batch teaching script for CBU verbs |
| `migrations/044_agent_teaching.sql` | `agent.teach_phrases_batch()` function |

---

## Session Verbs

Session manages which CBUs are loaded. Focus/camera is client-side.

| Verb | Purpose |
|------|---------|
| `session.load-cbu` | Add single CBU to session |
| `session.load-jurisdiction` | Load all CBUs in jurisdiction |
| `session.load-galaxy` | Load all CBUs under apex entity |
| `session.unload-cbu` | Remove CBU from session |
| `session.clear` | Clear all CBUs |
| `session.undo` / `session.redo` | History navigation |
| `session.info` / `session.list` | Query session state |

### View Verbs (ESPER Navigation)

| Verb | Description |
|------|-------------|
| `view.universe` | All CBUs with optional filters |
| `view.book` | CBUs for commercial client |
| `view.cbu` | Focus single CBU (mode: trading/ubo) |
| `view.drill` | Drill into entity (within UBO taxonomy) |
| `view.surface` | Surface up from drill |
| `view.trace` | Follow threads (money/control/risk) |
| `view.xray` | Show hidden layers |
| `view.refine` | Add/remove filters |

---

## Navigation (Unified Intent Pipeline)

All user input—including navigation commands—goes through the unified `IntentPipeline`. Navigation phrases match to `view.*` and `session.*` verbs via semantic search. There is no separate ESPER path.

### Architecture

```
User: "zoom in" / "load the allianz book" / "show ownership"
    │
    ▼
IntentPipeline.process()
    │
    ├─ HybridVerbSearcher.search()
    │       │
    │       ├─ Exact match (learned phrases)
    │       └─ Semantic search (BGE embeddings)
    │
    ├─ Top match: view.drill / session.load-galaxy / control.build-graph
    │
    └─ LLM extracts args → DSL generated → Execute
```

### Key Files

| File | Purpose |
|------|---------|
| `rust/src/mcp/intent_pipeline.rs` | Unified intent processing |
| `rust/src/mcp/verb_search.rs` | `HybridVerbSearcher` - semantic + exact match |
| `rust/src/api/agent_service.rs` | `AgentService.process_chat()` entry point |
| `rust/config/verbs/view.yaml` | Navigation verbs (view.drill, view.surface, etc.) |
| `rust/config/verbs/session.yaml` | Session verbs (session.load-galaxy, etc.) |

### Navigation Verbs

| Verb | Purpose | Example Phrases |
|------|---------|-----------------|
| `view.universe` | Show all CBUs | "show universe", "zoom out to all" |
| `view.cbu` | Focus single CBU | "focus on this cbu", "show cbu details" |
| `view.drill` | Drill into entity | "drill down", "go deeper" |
| `view.surface` | Surface back up | "go back", "zoom out" |
| `session.load-galaxy` | Load CBUs under apex | "load the allianz book" |
| `session.load-cbu` | Load single CBU | "load cbu acme fund" |
| `session.clear` | Clear session | "clear", "start fresh" |
| `session.undo` / `session.redo` | History navigation | "undo", "redo" |

### Verb Search Thresholds (BGE Asymmetric Mode)

BGE uses **asymmetric retrieval**: queries get an instruction prefix (`"Represent this sentence for searching relevant passages: "`), targets don't. This produces **lower** similarity scores than symmetric (target-to-target) comparison:

- **Symmetric (target→target):** scores 0.6-1.0 (same-embedding comparison, like DB tests)
- **Asymmetric (query→target):** scores 0.5-0.8 (instruction-prefixed query vs raw target)

| Threshold | Value | Purpose |
|-----------|-------|---------|
| `fallback_threshold` | 0.55 | Retrieval cutoff for DB queries (must be low enough to retrieve candidates) |
| `semantic_threshold` | 0.65 | Decision gate for accepting match |
| `blocklist_threshold` | 0.80 | Collision detection |

> **Warning:** If you see "No matching verbs found" for queries that should match, check that asymmetric thresholds are being used. The old symmetric thresholds (0.70/0.78) will reject valid asymmetric matches.

### Adding Navigation Phrases

Add to the verb's `invocation_phrases` in YAML, then run `populate_embeddings`:

```yaml
# rust/config/verbs/view.yaml
drill:
  description: "Drill into entity detail"
  invocation_phrases:
    - "drill down"
    - "go deeper"
    - "zoom into"
    - "enhance"      # Add new phrase here
```

```bash
DATABASE_URL="postgresql:///data_designer" \
  cargo run --release --package ob-semantic-matcher --bin populate_embeddings
```

---

## Adding Verbs

> ⚠️ **Before writing verb YAML, read `docs/verb-definition-spec.md`**
> Serde structs are strict. Invalid YAML silently fails to load.

### Verb Behaviors

| Behavior | Execution | Use Case |
|----------|-----------|----------|
| `crud` | Generic executor, single DB operation | Simple CRUD |
| `plugin` | Custom Rust handler | Complex logic |
| `template` | Expands to multi-statement DSL | Workflows, macros |

### Plugin Verb Implementation (CustomOperation)

Plugin verbs require a Rust `CustomOperation` implementation. Use the `#[register_custom_op]` macro for automatic registration via the inventory crate.

```rust
use ob_poc_macros::register_custom_op;

#[register_custom_op]
pub struct MyDomainCreateOp;

#[async_trait]
impl CustomOperation for MyDomainCreateOp {
    fn domain(&self) -> &'static str { "my-domain" }
    fn verb(&self) -> &'static str { "create" }
    fn rationale(&self) -> &'static str { "Complex validation logic" }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        // Implementation
    }
}
```

**Key points:**
- Macro generates `inventory::submit!` for automatic registration at startup
- Domain/verb must match YAML definition with `behavior: plugin`
- `rationale()` documents why plugin (not CRUD) is needed
- Test coverage: `test_plugin_verb_coverage` verifies all YAML plugin verbs have ops

**Verify coverage:**
```bash
cargo test --lib -- test_plugin_verb_coverage
```

### Quick Example (CRUD)

```yaml
# rust/config/verbs/my_domain.yaml
domains:
  my_domain:
    verbs:
      create:
        description: "Create a record"
        behavior: crud
        crud:
          operation: insert
          table: my_table
          schema: ob-poc
        metadata:
          tier: intent
          source_of_truth: operational
        invocation_phrases:        # Required for LLM discovery
          - "create my thing"
          - "add new record"
        args:
          - name: name
            type: string
            required: true
            maps_to: name
```

### Template Verbs (Macros)

Templates are **first-class verbs** that expand to multi-statement DSL. They enable the LLM to generate complex workflows without training on DSL syntax.

**Key insight:** LLM does INTENT CLASSIFICATION (picks the right template), not DSL generation. Templates emit DSL source that flows through the normal pipeline.

```
User: "Research Aviva group and onboard their Lux funds"
       │
       ▼
┌─────────────────────────────────────────────────────────────┐
│  LLM: Intent Classification                                 │
│  verb: "onboarding.research-group"                         │
│  lookups: { root-entity: "Aviva" }                         │
│  params: { jurisdiction: "LU" }                            │
└─────────────────────────────────────────────────────────────┘
       │
       ▼
┌─────────────────────────────────────────────────────────────┐
│  Entity Resolution (same as singleton verbs)               │
│  "Aviva" → UUID via EntityGateway                          │
└─────────────────────────────────────────────────────────────┘
       │
       ▼
┌─────────────────────────────────────────────────────────────┐
│  Template Expansion → DSL Source                           │
│                                                             │
│  (gleif.import-tree :entity-id "uuid-..." :as @import)     │
│  (bulk.create-cbu :filter "type=FUND AND jur=LU")          │
│  (bulk.add-products :cbu-set @created :products [...])     │
└─────────────────────────────────────────────────────────────┘
       │
       ▼
┌─────────────────────────────────────────────────────────────┐
│  Normal DSL Pipeline (unchanged)                           │
│  Parse → Compile → Session → Execute                       │
└─────────────────────────────────────────────────────────────┘
```

**Template Verb Example:**

```yaml
domains:
  onboarding:
    description: "High-level onboarding workflows"
    
    invocation_hints:
      - "onboard"
      - "bulk"
      - "research group"
    
    verbs:
      research-group:
        description: "Import group from GLEIF and bulk onboard funds"
        behavior: template
        
        invocation_phrases:
          - "research a group"
          - "import corporate hierarchy"
          - "bulk onboard funds from GLEIF"
        
        metadata:
          tier: composite
          source_of_truth: external
          scope: global
          noun: group_onboarding
        
        args:
          - name: root-entity
            type: uuid
            required: true
            description: "Group apex entity"
            lookup:
              table: entities
              entity_type: entity
              schema: ob-poc
              search_key: name
              primary_key: entity_id
          
          - name: jurisdiction
            type: string
            required: false
            default: "LU"
            valid_values: [LU, IE, DE, UK, US]
          
          - name: products
            type: list
            required: false
            default: [EQUITY, FIXED_INCOME]
        
        # Template body - expands to DSL source
        template:
          body: |
            (gleif.import-tree :entity-id $root-entity :direction BOTH :as @import)
            (bulk.create-cbu 
              :filter "source=gleif_import AND type=FUND AND jurisdiction=$jurisdiction"
              :as @created_cbus)
            (bulk.add-products 
              :cbu-set @created_cbus 
              :products $products)
```

**Template vs Singleton:**

| Aspect | Singleton Verb | Template Verb |
|--------|---------------|---------------|
| Invocation | `(cbu.assign-role ...)` | `(onboarding.research-group ...)` |
| YAML location | Same (`config/verbs/`) | Same (`config/verbs/`) |
| Intent matching | `invocation_phrases` | `invocation_phrases` (identical) |
| Entity resolution | `lookup:` on args | `lookup:` on args (identical) |
| Execution | Single operation | Expands → multiple statements |

**Why templates are verbs:**
- Single vocabulary for LLM tool (verbs + templates together)
- Same entity resolution via `lookup:` config
- Same linting and governance rules apply
- User can invoke directly: `(onboarding.research-group :root <Aviva>)`

### Verify

```bash
cargo x verbs check   # YAML matches DB
cargo x verbs lint    # Tiering rules
```

### Entity Resolution in DSL

Entity references in DSL use the `<entity_name>` syntax and are resolved via EntityGateway before execution.

**3-Stage Compiler Model:**
```
DSL Source → Parse (Syntax) → Compile (Semantics) → Execute
                                    │
                                    ▼
                         Detect unresolved <entity> refs
                                    │
                                    ▼
                         Resolution Modal (if ambiguous)
                                    │
                                    ▼
                         Substitute UUIDs → Execute
```

**Resolution config comes from verb YAML:**
```yaml
args:
  - name: entity-id
    type: uuid
    lookup:
      table: entities
      entity_type: entity
      schema: ob-poc
      search_key: name                    # Simple key
      # OR composite s-expression:
      # search_key: "(search_name (nationality :selectivity 0.7) (date_of_birth :selectivity 0.95))"
      primary_key: entity_id
```

**Resolution flow (simplified 2-hop):**
1. Agent generates DSL with `<BlackRock>` entity ref
2. Parser produces AST with `EntityRef { value: "BlackRock", resolved_key: None }`
3. `ChatResponse.unresolved_refs` carries refs directly to UI (no sub-session API call)
4. UI opens resolution modal via `pending_unresolved_refs` in `AsyncState`
5. User selects → `resolved_key` populated with UUID
6. Execute with fully resolved AST

> **Note:** Legacy `needs_resolution_check` flag and `start_resolution()` API removed. Resolution state is inline in session, not a sub-session.

### Two Session Models (Intentionally Separate)

| Model | API | Purpose | Scope Storage |
|-------|-----|---------|---------------|
| `AgentSession` | `/api/session/*` | Full agent workflow (chat, DSL, execution) | `context.cbu_ids` |
| `CbuSession` | `/api/cbu-session/*` | Standalone scope navigation (load/undo/redo) | `state.cbu_ids` (HashSet) |

**Primary workflow:** Use DSL verbs (`session.load-galaxy`, etc.) which update `AgentSession.context.cbu_ids` after execution. This is the integrated path that feeds the viewport.

**Standalone REST:** The `/api/cbu-session/*` endpoints are independent and don't sync to `AgentSession`. Use them for direct REST integration without the agent/DSL layer.

> **Warning:** Don't mix the two models. Pick one per client integration.

### Session = Run Sheet = Viewport Scope

The session is the **single source of truth** that ties the REPL, DSL execution, and viewport together.

> **Full design:** `ai-thoughts/035-session-runsheet-viewport-integration.md`, `ai-thoughts/036-session-rip-and-replace.md`

**Unified Session Types (`rust/src/api/session.rs`):**
- `AgentSession` - Single source of truth for agent conversations
- `ServerRunSheet` - DSL statement ledger with per-statement status (replaces legacy `assembled_dsl`, `pending`, `executed_results`)
- `ServerRunSheetEntry` - Individual DSL statement with status, AST, bindings, affected entities

**RunSheet Status (`DslStatus`):**
- `Draft` - Just added, not yet validated
- `Ready` - Validated, ready to execute
- `Executing` - Currently running
- `Executed` - Successfully completed
- `Failed` - Execution error
- `Cancelled` - User cancelled (undo/clear)

**Unified Session Types (`rust/src/session/unified.rs`):**
- `UnifiedSession` - High-level session combining scope + view state
- `TargetUniverse` - Book/Galaxy/Jurisdiction scope
- `EntityScope` - Active CBU set being worked on  
- `StateStack` - Navigation history (back/forward/go-to-start/go-to-last)
- `ViewState` - Viewport rendering state
- `ResolutionState` - Inline resolution (not a sub-session)

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         SESSION = UNIVERSE OF WORK                          │
│                                                                             │
│  Session ID: abc-123                                                        │
│  Entity Scope: [CBU-1, CBU-2, CBU-3, ...]  ← Drives viewport               │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  Run Sheet (accumulated DSL statements)                              │   │
│  │                                                                       │   │
│  │  ✓ (gleif.import-tree :entity-id "aviva" :as @import)                │   │
│  │  ✓ (bulk.create-cbu :filter "..." :as @cbus) → [CBU-1..50]          │   │
│  │  ⏳ (bulk.add-products :cbu-set @cbus :products [...])               │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                              │                                              │
│                              ▼                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  Viewport shows session.entity_scope                                 │   │
│  │  Updates real-time as DSL executes                                   │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────────────┘
```

**The closed feedback loop:**
- **REPL Panel** shows run sheet with per-statement status
- **Session** tracks entity IDs created/modified by each statement
- **Viewport** subscribes to `session.entity_scope` and shows those CBUs/entities
- **User sees** graph update as DSL executes

**Key fields:**
- `session.entity_scope.cbu_ids` - CBUs being worked on (viewport universe)
- `session.run_sheet.entries[]` - DSL statements with status + affected entities
- `entry.status` - Draft/Ready/Executing/Executed/Failed/Cancelled
- `entry.bindings` - Symbols produced (@cbu, @created_cbus)
- `entry.affected_entities` - Entity UUIDs modified by this statement

**Multi-CBU Viewport Refresh (Scope Graph):**

Session scope is set via `session.load-*` verbs, then DSL execution modifies those CBUs, and the viewport refreshes:

```
1. User sets scope: "Load Allianz Lux book"
    │
    ▼
(session.load-galaxy :apex-entity-id "allianz-se")
    │
    └─► CbuSession.load_many([cbu-1..cbu-50])
            │
            └─► Synced to context.cbu_ids after execution
    │
    ▼
2. User modifies CBUs: "Add custody accounts to all"
    │
    ▼
(bulk.add-custody-account :cbu-set @cbus ...)
    │
    └─► entry.affected_entities = [entity-1..entity-200]
    │
    ▼
3. UI fetches combined graph:
    │
    ▼
GET /api/session/:id/scope-graph
    │
    ▼
MultiCbuGraphResponse {
    graph: CbuGraph,              // Combined graph for all 50 CBUs
    cbu_ids: Vec<Uuid>,           // CBUs in scope
    affected_entity_ids: Vec<Uuid> // 200 entities modified
}
    │
    ▼
Viewport updates with combined graph
```

**Key files:**
- `rust/src/api/graph_routes.rs` - `get_session_scope_graph()` endpoint
- `rust/src/api/agent_routes.rs` - `exec_ctx.take_pending_cbu_session()` sync to `context.cbu_ids`

---

## REPL Session & Phased Execution

> **V1 LEGACY (Superseded):** This section describes the V1 REPL phased execution model.
> The active implementation is the **V2 REPL Architecture** (see below), which replaces
> the state machine, session model, and execution pipeline described here.
> V1 types are retained for reference but are not used in the V2 code path.

> **Full design:** `ai-thoughts/034-repl-session-phased-execution.md`

The agent REPL session is a **state machine** that handles multi-CBU bulk operations:

```
EMPTY → SCOPED → TEMPLATED → GENERATED → PARSED → READY → EXECUTED
         │                                  │        │
         │ scope_dsl                        │        │ user confirms "Run?"
         │ + derived CBU set                │        │
         └──────────────────────────────────┘        │
                                             resolve loop (picker)
```

**Key concepts:**

| Concept | Description |
|---------|-------------|
| **Scope derivation** | "Allianz Lux book" → queries GROUP structure → derives CBU set |
| **Template × CBU set** | Single verb template expanded for each CBU in scope |
| **DAG phases** | Statements grouped by dependency depth (0 = creators, 1 = needs phase 0, etc.) |
| **Two tollgates** | Pre-compile (reorder) and pre-run (phase extraction) using same DAG |
| **Phased execution** | Execute phase 0, collect PKs, substitute into phase 1, repeat |

**Entity type hierarchy** (verb target determines attachment point):

```
Universe → Book → CBU → TradingProfile → Product
                     → Entity (roles)
                     → ISDA → Counterparty
                     → KycCase
```

**Session record stores:**
- `scope_dsl` - DSL that derived the scope
- `template_dsl` - Unpopulated verb template  
- `sheet.statements[]` - Populated DSL with `dag_depth` per statement
- `returned_pk` - PKs collected after each phase executes

**Failure handling:**
- Single transaction wrapping all phases
- Failure → rollback → no partial state
- Downstream statements marked SKIPPED (blocked by failed dependency)
- POC recovery: trash session, start fresh (idempotent verbs are safe)

**`@session_cbus` iteration:**
Template batch operations can use `@session_cbus` as source to iterate over all CBUs in session scope:
```clojure
(template.batch :id "add-custody" :source @session_cbus :bind-as "cbu_id" ...)
```

---

## REPL Pipeline Redesign (077)

> **V1 LEGACY (Superseded):** The original 077 redesign introduced an explicit state machine
> and command ledger. This has been **superseded by the V2 REPL Architecture** which implements
> a 7-state machine with pack-scoped intent resolution, ContextStack fold, and preconditions engine.

**V1 to V2 Migration:**

| V1 Concept | V2 Replacement |
|------------|----------------|
| `ReplState` (5 states) | `ReplStateV2` in `orchestrator_v2.rs` (7 states) |
| `LedgerEntry` | Runbook entries in `session_v2.rs` |
| `UserInput` enum | `UserInputV2` in `types_v2.rs` |
| `ClarifyingState` | Integrated into orchestrator state machine |
| `ReplSession` with ledger | `ReplSessionV2` with runbook fold |
| `ReplOrchestrator` | `ReplOrchestratorV2` with pack-scoped scoring |
| `/api/repl/session/:id/input` | `/api/repl/v2/session/:id/input` (V2 routes) |
| `HybridIntentMatcher` | `IntentService` + `search_with_context()` |

See **V2 REPL Architecture** section below for the active implementation.

---

## V2 REPL Architecture — Pack-Scoped Intent Resolution

> **Status:** ✅ Complete (2026-02-07) — Feature flag `vnext-repl` (enabled in web server)
> **Tests:** 320 unit tests + 5 golden corpus tests
> **Invariants:** P-1 through P-5, E-1 through E-8 — see `docs/INVARIANT-VERIFICATION.md`

**Problem Solved:** V1 REPL had no concept of "what the user is trying to accomplish" — every turn was an independent verb search. V2 introduces **packs** (journey templates) that scope intent resolution, a **ContextStack** (pure fold from runbook), and a **preconditions engine** that filters verbs by eligibility.

### 7-State Machine

```
┌────────────┐
│  ScopeGate │ ◄── No client group in scope
└─────┬──────┘
      │ scope established
      ▼
┌────────────────┐
│ JourneySelect  │ ◄── Multiple packs match
└─────┬──────────┘
      │ pack selected (or auto-selected)
      ▼
┌────────────┐     ┌────────────┐
│   InPack   │ ──► │ Clarifying │ ──► (back to InPack after resolution)
└─────┬──────┘     └────────────┘
      │ all steps proposed
      ▼
┌────────────┐     ┌────────────┐
│  Proposing │ ──► │RunbookEdit │ ──► (user edits steps)
└─────┬──────┘     └────────────┘
      │ user confirms
      ▼
┌────────────┐
│ Executing  │ ──► Completed / Parked (human gate)
└────────────┘
```

**States:** `ScopeGate`, `JourneySelection`, `InPack`, `Clarifying`, `SentencePlayback`, `RunbookEditing`, `Executing`

### ContextStack (7-Layer Fold from Runbook)

The ContextStack is a **pure fold** over executed runbook entries — never mutated directly. Only constructor: `ContextStack::from_runbook()`.

| Layer | Type | Purpose |
|-------|------|---------|
| `derived_scope` | `DerivedScope` | Session scope from executed entries (CBU IDs, client group) |
| `pack_context` | `PackContext` | Active pack (staged preferred over executed) |
| `template_hint` | `TemplateStepHint` | Next expected verb from pack template |
| `focus` | `FocusContext` | Pronoun resolution ("it", "that entity") |
| `recent` | `RecentContext` | Recent entity mentions for carry-forward |
| `exclusion_set` | `ExclusionSet` | Rejected candidates with 3-turn decay |
| `outcome_registry` | `OutcomeRegistry` | Execution results for `@N` outcome references |

Additional derived fields: `executed_verbs`, `staged_verbs`, `accumulated_answers` (Q&A from pack questions).

**Key Invariant (P-3):** Session state is a runbook fold. No mutable `client_context` or `journey_context` — these are deprecated write-only persistence bridges.

### Pack System

Packs are YAML journey templates that scope intent resolution to a workflow:

| Pack | File | Purpose |
|------|------|---------|
| `session-bootstrap` | `config/packs/session-bootstrap.yaml` | Initial scope setup |
| `book-setup` | `config/packs/book-setup.yaml` | Client book creation |
| `onboarding-request` | `config/packs/onboarding-request.yaml` | Full client onboarding |
| `kyc-case` | `config/packs/kyc-case.yaml` | KYC case management |

**PackManifest** defines: `id`, `name`, `description`, `entry_conditions`, `questions` (with `options_source`), `template` (ordered steps), `risk_policy`, `handoff_target`.

**PackRouter** selects pack: force-select > substring match > semantic match. Conversation-first design — question `options_source` values are suggestions, not gate constraints.

### Scoring Pipeline

```
User input
    │
    ▼
search_with_context(input, context_stack)     ← IntentService
    │
    ▼
apply_pack_scoring(candidates, pack_context)  ← Pack verb boost/penalty
    │
    ▼
apply_ambiguity_policy(candidates)            ← Margin-based disambiguation
    │
    ▼
dual_decode(candidates, pack_context)         ← Joint pack+verb scoring
    │
    ▼
VerbDecision: Clear | Ambiguous | NeedsContext | NoMatch
```

**Scoring Constants:**

| Constant | Value | Purpose |
|----------|-------|---------|
| `PACK_VERB_BOOST` | 0.10 | Verbs listed in active pack template |
| `PACK_VERB_PENALTY` | 0.05 | Verbs outside active pack |
| `TEMPLATE_STEP_BOOST` | 0.15 | Matching next expected template step |
| `DOMAIN_AFFINITY_BOOST` | 0.03 | Domain matching from FocusMode |
| `ABSOLUTE_FLOOR` | 0.55 | Minimum score cutoff |
| `AMBIGUITY_MARGIN` | 0.05 | Gap required between top candidates |

### Preconditions Engine

Parsed from YAML verb `lifecycle` field. Formats: `requires_scope:cbu`, `requires_prior:cbu.create`, `forbids_prior:cbu.delete`.

**EligibilityMode:**
- `Executable` — verb can execute now (strict: all preconditions met)
- `Plan` — verb can be planned for future execution (relaxed: scope only)

`filter_by_preconditions()` removes ineligible verbs from candidates and generates **"why not" suggestions** explaining what's missing (e.g., "cbu.assign-role requires a CBU in scope — try cbu.create first").

### FocusMode

5 variants derived from active pack + recent verbs:

| Variant | Trigger | Domain Affinity |
|---------|---------|-----------------|
| `KycCase` | kyc-case pack active | kyc.*, screening.* |
| `Proofs` | Document collection phase | document.*, requirement.* |
| `Trading` | Trading profile setup | trading-profile.*, custody.* |
| `CbuManagement` | Book setup pack | cbu.*, session.* |
| `General` | No pack or mixed activity | No boost |

Provides a soft domain affinity boost (`DOMAIN_AFFINITY_BOOST = 0.03`), not a hard filter.

### DecisionLog (Per-Turn Audit)

Every turn produces a `DecisionLog` entry with:
- Raw verb candidates (pre-scoring) with scores
- Reranked candidates (post-scoring) with pack adjustments
- `VerbDecision` (Clear/Ambiguous/NeedsContext/NoMatch)
- Entity resolutions with method (deterministic vs LLM)
- Arg extraction method
- Context summary (pack, scope, focus mode)
- Final DSL proposed
- Privacy: `raw_input` redacted in operational mode (only `input_hash` kept)

### Replay Tuner

CLI tool for iterating on scoring constants against the golden corpus:

```bash
# Run corpus against current constants
cargo x replay-tuner run

# Sweep parameter space
cargo x replay-tuner sweep

# Compare two parameter sets
cargo x replay-tuner compare --baseline defaults --candidate experimental

# Generate report
cargo x replay-tuner report
```

**Key file:** `rust/xtask/src/replay_tuner.rs`

### Golden Corpus

102 test entries across 8 YAML files in `rust/tests/golden_corpus/`:

| File | Entries | Coverage |
|------|---------|----------|
| `seed.yaml` | 26 | Core verb matching |
| `kyc.yaml` | 16 | KYC workflow |
| `preconditions.yaml` | 15 | Precondition gates |
| `edge_cases.yaml` | 13 | Edge cases |
| `book_setup.yaml` | 11 | Book setup pack |
| `bootstrap.yaml` | 7 | Bootstrap pack |
| `pack_switching.yaml` | 7 | Pack transitions |
| `error_recovery.yaml` | 7 | Error handling |

CI gate: `test_corpus_total_at_least_50` ensures corpus doesn't regress.

### Invariant Verification

13 invariants documented in `docs/INVARIANT-VERIFICATION.md`:

**Pipeline (P-1 through P-5):**
- P-1: ContextStack is a pure fold (no mutation)
- P-2: Pack scoring is additive (never replaces base score)
- P-3: Session state is runbook fold (no mutable context)
- P-4: Preconditions never add candidates
- P-5: ExclusionSet decays after 3 turns

**Engine (E-1 through E-8):**
- E-1: dual_decode handles cross-pack disambiguation
- E-2: recover_from_rejection filters and re-scores
- E-3: check_pack_handoff detects completion
- E-4 through E-8: Template expansion, entity resolution, sentence generation, etc.

### API Endpoints

| Endpoint | Purpose |
|----------|---------|
| `POST /api/repl/v2/session` | Create V2 REPL session |
| `GET /api/repl/v2/session/:id` | Get full session state |
| `POST /api/repl/v2/session/:id/input` | Unified input (all `UserInputV2` types) |
| `DELETE /api/repl/v2/session/:id` | Delete session |
| `POST /api/repl/v2/signal` | External system completion signals |

**UserInputV2 types:** `Message`, `Confirm`, `Reject`, `Edit`, `Command`, `SelectPack`, `SelectVerb`, `SelectEntity`

**ReplResponseKindV2 types:** `ScopeRequired`, `JourneyOptions`, `Question`, `SentencePlayback`, `RunbookSummary`, `Clarification`, `Executed`, `Parked`, `StepProposals`, `Info`, `Prompt`, `Error`

### Key Files (33 files)

| File | Purpose |
|------|---------|
| **State Machine & Session** | |
| `rust/src/repl/orchestrator_v2.rs` | 7-state machine dispatcher (174K) |
| `rust/src/repl/session_v2.rs` | `ReplSessionV2` with runbook (19K) |
| `rust/src/repl/types_v2.rs` | `ReplStateV2`, `UserInputV2` types (11K) |
| `rust/src/repl/response_v2.rs` | `ReplResponseV2` types (9K) |
| **Intent & Scoring** | |
| `rust/src/repl/intent_service.rs` | 5-phase verb matching pipeline (26K) |
| `rust/src/repl/scoring.rs` | Pack scoring + ambiguity policy (29K) |
| `rust/src/repl/preconditions.rs` | Precondition engine + "why not" (18K) |
| `rust/src/repl/decision_log.rs` | Per-turn audit trail (32K) |
| `rust/src/repl/verb_config_index.rs` | In-memory verb metadata (26K) |
| **Context & Runbook** | |
| `rust/src/repl/context_stack.rs` | 7-layer ContextStack fold (81K) |
| `rust/src/repl/runbook.rs` | Runbook entries + execution (66K) |
| `rust/src/repl/bootstrap.rs` | ScopeGate bootstrap logic (15K) |
| **Proposal & Execution** | |
| `rust/src/repl/proposal_engine.rs` | Deterministic step proposals (29K) |
| `rust/src/repl/sentence_gen.rs` | Deterministic sentence templating (17K) |
| `rust/src/repl/deterministic_extraction.rs` | Pattern-based arg extraction (30K) |
| `rust/src/repl/executor_bridge.rs` | Bridge to DSL executor (5K) |
| `rust/src/repl/entity_resolution.rs` | Entity resolution pipeline (17K) |
| **Journey / Packs** | |
| `rust/src/journey/pack.rs` | `PackManifest`, `PackTemplate` (19K) |
| `rust/src/journey/router.rs` | `PackRouter` — pack selection (19K) |
| `rust/src/journey/template.rs` | Template expansion + provenance (23K) |
| `rust/src/journey/playback.rs` | Pack summary + chapter gen (10K) |
| `rust/src/journey/handoff.rs` | Context forwarding between packs (2K) |
| **API & Infrastructure** | |
| `rust/src/api/repl_routes_v2.rs` | V2 HTTP endpoints |
| `rust/src/repl/events.rs` | Event system (15K) |
| `rust/src/repl/service.rs` | Service layer (32K) |
| `rust/src/repl/session_repository.rs` | DB persistence (9K) |
| **Config** | |
| `rust/config/packs/session-bootstrap.yaml` | Bootstrap pack |
| `rust/config/packs/book-setup.yaml` | Book setup pack |
| `rust/config/packs/onboarding-request.yaml` | Onboarding pack |
| `rust/config/packs/kyc-case.yaml` | KYC case pack |
| **Test** | |
| `rust/tests/golden_corpus/*.yaml` | 102 test entries (8 files) |
| `rust/tests/golden_corpus_test.rs` | Corpus runner + CI gate |
| `rust/xtask/src/replay_tuner.rs` | Replay tuner CLI |

---

## BPMN-Lite Durable Orchestration Service

> ✅ **IMPLEMENTED (2026-02-10)**: Standalone Rust service for durable workflow orchestration. BPMN XML → Verified IR → Bytecode → Fiber VM. 123 core tests + 6 integration tests + gRPC smoke test. Includes race semantics (Phase 2), non-interrupting boundary events + timer cycles (Phase 2A), cancel/ghost signal protection (Phase 3), terminate end events + error boundary routing + bounded loops (Phase 5), inclusive (OR) gateways (Phase 5A), and authoring pipeline with verb contracts, BPMN XML export, and template publish lifecycle (Phases B-D).

**Problem Solved:** ob-poc has a DSL + verb runtime for deterministic, short-running work and a runbook/REPL model for auditable execution. It lacks **durable orchestration** — the ability to park a workflow for days/weeks (waiting for documents, human approvals, timers) and resume deterministically. BPMN-Lite fills this gap as a standalone gRPC service.

### Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    BPMN-LITE SERVICE                                          │
│                                                                              │
│  BPMN XML (.bpmn)                                                           │
│         │                                                                    │
│         ▼                                                                    │
│  Parser (quick-xml) → IRGraph (petgraph)                                    │
│         │                                                                    │
│         ▼                                                                    │
│  Verifier (structural checks: single start, reachable, paired gateways)     │
│         │                                                                    │
│         ▼                                                                    │
│  Lowering → Bytecode (Vec<Instr>, 23-opcode ISA)                           │
│         │                                                                    │
│         ▼                                                                    │
│  Fiber VM (tick-based executor)                                             │
│         │   - ExecNative parks fiber + enqueues job                         │
│         │   - CompleteJob resumes fiber with payload                        │
│         │   - Fork/Join for parallel paths                                  │
│         │   - WaitFor/WaitUntil/WaitMsg for timers/messages                │
│         │   - Race semantics (Msg vs Timer arms, boundary events)          │
│         │   - Non-interrupting fire (spawns child fiber, main stays)       │
│         │   - Timer cycles (R<n>/PT<dur>, exhaustion revert)               │
│         │   - Cancel/ghost signal protection (3 guards)                    │
│         │   - Terminate end events (kill all sibling fibers)               │
│         │   - Error boundary routing (BusinessRejection → catch path)      │
│         │   - Bounded retry loops (IncCounter/BrCounterLt)                 │
│         │   - Inclusive (OR) gateway (ForkInclusive/JoinDynamic)            │
│         │                                                                    │
│         ▼                                                                    │
│  BpmnLiteEngine (facade wrapping compiler + VM + store)                     │
│         │                                                                    │
│         ▼                                                                    │
│  gRPC Server (tonic 0.12) — 9 RPCs                                         │
│         │                                                                    │
│         ▼                                                                    │
│  ob-poc connects as job worker via gRPC (Phase B — complete)               │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Key Design Constraints

- **Standalone workspace** — `bpmn-lite/` at repo root, NOT inside `rust/` or as an ob-poc crate
- **gRPC is the only boundary** — no shared crates beyond protobuf-generated types
- **Hollow orchestration** — BPMN-Lite orchestrates control flow; all domain work happens in ob-poc verb handlers via the job worker protocol
- **Two-namespace payload** — `domain_payload` (opaque canonical JSON + SHA-256 hash) never parsed by VM; `orch_flags` (flat primitives) for branching only
- **Dual store** — `MemoryStore` (default, no deps) or `PostgresProcessStore` (feature-gated `postgres`, `--database-url` CLI arg / `DATABASE_URL` env var)

### 23-Opcode ISA

| Group | Instructions | Behavior |
|-------|-------------|----------|
| Control flow | `Jump`, `BrIf`, `BrIfNot` | Manipulate pc; BrIf/BrIfNot pop stack |
| Stack ops | `PushBool`, `PushI64`, `Pop` | Push/pop on fiber.stack |
| Flags | `LoadFlag`, `StoreFlag` | Read/write instance.flags; StoreFlag emits FlagSet event |
| Work | `ExecNative` | Park fiber + enqueue job (key v0.9 change) |
| Concurrency | `Fork`, `Join` | Fork spawns child fibers; Join increments barrier + parks |
| Waits | `WaitFor`, `WaitUntil`, `WaitMsg` | Park fiber in appropriate WaitState |
| Race | `WaitAny`, `CancelWait` | WaitAny races N arms (timer/msg/internal); CancelWait cleans up losers |
| Bounded loops | `IncCounter`, `BrCounterLt` | IncCounter increments loop counter; BrCounterLt branches if counter < limit |
| Inclusive gateway | `ForkInclusive`, `JoinDynamic` | ForkInclusive evaluates condition_flags, spawns fibers for truthy branches; JoinDynamic waits for dynamic count |
| Lifecycle | `End`, `EndTerminate`, `Fail` | End removes fiber; EndTerminate kills all sibling fibers + ends process; Fail creates incident |

### ExecNative (Job Worker Protocol)

1. Derive deterministic `job_key` from `(instance_id, service_task_id, pc)`
2. Check dedupe cache → if hit, apply cached completion, advance pc
3. If miss: emit `JobActivated` event, enqueue `JobActivation`, park fiber in `WaitState::Job`
4. Fiber does NOT advance pc — resumes when `CompleteJob` arrives
5. `CompleteJob` validates `domain_payload_hash`, merges `orch_flags`, advances pc

### gRPC API (9 RPCs)

| RPC | Purpose |
|-----|---------|
| `Compile` | BPMN XML → verified bytecode, returns `bytecode_version` hash |
| `StartProcess` | Create process instance from compiled program |
| `Signal` | Send named message to waiting process (correlation) |
| `Cancel` | Cancel all fibers, mark process Cancelled |
| `Inspect` | Snapshot of process state (fibers, waits, flags) |
| `ActivateJobs` | Server-streaming: dequeue jobs for worker (long-poll with timeout) |
| `CompleteJob` | Worker returns result (payload + hash + flags) |
| `FailJob` | Worker reports failure (creates incident) |
| `SubscribeEvents` | Server-streaming: tailing event log (polls until terminal event) |

### Workspace Structure

```
bpmn-lite/                          # Standalone workspace (repo root sibling to rust/)
├── Cargo.toml                      # Workspace manifest
├── rust-toolchain.toml             # Pinned to 1.91 (matches ob-poc)
├── Dockerfile                      # Multi-stage (rust:1.84-bookworm → debian:bookworm-slim)
├── .dockerignore
├── .gitignore
├── bpmn-lite-core/
│   ├── Cargo.toml
│   ├── migrations/                 # 12 SQL files (001-012), run by sqlx::migrate!()
│   ├── src/
│   │   ├── lib.rs                  # Module exports
│   │   ├── types.rs                # Value, Instr, Fiber, ProcessInstance, Job*, CompiledProgram
│   │   ├── events.rs               # RuntimeEvent enum (28 variants)
│   │   ├── store.rs                # ProcessStore trait (29 async methods)
│   │   ├── store_memory.rs         # MemoryStore (RwLock<HashMap> implementation)
│   │   ├── store_postgres.rs       # PostgresProcessStore (#[cfg(feature = "postgres")])
│   │   ├── vm.rs                   # tick_fiber() executor + CompleteJob/FailJob handlers
│   │   ├── engine.rs               # BpmnLiteEngine facade
│   │   └── compiler/
│   │       ├── mod.rs
│   │       ├── ir.rs               # IRGraph, IRNode, IREdge (petgraph)
│   │       ├── verifier.rs         # Structural verification (7 checks)
│   │       ├── lowering.rs         # IR → bytecode with debug_map
│   │       └── parser.rs           # BPMN XML → IR (quick-xml)
│   └── tests/
│       └── fixtures/
│           └── kyc-open-case.bpmn  # Test fixture: KYC workflow
├── bpmn-lite-server/
│   ├── Cargo.toml
│   ├── build.rs                    # tonic-build for proto compilation
│   ├── proto/
│   │   └── bpmn_lite/v1/
│   │       └── bpmn_lite.proto     # Full proto definition (9 RPCs)
│   ├── src/
│   │   ├── lib.rs                  # Re-exports grpc::proto for integration tests
│   │   ├── main.rs                 # gRPC server startup, conditional store (--database-url)
│   │   └── grpc.rs                 # Handler implementations delegating to Engine
│   └── tests/
│       └── integration.rs          # 6 integration tests + 1 gRPC smoke test (#[ignore])
```

### Core Types

| Type | Description |
|------|-------------|
| `Value` | Stack value: `Bool(bool)`, `I64(i64)`, `Str(u32)`, `Ref(u32)` |
| `Instr` | 23-opcode enum |
| `Fiber` | fiber_id, pc, stack, regs[8], wait state |
| `ProcessInstance` | instance_id, bytecode, domain_payload, hash, flags, state |
| `WaitState` | Running, Timer, Msg, Job{job_key}, Join, Race{race_id, timer, cycle}, Incident |
| `CycleSpec` | Timer cycle config: `interval_ms`, `max_fires` |
| `WaitArm` | Race arm: `Timer{deadline, resume_at, interrupting, cycle}` or `Msg{name, corr_key, resume_at}` |
| `RacePlanEntry` | Race plan arm with `boundary_element_id` for BPMN element tracing |
| `InclusiveBranch` | OR-gateway branch: `condition_flag` (Option<FlagKey>) + `target` (Addr) |
| `CompiledProgram` | bytecode_version (SHA-256), program, debug_map, join_plan, wait_plan, race_plan |
| `RuntimeEvent` | 28 variants: InstanceStarted, FiberSpawned, JobActivated, JobCompleted, RaceRegistered, RaceWon, BoundaryFired, TimerCycleIteration, TimerCycleExhausted, WaitCancelled, SignalIgnored, Terminated, ErrorRouted, CounterIncremented, InclusiveForkTaken, etc. |
| `ProcessStore` | Trait with 29 async methods (instances, fibers, joins, dedupe, jobs, programs, events, cancel) |

### Engine: `tick_instance` vs `run_instance`

The `BpmnLiteEngine` has two methods for advancing fibers:

| Method | Dequeues Jobs? | Used By |
|--------|---------------|---------|
| `tick_instance()` | No — leaves jobs in queue | gRPC handlers (`start_process`, `complete_job`, `signal`) |
| `run_instance()` | Yes — calls `tick_instance()` then dequeues | In-process tests and convenience callers |

gRPC handlers use `tick_instance` so that jobs remain in the store queue for the `ActivateJobs` RPC to deliver to external workers. The `complete_job` gRPC handler also calls `tick_instance` after resuming the fiber, so it can advance to the next instruction (End, or another ExecNative).

### Running the Service

```bash
# Native (via xtask, from rust/ directory)
cargo x bpmn-lite start            # Release build + start background (port 50051)
cargo x bpmn-lite start -p 50055   # Custom port
cargo x bpmn-lite status           # Show native + Docker status
cargo x bpmn-lite stop             # Stop native server

# Build and run directly (foreground, MemoryStore)
cd bpmn-lite && cargo run -p bpmn-lite-server
# Server starts on [::]:50051

# Build and run with PostgresProcessStore (migrations auto-applied)
cd bpmn-lite && cargo run -p bpmn-lite-server --features postgres -- --database-url postgresql:///data_designer

# Build/test only
cargo x bpmn-lite build            # Debug build
cargo x bpmn-lite build --release  # Release build
cargo x bpmn-lite test             # Run all 71 tests (+ 1 ignored gRPC smoke)
cargo x bpmn-lite clippy           # Lint

# Postgres integration tests (15 tests, require running Postgres)
cd bpmn-lite && DATABASE_URL="postgresql:///data_designer" \
  cargo test --features postgres -p bpmn-lite-core -- --ignored

# Docker
cargo x bpmn-lite docker-build     # Build image
cargo x bpmn-lite deploy           # Docker build + compose up (port 50053)

# Docker directly
docker build -t bpmn-lite ./bpmn-lite
docker run -p 50051:50051 bpmn-lite

# Docker Compose (port 50053 on host → 50051 in container)
docker compose up bpmn-lite
```

### Test Coverage (71 core + 6 integration + 15 postgres + 1 ignored smoke test)

| Phase | Tests | Coverage |
|-------|-------|----------|
| A2: MemoryStore | 7 | Instance/fiber/join/dedupe/job queue/event log/payload history |
| A3: VM (core) | 7 | Linear flow, flag round-trip, dedupe, hash validation, events, FlagSet |
| A3: VM (race) | 4 | Race msg wins, timer wins, replay after race, duplicate signal noop |
| A3: VM (boundary) | 3 | Job completes before timer, timer fires before job, verifier rejects invalid |
| A4: Compiler | 7 | IR lowering, XOR/parallel gateways, verifier rejects, end-to-end |
| A5: Parser | 6 | Minimal BPMN, task_type extraction, unsupported elements, full pipeline, boundary timer parse+lower, ISO cycle parse |
| A5: Full pipeline | 2 | kyc-open-case.bpmn end-to-end, fixture parsing |
| A6: Engine | 1 | Engine full lifecycle |
| Phase 2A: Non-interrupting | 5 | Spawns child fiber, cycle fires multiple, cycle exhaustion revert, job completes before timer, verifier rejects cycle+interrupting |
| Phase 3: Cancel | 5 | Complete-after-cancel, signal-after-complete, signal-no-match, duplicate-complete, job-purge-on-cancel |
| Phase 5.1: Terminate | 4 | EndTerminate kills siblings, terminate after parallel fork, terminate in sequential flow, Terminated event emitted |
| Phase 5.2: Error routing | 5 | BusinessRejection routes to error boundary, unmatched error creates incident, error routing with orch_flags, error after parallel fork, nested error boundaries |
| Phase 5.3: Bounded loops | 5 | Counter increment + branch, loop exits at limit, counter reset across epochs, loop with service task, counter events emitted |
| Phase 5A: Inclusive gateway | 6 | All branches taken, subset branches (condition_flag), single branch, default-only, dynamic join count, InclusiveForkTaken event |
| Integration | 6 | Full lifecycle, two-task, cancel, fail, compile error, hash integrity |
| Phase 4: PostgresProcessStore | 15 (`#[ignore]`) | Instance/fiber/join/dedupe/job/event/payload/program/dead-letter/incident round-trips, instance updates, teardown, concurrent dequeue, engine smoke, cancel_jobs_for_instance |
| gRPC smoke | 1 (`#[ignore]`) | Over-the-wire: Compile → Start → Inspect → ActivateJobs → CompleteJob → Inspect(COMPLETED) → SubscribeEvents |

The gRPC smoke test requires a running server. Run with:
```bash
# Against native server (port 50051)
cargo x bpmn-lite start
cd bpmn-lite && BPMN_LITE_URL=http://127.0.0.1:50051 cargo test --test integration test_grpc_smoke -- --ignored

# Against Docker container (port 50053)
cargo x bpmn-lite deploy
cd bpmn-lite && BPMN_LITE_URL=http://127.0.0.1:50053 cargo test --test integration test_grpc_smoke -- --ignored
```

### Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| `tonic` | 0.12 | gRPC server |
| `prost` | 0.13 | Protobuf codegen |
| `petgraph` | 0.6 | IR graph representation |
| `quick-xml` | 0.36 | BPMN XML parser |
| `tokio` | 1 | Async runtime |
| `uuid` | 1 (v7) | Instance/fiber IDs |
| `sha2` | 0.10 | Payload hash, bytecode version |
| `serde`/`serde_json` | 1 | Serialization |
| `tracing` | 0.1 | Structured logging |
| `sqlx` | 0.8 | PostgreSQL async driver (optional, `postgres` feature) |
| `chrono` | 0.4 | Timestamp conversion (optional, `postgres` feature) |

### Docker Compose

```yaml
# In docker-compose.yml at repo root
bpmn-lite:
  build:
    context: ./bpmn-lite
    dockerfile: Dockerfile    # Builds with --features postgres
  container_name: bpmn-lite
  ports:
    - "50053:50051"  # gRPC (50051 inside container, 50053 on host)
  environment:
    RUST_LOG: info
    DATABASE_URL: postgresql://localhost@host.docker.internal:5432/data_designer
  extra_hosts:
    - "host.docker.internal:host-gateway"
```

### Race Semantics (Phase 2)

Race semantics model BPMN constructs where multiple events compete (e.g., service task completion vs boundary timer). The first arm to fire wins.

**WaitState::Race fields:**

| Field | Type | Purpose |
|-------|------|---------|
| `race_id` | `RaceId` | Links to `race_plan` in CompiledProgram |
| `timer_deadline_ms` | `Option<u64>` | Absolute deadline for timer arm |
| `job_key` | `Option<String>` | Preserved from Job state during boundary promotion |
| `interrupting` | `bool` | If true, timer resolves race; if false, spawns child fiber |
| `timer_arm_index` | `Option<usize>` | Computed index into race_plan arms (not hardcoded) |
| `cycle_remaining` | `Option<u32>` | Remaining cycle fires (None = no cycle) |
| `cycle_fired_count` | `u32` | How many times timer has fired (for event numbering) |

**Engine promotion:** When a fiber is in `WaitState::Job` and the compiled program has a `race_plan` entry for that pc, the engine promotes it to `WaitState::Race` — preserving the job_key and adding the timer deadline.

### Non-Interrupting Boundary Events + Timer Cycles (Phase 2A)

**Non-interrupting fire** (5 non-negotiable constraints):
1. Does NOT resolve the race — spawns a child fiber at the escalation path, main fiber stays in Race
2. `timer_arm_index` computed from arms vec, never hardcoded
3. `cycle_fired_count` used for iteration numbering in events
4. Real BPMN element IDs emitted from `RacePlanEntry.boundary_element_id`
5. No `spawned_fibers` stored in `WaitState::Race`

**Timer cycles** use ISO 8601 `R<n>/PT<duration>` format:
- `R3/PT1H` = fire 3 times, 1 hour apart
- After each fire: spawn child fiber, decrement `cycle_remaining`, increment `cycle_fired_count`
- On exhaustion: emit `TimerCycleExhausted`, revert fiber from Race back to plain `WaitState::Job`

**New events:** `BoundaryFired`, `TimerCycleIteration`, `TimerCycleExhausted`

**Verifier rule:** Cycle timers MUST be non-interrupting (`cancelActivity="false"`). Cycle + interrupting = compile error.

### Cancel & Ghost Signal Protection (Phase 3)

Three guards in `engine.complete_job()` prevent ghost signals:

| Guard | Condition | Response |
|-------|-----------|----------|
| Instance not Running | `state != Running` | Emit `SignalIgnored`, return Ok |
| Fiber not found | No fiber for job_key | Emit `SignalIgnored`, return Ok |
| Fiber not waiting | `wait_state` mismatch | Emit `SignalIgnored`, return Ok |

**Cancel improvements:**
- `cancel()` now emits `WaitCancelled` for each active fiber before termination
- `cancel_jobs_for_instance()` purges pending jobs from the queue
- `signal()` checks instance state before matching, emits `SignalIgnored` on no-match

**New events:** `WaitCancelled`, `SignalIgnored`

### Terminate End Events (Phase 5.1)

`EndTerminate` immediately kills all sibling fibers in the same process instance. Unlike `End` (which removes only the executing fiber and lets others continue), `EndTerminate` is a hard stop.

**VM behavior:**
1. Fiber executes `EndTerminate` → returns `TickOutcome::Terminated` (does NOT delete fibers itself)
2. Engine receives `Terminated` outcome → deletes ALL fibers, sets instance state to `Completed`
3. Emits `RuntimeEvent::Terminated { at, fiber_id }` identifying which fiber triggered termination

**Use case:** BPMN terminate end events (e.g., "critical failure — abort entire process regardless of parallel branches").

### Error Boundary Routing (Phase 5.2)

Error boundary events catch `BusinessRejection` failures from service tasks and route to escalation/recovery paths instead of creating incidents.

**ErrorClass taxonomy:**

| Class | Behavior |
|-------|----------|
| `Transient` | Always creates incident (infra/network errors) |
| `ContractViolation` | Always creates incident (programmer error) |
| `BusinessRejection { rejection_code }` | Checks `error_route_map` for matching catch boundary |

**Routing logic in `engine.fail_job()`:**
1. Only `BusinessRejection` errors check routes — `Transient` and `ContractViolation` always create incidents
2. `error_route_map` maps `service_task_pc → Vec<ErrorRoute>` (compiled from BPMN `boundaryEvent` with `errorEventDefinition`)
3. Routes match by `error_code`: specific code match first, then catch-all (`error_code: None`)
4. On match: fiber jumps to `resume_at`, emits `ErrorRouted { job_key, error_code, boundary_id, resume_at }`
5. On no match: falls through to incident creation (existing behavior)

**Key types:**
```rust
pub struct ErrorRoute {
    pub error_code: Option<String>,    // None = catch-all
    pub resume_at: Addr,               // Escalation path entry point
    pub boundary_element_id: String,   // BPMN element ID for tracing
}
```

### Bounded Retry Loops (Phase 5.3)

Two new opcodes enable bounded loops without risk of infinite cycling:

| Opcode | Behavior |
|--------|----------|
| `IncCounter { counter_id }` | Increments `instance.counters[counter_id]`, emits `CounterIncremented` |
| `BrCounterLt { counter_id, limit, target }` | If `counter < limit`, jump to `target`; else fall through |

**Use case:** Retry patterns — "retry up to 3 times, then escalate":
```
IncCounter(0)          ; bump retry count
BrCounterLt(0, 3, L1) ; if retries < 3, go to L1 (retry)
Jump(L2)               ; else go to L2 (escalation)
```

**Counters:**
- Stored in `ProcessInstance.counters: BTreeMap<u32, u32>`
- Epoch-tracked via `loop_epoch` for counter reset across loop iterations
- `CounterIncremented { counter_id, new_value, loop_epoch }` event emitted on each increment

### Inclusive (OR) Gateway (Phase 5A)

Inclusive gateways evaluate condition flags at fork time and spawn fibers only for branches whose conditions are truthy. The join count is dynamic — set at fork time based on how many branches were actually taken.

**Two opcodes:**

| Opcode | Behavior |
|--------|----------|
| `ForkInclusive { branches, join_id, default_target }` | Evaluates each `InclusiveBranch.condition_flag` against `instance.flags`. Spawns fibers for truthy branches. Sets `instance.join_expected[join_id]` to actual count. If no conditions match and `default_target` is set, spawns single fiber there. |
| `JoinDynamic { id, next }` | Same barrier semantics as `Join`, but reads expected count from `instance.join_expected[id]` instead of compile-time constant |

**InclusiveBranch:**
```rust
pub struct InclusiveBranch {
    pub condition_flag: Option<FlagKey>,  // None = unconditional (always taken)
    pub target: Addr,                     // Fiber spawn target
}
```

**Semantics:**
- `condition_flag: None` → always taken (unconditional branch)
- `condition_flag: Some(key)` → taken if `instance.flags[key]` is truthy (`Bool(true)` or `I64(n)` where n != 0)
- At least one branch must be taken (enforced at runtime — if zero match and no default, returns error)
- `InclusiveForkTaken { gateway_id, branches_taken, join_id, expected }` event emitted

**New events:** `Terminated`, `ErrorRouted`, `CounterIncremented`, `InclusiveForkTaken`

### PostgresProcessStore (Phase 4)

> ✅ **IMPLEMENTED (2026-02-10)**: Feature-gated PostgreSQL-backed ProcessStore so workflows survive restarts.

**Feature gate:** All Postgres code behind `#[cfg(feature = "postgres")]`. Never import sqlx in non-gated code.

**Non-negotiable constraints:**
- **Runtime queries only** — `sqlx::query()` / `sqlx::query_as()`, NOT `sqlx::query!()` macro
- **Single SQL round-trip per trait method** — no method issues more than one query
- **BYTEA for `[u8; 32]`** — bind as `&hash[..]`, load as `Vec<u8>` → `.try_into::<[u8; 32]>()?`
- **JSONB for compound fields** — flags, counters, join_expected, state, wait_state, stack, regs, orch_flags, error_class, completion, event
- **`FOR UPDATE SKIP LOCKED`** for `dequeue_jobs` — CTE pattern for concurrent safety
- **Fiber regs padding** — deserialize JSONB as `Vec<Value>`, pad to 8 with `Value::Bool(false)` if short

**12 migrations** (`bpmn-lite-core/migrations/001-012`):
`process_instances`, `fibers`, `join_barriers`, `dedupe_cache`, `job_queue`, `compiled_programs`, `dead_letter_queue`, `event_sequences`, `event_log`, `payload_history`, `incidents`, `updated_at_trigger`

**29 trait methods by group:**

| Group | Methods | SQL Pattern |
|-------|---------|-------------|
| Instance (5) | save/load/update_state/update_flags/update_payload | Upsert, SELECT, UPDATE |
| Fibers (5) | save/load/load_all/delete/delete_all | Upsert, SELECT, DELETE |
| Joins (3) | arrive/reset/delete_all | Upsert+increment, reset, DELETE |
| Dedupe (2) | get/put | SELECT, Upsert |
| Jobs (4) | enqueue/dequeue/ack/cancel_for_instance | INSERT, CTE+SKIP LOCKED, DELETE, DELETE RETURNING |
| Programs (2) | store/load | INSERT ON CONFLICT DO NOTHING, SELECT |
| Dead-letter (2) | put/take | Upsert with computed expires_at, DELETE WHERE expires_at > now() |
| Events (2) | append/read | Atomic CTE (seq alloc + insert), SELECT WHERE seq >= |
| Payload history (2) | save/load | INSERT ON CONFLICT DO NOTHING, SELECT |
| Incidents (2) | save/load | INSERT, SELECT ORDER BY created_at |

**Server wiring:** `--database-url <url>` CLI arg or `DATABASE_URL` env var. Without either, falls back to MemoryStore. Migrations auto-applied on startup via `sqlx::migrate!()`.

**15 integration tests** (all `#[ignore]`, require running Postgres):
T-PG-1 through T-PG-15 covering instance/fiber/join/dedupe/job/event/payload/program/dead-letter/incident round-trips, instance updates, teardown, concurrent dequeue, full engine smoke, and cancel_jobs_for_instance.

### Authoring Pipeline (Phases B-D)

> ✅ **IMPLEMENTED (2026-02-10)**: Verb contracts + static lint rules, BPMN 2.0 XML export with round-trip verification, template registry with atomic publish lifecycle. 30 authoring tests (10 per phase).

The authoring pipeline extends the core compiler with workflow design-time tooling: static analysis against verb contracts, BPMN XML round-trip export/import, and a publish lifecycle with versioned templates.

**Pipeline:**

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  AUTHORING PIPELINE                                                          │
│                                                                              │
│  YAML Workflow Definition                                                    │
│         │                                                                    │
│         ▼                                                                    │
│  parse_workflow_yaml() → WorkflowGraphDto (12 NodeDto variants)             │
│         │                                                                    │
│         ▼                                                                    │
│  validate_dto() → reject if structural errors (15 validation rules)         │
│         │                                                                    │
│         ▼                                                                    │
│  lint_contracts() → 5 lint rules (L1-L5) against VerbContract registry      │
│         │   L1: Flag provenance (backward BFS)                              │
│         │   L2: Error code validity                                         │
│         │   L3: Correlation provenance                                      │
│         │   L4: Missing contract warning                                    │
│         │   L5: Unused writes warning                                       │
│         │                                                                    │
│         ▼                                                                    │
│  dto_to_ir() → IRGraph (petgraph)                                           │
│         │                                                                    │
│         ▼                                                                    │
│  verify() + lower() → CompiledProgram (bytecode)                            │
│         │                                                                    │
│         ▼                                                                    │
│  compute_bytecode_hash() → SHA-256 of instruction stream                    │
│         │                                                                    │
│         ▼                                                                    │
│  Optional: dto_to_bpmn_xml() → BPMN 2.0 XML (Zeebe-compatible)             │
│         │                                                                    │
│         ▼                                                                    │
│  Build WorkflowTemplate (state=Published, all artifacts)                    │
│         │                                                                    │
│         ▼                                                                    │
│  compile_and_publish() → ProcessStore + TemplateStore persistence           │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Verb Contracts (`contracts.rs`):**

| Type | Purpose |
|------|---------|
| `VerbContract` | Declares what a verb reads, writes, may raise, and produces |
| `CorrelationContract` | Declares a correlation key a verb produces |
| `ContractRegistry` | Registry of contracts + known workflow inputs (YAML-parseable) |

**Lint Rules (`lints.rs`):**

| Rule | Level | Description |
|------|-------|-------------|
| L1 | Error/Warning | Flag provenance — every flag condition must be written upstream (backward BFS); Warning if in `known_workflow_inputs` |
| L2 | Error | Error code validity — `on_error.error_code` must be in task's `may_raise_errors` (`"*"` catch-all satisfies) |
| L3 | Warning | Correlation provenance — MessageWait/HumanWait `corr_key_source` should match upstream `produces_correlation` |
| L4 | Warning | Missing contract — ServiceTask without registered contract |
| L5 | Warning | Unused writes — verb declares `writes_flags` but no edge condition references it |

**BPMN 2.0 XML Export (`export_bpmn.rs`):**

- Maps all 12 NodeDto variants to BPMN elements (startEvent, endEvent, serviceTask, exclusiveGateway, parallelGateway, inclusiveGateway, intermediateCatchEvent, eventBasedGateway, boundaryEvent)
- Deterministic `bpmn_id` generation: `sanitize_ncname(node_id) + "_" + sha256(node_id)[..4].hex()`
- Conditions exported as FEEL expressions: `= {flag} {op} {value}`
- Timer ms → ISO 8601 duration conversion
- DI layout: topological left-to-right (X = topo_rank * 200, Y = rank_index * 100)
- XOR default: set from `is_default=true` edge only (never from edge ordering)

**IR-to-DTO Reverse Mapping (`ir_to_dto.rs`):**

- Enables round-trip verification: DTO → IR → DTO → compare structure
- Maps all IRNode variants back to NodeDto
- Detects XOR default edges (unconditional from XOR with other conditional edges → `is_default=true`)

**Template Registry (`registry.rs`):**

| Type | Purpose |
|------|---------|
| `TemplateState` | `Draft`, `Published`, `Retired` — valid transitions: Draft→Published, Published→Retired |
| `SourceFormat` | `Yaml`, `BpmnImport`, `Agent` |
| `WorkflowTemplate` | Versioned publish artifact (dto_snapshot, task_manifest, bytecode_version, bpmn_xml, etc.) |
| `TemplateStore` | Async trait: save, load, list, set_state, load_latest_published |
| `MemoryTemplateStore` | In-memory implementation with immutability guards |

**Atomic Publish (`publish.rs`):**

- `publish_workflow(yaml_str, options) → PublishResult` — 10-step sync pipeline, no intermediate Draft artifact
- Rejects on validation errors, lint Error-level diagnostics, or verification failures
- `PublishResult` contains: `WorkflowTemplate` + `CompiledProgram` + `Vec<LintDiagnostic>`
- `compute_bytecode_hash()`: SHA-256 of debug-formatted instruction stream (excludes debug_map for stability)

**Engine Integration:**

- `BpmnLiteEngine::compile_and_publish()` — orchestrates publish pipeline + ProcessStore + TemplateStore persistence
- Program writes are idempotent (keyed by bytecode hash) — retry-safe

**PostgresTemplateStore (`store_postgres_templates.rs`):**

- Feature-gated behind `#[cfg(feature = "postgres")]`
- Migration `013_create_workflow_templates.sql`: table + published lookup index + immutability trigger
- Immutability trigger: published content cannot change (only state→retired); retired cannot revert

**Test Coverage (30 authoring tests):**

| Phase | Tests | Coverage |
|-------|-------|----------|
| B: Contracts + Lints | 10 (T-LINT-1..10) | Flag provenance, error codes, correlation, missing contracts, unused writes, race arm BFS, clean workflow |
| C: Export + IR-to-DTO | 10 (T-EXP-1..10) | Basic export, XOR default, timer ISO, FEEL conditions, deterministic IDs, boundary export, round-trip, linear IR, XOR conditions, boundary error |
| D: Registry + Publish | 10 (T-PUB-1..10) | Save/load round-trip, state transitions, published immutability, list filters, latest published, minimal publish, lint blocks, deterministic bytecode, artifact set, Postgres (ignored) |

### Key Files

| File | Purpose |
|------|---------|
| `bpmn-lite/bpmn-lite-core/src/types.rs` | All core types (Value, Instr, Fiber, CycleSpec, WaitArm, RacePlanEntry, InclusiveBranch, ErrorRoute, ErrorClass) |
| `bpmn-lite/bpmn-lite-core/src/events.rs` | RuntimeEvent enum (28 variants) |
| `bpmn-lite/bpmn-lite-core/src/store.rs` | ProcessStore trait (29 methods) |
| `bpmn-lite/bpmn-lite-core/src/store_memory.rs` | MemoryStore implementation |
| `bpmn-lite/bpmn-lite-core/src/store_postgres.rs` | PostgresProcessStore (`#[cfg(feature = "postgres")]`, 29 methods, 15 tests) |
| `bpmn-lite/bpmn-lite-core/migrations/` | 13 SQL migrations (tables, indexes, triggers, workflow_templates) |
| `bpmn-lite/bpmn-lite-core/src/vm.rs` | VM tick executor + race/boundary/terminate/inclusive tests |
| `bpmn-lite/bpmn-lite-core/src/engine.rs` | BpmnLiteEngine facade + compile_and_publish() |
| `bpmn-lite/bpmn-lite-core/src/compiler/parser.rs` | BPMN XML → IR (timeCycle, cancelActivity, errorEventDefinition, inclusiveGateway) |
| `bpmn-lite/bpmn-lite-core/src/compiler/ir.rs` | IRGraph, TimerSpec (Duration/Date/Cycle) |
| `bpmn-lite/bpmn-lite-core/src/compiler/verifier.rs` | Structural verification (cycle+interrupting rule) |
| `bpmn-lite/bpmn-lite-core/src/compiler/lowering.rs` | IR → bytecode (race_plan, error_route_map, inclusive branches) |
| `bpmn-lite/bpmn-lite-core/src/authoring/contracts.rs` | VerbContract, ContractRegistry, YAML parsing |
| `bpmn-lite/bpmn-lite-core/src/authoring/lints.rs` | 5 lint rules (L1-L5), lint_contracts() entry point |
| `bpmn-lite/bpmn-lite-core/src/authoring/export_bpmn.rs` | DTO → BPMN 2.0 XML (Zeebe-compatible) |
| `bpmn-lite/bpmn-lite-core/src/authoring/ir_to_dto.rs` | IR → DTO reverse mapping for round-trip |
| `bpmn-lite/bpmn-lite-core/src/authoring/registry.rs` | WorkflowTemplate, TemplateStore trait, MemoryTemplateStore |
| `bpmn-lite/bpmn-lite-core/src/authoring/publish.rs` | Atomic 10-step publish pipeline |
| `bpmn-lite/bpmn-lite-core/src/authoring/store_postgres_templates.rs` | PostgresTemplateStore (feature-gated) |
| `bpmn-lite/bpmn-lite-server/src/lib.rs` | Library re-export of proto module |
| `bpmn-lite/bpmn-lite-server/src/grpc.rs` | gRPC handler implementations |
| `bpmn-lite/bpmn-lite-server/src/main.rs` | Server startup |
| `bpmn-lite/bpmn-lite-server/proto/bpmn_lite/v1/bpmn_lite.proto` | Proto definition |
| `bpmn-lite/bpmn-lite-server/tests/integration.rs` | 6 integration tests + 1 gRPC smoke test |
| `rust/xtask/src/bpmn_lite.rs` | xtask build/test/deploy commands |

### Fiber-Based Execution Model — Design Rationale

**Problem:** We need a workflow runtime that supports long-running orchestration (minutes to weeks), pause/resume on timers/messages/human gates, deterministic replay, exactly-once domain effects over at-least-once delivery, and strict separation of orchestration from business logic.

**Why not interpreter-style BPMN execution?** A "DI/Spring" instinct is to load BPMN XML into a runtime interpreter and execute it. That fails because: interpreter state is opaque and hard to persist/replay deterministically; "magic" pushes failures to runtime and makes incident resolution non-reproducible; data mapping engines (FEEL/JUEL) encourage domain logic in the workflow engine; exactly-once semantics become hard to guarantee.

**Chosen approach:** BPMN-Lite **compiles** BPMN into compact bytecode, then executes it using **fibers**.

**What is a Fiber:** A fiber is the smallest unit of execution — a user-space "token + instruction pointer":
- **Serializable** — can be persisted to storage and resumed later
- **Explicit** — not hidden in a framework runtime
- **Controlled** — our scheduler decides step budgets, no hidden preemption
- **Deterministic** — same inputs produce same execution trace

```rust
struct Fiber {
    fiber_id: Uuid,
    pc: Addr,                    // Program counter into bytecode
    stack: Vec<Value>,           // Small values for branching
    regs: [Option<Value>; 8],    // Fixed locals
    wait_state: WaitState,       // Running | Job | Timer | Msg | Join | Race | Incident
}
```

**Yield points are explicit and finite.** A fiber may only stop at:
- `EXEC_NATIVE` → parks as `WaitState::Job` (waiting for ob-poc verb worker)
- `WAIT_*` (timer/message/human) → parks in appropriate wait state
- `JOIN` → parks at parallel join barrier
- `END` / `FAIL` → terminates fiber

**Why fibers instead of goroutines/threads:**
- **Massive concurrency** — thousands of tokens without OS threads
- **Deterministic scheduling** — no hidden preemption
- **Durable persistence** — serialize fiber state, restart safely
- **Exact invariants** — "no fiber → no mutation" guardrails
- **Replayability** — event log + fiber snapshots restore exact state

Goroutines/threads are great for in-memory concurrency but cannot reliably persist a goroutine's stack/PC and replay it deterministically.

**Two-stack separation (process vs data):**

| Stack | Owned By | Contents |
|-------|----------|----------|
| **Process / control-flow** | BPMN-Lite | Bytecode, fibers, joins, waits, races, event log |
| **Domain / business data** | ob-poc | `domain_payload` (opaque canonical JSON + SHA-256 hash) |

The engine **never interprets** `domain_payload`. Only ob-poc verbs mutate it. The engine branches only on `orch_flags` (flat primitives). This prevents "engine creep" into business logic.

**How work happens (EXEC_NATIVE → Job):**
1. Service task compiles to `EXEC_NATIVE` → creates `JobActivation` (job_key, task_type, domain_payload + hash, orch_flags)
2. ob-poc worker processes the job by running a verb
3. Worker returns `JobCompletion` (same job_key, updated payload + hash, updated orch_flags)
4. Idempotency: completion deduped by `job_key` — retries and duplicates are safe

**Durability invariants (non-negotiable):**
- A completion/signal must never mutate state unless there is a live fiber waiting for it
- Every irreversible decision is recorded in the event log (gateway branches, wait registrations, race winners, join releases, flag writes)
- PITR/replay must not re-execute verbs — it replays decisions and consults dedupe history

**Alternatives considered and rejected:**

| Alternative | Why Rejected |
|-------------|-------------|
| Interpreter-style BPMN | Opaque runtime state, hard PITR, encourages domain logic in engine |
| Full Camunda-like engine | Heavy platform surface area, data semantics not solved, "engine becomes system" |
| Thread-per-instance | Too heavy, not scalable, hard to persist |

**Where to look in the code:**

| File | What to Read |
|------|-------------|
| `bpmn-lite-core/src/types.rs` | `Fiber`, `WaitState`, `Instr`, `Value` — the core data model |
| `bpmn-lite-core/src/vm.rs` | `tick_fiber()` — the bytecode interpreter loop |
| `bpmn-lite-core/src/engine.rs` | `BpmnLiteEngine` — facade orchestrating compiler + VM + store |
| `bpmn-lite-core/src/store.rs` | `ProcessStore` trait — the persistence contract |
| `bpmn-lite-core/src/events.rs` | `RuntimeEvent` — the audit event log |
| `rust/src/bpmn_integration/dispatcher.rs` | `WorkflowDispatcher` — ob-poc side, routes verbs to BPMN-Lite |
| `rust/src/bpmn_integration/worker.rs` | `JobWorker` — ob-poc side, processes jobs from BPMN-Lite |

---

## Code Patterns

### Config Struct Pattern

For types with many optional parameters, use a builder-style config struct:

```rust
// Instead of many constructors:
// fn new(pool: PgPool) -> Self
// fn with_sessions(pool: PgPool, sessions: SessionStore) -> Self
// fn with_all_sessions(...) -> Self

// Use config struct with builder:
pub struct ToolHandlersConfig {
    pub pool: PgPool,
    pub sessions: Option<SessionStore>,
    pub cbu_sessions: Option<CbuSessionStore>,
    pub learned_data: Option<SharedLearnedData>,
    pub embedder: Option<SharedEmbedder>,
}

// Usage:
let handlers = ToolHandlersConfig::new(pool)
    .with_sessions(sessions)
    .with_embedder(embedder)
    .build();
```

See `rust/src/mcp/handlers/core.rs` for the `ToolHandlersConfig` pattern.

### Centralized DB Access

All database access should go through service modules in `rust/src/database/`:
- `VerbService` for verb discovery and semantic search
- `VisualizationRepository` for graph/viewport queries
- `SessionRepository` for session persistence
- `GenerationLogRepository` for training data

No direct `sqlx::query` calls outside of service modules.

---

## ESPER Navigation Crates (065)

> **DEPRECATED (2026-01-31):** The esper_* crates (`esper_snapshot`, `esper_core`, `esper_input`,
> `esper_policy`, `esper_egui`) are retained in the repository for reference but are no longer
> used by any active code path. All visualization is now handled by the React frontend
> (`ob-poc-ui-react/`) with data served via REST API endpoints.

**Crates retained for reference:** `esper_snapshot`, `esper_core`, `esper_input`, `esper_policy`, `esper_egui`, `esper_compiler`

For historical implementation details, see git history or `ai-thoughts/065-esper-navigation.md`.

---

## Graph Configuration (Policy-Driven)

> **DEPRECATED (2026-02-01):** The graph configuration system (`config/graph_settings.yaml`)
> was built for the esper_* visualization crates which are now deprecated. The configuration
> file and `ob-poc-graph` crate still exist but are not used by the React frontend.

**Key file retained:** `rust/config/graph_settings.yaml` — LOD, layout, animation, viewport config
**Crate retained:** `rust/crates/ob-poc-graph/` — Graph data structures (still used for API responses)

For historical details, see git history.

---

## Inspector-First Visualization (076)

> ✅ **IMPLEMENTED (2026-02-04)**: Deterministic tree/table-based Inspector UI replacing 3D visualization, with projection schema and `$ref` linking.

The Inspector-First Visualization provides a deterministic, compliance-focused alternative to the ESPER 3D visualization. It uses a flat node map with `$ref` linking for node-to-node relationships, enabling explainable and auditable visualizations.

### Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    INSPECTOR PROJECTION PIPELINE                             │
│                                                                              │
│  CBU Graph Data (entities, matrix, registers)                               │
│         │                                                                    │
│         ▼                                                                    │
│  CbuGenerator / MatrixGenerator                                             │
│         │   - Converts domain types to projection nodes                     │
│         │   - Creates flat node map with $ref links                         │
│         │   - Applies RenderPolicy (LOD, depth, filters)                    │
│         │                                                                    │
│         ▼                                                                    │
│  InspectorProjection                                                        │
│         │   - snapshot: schema_version, source_hash, policy_hash            │
│         │   - render_policy: lod, max_depth, filters                        │
│         │   - root: BTreeMap<chamber, RefValue>                             │
│         │   - nodes: BTreeMap<NodeId, Node>                                 │
│         │                                                                    │
│         ▼                                                                    │
│  Inspector Panel (egui)                                                     │
│         │   - Three-panel layout (nav tree, main view, detail pane)         │
│         │   - Table rendering for matrix/holdings/control                   │
│         │   - LOD controls, chamber toggles, filters                        │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Key Principles

| Principle | Implementation |
|-----------|----------------|
| **Correctness over polish** | Every visualization is explainable and traceable |
| **Determinism** | Same input + policy = same output, always |
| **Separation of concerns** | Projection contract independent of renderer |
| **Compliance-first** | Provenance, confidence, audit trails are first-class |

### Core Types

**NodeId** - Stable path-based identifier:
```rust
// Format: {kind}:{qualifier}[:{subpath}]
// Examples: "cbu:allianz-ie", "matrix:focus:mic:XLON", "entity:uuid:..."
// Pattern: ^[a-z]+:[A-Za-z0-9_:-]+$
```

**RefValue** - `$ref` linking (flat node map, no recursive embedding):
```rust
// Serializes as: { "$ref": "node_id" }
pub struct RefValue {
    #[serde(rename = "$ref")]
    pub target: NodeId,
}
```

**NodeKind** - 20 node type variants:
- CBU, MemberList, Entity
- ProductTree, Product, Service, Resource, ProductBinding
- InstrumentMatrix, MatrixSlice, SparseCellPage
- InvestorRegister, HoldingEdgeList, HoldingEdge
- ControlRegister, ControlTree, ControlNode, ControlEdge

**RenderPolicy** - LOD levels 0-3:

| LOD | Display |
|-----|---------|
| 0 | Glyph + ID only |
| 1 | Glyph + label_short |
| 2 | + tags, summary, collapsed branches |
| 3 | + label_full, attributes, provenance, expanded branches |

### API Endpoint

```
GET /api/cbu/:cbu_id/inspector?lod=2&max_depth=5
```

Returns `InspectorProjection` with full node tree for the CBU.

### UI Panel Layout

```
┌─────────────────────────────────────────────────────────────────────┐
│ [Search] [LOD: ▼] [Depth: ▼] [Chambers: ▼] [Filters: ▼] [Reset]    │
├─────────────────────────────────────────────────────────────────────┤
│ Breadcrumb: CBU > Members > Fund: Allianz IE ETF SICAV              │
├───────────────────┬─────────────────────────┬───────────────────────┤
│ Navigation Tree   │ Main View               │ Detail Pane           │
│ (30%)             │ (45%)                   │ (25%)                 │
│                   │                         │                       │
│ - CBU             │ [Tree or Table based    │ Kind: Entity          │
│   - Members       │  on focused node kind]  │ ID: entity:uuid:...   │
│     - Fund ←      │                         │ Label: Fund: ...      │
│     - IM          │                         │ Attributes: {...}     │
│   - Products      │                         │ Provenance: {...}     │
│   - Matrix        │                         │ Links: [...]          │
│   - Registers     │                         │                       │
└───────────────────┴─────────────────────────┴───────────────────────┘
```

### Table Rendering

For matrix slices, holdings, and control trees, the Inspector uses `egui_extras::TableBuilder`:

| View | Columns | Features |
|------|---------|----------|
| Matrix Slice | Instrument, Market, Currency, Status | Color-coded status, cell selection |
| Holdings | Holder, Amount, Percentage, Confidence | Sortable, provenance tooltips |
| Control Tree | Controller, Controlled, Voting %, Provenance | Hierarchical with expand/collapse |

### Validation

Pre-render validation ensures projection integrity:
1. **Dangling refs** - All `$ref` targets must exist in `nodes`
2. **Cycle detection** - DFS with visited set, report cycles as warnings
3. **Root validation** - All root refs must exist
4. **Provenance** - `HoldingEdge` and `ControlEdge` MUST have provenance
5. **Confidence range** - Must be 0.0-1.0 if present

### Crate Structure

```
rust/crates/inspector-projection/
├── src/
│   ├── lib.rs              # Public API exports
│   ├── model.rs            # InspectorProjection, Node, NodeKind
│   ├── node_id.rs          # NodeId newtype with regex validation
│   ├── ref_value.rs        # RefValue $ref linking
│   ├── validate.rs         # Referential integrity, cycle detection
│   ├── policy.rs           # RenderPolicy (LOD, filters)
│   ├── error.rs            # ValidationError enum
│   └── generator/
│       ├── mod.rs          # Generator traits
│       ├── cbu.rs          # CbuGenerator (graph → projection)
│       └── matrix.rs       # MatrixGenerator (trading matrix → projection)
```

### Key Files

| File | Purpose |
|------|---------|
| `rust/crates/inspector-projection/src/model.rs` | Core types: Node, NodeKind, InspectorProjection |
| `rust/crates/inspector-projection/src/node_id.rs` | NodeId with regex validation |
| `rust/crates/inspector-projection/src/validate.rs` | Projection validation |
| `rust/crates/inspector-projection/src/generator/cbu.rs` | CbuGenerator |
| `rust/src/api/graph_routes.rs` | `/api/cbu/:cbu_id/inspector` endpoint |
| `rust/tests/fixtures/inspector/sample.yaml` | Test fixture with all node kinds |

> **Note:** The egui Inspector panel (`ob-poc-ui`) has been removed. The projection crate (`inspector-projection`) remains active for API-driven inspection.

### Test Coverage

- 48 unit tests in `inspector-projection` crate
- 32 integration tests in `rust/tests/`
- 2 doc tests
- Sample YAML fixture with comprehensive node coverage

---

## Lexicon Service (072)

> ✅ **IMPLEMENTED (2026-02-02)**: In-memory verb/domain/concept lookup with bincode snapshots.

The Lexicon Service provides fast, in-memory lookup for DSL vocabulary without database queries at runtime.

### Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    LEXICON PIPELINE                                          │
│                                                                              │
│  config/verbs/*.yaml + config/lexicon/*.yaml                                │
│         │                                                                    │
│         ▼                                                                    │
│  cargo x lexicon compile                                                    │
│         │   - Loads verb definitions from YAML                              │
│         │   - Loads domain/entity_type/verb_concept config                  │
│         │   - Builds indexed snapshot                                       │
│         │   - Serializes to bincode                                         │
│         │                                                                    │
│         ▼                                                                    │
│  assets/lexicon.snapshot.bin                                                │
│         │                                                                    │
│         ▼                                                                    │
│  LexiconService::load() at server startup                                   │
│         │   - Deserializes snapshot                                         │
│         │   - Provides O(1) lookups for verbs, domains, concepts            │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Key Types

| Type | Purpose |
|------|---------|
| `LexiconSnapshot` | In-memory index with verb/domain/concept maps |
| `VerbEntry` | Verb metadata (domain, action, description, args) |
| `DomainEntry` | Domain with associated verbs |
| `VerbConceptEntry` | Semantic concept for intent matching |

### Commands

```bash
cd rust/

# Compile lexicon snapshot from YAML
cargo x lexicon compile [-v|--verbose]

# Lint lexicon config for issues  
cargo x lexicon lint [--errors-only]

# Show snapshot statistics
cargo x lexicon stats [--snapshot <path>]
```

### Key Files

| File | Purpose |
|------|---------|
| `rust/src/lexicon/mod.rs` | Module exports |
| `rust/src/lexicon/types.rs` | `LexiconSnapshot`, `VerbEntry`, `DomainEntry` |
| `rust/src/lexicon/compiler.rs` | YAML → snapshot compiler |
| `rust/src/lexicon/service.rs` | `LexiconService` trait and implementation |
| `rust/src/lexicon/snapshot.rs` | Bincode serialization |
| `rust/xtask/src/lexicon.rs` | CLI commands |
| `rust/config/lexicon/domains.yaml` | Domain definitions |
| `rust/config/lexicon/entity_types.yaml` | Entity type vocabulary |
| `rust/config/lexicon/verb_concepts.yaml` | Verb concept mappings |
| `rust/assets/lexicon.snapshot.bin` | Compiled snapshot artifact |

---

## Entity Linking Service (073)

> ✅ **IMPLEMENTED (2026-02-03)**: In-memory entity resolution with mention extraction and token overlap matching.

The Entity Linking Service resolves natural language entity references to database UUIDs without runtime DB queries for the lookup phase.

### Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    ENTITY LINKING PIPELINE                                   │
│                                                                              │
│  User utterance: "Set up ISDA with Goldman Sachs"                           │
│         │                                                                    │
│         ▼                                                                    │
│  MentionExtractor.extract()                                                 │
│         │   - N-gram scanning (1 to max_ngram_size tokens)                  │
│         │   - Alias index lookup                                            │
│         │   - Token overlap fallback for fuzzy matching                     │
│         │   - Non-overlapping span selection                                │
│         │                                                                    │
│         ▼                                                                    │
│  MentionSpan { text: "Goldman Sachs", candidate_ids: [uuid1, ...] }         │
│         │                                                                    │
│         ▼                                                                    │
│  EntityLinkingService.resolve_mentions()                                    │
│         │   - Score candidates by kind constraint + concept overlap         │
│         │   - Generate Evidence for disambiguation audit                    │
│         │                                                                    │
│         ▼                                                                    │
│  EntityResolution { entity_id, score, evidence: [...] }                     │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Snapshot Compilation

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  DATABASE TABLES                          SNAPSHOT INDEXES                   │
│                                                                              │
│  ob-poc.entities ──────────────────────► entities: Vec<EntityRow>           │
│  ob-poc.entity_names ──────────────────► name_index: HashMap<norm, id>      │
│  agent.entity_aliases ─────────────────► alias_index: HashMap<alias, ids>   │
│  ob-poc.entity_concept_link ───────────► concept_links: HashMap<id, concepts>│
│  ob-poc.entity_feature ────────────────► (future: feature vectors)          │
│                                                                              │
│  + Derived token_index for fuzzy matching                                   │
│  + Derived kind_index for type filtering                                    │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Key Types

| Type | Purpose |
|------|---------|
| `EntitySnapshot` | In-memory indexes (alias, name, token, concept, kind) |
| `EntityRow` | Entity record (id, kind, canonical_name, name_norm) |
| `MentionExtractor` | N-gram scanner for entity mentions |
| `MentionSpan` | Extracted mention with character positions and candidates |
| `EntityLinkingService` | Resolution trait with scoring and evidence |
| `EntityResolution` | Resolved entity with score and evidence trail |
| `Evidence` | Tagged enum for disambiguation audit (ExactAlias, TokenOverlap, etc.) |

### Commands

```bash
cd rust/

# Compile entity snapshot from database
DATABASE_URL="postgresql:///data_designer" cargo x entity compile [-v|--verbose]

# Lint entity data for quality issues
DATABASE_URL="postgresql:///data_designer" cargo x entity lint [--errors-only]

# Show snapshot statistics
cargo x entity stats [--snapshot <path>]
```

### Text Normalization

Entity matching uses Unicode NFKC normalization:

```rust
use ob_poc::entity_linking::normalize_entity_text;

// Basic normalization: lowercase, NFKC, strip punctuation
normalize_entity_text("Goldman Sachs & Co.", false)  // → "goldman sachs co"

// With legal suffix stripping
normalize_entity_text("Apple Inc.", true)  // → "apple"
normalize_entity_text("Ford Motor Company Ltd.", true)  // → "ford motor company"
```

### Key Files

| File | Purpose |
|------|---------|
| `rust/src/entity_linking/mod.rs` | Module exports |
| `rust/src/entity_linking/normalize.rs` | Unicode NFKC normalization, tokenization |
| `rust/src/entity_linking/snapshot.rs` | `EntitySnapshot` with indexes |
| `rust/src/entity_linking/mention.rs` | `MentionExtractor` for n-gram scanning |
| `rust/src/entity_linking/resolver.rs` | `EntityLinkingService` trait and impl |
| `rust/src/entity_linking/compiler.rs` | DB → snapshot compiler with SHA256 hash |
| `rust/xtask/src/entity.rs` | CLI commands |
| `rust/assets/entity.snapshot.bin` | Compiled snapshot artifact |
| `rust/migrations/073_entity_linking_support.sql` | Schema additions |

### Database Tables (073)

| Table | Purpose |
|-------|---------|
| `ob-poc.entity_concept_link` | Links entities to semantic concepts with weights |
| `ob-poc.entity_feature` | Entity feature flags (future: ML features) |
| `ob-poc.entities.name_norm` | Normalized name column with auto-update trigger |

### Views

| View | Purpose |
|------|---------|
| `ob-poc.v_entity_linking_data` | Flattened entity data for snapshot compilation |
| `ob-poc.v_entity_aliases` | Union of entity_names + agent.entity_aliases |
| `ob-poc.v_entity_linking_stats` | Statistics for monitoring |

### Agent Pipeline Integration

The EntityLinkingService is integrated into the agent chat pipeline for:
1. **Pre-resolution** - Entity mentions extracted BEFORE verb search
2. **Kind hints** - Resolved entity kinds inform verb argument expectations
3. **Dominant entity** - Highest confidence entity stored in session context
4. **Debug info** - Entity resolution details in `ChatDebugInfo.entity_resolution`

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  AGENT PIPELINE WITH ENTITY LINKING                                          │
│                                                                              │
│  User: "Set up ISDA with Goldman Sachs"                                     │
│         │                                                                    │
│         ▼                                                                    │
│  1. EntityLinkingService.resolve_mentions()                                 │
│         │   └─► "Goldman Sachs" → entity_id (score: 0.95)                   │
│         │   └─► dominant_entity_id stored in session.context                │
│         │                                                                    │
│         ▼                                                                    │
│  2. HybridVerbSearcher.search() (verb discovery)                            │
│         │   └─► "isda.create" (score: 0.88)                                 │
│         │                                                                    │
│         ▼                                                                    │
│  3. LLM extracts arguments (JSON)                                           │
│         │                                                                    │
│         ▼                                                                    │
│  4. DSL generation with entity resolution                                   │
│         │   └─► (isda.create :counterparty <Goldman Sachs>)                 │
│         │                                                                    │
│         ▼                                                                    │
│  5. Stage for execution or auto-run (navigation verbs)                      │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Key Integration Points:**

| Component | File | Integration |
|-----------|------|-------------|
| `AgentState` | `api/agent_routes.rs` | Loads snapshot at startup, field `entity_linker` |
| `AgentService` | `api/agent_service.rs` | `extract_entity_mentions()` in `process_chat()` |
| `SessionContext` | `api/session.rs` | `dominant_entity_id` field for implicit resolution |
| `ChatDebugInfo` | `ob-poc-types/src/lib.rs` | `entity_resolution` field for explainability |

**Debug Output (when OB_CHAT_DEBUG=1):**

```json
{
  "entity_resolution": {
    "snapshot_hash": "a1b2c3d4...",
    "entity_count": 1452,
    "mentions": [{
      "span": [15, 28],
      "text": "Goldman Sachs",
      "candidates": [{"entity_id": "...", "score": 0.95, "entity_kind": "company"}],
      "selected_id": "...",
      "confidence": 0.95
    }],
    "dominant_entity": {"entity_id": "...", "canonical_name": "Goldman Sachs Group Inc"},
    "expected_kinds": ["company"]
  }
}
```

**Graceful Degradation:**

If no entity snapshot is available, `StubEntityLinkingService` is used:
- Returns empty results for all lookups
- Pipeline continues without entity pre-resolution
- Full functionality restored when snapshot is compiled

---

## Unified Lookup Service (074)

> ✅ **IMPLEMENTED (2026-02-04)**: Consolidated verb search + entity linking with verb-first ordering.

The LookupService unifies verb discovery and entity resolution into a single analysis pass, implementing **verb-first ordering**: verbs are searched first, then expected entity kinds are derived from verb schema, and finally entities are resolved with kind constraints.

### Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  UNIFIED LOOKUP SERVICE - Verb-First Ordering                                │
│                                                                              │
│  User: "Load the Allianz book"                                              │
│         │                                                                    │
│         ▼                                                                    │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │  Step 1: VERB SEARCH (HybridVerbSearcher)                           │    │
│  │  "load the allianz book" → session.load-galaxy (score: 0.85)        │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│         │                                                                    │
│         ▼                                                                    │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │  Step 2: DERIVE EXPECTED KINDS (from verb schema)                   │    │
│  │  session.load-galaxy has :apex-entity-id arg with lookup config     │    │
│  │  → expected_kinds: ["company", "client_group"]                      │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│         │                                                                    │
│         ▼                                                                    │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │  Step 3: ENTITY RESOLUTION (EntityLinkingService)                   │    │
│  │  "allianz" + kind_constraint → Allianz SE (score: 0.92)            │    │
│  │  Kind constraint boosts company matches, penalizes person matches   │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│         │                                                                    │
│         ▼                                                                    │
│  LookupResult {                                                              │
│    verbs: [session.load-galaxy @ 0.85, ...],                                │
│    entities: [Allianz SE @ 0.92],                                           │
│    dominant_entity: Allianz SE,                                             │
│    expected_kinds: ["company", "client_group"],                             │
│    verb_matched: true,                                                       │
│    entities_resolved: true                                                   │
│  }                                                                           │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Key Insight: Verb-First vs Entity-First

| Approach | Flow | Problem |
|----------|------|---------|
| **Entity-first** | Entities → Verbs | "Goldman" matches 50 people AND the company |
| **Verb-first** | Verbs → Expected Kinds → Entities | "isda.create" expects company → "Goldman" = Goldman Sachs Group |

Verb-first ordering uses verb schema to **constrain** entity resolution, dramatically improving accuracy.

### Key Types

| Type | Module | Purpose |
|------|--------|---------|
| `LookupService` | `lookup::service` | Unified analysis combining verb search + entity linking |
| `LookupResult` | `lookup::service` | Combined result with verbs, entities, dominant entity |
| `DominantEntity` | `lookup::service` | Highest confidence resolved entity |

### LookupResult Fields

```rust
pub struct LookupResult {
    /// Verb candidates from search (sorted by score)
    pub verbs: Vec<VerbSearchResult>,
    
    /// Entity resolutions with kind-constrained scoring
    pub entities: Vec<EntityResolution>,
    
    /// Dominant entity (highest confidence, kind-matched)
    pub dominant_entity: Option<DominantEntity>,
    
    /// Expected entity kinds derived from top verb(s)
    pub expected_kinds: Vec<String>,
    
    /// Whether verb search found a clear winner (score >= 0.65)
    pub verb_matched: bool,
    
    /// Whether entity resolution found unambiguous matches
    pub entities_resolved: bool,
}
```

### Integration in AgentService

The `LookupService` is built on-demand in `AgentService.get_lookup_service()` using existing components:

```rust
// AgentService.process_chat() uses unified lookup when entity_linker is configured
let (entity_resolution_debug, dominant_entity_id, resolved_kinds) =
    if let Some(lookup_service) = self.get_lookup_service() {
        // Unified path: verb-first ordering
        let lookup_result = lookup_service.analyze(&request.message, 5).await;
        // ... build debug info from lookup_result
    } else {
        // Legacy path: separate entity linking
        self.extract_entity_mentions(&request.message, None)
    };
```

### Key Files

| File | Purpose |
|------|---------|
| `rust/src/lookup/mod.rs` | Module exports |
| `rust/src/lookup/service.rs` | `LookupService` implementation |
| `rust/src/api/agent_service.rs` | `get_lookup_service()` builder, integration in `process_chat()` |

### Graceful Degradation

If `entity_linker` is not configured (no snapshot), `get_lookup_service()` returns `None` and the legacy path is used. This ensures the pipeline always works, with enhanced accuracy when entity linking is available.

---

## Key Directories

```
ob-poc/
├── bpmn-lite/                  # Standalone BPMN orchestration service (NOT inside rust/)
│   ├── bpmn-lite-core/         # Core types, compiler, VM, store, authoring pipeline
│   │   ├── src/authoring/      # Contracts, lints, BPMN export, publish lifecycle
│   │   ├── src/compiler/       # Parser, IR, verifier, lowering
│   │   └── migrations/         # 13 SQL migrations (ProcessStore + templates)
│   └── bpmn-lite-server/       # gRPC server (tonic), proto definitions
├── ob-poc-ui-react/            # React/TypeScript frontend (PRIMARY UI)
│   ├── src/
│   │   ├── api/                # API client modules
│   │   ├── features/           # Chat, Inspector, Settings
│   │   ├── stores/             # Zustand state
│   │   └── types/              # TypeScript types
│   └── dist/                   # Production build (served by Rust)
├── rust/
│   ├── config/verbs/           # 103 YAML verb definitions
│   ├── crates/
│   │   ├── dsl-core/           # Parser, AST, compiler (no DB)
│   │   ├── dsl-lsp/            # LSP server + Zed extension + tree-sitter grammar
│   │   ├── ob-agentic/         # Onboarding pipeline (Intent→Plan→DSL)
│   │   ├── ob-poc-macros/      # Proc macros (#[register_custom_op], #[derive(IdType)])
│   │   ├── ob-poc-graph/       # Graph data structures
│   │   ├── ob-poc-web/         # Axum web server (serves React + API)
│   │   ├── inspector-projection/ # Projection schema generation
│   │   └── esper_*/            # DEPRECATED — 6 crates retained for reference
│   ├── config/packs/           # 4 pack YAML manifests (V2 REPL journeys)
│   ├── tests/golden_corpus/    # 102 test entries across 8 YAML files
│   └── src/
│       ├── repl/               # V2 REPL (vnext-repl feature flag)
│       │   ├── orchestrator_v2.rs  # 7-state machine
│       │   ├── context_stack.rs    # ContextStack fold
│       │   ├── scoring.rs          # Pack scoring + ambiguity
│       │   ├── preconditions.rs    # Precondition engine
│       │   └── ...                 # 27 files total
│       ├── journey/            # Pack system (router, manifests, handoff)
│       ├── dsl_v2/             # DSL execution
│       │   ├── custom_ops/     # Plugin handlers
│       │   └── generic_executor.rs
│       ├── domain_ops/         # CustomOperation implementations (~300+ ops)
│       ├── lexicon/            # In-memory verb/domain/concept lookup (072)
│       ├── entity_linking/     # In-memory entity resolution (073)
│       ├── session/            # Session state
│       ├── graph/              # Graph builders
│       └── api/                # REST routes
├── migrations/                 # SQLx migrations
├── docs/                       # Architecture docs
└── ai-thoughts/                # ADRs and design docs
```

---

## Environment Variables

```bash
# Required
DATABASE_URL="postgresql:///data_designer"

# LLM (for agent chat, not embeddings)
AGENT_BACKEND=anthropic
ANTHROPIC_API_KEY="sk-ant-..."

# Optional
DSL_CONFIG_DIR="/path/to/config"
BRAVE_SEARCH_API_KEY="..."       # Research macros
```

---

## Database Practices

### SQLx Compile-Time Verification

```bash
# After schema changes
psql -d data_designer -f your_migration.sql
cd rust && cargo sqlx prepare --workspace
cargo build  # Catches type mismatches
```

### Type Mapping

| PostgreSQL | Rust |
|------------|------|
| UUID | `Uuid` |
| TIMESTAMPTZ | `DateTime<Utc>` |
| INTEGER | `i32` |
| BIGINT | `i64` |
| NUMERIC | `BigDecimal` |
| NULLABLE | `Option<T>` |

---

## Error Handling

**Never use `.unwrap()` in production paths.** Use:
- `?` operator
- `.ok_or_else(|| anyhow!(...))`
- `let Some(x) = ... else { continue }`
- `match` / `if let`

---

## Trigger Phrases

When you see these in a task, read the corresponding annex first:

| Phrase | Read |
|--------|------|
| "add verb", "create verb", "verb YAML" | `docs/verb-definition-spec.md` |
| "React", "frontend", "chat UI", "scope panel" | CLAUDE.md §React Frontend |
| "entity model", "CBU", "UBO", "holdings" | `docs/strategy-patterns.md` §1 |
| "schema overview", "table structure", "ER diagram", "mermaid", "ob-poc schema" | `migrations/OB_POC_SCHEMA_ENTITY_OVERVIEW.md` |
| "agent", "MCP", "verb_search" | `docs/agent-architecture.md` |
| "session", "scope", "navigation" | `docs/session-visualization-architecture.md` |
| "ESPER", "drill", "trace", "xray" | `docs/session-visualization-architecture.md` |
| "investor register", "look-through" | `ai-thoughts/018-investor-register-visualization.md` |
| "GROUP", "ownership graph" | `ai-thoughts/019-group-taxonomy-intra-company-ownership.md` |
| "sheet", "phased execution", "DAG" | `ai-thoughts/035-repl-session-implementation-plan.md` |
| "solar navigation", "ViewState", "orbit", "nav_history" | `ai-thoughts/038-solar-navigation-unified-design.md` |
| "intent pipeline", "ambiguity", "normalize_candidates", "ref_id" | CLAUDE.md §Intent Pipeline Fixes (042) |
| "intent tier", "tier disambiguation", "what are you trying to do" | CLAUDE.md §Intent Tier Disambiguation (065) |
| "macro", "operator vocabulary", "structure.setup", "constraint cascade" | CLAUDE.md §Operator Vocabulary & Macros (058) |
| "onboarding pipeline", "RequirementPlanner", "OnboardingPlan", "ob-agentic" | CLAUDE.md §Structured Onboarding Pipeline |
| "LSP", "language server", "completion", "diagnostics", "dsl-lsp" | CLAUDE.md §DSL Language Server |
| "lexicon", "verb lookup", "domain lookup", "LexiconService" | CLAUDE.md §Lexicon Service (072) |
| "entity linking", "mention extraction", "entity resolution", "EntityLinkingService" | CLAUDE.md §Entity Linking Service (073) |
| "lookup service", "verb-first", "dual search", "LookupService" | CLAUDE.md §Unified Lookup Service (074) |
| "REPL redesign", "state machine", "V2 REPL", "ReplOrchestrator", "/api/repl" | CLAUDE.md §V2 REPL Architecture |
| "context stack", "ContextStack", "runbook fold", "DerivedScope" | CLAUDE.md §V2 REPL Architecture |
| "pack router", "PackManifest", "pack.select", "journey" | CLAUDE.md §V2 REPL Architecture |
| "scoring", "pack scoring", "dual decode", "ambiguity policy" | CLAUDE.md §V2 REPL Architecture |
| "preconditions", "EligibilityMode", "requires_scope" | CLAUDE.md §V2 REPL Architecture |
| "decision log", "DecisionLog", "replay tuner", "golden corpus" | CLAUDE.md §V2 REPL Architecture |
| "focus mode", "FocusMode", "domain affinity" | CLAUDE.md §V2 REPL Architecture |
| "orchestrator v2", "ReplOrchestratorV2", "session v2" | CLAUDE.md §V2 REPL Architecture |
| "invariant", "P-1", "P-2", "P-3", "P-4", "P-5" | `docs/INVARIANT-VERIFICATION.md` |
| "BPMN", "bpmn-lite", "orchestration", "fiber VM", "durable workflow" | CLAUDE.md §BPMN-Lite Durable Orchestration Service |
| "race", "WaitState::Race", "boundary timer", "race_plan", "RacePlanEntry" | CLAUDE.md §BPMN-Lite §Race Semantics (Phase 2) |
| "non-interrupting", "timer cycle", "CycleSpec", "timeCycle", "BoundaryFired", "cycle exhaustion" | CLAUDE.md §BPMN-Lite §Non-Interrupting Boundary Events (Phase 2A) |
| "ghost signal", "SignalIgnored", "WaitCancelled", "cancel_jobs", "complete_job guard" | CLAUDE.md §BPMN-Lite §Cancel & Ghost Signal Protection (Phase 3) |
| "fiber", "fiber-based execution", "two-stack separation", "EXEC_NATIVE", "durable fiber" | CLAUDE.md §Fiber-Based Execution Model |
| "WorkflowDispatcher", "JobWorker", "EventBridge", "correlation", "parked token", "bpmn_integration" | CLAUDE.md §BPMN-Lite Integration (Phase B) |
| "PendingDispatch", "pending dispatch", "queue resilience", "dispatch worker", "retry worker" | CLAUDE.md §BPMN-Lite Integration (Phase B) §Queue-Based Resilience |
| "EndTerminate", "terminate end", "error boundary", "ErrorRoute", "error_route_map", "BusinessRejection" | CLAUDE.md §BPMN-Lite §Terminate End Events / Error Boundary Routing (Phase 5) |
| "IncCounter", "BrCounterLt", "bounded loop", "retry loop", "counter" | CLAUDE.md §BPMN-Lite §Bounded Retry Loops (Phase 5.3) |
| "ForkInclusive", "JoinDynamic", "inclusive gateway", "OR gateway", "InclusiveBranch", "condition_flag" | CLAUDE.md §BPMN-Lite §Inclusive (OR) Gateway (Phase 5A) |
| "PostgresProcessStore", "store_postgres", "postgres feature", "database-url", "bpmn migrations" | CLAUDE.md §BPMN-Lite Phase 4 (PostgresProcessStore) |
| "authoring", "VerbContract", "ContractRegistry", "lint_contracts", "LintDiagnostic" | CLAUDE.md §BPMN-Lite §Authoring Pipeline (Phases B-D) |
| "export_bpmn", "dto_to_bpmn_xml", "BPMN XML export", "ir_to_dto", "round-trip" | CLAUDE.md §BPMN-Lite §Authoring Pipeline (Phases B-D) |
| "TemplateStore", "WorkflowTemplate", "publish_workflow", "compile_and_publish", "TemplateState" | CLAUDE.md §BPMN-Lite §Authoring Pipeline (Phases B-D) |
| "skeleton build", "skeleton_build_ops", "run_graph_validate", "run_ubo_compute", "run_coverage_compute", "run_outreach_plan", "run_tollgate_evaluate" | `rust/src/domain_ops/skeleton_build_ops.rs`, `KYC_SKELETON_FIXES.md` |
| "case transition", "CASE_TRANSITIONS", "KycCaseUpdateStatusOp", "update-status plugin", "is_valid_transition" | `rust/src/domain_ops/kyc_case_ops.rs`, `KYC_SKELETON_FIXES_S2.md` |
| "import run", "ImportRunBeginOp", "as_of", "idempotency", "case_import_runs" | `rust/src/domain_ops/import_run_ops.rs`, `rust/config/verbs/research/import-run.yaml` |

---

## Operator Vocabulary & Macros (058)

> ✅ **IMPLEMENTED (2026-01-28)**: Operator vocabulary layer, macro expansion, constraint cascade, DAG navigation.

**Problem Solved:** Implementation jargon (CBU, entity_ref, trading-profile) leaked to operators. Intent matching at 80%. Now operators see business terms (structure, party, mandate) and matching is 95%+.

### Architecture

```
Operator: "Set up a PE structure for Allianz"
    │
    ├─► Macro: structure.setup :type pe :name "..."
    │       │
    │       └─► Expands to: (cbu.create :kind private-equity :name "...")
    │
    └─► Constraint Cascade:
            client: Allianz → structure_type: PE → scope entity search
```

### Operator Domains (Macro Namespaces)

| Operator Domain | Wraps | Purpose |
|-----------------|-------|---------|
| `structure.*` | `cbu.*` | Fund structure operations |
| `party.*` | `entity.*` + `cbu-role.*` | People/orgs in roles |
| `case.*` | `kyc-case.*` | KYC case management |
| `mandate.*` | `trading-profile.*` | Investment mandates |

### Display Nouns (UI Never Shows Internal IDs)

| Internal | UI Shows |
|----------|----------|
| `cbu` | Structure / Client Unit |
| `entity` | Party |
| `trading-profile` | Mandate |
| `kyc-case` | Case |

### Macro Schema Example

> **IMPORTANT:** All macro YAML uses **kebab-case** for field names (`target-label`, `mode-tags`, `expands-to`).

```yaml
structure.setup:
  kind: macro
  id: structure.setup                    # Explicit ID (optional, defaults to YAML key)
  tier: composite                        # primitive | composite | template
  aliases: [setup-structure, new-fund]   # Alternative names
  taxonomy:
    domain: structure
    category: fund-setup
  docs-bundle: docs.bundle.ucits-baseline
  required-roles:
    - role: depositary
      cardinality: one
      entity-kinds: [bank, custodian]
  optional-roles:
    - role: prime-broker
      cardinality: zero-or-more
  ui:
    label: "Set up Structure"
    description: "Create a structure in the current client scope"
    target-label: "Structure"
  target:
    operates-on: client-ref
    produces: structure-ref
  args:
    required:
      structure-type:
        type: enum
        ui-label: "Type"
        values:
          - key: pe
            label: "Private Equity"
            internal: private-equity   # CRITICAL: UI key ≠ internal token
          - key: sicav
            label: "SICAV"
            internal: sicav
        default-key: pe
      depositary:
        type: party-ref
        ui-label: "Depositary"
        required-if: "structure-type = sicav"  # Conditional requirement
        placeholder-if-missing: true           # Auto-create placeholder entity
  expands-to:
    - verb: cbu.create
      args:
        kind: "${arg.structure-type.internal}"  # Uses internal token
        name: "${arg.name}"
```

### Advanced Macro Features (064)

**Conditional Expansion (`when:`):**
```yaml
expands-to:
  - when: "jurisdiction = LU"
    then:
      - verb: cbu.create
        args: { kind: "sicav", jurisdiction: "LU" }
    else:
      - verb: cbu.create
        args: { kind: "ucits" }
```

**Iteration (`foreach:`):**
```yaml
expands-to:
  - foreach: role
    in: "${required-roles}"
    do:
      - verb: party.assign
        args:
          structure-id: "${@structure}"
          role: "${role.role}"
          party-ref: "${role.party}"
```

**Condition Operators:**
- `var = value` - equality
- `var != value` - inequality
- `var in [a, b, c]` - membership
- `not: <condition>` - negation
- `any-of: [<cond1>, <cond2>]` - OR
- `all-of: [<cond1>, <cond2>]` - AND

**Conditional Requirements (`required-if:`):**
```yaml
depositary:
  type: party-ref
  required-if: "structure-type = sicav"  # Required only for SICAV
  
prime-broker:
  type: party-ref
  required-if:
    any-of:
      - "structure-type = hedge"
      - "has-prime-broker = true"
```

**Placeholder Entities (`placeholder-if-missing: true`):**
When arg is missing but `placeholder-if-missing: true`, expander generates:
```lisp
(entity.ensure-or-placeholder :kind "depositary" :ref "depositary-placeholder")
```

**Key Files:**
| File | Purpose |
|------|---------|
| `rust/src/dsl_v2/macros/schema.rs` | Schema types (`MacroSchema`, `WhenStep`, `ForEachStep`) |
| `rust/src/dsl_v2/macros/conditions.rs` | Condition evaluation for `when:` |
| `rust/src/dsl_v2/macros/expander.rs` | Macro expansion with When/ForEach/placeholder |
| `config/verb_schemas/macros/*.yaml` | Macro definitions (kebab-case) |

### Macro Lint (`cargo x verbs lint-macros`)

| Rule | Severity | What |
|------|----------|------|
| `MACRO011` | Error | UI fields required (label, description, target_label) |
| `MACRO012` | Error | Forbidden tokens in UI (cbu, entity_ref, etc.) |
| `MACRO042` | Error | No raw `entity_ref` - use `structure_ref`, `party_ref` |
| `MACRO043` | Error | `kinds:` only under `internal:` block |
| `MACRO044` | Error | Enums must use `${arg.X.internal}` in expansion |

### Constraint Cascade

```
1. CLIENT → Entities: 10,000 → 500
2. STRUCTURE TYPE → Structures: 500 → 50
3. VERB SCHEMA → Entity kinds for args
4. ENTITY → Search ONLY within scope
```

Session fields: `client`, `structure_type`, `current_structure`, `current_case`

### DAG Navigation

```
● structure.setup           ← START (ready)
  ├─► ○ structure.assign-role :role gp    (needs: setup)
  ├─► ○ structure.assign-role :role im    (needs: setup)
  └─► ○ case.open                         (needs: setup)
        └─► ○ case.approve                (needs: submit)

● = ready   ○ = blocked   ✓ = done
```

Prereq conditions: `VerbCompleted`, `AnyOf`, `StateExists`, `FactExists`

### Role Validation (Server-Side)

```yaml
role:
  type: enum
  values:
    - key: gp
      internal: general-partner
      valid-for: [pe, hedge]  # GP fails on SICAV
    - key: manco
      internal: manco
      valid-for: [sicav]      # ManCo fails on PE
```

### Key Files

| File | Purpose |
|------|---------|
| `rust/src/lint/macro_lint.rs` | MACRO000-MACRO080 lint rules |
| `rust/src/dsl_v2/macros/` | Macro registry, expander, schemas |
| `rust/src/session/unified.rs` | `UnifiedSession` with cascade context |
| `rust/src/mcp/macro_integration.rs` | DAG readiness, prereq checking |
| `config/verb_schemas/macros/` | Macro YAML definitions |

---

## Deprecated / Removed

| Removed | Replaced By |
|---------|-------------|
| `ViewMode` enum (5 modes) | Unit struct (always TRADING) |
| `OpenAIEmbedder` | `CandleEmbedder` (local) |
| `all-MiniLM-L6-v2` model | `bge-small-en-v1.5` (retrieval-optimized) |
| `embed()` / `embed_batch()` | `embed_query()` / `embed_target()` (asymmetric) |
| `IntentExtractor` (old MCP path) | MCP `verb_search` + `dsl_generate` |
| `AgentOrchestrator` | MCP pipeline |
| `verb_rag_metadata.rs` | YAML `invocation_phrases` + pgvector |
| `FeedbackLoop.generate_valid_dsl()` | MCP `dsl_generate` |
| `VerbPhraseIndex` (YAML phrase matcher) | `VerbService` + pgvector semantic search |
| Direct sqlx in `HybridVerbSearcher` | `VerbService` centralized DB access |
| `ob-poc-ui` (egui crate) | React frontend (`ob-poc-ui-react/`) |
| `esper_*` (6 navigation crates) | React frontend + REST API |
| `orchestrator.rs` (V1 REPL) | `orchestrator_v2.rs` (7-state machine, vnext-repl) |
| `session.rs` (V1 REPL) | `session_v2.rs` (runbook fold, vnext-repl) |
| `ReplState` (V1 5-state) | `ReplStateV2` in orchestrator_v2.rs |
| `ClientContext` / `JourneyContext` (mutable) | `ContextStack` (pure fold from runbook) |
| `HybridVerbSearcher` (V1 agent chat) | `IntentService` + `search_with_context()` (V2 REPL) |
| `IntentPipeline` (V1 agent chat) | `ReplOrchestratorV2.process()` (V2 REPL) |

---

## Domain Quick Reference

| Domain | Verbs | Purpose |
|--------|-------|---------|
| `cbu` | 25 | Client Business Unit lifecycle |
| `entity` | 30 | Natural/legal person management |
| `session` | 16 | Scope, navigation, history |
| `view` | 15 | Navigation verbs (legacy ESPER, now via React) |
| `trading-profile` | 30 | Trading matrix, CA policy |
| `kyc` | 20 | KYC case management |
| `investor` | 15 | Investor register, holdings |
| `custody` | 40 | Settlement, safekeeping |
| `gleif` | 15 | LEI lookup, hierarchy import |
| `research.*` | 30+ | External source workflows |
| `contract` | 14 | Legal contracts, rate cards, CBU subscriptions |
| `document` | 7 | Document solicitation, verification, rejection |
| `requirement` | 5 | Document requirements, waiver, status |
| `deal` | 30 | Deal record lifecycle, rate card negotiation, onboarding handoff |
| `billing` | 14 | Fee billing profiles, account targets, billing periods |
| `legal-entity` | 3 | Legal entity CRUD (booking principal layer) |
| `booking-location` | 3 | Booking location CRUD |
| `booking-principal` | 6 | Principal lifecycle, evaluation, selection |
| `service-availability` | 3 | Three-lane service availability |
| `client-principal-relationship` | 3 | Client ↔ principal relationships |
| `ruleset` | 3 | Eligibility ruleset lifecycle (draft → publish → retire) |
| `rule` | 3 | Individual eligibility rules within rulesets |
| `rule-field` | 2 | Closed-world field dictionary for rule validation |
| `contract-pack` | 3 | Contract template packs |
