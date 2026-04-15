# Frontend, Dev Tools & Onboarding Pipeline — Detailed Annex

> This annex contains detailed React frontend, Zed extension, LSP,
> and ob-agentic onboarding pipeline documentation extracted from CLAUDE.md.
> For the high-level overview, see the root `CLAUDE.md`.

---

## React Frontend (ob-poc-ui-react)

> **Current split:** React owns structural application UI. `observatory-wasm`
> owns the high-frequency constellation canvas. The older `ob-poc-ui` and
> `esper_egui` crates remain deprecated.

### Architecture — Cockpit Layout

The primary UI is a **cockpit layout**: the egui WASM constellation canvas occupies the center column (always visible), chat messages and panels occupy the right column. The canvas is the "windscreen" — navigation happens by interacting with nodes directly (hover, click, double-click).

```
ob-poc-ui-react/
├── src/
│   ├── api/              # API client (chat.ts, scope.ts, semOs.ts, observatory.ts)
│   ├── features/
│   │   ├── chat/         # Cockpit UI: egui canvas center + chat & panels right
│   │   ├── observatory/  # Full-screen Observatory (standalone option at /observatory/:id)
│   │   ├── inspector/    # Projection inspector (tree + detail)
│   │   └── settings/     # App settings
│   ├── stores/           # Zustand state management
│   ├── types/            # TypeScript types (observatory.ts, chat.ts)
│   └── lib/              # Utilities, query client
├── dist/                 # Production build (served by Rust)
└── package.json
```

**ChatPage cockpit layout:**
```
[Sessions w-64] | [egui Canvas flex-1     ] | [Chat + Panels w-[28rem]]
                  [FlightDeck status bar   ]   [Messages (scrollable)  ]
                  [Canvas (60fps WASM)     ]   [ChatInput              ]
                                               [Scope, Constellation   ]
                                               [Narration, Verbs       ]
```

- egui canvas renders `GraphSceneModel` from Observatory API (polled 5s)
- Canvas actions (drill, select, zoom) route through standard REPL input pipeline
- FlightDeck defaults to collapsed 1-line status bar (expand on click for lens controls)
- At session start, canvas shows the universe root with 8 satellites:
  7 workspace nodes plus `new-session`
- `SessionFeedback` populated from session creation with scoping verbs

### egui Canvas Contract

The egui canvas now uses a shared derived layout model for:

- paint
- hit testing
- hover inspection
- selection/drill
- minimap and anchor helpers

Renderers no longer rely on one geometry path while interaction uses another.
Hover and inspection are only considered correct when both consume the same
cached node shapes/positions.

### Key Endpoints (Backend → React)

| Endpoint | Purpose |
|----------|---------|
| `POST /api/session` | Create agent session (with optional `workflow_focus`) |
| `GET /api/session/:id` | Get session with messages |
| `POST /api/session/:id/input` | Unified session input ingress (`kind=utterance|decision_reply|repl_v2`) |
| `GET /api/session/:id/scope-graph` | Get loaded CBUs (scope) |
| `GET /api/cbu/:id/graph` | Get single CBU's entity graph |
| `GET /api/cbu/:id/constellation` | Get hydrated constellation tree for a CBU/case |
| `GET /api/cbu/:id/constellation/summary` | Get constellation summary metrics |
| `GET /api/cbu/:id/cases` | List available KYC cases for constellation binding |
| `GET /api/constellation/by-name` | Resolve a CBU by name and hydrate its constellation |
| `GET /api/constellation/search-cbus` | Search CBUs for constellation workflows |
| `GET /api/projections/:id` | Get Inspector projection |
| `GET /api/sem-os/context` | Get Semantic OS registry stats + recent changesets |

### Unified Input Cutover (2026-03-05)

- **Single ingress:** All user prompts/utterances from chat UI and REPL UI now post to `POST /api/session/:id/input`.
- **Adapter dispatch:** Request `kind` selects server adapter path:
  - `utterance` → agent chat orchestration
  - `decision_reply` → DecisionPacket reply handling
  - `repl_v2` → REPL V2 orchestrator input handling
- **Legacy hard block (enforced):** `POST /api/session/:id/chat`, `POST /api/session/:id/decision/reply`, and `POST /api/repl/v2/session/:id/input` return `410 Gone`.
- **Tracing requirement:** endpoint adoption is validated via component-level call-stack trace (UI callsite → API client → HTTP endpoint → server route → adapter).

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

