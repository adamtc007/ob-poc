# CLAUDE.md

> **Last reviewed:** 2026-03-29
> **Frontend:** React/TypeScript (`ob-poc-ui-react/`) — Chat UI with scope panel, Inspector, Semantic OS Tab
> **Backend:** Rust/Axum (`rust/crates/ob-poc-web/`) — Serves React + REST API
> **Crates:** 22 active Rust crates (16 ob-poc + 6 sem_os_*)
> **Verbs:** 1,442 canonical verbs, 24,075 intent patterns (DB-sourced)
> **Macros:** 93 operator macros (20 YAML files, 16 domains, 3 composite), Tier -2B in intent pipeline
> **MCP Tools:** ~102 tools (DSL, verbs, learning, session, batch, research, taxonomy, sem_reg, stewardship, db_introspect, session_verb_surface)
> **Latest schema addition:** `rust/migrations/20260329_phrase_bank_materialization.sql`
> **Workspaces:** 7 (CBU, KYC, Deal, OnBoarding, ProductMaintenance, InstrumentMatrix, SemOsMaintenance)
> **Schema Overview:** `migrations/OB_POC_SCHEMA_ENTITY_OVERVIEW.md`
> **Embeddings:** Candle local (384-dim, BGE-small-en-v1.5) — 15,940 patterns vectorized

This is the root project guide. **Detailed implementation docs live in annex files** — see [Domain Annexes](#domain-annexes) at the bottom.

---

## Quick Start

```bash
cd rust/

# Agentic Scenario Harness
cargo x harness list                               # List all suites + scenario counts
cargo x harness run --all                           # Run all 48 scenarios (needs DATABASE_URL)
cargo x harness run --suite scenarios/suites/governance_strict.yaml
cargo x harness run --scenario direct_dsl_denied_viewer
cargo x harness dump --scenario direct_dsl_denied_viewer

# Pre-commit (fast)
cargo x pre-commit          # Format + clippy + unit tests

# Full check
cargo x check --db          # Include database integration tests

# Deploy (Full stack: React frontend + Rust backend)
cargo x deploy              # Build React + server + start

# Chrome DevTools MCP — Automated UI Testing
# Requires: Chrome with remote debugging (chrome://inspect/#remote-debugging)
# Add to .mcp.json: { "chrome": { "command": "npx", "args": ["-y", "chrome-devtools-mcp", "--autoConnect"] } }
# Available tools: navigate_page, take_screenshot, take_snapshot, click, type_text,
#   press_key, list_console_messages, list_network_requests, get_network_request
# Test fixture: tests/fixtures/ui_smoke_test.toml (tollgate flow + demo sequences)
# Usage in Claude Code session:
#   @chrome navigate to http://localhost:3000 and run the scope gate flow
#   @chrome take a screenshot after each tollgate transition
#   @chrome check for console errors after the full flow
cargo x deploy --skip-frontend  # Skip React rebuild (backend only)

# Run server directly (serves React from ob-poc-ui-react/dist/)
DATABASE_URL="postgresql:///data_designer" cargo run -p ob-poc-web

# React development (hot reload)
cd ob-poc-ui-react && npm run dev  # Runs on port 5173, proxies API to :3000

# BPMN-Lite service (standalone workspace at bpmn-lite/)
cargo x bpmn-lite build            # Build
cargo x bpmn-lite test             # Run all tests
cargo x bpmn-lite start            # Build release + start (port 50051)
cargo x bpmn-lite start --database-url postgresql:///data_designer  # With PostgresProcessStore

# Schema overview (living doc with mermaid ER diagrams)
npx md-to-pdf migrations/OB_POC_SCHEMA_ENTITY_OVERVIEW.md

# Refresh schema exports from the live source DB
cargo x schema-export

# Populate embeddings (REQUIRED after verb YAML changes)
DATABASE_URL="postgresql:///data_designer" \
  cargo run --release --package ob-semantic-matcher --bin populate_embeddings
# Use --force to re-embed all patterns (e.g., after model change)

# Semantic OS standalone server
SEM_OS_DATABASE_URL="postgresql:///data_designer" SEM_OS_JWT_SECRET=dev-secret cargo run -p sem_os_server
```

Current schema export target: `migrations/master-schema.sql` (canonical), `schema_export.sql` (convenience copy)

**SemOS is the hub for all things.** All paths lead to SemOS — nowhere else. The PostgreSQL schema is a supplementary store, a materialized projection, switchable if needed.

SemOS-first attribute lifecycle (2026-03-28):
- `AttributeDefBody` carries ALL metadata (category, validation_rules, applicability, is_derived, derivation_spec_fqn, etc.)
- `attribute.define` publishes SemOS snapshot FIRST, then materializes to `attribute_registry` via `materialize_to_store()`
- Materialization trigger on `sem_reg.snapshots` auto-projects active AttributeDef snapshots to `attribute_registry`
- Identity resolution prioritizes SemOS FQNs (precedence 0) over store UUIDs (precedence 1)
- SRDEF loader resolves attributes via SemOS first, with store fallback
- Store write functions restricted to `pub(crate)` — verb handlers are the only callers
- CI lint: `rust/scripts/lint_write_paths.sh` enforces no new raw SQL writes outside allowlisted paths

Derived attribute persistence (2026-03-27):
- runtime derived values persist in `"ob-poc".derived_attribute_values`
- dependency lineage persists in `"ob-poc".derived_attribute_dependencies`
- CBU consumers read canonical derived rows through `"ob-poc".v_cbu_derived_values`
- legacy `"ob-poc".cbu_attr_values` remains the direct/manual/non-derived observation plane
- `set_cbu_attr_value()` rejects `source = 'derived'` — derived values go canonical only

SemOS Maintenance workspace (2026-03-28):
- `WorkspaceKind::SemOsMaintenance` — first-class agentic workspace
- ScopeGate fork: "infrastructure" bypasses client group selection, routes directly to SemOS workspace
- Constellation family: `registry_governance`, map: `registry.stewardship` (7 slots)
- 4 state machines: `changeset_lifecycle`, `attribute_def_lifecycle`, `derivation_spec_lifecycle`, `service_resource_def_lifecycle`
- Pack: `semos-maintenance` with 40+ allowed verbs (changeset, governance, registry, attribute, typed-attribute, derivation, service-resource)
- 4 governance macros (Tier -2B): `governance.bootstrap-attribute-registry`, `governance.define-service-dictionary`, `governance.full-publish-pipeline`, `governance.reconcile-registry`
- 4 governance scenarios (Tier -2A): compound intent resolution for SemOS maintenance utterances
- New verbs: `service-resource.check-attribute-gaps`, `service-resource.sync-definitions`, `typed-attribute.record/get/list-for-entity`, `derivation.recompute-stale`, `attribute.bridge-to-semos`
- Verb search: 6 phrasing detection improvements (domain_filter bypass for semantic/macro/scenario/learned tiers, short query threshold scaling, multi-domain pack dominant_domain suppression, noun index for new domains)
- Utterance test harness: 271 test cases across all 7 workspaces, per-workspace hit rate reporting
- Hit rates: 62.4% first-attempt, 84.5% two-attempt (all workspaces above 30%)
- Governed phrase authoring (v1.2): `phrase_bank` table (13,570 entries), `phrase_mapping` SemOS object type, `phrase_authoring_lifecycle` state machine (8 states), 9 phrase.* verbs, AI proposal pipeline with 5-signal confidence scoring + risk-tiered approval routing
- Onboarding product macros: `structure.product-suite-custody-fa-ta`, `structure.product-suite-full`, `structure.remove-all-products` — compound intent → multi-step runbook → per-entity expansion → DAG-ordered → confirm all → execute atomically
- Macro priority: ScenarioIndex (1.05) > MacroIndex (1.04) > exact phrase (1.0) — macros always win over single verbs when both match (safer, atomic, complete)
- Per-entity macro expansion: runbook compiler replicates macro steps per CBU UUID in scope
- Macro audit (2026-03-29): fixed `expands_to` → `expands-to` YAML key in `attribute.seed-*` macros (serde kebab-case deserialization bug); removed 8 KYC-domain macros from `book-setup` pack (screening, case, kyc-workflow macros leaked into CBU/InstrumentMatrix workspaces); added search overrides for `screening-ops.*` workstream-level macros; two screening families coexist: `screening.*` (party-level ad-hoc) and `screening-ops.*` (workstream-level KYC)
- PACK001 lint rule: workspace-macro bleed detection — checks every macro in a pack's `allowed_verbs` has mode-tags compatible with all pack workspaces; prevents KYC/screening macros from leaking into CBU/InstrumentMatrix contexts; workspace-to-mode-tag compatibility table in `docs/annex-macros.md`
- `cargo clippy` clean across entire codebase

---

## Non-Negotiable Implementation Rules

### 1. Type Safety First

**Never use untyped JSON (`serde_json::json!`) for structured data.** Always define typed structs.

```rust
// WRONG - Untyped
Ok(ExecutionResult::Record(serde_json::json!({ "groups_created": row.0 })))

// CORRECT - Typed struct with Serialize/Deserialize
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeriveGroupsResult { pub groups_created: i32 }
let result = DeriveGroupsResult { groups_created: row.0 };
Ok(ExecutionResult::Record(serde_json::to_value(result)?))
```

**Where to define types:** Domain results → `ob-poc-types`, DSL config → `dsl-core/src/config/types.rs`, API → near handler or shared `types.rs`.

### 2. Consistent Return Types

| YAML `returns.type` | Rust Pattern |
|---------------------|--------------|
| `uuid` | `ExecutionResult::Uuid(uuid)` |
| `record` | `ExecutionResult::Record(serde_json::to_value(typed_struct)?)` |
| `record_set` | `ExecutionResult::RecordSet(...)` |
| `affected` | `ExecutionResult::Affected(count)` |
| `void` | `ExecutionResult::Void` |

### 3. Option<T> for Nullable/Optional Values
Use `Option<T>` consistently. No sentinel values or silent nulls.

### 4. Error Types over Panics
Use `Result<T, E>` and `?` operator. Never `.unwrap()` in production code paths.
```rust
let value = map.get("key").ok_or_else(|| anyhow!("Missing key"))?;
```

### 5. Re-export Types at Module Boundary
When a module uses types from another crate, re-export them for consumers.

---

## Core Architecture: CBU-Centric Model

**CBU (Client Business Unit) is the atomic unit.** Everything resolves to sets of CBUs.

```
Session = Set<CBU>
  Universe (all CBUs)
    └── Book (commercial client's CBUs: Allianz, BlackRock)
         └── CBU (single trading unit)
              ├── TRADING view (default) — instruments, counterparties
              └── UBO view (KYC mode) — ownership/control taxonomy
  Group structure cross-links CBUs via ownership/control edges
  Clusters/galaxies are DERIVED from these edges, not stored
```

- **ViewMode:** Unit struct — always TRADING. Use `view.cbu :mode ubo` for KYC/UBO.
- **GraphScope:** `Empty | SingleCbu | Book | Jurisdiction | EntityNeighborhood | Custom`
- **Session = Run Sheet = Viewport Scope:** Session is single source of truth. `entity_scope.cbu_ids` drives the viewport. Run sheet tracks per-statement `DslStatus`: Draft → Ready → Executing → Executed → Failed → Cancelled.

### Unified Session Pipeline

All user input routes through `POST /api/session/:id/input` → `ReplOrchestratorV2.process()`.

**Mandatory tollgate sequence:**
1. **ScopeGate** → Client group selection (non-negotiable)
2. **WorkspaceSelection** → KYC | OnBoard | CBU | Deal | Product Maint | Instrument Matrix
3. **JourneySelection** → Pack selection within workspace
4. **InPack** → Verb matching + sentence generation
5. **SentencePlayback** → Confirm/reject proposed DSL
6. **RunbookEditing** → Review runsheet
7. **Executing** → Step-by-step execution with narration

`ReplSessionV2` is the canonical session. `UnifiedSession` retained for execution context only. `CbuSession` removed. Response adapter converts `ReplResponseV2` → `ChatResponse` for frontend compatibility.

**Closed-loop invariant:** After verb execution (`writes_since_push > 0`), the TOS constellation is re-hydrated from the database before building the response. This ensures the UI always renders post-execution entity state (updated slot states, available verbs, progress). Constellation refresh is triggered by entity state changes, not every turn. 153/153 REPL V2 tests pass.

> **Key files:** `rust/src/repl/orchestrator_v2.rs` (orchestrator), `rust/src/api/response_adapter.rs` (adapter), `rust/src/api/agent_enrichment.rs` (onboarding state enrichment)

---

## DSL Pipeline (Single Path)

ALL DSL generation goes through: **User → verb_search → dsl_generate (LLM extracts args as JSON) → deterministic DSL assembly → dsl_execute**

```
Search Priority (10-tier):
-2A. ScenarioIndex (journey-level compound intent, score 0.97)
-2B. MacroIndex (macro search parity, score 0.96)
 -1. ECIR noun taxonomy (deterministic, score 0.95)
  0. Operator macros (1.0 exact / 0.95 fuzzy)
  1-2. Learned exact (1.0)
  3. User semantic (pgvector, BGE asymmetric)
  5. Blocklist filter
  6. Global semantic fallback
  7. Phonetic fallback (0.80)
```

**PolicyGate:** Server-side single-pipeline enforcement. `SemOsContextEnvelope` replaces `SemRegVerbPolicy`: carries allowed verbs, pruned verbs with structured `PruneReason` (7 variants: AbacDenied, EntityKindMismatch, TierExcluded, TaxonomyNoOverlap, PreconditionFailed, AgentModeBlocked, PolicyDenied), `AllowedVerbSetFingerprint` (SHA-256), TOCTOU recheck. Pre-constrained verb search threads allowed verbs into `HybridVerbSearcher`.

**SessionVerbSurface:** 6-step compute pipeline (was 8): Registry → AgentMode → Scope+Workflow (merged) → SemReg CCIR → Lifecycle → Rank+CompositeStateBias. FailClosed default = ~30 safe-harbor verbs. Dual fingerprints: `vs1:<hex>` (surface) vs `v1:<hex>` (SemReg).

**LLM removed from semantic loop:** Verb discovery is pure Rust (5-15ms via Candle). LLM used only for arg extraction (200-500ms).

**Key files:** `rust/src/agent/orchestrator.rs`, `rust/src/mcp/verb_search.rs`, `rust/src/mcp/intent_pipeline.rs`, `rust/src/agent/sem_os_context_envelope.rs`, `rust/src/agent/verb_surface.rs`, `rust/src/mcp/noun_index.rs`, `rust/src/mcp/scenario_index.rs`, `rust/src/mcp/macro_index.rs`

> **Full details:** `docs/annex-dsl-and-intent.md`

---

## React Frontend

```
ob-poc-ui-react/src/
├── api/              # API client (chat.ts, scope.ts, semOs.ts)
├── features/chat/    # Agent chat UI with scope panel + constellation panel
├── features/semantic-os/  # Semantic OS workflow UI (/semantic-os route)
├── features/inspector/    # Projection inspector (tree + detail)
├── stores/           # Zustand state management
└── types/            # TypeScript types
```

**Key Endpoints:**

| Endpoint | Purpose |
|----------|---------|
| `POST /api/session` | Create session (optional `workflow_focus`) |
| `POST /api/session/:id/input` | **Unified ingress** (`kind`: utterance / decision_reply / repl_v2) |
| `GET /api/session/:id/scope-graph` | Loaded CBUs (scope) |
| `GET /api/cbu/:id/constellation` | Hydrated constellation tree |
| `GET /api/cbu/:id/cases` | KYC cases for constellation binding |
| `GET /api/projections/:id` | Inspector projection |
| `GET /api/sem-os/context` | Registry stats + recent changesets |
| `GET /api/constellation/by-name` | Resolve CBU by name + hydrate constellation |
| `POST /api/session/:id/runbook/compile` | Compile multi-workspace runbook plan |
| `GET /api/session/:id/runbook/plan` | Get current runbook plan |
| `POST /api/session/:id/runbook/approve` | Approve compiled plan for execution |
| `POST /api/session/:id/runbook/execute` | Execute next plan step (INV-3 gate) |
| `POST /api/session/:id/runbook/cancel` | Cancel plan mid-execution |
| `GET /api/session/:id/runbook/status` | Current plan status + cursor |
| `GET /api/session/:id/trace` | Session trace (append-only mutation log) |
| `GET /api/session/:id/trace/:seq` | Single trace entry by sequence |
| `POST /api/session/:id/trace/replay` | Replay trace (strict/relaxed/dry_run) |

**Legacy (410 Gone):** `POST /chat`, `POST /decision/reply`, `POST /repl-input`, `POST /select-verb`

```bash
cd ob-poc-ui-react && npm install && npm run dev  # Hot reload on :5173
npm run build && npm run typecheck && npm run lint
```

> **Full details:** `docs/annex-frontend-and-tools.md`

---

## Feature Status

**Complete (✅):** React Migration (077), V2 REPL (7-state, 320 tests), Runbook Compilation, Candle Semantic Pipeline, Agent Pipeline + PolicyGate, Solar Navigation (038), Promotion Pipeline (043), Teaching (044), Client Group Resolver (048), Workflow Task Queue (049), Transactional Execution (050), CustomOp Auto-Registration (051), Client Group Research (055), REPL Viewport Feedback (056), Verb Disambiguation UI (057), Unified Architecture (058), Playbook System (059), LSP (060/063), CBU Structure Macros (064), Unified Lookup (074), Lexicon (072), Entity Linking (073), Clarification UX (075), Inspector-First (076), Deal Record & Fee Billing (067), BPMN-Lite (all phases incl. Phase 4 PostgresProcessStore + Phase 5A Inclusive Gateway), BPMN-Lite Integration (Phase B), BPMN-Lite Authoring (Phases B-D), KYC/UBO Skeleton (S1-S2), Semantic OS (Phases 0-9 + Standalone v1.1 + Stewardship Phase 0-1), Governed Registry Authoring (v0.4, migrations 099-102), CCIR + SessionVerbSurface, Loopback Calibration (v0.3), Onboarding State View, Verb Disambiguation UX, Constellation Orphan Remediation, SemOS Grounded Action Surface, Pipeline Leak Remediation, Sage Intent Skeleton (Phase 1), Entity-First Utterance Parsing, Coder Rewrite (Phase 2), Sage-Primary Chat Narration, SemTaxonomy Three-Step, NLCI CBU Cutover, CBU Role Surface Reconciliation, Phase 0 Vocabulary Rationalization (Batches 1-3), Schema Consolidation (115-121), Domain Metadata Coverage (306/306 tables), Scenario-Based Intent Resolution (Phases 0.5-5), AffinityGraph & Diagram Generation, Discovery Pipeline (Phase 2), Utterance API Coverage Harness, Unified Session Input Cutover, Workspace-Scoped REPL Navigation, SemOS Attribute DSL + Schema Cleanup, SemOS Footprint Hydration S6, SemOS Document Governance Bootstrap (122-123), StateGraph Pipeline (Phase 0-3 substrate), Session Stack Machine Runbook Architecture (R1-R9, migrations 125-128), Unified Session Pipeline (ADR 040 — tollgates enforced, 149/149 tests, response adapter, dead code removal -4,480 lines), Derived Attribute Persistence (D0-D12 — canonical two-table model, staleness propagation, CBU projection view), SemOS-First Hub Implementation (Phases 1-7 — AttributeDefBody complete, SemOS-first write path, materialization trigger, identity resolution inverted, 7 new verbs, SemOS Maintenance workspace)

**In Progress / Parked (⚠️):** Sage/Coder GATE 5 (existing 43%, Sage+Coder 5% — vocabulary/routing work needed), Three-Step Harness (7.95% exact / 71% grounded — metadata quality is limiter), StateGraph Phase 1 reconciliation (parked pending external correction table)

**Removed (❌):** V1 Staged Runbook (054), ESPER Navigation Crates (065 — retained for reference)

---

## Session & Navigation Verbs

| Verb | Purpose |
|------|---------|
| `session.load-cbu / load-galaxy / load-jurisdiction` | Add CBUs to session |
| `session.unload-cbu`, `session.clear` | Remove CBUs |
| `session.undo / redo`, `session.info / list` | History & state |
| `view.universe / book / cbu` | Zoom levels |
| `view.drill / surface / trace / xray / refine` | Navigation within CBU |

All user input goes through unified `IntentPipeline` → `HybridVerbSearcher` → semantic match. No separate ESPER path.

---

## Adding Verbs

> ⚠️ **Read `docs/verb-definition-spec.md` before writing verb YAML.** Serde structs are strict. Invalid YAML silently fails to load.

**Behaviors:** `crud` (generic executor), `plugin` (`#[register_custom_op]` macro), `template` (multi-statement DSL expansion)

**Plugin verb pattern:**
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
    async fn execute(&self, verb_call: &VerbCall, ctx: &mut ExecutionContext, pool: &PgPool) -> Result<ExecutionResult> { /* ... */ }
}
```

```bash
cargo x verbs check   # YAML matches DB
cargo x verbs lint    # Tiering rules
# After YAML changes — MUST run or new verbs won't be discoverable:
cargo x verbs compile && DATABASE_URL="postgresql:///data_designer" \
  cargo run --release -p ob-semantic-matcher --bin populate_embeddings