### Constellation Panel

The chat right rail now also includes a session-integrated constellation surface:
- **Shared CBU focus**: Uses the same selected CBU as the scope panel
- **Case binding**: Lists available `"ob-poc".cases` for the selected CBU and defaults to the newest case when present
- **Server-side state**: Renders reducer-derived slot state from `/api/cbu/:id/constellation`
- **Ownership chain**: Shows graph node/edge payloads for `entity_graph` slots in the slot inspector
- **Agent loop closure**: "Ask Agent" and "Ask Why Blocked" buttons submit structured prompts through the same `POST /api/session/:id/input` chat path

### Semantic OS Tab

The `/semantic-os` route provides an agent-driven workflow for Semantic OS operations. Sessions start with a workflow selection prompt that constrains which verbs the pipeline considers via SemReg `phase_tags` filtering.

```
┌─────────────────────────────────────────────────────────────────────┐
│  [Sessions]  │  [Chat Messages]                    │  [Context]     │
│              │                                     │                │
│  Session 1   │  Agent: What would you like to      │  Registry Stats│
│  Session 2   │  work on?                           │  ├─ Attrs: 120 │
│  + New       │  [Onboarding] [KYC]                 │  ├─ Verbs: 340 │
│              │  [Data Mgmt] [Stewardship]          │  └─ ...        │
│              │                                     │  Changesets    │
│              │  [Input: Type a message...]         │  └─ Draft: 3   │
└─────────────────────────────────────────────────────────────────────┘
```

**Workflow Selection Flow:**
1. User clicks "New Session" → `POST /api/session { workflow_focus: "semantic-os" }`
2. Backend returns `DecisionPacket` with 4 workflow choices (Onboarding, KYC, Data Management, Stewardship)
3. User selects workflow → sets `session.context.stage_focus = "semos-{workflow}"`
4. All subsequent utterances flow through SemReg with `goals = ["{workflow}"]`
5. `filter_and_rank_verbs()` filters to verbs whose `phase_tags` overlap with goals

**Goals → Phase Tags Pipeline:**
```
stage_focus "semos-kyc" → goals ["kyc"] → ContextResolutionRequest
    → filter_and_rank_verbs(goals=["kyc"])
    → verb.phase_tags contains "kyc"? include + boost
    → ContextEnvelope { allowed_verbs } → IntentPipeline
```

**Key Files:**

| File | Purpose |
|------|---------|
| `ob-poc-ui-react/src/features/semantic-os/SemOsPage.tsx` | 3-column layout (sidebar, chat, context panel) |
| `ob-poc-ui-react/src/features/semantic-os/components/SemOsSidebar.tsx` | Session list with own localStorage (`ob-poc-semos-sessions`) |
| `ob-poc-ui-react/src/features/semantic-os/components/SemOsContextPanel.tsx` | Registry stats + recent changesets (auto-refresh 10s) |
| `ob-poc-ui-react/src/api/semOs.ts` | API client for `GET /api/sem-os/context` |
| `rust/src/api/agent_routes.rs` | Session creation with `workflow_focus` + DecisionPacket |
| `rust/src/sem_reg/context_resolution.rs` | `filter_and_rank_verbs()` with goals→phase_tags filtering |

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
| TOML parse error with curly quotes | Copy-paste from docs | Replace `\u201c\u201d` with `"`, `\u2018\u2019` with `'` |
| "expected newline" in TOML | Malformed string escaping | Check `autoclose_before` and bracket strings |

### config.toml Gotchas

**NEVER use curly/smart quotes** - they break TOML parsing:
```toml
# WRONG - curly quotes from copy-paste
autoclose_before = "]\u201c\u2018\u201d"
brackets = [{ start = "\u201c", end = "\u201d" }]

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

## ESPER Navigation Crates (065)

> **DEPRECATED (2026-01-31):** The esper_* crates (`esper_snapshot`, `esper_core`, `esper_input`,
> `esper_policy`, `esper_egui`) are retained in the repository for reference but are no longer
> used by any active code path. All visualization is now handled by the React frontend
> (`ob-poc-ui-react/`) with data served via REST API endpoints.

**Crates retained for reference:** `esper_snapshot`, `esper_core`, `esper_input`, `esper_policy`, `esper_egui`, `esper_compiler`

For historical implementation details, see git history or `ai-thoughts/065-esper-navigation.md`.

---