```

Verify plugin coverage: `cargo test --lib -- test_plugin_verb_coverage`

---

## Code Patterns

- **Config Struct Pattern:** Builder-style for types with many optional params. See `ToolHandlersConfig` in `rust/src/mcp/handlers/core.rs`.
- **Centralized DB Access:** All through service modules in `rust/src/database/`. No direct `sqlx::query` outside services.
- **Actor Resolution:** `ActorResolver::from_headers()` (HTTP), `from_env()` (MCP), `from_session_id()` (REPL). Default role: `viewer`.
- **Strum-based enums:** Core enums use `strum` derives (`Display`, `EnumString`, `AsRefStr`) — eliminates manual `as_str()`/`from_str()`.
- **`#[must_use]` on decision types:** `GateResult`, `ValidationReport`, `DryRunReport` must not be silently discarded.

---

## Key Directories

```
ob-poc/
├── bpmn-lite/                  # Standalone BPMN orchestration (NOT inside rust/)
│   ├── bpmn-lite-core/         # Core: types, compiler, VM, store, authoring
│   └── bpmn-lite-server/       # gRPC server (tonic), proto definitions
├── ob-poc-ui-react/            # React/TypeScript frontend (PRIMARY UI)
│   ├── src/features/           # Chat, Inspector, Semantic OS, Settings
│   └── dist/                   # Production build (served by Rust)
├── rust/
│   ├── config/verbs/           # 103 YAML verb definitions
│   ├── config/packs/           # 5 V2 REPL journey packs (onboarding, book-setup, kyc-case, deal-lifecycle, product-service-taxonomy)
│   ├── config/sem_os_seeds/    # Domain metadata, constellation maps, state machines
│   ├── config/noun_index.yaml  # 99-noun ECIR taxonomy
│   ├── config/scenario_index.yaml  # 16 journey scenario definitions
│   ├── crates/
│   │   ├── dsl-core/           # Parser, AST, compiler (no DB)
│   │   ├── dsl-lsp/            # LSP server + Zed extension + tree-sitter grammar
│   │   ├── ob-agentic/         # Onboarding pipeline (Intent→Plan→DSL)
│   │   ├── ob-poc-macros/      # Proc macros (#[register_custom_op])
│   │   ├── ob-poc-web/         # Axum web server (serves React + API)
│   │   ├── inspector-projection/ # Projection schema generation
│   │   ├── sem_os_core/        # Canonical types, ports, service logic
│   │   ├── sem_os_postgres/    # PostgreSQL store implementations
│   │   ├── sem_os_server/      # Standalone REST server + JWT
│   │   ├── sem_os_client/      # Client trait (InProcess + HTTP)
│   │   ├── sem_os_harness/     # Integration test harness
│   │   └── sem_os_obpoc_adapter/ # Verb YAML → seed bundles
│   ├── src/
│   │   ├── agent/              # Orchestrator, verb surface, onboarding state, context envelope
│   │   ├── repl/               # V2 REPL (30 files, always enabled) + session_trace, trace_repository, session_replay
│   │   ├── journey/            # Pack system (router, manifests, handoff)
│   │   ├── domain_ops/         # CustomOperation implementations (~300+ ops)
│   │   ├── sem_reg/            # Semantic Registry + stewardship (39 files)
│   │   ├── mcp/                # MCP tools, handlers, verb search, intent pipeline
│   │   ├── bpmn_integration/   # ob-poc ↔ bpmn-lite wiring (12 files)
│   │   ├── calibration/        # Loopback calibration (11 modules)
│   │   └── api/                # REST routes
│   ├── tests/                  # Integration tests + golden corpus
│   └── scenarios/suites/       # 10 suites, 48 agentic test scenarios
├── migrations/                 # 128 SQLx migrations
├── docs/                       # Architecture docs + annexes
├── ai-thoughts/                # ADRs and design docs
└── artifacts/                  # Calibration packs, footprints, peer review
```

---

## Environment Variables

```bash
# Required
DATABASE_URL="postgresql:///data_designer"
AGENT_BACKEND=anthropic
ANTHROPIC_API_KEY="sk-ant-..."

# Optional
BPMN_LITE_GRPC_URL=http://localhost:50052   # Enable BPMN integration
SEM_OS_MODE=inprocess                        # inprocess | remote
SEM_OS_DATABASE_URL="postgresql:///..."      # For standalone sem_os_server
SEM_OS_JWT_SECRET=dev-secret                 # JWT for sem_os_server
OBPOC_STRICT_SINGLE_PIPELINE=true            # PolicyGate (default: true)
OBPOC_STRICT_SEMREG=true                     # SemReg fail-closed (default: true)
OBPOC_ALLOW_RAW_EXECUTE=false                # Allow /execute with raw DSL
SAGE_FAST_PATH=1                             # Read+structure fast path
BRAVE_SEARCH_API_KEY="..."                   # Research macros
```

---

## Database Practices

```bash
# After schema changes
psql -d data_designer -f your_migration.sql
cd rust && cargo sqlx prepare --workspace
cargo build  # Catches type mismatches
```

| PostgreSQL | Rust |
|------------|------|
| UUID | `Uuid` |
| TIMESTAMPTZ | `DateTime<Utc>` |
| INTEGER / BIGINT | `i32` / `i64` |
| NUMERIC | `BigDecimal` |
| NULLABLE | `Option<T>` |

---

## Error Handling

Never `.unwrap()` in production. Use `?`, `.ok_or_else(|| anyhow!(...))`, `let Some(x) = ... else { continue }`, `match` / `if let`.

---

## Testing

### Backend Test Suites

```bash
# Unified pipeline tollgate tests (9 tests — gates + trace + adapter)
cargo test --test unified_pipeline_tollgates

# Full REPL V2 suite (149 tests across 8 files)
cargo test --test repl_v2_golden_loop --test repl_v2_phase2 --test repl_v2_phase3 \
  --test repl_v2_phase4 --test repl_v2_phase5 --test repl_v2_phase6 --test repl_v2_integration

# Intent hit rate (needs DATABASE_URL)
DATABASE_URL="postgresql:///data_designer" cargo test --features database --test intent_hit_rate -- --ignored --nocapture
```

### Chrome DevTools MCP — Live UI Testing

Automated browser testing via Chrome DevTools MCP. Claude Code can navigate, type, click, screenshot, inspect DOM, and verify console errors against the live UI.

**Setup:** Chrome with remote debugging enabled + `chrome-devtools-mcp` in `.mcp.json`.

**Capabilities:**
- **Smoke tests:** Automated tollgate flow (scope → workspace → journey → pack → verb)
- **Screenshot regression:** Capture expected states, compare on changes
- **Demo animations:** Scripted user flows at human-readable pace for presentations
- **Bulk phrase testing:** Fire 100+ utterances through the live UI, verify each gate renders
- **Console error detection:** Zero-error verification after every flow step
- **Network inspection:** Verify API response shapes match frontend expectations

**Test fixture:** `tests/fixtures/ui_smoke_test.toml` — 12 test cases + 1 demo sequence covering all 3 tollgates.

**Usage in Claude Code:**
```
@chrome navigate to http://localhost:3000 and create a new session
@chrome type "Allianz" and press Enter, then screenshot
@chrome click the CBU workspace button and verify journey options appear
@chrome check for console errors
```

---

## Domain Quick Reference

| Domain | Verbs | Purpose |
|--------|-------|---------|
| `cbu` | 25 | Client Business Unit lifecycle |
| `entity` | 30 | Natural/legal person management |
| `session` | 16 | Scope, navigation, history |
| `view` | 15 | Navigation verbs |
| `trading-profile` | 30 | Trading matrix, CA policy |
| `kyc` | 20 | KYC case management |
| `investor` | 15 | Investor register, holdings |
| `custody` | 40 | Settlement, safekeeping |
| `gleif` | 15 | LEI lookup, hierarchy import |
| `research.*` | 30+ | External source workflows |
| `contract` | 14 | Legal contracts, rate cards, subscriptions |
| `document` | 7 | Document solicitation, verification |
| `deal` | 30 | Deal lifecycle, rate card negotiation |
| `billing` | 14 | Fee billing profiles, periods |
| `ownership` | 4 | Ownership graph pipeline |
| `registry` | 20 | SemReg object CRUD |
| `changeset` | 14 | Changeset authoring |
| `governance` | 9 | Publish gates, impact, rollback |
| `schema` | 5 | Schema introspection |
| `agent` | 4+ | Agent mode/policy, telemetry |
| `sem_reg.*` | ~32 | Semantic Registry MCP tools |

---

## Domain Annexes

**Detailed implementation docs extracted from CLAUDE.md into topic-specific annexes:**

| When working on... | Read this annex |
|--------------------|-----------------|
| DSL pipeline, verb search, embeddings, intent resolution, disambiguation, teaching, promotion, scenarios, ECIR, AffinityGraph, discovery | `docs/annex-dsl-and-intent.md` |
| Semantic OS, SemReg, context resolution, ABAC, stewardship, governed authoring, CCIR, verb surface, scanner | `docs/annex-sem-os.md` |
| BPMN-Lite service, fiber VM, race semantics, gRPC, orchestration, bpmn_integration | `docs/annex-bpmn-lite.md` |
| V2 REPL, packs, scoring, preconditions, context stack, golden corpus, replay tuner | `docs/annex-repl-v2.md` |
| Macros: operator vocabulary, expansion engine, MacroIndex, lint, composite macros, state DAG, pack mapping | `docs/annex-macros.md` |
| Contracts, deals, billing, client groups, documents, entity linking, inspector, lexicon, lookup, playbooks, transactional execution | `docs/annex-domain-features.md` |
| React frontend details, Zed extension, LSP, ob-agentic onboarding pipeline | `docs/annex-frontend-and-tools.md` |

**Pre-existing annexes (unchanged):**

| When working on... | Read this annex |
|--------------------|-----------------|
| Semantic pipeline details | `docs/agent-semantic-pipeline.md` |
| Agent/MCP pipeline | `docs/agent-architecture.md` |
| Session & navigation | `docs/session-visualization-architecture.md` |
| Data model (CBU/Entity/UBO) | `docs/strategy-patterns.md` §1 |
| Verb authoring | `docs/verb-definition-spec.md` |
| Entity model & schema | `docs/entity-model-ascii.md` |
| Schema overview (living doc) | `migrations/OB_POC_SCHEMA_ENTITY_OVERVIEW.md` |
| DSL pipeline flow | `docs/dsl-verb-flow.md` |
| Research workflows | `docs/research-agent-annex.md` |
| V2 REPL invariants | `docs/INVARIANT-VERIFICATION.md` |

### AI-Thoughts (Design Decisions)

| Topic | Document |
|-------|----------|
| Group/UBO ownership | `ai-thoughts/019-group-taxonomy-intra-company-ownership.md` |
| Research workflows | `ai-thoughts/020-research-workflows-external-sources.md` |
| Entity disambiguation | `ai-thoughts/025-entity-disambiguation-ux.md` |
| Trading matrix pivot | `ai-thoughts/027-trading-matrix-canonical-pivot.md` |
| Entity resolution | `ai-thoughts/033-entity-resolution-wiring-plan.md` |
| REPL state model | `ai-thoughts/034-repl-state-model-dsl-agent-protocol.md` |
| Session-runsheet-viewport | `ai-thoughts/035-session-runsheet-viewport-integration.md` |
| Solar navigation | `ai-thoughts/038-solar-navigation-unified-design.md` |
| Lexicon service | `ai-thoughts/072-lexicon-service-implementation-plan.md` |
| Entity linking | `ai-thoughts/073-entity-linking-implementation-plan.md` |

---

## Trigger Phrases

When you see these in a task, read the corresponding annex first:

| Phrase | Read |
|--------|------|
| "add verb", "create verb", "verb YAML" | `docs/verb-definition-spec.md` |
| "React", "frontend", "chat UI", "scope panel", "constellation panel" | `docs/annex-frontend-and-tools.md` |
| "entity model", "CBU", "UBO", "holdings" | `docs/strategy-patterns.md` §1 |
| "schema overview", "table structure", "ER diagram", "mermaid" | `migrations/OB_POC_SCHEMA_ENTITY_OVERVIEW.md` |
| "agent", "MCP", "verb_search", "intent pipeline", "orchestrator" | `docs/annex-dsl-and-intent.md` |
| "session", "scope", "navigation", "ESPER", "ViewState" | `docs/session-visualization-architecture.md` |
| "DSL pipeline", "PolicyGate", "single pipeline" | `docs/annex-dsl-and-intent.md` |
| "embeddings", "Candle", "BGE", "populate_embeddings" | `docs/annex-dsl-and-intent.md` |
| "promotion", "teaching", "learning", "phrase", "blocklist" | `docs/annex-dsl-and-intent.md` |
| "disambiguation", "VerbOption", "intent tier", "clarification" | `docs/annex-dsl-and-intent.md` |
| "ScenarioIndex", "MacroIndex", "CompoundSignals", "ECIR", "NounIndex" | `docs/annex-dsl-and-intent.md` |
| "AffinityGraph", "DiagramModel", "MermaidRenderer", "DomainMetadata" | `docs/annex-dsl-and-intent.md` |
| "discovery", "registry.discover-dsl", "schema.generate" | `docs/annex-dsl-and-intent.md` |
| "semantic registry", "sem_reg", "semantic os", "context resolution" | `docs/annex-sem-os.md` |
| "ABAC", "security label", "governance tier", "trust class", "proof rule" | `docs/annex-sem-os.md` |
| "stewardship", "changeset", "guardrails", "show loop", "focus" | `docs/annex-sem-os.md` |
| "authoring", "propose", "validate", "dry-run", "publish", "AgentMode" | `docs/annex-sem-os.md` |
| "CCIR", "ContextEnvelope", "PruneReason", "AllowedVerbSetFingerprint", "TOCTOU" | `docs/annex-sem-os.md` |
| "SessionVerbSurface", "verb surface", "FailClosed", "safe-harbor" | `docs/annex-sem-os.md` |
| "GroundedActionSurface", "pipeline leak", "TOCTOU recheck" | `docs/annex-sem-os.md` |
| "scanner", "drift detection", "bootstrap", "seed bundle" | `docs/annex-sem-os.md` |
| "BPMN", "bpmn-lite", "fiber VM", "orchestration", "durable workflow" | `docs/annex-bpmn-lite.md` |
| "race", "boundary timer", "cancel", "ghost signal", "terminate" | `docs/annex-bpmn-lite.md` |
| "WorkflowDispatcher", "JobWorker", "EventBridge", "correlation", "parked token" | `docs/annex-bpmn-lite.md` |
| "PendingDispatch", "queue resilience", "dispatch worker" | `docs/annex-bpmn-lite.md` |
| "EndTerminate", "error boundary", "ErrorRoute", "BusinessRejection" | `docs/annex-bpmn-lite.md` |
| "IncCounter", "BrCounterLt", "bounded loop", "ForkInclusive", "JoinDynamic" | `docs/annex-bpmn-lite.md` |
| "PostgresProcessStore", "bpmn migrations", "authoring pipeline" | `docs/annex-bpmn-lite.md` |
| "REPL", "V2 REPL", "orchestrator v2", "context stack", "pack" | `docs/annex-repl-v2.md` |
| "scoring", "preconditions", "golden corpus", "replay tuner" | `docs/annex-repl-v2.md` |
| "VerbSearchIntentMatcher", "IntentMatcher", "3-pronged" | `docs/annex-repl-v2.md` |
| "compile_invocation", "CompiledRunbook", "RunbookStore", "execute_runbook" | `docs/annex-repl-v2.md` |
| "FocusMode", "DecisionLog", "ExclusionSet" | `docs/annex-repl-v2.md` |
| "macro", "operator vocabulary", "structure.setup", "MacroSchema", "expands-to" | `docs/annex-macros.md` |
| "invoke-macro", "MacroExpansionStep", "MacroIndex", "macro lint", "MACRO0" | `docs/annex-macros.md` |
| "screening-ops", "screening.full", "macro search overrides", "macro audit" | `docs/annex-macros.md` |
| "SequenceValidator", "CompoundSignals", "FixpointExpansion", "macro DAG" | `docs/annex-macros.md` |
| "PACK001", "workspace bleed", "mode-tags", "workspace_accepts_mode_tag", "fail closed" | `docs/annex-macros.md` |
| "new workspace", "add workspace", "WorkspaceKind", "workspace checklist" | `docs/annex-macros.md` |
| "constraint cascade" | `docs/annex-domain-features.md` |
| "contract", "deal", "billing", "rate card", "subscription" | `docs/annex-domain-features.md` |
| "client group", "alias", "anchor", "resolver" | `docs/annex-domain-features.md` |
| "entity linking", "mention extraction", "EntityLinkingService" | `docs/annex-domain-features.md` |
| "lexicon", "LexiconService", "bincode snapshot" | `docs/annex-domain-features.md` |
| "lookup service", "verb-first", "LookupService" | `docs/annex-domain-features.md` |
| "document", "requirement", "task queue", "cargo ref", "rejection" | `docs/annex-domain-features.md` |
| "inspector", "projection", "node_id", "ref_value" | `docs/annex-domain-features.md` |
| "transactional execution", "advisory lock", "expansion report" | `docs/annex-domain-features.md` |
| "skeleton build", "KYC case transition", "import run" | `docs/annex-domain-features.md` |
| "OnboardingStateView", "constellation", "forward_verbs", "revert_verbs" | `docs/annex-domain-features.md` |
| "playbook", "LSP", "language server", "Zed extension", "tree-sitter" | `docs/annex-frontend-and-tools.md` |
| "onboarding pipeline", "RequirementPlanner", "ob-agentic" | `docs/annex-frontend-and-tools.md` |
| "invariant", "P-1", "P-2", "P-3", "P-4", "P-5" | `docs/INVARIANT-VERIFICATION.md` |
| "runbook plan", "RunbookPlan", "multi-workspace execution", "plan compiler" | `rust/src/runbook/plan_compiler.rs`, `rust/src/runbook/plan_types.rs` |
| "session trace", "TraceEntry", "TraceOp", "trace replay", "session_replay" | `rust/src/repl/session_trace.rs`, `rust/src/repl/session_replay.rs` |
| "plan executor", "advance_plan_step", "forward ref", "EntityBinding" | `rust/src/runbook/plan_executor.rs`, `rust/src/runbook/plan_types.rs` |
| "narration", "StepNarration", "PlanNarration", "effect narration" | `rust/src/runbook/narration.rs` |
| "VerbOutput", "verb output", "outputs declaration" | `rust/crates/sem_os_core/src/verb_contract.rs` |
| "stack machine", "workspace stack", "writes_since_push", "is_peek" | `rust/src/repl/types_v2.rs`, `rust/src/repl/session_v2.rs` |

---

## Deprecated / Removed

| Removed | Replaced By |
|---------|-------------|
| `ViewMode` enum (5 modes) | Unit struct (always TRADING) |
| `OpenAIEmbedder` / `all-MiniLM-L6-v2` | `CandleEmbedder` / `bge-small-en-v1.5` |
| `ob-poc-ui` (egui) / `esper_*` crates | React frontend (`ob-poc-ui-react/`) |
| V1 REPL / `ReplState` / `ClientContext` | V2 REPL (`orchestrator_v2.rs`, `ContextStack`) |
| `ob-poc-graph` / `viewport` crate | React + REST API |
| `SemRegVerbPolicy` | `SemOsContextEnvelope` |
| Direct DSL bypass (`dsl:` prefix) | SemReg-filtered pipeline |
| `IntentPipeline` (V1 agent chat) | `ReplOrchestratorV2.process()` |
| V1 Staged Runbook (054) | V2 REPL pack-guided runbook |
| `manco` domain name | Renamed to `ownership` domain |
| `CbuSession` / `cbu_session_routes.rs` | Unified pipeline (`session_scoped_router`) |
| `agent_dsl_routes.rs` (DSL parse/resolve/generate) | Unified REPL pipeline |
| `agent_learning_routes.rs` (corrections/disambiguation) | Unified REPL pipeline |
| `vnext-repl` feature flag | Always enabled (flag deprecated) |
| Legacy `chat_session()` fallback in `session_input` | `try_route_through_repl()` |
